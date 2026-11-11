use super::*;

pub(super) fn unit(ui: &mut egui::Ui, d: &Data, e: &Entry, page: &str, nav: &mut Option<String>) {
    let v = &e.value;
    let book = d.get("enemy-data");
    hero(
        ui,
        d,
        e,
        "",
        &format!("{} · {}", text(v, "group"), text(v, "variant")),
    );
    ui.horizontal_wrapped(|ui| {
        if !v["level"].is_null() {
            ui.strong(format!("Lv. {}", number(&v["level"])));
        }
        for field in ["weapon", "detailLabel"] {
            if !text(v, field).is_empty() {
                ui.label(text(v, field));
            }
        }
    });
    if !text(v, "note").is_empty() {
        ui.label(text(v, "note"));
    }
    if page != "enemies" && !array(&v["points"]).is_empty() {
        section(ui, "地图位置", |ui| locations(ui, &v["points"], nav));
    }
    if !array(&v["shops"]).is_empty() {
        section(ui, "出售物品", |ui| {
            for key in array(&v["shops"]).iter().filter_map(Value::as_str) {
                let shop = &book["shopLists"][key];
                ui.strong(text(shop, "name"));
                if !text(shop, "note").is_empty() {
                    ui.weak(text(shop, "note"));
                }
                egui::Grid::new(("shop-stock", key))
                    .num_columns(4)
                    .striped(true)
                    .spacing([12., 8.])
                    .show(ui, |ui| {
                        for h in ["商品", "每份数量", "价格", "限制"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for r in array(&shop["rows"]) {
                            item(ui, d, r[0].as_str().unwrap_or(""), "", nav);
                            ui.label(format!("× {}", number(&r[1])));
                            item(
                                ui,
                                d,
                                r[3].as_str().unwrap_or(""),
                                &format!("× {}", number(&r[2])),
                                nav,
                            );
                            ui.label(if r[4] == true { "限购一次" } else { "—" });
                            ui.end_row();
                        }
                    });
                ui.add_space(8.);
            }
        });
    }
    if v["palStock"].is_object() {
        let stock = &v["palStock"];
        section(ui, "可出售帕鲁", |ui| {
            ui.weak(format!(
                "每批从候选中抽取 {} 只，等级 Lv. {}–{}。",
                number(&stock["count"]),
                number(&stock["min"]),
                number(&stock["max"])
            ));
            for key in array(&stock["pals"]).iter().filter_map(Value::as_str) {
                pal(ui, d, key, "", nav);
            }
        });
    }
    if !array(&v["requests"]).is_empty() {
        section(ui, "委托条件与报酬", |ui| {
            for (i, r) in array(&v["requests"]).iter().enumerate() {
                ui.strong(format!("委托 {}", i + 1));
                if let Some(key) = r["pal"].as_str() {
                    pal(ui, d, key, "向该人物展示", nav);
                } else {
                    item(
                        ui,
                        d,
                        text(r, "item"),
                        &format!("交付 × {}", number(&r["qty"])),
                        nav,
                    );
                }
                ui.indent(("request", i), |ui| {
                    ui.weak("可获得");
                    for reward in array(&r["rewards"]) {
                        item(
                            ui,
                            d,
                            reward[0].as_str().unwrap_or(""),
                            &format!("× {}", number(&reward[1])),
                            nav,
                        );
                    }
                });
            }
        });
    }
    if !array(&book["questTargets"][&e.id]).is_empty() {
        section(ui, "相关任务", |ui| {
            for r in array(&book["questTargets"][&e.id]) {
                route(
                    ui,
                    format!("#guide/quests/{}", r[0].as_str().unwrap_or("")),
                    r[1].as_str().unwrap_or(""),
                    nav,
                );
            }
        });
    }
    if page == "enemies" || !array(&v["drops"]).is_empty() {
        section(
            ui,
            if page == "enemies" {
                "击败掉落"
            } else {
                "人物掉落"
            },
            |ui| {
                if !array(&v["drops"]).is_empty() {
                    if !array(&v["dropTiers"]).is_empty() {
                        ui.strong("基础档");
                    }
                    ui.push_id("base", |ui| rewards(ui, d, array(&v["drops"]), nav));
                }
                for (i, tier) in array(&v["dropTiers"]).iter().enumerate() {
                    ui.strong(format!("Lv. {} 起", number(&tier["level"])));
                    ui.push_id(i, |ui| rewards(ui, d, array(&tier["drops"]), nav));
                }
                if array(&v["drops"]).is_empty() && array(&v["dropTiers"]).is_empty() {
                    ui.label("未收录此单位的直接掉落记录。");
                }
            },
        );
    }
    for (field, title) in [
        ("firstReward", "首次胜利奖励"),
        ("summonItem", "召唤所需物品"),
    ] {
        if let Some(id) = v[field].as_str() {
            section(ui, title, |ui| item(ui, d, id, "", nav));
        }
    }
    if page == "enemies" && !array(&v["points"]).is_empty() {
        section(ui, "地图位置", |ui| locations(ui, &v["points"], nav));
    }
    if let Some(key) = v["palKey"].as_str() {
        section(ui, "帕鲁资料", |ui| pal(ui, d, key, "帕鲁图鉴", nav));
    }
    if !array(&v["work"]).is_empty() {
        section(ui, "工作适应性", |ui| {
            jobs(ui, d, &v["work"], None, nav)
        });
    }
}

pub(super) fn facility(
    ui: &mut egui::Ui,
    d: &Data,
    e: &Entry,
    state: &mut DetailState,
    nav: &mut Option<String>,
) {
    let v = &e.value;
    hero(
        ui,
        d,
        e,
        "",
        &format!("{} / {}", text(v, "category"), text(v, "group")),
    );
    if !text(v, "purpose").is_empty() {
        ui.add_space(8.);
        ui.label(text(v, "purpose"));
    }
    let mut facts = vec![];
    if v["level"].as_f64().unwrap_or(0.) > 0. {
        facts.push(("解锁科技", format!("Lv. {}", number(&v["level"]))));
    }
    if v["energy"] == "Electric" {
        facts.push(("能源", "需要供电".into()));
    }
    if v["buildWork"].as_f64().unwrap_or(0.) > 0. {
        facts.push(("建造工作量", number(&v["buildWork"])));
    }
    if !facts.is_empty() {
        section(ui, "设施信息", |ui| stats(ui, &facts));
    }
    if !array(&v["required"]).is_empty() || !array(&v["jobs"]).is_empty() {
        section(
            ui,
            if v["group"] == "效率强化" {
                "加成工作"
            } else {
                "工作适应性"
            },
            |ui| {
                jobs(ui, d, &v["required"], None, nav);
                ui.horizontal_wrapped(|ui| {
                    for job in array(&v["jobs"]).iter().filter_map(Value::as_str) {
                        if !array(&v["required"]).iter().any(|r| r[0] == job) {
                            route(ui, format!("#work/efficiency/{job}"), job, nav);
                        }
                    }
                });
            },
        );
    }
    if !array(&v["effects"]).is_empty() {
        section(ui, "设施效果", |ui| {
            for effect in array(&v["effects"]).iter().filter_map(Value::as_str) {
                ui.label(effect);
            }
            if v["group"] == "效率强化" {
                ui.weak("同种设施的据点效果不叠加。");
            }
        });
    }
    if !array(&v["materials"]).is_empty() {
        section(ui, "建造材料", |ui| {
            for m in array(&v["materials"]) {
                item(
                    ui,
                    d,
                    m[0].as_str().unwrap_or(""),
                    &format!("× {}", number(&m[1])),
                    nav,
                );
            }
        });
    }
    let products: Vec<_> = array(&d.get("item-catalog-data")["items"])
        .iter()
        .filter(|item| {
            array(&item["recipes"])
                .iter()
                .any(|r| array(&r["stations"]).iter().any(|s| s == &e.name))
        })
        .collect();
    if !products.is_empty() {
        section(
            ui,
            &format!("可制作物品 · {} 种", products.len()),
            |ui| {
                let _response = ui.add(
                    egui::TextEdit::singleline(&mut state.product_query)
                        .hint_text("查找可制作物品"),
                );
                #[cfg(test)]
                track(ui, &_response, "facility-product-search");
                let q = crate::data::normalize(&state.product_query);
                let shown: Vec<_> = products
                    .iter()
                    .filter(|r| {
                        q.is_empty()
                            || crate::data::normalize(&format!(
                                "{} {} {}",
                                text(r, "name"),
                                text(r, "id"),
                                text(r, "en")
                            ))
                            .contains(&q)
                    })
                    .collect();
                ui.weak(format!("{} 项", shown.len()));
                egui::ScrollArea::vertical()
                    .id_salt("facility-products")
                    .max_height(300.)
                    .show_rows(ui, 34., shown.len(), |ui, range| {
                        for i in range {
                            item(ui, d, text(shown[i], "id"), "", nav);
                        }
                    });
            },
        );
    }
    section(ui, "相关工具", |ui| {
        for p in array(&v["production"]) {
            route(
                ui,
                format!("#work/production/{}", p[0].as_str().unwrap_or("")),
                "查看据点生产条件与产物",
                nav,
            );
        }
        for (id, path, label) in [
            ("Ranch", "#work/ranch", "查看牧场产物"),
            (
                "Pal_Expedition_Station",
                "#work/expeditions",
                "查看远征要求与奖励",
            ),
            ("Ancient_Relic_Recycler", "#work/relic", "查询遗物转换"),
            ("Pal_Surgery_Table", "#details/surgery", "查看手术项目"),
            ("Skillfruit_Orchard", "#details/fruits", "查看技能果实"),
        ] {
            if e.id == id {
                action(ui, path, label, nav);
            }
        }
        let jobs: std::collections::BTreeSet<_> = array(&v["required"])
            .iter()
            .filter_map(|r| r[0].as_str())
            .chain(array(&v["jobs"]).iter().filter_map(Value::as_str))
            .filter(|x| *x != "任意")
            .collect();
        ui.horizontal_wrapped(|ui| {
            for job in jobs {
                route(
                    ui,
                    format!("#work/efficiency/{job}"),
                    format!("计算{job}效率"),
                    nav,
                );
            }
        });
    });
}
pub(super) fn expedition(ui: &mut egui::Ui, d: &Data, e: &Entry, nav: &mut Option<String>) {
    let v = &e.value;
    hero(
        ui,
        d,
        e,
        "",
        &format!(
            "{} · {}",
            text(v, "region"),
            if v["hard"] == true {
                "高难"
            } else {
                "普通"
            }
        ),
    );
    section(ui, "派遣条件", |ui| {
        stats(
            ui,
            &[
                ("基础时长", format!("{} 分钟", number(&v["minutes"]))),
                ("推荐战斗力", number(&v["power"])),
                (
                    "派遣属性",
                    if v["element"].is_array() {
                        format!(
                            "{}属性 ×{}",
                            number(&v["element"][0]),
                            number(&v["element"][1])
                        )
                    } else {
                        "无属性限制".into()
                    },
                ),
            ],
        );
        ui.add_space(8.);
        ui.label(text(v, "unlock"));
    });
    ui.add_space(10.);
    ui.horizontal_wrapped(|ui| {
        action(
            ui,
            format!("#map/point/{}", text(v, "point")),
            &format!("地图定位 · {}", text(v, "tower")),
            nav,
        );
        action(ui, "#details/stats", "计算帕鲁远征战力", nav);
        action(
            ui,
            "#work/facilities/Pal_Expedition_Station",
            "远征所建造材料",
            nav,
        );
    });
    section(ui, "可能获得的物品", |ui| {
        ui.weak("数量和概率对应奖励表单个栏位；同一物品可出现在多个栏位。重复栏位均保留。");
        rewards(ui, d, array(&v["rewards"]), nav);
    });
}
pub(super) fn journal(ui: &mut egui::Ui, d: &Data, e: &Entry, nav: &mut Option<String>) {
    let v = &e.value;
    hero(ui, d, e, "", text(v, "group"));
    ui.add_space(10.);
    ui.label(RichText::new(text(v, "summary")).italics());
    ui.add_space(10.);
    action(
        ui,
        format!("#map/point/{}", text(v, "pointId")),
        &format!(
            "地图定位 · {}（{}，{}）",
            if v["map"] == "worldtree" {
                "世界树"
            } else {
                "帕洛斯群岛"
            },
            number(&v["x"]),
            number(&v["y"])
        ),
        nav,
    );
    section(ui, "手记原文", |ui| {
        for paragraph in text(v, "body").split("\n\n") {
            ui.label(RichText::new(paragraph).size(16.));
            ui.add_space(12.);
        }
    });
    let rows = array(&d.get("journals-data")["catalog"]);
    if let Some(index) = rows.iter().position(|r| r["id"] == e.id) {
        ui.horizontal_wrapped(|ui| {
            if let Some(prev) = index.checked_sub(1).and_then(|i| rows.get(i)) {
                action(
                    ui,
                    format!("#guide/journals/{}", text(prev, "id")),
                    &format!("← {}", text(prev, "title")),
                    nav,
                );
            }
            route(ui, "#guide/journals", "全部手记", nav);
            if let Some(next) = rows.get(index + 1) {
                action(
                    ui,
                    format!("#guide/journals/{}", text(next, "id")),
                    &format!("{} →", text(next, "title")),
                    nav,
                );
            }
        });
    }
    if !text(v, "source").is_empty() {
        ui.add_space(10.);
        ui.hyperlink_to("手记原文出处 ↗", text(v, "source"));
    }
}
pub(super) fn status(ui: &mut egui::Ui, d: &Data, e: &Entry, nav: &mut Option<String>) {
    let v = &e.value;
    hero(ui, d, e, "", text(v, "group"));
    section(ui, "状态效果", |ui| {
        ui.label(text(v, "text"));
    });
    if v["medicine"].is_array() {
        section(ui, "应对方式", |ui| {
            item(
                ui,
                d,
                v["medicine"][0].as_str().unwrap_or(""),
                &format!("对应药品 · 等级 {}", number(&v["rank"])),
                nav,
            );
        });
    }
    if matches!(text(v, "group"), "技能异常" | "野外状态") {
        ui.add_space(10.);
        action(ui, "#details/capture", "前往捕捉概率计算", nav);
    }
    if !array(&v["moves"]).is_empty() {
        section(ui, "可施加该状态的主动技能", |ui| {
            ui.weak("数值是技能记录的积蓄值。");
            egui::Grid::new("status-moves")
                .striped(true)
                .spacing([15., 8.])
                .show(ui, |ui| {
                    for name in ["技能", "属性", "积蓄值"] {
                        ui.strong(name);
                    }
                    ui.end_row();
                    for r in array(&v["moves"]) {
                        route(
                            ui,
                            format!("#details/active/move/{}", r[3].as_str().unwrap_or("")),
                            r[0].as_str().unwrap_or(""),
                            nav,
                        );
                        element(ui, r[1].as_str().unwrap_or(""));
                        ui.label(number(&r[2]));
                        ui.end_row();
                    }
                });
        });
    }
}
fn objective(ui: &mut egui::Ui, d: &Data, r: &Value, nav: &mut Option<String>) {
    if let Some(id) = r["item"].as_str() {
        item(ui, d, id, &format!("× {}", number(&r["qty"])), nav);
        return;
    }
    let label = text(r, "text");
    let href = if let Some(point) = r["point"].as_str() {
        format!("#map/point/{point}")
    } else if let Some(site) = r["site"].as_array().filter(|s| s.len() == 3) {
        format!(
            "#map/coords/{}/{}/{}",
            site[0].as_str().unwrap_or("palpagos"),
            number(&site[1]),
            number(&site[2])
        )
    } else if let Some(pal) = r["pal"].as_str() {
        format!("#paldex/{pal}")
    } else if let Some(unit) = r["unit"].as_str() {
        d.get("adventure-data")["unitRoutes"][unit]
            .as_str()
            .unwrap_or("")
            .into()
    } else if let Some(key) = r["passive"].as_str() {
        format!("#details/passive/trait/{key}")
    } else {
        String::new()
    };
    if href.is_empty() {
        ui.label(label);
    } else {
        route(ui, href, label, nav);
    }
}
pub(super) fn quest(ui: &mut egui::Ui, d: &Data, e: &Entry, nav: &mut Option<String>) {
    let v = &e.value;
    hero(
        ui,
        d,
        e,
        "",
        &format!(
            "{}{}",
            text(v, "group"),
            if v["repeatable"] == true {
                " · 可重复接取"
            } else {
                ""
            }
        ),
    );
    if !text(v, "summary").is_empty() {
        ui.add_space(8.);
        ui.label(text(v, "summary"));
    }
    section(ui, "目标与地点", |ui| {
        for (i, r) in array(&v["objectives"]).iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{}.", i + 1));
                objective(ui, d, r, nav);
            });
        }
    });
    section(ui, "任务奖励", |ui| {
        if array(&v["rewards"]).is_empty() {
            ui.weak("任务表没有列出固定奖励。");
        }
        for r in array(&v["rewards"]) {
            objective(ui, d, r, nav);
        }
    });
    let quests = array(&d.get("adventure-data")["quests"]);
    let before: Vec<_> = quests
        .iter()
        .filter(|q| array(&q["next"]).iter().any(|id| id == &e.id))
        .collect();
    let next: Vec<_> = array(&v["next"])
        .iter()
        .filter_map(|id| quests.iter().find(|q| q["id"] == *id))
        .collect();
    for (title, rows) in [("前序任务", before), ("后续任务", next)] {
        if !rows.is_empty() {
            section(ui, title, |ui| {
                for q in rows {
                    route(
                        ui,
                        format!("#guide/quests/{}", text(q, "id")),
                        text(q, "name"),
                        nav,
                    );
                }
            });
        }
    }
}
pub(super) fn loot(ui: &mut egui::Ui, d: &Data, e: &Entry, path: &str, nav: &mut Option<String>) {
    let v = &e.value;
    hero(ui, d, e, "", text(v, "group"));
    contextual(ui, d, "guide/loot", e, nav);
    let only = path.split_once("/item/").map(|(_, id)| id);
    if let Some(id) = only {
        ui.add_space(8.);
        ui.label(format!("当前仅显示 {} 的奖励栏位", d.item_name(id)));
        route(ui, e.route.clone(), "查看完整奖池", nav);
    }
    let rows: Vec<_> = array(&v["rows"])
        .iter()
        .filter(|r| only.is_none_or(|id| r[0] == id))
        .cloned()
        .collect();
    ui.add_space(8.);
    ui.weak(if v["slotted"] == true {
        "百分比含该栏位的触发率。同一物品可出现在多个栏位，每行分别显示。"
    } else {
        "此奖池使用发布表的数量与单项获得率，未标出抽取栏位。"
    });
    if v["slotted"] == true {
        let mut slots: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
        for r in rows {
            slots.entry(r[4].as_u64().unwrap_or(0)).or_default().push(r);
        }
        for (slot, rows) in slots {
            ui.push_id(slot, |ui| {
                section(ui, &format!("抽取栏位 {slot}"), |ui| {
                    rewards(ui, d, &rows, nav);
                    for r in &rows {
                        if r[5].as_f64().unwrap_or(1.) > 1. {
                            ui.weak(format!(
                                "{}：数量间隔 {}",
                                d.item_name(r[0].as_str().unwrap_or("")),
                                number(&r[5])
                            ));
                        }
                    }
                });
            });
        }
    } else {
        section(ui, "可能获得的物品", |ui| rewards(ui, d, &rows, nav));
    }
}
pub(super) fn gathering(ui: &mut egui::Ui, d: &Data, e: &Entry, nav: &mut Option<String>) {
    let v = &e.value;
    hero(ui, d, e, "item", text(v, "group"));
    ui.add_space(8.);
    ui.label(text(v, "note"));
    if v["minutes"].as_f64().unwrap_or(0.) > 0. {
        section(ui, "刷新时间", |ui| {
            stats(
                ui,
                &[("基础刷新间隔", format!("{} 分钟", number(&v["minutes"])))],
            );
        });
    }
    if !array(&v["tools"]).is_empty() || v["pal"].is_array() {
        section(ui, "采集工具与条件", |ui| {
            for r in array(&v["tools"]) {
                item(ui, d, r[0].as_str().unwrap_or(""), "", nav);
            }
            if v["pal"].is_array() {
                route(
                    ui,
                    format!("#details/partner/{}", v["pal"][0].as_str().unwrap_or("")),
                    format!("{} · 伙伴技能", v["pal"][1].as_str().unwrap_or("")),
                    nav,
                );
            }
        });
    }
    section(ui, "地图位置", |ui| {
        for key in array(&v["pools"]).iter().filter_map(Value::as_str) {
            for pool in array(&d.get("resource-guides-data")["pools"][key]) {
                ui.strong(text(pool, "label"));
                if let Some(href) = pool["href"].as_str() {
                    route(
                        ui,
                        href,
                        format!("查看全部 {} 处", number(&pool["count"])),
                        nav,
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    for s in array(&pool["samples"]) {
                        route(
                            ui,
                            format!("#map/point/{}", s[0].as_str().unwrap_or("")),
                            format!("（{}，{}）", number(&s[1]), number(&s[2])),
                            nav,
                        );
                    }
                });
                ui.add_space(6.);
            }
        }
        if let (Some(href), Some(label)) = (v["areaLink"][0].as_str(), v["areaLink"][1].as_str()) {
            route(ui, href, label, nav);
        }
    });
    section(ui, "关联条目", |ui| {
        item(ui, d, &e.id, "物品图鉴", nav);
        for p in array(&d.get("resource-guides-data")["production"]) {
            if p["item"] == e.id {
                route(
                    ui,
                    format!("#work/production/{}", text(p, "id")),
                    format!("{} · 据点生产", text(p, "name")),
                    nav,
                );
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (i, url) in array(&v["sources"])
            .iter()
            .filter_map(Value::as_str)
            .enumerate()
        {
            ui.hyperlink_to(format!("资料来源 {} ↗", i + 1), url);
        }
    });
}
pub(super) fn production(ui: &mut egui::Ui, d: &Data, e: &Entry, nav: &mut Option<String>) {
    let v = &e.value;
    ui.horizontal_top(|ui| {
        d.item_icon(ui, text(v, "item"), 64.);
        ui.vertical(|ui| {
            ui.weak(if v["auto"].as_f64().unwrap_or(0.) > 0. {
                "自动提炼"
            } else {
                "据点生产"
            });
            ui.heading(&e.name);
        });
    });
    section(ui, "产物", |ui| {
        item(ui, d, text(v, "item"), text(v, "itemName"), nav)
    });
    section(ui, "生产条件", |ui| {
        ui.label(text(v, "condition"));
        let mut facts = vec![("基础工作量", number(&v["work"]))];
        if v["level"].as_f64().unwrap_or(0.) > 0. {
            facts.push(("解锁科技", format!("Lv. {}", number(&v["level"]))));
        }
        if v["power"].as_f64().unwrap_or(0.) > 0. {
            facts.push(("消耗电力", format!("{} / 秒", number(&v["power"]))));
        }
        if let Some(auto) = v["auto"].as_f64().filter(|n| *n > 0.) {
            facts.push(("自动工作量", format!("{} / 秒", number(&v["auto"]))));
            facts.push((
                "表内基础周期",
                format!("约 {:.1} 秒", v["work"].as_f64().unwrap_or(0.) / auto),
            ));
        }
        stats(ui, &facts);
        jobs(ui, d, &v["jobs"], None, nav);
    });
    contextual(ui, d, "work/production", e, nav);
}
