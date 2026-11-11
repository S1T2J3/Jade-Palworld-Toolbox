//! These tests only read the committed public research corpus. No real save path is used.
use jade_saves::{read_terminal_files,PlayerFile};
use sha2::{Sha256,Digest};
use std::{collections::BTreeMap,path::PathBuf};

fn fixture()->PathBuf{
    std::env::var_os("JADE_SAVE_FIXTURES").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../terminal-import/parser/research"))
}
fn canonical(s:&str)->String{s.to_lowercase().replace('-',"")}
fn hash(path:&std::path::Path)->String{format!("{:x}",Sha256::digest(std::fs::read(path).unwrap()))}
fn modern_players(root:&std::path::Path)->Vec<PlayerFile>{
    ["00000000000000000000000000000001","74E02BF7000000000000000000000000"].into_iter().map(|id|PlayerFile{id:id.into(),path:root.join(format!("modern-Players-{id}.sav"))}).collect()
}

#[test]
#[ignore="requires the public research corpus; set JADE_SAVE_FIXTURES"]
fn modern_matches_independent_inventory_with_party_and_unchanged_bytes(){
    let root=fixture();let level=root.join("modern-Level.sav");let players=modern_players(&root);
    let before:Vec<_>=std::iter::once(&level).chain(players.iter().map(|p|&p.path)).map(|p|hash(p)).collect();
    let result=read_terminal_files(&[level.clone()],&players).unwrap();
    let source=std::env::var_os("JADE_EXPECTED_PLAYERS").map(PathBuf::from).unwrap_or_else(||root.join("../../../qa-v114/expected-players.json"));
    let expected:serde_json::Value=serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
    let mut wanted=BTreeMap::new();
    for owner in expected["players"].as_array().unwrap(){
        for pal in owner["pals"].as_array().unwrap().iter().filter(|p|matches!(p["location"].as_str(),Some("storage"|"party"))){
            wanted.insert(canonical(pal["instanceId"].as_str().unwrap()),serde_json::json!({
                "ownerId":canonical(owner["playerUId"].as_str().unwrap()),"speciesKey":pal["species"],"name":pal["nickname"].as_str().unwrap_or(""),
                "sex":match pal["gender"].as_str(){Some("Male")=>"M",Some("Female")=>"F",_=>"unknown"},"traits":pal["passiveSkills"].as_array().cloned().unwrap_or_default()
            }));
        }
    }
    let actual:BTreeMap<_,_>=result.records.iter().map(|p|(p.instance_id.clone(),serde_json::json!({"ownerId":p.owner_id,"speciesKey":p.species_key,"name":p.name,"sex":p.sex,"traits":p.traits}))).collect();
    assert_eq!(actual,wanted);assert_eq!(actual.len(),result.records.len());
    assert_eq!(result.owners.len(),2);assert!(result.warnings.is_empty(),"{:?}",result.warnings);
    assert_eq!(result,read_terminal_files(&[level.clone()],&players).unwrap());
    assert_eq!(before,std::iter::once(&level).chain(players.iter().map(|p|&p.path)).map(|p|hash(p)).collect::<Vec<_>>());
}

#[test]
#[ignore="requires the public research corpus; set JADE_SAVE_FIXTURES"]
fn cnk_split_and_rejected_incomplete_or_duplicate_parts(){
    let root=fixture();let players=modern_players(&root);let source=root.join("modern-Level.sav");
    let expected=read_terminal_files(&[source.clone()],&players).unwrap();
    let bytes=std::fs::read(&source).unwrap();let split=bytes.len()/2;
    let temp=tempfile::tempdir().unwrap();let first=temp.path().join("first");let second=temp.path().join("second");
    let header=[0,0,0,0,0,0,0,0,b'C',b'N',b'K',b'0'];
    std::fs::write(&first,[header.as_slice(),&bytes[..split]].concat()).unwrap();
    std::fs::write(&second,[header.as_slice(),&bytes[split..]].concat()).unwrap();
    assert_eq!(expected,read_terminal_files(&[first.clone(),second.clone()],&players).unwrap());
    assert!(read_terminal_files(&[source.clone(),source.clone()],&players).is_err());
    std::fs::write(&second,[header.as_slice(),&bytes[split+8..]].concat()).unwrap();
    assert!(read_terminal_files(&[first,second],&players).is_err());
    assert!(read_terminal_files(&[source],&[]).is_err());
}

#[test]
#[ignore="requires the public research corpus; set JADE_SAVE_FIXTURES"]
fn legacy_empty_box_still_reads_carried_party_and_all_containers(){
    use std::io::{Read,Write};
    let root=fixture();let level=root.join("old-Level.sav");
    let players=[PlayerFile{id:"00000000000000000000000000000001".into(),path:root.join("old-00000000000000000000000000000001.sav")}];
    let expected=read_terminal_files(&[level.clone()],&players).unwrap();assert!(!expected.records.is_empty());
    let data=std::fs::read(level).unwrap();let mut inner=vec![];flate2::read::ZlibDecoder::new(&data[12..]).read_to_end(&mut inner).unwrap();
    let mut raw=vec![];flate2::read::ZlibDecoder::new(inner.as_slice()).read_to_end(&mut raw).unwrap();
    let mut z=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::default());z.write_all(&raw).unwrap();let zipped=z.finish().unwrap();
    let mut plz=vec![];plz.extend_from_slice(&(raw.len() as u32).to_le_bytes());plz.extend_from_slice(&(zipped.len() as u32).to_le_bytes());plz.extend_from_slice(b"PlZ1");plz.extend_from_slice(&zipped);
    let temp=tempfile::tempdir().unwrap();let path=temp.path().join("single");
    for payload in [raw,plz,data.clone(),[&[0,0,0,0,0,0,0,0,b'C',b'N',b'K',b'0'][..],data.as_slice()].concat()] {
        std::fs::write(&path,payload).unwrap();assert_eq!(expected,read_terminal_files(&[path.clone()],&players).unwrap());
    }
}
