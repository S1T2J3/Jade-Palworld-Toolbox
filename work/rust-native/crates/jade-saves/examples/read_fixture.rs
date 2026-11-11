//! Explicit JSON file interface for repeatable public-fixture latency comparisons.
//! Never discovers or reads the machine's default SaveGames folders.
use serde::Deserialize;
use std::path::PathBuf;
#[derive(Deserialize)]
struct Input{level:Vec<PathBuf>,players:Vec<jade_saves::PlayerFile>}
fn main(){
    let Some(path)=std::env::args_os().nth(1)else{eprintln!("Usage: read_fixture <explicit-fixture-manifest.json>");std::process::exit(2)};
    let run=||->Result<_,String>{let input:Input=serde_json::from_slice(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;jade_saves::read_terminal_files(&input.level,&input.players)};
    match run(){Ok(result)=>println!("{}",serde_json::to_string(&result).unwrap()),Err(e)=>{println!("{}",serde_json::json!({"error":e}));std::process::exit(1)}}
}
