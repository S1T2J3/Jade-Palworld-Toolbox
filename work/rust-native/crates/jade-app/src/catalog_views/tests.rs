use super::*;
use crate::catalog::Catalog;

#[test]
fn every_item_has_exact_id_effects_and_quality_specific_stats() {
    let mut h = Harness::new();
    let effects = &h.d.get("item-effects-data")["items"];
    assert_eq!(effects.as_object().unwrap().len(), h.c.pages["items"].len());
    for e in &h.c.pages["items"] {
        assert!(!effects[&e.id].is_null(), "missing effects: {}", e.id);
        for id in array(&effects[&e.id]["relatedItems"])
            .iter()
            .filter_map(Value::as_str)
        {
            assert!(!effects[id].is_null(), "broken related item: {id}");
        }
        let skill = text(&effects[&e.id], "skill");
        if !skill.is_empty() {
            assert!(
                !h.d.get("pal-details-data")["moves"][skill].is_null(),
                "broken skill {skill}"
            );
        }
    }
    for (id, expected) in [
        ("StealArmor", vec!["220", "750", "5000", "耐寒", "耐暑"]),
        ("StealArmor_5", vec!["352", "1200", "26000", "+12%"]),
        ("Accessory_AT_1", vec!["+15%"]),
        ("Salad", vec!["工作速度 +30%", "600 秒"]),
        ("SkillCard_AquaJet", vec!["使用后学习", "水流射击"]),
    ] {
        let e = h.entry("items", id);
        let screen = h.warm("items", &e, &format!("#items/{id}"));
        for expected in expected {
            assert!(screen.contains(expected), "{id}: missing {expected}");
        }
    }
    for (id, target) in [
        ("SkillCard_AquaJet", "#details/active/move/AquaJet"),
        ("PalPassiveSkillChange_Consumable_Vampire", "#details/surgery/trait/Vampire"),
    ] {
        let e = h.entry("items", id);
        h.warm("items", &e, &e.route);
        assert_eq!(h.click("items", &e, &e.route, target).0.as_deref(), Some(target));
    }
}

#[test]
fn catalogue_images_resolve_to_decodable_bundled_art() {
    let h = Harness::new();
    let mut no_rank = h.c.pages["details/passive"][0].clone();
    no_rank.value["rank"] = json!(0);
    assert!(entry_image(&h.d, "details/passive", &no_rank).is_none());
    let mut checked = std::collections::HashSet::new();
    for page in [
        "paldex",
        "details/partner",
        "details/active",
        "details/fruits",
        "details/passive",
        "details/surgery",
        "work/facilities",
        "guide/gathering",
        "work/production",
        "guide/journals",
        "enemies",
        "merchants",
        "neutrals",
        "items",
    ] {
        let mut resolved = 0;
        for e in &h.c.pages[page] {
            let source = entry_image(&h.d, page, e);
            // These sections all have a documented source for every entry. A
            // missing connection used to silently erase both thumbnail and hero.
            if page != "items" {
                assert!(source.is_some(), "missing image for {page}/{}", e.id);
            }
            let Some(source) = source else {
                continue;
            };
            resolved += 1;
            let (name, bytes) = match source {
                egui::ImageSource::Bytes { uri, bytes } => (uri.to_string(), bytes.to_vec()),
                egui::ImageSource::Uri(uri) => {
                    let name = uri.to_string();
                    if !checked.insert(name.clone()) {
                        continue;
                    }
                    let path = name.strip_prefix("file:///").expect("bundled file image");
                    let bytes = std::fs::read(path)
                        .unwrap_or_else(|error| panic!("{page}/{}: {name}: {error}", e.id));
                    (name, bytes)
                }
                egui::ImageSource::Texture(_) => continue,
            };
            if name.ends_with(".svg") {
                assert!(
                    std::str::from_utf8(&bytes).unwrap().contains("<svg"),
                    "invalid SVG: {name}"
                );
            } else {
                image::load_from_memory(&bytes)
                    .unwrap_or_else(|error| panic!("invalid image {name}: {error}"));
            }
        }
        println!(
            "{page}: {resolved}/{} connected images",
            h.c.pages[page].len()
        );
    }
}

