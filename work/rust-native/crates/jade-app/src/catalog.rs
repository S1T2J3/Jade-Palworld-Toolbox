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
    views: HashMap<String, Filters>,
    pub filter: String,
    ranch_level: usize,
    sort: String,
    scope: String,
    pal_filter: String,
    pal_query: String,
    tags: BTreeSet<String>,
    last_route: String,
    match_key: String,
    matched: Vec<usize>,
    pub detail: crate::catalog_views::DetailState,
}
#[derive(Clone, Default)]
struct Filters {
    query: String,
    group: String,
    filter: String,
    scope: String,
    pal: String,
    tags: BTreeSet<String>,
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
        self.match_key.clear();
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
        for rows in self.pages.values_mut() {
            for e in rows {
                let mut names = BTreeSet::new();
                search_names(d, &e.value, &mut names);
                for shop in array(&e.value["shops"]).iter().filter_map(Value::as_str) {
                    search_names(d, &d.get("enemy-data")["shopLists"][shop], &mut names);
                }
                for name in names {
                    e.needle.push(' ');
                    e.needle.push_str(&crate::data::normalize(&name));
                }
            }
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
        if self.last != page {
            self.views.insert(
                self.last.clone(),
                Filters {
                    query: self.query.clone(),
                    group: self.group.clone(),
                    filter: self.filter.clone(),
                    scope: self.scope.clone(),
                    pal: self.pal_filter.clone(),
                    tags: self.tags.clone(),
                    sort: self.sort.clone(),
                },
            );
            let view = self.views.get(page).cloned().unwrap_or_default();
            self.query = view.query;
            self.group = view.group;
            self.filter = view.filter;
            self.scope = view.scope;
            self.pal_filter = view.pal;
            self.tags = view.tags;
            self.sort = view.sort;
            self.last = page.into();
        }
        if page == "work/ranch" {
            return self.ranch(ui, d, route);
        }
        let Some(entries) = self.pages.get(page) else {
            ui.spinner();
            ui.label("正在准备资料…");
            return None;
        };
        let mut navigate = None;
        let raw = route
            .trim_start_matches('#')
            .strip_prefix(page)
            .unwrap_or("")
            .trim_start_matches('/');
        let wanted = raw
            .strip_prefix("move/")
            .or_else(|| raw.strip_prefix("trait/"))
            .unwrap_or(raw);
        let wanted = wanted.strip_suffix("/shops").unwrap_or(wanted);
        let (wanted, context_pal) = if ["details/active", "details/passive"].contains(&page) {
            if let Some((pal, id)) = wanted.split_once('/') {
                if d.pal(pal).is_some() {
                    (id, Some(pal))
                } else {
                    (wanted, None)
                }
            } else if d.pal(wanted).is_some() {
                ("", Some(wanted))
            } else {
                (wanted, None)
            }
        } else {
            (wanted, None)
        };
        if route != self.last_route {
            if let Some(pal) = context_pal {
                self.pal_filter = pal.into();
            }
            self.last_route = route.into();
        }
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
        if page == "paldex" && selected.is_some() {
            let e = selected.unwrap();
            ui.horizontal(|ui| {
                if ui.button("← 返回图鉴列表").clicked() {
                    navigate = Some("#paldex".into());
                }
                if let Some(index) = entries.iter().position(|r| r.id == e.id) {
                    if ui
                        .add_enabled(index > 0, egui::Button::new("上一只"))
                        .clicked()
                    {
                        navigate = Some(entries[index - 1].route.clone());
                    }
                    if ui
                        .add_enabled(index + 1 < entries.len(), egui::Button::new("下一只"))
                        .clicked()
                    {
                        navigate = Some(entries[index + 1].route.clone());
                    }
                }
            });
            egui::ScrollArea::vertical()
                .id_salt(("dex-detail", &e.id))
                .show(ui, |ui| {
                    if let Some(n) =
                        crate::catalog_views::show(ui, d, page, e, route, surgery, &mut self.detail)
                    {
                        navigate = Some(n);
                    }
                });
            return navigate;
        }
        let groups: BTreeSet<String> = if page == "paldex" {
            entries
                .iter()
                .flat_map(|e| {
                    array(&e.value["基础属性"]["work"])
                        .iter()
                        .filter_map(|v| v[0].as_str().map(str::to_owned))
                })
                .collect()
        } else {
            entries
                .iter()
                .map(|e| e.group.clone())
                .filter(|v| !v.is_empty())
                .collect()
        };
        crate::design::card(ui).inner_margin(10).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("搜索名称、编号或效果")
                        .desired_width(250.),
                );
                if !groups.is_empty()
                    && !["details/partner", "details/active", "details/fruits"].contains(&page)
                {
                    choice(
                        ui,
                        "catalog-group",
                        &mut self.group,
                        if page == "paldex" {
                            "全部工作"
                        } else {
                            "全部分类"
                        },
                        groups.iter().map(|s| (s.as_str(), group_label(s))),
                    );
                }
                if page == "paldex"
                    || page == "details/partner"
                    || page == "details/active"
                    || page == "details/fruits"
                {
                    crate::breeding_ui::element_picker(ui, &mut self.filter);
                }
                if page == "paldex" {
                    choice(
                        ui,
                        "dex-sort",
                        &mut self.sort,
                        "编号排序",
                        [
                            "生命从高到低",
                            "攻击从高到低",
                            "防御从高到低",
                            "工作等级从高到低",
                        ]
                        .into_iter()
                        .map(|s| (s, s)),
                    );
                }
                if ["details/active", "details/passive"].contains(&page) {
                    egui::ComboBox::from_id_salt("catalog-pal")
                        .selected_text(if self.pal_filter.is_empty() {
                            "按帕鲁查看".to_owned()
                        } else {
                            d.pal_name(&self.pal_filter).to_owned()
                        })
                        .width(170.)
                        .show_ui(ui, |ui| {
                            ui.text_edit_singleline(&mut self.pal_query);
                            ui.selectable_value(&mut self.pal_filter, String::new(), "全部帕鲁");
                            let q = crate::data::normalize(&self.pal_query);
                            for p in &d.species {
                                if crate::data::search(p, &q) {
                                    ui.selectable_value(
                                        &mut self.pal_filter,
                                        text(p, "key").into(),
                                        format!("#{} {}", text(p, "dex"), text(p, "zh")),
                                    );
                                }
                            }
                        });
                }
                if page == "details/passive" {
                    choice(
                        ui,
                        "passive-scope",
                        &mut self.scope,
                        "全部词条",
                        [
                            ("random", "可随机出现"),
                            ("fixed", "有固定携带种"),
                            ("surgery", "手术可用"),
                            ("negative", "负阶级"),
                        ],
                    );
                }
                if page == "details/surgery" {
                    choice(
                        ui,
                        "surgery-scope",
                        &mut self.scope,
                        "全部手术",
                        [
                            ("owned", "当前可用"),
                            ("needed", "尚未持有"),
                            ("coins", "直接付费"),
                            ("unlock", "解锁植入体"),
                            ("consume", "耗材植入体"),
                        ],
                    );
                }
                if page == "items" {
                    choice(
                        ui,
                        "item-method",
                        &mut self.filter,
                        "全部获取方式",
                        [
                            "配方制作",
                            "商人出售",
                            "牧场生产",
                            "遗物转换",
                            "帕鲁远征",
                            "敌人掉落",
                            "野外采集",
                            "任务奖励",
                        ]
                        .into_iter()
                        .map(|s| (s, s)),
                    );
                }
                if page == "work/facilities" {
                    let jobs: BTreeSet<String> = entries
                        .iter()
                        .flat_map(|e| {
                            array(&e.value["required"])
                                .iter()
                                .chain(array(&e.value["jobs"]).iter())
                                .filter_map(|r| {
                                    r.as_str().or_else(|| r[0].as_str()).map(str::to_owned)
                                })
                        })
                        .collect();
                    choice(
                        ui,
                        "facility-job",
                        &mut self.filter,
                        "全部工作",
                        jobs.iter().map(|s| (s.as_str(), s.as_str())),
                    );
                }
                if ["enemies", "neutrals", "merchants"].contains(&page) {
                    let mut options = vec![("loot", "有掉落物"), ("map", "可地图定位")];
                    if page == "merchants" {
                        options.extend([("items", "出售物品"), ("pals", "出售帕鲁")]);
                    }
                    choice(ui, "unit-scope", &mut self.scope, "全部单位", options);
                }
                if page == "work/expeditions" {
                    choice(
                        ui,
                        "expedition-mode",
                        &mut self.filter,
                        "全部难度",
                        [("普通", "普通"), ("高难", "高难")],
                    );
                    let elements: BTreeSet<_> = entries
                        .iter()
                        .filter_map(|e| e.value["element"][0].as_str())
                        .collect();
                    choice(
                        ui,
                        "expedition-element",
                        &mut self.scope,
                        "全部加成属性",
                        elements.iter().map(|s| (*s, *s)),
                    );
                }
                if page == "guide/journals" {
                    choice(
                        ui,
                        "journal-map",
                        &mut self.scope,
                        "全部地图",
                        [("palpagos", "帕洛斯群岛"), ("worldtree", "世界树")],
                    );
                }
                if ui.button("清除筛选").clicked() {
                    self.query.clear();
                    self.group.clear();
                    self.filter.clear();
                    self.scope.clear();
                    self.pal_filter.clear();
                    self.pal_query.clear();
                    self.tags.clear();
                    self.sort.clear();
                }
            });
            if page == "details/partner" {
                let tags: BTreeSet<_> = entries
                    .iter()
                    .flat_map(|e| array(&e.value["tags"]).iter().filter_map(Value::as_str))
                    .collect();
                ui.horizontal_wrapped(|ui| {
                    ui.small("技能类型 · 多选匹配任一");
                    for tag in tags {
                        let mut enabled = self.tags.contains(tag);
                        if ui.checkbox(&mut enabled, group_label(tag)).changed() {
                            if enabled {
                                self.tags.insert(tag.into());
                            } else {
                                self.tags.remove(tag);
                            }
                        }
                    }
                });
            }
            if !self.pal_filter.is_empty() {
                ui.horizontal(|ui| {
                    d.portrait(ui, &self.pal_filter, 28.);
                    ui.label(format!(
                        "当前查看：{}{}",
                        d.pal_name(&self.pal_filter),
                        if page == "details/active" {
                            "的升级技能"
                        } else {
                            "的固定词条"
                        }
                    ));
                });
            }
        });
        let filters = Filters {
            query: crate::data::normalize(&self.query),
            group: self.group.clone(),
            filter: self.filter.clone(),
            scope: self.scope.clone(),
            pal: self.pal_filter.clone(),
            tags: self.tags.clone(),
            sort: self.sort.clone(),
        };
        let match_key = format!(
            "{page}|{}|{}|{}|{}|{}|{:?}|{loot_item:?}|{surgery}",
            filters.query, filters.group, filters.filter, filters.scope, filters.pal, filters.tags
        );
        if self.match_key != match_key {
            self.matched = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| matches_filters(d, page, e, &filters, surgery))
                .filter(|(_, e)| {
                    loot_item.is_none_or(|(chapter, item)| {
                        e.id.starts_with(&format!("{chapter}/"))
                            && array(&e.value["rows"])
                                .iter()
                                .any(|r| r[0].as_str() == Some(item))
                    })
                })
                .map(|(i, _)| i)
                .collect();
            self.match_key = match_key;
        }
        let mut matches: Vec<_> = self
            .matched
            .iter()
            .filter_map(|i| entries.get(*i))
            .collect();
        if page == "paldex" {
            let key = match self.sort.as_str() {
                "生命从高到低" => "hp",
                "攻击从高到低" => "attack",
                "防御从高到低" => "defense",
                _ => "",
            };
            if !key.is_empty() {
                matches.sort_by(|a, b| {
                    b.value["基础属性"][key]
                        .as_f64()
                        .unwrap_or(0.)
                        .total_cmp(&a.value["基础属性"][key].as_f64().unwrap_or(0.))
                });
            }
            if self.sort == "工作等级从高到低" {
                matches.sort_by_key(|e| {
                    std::cmp::Reverse(
                        array(&e.value["基础属性"]["work"])
                            .iter()
                            .filter(|r| self.group.is_empty() || r[0].as_str() == Some(&self.group))
                            .filter_map(|r| r[1].as_u64())
                            .max()
                            .unwrap_or(0),
                    )
                });
            }
        }
        let full_grid = [
            "paldex",
            "details/active",
            "details/passive",
            "details/surgery",
            "details/fruits",
            "guide/status",
        ]
        .contains(&page);
        let selected = selected.or_else(|| {
            if full_grid {
                None
            } else {
                matches.first().copied()
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} 条", matches.len())).strong());
            if page == "details/partner" {
                ui.weak("按图鉴编号排列");
            }
            if selected.is_some() && full_grid && ui.small_button("展开全部列表").clicked() {
                navigate = Some(format!("#{page}"));
            }
        });
        if matches.is_empty() {
            ui.label("没有符合条件的条目，清除筛选可查看全部资料。");
        }
        let total = ui.available_width();
        let list_width = if selected.is_some() {
            (total * 0.51).max(260.).min(total - 300.)
        } else {
            total
        };
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(list_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(list_width);
                    let min_width = if page == "items" {
                        168.
                    } else if page == "paldex" {
                        175.
                    } else {
                        218.
                    };
                    let cols = ((list_width + 8.) / (min_width + 8.)).floor().max(1.) as usize;
                    let rows = matches.len().div_ceil(cols);
                    let width = (list_width - 8. * (cols - 1) as f32) / cols as f32;
                    egui::ScrollArea::vertical()
                        .id_salt((
                            "catalog-rows",
                            page,
                            &filters.query,
                            &filters.group,
                            &filters.filter,
                            &filters.scope,
                            &filters.pal,
                        ))
                        .auto_shrink([false, false])
                        .show_rows(ui, 66., rows, |ui, range| {
                            for row in range {
                                ui.horizontal_top(|ui| {
                                    for e in matches.iter().skip(row * cols).take(cols) {
                                        let response = entry_card(
                                            ui,
                                            d,
                                            page,
                                            e,
                                            width,
                                            selected.is_some_and(|s| s.id == e.id),
                                        );
                                        if response.clicked() {
                                            navigate = Some(e.route.clone());
                                        }
                                    }
                                });
                            }
                        });
                },
            );
            if let Some(e) = selected {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical()
                            .id_salt(("catalog-detail", page, &e.id))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if let Some(n) = crate::catalog_views::show(
                                    ui,
                                    d,
                                    page,
                                    e,
                                    route,
                                    surgery,
                                    &mut self.detail,
                                ) {
                                    navigate = Some(n);
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
                        crate::design::card(ui).show(ui, |ui| {
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
fn search_names(d: &Data, value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::String(s) if s.len() < 160 => {
            if let Some((_, name)) = d.resolve(s) {
                names.insert(name.clone());
            }
        }
        Value::Array(rows) => {
            for row in rows {
                search_names(d, row, names);
            }
        }
        Value::Object(fields) => {
            for (key, v) in fields {
                if !["icon", "image", "body", "description", "effect", "source"]
                    .contains(&key.as_str())
                {
                    search_names(d, v, names);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod restored_filters_tests {
    use super::*;
    #[test]
    fn original_filters_and_related_name_search() {
        let mut d = Data::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
            egui::Context::default(),
        )
        .unwrap();
        let start = std::time::Instant::now();
        while d.pending > 0 && start.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            d.poll();
        }
        assert_eq!(d.pending, 0);
        let mut c = Catalog::default();
        c.rebuild(&d);
        let none = json!([]);
        let active = &c.pages["details/active"];
        let filter = Filters {
            pal: "SheepBall".into(),
            ..Default::default()
        };
        assert_eq!(
            active
                .iter()
                .filter(|e| matches_filters(&d, "details/active", e, &filter, &none))
                .count(),
            array(&d.get("pal-details-data")["profiles"]["SheepBall"]["levels"]).len()
        );
        let surgeries = &c.pages["details/surgery"];
        let implant = surgeries
            .iter()
            .find(|e| e.value["surgery"]["mode"] == "consume")
            .unwrap();
        let owned = Filters {
            scope: "owned".into(),
            ..Default::default()
        };
        assert!(!matches_filters(
            &d,
            "details/surgery",
            implant,
            &owned,
            &none
        ));
        assert!(matches_filters(
            &d,
            "details/surgery",
            implant,
            &owned,
            &json!([implant.id])
        ));
        let coins = surgeries
            .iter()
            .find(|e| e.value["surgery"]["mode"] == "coins")
            .unwrap();
        assert!(matches_filters(&d, "details/surgery", coins, &owned, &none));
        let query = Filters {
            query: "帕鲁球".into(),
            ..Default::default()
        };
        assert!(c.pages["enemies"].iter().any(|e|e.id=="Hunter_Rifle"&&matches_filters(&d,"enemies",e,&query,&none)));
        assert!(
            c.pages["merchants"].iter().any(
                |e| e.id == "SalesPerson" && matches_filters(&d, "merchants", e, &query, &none)
            )
        );
        let pal_shops = Filters {
            scope: "pals".into(),
            ..Default::default()
        };
        let shops: Vec<_> = c.pages["merchants"]
            .iter()
            .filter(|e| matches_filters(&d, "merchants", e, &pal_shops, &none))
            .collect();
        assert!(!shops.is_empty());
        assert!(shops.iter().all(|e| e.value["palStock"].is_object()));
    }
}
fn choice<'a>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    all: &str,
    options: impl IntoIterator<Item = (&'a str, &'a str)>,
) {
    let options: Vec<_> = options.into_iter().collect();
    let label = options
        .iter()
        .find(|(key, _)| *key == value)
        .map(|(_, label)| *label)
        .unwrap_or(all);
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(145.)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), all);
            for (key, label) in options {
                ui.selectable_value(value, key.into(), label);
            }
        });
}
fn matches_filters(d: &Data, page: &str, e: &Entry, f: &Filters, surgery: &Value) -> bool {
    if !f.query.split_whitespace().all(|q| e.needle.contains(q)) {
        return false;
    }
    if !f.group.is_empty()
        && e.group != f.group
        && !(page == "paldex"
            && array(&e.value["基础属性"]["work"])
                .iter()
                .any(|r| r[0].as_str() == Some(&f.group)))
    {
        return false;
    }
    if !f.tags.is_empty()
        && !array(&e.value["tags"])
            .iter()
            .filter_map(Value::as_str)
            .any(|s| f.tags.contains(s))
    {
        return false;
    }
    if !f.pal.is_empty() && ["details/active", "details/passive"].contains(&page) {
        let profile = &d.get("pal-details-data")["profiles"][&f.pal];
        let present = if page == "details/active" {
            array(&profile["levels"])
                .iter()
                .any(|r| r[1].as_str() == Some(&e.id))
        } else {
            array(&profile["fixed"])
                .iter()
                .any(|v| v.as_str() == Some(&e.id))
        };
        if !present {
            return false;
        }
    }
    if !f.filter.is_empty() {
        let matched = match page {
            "paldex" => array(&e.value["types"])
                .iter()
                .any(|v| v.as_str() == Some(&f.filter)),
            "details/partner" => d.pal(&e.id).is_some_and(|p| {
                array(&p["types"])
                    .iter()
                    .any(|v| v.as_str() == Some(&f.filter))
            }),
            "details/active" | "details/fruits" => text(&e.value, "type") == f.filter,
            "items" => crate::links::has_method(d, &e.value, &f.filter),
            "work/expeditions" => (e.value["hard"] == true) == (f.filter == "高难"),
            "work/facilities" => array(&e.value["required"])
                .iter()
                .chain(array(&e.value["jobs"]).iter())
                .any(|r| r.as_str().or_else(|| r[0].as_str()) == Some(&f.filter)),
            _ => true,
        };
        if !matched {
            return false;
        }
    }
    if !f.scope.is_empty() {
        let owned = e.value["surgery"]["mode"] == "coins"
            || array(surgery).iter().any(|v| v.as_str() == Some(&e.id));
        let matched = match page {
            "details/passive" => match f.scope.as_str() {
                "random" => e.value["random"] == true,
                "negative" => e.value["rank"].as_i64().unwrap_or(0) < 0,
                "surgery" => e.value["surgery"].is_object(),
                "fixed" => d.get("pal-details-data")["profiles"]
                    .as_object()
                    .is_some_and(|p| {
                        p.values()
                            .any(|p| array(&p["fixed"]).iter().any(|v| v.as_str() == Some(&e.id)))
                    }),
                _ => true,
            },
            "details/surgery" => match f.scope.as_str() {
                "owned" => owned,
                "needed" => !owned,
                mode => text(&e.value["surgery"], "mode") == mode,
            },
            "guide/journals" => text(&e.value, "map") == f.scope,
            "work/expeditions" => e.value["element"][0].as_str() == Some(&f.scope),
            "enemies" | "neutrals" | "merchants" => match f.scope.as_str() {
                "loot" => {
                    !array(&e.value["drops"]).is_empty()
                        || array(&e.value["dropTiers"])
                            .iter()
                            .any(|tier| !array(&tier["drops"]).is_empty())
                }
                "map" => !array(&e.value["points"]).is_empty(),
                "items" => !array(&e.value["shops"]).is_empty(),
                "pals" => e.value["palStock"].is_object(),
                _ => true,
            },
            _ => true,
        };
        if !matched {
            return false;
        }
    }
    true
}
fn entry_card(
    ui: &mut egui::Ui,
    d: &Data,
    page: &str,
    e: &Entry,
    width: f32,
    selected: bool,
) -> egui::Response {
    let p = crate::design::palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 66.), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 7, if selected { p.selected } else { p.surface });
    ui.painter().rect_stroke(
        rect,
        7,
        egui::Stroke::new(
            1.,
            if selected || response.hovered() {
                p.accent
            } else {
                p.line.gamma_multiply(0.6)
            },
        ),
        egui::StrokeKind::Inside,
    );
    let mut cell = ui.new_child(
        egui::UiBuilder::new()
            .id_salt((&e.route, "card"))
            .max_rect(rect.shrink(6.))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    cell.set_clip_rect(rect.intersect(ui.clip_rect()));
    if ["paldex", "details/partner"].contains(&page) {
        d.portrait(&mut cell, &e.id, 42.);
    } else if page == "items" {
        d.item_icon(&mut cell, &e.id, 32.);
    } else if !e.icon.is_empty() {
        d.image(&mut cell, &e.icon, Vec2::splat(32.));
    } else if let Some(rank) = e.value["rank"].as_i64() {
        if let Some(icon) = d.get("trait-ranks")[rank.unsigned_abs() as usize].as_str() {
            d.image(&mut cell, icon, Vec2::splat(22.));
        }
    }
    cell.vertical(|ui| {
        let title = if page == "details/partner" {
            d.pal(&e.id)
                .map(|p| format!("#{} {}", text(p, "dex"), text(p, "zh")))
                .unwrap_or_else(|| e.name.clone())
        } else {
            e.name.clone()
        };
        ui.add(egui::Label::new(RichText::new(title).size(14.).strong()).truncate());
        let summary = if page == "details/partner" {
            text(&e.value, "name").to_owned()
        } else if page == "paldex" {
            format!(
                "#{}  {}",
                text(&e.value, "dex"),
                array(&e.value["types"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(group_label)
                    .collect::<Vec<_>>()
                    .join(" · ")
            )
        } else {
            crate::catalog_views::summary(page, &e.value)
        };
        ui.add(egui::Label::new(RichText::new(summary).size(11.).color(p.muted)).truncate());
    });
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!(
            "{}\n{}",
            e.name,
            crate::catalog_views::summary(page, &e.value)
        ))
}
fn group_label(s: &str) -> &str {
    match s {
        "combat" => "战斗",
        "work" => "工作",
        "travel" => "骑乘与移动",
        "explore" => "探索与负重",
        "loot" => "掉落与垂钓",
        "catch" => "捕捉",
        "care" => "支援与恢复",
        "neutral" => "无",
        "fire" => "火",
        "water" => "水",
        "grass" => "草",
        "electric" => "雷",
        "ice" => "冰",
        "ground" => "地",
        "dark" => "暗",
        "dragon" => "龙",
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
                            if [
                                "id",
                                "key",
                                "code",
                                "name",
                                "zh",
                                "en",
                                "title",
                                "category",
                                "group",
                                "region",
                                "summary",
                                "body",
                                "text",
                                "description",
                                "purpose",
                                "note",
                            ]
                            .contains(&key.as_str())
                                && value.is_string()
                            {
                                ui.add(egui::Label::new(value.as_str().unwrap()).wrap());
                            } else {
                                render_value(ui, d, value, depth + 1, nav);
                            }
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
