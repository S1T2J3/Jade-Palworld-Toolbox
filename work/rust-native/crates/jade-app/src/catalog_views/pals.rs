use super::*;

pub(super) fn paldex(
    ui: &mut egui::Ui,
    d: &Data,
    e: &Entry,
    state: &mut DetailState,
    nav: &mut Option<String>,
) {
    let v = &e.value;
    let base = &v["基础属性"];
    let extra = &v["详细资料"];
    let lessons = &v["技能与固定词条"];
    hero(
        ui,
        d,
        e,
        "pal",
        &format!("#{} · {}", text(v, "dex"), text(v, "en")),
    );
    ui.horizontal_wrapped(|ui| {
        for kind in array(&v["types"]).iter().filter_map(Value::as_str) {
            element(ui, kind);
        }
        if base["nocturnal"] == true {
            ui.weak("夜间也能工作");
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (path, label) in [
            ("spawns", "刷新位置"),
            ("lookup", "配种组合"),
            ("details/stats", "属性值计算"),
            ("details/capture", "捕捉概率"),
        ] {
            if path != "lookup" || text(v, "reason").is_empty() {
                action(ui, format!("#{path}/{}", e.id), label, nav);
            }
        }
    });
    let left = |ui: &mut egui::Ui, nav: &mut Option<String>| {
        section(ui, "基础属性", |ui| {
            stats(
                ui,
                &[
                    ("生命", number(&base["hp"])),
                    ("攻击", number(&base["attack"])),
                    ("防御", number(&base["defense"])),
                    ("食量", number(&extra["food"])),
                ],
            );
        });
        section(ui, "伙伴技能", |ui| {
            if text(base, "partner").is_empty() {
                ui.label("该形态暂无独立的伙伴技能记录。");
            } else {
                route(
                    ui,
                    format!("#details/partner/{}", e.id),
                    text(base, "partner"),
                    nav,
                );
                ui.label(text(extra, "partnerEffect"));
            }
        });
        section(ui, "工作适应性", |ui| {
            if array(&base["work"]).is_empty() {
                ui.label("无工作适应性。");
            } else {
                jobs(ui, d, &base["work"], Some(&e.id), nav);
            }
            ui.horizontal_wrapped(|ui| {
                route(
                    ui,
                    format!("#work/suitability/{}", e.id),
                    "查看工作排行 →",
                    nav,
                );
                route(
                    ui,
                    format!("#work/efficiency/{}", e.id),
                    "计算工作效率 →",
                    nav,
                );
            });
        });
        let ranch = &d.get("ranch-data")["pals"][&e.id];
        if !ranch.is_null() {
            section(ui, "牧场产物", |ui| {
                let ids: std::collections::BTreeSet<_> = array(&ranch["tiers"])
                    .iter()
                    .flat_map(array)
                    .filter_map(|r| r["item"].as_str())
                    .collect();
                for id in ids {
                    item(ui, d, id, "", nav);
                }
                route(
                    ui,
                    format!("#work/ranch/{}", e.id),
                    "查看各放牧等级产量 →",
                    nav,
                );
            });
        }
        if !text(v, "reason").is_empty() {
            section(ui, "配种条件", |ui| {
                ui.label(text(v, "reason"));
            });
        }
        if !array(&extra["sellers"]).is_empty() {
            section(ui, "可能出售此帕鲁的商人", |ui| {
                routes(ui, &extra["sellers"], nav);
            });
        }
        if !array(&extra["enemies"]).is_empty() {
            section(ui, "敌人形态", |ui| {
                for row in array(&extra["enemies"]) {
                    route(
                        ui,
                        format!("#enemies/{}", row[0].as_str().unwrap_or("")),
                        row[1].as_str().unwrap_or("头目资料"),
                        nav,
                    );
                }
            });
        }
    };
    let mut right = |ui: &mut egui::Ui, nav: &mut Option<String>| {
        section(ui, "升级习得技能", |ui| {
            let rows = array(&lessons["levels"]);
            if rows.is_empty() {
                ui.label("该形态没有已记录的升级习得技能。");
            } else {
                egui::Grid::new("pal-learnset")
                    .num_columns(4)
                    .striped(true)
                    .spacing([12., 9.])
                    .show(ui, |ui| {
                        for label in ["等级", "主动技能", "属性", "威力"] {
                            ui.strong(label);
                        }
                        ui.end_row();
                        for row in rows {
                            let key = row[1].as_str().unwrap_or("");
                            let m = &d.get("pal-details-data")["moves"][key];
                            ui.label(format!("Lv. {}", number(&row[0])));
                            route(
                                ui,
                                format!("#details/active/{}/{key}", e.id),
                                text(m, "zh"),
                                nav,
                            );
                            element(ui, text(m, "type"));
                            ui.label(number(&m["power"]));
                            ui.end_row();
                        }
                    });
            }
            route(
                ui,
                format!("#details/active/{}", e.id),
                "查看技能资料 →",
                nav,
            );
        });
        section(ui, "固定被动", |ui| {
            let mut fixed: Vec<_> = array(&lessons["fixed"])
                .iter()
                .filter_map(Value::as_str)
                .collect();
            fixed.sort_by_key(|key| {
                std::cmp::Reverse(
                    d.get("pal-details-data")["passives"][*key]["rank"]
                        .as_i64()
                        .unwrap_or(0),
                )
            });
            if fixed.is_empty() {
                ui.label("无已记录的固定词条。");
            }
            for key in fixed {
                let p = &d.get("pal-details-data")["passives"][key];
                ui.horizontal_wrapped(|ui| {
                    rank(ui, d, &p["rank"]);
                    route(
                        ui,
                        format!("#details/passive/{}/{key}", e.id),
                        text(p, "zh"),
                        nav,
                    );
                });
                ui.label(text(p, "effect"));
                ui.add_space(6.);
            }
            route(
                ui,
                format!("#details/passive/{}", e.id),
                "查看被动技能 →",
                nav,
            );
        });
        let tiers = array(&extra["drops"]);
        if !tiers.is_empty() {
            section(ui, "普通个体掉落", |ui| {
                state.drop_tier = state.drop_tier.min(tiers.len() - 1);
                let _combo = egui::ComboBox::from_id_salt("drop-tier")
                    .selected_text(tier_label(&tiers[state.drop_tier][0]))
                    .show_ui(ui, |ui| {
                        for (i, t) in tiers.iter().enumerate() {
                            let _response =
                                ui.selectable_value(&mut state.drop_tier, i, tier_label(&t[0]));
                            #[cfg(test)]
                            track(ui, &_response, &format!("drop-tier/{i}"));
                        }
                    });
                #[cfg(test)]
                track(ui, &_combo.response, "drop-tier/open");
                rewards(ui, d, array(&tiers[state.drop_tier][1]), nav);
            });
        }
    };
    if ui.available_width() > 780. {
        ui.columns(2, |cols| {
            left(&mut cols[0], nav);
            right(&mut cols[1], nav);
        });
    } else {
        left(ui, nav);
        right(ui, nav);
    }
    if let Some(index) = d.species.iter().position(|p| p["key"] == e.id) {
        ui.add_space(14.);
        ui.horizontal_wrapped(|ui| {
            if let Some(prev) = index.checked_sub(1).and_then(|i| d.species.get(i)) {
                action(
                    ui,
                    format!("#paldex/{}", text(prev, "key")),
                    &format!("← {}", text(prev, "zh")),
                    nav,
                );
            }
            route(ui, "#paldex", "返回帕鲁图鉴", nav);
            if let Some(next) = d.species.get(index + 1) {
                action(
                    ui,
                    format!("#paldex/{}", text(next, "key")),
                    &format!("{} →", text(next, "zh")),
                    nav,
                );
            }
        });
    }
}
fn tier_label(v: &Value) -> String {
    if v.as_i64() == Some(0) {
        "基础档".into()
    } else {
        format!("Lv. {} 起", number(v))
    }
}

pub(super) fn partner(
    ui: &mut egui::Ui,
    d: &Data,
    e: &Entry,
    state: &mut DetailState,
    nav: &mut Option<String>,
) {
    let v = &e.value;
    let mut title = e.clone();
    title.name = text(v, "name").into();
    let p = d.species.iter().find(|p| p["key"] == e.id);
    hero(
        ui,
        d,
        &title,
        "pal",
        &p.map(|p| format!("#{} · {}", text(p, "dex"), text(p, "zh")))
            .unwrap_or_default(),
    );
    ui.horizontal_wrapped(|ui| {
        for t in array(&v["tags"]).iter().filter_map(Value::as_str) {
            ui.label(
                RichText::new(match t {
                    "combat" => "战斗",
                    "work" => "工作",
                    "travel" => "骑乘与移动",
                    "explore" => "探索与负重",
                    "loot" => "掉落与垂钓",
                    "catch" => "捕捉",
                    "care" => "支援与恢复",
                    _ => t,
                })
                .strong(),
            );
        }
    });
    let leveled = v["curves"].is_object() || v["stats"].is_array();
    if leveled {
        ui.horizontal_wrapped(|ui| {
            ui.label("技能等级");
            for level in 0..=5 {
                let _response = ui.selectable_value(
                    &mut state.partner_level,
                    level,
                    if level == 0 {
                        "全部等级".into()
                    } else {
                        format!("Lv. {level}")
                    },
                );
                #[cfg(test)]
                track(ui, &_response, &format!("partner-level/{level}"));
            }
        });
    }
    let mut effect = text(v, "effect").to_owned();
    if state.partner_level > 0 {
        if let Some(curves) = v["curves"].as_object() {
            for (token, values) in curves {
                if let Some(value) = values.get(state.partner_level - 1) {
                    effect = effect.replace(token, &number(value));
                }
            }
        }
    }
    section(ui, "技能效果", |ui| {
        ui.label(&effect);
        ui.horizontal_wrapped(|ui| {
            for mention in array(&v["mentions"]) {
                if let (Some(label), Some(href)) = (mention[0].as_str(), mention[1].as_str()) {
                    route(ui, href, label, nav);
                }
            }
        });
    });
    if !array(&v["stats"]).is_empty() {
        section(ui, "各等级数值", |ui| {
            egui::Grid::new("partner-values")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("效果");
                    if state.partner_level == 0 {
                        for i in 1..=5 {
                            ui.strong(format!("Lv. {i}"));
                        }
                    } else {
                        ui.strong(format!("Lv. {}", state.partner_level));
                    }
                    ui.end_row();
                    for r in array(&v["stats"]) {
                        ui.label(r[0].as_str().unwrap_or(""));
                        if state.partner_level == 0 {
                            for value in array(&r[1]) {
                                ui.label(number(value));
                            }
                        } else {
                            ui.label(number(&r[1][state.partner_level - 1]));
                        }
                        ui.end_row();
                    }
                });
        });
    }
    if !array(&v["gear"]).is_empty() || !v["tech"].is_null() {
        section(ui, "解锁装备", |ui| {
            if !v["tech"].is_null() {
                ui.label(format!("科技 Lv. {}", number(&v["tech"])));
            }
            for g in array(&v["gear"]) {
                item(ui, d, g[0].as_str().unwrap_or(""), "", nav);
            }
        });
    }
    ui.add_space(10.);
    ui.horizontal_wrapped(|ui| {
        action(ui, format!("#paldex/{}", e.id), "帕鲁图鉴", nav);
        if array(&v["tags"]).iter().any(|t| t == "work") {
            action(ui, format!("#work/suitability/{}", e.id), "工作适应性", nav);
        }
        if v["ranch"] == true {
            action(ui, format!("#work/ranch/{}", e.id), "牧场产物", nav);
        }
    });
}

