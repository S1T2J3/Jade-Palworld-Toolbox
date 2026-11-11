use jade_core::*;
use serde_json::Value;

fn equivalent(a:&Value,b:&Value)->bool{
    match(a,b){
        (Value::Number(a),Value::Number(b))=>{let (a,b)=(a.as_f64().unwrap(),b.as_f64().unwrap());(a-b).abs()<=1e-9*a.abs().max(b.abs()).max(1.0)},
        (Value::Array(a),Value::Array(b))=>a.len()==b.len()&&a.iter().zip(b).all(|(a,b)|equivalent(a,b)),
        (Value::Object(a),Value::Object(b))=>a.len()==b.len()&&a.iter().all(|(k,a)|b.get(k).is_some_and(|b|equivalent(a,b))),
        _=>a==b,
    }
}

#[test]
fn native_matches_v1_0_17_reference(){
    let cases:Vec<Value>=serde_json::from_str(include_str!("golden.json")).unwrap();
    for (i,case) in cases.iter().enumerate(){let input=&case["input"];
        let output=match case["kind"].as_str().unwrap(){
            "traits"=>trait_solve(input),
            "plan"=>plan_request(input),
            "offspring"=>offspring_solve(input),
            "warehouse"=>warehouse_solve(input["pairs"].as_array().unwrap(),input["speciesCount"].as_u64().unwrap() as usize,&input["owned"].as_array().unwrap().iter().map(|i|i.as_u64().unwrap() as usize).collect::<Vec<_>>(),Some(input["maxSteps"].as_u64().unwrap() as usize)),
            "warehouseRoute"=>serde_json::json!(warehouse_route(&input["plan"],input["target"].as_u64().unwrap() as usize,input["includeOwned"]==true)),
            "stats"=>calculate_pal_stats(&input["profile"],&input["config"],&input["passive"]),
            "capture"=>estimate_capture(input),
            "craft"=>plan_item_craft(&input["items"],input["target"].as_str().unwrap(),input["quantity"].as_u64().unwrap(),&input["choices"]).unwrap(),
            "work"=>calculate_work_efficiency(&input["data"],input["job"].as_str().unwrap(),input["level"].as_u64().unwrap() as usize,&input["keys"].as_array().unwrap().iter().map(|k|k.as_str().unwrap().into()).collect::<Vec<_>>()).unwrap(),
            "relic"=>relic_product_odds(&input["tier"],&input["pools"]),
            "relicSim"=>{let mut draws=input["draws"].as_array().unwrap().iter();simulate_relic(&input["tier"],&input["pools"],||draws.next().unwrap().as_f64().unwrap())},
            other=>panic!("Unknown kind {other}"),
        };
        assert!(equivalent(&output,&case["expected"]),"case {i} {} input={}\nactual={}\nexpected={}",case["kind"],input,output,case["expected"]);
    }
}