struct Harness {
    ctx: egui::Context,
    d: Data,
    c: Catalog,
    state: DetailState,
    surgery: Value,
    time: f64,
    width: f32,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::design::apply(&ctx, "jade");
        let mut d = Data::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
            ctx.clone(),
        )
        .unwrap();
        let start = std::time::Instant::now();
        while d.pending > 0 && start.elapsed().as_secs() < 15 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            d.poll();
        }
        assert!(d.errors.is_empty());
        assert_eq!(d.pending, 0);
        let mut c = Catalog::default();
        c.rebuild(&d);
        Self {
            ctx,
            d,
            c,
            state: DetailState::default(),
            surgery: json!([]),
            time: 0.,
            width: 1100.,
        }
    }
    fn entry(&self, page: &str, id: &str) -> Entry {
        self.c.pages[page]
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing {page}/{id}"))
            .clone()
    }
    fn frame(
        &mut self,
        page: &str,
        e: &Entry,
        path: &str,
        events: Vec<egui::Event>,
    ) -> (Option<String>, String) {
        self.time += 0.08;
        self.ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("catalog-controls"),
                Vec::<(String, egui::Rect)>::new(),
            )
        });
        let mut nav = None;
        let output = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(self.width, 20000.),
                )),
                time: Some(self.time),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    nav = show(
                        ui,
                        &self.d,
                        page,
                        e,
                        path,
                        &mut self.surgery,
                        &mut self.state,
                    )
                });
            },
        );
        fn collect(shape: &egui::epaint::Shape, out: &mut String) {
            match shape {
                egui::epaint::Shape::Text(t) => {
                    out.push_str(&t.galley.job.text);
                    out.push('\n');
                }
                egui::epaint::Shape::Vec(a) => {
                    for x in a {
                        collect(x, out)
                    }
                }
                _ => {}
            }
        }
        let mut text = String::new();
        for shape in output.shapes {
            collect(&shape.shape, &mut text);
        }
        (nav, text)
    }
    fn warm(&mut self, page: &str, e: &Entry, path: &str) -> String {
        self.frame(page, e, path, vec![]);
        self.frame(page, e, path, vec![]).1
    }
    fn controls(&self) -> Vec<(String, egui::Rect)> {
        self.ctx.data(|d| {
            d.get_temp(egui::Id::new("catalog-controls"))
                .unwrap_or_default()
        })
    }
    fn click(&mut self, page: &str, e: &Entry, path: &str, key: &str) -> (Option<String>, String) {
        let pos = self
            .controls()
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("missing clickable {key}"))
            .1
            .center();
        self.frame(
            page,
            e,
            path,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        self.frame(
            page,
            e,
            path,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        )
    }
}

#[test]
fn reward_names_keep_readable_width_in_a_detail_pane() {
    let mut h = Harness::new();
    h.width = 600.;
    let e = h.entry("work/expeditions", "Expedition_WorldTree");
    h.warm("work/expeditions", &e, &e.route);
    let items: Vec<_> = h
        .controls()
        .into_iter()
        .filter(|(key, _)| key.starts_with("#items/"))
        .collect();
    assert!(items.len() > 3);
    for (key, rect) in items {
        assert!(
            rect.height() <= 28.,
            "{key} wrapped into unreadable stacked text: {rect:?}"
        );
        assert!(
            rect.right() < h.width,
            "{key} overflows detail pane: {rect:?}"
        );
    }
    let e = h.entry("details/active", "ThunderFunnel");
    h.warm("details/active", &e, &e.route);
    let pals: Vec<_> = h
        .controls()
        .into_iter()
        .filter(|(key, _)| key.starts_with("#paldex/"))
        .collect();
    assert!(pals.len() > 1);
    for (key, rect) in pals {
        assert!(
            rect.height() <= 28.,
            "{key} wrapped into stacked text: {rect:?}"
        );
        assert!(
            rect.right() < h.width,
            "{key} overflows detail pane: {rect:?}"
        );
    }
}