pub(super) fn active(ui: &mut egui::Ui, d: &Data, e: &Entry, page: &str, nav: &mut Option<String>) {
    let v = &e.value;
    let fruit_id = format!("SkillCard_{}", e.id);
    hero(
        ui,
        d,
        e,
        "",
        if page == "details/fruits" {
            text(v, "zh")
        } else {
            text(v, "en")
        },
    );
    ui.horizontal_wrapped(|ui| {
        element(ui, text(v, "type"));
        ui.weak(if v["inherit"] == true {
            "可遗传"
        } else {
            "不可随机遗传"
        });
    });
    section(ui, "技能数值", |ui| {
        stats(
            ui,
            &[
                ("威力", number(&v["power"])),
                ("冷却", format!("{} 秒", number(&v["cooldown"]))),
            ],
        );
    });
    if !array(&v["effects"]).is_empty() {
        section(ui, "异常状态积蓄", |ui| {
            for r in array(&v["effects"]) {
                ui.horizontal_wrapped(|ui| {
                    let name = r[0].as_str().unwrap_or("");
                    let status = match name {
                        "浸湿" => "wet",
                        "点燃" => "burn",
                        "缠绕" => "ivy",
                        "冻结" => "freeze",
                        "感电" => "shock",
                        "泥泞" => "mud",
                        "眩晕" => "stun",
                        "中毒" => "poison",
                        "黑暗" => "blind",
                        _ => "",
                    };
                    if status.is_empty() {
                        ui.label(name);
                    } else {
                        route(ui, format!("#guide/status/{status}"), name, nav);
                    }
                    ui.label(number(&r[1]));
                });
            }
        });
    }
    if !text(v, "fruit").is_empty() {
        section(ui, "对应技能果实", |ui| {
            item(ui, d, &fruit_id, "查看物品获取方式", nav);
        });
    }
    section(ui, "升级习得的帕鲁", |ui| {
        let mut count = 0;
        egui::Grid::new("move-learners")
            .striped(true)
            .spacing([18., 7.])
            .show(ui, |ui| {
                ui.allocate_ui(egui::vec2(200., 20.), |ui| {
                    ui.strong("帕鲁");
                });
                ui.strong("习得等级");
                ui.end_row();
                for p in &d.species {
                    let key = text(p, "key");
                    for level in array(&d.get("pal-details-data")["profiles"][key]["levels"]) {
                        if level[1] == e.id {
                            pal(ui, d, key, "", nav);
                            ui.label(format!("Lv. {}", number(&level[0])));
                            ui.end_row();
                            count += 1;
                        }
                    }
                }
            });
        if count == 0 {
            ui.label("此技能没有普通帕鲁的升级习得记录。");
        }
    });
}

