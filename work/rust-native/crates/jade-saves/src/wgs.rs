//! Read-only Xbox WGS index and blob resolver. Never follows backup/reparse trees.
use crate::{container::{local_metadata, read_file}, props::canonical, SaveResult};
use std::{collections::BTreeMap, io::{Cursor, Read}, path::{Path, PathBuf}};

pub const PACKAGE:&str="PocketpairInc.Palworld_ad4psfrxyesvt";
#[derive(Clone,Debug)]
pub struct Entry {pub world:String,pub part:String,pub folder:String,pub generation:u8,pub timestamp:i64}
fn bytes<const N:usize>(r:&mut Cursor<&[u8]>)->SaveResult<[u8;N]>{let mut v=[0;N];r.read_exact(&mut v).map_err(|_|"WGS 索引不完整。".to_string())?;Ok(v)}
fn u32r(r:&mut Cursor<&[u8]>)->SaveResult<u32>{Ok(u32::from_le_bytes(bytes(r)?))}
fn i64r(r:&mut Cursor<&[u8]>)->SaveResult<i64>{Ok(i64::from_le_bytes(bytes(r)?))}
fn string(r:&mut Cursor<&[u8]>)->SaveResult<String>{
    let n=u32r(r)? as usize;if n>65535{return Err("WGS 字符串长度无效。".into());}
    let mut v=vec![0;n*2];r.read_exact(&mut v).map_err(|e|e.to_string())?;
    String::from_utf16(&v.chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).collect::<Vec<_>>()).map(|s|s.trim_end_matches('\0').to_string()).map_err(|e|e.to_string())
}
fn windows_guid(b:[u8;16])->String {
    format!("{:08X}{:04X}{:04X}{}",u32::from_le_bytes(b[..4].try_into().unwrap()),u16::from_le_bytes(b[4..6].try_into().unwrap()),u16::from_le_bytes(b[6..8].try_into().unwrap()),b[8..].iter().map(|b|format!("{b:02X}")).collect::<String>())
}
pub fn index(account:&Path)->SaveResult<Vec<Entry>> {
    local_metadata(account)?;
    let data=read_file(&account.join("containers.index"),16*1024*1024)?;
    let mut r=Cursor::new(data.as_slice());
    if u32r(&mut r)?!=14{return Err("WGS 索引版本不支持。".into());}
    let count=u32r(&mut r)?;string(&mut r)?;let app=string(&mut r)?;
    if !app.split('!').next().unwrap_or("").eq_ignore_ascii_case(PACKAGE){return Ok(vec![]);}
    i64r(&mut r)?;u32r(&mut r)?;string(&mut r)?;i64r(&mut r)?;
    if count as usize>(data.len().saturating_sub(r.position() as usize))/57 {return Err("WGS 条目数量无效。".into());}
    let regex=regex::Regex::new(r"(?i)^([0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})[-/\\](.+)$").unwrap();
    let mut result=vec![];
    for _ in 0..count {
        let logical=string(&mut r)?;string(&mut r)?;string(&mut r)?;
        let generation=bytes::<1>(&mut r)?[0];let state=u32r(&mut r)?;let folder=windows_guid(bytes(&mut r)?);
        let timestamp=i64r(&mut r)?;i64r(&mut r)?;i64r(&mut r)?;
        if state==3{continue;}
        if let Some(c)=regex.captures(&logical){result.push(Entry{world:canonical(&c[1]).to_ascii_uppercase(),part:c[2].to_string(),folder,generation,timestamp});}
    }
    Ok(result)
}

