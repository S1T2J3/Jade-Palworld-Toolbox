//! Distinguish how a Pal gets a passive, a naturally learned attack, or an egg move.
use super::{
    AssistantReply, Data, array, has, mentioned_traits, named, reply, species_mentions, text,
    trait_entry,
};
use serde_json::Value;
use std::collections::VecDeque;

pub(in crate::assistant_engine) fn profile_question(d: &Data, q: &str) -> Option<AssistantReply> {
    if let Some(answer) = inherited_options(d, q) {return Some(answer);}
    // A title names the subject. Other Pals and items in the body may be examples.
    let title = q
        .split(['?', '？', '\n'])
        .find(|s| !s.trim().is_empty())
        .unwrap_or(q);
    if has(
        title,
        &[
            "遗传",
            "配种",
            "繁殖",
            "从哪来",
            "怎么获得",
            "怎么出",
            "怎么刷",
            "获取",
            "刷出",
            "保底",
            "拿到",
            "获得",
            "way to get",
            "how do i get",
            "how to get",
            "breed",
            "pass down",
            "roll ",
        ],
    ) && !has(
        title,
        &[
            "概率多大",
            "几率多大",
            "百分之",
            "模拟",
            "calculate",
            "odds of",
            "combine",
            "叠加",
            "配方材料",
        ],
    ) && let [id] = mentioned_traits(d, title).as_slice()
        && let Some(result) = passive_source(d, title, id)
    {
        return Some(result);
    }
    let egg_moves = has(
        title,
        &[
            "egg move",
            "遗传招式",
            "遗传主动",
            "遗传技能",
            "breed moves",
            "breed attacks",
        ],
    );
    // Level, learning and the name of the move can occur in separate clauses.
    let request = q.lines().next().unwrap_or(q);
    let asks_level = has(request, &["几级","多少级","什么等级","哪个等级","几等","多少等","what level","which level","at level","at what lvl"]);
    let learns = (asks_level && has(request, &["学","练","领悟","解锁","learn","unlock","level up"])) || has(
        title,
        &[
            "自学",
            "自然学",
            "升级学",
            "几级学",
            "自己学",
            "learnset",
            "learn by level",
            "naturally learn",
            "what level",
            "learn at",
        ],
    );
    let signature = has(
        q,
        &[
            "专属招",
            "专用招",
            "专属技",
            "signature attack",
            "signature move",
            "exclusive move",
            "exclusive attack",
            "怎么放",
            "how do i execute",
            "how do i use",
            "怎么发动",
            "怎么使出",
        ],
    ) && has(q, &["招", "攻击", "技", "attack", "move", "dash"]);
    if !egg_moves && !learns && !signature {
        return None;
    }
    let (spans, _) = species_mentions(&d.species, if learns {request} else {title});
    let [(index, _, _)] = spans.as_slice() else {
        return None;
    };
    let pal = &d.species[*index];
    let key = text(pal, "key");
    let details = d.get("pal-details-data");
    let levels = array(&details["profiles"][key]["levels"]);
    let moves = details["moves"].as_object()?;
    let specific: Vec<_> = moves
        .iter()
        .filter(|(_, row)| named(request, text(row, "zh")) || named(request, text(row, "en")))
        .collect();
    if specific.len() > 1 {
        return None;
    }
    if !egg_moves
        && !signature
        && specific.is_empty()
        && !has(title, &["招式", "技能", "learnset", "move", "attack"])
    {
        return None;
    }
    let mut lines = Vec::new();
    let mut links = vec![(text(pal, "zh").into(), format!("#paldex/{key}"))];
    if let Some((id, row)) = specific.first() {
        let label = format!("{} ({})", text(row, "zh"), text(row, "en"));
        if let Some(entry) = levels.iter().find(|r| r[1].as_str() == Some(id.as_str())) {
            lines.push(format!(
                "{}升到 Lv.{} 就会学到{label}。",
                text(pal, "zh"),
                entry[0]
            ));
        } else {
            lines.push(format!("{}的升级招式表没有{label}。", text(pal, "zh")));
        }
        if let Some(fruit) = row["fruit"].as_str() {
            lines.push(format!("图鉴列出的教学道具是「{fruit}」。"));
        }
        if has(request,&["冷却","间隔","多久一次","cooldown","recharge","how often"," cd"])
            && let Some(seconds)=row["cooldown"].as_u64() {
            lines.push(format!("{label} 的基础冷却为 {seconds} 秒。"));
        }
        if has(request,&["威力","power","招式面板","move stats"])
            && let Some(power)=row["power"].as_u64() {
            lines.push(format!("基础威力 {power}。"));
        }
        if egg_moves {
            lines.push(
                match row["inherit"].as_bool()? {
                    true => "招式表标记为可遗传。",
                    false => "招式表标记为不可遗传。",
                }
                .into(),
            );
        }
        links.push((label, format!("#details/active/move/{id}")));
    } else {
        for entry in levels {
            let id = entry[1].as_str()?;
            if signature && !egg_moves && !id.starts_with("Unique_") {
                continue;
            }
            let row = moves.get(id)?;
            let label = format!("{} ({})", text(row, "zh"), text(row, "en"));
            lines.push(format!("Lv.{}  {label}", entry[0]));
            links.push((label, format!("#details/active/move/{id}")));
        }
        if lines.is_empty() {
            return None;
        }
        lines.insert(
            0,
            format!(
                "{} ({}) 可通过升级学会以下{}。",
                text(pal, "zh"),
                text(pal, "en"),
                if signature && !egg_moves {
                    "专用主动技"
                } else {
                    "主动技"
                }
            ),
        );
        if egg_moves {
            lines.push("上面这些招式都不需要靠配种获得。要补特定招式时，可再查它有没有技能果实、能不能遗传。".into());
        }
    }
    if signature && !egg_moves {
        lines.push("先在帕鲁详情把已学的招式装入主动技栏。可骑乘的帕鲁在骑乘时按该栏位对应的键发动，冷却结束才能再放。".into());
    }
    Some(reply(lines, links))
}