pub(super) fn passive(
    ui: &mut egui::Ui,
    d: &Data,
    e: &Entry,
    page: &str,
    surgery: &mut Value,
    nav: &mut Option<String>,
) {
    let v = &e.value;
    hero(ui, d, e, "", text(v, "en"));
    ui.horizontal_wrapped(|ui| {
        rank(ui, d, &v["rank"]);
    });
    section(ui, "词条效果", |ui| {
        for line in text(v, "effect").lines() {
            ui.label(line);
        }
        ui.weak(if v["random"] == true {
            "可随机出现"
        } else {
            "不在普通随机词条池"
        });
    });
    if v["surgery"].is_object() {
        let operation = &v["surgery"];
        section(ui, "手术台添加", |ui| {
            if !text(operation, "item").is_empty() {
                item(ui, d, text(operation, "item"), text(operation, "name"), nav);
            }
            let mode = text(operation, "mode");
            ui.label(match mode {
                "consume" => "每次消耗 1 个植入体".into(),
                "unlock" => format!(
                    "{}解锁 · 每次 {} 金币",
                    text(operation, "source"),
                    number(&operation["cost"])
                ),
                _ => format!("每次 {} 金币", number(&operation["cost"])),
            });
            if page == "details/surgery" {
                if !surgery.is_array() {
                    *surgery = json!([]);
                }
                let mut owned = mode == "coins" || array(surgery).iter().any(|x| x == &e.id);
                let _response = ui.add_enabled(
                    mode != "coins",
                    egui::Checkbox::new(
                        &mut owned,
                        match mode {
                            "coins" => "手术台自带 · 始终可用",
                            "consume" => "持有耗材",
                            _ => "已获取植入体",
                        },
                    ),
                );
                #[cfg(test)]
                track(ui, &_response, "surgery-owned");
                if _response.changed() {
                    let rows = surgery.as_array_mut().unwrap();
                    rows.retain(|x| x != &e.id);
                    if owned {
                        rows.push(json!(e.id));
                    }
                }
                ui.weak("标记后，可在目标配种中勾选使用已拥有的手术台词条。");
                action(ui, "#planner", "前往目标配种", nav);
            } else {
                action(
                    ui,
                    format!("#details/surgery/trait/{}", e.id),
                    "查看手术项目并标记已有",
                    nav,
                );
            }
        });
    }
    if page == "details/surgery" {
        section(ui, "手术台与更改性别", |ui| {
            route(
                ui,
                "#work/facilities/Pal_Surgery_Table",
                "建造帕鲁手术台 · 科技 Lv. 38",
                nav,
            );
            item(ui, d, "IronIngot", "× 30", nav);
            item(ui, d, "Cloth2", "× 20", nav);
            item(ui, d, "Corrosive_Solvent", "× 10", nav);
            item(
                ui,
                d,
                "PalGenderReverse",
                "在手术台使用，可更改帕鲁性别",
                nav,
            );
        });
    }
    section(ui, "固定携带", |ui| {
        let fixed: Vec<_> = d
            .species
            .iter()
            .filter(|p| {
                array(&d.get("pal-details-data")["profiles"][text(p, "key")]["fixed"])
                    .iter()
                    .any(|x| x == &e.id)
            })
            .collect();
        if fixed.is_empty() {
            ui.label("没有已记录的固定携带帕鲁。");
        } else {
            ui.weak("个体也可能通过随机生成、遗传或手术获得其他词条。");
            for p in fixed {
                pal(ui, d, text(p, "key"), "", nav);
            }
        }
    });
}
