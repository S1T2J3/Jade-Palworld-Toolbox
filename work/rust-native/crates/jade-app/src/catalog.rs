use crate::data::{Data, array, text};
use eframe::egui::{self, RichText, Vec2};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub route: String,
    pub group: String,
    pub icon: String,
    pub value: Value,
    pub needle: String,
}
#[derive(Default)]
pub struct Catalog {
    pub pages: HashMap<String, Vec<Entry>>,
    pub query: String,
    pub group: String,
    pub last: String,
    pub quantity: u64,
    pub choices: Value,
    pub views: HashMap<String, (String, String, String)>,
    pub filter: String,
    craft_key: String,
    craft_plan: Value,
    ranch_level: usize,
    sort: String,
}
fn entry(id: &str, name: &str, route: &str, value: Value) -> Entry {
    Entry {
        id: id.into(),
        name: name.into(),
        route: route.into(),
        group: ["group", "category", "region", "type"]
            .iter()
            .find_map(|k| value.get(k).and_then(Value::as_str))
            .unwrap_or("")
            .into(),
        icon: text(&value, "icon").into(),
        needle: crate::data::normalize(&format!("{name} {id} {value}")),
        value,
    }
}
impl Catalog {
    pub fn rebuild(&mut self, d: &Data) {
        self.pages.clear();
        for (route, table, field) in [
            ("items", "item-catalog-data", "items"),
            ("enemies", "enemy-data", "enemies"),
            ("merchants", "enemy-data", "people"),
            ("neutrals", "enemy-data", "people"),
            ("work/facilities", "facilities-data", "facilities"),
            ("work/expeditions", "expedition-data", "stages"),
            ("guide/journals", "journals-data", "catalog"),
            ("guide/status", "combat-guide-data", "status"),
            ("guide/gathering", "resource-guides-data", "gathering"),
            ("work/production", "resource-guides-data", "production"),
            ("guide/quests", "adventure-data", "quests"),
            ("guide/loot", "adventure-data", "chapters"),
        ] {
            let mut rows = vec![];
            for (i, v) in array(&d.get(table)[field]).iter().enumerate() {
                if route == "merchants" && !is_merchant(v) {
                    continue;
                }
                if route == "neutrals" && is_merchant(v) {
                    continue;
                }
                let fallback = i.to_string();
                let id = ["id", "code", "key"]
                    .iter()
                    .find_map(|k| v[*k].as_str())
                    .unwrap_or(&fallback);
                let name = ["name", "title", "zh", "label"]
                    .iter()
                    .find_map(|k| v[*k].as_str())
                    .unwrap_or(id);
                if route == "guide/loot" {
                    for pool in array(&v["pools"]) {
                        let pid = text(pool, "id");
                        let mut value = pool.clone();
                        value["group"] = json!(name);
                        let composite = format!("{id}/{pid}");
                        rows.push(entry(
                            &composite,
                            &format!("{} · {}", name, text(pool, "name")),
                            &format!("#{route}/{composite}"),
                            value,
                        ));
                    }
                } else {
                    let mut row = entry(id, name, &format!("#{route}/{id}"), v.clone());
                    if let Some(icon) = d.get("enemy-data")["icons"][&row.icon].as_str() {
                        row.icon = icon.into();
                    }
                    rows.push(row);
                }
            }
            self.pages.insert(route.into(), rows);
        }
        let mut pals = vec![];
        let mut partner = vec![];
        let mut ranch = vec![];
        for p in &d.species {
            let key = text(p, "key");
            let mut v = p.clone();
            v["基础属性"] = d.get("paldex-index-data")["profiles"][key].clone();
            v["详细资料"] = d.get("paldex-data")[key].clone();
            v["技能与固定词条"] = d.get("pal-details-data")["profiles"][key].clone();
            pals.push(entry(key, text(p, "zh"), &format!("#paldex/{key}"), v));
            if !d.get("partner-skills-data")[key].is_null() {
                let mut v = d.get("partner-skills-data")[key].clone();
                v["speciesKey"] = json!(key);
                partner.push(entry(
                    key,
                    &format!(
                        "#{} {} · {}",
                        text(p, "dex"),
                        text(p, "zh"),
                        text(&v, "name")
                    ),
                    &format!("#details/partner/{key}"),
                    v,
                ));
            }
            if !d.get("ranch-data")["pals"][key].is_null() {
                let mut v = d.get("ranch-data")["pals"][key].clone();
                v["speciesKey"] = json!(key);
                ranch.push(entry(key, text(p, "zh"), &format!("#work/ranch/{key}"), v));
            }
        }
        self.pages.insert("paldex".into(), pals);
        self.pages.insert("details/partner".into(), partner);
        self.pages.insert("work/ranch".into(), ranch);
        for (page, field) in [
            ("details/active", "moves"),
            ("details/passive", "passives"),
            ("details/fruits", "moves"),
            ("details/surgery", "passives"),
        ] {
            let mut rows = vec![];
            if let Some(map) = d.get("pal-details-data")[field].as_object() {
                for (id, v) in map {
                    if page == "details/fruits" && v["fruit"].is_null() {
                        continue;
                    }
                    if page == "details/surgery"
                        && (v["surgery"].is_null() || v["surgery"] == false)
                    {
                        continue;
                    }
                    let name = if page == "details/fruits" {
                        text(v, "fruit")
                    } else {
                        text(v, "zh")
                    };
                    let kind = if field == "moves" { "move" } else { "trait" };
                    let mut row = entry(id, name, &format!("#{page}/{kind}/{id}"), v.clone());
                    if field == "passives" {
                        row.group = format!("{} 级", v["rank"]);
                    }
                    rows.push(row);
                }
            }
            if field == "passives" {
                rows.sort_by_key(|e| std::cmp::Reverse(e.value["rank"].as_i64().unwrap_or(0)));
            }
            self.pages.insert(page.into(), rows);
        }
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        page: &str,
        route: &str,
        surgery: &mut Value,
    ) -> Option<String> {
        if page == "work/ranch" {
            return self.ranch(ui, d, route);
        }
        let mut navigate = None;
        if self.last != page {
            self.views.insert(
                self.last.clone(),
                (self.query.clone(), self.group.clone(), self.filter.clone()),
            );
            let v = self.views.get(page).cloned().unwrap_or_default();
            self.query = v.0;
            self.group = v.1;
            self.filter = v.2;
            self.last = page.into();
        }
        let entries = match self.pages.get(page) {
            Some(v) => v,
            None => {
                ui.spinner();
                ui.label("正在准备资料…");
                return None;
            }
        };
        let wanted = route
            .trim_start_matches('#')
            .strip_prefix(page)
            .unwrap_or("")
            .trim_start_matches('/');
        let wanted = wanted
            .strip_prefix("move/")
            .or_else(|| wanted.strip_prefix("trait/"))
            .unwrap_or(wanted);
        let item_shops = page == "items" && wanted.ends_with("/shops");
        let wanted = wanted.strip_suffix("/shops").unwrap_or(wanted);
        let loot_item = if page == "guide/loot" {
            wanted.split_once("/item/")
        } else {
            None
        };
        let selected = entries.iter().find(|e| {
            e.id == wanted
                || loot_item.is_some_and(|(chapter, item)| {
                    e.id.starts_with(&format!("{chapter}/"))
                        && array(&e.value["rows"])
                            .iter()
                            .any(|r| r[0].as_str() == Some(item))
                })
                || page == "guide/loot"
                    && !wanted.is_empty()
                    && e.id.starts_with(&format!("{wanted}/"))
        });
        let groups: BTreeSet<_> = if page == "details/partner" {
            entries
                .iter()
                .flat_map(|e| {
                    array(&e.value["tags"])
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        } else if page == "paldex" {
            entries
                .iter()
                .flat_map(|e| {
                    array(&e.value["基础属性"]["work"])
                        .iter()
                        .filter_map(|r| r[0].as_str().map(str::to_owned))
                })
                .collect()
        } else {
            entries
                .iter()
                .map(|e| e.group.clone())
                .filter(|g| !g.is_empty())
                .collect()
        };
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("搜索名称、编号或效果")
                    .desired_width(280.),
            );
            egui::ComboBox::from_id_salt("catalog-group")
                .selected_text(if self.group.is_empty() {
                    "全部分类"
                } else {
                    group_label(&self.group)
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.group, String::new(), "全部分类");
                    for g in groups {
                        ui.selectable_value(&mut self.group, g.clone(), group_label(&g));
                    }
                });
            if page == "paldex" {
                crate::breeding_ui::element_picker(ui, &mut self.filter);
                egui::ComboBox::from_id_salt("dex-sort")
                    .selected_text(if self.sort.is_empty() {
                        "编号排序"
                    } else {
                        &self.sort
                    })
                    .show_ui(ui, |ui| {
                        for s in [
                            "编号排序",
                            "生命从高到低",
                            "攻击从高到低",
                            "防御从高到低",
                            "工作等级从高到低",
                        ] {
                            ui.selectable_value(&mut self.sort, s.into(), s);
                        }
                    });
            }
            if page == "items" {
                egui::ComboBox::from_id_salt("item-method")
                    .selected_text(if self.filter.is_empty() {
                        "全部获取方式"
                    } else {
                        &self.filter
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filter, String::new(), "全部获取方式");
                        for k in [
                            "配方制作",
                            "商人出售",
                            "牧场生产",
                            "遗物转换",
                            "帕鲁远征",
                            "敌人掉落",
                            "野外采集",
                            "任务奖励",
                        ] {
                            ui.selectable_value(&mut self.filter, k.into(), k);
                        }
                    });
            }
            if page == "work/expeditions" {
                egui::ComboBox::from_id_salt("expedition-difficulty")
                    .selected_text(if self.filter.is_empty() {
                        "全部难度"
                    } else {
                        &self.filter
                    })
                    .show_ui(ui, |ui| {
                        for s in ["", "普通", "高难"] {
                            ui.selectable_value(
                                &mut self.filter,
                                s.into(),
                                if s.is_empty() { "全部难度" } else { s },
                            );
                        }
                    });
            }
        });
        let query = crate::data::normalize(&self.query);
        let mut matches: Vec<_> = entries
            .iter()
            .filter(|e| {
                (self.group.is_empty()
                    || e.group == self.group
                    || page == "paldex"
                        && array(&e.value["基础属性"]["work"])
                            .iter()
                            .any(|r| r[0].as_str() == Some(&self.group))
                    || array(&e.value["tags"])
                        .iter()
                        .any(|v| v.as_str() == Some(&self.group)))
                    && query.split_whitespace().all(|q| e.needle.contains(q))
                    && (self.filter.is_empty()
                        || if page == "paldex" {
                            array(&e.value["types"])
                                .iter()
                                .any(|v| v.as_str() == Some(&self.filter))
                        } else if page == "items" {
                            crate::links::has_method(d, &e.value, &self.filter)
                        } else if page == "work/expeditions" {
                            (e.value["hard"] == true) == (self.filter == "高难")
                        } else {
                            true
                        })
                    && loot_item.is_none_or(|(chapter, item)| {
                        e.id.starts_with(&format!("{chapter}/"))
                            && array(&e.value["rows"])
                                .iter()
                                .any(|r| r[0].as_str() == Some(item))
                    })
                    && (page != "work/ranch"
                        || !wanted.starts_with("item/")
                        || e.value["tiers"]
                            .to_string()
                            .contains(wanted.trim_start_matches("item/")))
            })
            .collect();
        if page == "paldex" {
            let stat = match self.sort.as_str() {
                "生命从高到低" => "hp",
                "攻击从高到低" => "attack",
                "防御从高到低" => "defense",
                _ => "",
            };
            if !stat.is_empty() {
                matches.sort_by(|a, b| {
                    b.value["基础属性"][stat]
                        .as_f64()
                        .unwrap_or(0.)
                        .total_cmp(&a.value["基础属性"][stat].as_f64().unwrap_or(0.))
                });
            } else if self.sort == "工作等级从高到低" {
                let level = |e: &Entry| {
                    array(&e.value["基础属性"]["work"])
                        .iter()
                        .filter(|r| self.group.is_empty() || r[0].as_str() == Some(&self.group))
                        .filter_map(|r| r[1].as_u64())
                        .max()
                        .unwrap_or(0)
                };
                matches.sort_by_key(|e| std::cmp::Reverse(level(e)));
            }
        }
        ui.label(format!("{} 条", matches.len()));
        let list_width = if selected.is_some() {
            ui.available_width() * 0.44
        } else {
            ui.available_width()
        };
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(list_width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(("catalog-rows", page, &self.query, &self.group, &self.filter))
                        .show_rows(ui, 56., matches.len(), |ui, range| {
                            for i in range {
                                let e = matches[i];
                                ui.horizontal(|ui| {
                                    if ["paldex", "details/partner", "work/ranch"].contains(&page) {
                                        d.portrait(ui, &e.id, 44.);
                                    } else if page == "items" {
                                        d.item_icon(ui, &e.id, 36.);
                                    } else if e.icon.starts_with("assets/") {
                                        d.image(ui, &e.icon, Vec2::splat(36.));
                                    }
                                    if let Some(rank) = e.value["rank"].as_i64() {
                                        if let Some(icon) = d.get("trait-ranks")
                                            [rank.unsigned_abs() as usize]
                                            .as_str()
                                        {
                                            d.image(ui, icon, Vec2::splat(16.));
                                        }
                                    }
                                    ui.vertical(|ui| {
                                        if ui
                                            .selectable_label(
                                                selected.map(|s| &s.id) == Some(&e.id),
                                                RichText::new(&e.name).strong(),
                                            )
                                            .clicked()
                                        {
                                            navigate = Some(e.route.clone());
                                        }
                                        if !e.group.is_empty() {
                                            ui.small(&e.group);
                                        } else if let Some(rank) = e.value["rank"].as_i64() {
                                            ui.small(format!("词条等级 {rank}"));
                                        }
                                    });
                                });
                            }
                        });
                },
            );
            if let Some(e) = selected {
                ui.separator();
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt(("catalog-detail", &e.route))
                            .show(ui, |ui| {
                                ui.heading(&e.name);
                                if ["paldex", "details/partner", "work/ranch"].contains(&page) {
                                    d.portrait(ui, &e.id, 84.);
                                    ui.horizontal_wrapped(|ui| {
                                        for (label, path) in [
                                            ("配种", "lookup"),
                                            ("刷新位置", "spawns"),
                                            ("属性计算", "details/stats"),
                                            ("捕捉概率", "details/capture"),
                                            ("伙伴技能", "details/partner"),
                                        ] {
                                            if ui.link(label).clicked() {
                                                navigate = Some(format!("#{path}/{}", e.id));
                                            }
                                        }
                                    });
                                }
                                if page == "details/surgery" {
                                    if !surgery.is_array() {
                                        *surgery = json!([]);
                                    }
                                    let coins = e.value["surgery"]["mode"] == "coins";
                                    let mut owned = coins
                                        || array(surgery).iter().any(|v| v.as_str() == Some(&e.id));
                                    if ui
                                        .add_enabled(
                                            !coins,
                                            egui::Checkbox::new(
                                                &mut owned,
                                                if coins {
                                                    "金币手术：始终可用"
                                                } else {
                                                    "已获得此手术台词条"
                                                },
                                            ),
                                        )
                                        .changed()
                                    {
                                        let a = surgery.as_array_mut().unwrap();
                                        a.retain(|v| v.as_str() != Some(&e.id));
                                        if owned {
                                            a.push(json!(e.id));
                                        }
                                    }
                                }
                                if page == "items" {
                                    d.item_icon(ui, &e.id, 64.);
                                    if !array(&e.value["recipes"]).is_empty() {
                                        self.quantity = self.quantity.max(1);
                                        ui.horizontal(|ui| {
                                            ui.label("制作数量");
                                            ui.add(
                                                egui::DragValue::new(&mut self.quantity)
                                                    .range(1..=99999),
                                            );
                                        });
                                        let key =
                                            format!("{}|{}|{}", e.id, self.quantity, self.choices);
                                        if key != self.craft_key {
                                            self.craft_plan = jade_core::plan_item_craft(
                                                &d.get("item-catalog-data")["items"],
                                                &e.id,
                                                self.quantity,
                                                &self.choices,
                                            )
                                            .unwrap_or(Value::Null);
                                            self.craft_key = key;
                                        }
                                        crate::links::craft(
                                            ui,
                                            d,
                                            &self.craft_plan,
                                            &mut self.choices,
                                            &mut navigate,
                                        );
                                    }
                                }
                                if item_shops {
                                    crate::links::sellers(ui, d, &e.value, &mut navigate);
                                } else {
                                    if let Some((_, item)) = loot_item {
                                        let mut shown = e.value.clone();
                                        if let Some(rows) = shown["rows"].as_array_mut() {
                                            rows.retain(|r| r[0].as_str() == Some(item));
                                        }
                                        ui.label(format!(
                                            "当前仅显示 {} 的奖励栏位",
                                            d.item_name(item)
                                        ));
                                        if ui.link("查看完整奖池").clicked() {
                                            navigate = Some(e.route.clone());
                                        }
                                        render_value(ui, d, &shown, 0, &mut navigate);
                                    } else {
                                        render_value(ui, d, &e.value, 0, &mut navigate);
                                    }
                                    related(ui, d, page, e, &mut navigate);
                                    crate::links::related(
                                        ui,
                                        d,
                                        page,
                                        &e.id,
                                        &e.value,
                                        &mut navigate,
                                    );
                                }
                            });
                    },
                );
            }
        });
        navigate
    }
    fn ranch(&mut self, ui: &mut egui::Ui, d: &Data, route: &str) -> Option<String> {
        let mut nav = None;
        let path = route.strip_prefix("#work/ranch/").unwrap_or("");
        let reverse = path.starts_with("item/");
        let id = path.trim_start_matches("item/");
        let pals = d.get("ranch-data")["pals"].as_object()?;
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(!reverse, "按帕鲁查看").clicked() {
                nav = Some("#work/ranch".into());
            }
            if ui.selectable_label(reverse, "按产物反查").clicked() {
                nav = Some("#work/ranch/item/".into());
            }
            ui.label("牧场适应性等级（0 = 天然等级）");
            ui.add(egui::DragValue::new(&mut self.ranch_level).range(0..=10));
            ui.text_edit_singleline(&mut self.query);
        });
        let products: BTreeSet<String> = pals
            .values()
            .flat_map(|p| {
                array(&p["tiers"])
                    .iter()
                    .flat_map(array)
                    .filter_map(|r| r["item"].as_str().map(str::to_owned))
            })
            .collect();
        let candidates: Vec<String> = if reverse {
            products.into_iter().collect()
        } else {
            d.species
                .iter()
                .map(|p| text(p, "key"))
                .filter(|k| pals.contains_key(*k))
                .map(str::to_owned)
                .collect()
        };
        let selected = if candidates.iter().any(|s| s == id) {
            id.to_owned()
        } else {
            candidates.first().cloned().unwrap_or_default()
        };
        ui.columns(2, |cols| {
            egui::ScrollArea::vertical()
                .id_salt(("ranch-list", reverse))
                .show(&mut cols[0], |ui| {
                    for key in &candidates {
                        let name = if reverse {
                            d.item_name(key)
                        } else {
                            d.pal_name(key)
                        };
                        if !crate::data::normalize(&name)
                            .contains(&crate::data::normalize(&self.query))
                            && !crate::data::normalize(key)
                                .contains(&crate::data::normalize(&self.query))
                        {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if reverse {
                                d.item_icon(ui, key, 36.);
                            } else {
                                d.portrait(ui, key, 36.);
                            }
                            if ui.selectable_label(*key == selected, name).clicked() {
                                nav = Some(format!(
                                    "#work/ranch/{}{key}",
                                    if reverse { "item/" } else { "" }
                                ));
                            }
                        });
                    }
                });
            egui::ScrollArea::vertical()
                .id_salt(("ranch-detail", &selected, reverse))
                .show(&mut cols[1], |ui| {
                    ui.heading(if reverse {
                        d.item_name(&selected)
                    } else {
                        d.pal_name(&selected)
                    });
                    ui.label(
                        "每次产出抽取一种物品，数量为抽中后的个数。等级包含浓缩和适应性手册提升。",
                    );
                    let suppliers: Vec<_> = if reverse {
                        pals.iter()
                            .filter(|(_, p)| {
                                array(&p["tiers"])
                                    .iter()
                                    .flat_map(array)
                                    .any(|r| r["item"] == selected)
                            })
                            .collect()
                    } else {
                        pals.iter().filter(|(k, _)| **k == selected).collect()
                    };
                    for (key, p) in suppliers {
                        let base = p["base"].as_u64().unwrap_or(1) as usize;
                        let level = if self.ranch_level == 0 {
                            base
                        } else {
                            self.ranch_level
                        };
                        ui.group(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                d.portrait(ui, key, 44.);
                                if ui.link(d.pal_name(key)).clicked() {
                                    nav = Some(format!("#paldex/{key}"));
                                }
                                ui.label(format!(
                                    "天然 Lv.{base} · 满星 Lv.{} · 当前 Lv.{level}",
                                    p["condensed"]
                                ));
                            });
                            if level < base {
                                ui.label(format!("该帕鲁天然适应性为 Lv.{base}，不能低于此级。"));
                            } else {
                                for r in array(&p["tiers"][level - 1]) {
                                    let item = text(r, "item");
                                    if reverse && item != selected {
                                        continue;
                                    }
                                    ui.horizontal_wrapped(|ui| {
                                        d.item_icon(ui, item, 32.);
                                        if ui.link(d.item_name(item)).clicked() {
                                            nav = Some(format!("#items/{item}"));
                                        }
                                        ui.label(format!(
                                            "{}～{} 件 · {}%",
                                            r["min"], r["max"], r["chance"]
                                        ));
                                        if ui.small_button("反查产量").clicked() {
                                            nav = Some(format!("#work/ranch/item/{item}"));
                                        }
                                    });
                                }
                            }
                            if ui.link("伙伴技能").clicked() {
                                nav = Some(format!("#details/partner/{key}"));
                            }
                            if ui.link("查看全部产物").clicked() {
                                nav = Some(format!("#work/ranch/{key}"));
                            }
                        });
                    }
                });
        });
        nav
    }
}
fn is_merchant(v: &Value) -> bool {
    v["category"] == "merchants"
}
pub fn elements(ui: &mut egui::Ui, d: &Data) -> Option<String> {
    let mut nav = None;
    let id = egui::Id::new("element-guide-selection");
    let mut selected = ui
        .ctx()
        .data_mut(|m| m.get_temp::<String>(id).unwrap_or("fire".into()));
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(chart) = d.get("combat-guide-data")["chart"].as_str() {
            d.image(ui, chart, Vec2::new(ui.available_width().min(900.), 550.));
        }
        crate::breeding_ui::element_picker(ui, &mut selected);
        if !selected.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.strong("克制：");
                render_value(
                    ui,
                    d,
                    &d.get("combat-guide-data")["strong"][&selected],
                    0,
                    &mut nav,
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.strong("被克制：");
                if let Some(strong) = d.get("combat-guide-data")["strong"].as_object() {
                    for (key, targets) in strong {
                        if array(targets).iter().any(|v| v.as_str() == Some(&selected)) {
                            render_value(ui, d, &json!(key), 0, &mut nav);
                        }
                    }
                }
            });
            ui.separator();
            ui.strong("具有该属性的帕鲁");
            ui.horizontal_wrapped(|ui| {
                for p in &d.species {
                    if array(&p["types"])
                        .iter()
                        .any(|v| v.as_str() == Some(&selected))
                        && ui.link(text(p, "zh")).clicked()
                    {
                        nav = Some(format!("#paldex/{}", text(p, "key")));
                    }
                }
            });
        }
    });
    ui.ctx().data_mut(|m| m.insert_temp(id, selected));
    nav
}
fn group_label(s: &str) -> &str {
    match s {
        "combat" => "战斗",
        "work" => "工作",
        "travel" => "移动",
        "ride" => "骑乘",
        "support" => "辅助",
        "ranch" => "牧场",
        _ => s,
    }
}
fn label(key: &str) -> &str {
    match key {
        "dex" => "图鉴编号",
        "types" => "属性",
        "speciesKey" => "对应帕鲁",
        "firstRewards" => "首次击败奖励",
        "summon" | "summons" => "召唤材料",
        "rows" => "奖励栏位",
        "minLevel" | "levelMin" => "最低等级",
        "maxLevel" | "levelMax" => "最高等级",
        "map" => "地图区域",
        "x" => "坐标 X",
        "y" => "坐标 Y",
        "requests" => "委托请求",
        "previous" => "前置任务",
        "buy" => "收购物品",
        "points" => "地图点位",
        "slotted" => "按奖励栏位抽取",
        "weapon" => "武器",
        "variant" => "变种或货单",
        "enemiesByPal" => "相关敌人",
        "affected" => "适用对象",
        "trigger" => "触发方式",
        "until" => "结束条件",
        "id" | "key" | "code" => "游戏编号",
        "name" | "zh" | "title" => "名称",
        "en" => "英文名称",
        "category" | "group" => "类别",
        "effect" | "text" | "description" | "purpose" | "summary" => "说明",
        "body" => "正文",
        "power" => "威力 / 战力",
        "cooldown" => "冷却时间",
        "type" | "element" | "elements" => "属性",
        "inherit" => "可遗传",
        "rank" | "rarity" => "等级",
        "fruit" => "技能果实",
        "drops" => "掉落物",
        "rewards" => "奖励",
        "recipes" => "合成配方",
        "materials" | "inputs" => "材料",
        "qty" | "quantity" => "数量",
        "stations" => "制作设施",
        "tech" => "科技等级",
        "plan" => "设计图",
        "level" => "等级",
        "work" => "工作适应性",
        "fixed" => "固定词条",
        "levels" => "习得等级",
        "surgery" => "手术台",
        "cost" => "费用",
        "item" => "物品",
        "minutes" => "分钟",
        "unlock" => "解锁条件",
        "point" | "pointId" => "地图位置",
        "site" => "坐标",
        "required" => "工作要求",
        "jobs" => "工作",
        "effects" => "效果",
        "nocturnal" => "夜间工作",
        "hp" => "生命",
        "attack" => "攻击",
        "defense" => "防御",
        "food" => "饱食度",
        "partner" => "伙伴技能",
        "partnerEffect" => "伙伴效果",
        "sellers" => "出售商人",
        "enemies" => "关联敌人",
        "source" | "sources" => "资料来源",
        "note" => "说明",
        "tiers" => "各等级产物",
        "chance" => "概率（%）",
        "min" => "最少",
        "max" => "最多",
        "base" => "基础等级",
        "condensed" => "满星等级",
        "objectives" => "任务目标",
        "next" => "后续任务",
        "repeatable" => "可重复",
        "shops" => "商店",
        "hard" => "困难模式",
        "region" => "地区",
        "tower" => "解锁高塔",
        "tags" => "类型",
        "gear" => "装备",
        "stats" => "数值",
        "curves" => "等级变化",
        "mentions" => "关联词条",
        "buildWork" => "建造工作量",
        "energy" => "能源",
        "production" => "生产",
        "tools" => "采集工具",
        "external" => "需准备材料",
        "steps" => "制作步骤",
        "cycles" => "循环依赖",
        "batches" => "批次",
        "made" => "产出数量",
        "requiredQuantity" => "需求数量",
        "route" | "href" => "关联入口",
        _ => key,
    }
}
fn resolve(d: &Data, value: &str) -> Option<(String, String)> {
    if value.starts_with('#') {
        return Some((value.into(), "查看关联详情".into()));
    }
    if d.pal(value).is_some() {
        return Some((format!("#paldex/{value}"), d.pal_name(value)));
    }
    if let Some(row) = d.resolve(value) {
        return Some(row.clone());
    }
    if value.starts_with("palpagos-") || value.starts_with("worldtree-") {
        return Some((format!("#map/point/{value}"), "在地图上定位".into()));
    }
    None
}
pub fn render_value(
    ui: &mut egui::Ui,
    d: &Data,
    v: &Value,
    depth: usize,
    nav: &mut Option<String>,
) {
    if depth > 9 {
        ui.label("更多层级请通过关联条目查看");
        return;
    }
    match v {
        Value::Null => {}
        Value::Bool(b) => {
            ui.label(if *b { "是" } else { "否" });
        }
        Value::Number(n) => {
            ui.label(n.to_string());
        }
        Value::String(s) => {
            let element = match s.as_str() {
                "neutral" => Some("无属性"),
                "fire" => Some("火属性"),
                "water" => Some("水属性"),
                "grass" => Some("草属性"),
                "electric" => Some("雷属性"),
                "ice" => Some("冰属性"),
                "ground" => Some("地属性"),
                "dark" => Some("暗属性"),
                "dragon" => Some("龙属性"),
                "palpagos" => Some("帕洛斯群岛"),
                "worldtree" => Some("世界树"),
                _ => None,
            };
            if let Some(name) = element {
                ui.label(name);
            } else if s.starts_with("assets/") {
                d.image(ui, s, Vec2::splat(64.));
            } else if s.starts_with("https://") {
                ui.hyperlink_to("原始资料", s);
            } else if let Some((route, name)) = resolve(d, s) {
                if ui.link(name).clicked() {
                    *nav = Some(route);
                }
            } else {
                ui.add(egui::Label::new(s).wrap());
            }
        }
        Value::Array(rows) => {
            if rows.is_empty() {
                ui.weak("无");
                return;
            }
            if rows.len() == 2
                && rows[0].as_str().is_some_and(|s| s.starts_with('#'))
                && rows[1].is_string()
            {
                if ui.link(rows[1].as_str().unwrap()).clicked() {
                    *nav = rows[0].as_str().map(str::to_owned);
                }
                return;
            }
            for (i, row) in rows.iter().enumerate() {
                ui.push_id(i, |ui| {
                    if let Some(tuple) = row.as_array() {
                        ui.horizontal_wrapped(|ui| {
                            for x in tuple {
                                render_value(ui, d, x, depth + 1, nav);
                            }
                        });
                    } else {
                        render_value(ui, d, row, depth + 1, nav);
                    }
                });
            }
        }
        Value::Object(map) => {
            for (key, value) in map {
                if value.is_null() || key == "icon" || key == "slug" {
                    continue;
                }
                ui.push_id(key, |ui| {
                    if ["rows", "drops", "rewards", "firstRewards"].contains(&key.as_str())
                        && array(value)
                            .first()
                            .is_some_and(|r| r.is_array() && r[0].is_string())
                    {
                        egui::CollapsingHeader::new(label(key))
                            .default_open(true)
                            .show(ui, |ui| {
                                for (i, r) in array(value).iter().enumerate() {
                                    ui.push_id(i, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            let id = r[0].as_str().unwrap_or("");
                                            d.item_icon(ui, id, 28.);
                                            if ui.link(d.item_name(id)).clicked() {
                                                *nav = Some(format!("#items/{id}"));
                                            }
                                            if array(r).len() >= 4 {
                                                ui.label(format!(
                                                    "{}～{} 件 · 获得率 {}%",
                                                    r[1], r[2], r[3]
                                                ));
                                                if let Some(slot) = r.get(4) {
                                                    ui.small(format!("栏位 {slot}"));
                                                }
                                                if let Some(step) =
                                                    r.get(5).filter(|v| v.as_u64().unwrap_or(1) > 1)
                                                {
                                                    ui.small(format!("数量间隔 {step}"));
                                                }
                                            } else if array(r).len() >= 2 {
                                                ui.label(format!("× {}", r[1]));
                                            }
                                        });
                                    });
                                }
                                if key == "rows" || key == "rewards" {
                                    ui.small(
                                        "概率按奖励表的单个栏位显示，同一物品可能出现在多个栏位。",
                                    );
                                }
                            });
                    } else if value.is_object() || value.is_array() {
                        egui::CollapsingHeader::new(label(key))
                            .default_open(depth < 1)
                            .show(ui, |ui| render_value(ui, d, value, depth + 1, nav));
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(format!("{}：", label(key)));
                            if ["id","key","code","name","zh","en","title","category","group","region","summary","body","text","description","purpose","note"].contains(&key.as_str())&&value.is_string(){ui.add(egui::Label::new(value.as_str().unwrap()).wrap());}else{render_value(ui, d, value, depth + 1, nav);}
                        });
                    }
                });
            }
        }
    }
}
fn related(ui: &mut egui::Ui, d: &Data, page: &str, e: &Entry, nav: &mut Option<String>) {
    if page == "items" {
        let book = d.get("item-catalog-data");
        for (field, title) in [
            ("acquisitionLinks", "获取方式"),
            ("enemyDrops", "敌人掉落"),
            ("unitDrops", "单位掉落"),
            ("unitRewards", "单位奖励"),
            ("guideLinks", "采集与探索"),
            ("related", "关联资料"),
        ] {
            if !book[field][&e.id].is_null() {
                ui.separator();
                ui.strong(title);
                render_value(ui, d, &book[field][&e.id], 0, nav);
            }
        }
        for field in ["shops", "points", "ruins"] {
            if !e.value[field].is_null() {
                ui.strong(label(field));
                render_value(ui, d, &e.value[field], 0, nav);
            }
        }
        let uses: Vec<_> = array(&book["items"])
            .iter()
            .filter(|p| {
                array(&p["recipes"]).iter().any(|r| {
                    array(&r["materials"])
                        .iter()
                        .any(|m| m[0].as_str() == Some(&e.id))
                })
            })
            .collect();
        if !uses.is_empty() {
            ui.strong("可作为材料制作");
            ui.horizontal_wrapped(|ui| {
                for row in uses {
                    if ui.link(text(row, "name")).clicked() {
                        *nav = Some(format!("#items/{}", text(row, "id")));
                    }
                }
            });
        }
    }
    if page == "details/active" || page == "details/passive" {
        ui.separator();
        ui.strong("关联帕鲁");
        for p in &d.species {
            let key = text(p, "key");
            let profile = &d.get("pal-details-data")["profiles"][key];
            let levels = array(&profile["levels"]);
            let fixed = array(&profile["fixed"]);
            if levels.iter().any(|x| x[1].as_str() == Some(&e.id))
                || fixed.iter().any(|x| x.as_str() == Some(&e.id))
            {
                ui.horizontal(|ui| {
                    if ui.link(text(p, "zh")).clicked() {
                        *nav = Some(format!("#paldex/{key}"));
                    }
                    if let Some(level) = levels.iter().find(|x| x[1].as_str() == Some(&e.id)) {
                        ui.small(format!("Lv. {}", level[0]));
                    }
                });
            }
        }
    }
}