/// Recommend only moves with positive inheritance flags and the requested teaching method.
fn inherited_options(d:&Data,q:&str)->Option<AssistantReply>{
    if !has(q,&["遗传","继承","inherit","breed"]) ||
        !has(q,&["没有果实","无果实","没果实","没有技能果实","无技能果实","no fruit","without fruit","no skill fruit","without a skill fruit"]) ||
        !has(q,&["推荐","哪些","什么","强","值得","which","recommend","best","strong"]){return None;}
    if !species_mentions(&d.species,q).0.is_empty(){return None;}
    let details=d.get("pal-details-data");
    let types=d.get("combat-guide-data")["elements"].as_object()?;
    let selected:Vec<_>=types.iter().filter(|(key,label)|{
        let zh=label.as_str().unwrap_or("");
        (!zh.is_empty() && (q.contains(&format!("{zh}属性")) || q.contains(&format!("{zh}系"))))
            || q.split(|c:char|!c.is_ascii_alphabetic()).any(|w|w==key.as_str())
    }).map(|(key,_)|key).collect();
    let mut candidates:Vec<_>=details["moves"].as_object()?.iter().filter(|(id,m)|{
        m["inherit"]==true && m["fruit"].as_str().is_none_or(str::is_empty)
            && m["power"].as_u64().is_some_and(|p|p>0)
            && (selected.is_empty() || selected.iter().any(|k|m["type"]==k.as_str()))
            && !id.to_ascii_lowercase().contains("self") && !id.starts_with("Unique_")
    }).collect();
    candidates.sort_by_key(|(id,m)|(std::cmp::Reverse(m["power"].as_u64()),m["cooldown"].as_u64(),id.as_str()));
    let profiles=details["profiles"].as_object()?;
    let mut lines=Vec::new();
    let mut links=Vec::new();
    for (id,m) in candidates{
        let Some(parent)=d.species.iter().find(|pal|profiles.get(text(pal,"key")).is_some_and(|p|
            array(&p["levels"]).iter().any(|row|row[1]==id.as_str()))) else {continue;};
        let name=text(m,"zh");
        lines.push(format!("{name}（{}）：基础威力 {}，冷却 {} 秒。可从会这招的{}（{}）亲本开始。",text(m,"en"),m["power"],m["cooldown"],text(parent,"zh"),text(parent,"en")));
        links.push((name.into(),format!("#details/active/move/{id}")));
        if lines.len()==3{break;}
    }
    if lines.is_empty(){return None;}
    lines.insert(0,"按本版图鉴筛选了标记可遗传、未列对应技能果实的招式。以下优先考虑单招威力，实战还要看冷却和对手属性。".into());
    lines.push("把想传的招式装备在亲本的主动栏，孵出后检查子代；传承不是每颗蛋必定触发。".into());
    Some(reply(lines,links))
}

