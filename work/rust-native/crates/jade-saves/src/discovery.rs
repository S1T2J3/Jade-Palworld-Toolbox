use crate::{container::{local_metadata,parse_file,MAX_META}, props, wgs, SaveResult};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{collections::{BTreeSet,HashSet},path::{Path,PathBuf}};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct GameWorld {pub id:String,pub name:String,pub platform:String,pub path:PathBuf,pub modified:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ScanResult {pub state:String,pub saves:Vec<GameWorld>,pub warnings:Vec<String>,pub roots:Vec<PathBuf>,pub default_directory:PathBuf,pub default_directory_exists:bool}

pub(crate) fn make_id(platform:&str,account:&str,world:&str)->String{
    format!("pal-{:x}",Sha256::digest(format!("{platform}\0{account}\0{world}").to_uppercase().as_bytes()))
}
pub(crate) fn describe_world(path:&Path)->SaveResult<(String,String,String)>{
    let parent=path.parent().ok_or("世界目录缺少父目录。")?;
    let account=parent.file_name().unwrap_or_default().to_string_lossy();
    let name=path.file_name().unwrap_or_default().to_string_lossy();
    let world=if props::valid_id(&name){props::canonical(&name)}else{name.to_string()};
    let (platform,account)=if account=="0"{("专用服务器",parent.to_string_lossy().to_string())}
    else if account.is_empty()||account.len()>20||!account.bytes().all(|x|x.is_ascii_digit()){("本地存档",parent.to_string_lossy().to_string())}
    else{("Steam",account.to_string())};
    Ok((platform.to_string(),account,world))
}
fn is_backup(path:&Path)->bool {path.components().any(|p|{let n=p.as_os_str().to_string_lossy();n.eq_ignore_ascii_case("backup")||n.eq_ignore_ascii_case("backups")})}
fn dirs(path:&Path)->Vec<PathBuf>{std::fs::read_dir(path).into_iter().flatten().flatten().map(|e|e.path()).filter(|p|local_metadata(p).is_ok_and(|m|m.is_dir())).collect()}
fn world_name(path:&Path)->SaveResult<String>{
    let save=parse_file(path,MAX_META)?;
    let name=props::get(&save.root.properties,&["SaveData","WorldName"]).and_then(props::text).filter(|s|!s.is_empty()).ok_or("世界元数据缺少名称。")?;
    Ok(name.to_string())
}
fn stamp(path:&Path)->String {local_metadata(path).ok().and_then(|m|m.modified().ok()).map(|time|chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339_opts(chrono::SecondsFormat::Millis,true)).unwrap_or_default()}
fn warning(result:&mut ScanResult,text:String){if result.warnings.len()<64&&!result.warnings.contains(&text){result.warnings.push(text);}}
fn add(result:&mut ScanResult,known:&mut HashSet<String>,platform:&str,account:&str,world:&str,meta:&Path,path:&Path) {
    if result.saves.len()>=512 {warning(result,"本次最多列出 512 个世界，请选择单个存档目录。".into());return;}
    let id=make_id(platform,account,world);
    if known.contains(&id){return;}
    match world_name(meta) {
        Ok(name)=>{known.insert(id.clone());result.saves.push(GameWorld{id,name,platform:platform.to_string(),path:path.to_path_buf(),modified:stamp(meta)});},
        Err(e)=>warning(result,format!("无法读取世界元数据（{}）：{e}",path.display()))
    }
}
fn visit_world(result:&mut ScanResult,known:&mut HashSet<String>,path:&Path) {
    if is_backup(path)||local_metadata(path).is_err(){return;}
    let meta=path.join("LevelMeta.sav");
    if !meta.is_file(){
        if path.join("Level.sav").is_file(){warning(result,format!("世界目录缺少 LevelMeta.sav（{}）。",path.display()));}
        else if path.join("LocalData.sav").is_file(){warning(result,"部分联机世界只有本机玩家数据，请在主机或服务器上选择完整世界目录。".into());}
        return;
    }
    if let Ok((platform,account,world))=describe_world(path){add(result,known,&platform,&account,&world,&meta,path);}
}
fn scan_folder(result:&mut ScanResult,known:&mut HashSet<String>,selected:&Path) {
    let path=if selected.is_file(){selected.parent().unwrap_or(selected)}else{selected};
    if is_backup(path){warning(result,"所选目录属于备份，请选择当前正在使用的存档。".into());return;}
    if !path.is_dir(){return;}
    if !result.roots.iter().any(|p|same_path(p,path)){result.roots.push(path.to_path_buf());}
    if path.join("LevelMeta.sav").is_file()||props::valid_id(&path.file_name().unwrap_or_default().to_string_lossy()) {visit_world(result,known,path);return;}
    for suffix in ["SaveGames","Saved/SaveGames","Pal/Saved/SaveGames"] {
        let child=path.join(suffix);if child.is_dir(){scan_folder(result,known,&child);return;}
    }
    for child in dirs(path) {
        if is_backup(&child){continue;}
        if child.join("LevelMeta.sav").is_file()||props::valid_id(&child.file_name().unwrap_or_default().to_string_lossy()){visit_world(result,known,&child);}
        else if child.file_name().is_some_and(|s|s.to_string_lossy().bytes().all(|b|b.is_ascii_digit())) {
            for world in dirs(&child){visit_world(result,known,&world);}
        }
    }
}
pub(crate) fn same_path(a:&Path,b:&Path)->bool {a.to_string_lossy().trim_end_matches(['\\','/']).eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches(['\\','/']))}

