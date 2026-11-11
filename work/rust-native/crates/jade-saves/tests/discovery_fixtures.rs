use jade_saves::{scan_from_locations,read_terminal,read_terminal_files,PlayerFile};
use std::{path::{Path,PathBuf},fs};

fn work()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")}
fn write(path:&Path,data:&[u8]){fs::create_dir_all(path.parent().unwrap()).unwrap();fs::write(path,data).unwrap();}
fn text(out:&mut Vec<u8>,s:&str){let words=s.encode_utf16().collect::<Vec<_>>();out.extend_from_slice(&(words.len() as u32).to_le_bytes());for w in words{out.extend_from_slice(&w.to_le_bytes());}}
fn guid(n:u8)->[u8;16]{let mut b=[0;16];b[15]=n;b}
fn gn(n:u8)->String{format!("000000000000000000000000000000{n:02X}")}
struct Blob<'a>{n:u8,name:String,state:u32,data:&'a[u8]}
fn profile(account:&Path,package:&str,blobs:&[Blob<'_>]){
    let mut index=14u32.to_le_bytes().to_vec();index.extend_from_slice(&(blobs.len() as u32).to_le_bytes());text(&mut index,"幻兽帕鲁");text(&mut index,&format!("{package}!App"));index.extend_from_slice(&133669549210000000i64.to_le_bytes());index.extend_from_slice(&0u32.to_le_bytes());text(&mut index,"fixture");index.extend_from_slice(&0i64.to_le_bytes());
    for b in blobs{
        text(&mut index,&b.name);text(&mut index,&b.name);text(&mut index,"");index.push(7);index.extend_from_slice(&b.state.to_le_bytes());index.extend_from_slice(&guid(b.n));index.extend_from_slice(&133669549210000000i64.to_le_bytes());index.extend_from_slice(&0i64.to_le_bytes());index.extend_from_slice(&(b.data.len() as i64).to_le_bytes());
        let folder=account.join(gn(b.n));let mut manifest=4u32.to_le_bytes().to_vec();manifest.extend_from_slice(&1u32.to_le_bytes());let mut name=[0;128];name[..8].copy_from_slice(&[b'D',0,b'a',0,b't',0,b'a',0]);manifest.extend_from_slice(&name);manifest.extend_from_slice(&guid(b.n+40));manifest.extend_from_slice(&guid(b.n+40));write(&folder.join("container.7"),&manifest);write(&folder.join(gn(b.n+40)),b.data);
    }
    write(&account.join("containers.index"),&index);
}

#[test]
#[ignore="requires public LevelMeta fixtures"]
fn scan_isolated_steam_server_unicode_backups_and_identity(){
    let temp=tempfile::tempdir().unwrap();let local=temp.path().join("本机资料");let library=temp.path().join("游戏库");
    let default=local.join("Pal/Saved/SaveGames");let world="13579BDF24684ABCACBD0123456789FE";
    let legacy=fs::read(work().join("game-saves/discovery-ref/legacy.sav")).unwrap();
    let modern=fs::read(work().join("qa-v113/palhelm-modern.sav")).unwrap();
    let a=default.join("76561111111111111").join(world);let b=default.join("76561111111111112").join(world);
    let server=library.join("steamapps/common/PalServer/Pal/Saved/SaveGames/0").join(world);
    write(&a.join("LevelMeta.sav"),&legacy);write(&b.join("LevelMeta.sav"),&legacy);write(&server.join("LevelMeta.sav"),&modern);
    write(&a.join("backup/world/2026-05-30/LevelMeta.sav"),&legacy);
    let scan=scan_from_locations(None,&local,&[library]).unwrap();assert_eq!(scan.saves.len(),3,"{:?}",scan);
    assert_eq!(scan.saves.iter().filter(|s|s.platform=="Steam").count(),2);assert_eq!(scan.saves.iter().filter(|s|s.platform=="专用服务器").count(),1);
    let first=scan.saves.iter().find(|s|s.path==a).unwrap();assert_eq!(first.name,"Snacks Local to Dedi Test");
    for path in [&a,&a.join("LevelMeta.sav"),a.parent().unwrap(),&default,&local.join("Pal")]{let scan=scan_from_locations(Some(path),&local,&[]).unwrap();assert!(scan.saves.iter().any(|s|s==first),"{}: {:?}",path.display(),scan);}
    assert!(scan_from_locations(Some(&a.join("backup")),&local,&[]).unwrap().saves.is_empty());
    let raw=if legacy.get(8..11)==Some(b"PlZ") {use std::io::Read;let mut raw=Vec::new();flate2::read::ZlibDecoder::new(&legacy[12..]).read_to_end(&mut raw).unwrap();raw}else{legacy.clone()};
    let replaced=raw.windows(first.name.len()).position(|w|w==first.name.as_bytes()).unwrap();let mut renamed=raw;renamed[replaced..replaced+first.name.len()].copy_from_slice(&vec![b'A';first.name.len()]);write(&a.join("LevelMeta.sav"),&renamed);
    let changed=scan_from_locations(Some(&a),&local,&[]).unwrap();assert_eq!(changed.saves[0].id,first.id);assert_ne!(changed.saves[0].name,first.name);
}

#[test]
#[ignore="requires public modern save fixtures"]
fn xbox_scan_and_native_terminal_read_match_folder_and_reject_ambiguous_blobs(){
    let temp=tempfile::tempdir().unwrap();let local=temp.path().join("local");let root=local.join("Packages/PocketpairInc.Palworld_ad4psfrxyesvt/SystemAppData/wgs");let account=root.join("ABCD0123456789_01234567000000000000000000000001");
    let sources=work().join("terminal-import/parser/research");
    let meta=fs::read(work().join("qa-v113/palhelm-modern.sav")).unwrap();let level=fs::read(sources.join("modern-Level.sav")).unwrap();
    let p1=fs::read(sources.join("modern-Players-00000000000000000000000000000001.sav")).unwrap();let p2=fs::read(sources.join("modern-Players-74E02BF7000000000000000000000000.sav")).unwrap();
    let world="29A1F9DAD5D84501A3B719B7C3BCDE6B";
    let h=[0,0,0,0,0,0,0,0,b'C',b'N',b'K',b'0'];let cut=level.len()/2;let first=[h.as_slice(),&level[..cut]].concat();let second=[h.as_slice(),&level[cut..]].concat();
    profile(&account,"PocketpairInc.Palworld_ad4psfrxyesvt",&[
        Blob{n:1,name:format!("{world}-LevelMeta"),state:0,data:&meta},
        Blob{n:2,name:format!("{world}-Level-0"),state:0,data:&first},Blob{n:3,name:format!("{world}-Level-1"),state:0,data:&second},
        Blob{n:4,name:format!("{world}-Players-00000000000000000000000000000001"),state:0,data:&p1},Blob{n:5,name:format!("{world}-Players-74E02BF7000000000000000000000000"),state:0,data:&p2},
        Blob{n:6,name:format!("{world}-LevelMeta"),state:3,data:&meta},Blob{n:7,name:format!("{world}-Slot1-LevelMeta"),state:0,data:&meta}
    ]);
    profile(&root.join("Backup"),"PocketpairInc.Palworld_ad4psfrxyesvt",&[Blob{n:1,name:format!("{world}-LevelMeta"),state:0,data:&meta}]);
    profile(&root.join("AAAA_1111"),"OtherGame_other",&[Blob{n:1,name:format!("{world}-LevelMeta"),state:0,data:&meta}]);
    let scan=scan_from_locations(None,&local,&[]).unwrap();assert_eq!(scan.saves.len(),1,"{:?}",scan);assert!(scan.warnings.is_empty());
    let players=[PlayerFile{id:"00000000000000000000000000000001".into(),path:sources.join("modern-Players-00000000000000000000000000000001.sav")},PlayerFile{id:"74E02BF7000000000000000000000000".into(),path:sources.join("modern-Players-74E02BF7000000000000000000000000.sav")}];
    let expected=read_terminal_files(&[sources.join("modern-Level.sav")],&players).unwrap();assert_eq!(expected,read_terminal(&scan.saves[0]).unwrap());
    for selected in [&account.join(gn(2)),&account.join(gn(4)),&account.join("containers.index"),root.parent().unwrap()]{assert_eq!(scan_from_locations(Some(selected),&local,&[]).unwrap().saves.len(),1);}
    assert!(scan_from_locations(Some(&account.join(gn(7))),&local,&[]).unwrap().saves.is_empty());
    let blob=account.join(gn(1));let manifest=blob.join("container.7");let mut bytes=fs::read(&manifest).unwrap();bytes[152..168].copy_from_slice(&guid(99));write(&manifest,&bytes);write(&blob.join(gn(99)),&meta);
    let broken=scan_from_locations(None,&local,&[]).unwrap();assert!(broken.saves.is_empty());assert!(!broken.warnings.is_empty());
}