fn passive_source(d: &Data, title: &str, id: &str) -> Option<AssistantReply> {
    let details = d.get("pal-details-data");
    let row = &details["passives"][id];
    let label = text(row, "zh");
    let random = row["random"].as_bool()?;
    // Short passive names such as "Lucky", "Serious" or "Rare" can describe a
    // Pal, a schematic or an item without referring to a passive at all.
    let explicit = has(
        title,
        &[
            "词条",
            "被动",
            "配种",
            "遗传",
            "passive",
            "trait",
            "pass down",
            "roll ",
        ],
    );
    if !explicit
        && (has(
            title,
            &[
                "schematic",
                "blueprint",
                "设计图",
                "稀有帕鲁",
                "闪光",
                "lucky pal",
                "lucky pals",
            ],
        ) || text(row, "en").split_whitespace().count() == 1 && title.is_ascii())
    {
        return None;
    }
    let (detail, link) = trait_entry(d, id, row["surgery"].is_object())?;
    let mut links = vec![link];
    let mut lines = Vec::new();
    let donors: Vec<_> = d
        .species
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            array(&details["profiles"][text(s, "key")]["fixed"])
                .iter()
                .any(|t| t.as_str() == Some(id))
        })
        .map(|(i, _)| i)
        .collect();
    if random {
        lines.push(format!("「{label}」在随机被动池中，野外捕获或孵蛋都可能得到。图鉴没有给连续落空后的保底次数。有了带该词条的亲本，可选它配种并检查子代有没有继承。"));
    } else if !donors.is_empty() {
        lines.push(format!(
            "「{label}」不在普通随机池。图鉴登记的固定携带物种有{}。",
            donors
                .iter()
                .map(|&i| text(&d.species[i], "zh"))
                .collect::<Vec<_>>()
                .join("、")
        ));
    } else {
        lines.push(format!(
            "「{label}」不在普通随机被动池，无法从这个标记推断具体来源。"
        ));
    }
    let (pals, _) = species_mentions(&d.species, title);
    let needs_route = has(title, &["遗传", "配种", "繁殖", "breed", "pass down"]);
    if needs_route && pals.len() > 1 {
        return None;
    } // Which Pal is the donor matters.
    if !random
        && needs_route
        && let [(target, _, _)] = pals.as_slice()
    {
        // This graph models species and ordinary transferable exclusive passives.
        // It cannot infer transfer eligibility for mutation or scenario-only passives.
        if !(id.starts_with("ElementBoost_") || id == "Legend") {
            return None;
        }
        if donors.contains(target) {
            lines.push("目标物种本身就有这条固定词条，可以先取得携带它的个体。".into());
        } else if let Some(steps) = carrier_path(&d.pairs, d.species.len(), &donors, *target) {
            lines.push("假设能取得每一步的配偶，以下是按繁殖代数搜索的一条最短路线。".into());
            for edge in steps {
                let parents = [0, 1].map(|i| {
                    let pal = &d.species[edge[i].as_u64().unwrap() as usize];
                    let sex = match edge.get(i + 3).and_then(Value::as_str) {
                        Some("M") => "♂",
                        Some("F") => "♀",
                        _ => "",
                    };
                    format!("{}{sex}", text(pal, "zh"))
                });
                let child = &d.species[edge[2].as_u64()? as usize];
                lines.push(format!(
                    "{} × {} → {}",
                    parents[0],
                    parents[1],
                    text(child, "zh")
                ));
                links.push((
                    text(child, "zh").into(),
                    format!("#lookup/parents/{}", text(child, "key")),
                ));
            }
            lines.push(format!(
                "每代都要选实际继承「{label}」的子代，和下一只异性亲本配对；一次孵化不保证继承。"
            ));
        } else {
            lines.push("当前配种表中，从这些固定携带者出发没有通往目标物种的传递路线。".into());
        }
    }
    lines.push(detail);
    if text(&row["surgery"], "mode") == "consume" {
        lines.push("如果手上已有相应的耗材植入体，也可在帕鲁手术台指定植入这条被动。".into());
    }
    Some(reply(lines, links))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn search_starts_from_any_carrier_and_preserves_sex_specific_recipes() {
        let pairs = vec![
            json!([0, 1, 2, "F", "M"]),
            json!([3, 2, 4]),
            json!([6, 6, 6]),
        ];
        let path = carrier_path(&pairs, 7, &[6, 0], 4).unwrap();
        assert_eq!(path, vec![&pairs[0], &pairs[1]]);
        assert!(carrier_path(&pairs, 7, &[6], 4).is_none());
        assert!(carrier_path(&pairs, 7, &[6, 4], 4).unwrap().is_empty());
    }
}

/// Multi-source BFS treats each recipe as an edge from either carrier to its child.
fn carrier_path<'a>(
    pairs: &'a [Value],
    size: usize,
    donors: &[usize],
    target: usize,
) -> Option<Vec<&'a Value>> {
    let mut edges = vec![Vec::new(); size];
    for (i, p) in pairs.iter().enumerate() {
        for k in [0, 1] {
            edges.get_mut(p[k].as_u64()? as usize)?.push(i);
        }
    }
    let mut queue: VecDeque<_> = donors.iter().copied().collect();
    let mut visited = vec![false; size];
    for &id in donors {
        visited[id] = true;
    }
    let mut parent = vec![None; size];
    while let Some(current) = queue.pop_front() {
        if current == target {
            break;
        }
        for &edge in &edges[current] {
            let child = pairs[edge][2].as_u64()? as usize;
            if !visited[child] {
                visited[child] = true;
                parent[child] = Some((current, edge));
                queue.push_back(child);
            }
        }
    }
    if !visited[target] {
        return None;
    }
    let mut path = Vec::new();
    let mut cursor = target;
    while let Some((previous, edge)) = parent[cursor] {
        path.push(&pairs[edge]);
        cursor = previous;
    }
    path.reverse();
    Some(path)
}
