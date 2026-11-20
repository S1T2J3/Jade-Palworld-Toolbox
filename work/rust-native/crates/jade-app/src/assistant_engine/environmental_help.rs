//! Source-bound advice for the player in the world's environment.
use super::AssistantReply;
use crate::data::{Data, array, text};

fn any(q: &str, words: &[&str]) -> bool {
    words.iter().any(|word| q.contains(word))
}

// A general question about the player's environment must not match, for example,
// a cold incubator, a spoiled meal, a pal's ice attack or a named modded item.
pub(crate) fn only_phrases(input: &str, phrases: &[&str]) -> bool {
    let compact: String = input.chars().filter(|c| c.is_alphanumeric()).collect();
    let mut rest = compact.as_str();
    while !rest.is_empty() {
        let Some(word) = phrases.iter().filter(|word| rest.starts_with(**word)).max_by_key(|w| w.len()) else {
            return false;
        };
        rest = &rest[word.len()..];
    }
    true
}

pub(crate) fn temperature(q: &str) -> Option<bool> {
    let cold = any(q, &["冷", "寒", "冻", "保暖", "取暖"]);
    let hot = any(q, &["热", "暑", "高温", "降温"]);
    if cold == hot || !any(q, &["怎么办", "咋办", "怎么", "如何", "怎样", "用什么", "穿什么", "哪件", "什么装备"]) {
        return None;
    }
    only_phrases(q, &[
        "请问一下", "请问", "幻兽帕鲁", "游戏", "玩家", "人物", "角色", "自己", "我", "身上", "一直", "到了", "时", "在", "据点", "基地", "里", "内",
        "站", "野外", "外面", "出门", "持续", "老是", "受寒", "受冻", "冻得", "热得", "冷得", "冻到", "热到", "冷到", "直",
        "雪山", "雪地", "沙漠", "火山", "夜里", "晚上", "白天", "现在", "太", "好", "很", "感觉", "又",
        "怎么办", "咋办", "怎么", "如何", "怎样", "用什么", "穿什么", "哪件", "什么装备", "装备", "衣服",
        "防御", "抵御", "抵抗", "防止", "防", "解决", "御寒", "驱寒", "保暖", "取暖", "防暑", "降温",
        "寒冷", "炎热", "酷热", "高温", "中暑", "冻伤", "冻", "冷", "热", "寒", "值", "环境", "地区",
        "掉血", "扣血", "伤害", "要", "该", "能", "可以", "了", "呀", "啊", "呢", "吗", "的",
    ]).then_some(cold)
}

