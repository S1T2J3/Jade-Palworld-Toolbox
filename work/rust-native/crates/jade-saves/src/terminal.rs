use crate::{container::{local_metadata,parse_file,parse_bytes,join_parts,MAX_PLAYER,MAX_WORLD},discovery::{GameWorld,make_id,describe_world,same_path},props::{self,Property,StructValue},wgs,SaveResult};
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::{collections::{BTreeMap,HashMap,HashSet},path::{Path,PathBuf}};
use uesave::games::palworld::PalStruct;

const HOST:&str="00000000000000000000000000000001";
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct PlayerFile {pub id:String,pub path:PathBuf}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct TerminalOwner {pub id:String,pub name:String,#[serde(default,skip_serializing_if="std::ops::Not::not")] pub is_local:bool}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct TerminalRecord {pub instance_id:String,pub species_key:String,pub name:String,pub sex:String,pub traits:Vec<String>,pub owner_id:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct TerminalResult {pub owners:Vec<TerminalOwner>,pub records:Vec<TerminalRecord>,pub warnings:Vec<String>}

fn snapshot(levels:&[PathBuf],players:&[PlayerFile])->SaveResult<String> {
    let mut hash=Sha256::new();
    for path in levels.iter().chain(players.iter().map(|p|&p.path)) {
        let meta=local_metadata(path)?;
        if !meta.is_file(){return Err("清单中包含无效存档。".into());}
        hash.update(path.to_string_lossy().as_bytes());hash.update([0]);hash.update(meta.len().to_le_bytes());
        hash.update(meta.modified().map_err(|e|e.to_string())?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos().to_le_bytes());
    }
    Ok(format!("{:x}",hash.finalize()))
}

pub fn read_terminal_files(levels:&[PathBuf],players:&[PlayerFile])->SaveResult<TerminalResult>{
    if players.is_empty()||players.len()>2048{return Err("缺少 Players 玩家存档，无法确认归属。".into());}
    let before=snapshot(levels,players)?;
    let mut owners=Vec::new();let mut owner_index=HashMap::new();let mut boxes=HashMap::new();
    let mut warnings=Vec::new();
    for file in players {
        let player=parse_file(&file.path,MAX_PLAYER)?;
        let uid=props::get(&player.root.properties,&["SaveData","PlayerUId"]).and_then(props::guid).ok_or("玩家存档缺少角色编号。")?;
        if !file.id.is_empty() && (!props::valid_id(&file.id)||props::canonical(&file.id)!=uid){return Err("玩家文件名与角色编号不符，请重新检测。".into());}
        if owner_index.insert(uid.clone(),owners.len()).is_some(){return Err("多个玩家存档使用同一角色编号。".into());}
        owners.push(TerminalOwner{id:uid.clone(),name:String::new(),is_local:false});
        for container in ["PalStorageContainerId","OtomoCharacterContainerId"] {
            if let Some(id)=props::get(&player.root.properties,&["SaveData",container,"ID"]).and_then(props::guid) {
                if let Some(prior)=boxes.insert(id,uid.clone()){if prior!=uid{return Err("一个容器出现两个玩家归属。".into());}}
            }else {warnings.push(format!("玩家 {} 缺少 {container} 编号。",file.id));}
        }
    }
    let bytes=join_parts(levels,MAX_WORLD)?;
    let level=parse_bytes(&bytes,MAX_WORLD)?;
    let world=props::get(&level.root.properties,&["worldSaveData"]).and_then(props::bag).ok_or("所选存档缺少世界数据。")?;
    let containers=props::get(world,&["CharacterContainerSaveData"]).and_then(props::maps).ok_or("世界存档缺少帕鲁容器数据。")?;
    let mut wanted:HashMap<String,(String,i32)>=HashMap::new();let mut found=HashSet::new();
    for entry in containers {
        let Some(id)=props::bag(&entry.key).and_then(|b|props::get(b,&["ID"])).and_then(props::guid)else{continue};
        let Some(owner)=boxes.get(&id)else{continue};found.insert(id);
        let bag=props::bag(&entry.value).ok_or("帕鲁容器结构无效。")?;
        let Some(slots)=props::get(bag,&["Slots"])else{continue};
        let slots=props::structs(slots).ok_or("帕鲁容器格位结构无效。")?;
        for slot in slots {
            let StructValue::Struct(bag)=slot else{return Err("帕鲁格位数据无效。".into());};
            let mut id=props::get(bag,&["IndividualId","InstanceId"]).and_then(props::guid);
            if let Some(raw)=props::get(bag,&["RawData"]){match raw {
                Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))=>{if !raw.instance_id.is_nil(){id=Some(props::canonical(&raw.instance_id.to_string()));}},
                Property::Array(props::ValueVec::Byte(uesave::ByteArray::Byte(bytes))) if bytes.is_empty()=>{},
                _=>return Err("帕鲁格位 RawData 无法识别，未导入不完整快照。".into())
            }}
            if let Some(id)=id {
                let index=props::get(bag,&["SlotIndex"]).and_then(props::integer).unwrap_or(0);
                if let Some((prior,_))=wanted.insert(id,(owner.clone(),index)){if &prior!=owner{return Err("同一帕鲁出现在不同玩家的容器中。".into());}}
            }
        }
    }
    if found.len()<boxes.len(){return Err(format!("有 {} 个玩家容器尚未在世界文件中找到，未导入不完整快照，请在游戏保存完成后重试。",boxes.len()-found.len()));}
    let characters=props::get(world,&["CharacterSaveParameterMap"]).and_then(props::maps).ok_or("世界存档缺少帕鲁个体数据。")?;
    let mut records=Vec::new();
    for entry in characters {
        let Some(key)=props::bag(&entry.key)else{continue};
        let uid=props::get(key,&["PlayerUId"]).and_then(props::guid).unwrap_or_default();
        let instance=props::get(key,&["InstanceId"]).and_then(props::guid).unwrap_or_default();
        if !owner_index.contains_key(&uid)&&!wanted.contains_key(&instance){continue;}
        let raw=props::bag(&entry.value).and_then(|b|props::get(b,&["RawData"])).ok_or("帕鲁个体 RawData 缺失。")?;
        let Property::Struct(StructValue::Game(PalStruct::CharacterData(raw)))=raw else{return Err("帕鲁个体数据无法识别。".into());};
        let param=props::get(&raw.object,&["SaveParameter"]).and_then(props::bag).ok_or("帕鲁个体参数无法识别。")?;
        let name=props::get(param,&["NickName"]).and_then(props::text).unwrap_or("").to_string();
        if props::get(param,&["IsPlayer"]).and_then(props::boolean).unwrap_or(false) {
            if let Some(i)=owner_index.get(&uid){owners[*i].name=name;}continue;
        }
        let Some((owner,slot))=wanted.remove(&instance)else{continue};
        let species=props::get(param,&["CharacterID"]).and_then(props::text).filter(|s|!s.is_empty()).ok_or("帕鲁物种编号缺失。")?;
        let gender=props::get(param,&["Gender"]).and_then(props::text).unwrap_or("").rsplit("::").next().unwrap_or("").to_ascii_lowercase();
        let sex=match gender.as_str(){"male"=>"M","female"=>"F",_=>"unknown"};
        let traits=props::get(param,&["PassiveSkillList"]).and_then(props::strings).cloned().unwrap_or_default();
        records.push((slot,TerminalRecord{instance_id:instance,species_key:species.to_string(),name,sex:sex.into(),traits,owner_id:owner}));
    }
    if !wanted.is_empty(){return Err(format!("有 {} 个格位缺少个体资料，未导入不完整快照，请在游戏保存完成后重试。",wanted.len()));}
    if before!=snapshot(levels,players)?{return Err("游戏刚刚写入了新存档，请重新读取完整快照。".into());}
    records.sort_by(|(sa,a),(sb,b)|a.owner_id.cmp(&b.owner_id).then(sa.cmp(sb)).then(a.instance_id.cmp(&b.instance_id)));
    Ok(TerminalResult{owners,records:records.into_iter().map(|(_,r)|r).collect(),warnings})
}

