//! Explicit cross-catalog relationships, shared by details and acquisition views.
use crate::{
    catalog::render_value,
    data::{Data, array, text},
};
use eframe::egui;
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn link(
    ui: &mut egui::Ui,
    route: impl Into<String>,
    label: impl Into<String>,
    nav: &mut Option<String>,
) {
    if ui.link(label.into()).clicked() {
        *nav = Some(route.into());
    }
}
fn item(ui: &mut egui::Ui, d: &Data, id: &str, nav: &mut Option<String>) {
    link(ui, format!("#items/{id}"), d.item_name(id), nav);
}
fn point(ui: &mut egui::Ui, d: &Data, id: &str, nav: &mut Option<String>) {
    let b = d.get("item-catalog-data");
    let p = if b["points"][id].is_null() {
        &b["ruins"][id]
    } else {
        &b["points"][id]
    };
    let name = if p.is_null() {
        "在地图定位".into()
    } else {
        format!("{} ({}, {})", text(p, "nearest"), p["x"], p["y"])
    };
    link(ui, format!("#map/point/{id}"), name, nav);
}
pub fn has_method(d: &Data, v: &Value, m: &str) -> bool {
    let id = text(v, "id");
    match m {
        "配方制作" => !array(&v["recipes"]).is_empty(),
        "商人出售" => !array(&v["sellers"]).is_empty(),
        "牧场生产" => d.get("ranch-data")["pals"]
            .as_object()
            .is_some_and(|p| p.values().any(|p| ranch_has(p, id))),
        "遗物转换" => array(&d.get("relic-data")["outputs"])
            .iter()
            .any(|s| s.as_str() == Some(id)),
        "帕鲁远征" => array(&d.get("expedition-data")["stages"]).iter().any(|s| {
            array(&s["rewards"])
                .iter()
                .any(|r| r[0].as_str() == Some(id))
        }),
        _ => {
            array(&v["methods"])
                .iter()
                .any(|r| r[0].as_str() == Some(m))
                || !d.get("item-catalog-data")["guideLinks"][id][m].is_null()
                || m == "敌人掉落" && !d.get("item-catalog-data")["enemyDrops"][id].is_null()
        }
    }
}
fn ranch_has(p: &Value, id: &str) -> bool {
    array(&p["tiers"])
        .iter()
        .flat_map(array)
        .any(|r| r["item"].as_str() == Some(id))
}
pub fn sellers(ui: &mut egui::Ui, d: &Data, v: &Value, nav: &mut Option<String>) {
    let book = d.get("item-catalog-data");
    ui.strong("出售商人 · 点击坐标定位");
    for s in array(&v["sellers"]) {
        let shop = &book["shops"][text(s, "shop")];
        ui.group(|ui| {
            ui.strong(text(shop, "name"));
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("售价 {} / {} 件", s["cost"], s["qty"]));
                item(ui, d, text(s, "currency"), nav);
                if s["once"] == true {
                    ui.label("限购一次");
                }
            });
            render_value(ui, d, &shop["units"], 0, nav);
            for id in array(&shop["points"]).iter().filter_map(Value::as_str) {
                point(ui, d, id, nav);
            }
            if let Some(note) = shop["note"].as_str() {
                ui.label(note);
            }
        });
    }
}
pub fn craft(
    ui: &mut egui::Ui,
    d: &Data,
    plan: &Value,
    choices: &mut Value,
    nav: &mut Option<String>,
) {
    egui::CollapsingHeader::new("合成路径与总材料")
        .default_open(true)
        .show(ui, |ui| {
            for n in array(&plan["nodes"]) {
                let id = text(n, "id");
                let Some(it) = array(&d.get("item-catalog-data")["items"])
                    .iter()
                    .find(|i| i["id"] == id)
                else {
                    continue;
                };
                let recipes = array(&it["recipes"]);
                if recipes.is_empty() {
                    continue;
                }
                let mut choice = n["index"].as_i64().unwrap_or(-1);
                ui.horizontal_wrapped(|ui| {
                    item(ui, d, id, nav);
                    egui::ComboBox::from_id_salt(("craft-choice", id))
                        .selected_text(if choice < 0 {
                            "直接获取".into()
                        } else {
                            format!("配方 {}", choice + 1)
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut choice, -1, "直接获取");
                            for (i, r) in recipes.iter().enumerate() {
                                let stations = array(&r["stations"])
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("、");
                                ui.selectable_value(
                                    &mut choice,
                                    i as i64,
                                    format!("配方 {} · {} · 产出 {}", i + 1, stations, r["qty"]),
                                );
                            }
                        });
                });
                if choice != n["index"].as_i64().unwrap_or(-1) {
                    if !choices.is_object() {
                        *choices = json!({});
                    }
                    choices[id] = json!(choice);
                }
            }
            ui.strong("需自行准备的材料");
            for m in array(&plan["external"]) {
                ui.horizontal_wrapped(|ui| {
                    item(ui, d, m[0].as_str().unwrap_or(""), nav);
                    ui.label(format!("× {}", m[1]));
                });
            }
            ui.strong("按顺序制作");
            for (i, s) in array(&plan["steps"]).iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{}. ", i + 1));
                        item(ui, d, text(s, "id"), nav);
                        ui.label(format!(
                            "制作 {} 次 → {} 件（需要 {} 件）",
                            s["batches"], s["made"], s["required"]
                        ));
                    });
                    for m in array(&s["inputs"]) {
                        ui.horizontal_wrapped(|ui| {
                            item(ui, d, m[0].as_str().unwrap_or(""), nav);
                            ui.label(format!("× {}", m[1]));
                        });
                    }
                    render_value(ui, d, &s["recipe"]["stations"], 0, nav);
                });
            }
            if !array(&plan["cycles"]).is_empty() {
                ui.colored_label(
                    egui::Color32::DARK_RED,
                    "存在循环配方，相关材料已计入需自行准备的材料。",
                );
                render_value(ui, d, &plan["cycles"], 0, nav);
            }
        });
}
pub fn related(
    ui: &mut egui::Ui,
    d: &Data,
    page: &str,
    id: &str,
    v: &Value,
    nav: &mut Option<String>,
) {
    if page == "items" {
        item_related(ui, d, id, v, nav);
    }
    if ["merchants", "neutrals"].contains(&page) {
        for s in array(&v["shops"]).iter().filter_map(Value::as_str) {
            let shop = &d.get("enemy-data")["shopLists"][s];
            ui.separator();
            ui.strong(text(shop, "name"));
            for r in array(&shop["rows"]) {
                ui.horizontal_wrapped(|ui| {
                    item(ui, d, r[0].as_str().unwrap_or(""), nav);
                    ui.label(format!("× {} · 售价 {}", r[1], r[2]));
                    item(ui, d, r[3].as_str().unwrap_or(""), nav);
                    if r[4] == true {
                        ui.label("限购一次");
                    }
                });
            }
        }
        for p in array(&v["points"]) {
            link(
                ui,
                format!("#map/point/{}", text(p, "id")),
                format!("{} ({}, {})", text(p, "name"), p["x"], p["y"]),
                nav,
            );
        }
    }
    if page == "guide/gathering" {
        item(ui, d, id, nav);
        for key in array(&v["pools"]).iter().filter_map(Value::as_str) {
            render_value(ui, d, &d.get("resource-guides-data")["pools"][key], 0, nav);
        }
    }
    if page == "work/ranch" {
        link(
            ui,
            format!("#details/partner/{id}"),
            "伙伴技能各等级效果",
            nav,
        );
    }
    if page == "details/fruits" {
        if let Some(row) = d.resolve(text(v, "fruit")) {
            link(ui, row.0.clone(), "查看果实获取方式", nav);
        }
    }
    if page == "paldex" {
        link(ui, format!("#work/efficiency/{id}"), "计算工作效率", nav);
        if !d.get("ranch-data")["pals"][id].is_null() {
            link(ui, format!("#work/ranch/{id}"), "牧场产物", nav);
        }
    }
    let contextual = context_routes(d, page, id, v);
    if !contextual.is_empty() {
        ui.separator();
        ui.strong("相关地点与工具");
        for (route, name) in contextual {
            link(ui, route, name, nav);
        }
    }
}
fn item_related(ui: &mut egui::Ui, d: &Data, id: &str, v: &Value, nav: &mut Option<String>) {
    if !array(&v["sellers"]).is_empty() {
        ui.separator();
        link(
            ui,
            format!("#items/{id}/shops"),
            "查看出售商人、价格与坐标",
            nav,
        );
        sellers(ui, d, v, nav);
    }
    for ruin in array(&v["ruins"]).iter().filter_map(Value::as_str) {
        ui.label("远古遗迹可获取制造图");
        point(ui, d, ruin, nav);
    }
    let refs = item_routes(d, id, v);
    if !refs.is_empty() {
        ui.separator();
        ui.strong("获取方式与关联词条");
        for (route, label) in refs {
            link(ui, route, label, nav);
        }
    }
}
fn item_routes(d: &Data, id: &str, v: &Value) -> BTreeSet<(String, String)> {
    let book = d.get("item-catalog-data");
    let mut refs: BTreeSet<(String, String)> = BTreeSet::new();
    for (field, title) in [
        ("enemyDrops", "敌人掉落"),
        ("enemyFirstRewards", "敌人首胜奖励"),
        ("enemySummons", "可召唤敌人"),
    ] {
        for r in array(&book[field][id]) {
            refs.insert((
                format!("#enemies/{}", r[0].as_str().unwrap_or("")),
                format!("{} · {}", title, r[1].as_str().unwrap_or("")),
            ));
        }
    }
    for field in ["unitDrops", "unitRewards", "related", "currencies"] {
        for r in array(&book[field][id]) {
            if let (Some(route), Some(name)) = (r[0].as_str(), r[1].as_str()) {
                refs.insert((route.into(), name.into()));
            }
        }
    }
    if !array(&v["sellers"]).is_empty() {
        refs.insert((format!("#items/{id}/shops"), "出售商人".into()));
    }
    for ruin in array(&v["ruins"]).iter().filter_map(Value::as_str) {
        refs.insert((format!("#map/point/{ruin}"), "远古遗迹制造图".into()));
    }
    if let Some(pals) = d.get("ranch-data")["pals"].as_object() {
        for (key, p) in pals {
            if ranch_has(p, id) {
                refs.insert((
                    format!("#work/ranch/{key}"),
                    format!("牧场产出 · {}", d.pal_name(key)),
                ));
                refs.insert((format!("#work/ranch/item/{id}"), "比较牧场产量".into()));
            }
        }
    }
    if array(&d.get("relic-data")["outputs"])
        .iter()
        .any(|o| o.as_str() == Some(id))
    {
        refs.insert((
            format!("#work/relic/item/{id}"),
            "比较五档遗物产出概率".into(),
        ));
    }
    if array(&d.get("relic-data")["tiers"])
        .iter()
        .any(|t| t["id"] == id)
    {
        refs.insert((format!("#work/relic/{id}"), "查看遗物转换产物".into()));
    }
    for s in array(&d.get("expedition-data")["stages"]) {
        if array(&s["rewards"])
            .iter()
            .any(|r| r[0].as_str() == Some(id))
        {
            refs.insert((
                format!("#work/expeditions/{}", text(s, "id")),
                format!("远征 · {}", text(s, "name")),
            ));
        }
    }
    if let Some(g) = book["guideLinks"][id].as_object() {
        for (kind, rows) in g {
            for r in array(rows) {
                if let (Some(href), Some(name)) = (r[0].as_str(), r[1].as_str()) {
                    refs.insert((href.into(), format!("{kind} · {name}")));
                }
            }
        }
    }
    for m in array(&v["methods"]) {
        let kind = m[0].as_str().unwrap_or("");
        let desc = m[1].as_str().unwrap_or("");
        let region = if desc.contains("世界树") {
            "worldtree"
        } else {
            "palpagos"
        };
        let category = match kind {
            "野外钓鱼" => "fishing",
            "敌方营地" => "enemy_camp",
            "地下城宝箱" => "dungeon",
            "区域宝箱" | "属性宝箱" | "油田宝箱" => "chest",
            "技能果实树" => "skill_fruit",
            "野外帕鲁蛋" | "世界树拾取" => "egg",
            _ => "",
        };
        if !category.is_empty() {
            refs.insert((
                format!("#map/category/{category}/{region}"),
                format!("地图 · {kind}"),
            ));
        }
        if kind == "野外采集" {
            let mineral = match id {
                "CopperOre" => "ore",
                "Coal" => "coal",
                "Sulfur" => "sulfur",
                "Quartz" => "pure_quartz",
                "RainbowCrystal" => "hexolite_quartz",
                "Chromium" => "chromite",
                "SkyIslandOre" => "soralite",
                "WorldTreeOre" => "paloxite",
                _ => "",
            };
            if !mineral.is_empty() {
                refs.insert((
                    format!("#map/category/{mineral}/{region}"),
                    "地图矿脉".into(),
                ));
            }
        }
        if kind == "据点生产" {
            if let Some(p) = book["productionNames"][desc].as_str() {
                refs.insert((format!("#work/production/{p}"), format!("生产 · {desc}")));
            }
            if id == "CrudeOil" {
                refs.insert(("#map/category/crude_oil/palpagos".into(), "原油点位".into()));
            }
        }
        if let Some(names) = book["facilityLinks"].as_object() {
            for (name, key) in names {
                if desc.contains(name) {
                    refs.insert((
                        format!("#work/facilities/{}", key.as_str().unwrap_or("")),
                        format!("设施 · {name}"),
                    ));
                }
            }
        }
        for (kind_match, key) in [
            ("钓鱼池", "Fishing_Pond"),
            ("钓鱼池", "Large_Fishing_Pond"),
            ("技能果树园", "Skillfruit_Orchard"),
            ("召唤战胜利奖励", "Summoning_Altar"),
            ("解体产物", "Pal_Disassembly_Conveyor"),
            ("据点配种", "Breeding_Farm"),
            ("配种突变", "Breeding_Farm"),
        ] {
            if kind == kind_match {
                refs.insert((format!("#work/facilities/{key}"), format!("{kind}设施")));
            }
        }
        for p in &d.species {
            if desc.contains(text(p, "zh")) {
                let key = text(p, "key");
                refs.insert((format!("#paldex/{key}"), d.pal_name(key)));
                for point in array(&book["acquisitionLinks"]["bosses"][key]) {
                    if let Some(pid) = point[0].as_str() {
                        refs.insert((
                            format!("#map/point/{pid}"),
                            format!("{} 头目 ({}, {})", d.pal_name(key), point[1], point[2]),
                        ));
                    }
                }
            }
        }
        if let Some(point) = book["acquisitionLinks"]["bounties"][desc].as_str() {
            refs.insert((format!("#map/point/{point}"), format!("定位 · {desc}")));
        }
        for q in array(&book["acquisitionLinks"]["quests"][desc]) {
            if array(&q[2]).iter().any(|i| i.as_str() == Some(id)) {
                refs.insert((
                    format!("#guide/quests/{}", q[0].as_str().unwrap_or("")),
                    format!("任务 · {desc}"),
                ));
                for loc in array(&q[1]) {
                    refs.insert((
                        format!(
                            "#map/coords/{}/{}/{}",
                            loc[0].as_str().unwrap_or("palpagos"),
                            loc[1],
                            loc[2]
                        ),
                        format!("任务坐标 ({}, {})", loc[1], loc[2]),
                    ));
                }
            }
        }
    }
    refs
}

