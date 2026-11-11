use serde_json::{json, Value};

fn n(v: &Value, key: &str) -> f64 { v[key].as_f64().unwrap_or(0.0) }
fn pool_for<'a>(pools:&'a Value,key:&Value)->&'a Value {if let Some(index)=key.as_u64(){&pools[index as usize]}else{&pools[key.as_str().unwrap_or("")]}}

/// Preserves the game's float32 intermediate rounding, including the 944 HP
/// one-star, level-10 Lamball case (integer-percent arithmetic gives 945).
pub fn calculate_pal_stats(profile: &Value, config: &Value, passive: &Value) -> Value {
    let mut panel = json!({}); let mut expedition = json!({});
    let stars = (1.0 + 0.05 * n(config, "stars")) as f32;
    for (i, stat) in ["hp", "attack", "defense"].iter().enumerate() {
        let growth = profile["growth"][i].as_f64().unwrap_or(0.0);
        let trust = profile["trust"][i].as_f64().unwrap_or(0.0);
        let tenths = (growth * if config["awaken"].as_bool().unwrap_or(false) {11.0} else {10.0}
            + trust * 10.0 * n(config, "trust")).round();
        let level = n(config, "level");
        let scaled = tenths * level * (1000.0 + 3.0 * n(&config["iv"], stat));
        let base = if *stat == "hp" {500.0 + 5.0 * level + (scaled / 20000.0).floor()}
            else {(if *stat == "attack" {100.0} else {50.0}) + (scaled * 3.0 / 400000.0).floor()};
        let souls = (1.0 + 0.03 * n(&config["souls"], stat)) as f32;
        let train = |v: f32| ((v * souls) * stars).floor() as f64;
        expedition[*stat] = json!(train(base as f32));
        panel[*stat] = json!(train(base as f32 * (1.0 + n(passive, stat) / 100.0) as f32).max(0.0));
    }
    panel["work"] = json!(((70.0 + 7.0 * n(config,"stars")) * (100.0 + 3.0*n(&config["souls"],"work"))
        * (100.0+n(passive,"work")) / 10000.0).floor().max(0.0));
    let factor = (n(config,"stars") + 1.0).powi(2);
    let power = ((n(&expedition,"hp")/5.0).floor() + n(&expedition,"attack") + n(&expedition,"defense")) * factor;
    json!({"panel":panel,"expedition":expedition,"power":power,"factor":factor})
}

pub fn estimate_capture(config: &Value) -> Value {
    let hp=n(config,"hp"); let coefficient=n(config,"coefficient"); let world=n(config,"world");
    if config["blocked"].as_bool().unwrap_or(false) || hp<=0.0 || coefficient<=0.0 || world<=0.0 {
        return json!({"reticle":0.0,"chance":0.0});
    }
    let advantage=((n(config,"power")-n(config,"level")+50.0)/99.0).clamp(0.0,1.0);
    let score=advantage*1.3_f64.powf(-hp/100.0);
    let base=if score<0.5 {256.0*score.powi(9)} else {1.0-(2.0-2.0*score).powi(9)/2.0};
    let roll=(base*(1.0+n(config,"status"))*coefficient*world).clamp(0.0,1.0);
    json!({"reticle":roll.powf(5.0/12.0),"chance":roll.cbrt()})
}

pub fn calculate_work_efficiency(data: &Value, job: &str, level: usize, keys: &[String]) -> Result<Value,String> {
    let curve=data["levels"][job].as_array().ok_or("工作种类或等级无效")?;
    if level>curve.len(){return Err("工作种类或等级无效".into());}
    let mut selected=Vec::new();
    for key in keys {if !selected.contains(key){selected.push(key.clone());}}
    if selected.len()>4{return Err("最多选择四个词条".into());}
    let (mut work,mut ranch,mut movement)=(0.0,0.0,0.0);
    for key in selected {let m=&data["modifiers"][key]; work+=n(m,"work");ranch+=n(m,"ranch");movement+=n(m,"movement");}
    let rank=if level==0 {0} else {(level as f64+if job=="牧场"{ranch}else{0.0}).min(curve.len() as f64).max(0.0) as usize};
    let base=if rank==0 {0.0}else{curve[rank-1].as_f64().unwrap_or(0.0)};
    let amount=if job!="搬运" && job!="牧场" {(base*(100.0+work)/100.0).max(0.0)}else{base};
    Ok(json!({"rank":rank,"base":base,"work":work,"ranch":ranch,"movement":movement,"amount":amount}))
}

pub fn relic_product_odds(tier:&Value,pools:&Value)->Value {
    let mut products:Vec<Value>=Vec::new();
    for slot in tier["slots"].as_array().into_iter().flatten(){
        let pool=pool_for(pools,&slot["pool"]);
        let rows:Vec<&Value>=pool.as_array().into_iter().flatten().collect();
        let total:f64=rows.iter().map(|r|r[3].as_f64().unwrap_or(0.0)).sum();
        if total<=0.0{continue}
        for row in rows{
            let id=&row[0];let min=row[1].as_f64().unwrap_or(0.0);let max=row[2].as_f64().unwrap_or(0.0);
            let p=n(slot,"probability")/100.0*row[3].as_f64().unwrap_or(0.0)/total;
            let index=products.iter().position(|v|v["id"]==*id).unwrap_or_else(||{products.push(json!({"id":id,"probability":0.0,"min":min,"max":0.0,"slots":0}));products.len()-1});
            let product=&mut products[index];let probability=n(product,"probability");
            product["probability"]=json!(probability+(1.0-probability)*p);product["min"]=json!(n(product,"min").min(min));product["max"]=json!(n(product,"max")+max);product["slots"]=json!(product["slots"].as_u64().unwrap()+1);
        }
    }json!(products)
}

/// Caller supplies its RNG, enabling reproducible tests without global state.
pub fn simulate_relic(tier:&Value,pools:&Value,mut random:impl FnMut()->f64)->Value {
    let mut result:Vec<(String,u64)>=Vec::new();
    for slot in tier["slots"].as_array().into_iter().flatten(){
        if random()*100.0>=n(slot,"probability"){continue}
        let rows:Vec<&Value>=pool_for(pools,&slot["pool"]).as_array().into_iter().flatten().collect();
        let mut draw=random()*rows.iter().map(|r|r[3].as_f64().unwrap_or(0.0)).sum::<f64>();
        for row in rows{draw-=row[3].as_f64().unwrap_or(0.0);if draw>=0.0{continue}
            let min=row[1].as_u64().unwrap_or(0);let max=row[2].as_u64().unwrap_or(min);let amount=min+(random()*(max-min+1) as f64).floor() as u64;let id=row[0].as_str().unwrap_or("");
            if let Some((_,n))=result.iter_mut().find(|(k,_)|k==id){*n+=amount}else{result.push((id.into(),amount))}break
        }
    }json!(result)
}
