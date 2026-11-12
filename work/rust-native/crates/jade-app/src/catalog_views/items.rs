use super::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    d: &Data,
    e: &Entry,
    path: &str,
    state: &mut DetailState,
    nav: &mut Option<String>,
) {
    let v = &e.value;
    let book = d.get("item-catalog-data");
    let shops = path.ends_with("/shops");
    if shops {
        route(ui, e.route.clone(), "← 返回物品详情", nav);
    }
    let rarity = match v["rarity"].as_i64().unwrap_or(0) {
        0 => "普通",
        1 => "优秀",
        2 => "稀有",
        3 => "史诗",
        4 => "传说",
        _ => "特殊",
    };
    hero(
        ui,
        d,
        e,
        "item",
        &format!("{} · {}", text(v, "category"), rarity),
    );
    if !text(v, "en").is_empty() {
        ui.weak(text(v, "en"));
    }
    ui.small(format!("物品编号 · {}", e.id));
    let sellers = array(&v["sellers"]);
    if shops {
        section(ui, "出售商人 · 点击坐标定位", |ui| {
            if sellers.is_empty() {
                ui.label("尚未收录出售此物品的商人。");
            }
            for seller in sellers {
                seller_card(ui, d, seller, nav);
            }
        });
        return;
    }
    effects(ui, d, &e.id, nav);
    if !sellers.is_empty() {
        section(ui, "商人出售", |ui| {
            let points: std::collections::BTreeSet<_> = sellers
                .iter()
                .flat_map(|s| array(&book["shops"][text(s, "shop")]["points"]))
                .filter_map(Value::as_str)
                .collect();
            ui.label(format!(
                "{} 张货单 · {} 个固定位置",
                sellers.len(),
                points.len()
            ));
            ui.horizontal_wrapped(|ui| {
                action(
                    ui,
                    format!("#items/{}/shops", e.id),
                    "查看商人、售价与坐标",
                    nav,
                );
                if sellers.len() == 1 && points.len() == 1 {
                    action(
                        ui,
                        format!("#map/point/{}", points.first().unwrap()),
                        "直接定位商人",
                        nav,
                    );
                }
            });
        });
    }
    if !text(v, "note").is_empty() {
        section(ui, "物品说明", |ui| {
            ui.label(text(v, "note"));
        });
    }
    for (field, title) in [("related", "关联条目"), ("currencies", "接受此货币的商人")]
    {
        if !array(&book[field][&e.id]).is_empty() {
            section(ui, title, |ui| {
                routes(ui, &book[field][&e.id], nav);
            });
        }
    }
    if !array(&v["ruins"]).is_empty() {
        section(ui, "远古遗迹制造图", |ui| {
            for id in array(&v["ruins"]).iter().filter_map(Value::as_str) {
                let p = &book["ruins"][id];
                ui.label(format!("遗迹奖励 · {}", text(p, "rewardName")));
                point_link(ui, d, id, nav);
            }
        });
    }
    section(ui, "合成表", |ui| {
        if array(&v["recipes"]).is_empty() {
            ui.label("未收录此物品的制作配方。");
        } else {
            for (i, r) in array(&v["recipes"]).iter().enumerate() {
                ui.push_id(i, |ui| {
                    recipe(ui, d, r, i, nav);
                });
            }
        }
    });
    if !array(&v["recipes"]).is_empty() {
        craft(ui, d, e, state, nav);
    }
    let cache = format!("{}:{}", e.id, d.tables.len());
    if state.methods_key != cache {
        state.methods = acquisition_groups(d, &e.id, v);
        state.methods_key = cache;
    }
    section(ui, "获取途径", |ui| {
        if state.methods.is_empty() {
            ui.label(if v["other"] == true {
                "游戏表中保留了这个条目，尚未核实玩家能否取得。"
            } else {
                "当前资料尚未给出可核实的获取途径。"
            });
        }
        for (kind, rows) in &state.methods {
            ui.strong(kind);
            let draw = |ui: &mut egui::Ui, rows: &[(String, String)], nav: &mut Option<String>| {
                ui.horizontal_wrapped(|ui| {
                    for (h, n) in rows {
                        if h.is_empty() {
                            resolved(ui, d, n, nav);
                        } else {
                            route(ui, h, n, nav);
                        }
                    }
                });
            };
            draw(ui, &rows[..rows.len().min(12)], nav);
            if rows.len() > 12 {
                egui::CollapsingHeader::new(format!("另有 {} 条{kind}", rows.len() - 12))
                    .id_salt(("methods", kind))
                    .show(ui, |ui| draw(ui, &rows[12..], nav));
            }
            ui.add_space(6.);
        }
    });
    uses(ui, d, &e.id, nav);
    if !text(v, "slug").is_empty() {
        ui.add_space(12.);
        ui.hyperlink_to(
            "PalDB · 物品资料来源 ↗",
            format!("https://paldb.cc/cn/{}", text(v, "slug")),
        );
    }
}
fn effects(ui: &mut egui::Ui, d: &Data, id: &str, nav: &mut Option<String>) {
    section(ui, "物品效果与属性", |ui| {
        if !d.ready("item-effects-data") {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("正在读取物品效果…");
            });
            return;
        }
        let v = &d.get("item-effects-data")["items"][id];
        if !text(v, "description").is_empty() {
            ui.label(text(v, "description"));
        }
        let values: Vec<_> = array(&v["stats"])
            .iter()
            .map(|r| {
                (
                    r[0].as_str().unwrap_or(""),
                    r[1].as_str().unwrap_or("").to_owned(),
                )
            })
            .collect();
        stats(ui, &values);
        for effect in array(&v["effects"]).iter().filter_map(Value::as_str) {
            ui.label(format!("• {effect}"));
        }
        if !text(&v["passive"], "key").is_empty() {
            route(ui, format!("#details/surgery/trait/{}", text(&v["passive"], "key")),
                format!("手术台词条 · {}", text(&v["passive"], "name")), nav);
        }
        if !text(v, "skill").is_empty() {
            let key = text(v, "skill");
            let title = text(&d.get("pal-details-data")["moves"][key], "zh");
            route(
                ui,
                format!("#details/active/move/{key}"),
                format!("使用后学习 · {title}"),
                nav,
            );
        }
        if !array(&v["relatedItems"]).is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("说明中提到的物品：");
                for key in array(&v["relatedItems"]).iter().filter_map(Value::as_str) {
                    item(ui, d, key, "", nav);
                }
            });
        }
    });
}
fn point_link(ui: &mut egui::Ui, d: &Data, id: &str, nav: &mut Option<String>) {
    let book = d.get("item-catalog-data");
    let p = if book["points"][id].is_null() {
        &book["ruins"][id]
    } else {
        &book["points"][id]
    };
    route(
        ui,
        format!("#map/point/{id}"),
        format!(
            "地图定位 · {}（{}，{}）",
            text(p, "nearest"),
            number(&p["x"]),
            number(&p["y"])
        ),
        nav,
    );
}
fn seller_card(ui: &mut egui::Ui, d: &Data, s: &Value, nav: &mut Option<String>) {
    let shop = &d.get("item-catalog-data")["shops"][text(s, "shop")];
    ui.add_space(8.);
    ui.strong(text(shop, "name"));
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "售价 {} / {} 件",
            number(&s["cost"]),
            number(&s["qty"])
        ));
        item(ui, d, text(s, "currency"), "", nav);
        if s["once"] == true {
            ui.weak("限购一次");
        }
    });
    for id in array(&shop["points"]).iter().filter_map(Value::as_str) {
        point_link(ui, d, id, nav);
    }
    routes(ui, &shop["units"], nav);
    if !text(shop, "note").is_empty() {
        ui.weak(text(shop, "note"));
    }
}
fn station(ui: &mut egui::Ui, d: &Data, name: &str, nav: &mut Option<String>) {
    if let Some(key) = d.get("item-catalog-data")["facilityLinks"][name].as_str() {
        route(ui, format!("#work/facilities/{key}"), name, nav);
    } else {
        ui.label(name);
    }
}
fn recipe(ui: &mut egui::Ui, d: &Data, r: &Value, index: usize, nav: &mut Option<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("配方 {} · 产出 × {}", index + 1, number(&r["qty"])));
        for name in array(&r["stations"]).iter().filter_map(Value::as_str) {
            station(ui, d, name, nav);
        }
    });
    for m in array(&r["materials"]) {
        item(
            ui,
            d,
            m[0].as_str().unwrap_or(""),
            &format!("× {}", number(&m[1])),
            nav,
        );
    }
    ui.horizontal_wrapped(|ui| {
        if !r["tech"].is_null() && r["tech"] != 0 {
            ui.weak(format!("科技等级 {}", number(&r["tech"])));
        }
        if let Some(plan) = r["plan"].as_str().filter(|s| *s != "None") {
            item(ui, d, plan, "需要设计图", nav);
        }
    });
    ui.add_space(8.);
}
fn compute_plan(d: &Data, e: &Entry, state: &mut DetailState) {
    let key = format!("{}|{}|{}", e.id, state.quantity, state.choices);
    if state.craft_key != key {
        state.craft_plan = jade_core::plan_item_craft(
            &d.get("item-catalog-data")["items"],
            &e.id,
            state.quantity,
            &state.choices,
        )
        .unwrap_or(Value::Null);
        state.craft_key = key;
    }
}
fn source_picker(
    ui: &mut egui::Ui,
    d: &Data,
    id: &str,
    recipes: &[Value],
    root: bool,
    choices: &mut Value,
) -> bool {
    if !choices.is_object() {
        *choices = json!({});
    }
    let mut value = choices[id].as_i64().unwrap_or(if root { 0 } else { -2 });
    let before = value;
    let _combo = egui::ComboBox::from_id_salt(("craft-source", id))
        .selected_text(match value {
            -2 => "自动展开".into(),
            -1 => "直接取得".into(),
            n => format!("配方 {}", n + 1),
        })
        .width(245.)
        .show_ui(ui, |ui| {
            if !root {
                for (n, label) in [(-2, "自动展开"), (-1, "直接取得")] {
                    let _response = ui.selectable_value(&mut value, n, label);
                    #[cfg(test)]
                    track(ui, &_response, &format!("craft-source/{id}/{n}"));
                }
            }
            for (i, r) in recipes.iter().enumerate() {
                let input = array(&r["materials"])
                    .iter()
                    .map(|m| {
                        format!(
                            "{} ×{}",
                            d.item_name(m[0].as_str().unwrap_or("")),
                            number(&m[1])
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("、");
                let _response = ui.selectable_value(
                    &mut value,
                    i as i64,
                    format!("配方 {} · {} → ×{}", i + 1, input, number(&r["qty"])),
                );
                #[cfg(test)]
                track(ui, &_response, &format!("craft-source/{id}/{i}"));
            }
        });
    #[cfg(test)]
    track(ui, &_combo.response, &format!("craft-source/{id}/open"));
    if value != before {
        if value == -2 {
            choices.as_object_mut().unwrap().remove(id);
        } else {
            choices[id] = json!(value);
        }
        true
    } else {
        false
    }
}
fn craft(
    ui: &mut egui::Ui,
    d: &Data,
    e: &Entry,
    state: &mut DetailState,
    nav: &mut Option<String>,
) {
    section(ui, "合成路径", |ui| {
        state.quantity = state.quantity.max(1);
        ui.horizontal_wrapped(|ui| {
            ui.label("目标数量");
            let _response = ui.add(egui::DragValue::new(&mut state.quantity).range(1..=99999));
            #[cfg(test)]
            track(ui, &_response, "craft-quantity");
            if array(&e.value["recipes"]).len() > 1 {
                ui.label("成品配方");
                source_picker(
                    ui,
                    d,
                    &e.id,
                    array(&e.value["recipes"]),
                    true,
                    &mut state.choices,
                );
            }
        });
        compute_plan(d, e, state);
        let nodes = array(&state.craft_plan["nodes"]).to_vec();
        let adjustable: Vec<_> = nodes
            .iter()
            .filter(|n| n["id"] != e.id)
            .filter_map(|n| {
                array(&d.get("item-catalog-data")["items"])
                    .iter()
                    .find(|i| i["id"] == n["id"])
            })
            .filter(|i| !array(&i["recipes"]).is_empty())
            .collect();
        if !adjustable.is_empty() {
            egui::CollapsingHeader::new("调整中间材料的获取方式")
                .id_salt("craft-material-options")
                .show(ui, |ui| {
                    for i in adjustable {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(text(i, "name"));
                            source_picker(
                                ui,
                                d,
                                text(i, "id"),
                                array(&i["recipes"]),
                                false,
                                &mut state.choices,
                            );
                        });
                    }
                });
        }
        compute_plan(d, e, state);
        let plan = &state.craft_plan;
        if plan.is_null() {
            ui.label("暂时无法计算这组配方。");
            return;
        }
        if !array(&plan["cycles"]).is_empty() {
            ui.colored_label(
                egui::Color32::from_rgb(160, 70, 50),
                "所选配方形成循环。相关原料已按直接取得计算；可更换中间材料的获取方式。",
            );
        }
        ui.add_space(8.);
        ui.strong("需备材料");
        for m in array(&plan["external"]) {
            item(
                ui,
                d,
                m[0].as_str().unwrap_or(""),
                &format!("× {}", number(&m[1])),
                nav,
            );
        }
        let blueprints: std::collections::BTreeSet<_> = array(&plan["steps"])
            .iter()
            .filter_map(|s| s["recipe"]["plan"].as_str())
            .filter(|x| *x != "None" && !x.is_empty())
            .collect();
        if !blueprints.is_empty() {
            ui.add_space(6.);
            ui.strong("所需设计图");
            for id in blueprints {
                item(ui, d, id, "", nav);
            }
        }
        ui.add_space(8.);
        ui.strong("制作顺序");
        for (i, step) in array(&plan["steps"]).iter().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("{}.", i + 1));
                    item(
                        ui,
                        d,
                        text(step, "id"),
                        &format!(
                            "产出 × {} · {} 批",
                            number(&step["made"]),
                            number(&step["batches"])
                        ),
                        nav,
                    );
                });
                ui.indent("step-input", |ui| {
                    for m in array(&step["inputs"]) {
                        item(
                            ui,
                            d,
                            m[0].as_str().unwrap_or(""),
                            &format!("× {}", number(&m[1])),
                            nav,
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        for station_name in array(&step["recipe"]["stations"])
                            .iter()
                            .filter_map(Value::as_str)
                        {
                            station(ui, d, station_name, nav);
                        }
                    });
                });
            });
        }
    });
}
fn acquisition_groups(d: &Data, id: &str, v: &Value) -> BTreeMap<String, Vec<(String, String)>> {
    let mut out: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let book = d.get("item-catalog-data");
    for m in array(&v["methods"]) {
        let kind = m[0].as_str().unwrap_or("其他方式");
        let description = m[1].as_str().unwrap_or("");
        let href = d
            .resolve(description)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        out.entry(kind.into())
            .or_default()
            .push((href, description.into()));
    }
    for (field, title, enemy) in [
        ("enemyDrops", "敌人掉落", true),
        ("enemyFirstRewards", "敌人首胜奖励", true),
        ("enemySummons", "可召唤的敌人", true),
        ("unitDrops", "人物掉落", false),
        ("unitRewards", "委托与评赏报酬", false),
    ] {
        for row in array(&book[field][id]) {
            out.entry(title.into()).or_default().push((
                if enemy {
                    format!("#enemies/{}", row[0].as_str().unwrap_or(""))
                } else {
                    row[0].as_str().unwrap_or("").into()
                },
                row[1].as_str().unwrap_or("").into(),
            ));
        }
    }
    if let Some(groups) = book["guideLinks"][id].as_object() {
        for (kind, rows) in groups {
            for row in array(rows) {
                if let (Some(h), Some(n)) = (row[0].as_str(), row[1].as_str()) {
                    out.entry(kind.into())
                        .or_default()
                        .push((h.into(), n.into()));
                }
            }
        }
    }
    for (h, n) in crate::links::item_routes(d, id, v) {
        if out
            .values()
            .any(|rows| rows.iter().any(|(href, _)| href == &h))
        {
            continue;
        }
        let kind = if h.starts_with("#work/ranch") {
            "牧场生产"
        } else if h.starts_with("#work/relic") {
            "遗物转换"
        } else if h.starts_with("#work/expeditions") {
            "帕鲁远征"
        } else if h.starts_with("#work/production") || h.starts_with("#work/facilities") {
            "据点生产与设施"
        } else if h.starts_with("#map") {
            "相关地图位置"
        } else if h.starts_with("#guide/quests") {
            "任务奖励"
        } else if h.starts_with("#guide/loot") {
            "探索奖励表"
        } else {
            "相关条目"
        };
        out.entry(kind.into()).or_default().push((h, n));
    }
    for rows in out.values_mut() {
        let mut seen = std::collections::BTreeSet::new();
        rows.retain(|r| seen.insert(r.clone()));
    }
    out
}
fn uses(ui: &mut egui::Ui, d: &Data, id: &str, nav: &mut Option<String>) {
    for (title, blueprint) in [("作为材料可制作", false), ("作为设计图可解锁", true)]
    {
        let found: Vec<_> = array(&d.get("item-catalog-data")["items"])
            .iter()
            .filter(|i| {
                array(&i["recipes"]).iter().any(|r| {
                    if blueprint {
                        r["plan"] == id
                    } else {
                        array(&r["materials"]).iter().any(|m| m[0] == id)
                    }
                })
            })
            .collect();
        if found.is_empty() {
            continue;
        }
        section(ui, &format!("{title} · {} 项", found.len()), |ui| {
            for i in found.iter().take(10) {
                item(ui, d, text(i, "id"), "", nav);
            }
            if found.len() > 10 {
                egui::CollapsingHeader::new(format!("其余 {} 项", found.len() - 10))
                    .id_salt(title)
                    .show(ui, |ui| {
                        for i in found.iter().skip(10) {
                            item(ui, d, text(i, "id"), "", nav);
                        }
                    });
            }
        });
    }
}