pub fn scan_game_saves(selected:Option<&Path>)->SaveResult<ScanResult>{
    let local=std::env::var_os("LOCALAPPDATA").map(PathBuf::from).ok_or("无法找到本机存档目录。")?;
    scan_from_locations(selected,&local,&steam_libraries())
}

/// Dependency-injected locations for deterministic tests; never consults real save folders.
pub fn scan_from_locations(selected:Option<&Path>,local_app_data:&Path,steam_libraries:&[PathBuf])->SaveResult<ScanResult>{
    let default=local_app_data.join("Pal/Saved/SaveGames");
    let mut result=ScanResult{state:"ok".into(),saves:vec![],warnings:vec![],roots:vec![],default_directory_exists:default.is_dir(),default_directory:default.clone()};
    let mut known=HashSet::new();
    let xbox=if let Some(selected)=selected {
        if !selected.is_absolute(){return Err("请选择绝对路径。".into());}
        scan_folder(&mut result,&mut known,selected);selected.to_path_buf()
    }else{
        result.roots.push(default.clone());scan_folder(&mut result,&mut known,&default);
        for library in steam_libraries{scan_folder(&mut result,&mut known,&library.join("steamapps/common/PalServer/Pal/Saved/SaveGames"));}
        local_app_data.join("Packages").join(wgs::PACKAGE).join("SystemAppData/wgs")
    };
    if !is_backup(&xbox){
        if !result.roots.iter().any(|p|same_path(p,&xbox)){result.roots.push(xbox.clone());}
        for (account,selected_container) in wgs::accounts(&xbox) {
            let entries=match wgs::index(&account){Ok(v)=>v,Err(e)=>{warning(&mut result,e);continue;}};
            let selected_world=selected_container.as_ref().and_then(|s|entries.iter().find(|e|e.folder.eq_ignore_ascii_case(s)&&!e.part.to_ascii_lowercase().starts_with("slot")).map(|e|e.world.clone()));
            if selected_container.is_some()&&selected_world.is_none(){continue;}
            let mut seen=std::collections::HashMap::new();let mut ambiguous=HashSet::new();
            for e in entries.iter().filter(|e|e.part.eq_ignore_ascii_case("LevelMeta")&&selected_world.as_ref().is_none_or(|w|w==&e.world)){
                if let Some(old)=seen.insert(e.world.clone(),e){if old.folder!=e.folder||old.generation!=e.generation{ambiguous.insert(e.world.clone());}}
            }
            for (world,entry) in seen {
                if ambiguous.contains(&world){warning(&mut result,"Xbox 世界有多个同步版本，请等云同步完成。".into());continue;}
                match wgs::blob(&account,entry){
                    Ok(meta)=>{
                        add(&mut result,&mut known,"Xbox / Game Pass",&account.file_name().unwrap_or_default().to_string_lossy(),&world,&meta,meta.parent().unwrap());
                        if let Some(saved)=result.saves.iter_mut().find(|s|same_path(&s.path,meta.parent().unwrap())) {
                            if let Some(ticks)=entry.timestamp.checked_sub(116444736000000000) {
                                if let Some(time)=chrono::DateTime::<chrono::Utc>::from_timestamp(ticks.div_euclid(10_000_000),(ticks.rem_euclid(10_000_000)*100) as u32){saved.modified=time.to_rfc3339_opts(chrono::SecondsFormat::Millis,true);}
                            }
                        }
                    },
                    Err(e)=>warning(&mut result,e)
                }
            }
        }
    }
    result.saves.sort_by(|a,b|b.modified.cmp(&a.modified).then_with(||a.id.cmp(&b.id)));
    Ok(result)
}

fn steam_libraries()->Vec<PathBuf>{
    let mut paths=BTreeSet::new();
    if let Some(p)=std::env::var_os("ProgramFiles(x86)"){paths.insert(PathBuf::from(p).join("Steam"));}
    #[cfg(windows)] if let Ok(key)=winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(path)=key.get_value::<String,_>("SteamPath"){paths.insert(PathBuf::from(path));}
    }
    let regex=regex::Regex::new(r#"^\s*"path"\s*"((?:\\.|[^"\\])*)""#).unwrap();
    for root in paths.clone(){
        let path=root.join("steamapps/libraryfolders.vdf");
        if !local_metadata(&path).is_ok_and(|m|m.len()<1024*1024){continue;}
        if let Ok(text)=std::fs::read_to_string(path){for line in text.lines(){if let Some(c)=regex.captures(line){let path=PathBuf::from(c[1].replace("\\\\","\\").replace("\\\"","\""));if path.is_absolute(){paths.insert(path);}}}}
    }
    paths.into_iter().collect()
}