fn player_temperature(data: &Data, q: &str, cold: bool) -> Option<AssistantReply> {
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let armor = if cold { "FurArmorCold" } else { "FurArmorHeat" };
    let effects = &data.get("item-effects-data")["items"];
    let items = array(&data.get("item-catalog-data")["items"]);
    for id in [armor, "Torch"] {
        let Some(item) = items.iter().find(|row| text(row, "id") == id) else { continue };
        let name = text(item, "name");
        let info = &effects[id];
        let description = text(info, "description");
        let sentence = if id == "Torch" {
            if !cold || !description.contains("为人驱逐寒意") { continue; }
            format!("在野外也可装备「{name}」来驱寒。")
        } else {
            let resistance = if cold { "耐寒" } else { "耐暑" };
            let Some(level) = array(&info["effects"]).iter().filter_map(|s| s.as_str()).find(|s| s.starts_with(resistance)) else { continue };
            format!("角色可以穿上「{name}」，这件防具提供{level}。")
        };
        lines.push(sentence);
        links.push((name.to_owned(), format!("#items/{id}")));
    }
    if any(q, &["据点","基地"]) {
        let id = if cold { "Heater" } else { "Cooler" };
        let facilities = array(&data.get("facilities-data")["facilities"]);
        if let Some(f) = facilities.iter().find(|f| text(f,"id") == id)
            && let Some(job) = f["required"][0][0].as_str()
            && !text(f,"purpose").is_empty() {
            let name = text(f, "name");
            lines.insert(0, format!("在据点可让有{job}适性的帕鲁运转「{name}」。{}角色要待在设施附近。", text(f, "purpose")));
            links.insert(0, (name.to_owned(), format!("#work/facilities/{id}")));
        }
    }
    (!lines.is_empty()).then(|| AssistantReply { text: lines.join("\n"), links, suggestions: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{field_text, Conversation};
    use std::{path::PathBuf, sync::Arc};

    fn handbook() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for table in ["item-catalog-data","item-effects-data","facilities-data"] {
            let bytes = std::fs::read(root.join(format!("{table}.json"))).unwrap();
            data.tables.insert(table.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        data
    }

    #[test]
    fn player_temperature_is_a_direct_lookup_and_keeps_actor_and_equipment_in_scope() {
        let d = handbook();
        for (q, term, route) in [("寒冷值怎麼辦？","耐寒","#items/FurArmorCold"),
            ("在野外冻得直掉血，该怎么保暖？","耐寒","#items/Torch"),
            ("在沙漠太热怎么办？","耐暑","#items/FurArmorHeat"),
            ("据点里太冷怎么取暖？","取暖器","#work/facilities/Heater"),
            ("基地里太热如何降温？","降温器","#work/facilities/Cooler")] {
            let mut c = Conversation::default();
            let r = c.ask_world(q, &d, &serde_json::json!({}));
            assert!(c.direct_result() && r.links.iter().any(|(_,href)| href == route), "{q}: {}", r.text);
            assert!(r.text.contains(term), "{q}: {}", r.text);
        }
        let mut c = Conversation::default();
        c.ask_world("寒冷值怎麼辦？", &d, &serde_json::json!({}));
        let low = c.ask_world("我只有 4 级", &d, &serde_json::json!({}));
        assert!(c.direct_result() && low.text.contains("布衣服") && !low.text.contains("民族服饰"));
        let egg = c.ask_world("是孵化器的提示", &d, &serde_json::json!({}));
        assert!(c.direct_result() && egg.text.contains("孵化器") && egg.links.iter().any(|(_,u)| u == "#work/facilities/Heater"));
        for q in ["孵化器太冷怎么办？","我的帕鲁冻伤怎么办？","天气预报显示多冷？","耐寒护甲受冰属性攻击扣多少血？",
            "使用模组之后角色太热怎么办？","我如何用冷却设施保鲜？","寒冷值怎么办？另一个问题是孵蛋温度。"] {
            assert!(answer(&d,&field_text(q)).is_none(), "{q}");
        }
    }

    #[test]
    fn community_height_estimate_retains_its_sea_level_reference() {
        let d = handbook();
        let mut c = Conversation::default();
        let r = c.ask_world("《幻兽帕鲁》里能把建筑盖多高？",&d,&serde_json::json!({}));
        assert!(c.direct_result() && r.text.contains("海平面") && r.text.contains("玩家报告")
            && r.text.contains("据点的工作范围") && r.links[0].1.contains("steamcommunity.com"));
        for q in ["浓缩机本身多高？","模组能让高塔建多高？","飞行坐骑能飞多高？","建筑悬挑几格？","据点的工作范围有多高？"] {
            assert!(answer(&d, &field_text(q)).is_none(), "{q}");
        }
    }
}

fn building_height(q: &str) -> bool {
    any(q, &["建", "盖", "楼", "叠墙", "造塔"])
        && any(q, &["多高", "几层", "多少层", "高度上限", "高度限制", "高度是多少"])
        && only_phrases(q, &["请问一下","请问","一下","幻兽帕鲁","这个","游戏","里","玩家","我","的","在","从","能","可不可以",
            "可以","最多","最高","究竟","到底","把","建筑","建筑物","建造","搭建","建","盖",
            "楼房","大楼","高楼","高塔","造塔","一座","一栋","楼","塔","墙","叠墙","海平面","地面","起","往上",
            "到","多高","几层","多少层","高度","上限","限制","是多少","是","有","吗","呢","啊"])
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if q.contains('\n') { return None; }
    if let Some(cold) = temperature(q) { return player_temperature(data, q, cold); }
    if !building_height(q) { return None; }
    Some(AssistantReply {
        text: "有玩家报告从海平面起约能叠 255～256 面墙。山上起建，地面已经占去一部分海拔，可用高度会少一些。据点的工作范围和整张地图可放置建筑的最高位置是两种限制。这是玩家报告的估计，不能用它保证任意版本、任意地点都能从脚下再盖 256 层。".into(),
        links: vec![("玩家讨论 · 从海平面计的墙高与据点范围".into(), "https://steamcommunity.com/app/1623730/discussions/0/588433527532571099/".into())],
        suggestions: vec![],
    })
}