fn files(world:&GameWorld)->SaveResult<(Vec<PathBuf>,Vec<PlayerFile>,bool)> {
    if !world.path.is_absolute(){return Err("游戏世界路径必须是绝对路径。".into());}
    local_metadata(&world.path)?;
    let mut parts=BTreeMap::new();
    let local;
    if world.platform=="Xbox / Game Pass" {
        let (id,account,rows)=wgs::world_parts(&world.path)?;
        if make_id(&world.platform,&account.file_name().unwrap_or_default().to_string_lossy(),&id)!=world.id{return Err("游戏世界已变化，请重新扫描。".into());}
        parts=rows;
        let actual=std::env::var_os("LOCALAPPDATA").map(|p|PathBuf::from(p).join("Packages").join(wgs::PACKAGE).join("SystemAppData/wgs"));
        local=actual.as_ref().is_some_and(|root|account.parent().is_some_and(|p|same_path(p,root))&&wgs::accounts(root).len()==1);
    } else {
        let (platform,account,id)=describe_world(&world.path)?;
        if make_id(&platform,&account,&id)!=world.id{return Err("游戏世界已变化，请重新扫描。".into());}
        local_metadata(&world.path.join("LevelMeta.sav"))?;
        for entry in std::fs::read_dir(&world.path).map_err(|e|e.to_string())?.flatten(){let path=entry.path();if path.extension().is_some_and(|s|s.eq_ignore_ascii_case("sav")){parts.insert(path.file_stem().unwrap().to_string_lossy().to_ascii_lowercase(),path);}}
        for (folder,label) in [("Level","level"),("Players","players")] {
            let directory=world.path.join(folder);if !directory.is_dir(){continue;}local_metadata(&directory)?;
            for entry in std::fs::read_dir(directory).map_err(|e|e.to_string())?.flatten(){let path=entry.path();if path.extension().is_some_and(|s|s.eq_ignore_ascii_case("sav")){parts.insert(format!("{label}/{}",path.file_stem().unwrap().to_string_lossy().to_ascii_lowercase()),path);}}
        }
        local=platform=="Steam"&&local_steam_world(&world.path);
    }
    let lr=regex::Regex::new(r"(?i)^level(?:[-/\\](\d{1,4}))?$").unwrap();
    let pr=regex::Regex::new(r"(?i)^players[-/\\]([a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})$").unwrap();
    let mut levels=BTreeMap::new();let mut players=BTreeMap::new();
    for (part,path) in parts {
        if let Some(c)=lr.captures(&part){let i=c.get(1).map(|m|m.as_str().parse::<i32>().unwrap()).unwrap_or(-1);if levels.insert(i,path).is_some(){return Err("世界存档分段重复。".into());}}
        else if let Some(c)=pr.captures(&part){let id=props::canonical(&c[1]);if players.insert(id.clone(),PlayerFile{id,path}).is_some(){return Err("玩家存档重复。".into());}}
    }
    if levels.is_empty()||players.is_empty(){return Err("缺少 Level 或 Players 文件，请选择主机上的完整世界目录。".into());}
    Ok((levels.into_values().collect(),players.into_values().collect(),local))
}

