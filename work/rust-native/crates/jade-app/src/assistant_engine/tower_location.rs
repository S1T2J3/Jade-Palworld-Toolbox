use super::{AssistantReply, habitat::landmark};
use crate::data::{Data, array, text};

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    let focus = q.split(['?', '？', '。', '.', '\n']).find(|part| {
        ["哪里", "哪儿", "在哪", "坐标", "位置", "怎么去", "如何前往", "where", "coordinates",
         "location", "how to reach", "how do i get to"].iter().any(|term| part.contains(term))
    })?;
    let has = |terms: &[&str]| terms.iter().any(|term| focus.contains(term));
    if !has(&["塔", "tower"])
        || has(&["附近", "旁边", "周围", "周边", "near ", "nearby", "around ", " close to ",
                 "野生", "掉落", "设计图", "材料", "怎么打", "哪种帕鲁", "哪里抓", "在哪抓",
                 "能抓", "宝箱", "推荐", "捕获", "捕捉", "刷新", "wild ", " drop", " loot",
                 " schematic", "chest", " catch", " beat", " defeat", "farm ", "spawn"])
    {
        return None;
    }
    let point = landmark(data, focus).or_else(|| {
        if !has(&["世界树", "world tree", "worldtree"]) { return None; }
        let stage = array(&data.get("expedition-data")["stages"]).iter()
            .find(|s| s["region"] == "世界树" && s["hard"] == false)?;
        array(&data.get("map-data")["points"]).iter().find(|p| p["id"] == stage["point"])
    })?;
    if text(point, "category") != "syndicate_tower" { return None; }
    let map = array(&data.get("map-data")["maps"]).iter().find(|map| map["id"] == point["map"])?;
    let id = point["id"].as_str()?;
    let (x, y) = (point["x"].as_i64()?, point["y"].as_i64()?);
    let name = text(point, "name");
    let mut line = format!("{name}位于{}，{}为（{x}，{y}）。", text(map, "name"), text(map, "coordLabel"));
    let mut links = vec![(format!("定位{name}"), format!("#map/point/{id}"))];
    let variant = if has(&["困难", "hard mode", "hard-mode"]) { "困难" } else { "普通" };
    if let Some(enemy) = array(&data.get("enemy-data")["enemies"]).iter().find(|e| {
        e["variant"] == variant && array(&e["points"]).iter().any(|p| p["id"] == id)
    }) {
        line.push_str(&format!("{variant}挑战的首领是「{}」", text(enemy, "name")));
        if let Some(level) = enemy["level"].as_u64() { line.push_str(&format!("（Lv.{level}）")); }
        line.push('。');
        links.push((text(enemy, "name").into(), format!("#enemies/{}", text(enemy, "id"))));
    }
    Some(AssistantReply { text: line, links, suggestions: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn player_party_does_not_replace_the_tower_and_nearby_wildlife_keeps_its_route() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["map-data", "expedition-data", "enemy-data"] {
            let bytes = std::fs::read(dir.join(format!("{name}.json"))).unwrap();
            data.tables.insert(name.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let reply = answer(&data, "where is the feybreak tower boss? i use an ice-type pal.").unwrap();
        assert!(reply.text.contains("-1294，-1669") && reply.text.contains("霜牙王"));
        assert!(reply.links[0].1.starts_with("#map/point/"));
        let second = answer(&data, "世界树的塔主在哪？").unwrap();
        assert!(second.text.contains("区域坐标") && second.links[0].1.contains("worldtree"));
        assert!(answer(&data, "天坠之塔附近哪里能抓冰系帕鲁？").is_none());
        assert!(answer(&data, "where can i find schematic drops from the feybreak tower?").is_none());
    }
}
