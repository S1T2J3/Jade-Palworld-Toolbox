use serde_json::{json,Value};
use std::collections::{HashMap,HashSet};

fn arr(v:&Value)->&[Value]{v.as_array().map(Vec::as_slice).unwrap_or(&[])}

/// Shared intermediate requirements are combined before rounding to batches.
/// Choices is an object keyed by item id; -1 means acquire directly.
pub fn plan_item_craft(items:&Value,target:&str,quantity:u64,choices:&Value)->Option<Value>{
    let by_id:HashMap<String,Value>=if let Some(rows)=items.as_array(){rows.iter().filter_map(|r|r["id"].as_str().map(|id|(id.to_string(),r.clone()))).collect()}
        else {items.as_object()?.iter().map(|(id,v)|(id.clone(),v.clone())).collect()};
    if quantity==0||quantity>9_007_199_254_740_991||!by_id.contains_key(target){return None}
    struct Node{ id:String,index:i64,recipe:Option<Value>,blocked:HashSet<String> }
    fn visit(id:&str,by_id:&HashMap<String,Value>,choices:&Value,pending:&mut HashSet<String>,nodes:&mut Vec<Node>,order:&mut Vec<String>){
        if nodes.iter().any(|n|n.id==id){return}pending.insert(id.into());
        let recipes=by_id.get(id).map(|r|arr(&r["recipes"])).unwrap_or(&[]);
        let selected=choices[id].as_i64().filter(|i|*i>=-1&&*i<recipes.len() as i64);
        let index=selected.unwrap_or_else(||recipes.iter().position(|r|arr(&r["materials"]).iter().all(|m|!pending.contains(m[0].as_str().unwrap_or("")))).map(|i|i as i64).unwrap_or(-1));
        let recipe=if index>=0{recipes.get(index as usize).cloned()}else{None};
        let materials=recipe.as_ref().map(|r|arr(&r["materials"]).to_vec()).unwrap_or_default();
        let node_index=nodes.len();nodes.push(Node{id:id.into(),index,recipe,blocked:HashSet::new()});
        for material in materials{let m=material[0].as_str().unwrap_or("");if pending.contains(m){nodes[node_index].blocked.insert(m.into());}else{visit(m,by_id,choices,pending,nodes,order)}}
        pending.remove(id);order.push(id.into());
    }
    let mut nodes=Vec::new();let mut order=Vec::new();visit(target,&by_id,choices,&mut HashSet::new(),&mut nodes,&mut order);
    let mut need=HashMap::from([(target.to_string(),quantity as f64)]);let mut external:Vec<(String,f64)>=Vec::new();let mut steps=Vec::new();let mut cycles=Vec::new();
    fn add(v:&mut Vec<(String,f64)>,id:&str,count:f64){if let Some((_,n))=v.iter_mut().find(|(k,_)|k==id){*n+=count}else{v.push((id.into(),count))}}
    for id in order.iter().rev(){let node=nodes.iter().find(|n|n.id==*id).unwrap();let required=need.get(id).copied().unwrap_or(0.0);
        let Some(recipe)=&node.recipe else{add(&mut external,id,required);continue};
        let output=recipe["qty"].as_f64().unwrap_or(1.0);if output<=0.0{return None}let batches=(required/output).ceil();
        let inputs:Vec<Value>=arr(&recipe["materials"]).iter().map(|m|json!([m[0],m[1].as_f64().unwrap_or(0.0)*batches])).collect();
        for material in &inputs{let m=material[0].as_str().unwrap_or("");let count=material[1].as_f64().unwrap_or(0.0);
            if node.blocked.contains(m){cycles.push(json!([id,m,count]));add(&mut external,m,count)}else{*need.entry(m.into()).or_default()+=count}
        }
        steps.push(json!({"id":id,"required":required,"batches":batches,"made":batches*output,"inputs":inputs,"recipe":recipe}));
    }
    steps.reverse();let nodes:Vec<Value>=nodes.iter().map(|n|{let mut blocked:Vec<_>=n.blocked.iter().cloned().collect();blocked.sort();json!({"id":n.id,"index":n.index,"recipe":n.recipe,"blocked":blocked})}).collect();
    Some(json!({"steps":steps,"external":external,"cycles":cycles,"nodes":nodes}))
}
