//! Look up the property on the named piece of gear. A shared display name may
//! represent several qualities with different stats.
use super::{AssistantReply, facility_effect, field_text};
use crate::data::{Data, array, text};

fn any(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| q.contains(term))
}

pub(super) fn answer(data: &Data, question: &str) -> Option<AssistantReply> {
    if !any(question, &["潜力", "才能", "御寒", "防寒", "耐寒", "耐冷", "保暖", "耐热", "防暑", "耐暑", "抗热",
        "减伤", "属性伤害", "抵抗火", "抵抗冰", "火抗", "冰抗"]) { return None; }
    if any(question, &["模组", "魔改", "mod版", "自制装备", "几级解锁", "哪里买", "如何制作", "怎么制作",
        "怎么获得", "哪里掉", "掉落概率", "掉落率", "多少钱", "哪里卖", "设计图", "配种", "遗传", "能否叠加", "怎么叠", "叠加吗"]) ||
        !crate::assistant_discovery::handbook_version_matches(question) { return None; }
    let title = question.lines().next()?;
    let named = facility_effect::items(data, title);
    let [item] = named.as_slice() else { return None };
    let name = text(item,"name");
    let requested = title.replace(&field_text(name), "");
    let items = array(&data.get("item-catalog-data")["items"]);
    let mut variants: Vec<_> = items.iter().filter(|r| text(r,"name")==name).collect();
    variants.sort_by_key(|r| r["rarity"].as_i64().unwrap_or(i64::MAX));
    if any(&requested,&["普通款","普通的","基础款","基础版","白色品质"]) || title.contains(&format!("普通{}",field_text(name))) {
        let min = variants.first()?["rarity"].as_i64()?;
        variants.retain(|r| r["rarity"].as_i64()==Some(min));
    }
    let first = text(variants.first()?,"id");
    let db = &data.get("item-effects-data")["items"];
    let description = text(&db[first],"description").trim();
    let mut facts = Vec::new();
    if any(&requested,&["潜力","才能"]) && any(description,&["潜力","隐藏才能"])
        && variants.iter().all(|r| text(&db[text(r,"id")],"description").trim()==description) {
        facts.push(description.to_owned());
    }
    let fields = [
        ("耐寒Lv", any(&requested,&["御寒","耐寒","保暖","防寒","耐冷"])),
        ("耐暑Lv", any(&requested,&["耐热","防暑","耐暑","抗热"])),
        ("减轻火属性伤害", any(&requested,&["火属性","火抗","抵抗火"])),
        ("减轻冰属性伤害", any(&requested,&["冰属性","冰抗","抵抗冰"])),
    ];
    for line in array(&db[first]["effects"]).iter().filter_map(|v| v.as_str()) {
        if !fields.iter().any(|(prefix,asked)| *asked && line.starts_with(prefix)) { continue; }
        if variants.iter().all(|r| array(&db[text(r,"id")]["effects"]).iter().any(|v|v.as_str()==Some(line))) {
            facts.push(line.to_owned());
        }
    }
    if facts.is_empty() { return None; }
    let mut answer = format!("按图鉴，{name}：{}。",facts.join("；").trim_end_matches('。'));
    let exact_loss = any(&requested,&["扣多少血","会扣多少","扣几滴","掉多少血","掉几滴","掉多少生命","掉多少hp"]);
    if exact_loss {
        answer.push_str("装备条目只给出防护效果。还需要所在区域的温度、实际暴露时间和服务器设置才能判断具体扣血量。");
    } else if fields.iter().any(|(prefix,asked)| *asked && !facts.iter().any(|s|s.starts_with(prefix))) {
        answer.push_str("上面列出的是这件装备可以确认的对应效果；其余属性没有相同的已核实数值。");
    }
    Some(AssistantReply {text:answer,links:vec![(name.to_owned(),format!("#items/{first}"))],suggestions:vec![]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn gear_reports_only_a_property_shared_by_matching_records() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data=Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for table in ["item-catalog-data","item-effects-data"] {
            let bytes = std::fs::read(root.join(format!("{table}.json"))).unwrap();
            data.tables.insert(table.into(),std::sync::Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let test=|q| answer(&data,&field_text(q));
        let armor=test("耐寒毛皮盔甲能防寒吗？我在山上会扣多少血？").unwrap();
        assert!(armor.text.contains("耐寒Lv2") && armor.text.contains("服务器设置") && !armor.text.contains("自身攻击力"));
        let ice=test("穿耐寒毛皮盔甲，冰属性的减伤是多少？").unwrap();
        assert!(ice.text.contains("冰属性伤害 -15%") && !ice.text.contains("火属性伤害"));
        let potential=test("伯乐眼镜用来显示实际攻击还是隐藏潜力？").unwrap();
        assert!(potential.text.contains("隐藏才能") && potential.text.contains("防御力") && !potential.text.contains("面板攻击"));
        assert!(test("耐寒毛皮盔甲设计图哪里掉？").is_none());
        assert!(test("有耐寒毛皮盔甲和耐热毛皮盔甲，哪件防寒？").is_none());
    }
}
