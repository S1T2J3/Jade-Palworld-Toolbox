use super::{AssistantReply, facility_effect, quantities::integers};
use crate::data::{Data, array, text};

/// A stat on a dish belongs to the individual that ate it. Keep the item ID and
/// the complete effect together, including the duration and any other effects.
pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    let has = |terms: &[&str]| terms.iter().any(|term| q.contains(term));
    if !has(&["食物", "料理", "饭菜", "菜肴"])
        || !has(&["吃", "喂", "哪些", "哪种", "哪道", "什么食物", "什么料理"])
        || has(&["怎么做", "制作", "配方", "材料", "哪里", "购买", "卖", "叠加", "叠满", "对比", "哪种最", "最高", "抗性", "属性伤害", "营养"])
    {
        return None;
    }
    let fields = [
        ("攻击力", has(&["攻击", "加攻", "伤害"]) && !has(&["减伤", "受到伤害", "受到的伤害", "承受伤害"])),
        ("防御力", has(&["防御", "加防"])),
        ("工作速度", has(&["工作速度", "工作效率", "干活速度"])),
        ("经验获取量", has(&["经验"])),
    ];
    let mut requested = fields.into_iter().filter_map(|(name, present)| present.then_some(name));
    let stat = requested.next()?;
    if requested.next().is_some() { return None; }

    let named = facility_effect::items(data, q);
    let named_food = match named.as_slice() {
        [] if has(&["的食物", "的料理", "哪种", "哪些", "哪道", "什么食物", "什么料理"]) => None,
        [item] if text(item, "category") == "食物" => Some(*item),
        _ => return None,
    };
    let percentages: Vec<_> = integers(q).into_iter()
        .filter(|n| q[n.end..].trim_start().starts_with('%')).map(|n| n.value).collect();
    if percentages.len() > 1 { return None; }
    let prefix = format!("{stat} +");
    let wanted = percentages.first().map(|value| format!("{stat} +{value}%"));
    let effects = &data.get("item-effects-data")["items"];
    let matching = |id: &str| {
        array(&effects[id]["effects"]).iter().filter_map(|line| line.as_str()).any(|line| {
            line.starts_with(&prefix) && wanted.as_ref().is_none_or(|value| line.starts_with(value))
        })
    };
    let (item, introduction) = if let Some(item) = named_food {
        (item, "这道料理的食用效果")
    } else {
        let item = array(&data.get("item-catalog-data")["items"]).iter()
            .filter(|item| text(item, "category") == "食物" && matching(text(item, "id")))
            .min_by_key(|item| (array(&effects[text(item, "id")]["effects"]).len(), text(item, "name").chars().count()))?;
        (item, "符合这项条件的料理举例")
    };
    let id = text(item, "id");
    let name = text(item, "name");
    let recorded = array(&effects[id]["effects"]).iter().filter_map(|v| v.as_str()).collect::<Vec<_>>();
    if recorded.is_empty() { return None; }
    let mut answer = format!("{introduction}：{name}，{}。加成作用于吃下它的个体。", recorded.join("；"));
    if stat == "攻击力" && matching(id) && q.contains("伤害") {
        answer.push_str("这里加的是攻击力。单凭这项数值无法确定一击的最终伤害会增加多少。");
    } else if !matching(id) {
        answer.push_str(&format!("这道料理的效果栏没有列出题述的{stat}加成。"));
    }
    Some(AssistantReply { text:answer, links:vec![(name.to_owned(), format!("#items/{id}"))], suggestions:vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn labels_the_eater_and_reads_the_named_dish_before_other_similar_names() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["item-catalog-data", "item-effects-data"] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            data.tables.insert(name.into(), std::sync::Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let r = answer(&data,"给帕鲁吃增加10%伤害的食物会影响伤害吗").unwrap();
        assert!(r.text.contains("攻击力 +10%") && r.text.contains("吃下它的个体") && r.text.contains("最终伤害"));
        let named = answer(&data,"喂波霸牛汉堡这种料理能加攻击力吗").unwrap();
        assert!(named.text.contains("工作速度 +50%") && named.text.contains("没有列出") && !named.text.contains("攻击力 +20%"));
        assert_eq!(named.links[0].1, "#items/Hamburger_2");
        assert!(answer(&data,"哪些食物能提升45%攻击力").is_none());
        assert!(answer(&data,"吃了两种加攻食物能不能叠加").is_none());
        assert!(answer(&data,"哪种料理能提升火属性伤害").is_none());
    }
}