fn context_routes(d: &Data, page: &str, id: &str, v: &Value) -> BTreeSet<(String, String)> {
    let mut refs = BTreeSet::new();
    if page == "guide/quests" {
        let quests = array(&d.get("adventure-data")["quests"]);
        for objective in array(&v["objectives"]).iter().chain(array(&v["rewards"])) {
            let label = text(objective, "text");
            if let Some(point) = objective["point"].as_str() {
                refs.insert((format!("#map/point/{point}"), format!("地图 · {label}")));
            } else if let Some(site) = objective["site"].as_array() {
                if site.len() == 3
                    && matches!(site[0].as_str(), Some("palpagos" | "worldtree"))
                    && site[1].as_f64().is_some()
                    && site[2].as_f64().is_some()
                {
                    refs.insert((
                        format!(
                            "#map/coords/{}/{}/{}",
                            site[0].as_str().unwrap(),
                            site[1],
                            site[2]
                        ),
                        format!("地图 · {label} ({}, {})", site[1], site[2]),
                    ));
                }
            }
            if let Some(key) = objective["pal"].as_str() {
                refs.insert((
                    format!("#paldex/{key}"),
                    format!("帕鲁 · {}", d.pal_name(key)),
                ));
            }
            if let Some(key) = objective["passive"].as_str() {
                refs.insert((
                    format!("#details/passive/trait/{key}"),
                    format!("词条 · {}", d.trait_name(key)),
                ));
            }
            if let Some(unit) = objective["unit"].as_str() {
                if let Some(route) = d.get("adventure-data")["unitRoutes"][unit].as_str() {
                    refs.insert((route.into(), label.into()));
                }
            }
            if let Some(item) = objective["item"].as_str() {
                refs.insert((format!("#items/{item}"), d.item_name(item)));
            }
        }
        for next in array(&v["next"]).iter().filter_map(Value::as_str) {
            if let Some(q) = quests.iter().find(|q| q["id"] == next) {
                refs.insert((
                    format!("#guide/quests/{next}"),
                    format!("后续任务 · {}", text(q, "name")),
                ));
            }
        }
        for q in quests.iter().filter(|q| {
            array(&q["next"])
                .iter()
                .any(|next| next.as_str() == Some(id))
        }) {
            refs.insert((
                format!("#guide/quests/{}", text(q, "id")),
                format!("前序任务 · {}", text(q, "name")),
            ));
        }
    }
    if page == "guide/loot" {
        if let Some((chapter, pool)) = id.split_once('/') {
            let region = if pool.to_lowercase().contains("worldtree") {
                "worldtree"
            } else {
                "palpagos"
            };
            let category = match chapter {
                "EnemyCampLoots" => "enemy_camp",
                "SkillFruitLoots" => "skill_fruit",
                "DungeonsLoots" => "dungeon",
                "ElementalChestsLoots" | "OilrigLoots" => "chest",
                "FishingLoots" => "fishing",
                _ => "",
            };
            if !category.is_empty() {
                refs.insert((
                    format!(
                        "#map/category/{category}/{region}{}",
                        if chapter == "OilrigLoots" {
                            "/油田"
                        } else {
                            ""
                        }
                    ),
                    "地图 · 对应探索地点".into(),
                ));
            }
            if chapter == "PalExpeditionStationLoots" {
                refs.insert((
                    format!("#work/expeditions/{pool}"),
                    "查看远征条件与时间".into(),
                ));
            }
            if chapter == "AncientRelicRecyclerLoots" {
                refs.insert((
                    format!(
                        "#work/relic/{}",
                        pool.trim_start_matches("AncientRelicRecycler_")
                    ),
                    "查看遗物转换产量".into(),
                ));
            }
        }
    }
    if page == "guide/gathering" {
        for production in array(&d.get("resource-guides-data")["production"])
            .iter()
            .filter(|p| p["item"] == id)
        {
            refs.insert((
                format!("#work/production/{}", text(production, "id")),
                format!("据点生产 · {}", text(production, "name")),
            ));
        }
        if let Some(pal) = v["pal"][0].as_str() {
            refs.insert((
                format!("#details/partner/{pal}"),
                format!("采集伙伴 · {}", d.pal_name(pal)),
            ));
        }
        if let Some(route) = v["areaLink"][0].as_str() {
            refs.insert((
                route.into(),
                v["areaLink"][1].as_str().unwrap_or("采集区域").into(),
            ));
        }
        for key in array(&v["pools"]).iter().filter_map(Value::as_str) {
            for pool in array(&d.get("resource-guides-data")["pools"][key]) {
                if let Some(route) = pool["href"].as_str() {
                    refs.insert((
                        route.into(),
                        format!("{} · 全部 {} 处", text(pool, "label"), pool["count"]),
                    ));
                }
                for p in array(&pool["samples"]) {
                    if let Some(point) = p[0].as_str() {
                        refs.insert((
                            format!("#map/point/{point}"),
                            format!("{} ({}, {})", text(pool, "label"), p[1], p[2]),
                        ));
                    }
                }
            }
        }
    }
    if page == "work/production" {
        let item = text(v, "item");
        if !item.is_empty() {
            refs.insert((
                format!("#items/{item}"),
                format!("产物 · {}", d.item_name(item)),
            ));
            if array(&d.get("resource-guides-data")["gathering"])
                .iter()
                .any(|g| g["id"] == item)
            {
                refs.insert((format!("#guide/gathering/{item}"), "野外采集位置".into()));
            }
        }
        if let Some(facility) = v["facility"].as_str() {
            refs.insert((
                format!("#work/facilities/{facility}"),
                "设施与建造材料".into(),
            ));
        }
        if !array(&v["jobs"]).is_empty() {
            refs.insert(("#work/efficiency".into(), "计算帕鲁工作效率".into()));
        }
        if id == "OilPump" {
            refs.insert((
                "#map/category/crude_oil/palpagos".into(),
                "定位原油油田".into(),
            ));
        }
    }
    if page == "paldex" {
        for job in array(&d.get("paldex-index-data")["profiles"][id]["work"]) {
            if let Some(name) = job[0].as_str() {
                refs.insert((
                    format!("#work/efficiency/{id}/{name}"),
                    format!("计算 {name} Lv.{} 工作效率", job[1]),
                ));
            }
        }
        refs.insert((
            format!("#work/suitability/{id}"),
            "查看工作适应性排行".into(),
        ));
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    fn fixture() -> Data {
        let mut data = Data::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
            egui::Context::default(),
        )
        .unwrap();
        let start = std::time::Instant::now();
        while data.pending > 0 && start.elapsed().as_secs() < 15 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            data.poll();
        }
        assert!(data.errors.is_empty());
        assert_eq!(data.pending, 0);
        data
    }
    fn embedded(v: &Value, found: &mut BTreeSet<String>) {
        match v {
            Value::String(s)
                if s.starts_with('#')
                    && [
                        "items",
                        "paldex",
                        "details",
                        "work",
                        "guide",
                        "map",
                        "spawns",
                        "lookup",
                        "enemies",
                        "merchants",
                        "neutrals",
                    ]
                    .contains(&s[1..].split('/').next().unwrap_or("")) =>
            {
                found.insert(crate::decode_route(s));
            }
            Value::Array(a) => {
                for v in a {
                    embedded(v, found)
                }
            }
            Value::Object(o) => {
                for v in o.values() {
                    embedded(v, found)
                }
            }
            _ => {}
        }
    }
    fn destination(d: &Data, c: &Catalog, route: &str) -> bool {
        let route = crate::decode_route(route);
        let raw = route.trim_start_matches('#');
        let parts: Vec<_> = raw.split('/').collect();
        let page = crate::page_for(&route);
        if parts[0] == "map" {
            return match parts.get(1).copied() {
                None => true,
                Some("point") => array(&d.get("map-data")["points"])
                    .iter()
                    .any(|p| p["id"].as_str() == parts.get(2).copied()),
                Some("category") => parts.get(2).is_some_and(|id| {
                    *id == "all"
                        || *id == "custom"
                        || array(&d.get("map-data")["categories"])
                            .iter()
                            .any(|c| c["id"].as_str() == Some(id))
                }),
                Some("coords") => {
                    matches!(parts.get(2).copied(), Some("palpagos" | "worldtree"))
                        && parts.get(3).is_some_and(|v| v.parse::<f64>().is_ok())
                        && parts.get(4).is_some_and(|v| v.parse::<f64>().is_ok())
                }
                _ => false,
            };
        }
        if ["spawns", "lookup"].contains(&page.as_str()) {
            return parts.len() == 1 || parts.get(1).is_some_and(|key| d.pal(key).is_some());
        }
        let wanted = raw
            .strip_prefix(&page)
            .unwrap_or("")
            .trim_start_matches('/');
        if wanted.is_empty() {
            return c.pages.contains_key(&page)
                || [
                    "work/efficiency",
                    "work/suitability",
                    "work/relic",
                    "details/stats",
                    "details/capture",
                ]
                .contains(&page.as_str());
        }
        if [
            "details/stats",
            "details/capture",
            "work/efficiency",
            "work/suitability",
        ]
        .contains(&page.as_str())
        {
            return d.pal(wanted.split('/').next().unwrap_or("")).is_some();
        }
        if page == "work/relic" {
            return if let Some(id) = wanted.strip_prefix("item/") {
                array(&d.get("relic-data")["outputs"])
                    .iter()
                    .any(|v| v.as_str() == Some(id))
            } else {
                array(&d.get("relic-data")["tiers"])
                    .iter()
                    .any(|v| v["id"] == wanted)
            };
        }
        if page == "work/ranch" && wanted.starts_with("item/") {
            return d.get("ranch-data")["pals"].as_object().is_some_and(|p| {
                p.values()
                    .any(|p| ranch_has(p, wanted.trim_start_matches("item/")))
            });
        }
        if page == "guide/loot" {
            if let Some((chapter, item)) = wanted.split_once("/item/") {
                return array(&d.get("adventure-data")["chapters"]).iter().any(|c| {
                    c["id"] == chapter
                        && array(&c["pools"]).iter().any(|p| {
                            array(&p["rows"])
                                .iter()
                                .any(|r| r[0].as_str() == Some(item))
                        })
                });
            }
        }
        let wanted = wanted
            .strip_prefix("move/")
            .or_else(|| wanted.strip_prefix("trait/"))
            .unwrap_or(wanted)
            .trim_end_matches("/shops");
        c.pages.get(&page).is_some_and(|rows| {
            rows.iter().any(|r| {
                r.id == wanted || page == "guide/loot" && r.id.starts_with(&format!("{wanted}/"))
            })
        })
    }
    #[test]
    fn all_embedded_and_generated_cross_catalog_links_have_destinations() {
        let d = fixture();
        let mut catalog = Catalog::default();
        catalog.rebuild(&d);
        let mut routes = BTreeSet::new();
        for value in d.tables.values() {
            embedded(value, &mut routes);
        }
        assert!(routes.len() >= 8064);
        let embedded_count = routes.len();
        for item in array(&d.get("item-catalog-data")["items"]) {
            routes.extend(
                item_routes(&d, text(item, "id"), item)
                    .into_iter()
                    .map(|p| p.0),
            );
        }
        for (page, rows) in &catalog.pages {
            for row in rows {
                routes.extend(
                    context_routes(&d, page, &row.id, &row.value)
                        .into_iter()
                        .map(|p| p.0),
                );
            }
        }
        let missing: Vec<_> = routes
            .iter()
            .filter(|route| !destination(&d, &catalog, route))
            .collect();
        assert!(
            missing.is_empty(),
            "Missing destinations ({}/{} embedded, {} total): {missing:?}",
            missing.len(),
            embedded_count,
            routes.len()
        );
    }
    #[test]
    fn quest_coordinates_predecessors_and_reward_pool_connections_are_explicit() {
        let d = fixture();
        let quest = array(&d.get("adventure-data")["quests"])
            .iter()
            .find(|q| q["id"] == "the-first-islander")
            .unwrap();
        let refs = context_routes(&d, "guide/quests", "the-first-islander", quest);
        assert!(
            refs.iter()
                .any(|(r, _)| r == "#map/coords/palpagos/241/-509")
        );
        assert!(
            refs.iter()
                .any(|(r, _)| r == "#guide/quests/activate-great-eagle-statue")
        );
        assert!(
            refs.iter()
                .any(|(r, _)| r == "#guide/quests/how-to-stay-alive")
        );
        let refs = context_routes(
            &d,
            "guide/loot",
            "PalExpeditionStationLoots/Expedition_Grass",
            &Value::Null,
        );
        assert!(
            refs.iter()
                .any(|(r, _)| r == "#work/expeditions/Expedition_Grass")
        );
        let refs = context_routes(&d, "guide/loot", "OilrigLoots/Oilrig_01", &Value::Null);
        assert!(
            refs.iter()
                .any(|(r, _)| r == "#map/category/chest/palpagos/油田")
        );
    }
    #[test]
    fn early_and_loaded_catalogue_detail_frames_do_not_panic() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let ctx = egui::Context::default();
        let mut d = Data::new(root, ctx.clone()).unwrap();
        let mut catalog = Catalog::default();
        for loaded in [false, true] {
            if loaded {
                let start = std::time::Instant::now();
                while d.pending > 0 && start.elapsed().as_secs() < 15 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    d.poll();
                }
            }
            catalog.rebuild(&d);
            let pages: Vec<_> = catalog
                .pages
                .iter()
                .map(|(p, rows)| {
                    (
                        p.clone(),
                        rows.first()
                            .map(|e| e.route.clone())
                            .unwrap_or_else(|| format!("#{p}")),
                    )
                })
                .collect();
            for (page, route) in pages {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280., 800.),
                    )),
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let mut surgery = json!([]);
                        catalog.show(ui, &d, &page, &route, &mut surgery);
                    });
                });
            }
        }
    }
}