#[test]
fn semantic_detail_pages_render_named_sections_without_raw_json() {
    let mut h = Harness::new();
    for (page, title) in [
        ("paldex", "基础属性"),
        ("details/partner", "技能效果"),
        ("details/active", "技能数值"),
        ("details/passive", "词条效果"),
        ("details/fruits", "对应技能果实"),
        ("details/surgery", "手术台添加"),
        ("items", "合成表"),
        ("enemies", "击败掉落"),
        ("merchants", "地图位置"),
        ("neutrals", "工作适应性"),
        ("work/facilities", "建造材料"),
        ("work/expeditions", "派遣条件"),
        ("guide/journals", "手记原文"),
        ("guide/status", "状态效果"),
        ("guide/quests", "任务奖励"),
        ("guide/loot", "可能获得的物品"),
        ("guide/gathering", "地图位置"),
        ("work/production", "生产条件"),
    ] {
        let entries = &h.c.pages[page];
        let e = if page == "neutrals" {
            entries
                .iter()
                .find(|e| !array(&e.value["work"]).is_empty())
                .unwrap()
        } else if page == "merchants" {
            entries
                .iter()
                .find(|e| !array(&e.value["points"]).is_empty())
                .unwrap()
        } else if page == "guide/loot" {
            entries.iter().find(|e| e.value["slotted"] != true).unwrap()
        } else if page == "work/facilities" {
            entries
                .iter()
                .find(|e| !array(&e.value["materials"]).is_empty())
                .unwrap()
        } else {
            &entries[0]
        }
        .clone();
        let rendered = h.warm(page, &e, &e.route);
        assert!(rendered.contains(title), "{page}: missing {title}");
        for field in [
            "speciesKey",
            "sourceIds",
            "dropTiers",
            "slotted",
            "buildWork",
            "[null",
            "\"recipes\"",
        ] {
            assert!(
                !rendered.contains(field),
                "{page}: raw field leaked {field}"
            );
        }
    }
}
#[test]
fn partner_level_and_surgery_checkbox_respond_to_native_pointer_events() {
    let mut h = Harness::new();
    let e = h.entry("details/partner", "Anubis");
    assert!(h.warm("details/partner", &e, &e.route).contains("(30~50)"));
    h.click("details/partner", &e, &e.route, "partner-level/3");
    assert_eq!(h.state.partner_level, 3);
    assert!(h.warm("details/partner", &e, &e.route).contains("40%"));
    assert_eq!(
        h.click("details/partner", &e, &e.route, "#paldex/Anubis")
            .0
            .as_deref(),
        Some("#paldex/Anubis")
    );
    let sheep = h.entry("details/partner", "SheepBall");
    h.warm("details/partner", &sheep, &sheep.route);
    assert_eq!(
        h.click("details/partner", &sheep, &sheep.route, "#items/Wool")
            .0
            .as_deref(),
        Some("#items/Wool")
    );
    let e = h.c.pages["details/surgery"]
        .iter()
        .find(|e| e.value["surgery"]["mode"] == "consume")
        .unwrap()
        .clone();
    h.warm("details/surgery", &e, &e.route);
    h.click("details/surgery", &e, &e.route, "surgery-owned");
    assert!(array(&h.surgery).iter().any(|x| x == &e.id));
    h.warm("details/surgery", &e, &e.route);
    h.click("details/surgery", &e, &e.route, "surgery-owned");
    assert!(!array(&h.surgery).iter().any(|x| x == &e.id));
    assert_eq!(
        h.click("details/surgery", &e, &e.route, "#planner")
            .0
            .as_deref(),
        Some("#planner")
    );
}
#[test]
fn item_recipe_choice_quantity_and_shop_coordinates_are_live() {
    let mut h = Harness::new();
    let e = h.entry("items", "CarbonFiber");
    h.warm("items", &e, &e.route);
    let before = h.state.craft_plan.clone();
    h.click("items", &e, &e.route, "craft-source/CarbonFiber/open");
    h.frame("items", &e, &e.route, vec![]);
    h.click("items", &e, &e.route, "craft-source/CarbonFiber/1");
    h.warm("items", &e, &e.route);
    assert_eq!(h.state.choices["CarbonFiber"], 1);
    assert_ne!(h.state.craft_plan["external"], before["external"]);
    // DragValue opens text editing on double-click, then accepts typed quantities.
    h.click("items", &e, &e.route, "craft-quantity");
    h.click("items", &e, &e.route, "craft-quantity");
    h.frame(
        "items",
        &e,
        &e.route,
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
            egui::Event::Text("3".into()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
    h.warm("items", &e, &e.route);
    assert_eq!(h.state.quantity, 3);
    let other = h.entry("items", "PalSphere");
    h.warm("items", &other, &other.route);
    assert_eq!(h.state.quantity, 1);
    assert_eq!(
        h.click("items", &other, &other.route, "#items/PalSphere/shops")
            .0
            .as_deref(),
        Some("#items/PalSphere/shops")
    );
    let path = "#items/PalSphere/shops";
    let content = h.warm("items", &other, path);
    assert!(content.contains("售价") && content.contains("坐标"));
    let points: Vec<_> = h
        .controls()
        .into_iter()
        .filter(|(key, _)| key.starts_with("#map/point/"))
        .collect();
    assert!(points.len() > 1);
    assert_eq!(
        h.click("items", &other, path, &points[0].0).0,
        Some(points[0].0.clone())
    );
    h.warm("items", &e, &e.route);
    assert_eq!(h.state.quantity, 3);
    assert_eq!(h.state.choices["CarbonFiber"], 1);
}
#[test]
fn pal_drop_tier_and_quest_journal_links_are_live() {
    let mut h = Harness::new();
    let e = h.entry("paldex", "Anubis");
    h.warm("paldex", &e, &e.route);
    h.click("paldex", &e, &e.route, "drop-tier/open");
    h.frame("paldex", &e, &e.route, vec![]);
    h.click("paldex", &e, &e.route, "drop-tier/1");
    assert_eq!(h.state.drop_tier, 1);
    assert!(h.warm("paldex", &e, &e.route).contains("× 10–20"));
    let q = h.c.pages["guide/quests"]
        .iter()
        .find(|e| {
            array(&e.value["objectives"])
                .iter()
                .any(|r| r["site"].is_array() && r["point"].is_null())
        })
        .unwrap()
        .clone();
    h.warm("guide/quests", &q, &q.route);
    let key = h
        .controls()
        .iter()
        .find(|(k, _)| k.starts_with("#map/coords/"))
        .unwrap()
        .0
        .clone();
    assert_eq!(h.click("guide/quests", &q, &q.route, &key).0, Some(key));
    let e = h.c.pages["guide/journals"][0].clone();
    let content = h.warm("guide/journals", &e, &e.route);
    assert!(content.contains(text(&e.value, "body").split("\n\n").next().unwrap()));
    let key = format!("#map/point/{}", text(&e.value, "pointId"));
    assert_eq!(h.click("guide/journals", &e, &e.route, &key).0, Some(key));
}
#[test]
fn reward_item_subpage_filters_real_rows_and_returns_to_full_pool() {
    let mut h = Harness::new();
    let e = h.c.pages["guide/loot"]
        .iter()
        .find(|e| array(&e.value["rows"]).len() > 3)
        .unwrap()
        .clone();
    let rows = array(&e.value["rows"]);
    let id = rows[0][0].as_str().unwrap();
    let path = format!("#guide/loot/{}/item/{id}", e.id.split('/').next().unwrap());
    let text = h.warm("guide/loot", &e, &path);
    assert!(text.contains("当前仅显示"));
    let shown: Vec<_> = h
        .controls()
        .into_iter()
        .filter(|(k, _)| k.starts_with("#items/"))
        .collect();
    assert!(!shown.is_empty());
    assert!(shown.iter().all(|(k, _)| k == &format!("#items/{id}")));
    assert_eq!(
        h.click("guide/loot", &e, &path, &e.route).0,
        Some(e.route.clone())
    );
    h.warm("guide/loot", &e, &e.route);
    let all: std::collections::BTreeSet<_> = h
        .controls()
        .into_iter()
        .filter(|(k, _)| k.starts_with("#items/"))
        .map(|x| x.0)
        .collect();
    assert!(all.len() > 1);
}

#[test]
fn facility_product_search_and_world_reference_actions_are_live() {
    let mut h = Harness::new();
    let e = h.entry("work/facilities", "Primitive_Workbench");
    h.warm("work/facilities", &e, &e.route);
    h.click("work/facilities", &e, &e.route, "facility-product-search");
    h.frame(
        "work/facilities",
        &e,
        &e.route,
        vec![egui::Event::Text("Arrow_Fire".into())],
    );
    let content = h.warm("work/facilities", &e, &e.route);
    assert_eq!(h.state.product_query, "Arrow_Fire");
    assert!(content.contains("1 项"));
    assert_eq!(
        h.click("work/facilities", &e, &e.route, "#items/Arrow_Fire")
            .0
            .as_deref(),
        Some("#items/Arrow_Fire")
    );

    for (page, prefix) in [
        ("work/expeditions", "#map/point/"),
        ("merchants", "#items/"),
        ("enemies", "#items/"),
        ("guide/status", "#details/active/move/"),
        ("guide/gathering", "#map/"),
        ("work/production", "#items/"),
    ] {
        let candidates = h.c.pages[page].clone();
        let (e, href) = candidates
            .into_iter()
            .find_map(|e| {
                h.warm(page, &e, &e.route);
                h.controls()
                    .iter()
                    .find(|(key, _)| key.starts_with(prefix))
                    .map(|(key, _)| (e, key.clone()))
            })
            .unwrap_or_else(|| panic!("missing {page} action {prefix}"));
        assert_eq!(h.click(page, &e, &e.route, &href).0, Some(href));
    }
}
