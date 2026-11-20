use super::{AssistantReply, Data, has, name_span, text};
use std::cmp::Ordering;

pub(in crate::assistant_engine) fn compare(data:&Data, q:&str)->Option<AssistantReply> {
    let power = has(q,&["面板威力","技能威力","威力","base power"]);
    let cooldown = has(q,&["冷却","冷卻","cooldown"]);
    if !(power || cooldown) || !has(q,&["两招","两种","哪个","哪一","哪招","哪种","谁","相比","比较","分别","which","compare"]) ||
        has(q,&["几级学","哪里买","怎么获得","学习果实","武器配方","模组","modded"]) {return None;}
    let moves=data.get("pal-details-data")["moves"].as_object()?;
    let mut matched=Vec::new();
    for (id,row) in moves {
        if let Some((start,end))=[text(row,"zh"),text(row,"en")].iter().filter_map(|label|name_span(q,label)).max_by_key(|(start,end)|end-start) {
            matched.push((start,end,id,row));
        }
    }
    matched.sort_by_key(|(a,b,_,_)|std::cmp::Reverse(b-a));
    let mut picked=Vec::new();
    for (a,b,id,row) in matched {
        if picked.iter().any(|(x,y,_,_)|a==*x&&b==*y) {return None;}
        if picked.iter().all(|(x,y,_,_)| b<=*x||*y<=a) {picked.push((a,b,id,row));}
    }
    picked.sort_by_key(|entry|entry.0);
    let [(_,_,ida,a),(_,_,idb,b)]=picked.as_slice() else {return None;};
    let (na,nb)=(text(a,"zh"),text(b,"zh"));
    let mut sentences=Vec::new();
    for (requested,field,label,unit,larger) in [(power,"power","面板威力","","威力更高"),(cooldown,"cooldown","冷却时间","秒","冷却更久")] {
        if !requested {continue;}
        let (Some(x),Some(y))=(a[field].as_u64(),b[field].as_u64()) else {continue;};
        let result=match x.cmp(&y) {Ordering::Equal=>"两招相同".to_owned(),Ordering::Greater=>format!("{na}{larger}"),Ordering::Less=>format!("{nb}{larger}")};
        sentences.push(format!("{label}：{na} {x}{unit}，{nb} {y}{unit}，{result}。"));
    }
    if sentences.is_empty(){return None;}
    if has(q,&["实际伤害","最终伤害","每秒伤害","实战伤害"]) {sentences.push("实战伤害还需结合使用者、目标和命中情况。".into());}
    Some(AssistantReply{text:sentences.join("\n"),links:vec![(na.into(),format!("#details/active/move/{ida}")),(nb.into(),format!("#details/active/move/{idb}"))],suggestions:vec![]})
}