fn local_steam_world(world:&Path)->bool {
    let Some(account)=world.parent()else{return false};let Some(root)=account.parent()else{return false};
    let Some(local)=std::env::var_os("LOCALAPPDATA")else{return false};
    if !same_path(root,&PathBuf::from(local).join("Pal/Saved/SaveGames"))||!world.join("LocalData.sav").is_file(){return false;}
    let account_name=account.file_name().unwrap_or_default().to_string_lossy();
    #[cfg(windows)] if let Ok(key)=winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam\\ActiveProcess") {
        if let Ok(active)=key.get_value::<u32,_>("ActiveUser"){if active!=0{return account_name==(76561197960265728u64+active as u64).to_string();}}
    }
    let accounts:Vec<_>=std::fs::read_dir(root).into_iter().flatten().flatten().filter(|e|e.file_type().is_ok_and(|t|t.is_dir())).map(|e|e.file_name().to_string_lossy().to_string()).filter(|s|s.len()==17&&s.bytes().all(|b|b.is_ascii_digit())).collect();
    accounts.len()==1&&accounts[0]==account_name
}

pub fn read_terminal(world:&GameWorld)->SaveResult<TerminalResult>{
    let (levels,players,local)=files(world)?;
    let before=snapshot(&levels,&players)?;
    let mut result=read_terminal_files(&levels,&players)?;
    let (after_levels,after_players,_)=files(world)?;
    if before!=snapshot(&after_levels,&after_players)?{return Err("存档文件列表已变化，请重新读取。".into());}
    if local {for owner in &mut result.owners{owner.is_local=owner.id==HOST;}}
    Ok(result)
}