pub fn blob(account:&Path,entry:&Entry)->SaveResult<PathBuf> {
    let folder=account.join(&entry.folder);local_metadata(&folder)?;
    let data=read_file(&folder.join(format!("container.{}",entry.generation)),2*1024*1024)?;
    let mut r=Cursor::new(data.as_slice());
    if u32r(&mut r)?!=4{return Err("WGS 清单版本不支持。".into());}
    let count=u32r(&mut r)?;
    if count==0 || count as usize>(data.len()-8)/160{return Err("WGS 清单条目数量无效。".into());}
    let mut found=None;
    for _ in 0..count {
        let b=bytes::<128>(&mut r)?;
        let name=String::from_utf16(&b.chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).collect::<Vec<_>>()).map_err(|e|e.to_string())?;
        let cloud=windows_guid(bytes(&mut r)?);let local=windows_guid(bytes(&mut r)?);
        if count>1 && !name.trim_end_matches('\0').eq_ignore_ascii_case("Data"){continue;}
        if found.is_some(){return Err("WGS 数据清单存在歧义。".into());}found=Some((cloud,local));
    }
    let (cloud,local)=found.ok_or("WGS 数据文件缺失。")?;
    let live=folder.join(&local);let fallback=folder.join(&cloud);
    let has_live=local!="00000000000000000000000000000000"&&live.is_file();
    let has_cloud=cloud!="00000000000000000000000000000000"&&fallback.is_file();
    if (!has_live&&!has_cloud)||(has_live&&has_cloud&&local!=cloud){return Err("WGS 数据文件缺失或同步版本不明确。".into());}
    let path=if has_live{live}else{fallback};local_metadata(&path)?;Ok(path)
}

pub fn accounts(path:&Path)->Vec<(PathBuf,Option<String>)> {
    let root=if path.is_file(){path.parent().unwrap_or(path)}else{path};
    if root.join("containers.index").is_file(){return vec![(root.to_path_buf(),None)];}
    if let Some(parent)=root.parent(){if parent.join("containers.index").is_file(){return vec![(parent.to_path_buf(),root.file_name().map(|x|x.to_string_lossy().to_string()))];}}
    let root=if root.join("wgs").is_dir(){root.join("wgs")}else if root.join("SystemAppData/wgs").is_dir(){root.join("SystemAppData/wgs")}else{root.to_path_buf()};
    if !root.file_name().is_some_and(|s|s.eq_ignore_ascii_case("wgs")){return vec![];}
    let account_name=regex::Regex::new(r"(?i)^[0-9a-f]{1,32}_[0-9a-f-]{1,64}$").unwrap();
    std::fs::read_dir(root).into_iter().flatten().flatten().filter_map(|e|{
        let path=e.path();
        (account_name.is_match(&e.file_name().to_string_lossy())&&local_metadata(&path).is_ok_and(|m|m.is_dir())&&path.join("containers.index").is_file()).then_some((path,None))
    }).collect()
}

pub fn world_parts(metadata_folder:&Path)->SaveResult<(String,PathBuf,BTreeMap<String,PathBuf>)> {
    let account=metadata_folder.parent().ok_or("WGS 世界路径无效。")?;
    let selected=metadata_folder.file_name().ok_or("WGS 世界路径无效。")?.to_string_lossy();
    let entries=index(account)?;
    let world=entries.iter().find(|e|e.folder.eq_ignore_ascii_case(&selected)&&e.part.eq_ignore_ascii_case("LevelMeta")).ok_or("Xbox 世界已迁移，请重新扫描。")?.world.clone();
    let mut known:BTreeMap<String,(String,u8,PathBuf)>=BTreeMap::new();
    for e in entries.iter().filter(|e|e.world==world) {
        let key=e.part.to_ascii_lowercase();
        if !(key=="level"||key.starts_with("level-")||key.starts_with("level/")||key.starts_with("level\\")||key.starts_with("players-")||key.starts_with("players/")||key.starts_with("players\\")){continue;}
        if let Some((folder,generation,_))=known.get(&key) {if folder!=&e.folder||*generation!=e.generation{return Err("Xbox 存档有多个同步版本，请等待云同步完成。".into());}}
        known.insert(key,(e.folder.clone(),e.generation,blob(account,e)?));
    }
    Ok((world,account.to_path_buf(),known.into_iter().map(|(k,(_,_,v))|(k,v)).collect()))
}
