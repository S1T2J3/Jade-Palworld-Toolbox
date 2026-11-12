use super::*;

struct CardHarness {
    ctx: egui::Context,
    d: Data,
    c: Catalog,
    surgery: Value,
    time: f64,
}
impl CardHarness {
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
        assert_eq!(d.pending, 0);
        assert!(d.errors.is_empty());
        let mut c = Catalog::default();
        c.rebuild(&d);
        Self {
            ctx,
            d,
            c,
            surgery: json!([]),
            time: 0.,
        }
    }
    fn frame(
        &mut self,
        page: &str,
        e: &Entry,
        events: Vec<egui::Event>,
    ) -> (Option<String>, String) {
        self.time += 0.08;
        let mut nav = None;
        let output = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 1000.),
                )),
                time: Some(self.time),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Match the actual multi-column row: Frame inherits this
                    // horizontal layout unless the card resets it explicitly.
                    ui.horizontal_top(|ui| {
                        nav = skill_overview_card(ui, &self.d, page, e, 230., &mut self.surgery);
                        ui.allocate_exact_size(egui::vec2(100., 20.), egui::Sense::hover());
                    });
                });
            },
        );
        fn collect(shape: &egui::epaint::Shape, text: &mut String) {
            match shape {
                egui::epaint::Shape::Text(t) => {
                    text.push_str(&t.galley.job.text);
                    text.push('\n');
                }
                egui::epaint::Shape::Vec(children) => {
                    for child in children {
                        collect(child, text);
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
    fn click(&mut self, page: &str, e: &Entry, control: &str) -> Option<String> {
        let rect = self
            .ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new((control, &e.id))))
            .unwrap();
        let pos = rect.center();
        for pressed in [true, false] {
            let result = self.frame(
                page,
                e,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
            if !pressed {
                return result.0;
            }
        }
        None
    }
}

#[test]
fn skill_cards_stay_within_their_columns_and_stack_effects_above_controls() {
    let mut h = CardHarness::new();
    for page in [
        "details/active",
        "details/passive",
        "details/fruits",
        "details/surgery",
    ] {
        // Include the longest fruit name, which used to force the next column
        // beyond the right edge when its values shared the header's row.
        let e = h.c.pages[page]
            .iter()
            .max_by_key(|e| {
                if matches!(page, "details/passive" | "details/surgery") {
                    text(&e.value, "effect").chars().count()
                } else {
                    e.name.chars().count()
                }
            })
            .unwrap()
            .clone();
        h.frame(page, &e, vec![]);
        h.frame(page, &e, vec![]);
        let card = h
            .ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new(("catalog-rich-card", &e.id))))
            .unwrap();
        assert!(
            card.width() <= 232.,
            "{page} exceeds its 230px column: {card:?}"
        );
        assert!(
            card.height() < 500.,
            "{page} has vertical one-character wrapping: {card:?}"
        );
        let title = h
            .ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new(("catalog-rich-title", &e.id))))
            .unwrap();
        assert!(card.contains_rect(title), "{page} title outside card");
        if matches!(page, "details/passive" | "details/surgery") {
            let effect = h
                .ctx
                .data(|m| m.get_temp::<egui::Rect>(egui::Id::new(("catalog-rich-effect", &e.id))))
                .unwrap();
            assert!(
                effect.top() > title.bottom(),
                "{page} effect must follow the header, not share its row"
            );
            assert!(
                effect.width() > 180.,
                "{page} effect lost usable width: {effect:?}"
            );
            assert!(card.contains_rect(effect));
            if page == "details/surgery" {
                let owned = h
                    .ctx
                    .data(|m| {
                        m.get_temp::<egui::Rect>(egui::Id::new(("catalog-rich-owned", &e.id)))
                    })
                    .unwrap();
                assert!(owned.top() > effect.bottom());
                assert!(card.contains_rect(owned));
            }
        }
    }
}

#[test]
fn skill_overview_shows_complete_effect_and_directly_updates_implant_ownership() {
    let mut h = CardHarness::new();
    let mut e = h.c.pages["details/surgery"]
        .iter()
        .find(|e| e.value["surgery"]["mode"] == "consume")
        .unwrap()
        .clone();
    let effect = "工作速度提高50%，移动速度提高20%。这个较长的完整效果必须在230像素的卡片中换行显示，不应截断或要求先打开详情。";
    e.value["effect"] = json!(effect);
    h.frame("details/surgery", &e, vec![]);
    let text = h.frame("details/surgery", &e, vec![]).1;
    assert!(text.contains(effect));
    assert!(!array(&h.surgery).iter().any(|v| v == &e.id));
    assert!(
        h.click("details/surgery", &e, "catalog-rich-owned")
            .is_none()
    );
    assert!(array(&h.surgery).iter().any(|v| v == &e.id));
    assert!(
        h.click("details/surgery", &e, "catalog-rich-owned")
            .is_none()
    );
    assert!(!array(&h.surgery).iter().any(|v| v == &e.id));
    assert_eq!(
        h.click("details/surgery", &e, "catalog-rich-title"),
        Some(e.route.clone())
    );
}

#[test]
fn fruit_overview_name_opens_its_item_acquisition_page() {
    let mut h = CardHarness::new();
    let e = h.c.pages["details/fruits"][0].clone();
    let item = format!("SkillCard_{}", e.id);
    assert!(
        array(&h.d.get("item-catalog-data")["items"])
            .iter()
            .any(|v| v["id"] == item)
    );
    h.frame("details/fruits", &e, vec![]);
    h.frame("details/fruits", &e, vec![]);
    assert_eq!(
        h.click("details/fruits", &e, "catalog-rich-title"),
        Some(format!("#items/{item}"))
    );
}

#[test]
fn all_item_acquisition_categories_and_fuzzy_passive_filters_are_reachable() {
    let h = CardHarness::new();
    let none = json!([]);
    for (field, title) in [
        ("enemyFirstRewards", "敌人首胜奖励"),
        ("unitDrops", "人物掉落"),
        ("unitRewards", "委托与评赏报酬"),
    ] {
        let records = h.d.get("item-catalog-data")[field].as_object().unwrap();
        let e = h.c.pages["items"]
            .iter()
            .find(|e| records.get(&e.id).is_some_and(|v| !array(v).is_empty()))
            .unwrap();
        let filter = Filters {
            filter: title.into(),
            ..Default::default()
        };
        assert!(
            matches_filters(&h.d, "items", e, &filter, &none),
            "{title}: {}",
            e.id
        );
    }
    let e = h.c.pages["items"]
        .iter()
        .find(|e| !array(&e.value["ruins"]).is_empty())
        .unwrap();
    assert!(e.methods.contains("远古遗迹"));
    let e = h.c.pages["details/passive"]
        .iter()
        .find(|e| e.name.chars().count() >= 4)
        .unwrap();
    let query = format!(
        "{}{}",
        e.name.chars().next().unwrap(),
        e.name.chars().last().unwrap()
    );
    assert!(matches_filters(
        &h.d,
        "details/passive",
        e,
        &Filters {
            query,
            ..Default::default()
        },
        &none
    ));
    let ranks: Vec<_> = h.c.pages["details/passive"]
        .iter()
        .map(|e| e.value["rank"].as_i64().unwrap())
        .collect();
    assert!(ranks.windows(2).all(|r| r[0] >= r[1]));
}
