use jade_core::*;
use serde_json::{json,Value};
use std::time::Instant;
fn main(){
    let path=std::env::args().nth(1).expect("JSON cases file");let cases:Vec<Value>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();let mut results=Vec::new();
    for case in cases{let mut samples=Vec::new();let mut output=Value::Null;
        for repeat in 0..7{let begin=Instant::now();output=match case["kind"].as_str().unwrap(){"plan"=>plan_request(&case["input"]),"offspring"=>offspring_solve(&case["input"]),_=>trait_solve(&case["input"])};let ms=begin.elapsed().as_secs_f64()*1000.0;if repeat>1{samples.push(ms)}}
        samples.sort_by(f64::total_cmp);results.push(json!({"name":case["name"],"samplesMs":samples,"medianMs":samples[2],"output":output}));
    }println!("{}",json!(results));
}
