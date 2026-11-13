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
    fuzzy: crate::search::CachedText,
    methods: BTreeSet<String>,
}
#[derive(Default)]
pub struct Catalog {
    pub pages: HashMap<String, Vec<Entry>>,
    route_entries: HashMap<String, (String, usize)>,
    pub query: String,
    pub group: String,
    pub last: String,
    views: HashMap<String, Filters>,
    pub filter: String,
    ranch: crate::work_views::RanchState,
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
        fuzzy: crate::search::CachedText::from_row(&json!({
            "name":name,"href":route,"category":text(&value,"category"),
            "hint":text(&value,"effect"),
            "aliases":[id,text(&value,"en"),text(&value,"zh"),text(&value,"title")]
        })),
        value,
        methods: BTreeSet::new(),
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
                    if route == "items" {
                        row.methods = item_method_tags(d, &row.value);
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
        self.route_entries = self
            .pages
            .iter()
            .flat_map(|(page, entries)| {
                entries
                    .iter()
                    .enumerate()
                    .map(move |(index, e)| (e.route.clone(), (page.clone(), index)))
            })
            .collect();
    }
    pub fn image_for_route(&self, d: &Data, route: &str) -> Option<egui::ImageSource<'static>> {
        let (page, index) = self.route_entries.get(route)?;
        let entry = self.pages.get(page)?.get(*index)?;
        crate::catalog_views::entry_image(d, page, entry)
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
                || page == "guide/journals" && e.value["pointId"].as_str() == Some(wanted)
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
        if matches!(page, "paldex" | "guide/journals") && selected.is_some() {
            let e = selected.unwrap();
            ui.horizontal(|ui| {
                if crate::design::small_action(
                    ui,
                    "left",
                    if page == "paldex" {
                        "返回图鉴列表"
                    } else {
                        "返回手记目录"
                    },
                )
                .clicked()
                {
                    navigate = Some(format!("#{page}"));
                }
                if let Some(index) = entries.iter().position(|r| r.id == e.id) {
                    if ui
                        .add_enabled_ui(index > 0, |ui| {
                            crate::design::small_action(
                                ui,
                                "left",
                                if page == "paldex" {
                                    "上一只"
                                } else {
                                    "上一篇"
                                },
                            )
                        })
                        .inner
                        .clicked()
                    {
                        navigate = Some(entries[index - 1].route.clone());
                    }
                    if ui
                        .add_enabled_ui(index + 1 < entries.len(), |ui| {
                            crate::design::small_action(
                                ui,
                                "right",
                                if page == "paldex" {
                                    "下一只"
                                } else {
                                    "下一篇"
                                },
                            )
                        })
                        .inner
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
        let mut groups: BTreeSet<String> = if page == "paldex" {
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
        if page == "items" && entries.iter().any(|e| e.value["other"] == true) {
            groups.insert("其他游戏数据".into());
        }
        crate::design::card(ui).inner_margin(10).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("模糊搜索名称、编号或效果")
                        .desired_width(if ui.ctx().content_rect().width() < 1120. {
                            200.
                        } else {
                            250.
                        }),
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
                    let methods: BTreeSet<_> =
                        entries.iter().flat_map(|e| e.methods.iter()).collect();
                    choice(
                        ui,
                        "item-method",
                        &mut self.filter,
                        "全部获取方式",
                        methods.iter().map(|s| (s.as_str(), s.as_str())),
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
                if crate::design::small_action(ui, "reset", "清除筛选").clicked() {
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
                        if crate::design::checkbox(ui, &mut enabled, group_label(tag)).changed() {
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
            let prepared = crate::search::PreparedQuery::new(&filters.query);
            self.matched = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| matches_filters_prepared(d, page, e, &filters, surgery, &prepared))
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
            if !filters.query.trim().is_empty()
                && !matches!(page, "details/passive" | "details/surgery")
            {
                self.matched.sort_by_cached_key(|i| {
                    entries[*i]
                        .fuzzy
                        .match_query(&prepared)
                        .map(|m| m.score)
                        .unwrap_or(110)
                });
            }
            self.match_key = match_key;
        }
        let mut matches: Vec<_> = self
            .matched
            .iter()
            .filter_map(|i| entries.get(*i))
            .collect();
        if matches!(page, "details/passive" | "details/surgery") && !filters.query.trim().is_empty()
        {
            matches.sort_by_key(|e| {
                crate::search::trait_score(d, &e.id, &filters.query).unwrap_or(u32::MAX)
            });
        }
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
            "guide/journals",
        ]
        .contains(&page);
        let selected = selected.or_else(|| {
            if full_grid {
                None
            } else {
                matches.first().copied()
            }
        });
        if page == "details/surgery" && selected.is_none() {
            if let Some(route) = crate::catalog_views::surgery_overview(ui, d) {
                navigate = Some(route);
            }
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} 条", matches.len())).strong());
            if page == "details/partner" {
                ui.weak("按图鉴编号排列");
            }
            if selected.is_some()
                && full_grid
                && crate::design::small_action(ui, "list", "展开全部列表").clicked()
            {
                navigate = Some(format!("#{page}"));
            }
        });
        if matches.is_empty() {
            ui.label("没有符合条件的条目，清除筛选可查看全部资料。");
        }
        if page == "guide/journals" && selected.is_none() {
            let columns = ((ui.available_width() + 8.) / 225.).floor().max(1.) as usize;
            let width = (ui.available_width() - (columns - 1) as f32 * 8.) / columns as f32;
            let mut series = Vec::<&str>::new();
            for e in &matches {
                if !series.contains(&e.group.as_str()) {
                    series.push(&e.group);
                }
            }
            egui::ScrollArea::vertical()
                .id_salt((
                    "journal-index",
                    &filters.query,
                    &filters.group,
                    &filters.scope,
                ))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for group in series {
                        ui.heading(group);
                        let rows: Vec<_> = matches.iter().filter(|e| e.group == group).collect();
                        for row in rows.chunks(columns) {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.;
                                for e in row {
                                    if entry_card(ui, d, page, e, width, false).0.clicked() {
                                        navigate = Some(e.route.clone());
                                    }
                                }
                            });
                        }
                        ui.add_space(12.);
                    }
                });
            return navigate;
        }
        if selected.is_none()
            && matches!(
                page,
                "details/active" | "details/passive" | "details/fruits" | "details/surgery"
            )
        {
            let columns = ((ui.available_width() + 8.) / 230.).floor().max(1.) as usize;
            let width = (ui.available_width() - (columns - 1) as f32 * 8.) / columns as f32;
            // These pages have at most 316 short cards. Variable-height rows preserve
            // the original full effects and ownership controls; the large item/unit
            // catalogues below retain their virtual rows.
            egui::ScrollArea::vertical()
                .id_salt((
                    "skill-overview",
                    page,
                    &filters.query,
                    &filters.scope,
                    &filters.group,
                ))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in matches.chunks(columns) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.;
                            for e in row {
                                if let Some(route) =
                                    skill_overview_card(ui, d, page, e, width, surgery)
                                {
                                    navigate = Some(route);
                                }
                            }
                        });
                    }
                });
            return navigate;
        }
        // A species opens a full overview with its function choices at the top.
        // The catalogue index keeps its dense grid and remembers its filters.
        if page == "paldex" {
            if let Some(e) = selected {
                egui::ScrollArea::vertical()
                    .id_salt(("pal-overview", &e.id))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(route) = crate::catalog_views::show(
                            ui,
                            d,
                            page,
                            e,
                            route,
                            surgery,
                            &mut self.detail,
                        ) {
                            navigate = Some(route);
                        }
                    });
                return navigate;
            }
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
                                        let (response, identity) = entry_card(
                                            ui,
                                            d,
                                            page,
                                            e,
                                            width,
                                            selected.is_some_and(|s| s.id == e.id),
                                        );
                                        if let Some(route) = identity {
                                            navigate = Some(route);
                                        } else if response.clicked() {
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
        self.ranch.show(ui, d, route)
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
            let width = ui.available_width().min(900.);
            d.image(ui, chart, Vec2::new(width, width * 675. / 1200.));
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
#[path = "catalog_views/overview_tests.rs"]
mod overview_tests;
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
/// Cache the same complete acquisition categories that the original item view
/// offered. Building this once also avoids scanning ranches and expeditions on
/// every character typed into the item search.
fn item_method_tags(d: &Data, v: &Value) -> BTreeSet<String> {
    let id = text(v, "id");
    let book = d.get("item-catalog-data");
    let mut methods = BTreeSet::new();
    if let Some(groups) = book["guideLinks"][id].as_object() {
        methods.extend(groups.keys().cloned());
    }
    for method in ["配方制作", "商人出售", "牧场生产", "遗物转换", "帕鲁远征"] {
        if crate::links::has_method(d, v, method) {
            methods.insert(method.into());
        }
    }
    if !array(&v["ruins"]).is_empty() {
        methods.insert("远古遗迹".into());
    }
    for (field, title) in [
        ("enemyDrops", "敌人掉落"),
        ("enemyFirstRewards", "敌人首胜奖励"),
        ("unitDrops", "人物掉落"),
        ("unitRewards", "委托与评赏报酬"),
    ] {
        if !array(&book[field][id]).is_empty() {
            methods.insert(title.into());
        }
    }
    for row in array(&v["methods"]) {
        let mut name = row[0].as_str().unwrap_or("");
        let description = row[1].as_str().unwrap_or("");
        if name == "古代遗物回收" && methods.contains("遗物转换") {
            name = "遗物转换";
        } else if name == "人物掉落"
            && array(&book["enemyDrops"][id])
                .iter()
                .any(|r| r[1].as_str() == Some(description))
        {
            name = "敌人掉落";
        } else if name == "头目掉落"
            && description.contains("首次击败")
            && methods.contains("敌人首胜奖励")
        {
            name = "敌人首胜奖励";
        }
        if !name.is_empty() {
            methods.insert(name.into());
        }
    }
    methods
}
fn matches_filters(d: &Data, page: &str, e: &Entry, f: &Filters, surgery: &Value) -> bool {
    matches_filters_prepared(
        d,
        page,
        e,
        f,
        surgery,
        &crate::search::PreparedQuery::new(&f.query),
    )
}
fn matches_filters_prepared(
    d: &Data,
    page: &str,
    e: &Entry,
    f: &Filters,
    surgery: &Value,
    query: &crate::search::PreparedQuery,
) -> bool {
    let query_matches = if matches!(page, "details/passive" | "details/surgery") {
        crate::search::trait_matches(d, &e.id, &f.query)
    } else {
        f.query.split_whitespace().all(|q| e.needle.contains(q))
            || e.fuzzy.match_query(query).is_some()
    };
    if !query_matches {
        return false;
    }
    if !f.group.is_empty()
        && e.group != f.group
        && !(page == "paldex"
            && array(&e.value["基础属性"]["work"])
                .iter()
                .any(|r| r[0].as_str() == Some(&f.group)))
        && !(page == "items" && f.group == "其他游戏数据" && e.value["other"] == true)
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
            "items" => e.methods.contains(&f.filter),
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
fn skill_overview_card(
    ui: &mut egui::Ui,
    d: &Data,
    page: &str,
    e: &Entry,
    width: f32,
    surgery: &mut Value,
) -> Option<String> {
    let mut nav = None;
    ui.push_id((&e.route, "full-card"), |ui| {
        let _card = crate::design::card(ui).inner_margin(10).show(ui, |ui| {
            // The grid places cards in horizontal rows. A Frame inherits that
            // layout, so explicitly stack its header, effect and controls.
            ui.vertical(|ui| {
                ui.set_width((width - 20.).max(90.));
                let passive = matches!(page, "details/passive" | "details/surgery");
                ui.horizontal_top(|ui| {
                    if passive {
                        let emblem = crate::breeding_ui::passive_rank(
                            ui,
                            d,
                            e.value["rank"].as_i64().unwrap_or(0),
                            19.,
                        )
                        .interact(egui::Sense::click());
                        if emblem.clicked() {
                            nav = Some(e.route.clone());
                        }
                    } else if let Some(source) = crate::catalog_views::entry_image(d, page, e) {
                        if ui
                            .add(
                                egui::Image::new(source)
                                    .fit_to_exact_size(Vec2::splat(if page == "details/fruits" {
                                        30.
                                    } else {
                                        22.
                                    }))
                                    .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            nav = Some(if page == "details/fruits" {
                                format!("#items/SkillCard_{}", e.id)
                            } else {
                                e.route.clone()
                            });
                        }
                    }
                    ui.vertical(|ui| {
                        let response = ui
                            .add(
                                egui::Label::new(
                                    RichText::new(&e.name)
                                        .strong()
                                        .color(ui.visuals().hyperlink_color),
                                )
                                .wrap()
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        #[cfg(test)]
                        ui.ctx().data_mut(|m| {
                            m.insert_temp(
                                egui::Id::new(("catalog-rich-title", &e.id)),
                                response.rect,
                            )
                        });
                        if response.clicked() {
                            nav = Some(if page == "details/fruits" {
                                format!("#items/SkillCard_{}", e.id)
                            } else {
                                e.route.clone()
                            });
                        }
                    });
                });
                if passive {
                    ui.small(format!("等级 {}", e.value["rank"].as_i64().unwrap_or(0)));
                    let _effect = ui.add(
                        egui::Label::new(RichText::new(text(&e.value, "effect")).size(12.)).wrap(),
                    );
                    #[cfg(test)]
                    ui.ctx().data_mut(|m| {
                        m.insert_temp(egui::Id::new(("catalog-rich-effect", &e.id)), _effect.rect)
                    });
                    if page == "details/surgery" {
                        let operation = &e.value["surgery"];
                        let mode = text(operation, "mode");
                        ui.separator();
                        ui.small(match mode {
                            "consume" => "每次消耗 1 个植入体".into(),
                            "unlock" => format!(
                                "{}解锁 · 每次 {} 金币",
                                text(operation, "source"),
                                operation["cost"].as_u64().unwrap_or(0)
                            ),
                            _ => format!("每次 {} 金币", operation["cost"].as_u64().unwrap_or(0)),
                        });
                        let item = text(operation, "item");
                        if !item.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                d.item_icon(ui, item, 18.);
                                if ui.link(text(operation, "name")).clicked() {
                                    nav = Some(format!("#items/{item}"));
                                }
                            });
                        }
                        if !surgery.is_array() {
                            *surgery = json!([]);
                        }
                        let mut owned =
                            mode == "coins" || array(surgery).iter().any(|value| value == &e.id);
                        let response = ui
                            .add_enabled_ui(mode != "coins", |ui| {
                                crate::design::checkbox(
                                    ui,
                                    &mut owned,
                                    match mode {
                                        "coins" => "手术台自带",
                                        "consume" => "持有耗材",
                                        _ => "已获取植入体",
                                    },
                                )
                            })
                            .inner;
                        #[cfg(test)]
                        ui.ctx().data_mut(|m| {
                            m.insert_temp(
                                egui::Id::new(("catalog-rich-owned", &e.id)),
                                response.rect,
                            )
                        });
                        if response.changed() {
                            let rows = surgery.as_array_mut().unwrap();
                            rows.retain(|value| value != &e.id);
                            if owned {
                                rows.push(json!(e.id));
                            }
                        }
                    }
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.small(format!("威力 {}", e.value["power"].as_u64().unwrap_or(0)));
                        ui.small(format!(
                            "冷却 {} 秒",
                            e.value["cooldown"].as_f64().unwrap_or(0.)
                        ));
                    });
                    if crate::design::small_action(
                        ui,
                        "right",
                        if page == "details/fruits" {
                            "技能详情"
                        } else {
                            "展开技能详情"
                        },
                    )
                    .clicked()
                    {
                        nav = Some(e.route.clone());
                    }
                }
            });
        });
        #[cfg(test)]
        ui.ctx().data_mut(|m| {
            m.insert_temp(
                egui::Id::new(("catalog-rich-card", &e.id)),
                _card.response.rect,
            )
        });
    });
    nav
}
fn entry_card(
    ui: &mut egui::Ui,
    d: &Data,
    page: &str,
    e: &Entry,
    width: f32,
    selected: bool,
) -> (egui::Response, Option<String>) {
    let mut identity = None;
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
    if matches!(page, "details/passive" | "details/surgery") {
        crate::breeding_ui::passive_rank(&mut cell, d, e.value["rank"].as_i64().unwrap_or(0), 22.);
    } else if let Some(source) = crate::catalog_views::entry_image(d, page, e) {
        let response = cell.add(
            egui::Image::new(source)
                .fit_to_exact_size(Vec2::splat(36.))
                .sense(if page == "details/partner" {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                }),
        );
        if page == "details/partner" {
            #[cfg(test)]
            cell.ctx().data_mut(|m| {
                m.insert_temp(egui::Id::new(("catalog-pal-avatar", &e.id)), response.rect);
            });
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                identity = Some(format!("#paldex/{}", e.id));
            }
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
        let response = ui.add(
            egui::Label::new(RichText::new(title).size(14.).strong())
                .truncate()
                .sense(if page == "details/partner" {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                }),
        );
        if page == "details/partner" {
            #[cfg(test)]
            ui.ctx().data_mut(|m| {
                m.insert_temp(egui::Id::new(("catalog-pal-name", &e.id)), response.rect);
            });
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                identity = Some(format!("#paldex/{}", e.id));
            }
        }
        if page == "paldex" {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.;
                ui.small(format!("#{}", text(&e.value, "dex")));
                for kind in array(&e.value["types"]).iter().filter_map(Value::as_str) {
                    crate::elements::image(ui, kind, 16.)
                        .on_hover_text(format!("{}属性", group_label(kind)));
                }
            });
            return;
        }
        let summary = if page == "details/partner" {
            text(&e.value, "name").to_owned()
        } else {
            crate::catalog_views::summary(page, &e.value)
        };
        ui.add(egui::Label::new(RichText::new(summary).size(11.).color(p.muted)).truncate());
    });
    (
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "{}\n{}",
                e.name,
                crate::catalog_views::summary(page, &e.value)
            )),
        identity,
    )
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
