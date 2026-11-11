use crate::SaveResult;
use std::{fs, io::{Cursor, Read}, path::{Path, PathBuf}};
use uesave::{games::palworld::{Palworld, palworld_types}, Save, SaveReader};

pub const MAX_WORLD: usize = 1024 * 1024 * 1024;
pub const MAX_PLAYER: usize = 64 * 1024 * 1024;
pub const MAX_META: usize = 2 * 1024 * 1024;

pub fn local_metadata(path: &Path) -> SaveResult<fs::Metadata> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("无法读取 {}：{e}", path.display()))?;
    if metadata.file_type().is_symlink() { return Err("存档路径不能是符号链接。".into()); }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & (0x400 | 0x1000) != 0 { return Err("存档为链接或尚未下载到本地。".into()); }
    }
    Ok(metadata)
}

pub fn read_file(path: &Path, limit: usize) -> SaveResult<Vec<u8>> {
    let before=local_metadata(path)?;
    if !before.is_file() || before.len()<12 || before.len()>limit as u64 { return Err("存档大小无效或超出读取上限。".into()); }
    let mut result=Vec::with_capacity(before.len() as usize);
    fs::File::open(path).map_err(|e| e.to_string())?.take(limit as u64+1).read_to_end(&mut result).map_err(|e|e.to_string())?;
    let after=local_metadata(path)?;
    if before.len()!=result.len() as u64 || before.len()!=after.len() || before.modified().ok()!=after.modified().ok() {
        return Err("游戏正在写入存档，请保存完成后重新读取。".into());
    }
    Ok(result)
}

pub fn join_parts(paths: &[PathBuf], limit: usize) -> SaveResult<Vec<u8>> {
    if paths.is_empty() || paths.len()>512 {return Err("存档缺少分块或分块数异常。".into());}
    let mut result=Vec::new();
    for (i,path) in paths.iter().enumerate() {
        let bytes=read_file(path,limit.saturating_sub(result.len()))?;
        let mut offset=0;
        if i>0 {
            if bytes.starts_with(b"GVAS") || matches!(bytes.get(8..11),Some(b"PlM"|b"PlZ")) {return Err("分块清单包含多个完整存档。".into());}
            if bytes.get(8..11)==Some(b"CNK") {
                if matches!(bytes.get(20..23),Some(b"PlM"|b"PlZ")) {return Err("分块清单包含多个完整存档。".into());}
                offset=12;
            }
        }
        result.extend_from_slice(&bytes[offset..]);
    }
    Ok(result)
}

fn inflate(bytes:&[u8],limit:usize)->SaveResult<Vec<u8>> {
    let mut result=Vec::new();
    flate2::read::ZlibDecoder::new(bytes).take(limit as u64+1).read_to_end(&mut result).map_err(|e|format!("存档解压失败：{e}"))?;
    if result.len()>limit {return Err("存档解压大小超过上限。".into());}
    Ok(result)
}

pub fn decompress(bytes:&[u8],limit:usize)->SaveResult<Vec<u8>> {
    if bytes.starts_with(b"GVAS") {
        if bytes.len()>limit {return Err("存档解压大小超过上限。".into());}
        return Ok(bytes.to_vec());
    }
    let h=if bytes.get(8..11)==Some(b"CNK") {12}else{0};
    if bytes.len()<h+12 {return Err("存档压缩头不完整。".into());}
    let raw=u32::from_le_bytes(bytes[h..h+4].try_into().unwrap()) as usize;
    let compressed=u32::from_le_bytes(bytes[h+4..h+8].try_into().unwrap()) as usize;
    if raw==0 || raw>limit || compressed==0 || compressed>limit {return Err("存档压缩头大小无效。".into());}
    let payload=&bytes[h+12..];
    let output=match (&bytes[h+8..h+11], bytes[h+11]) {
        (b"PlM",0x31)=>{
            if payload.len()!=compressed {return Err("存档压缩数据不完整。".into());}
            let mut result=vec![0;raw];
            let count=oozextract::Extractor::new().read_from_slice(payload,&mut result).map_err(|e|format!("Mermaid 解压失败：{e}"))?;
            if count!=raw {return Err("Mermaid 解压大小不符。".into());}
            result
        },
        (b"PlZ",0x31)=>{
            if payload.len()!=compressed {return Err("存档压缩数据不完整。".into());}
            inflate(payload,limit)?
        },
        (b"PlZ",0x32)=>{
            let inner=inflate(payload,limit)?;
            if inner.len()!=compressed {return Err("存档内层压缩大小不符。".into());}
            inflate(&inner,limit)?
        },
        _=>return Err("尚未支持的存档压缩格式。".into())
    };
    if output.len()!=raw || !output.starts_with(b"GVAS") {return Err("存档解压长度或 GVAS 标识无效。".into());}
    Ok(output)
}

pub fn parse_bytes(bytes:&[u8],limit:usize)->SaveResult<Save<Palworld>> {
    let raw=decompress(bytes,limit)?;
    // Unrelated structures change between game versions. Preserve them as opaque
    // data; terminal.rs still requires every imported container/character to be
    // typed and complete, so a degraded terminal never becomes an empty snapshot.
    std::panic::catch_unwind(||SaveReader::new().game::<Palworld>().types(palworld_types()).error_to_raw(true).log(std::env::var_os("JADE_SAVE_PARSE_LOG").is_some()).read(Cursor::new(raw)))
        .map_err(|_|"存档解析遇到无效内容，已中止读取。".to_string())?
        .map_err(|e|format!("GVAS 解析失败：{e}"))
}
pub fn parse_file(path:&Path,limit:usize)->SaveResult<Save<Palworld>> {parse_bytes(&read_file(path,limit)?,limit)}
