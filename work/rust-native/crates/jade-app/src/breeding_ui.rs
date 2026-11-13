use crate::design::{action as icon_action, icon_button};
use crate::{
    data::{Data, array, search, text},
    store::Store,
};
use eframe::egui::{self, Vec2};
use serde_json::{Value, json};
use std::sync::mpsc;

pub struct Breeding {
    pub pal: usize,
    pub lookup_pal: usize,
    lookup_query: String,
    lookup_filter: String,
    lookup_element: String,
    lookup_page: usize,
    lookup_size: usize,
    stock_page: usize,
    stock_size: usize,
    offspring_page: usize,
    offspring_size: usize,
    plan_name: String,
    recalculate: bool,
    pub query: String,
    pub filter: String,
    pub reverse: bool,
    pub element: String,
    pub traits: Vec<String>,
    pub picker: bool,
    pub random: bool,
    pub surgery: bool,
    pub result: Value,
    pub busy: bool,
    pub latency: f64,
    pub max_steps: usize,
    pub edit: Option<Value>,
    pub stock_filter: String,
    receiver: Option<mpsc::Receiver<(u64, Value, f64)>>,
    epoch: u64,
    stamp: String,
    pub notice: String,
    recording: Option<(String, Value)>,
    offspring_selected: Option<usize>,
    new_only: bool,
    active_world: String,
    active_plan: String,
    edit_world: String,
    edit_query: String,
    all_steps: bool,
    mode: Option<bool>,
    delete_plan: Option<String>,
    batch: String,
    qa_started: bool,
}
impl Default for Breeding {
    fn default() -> Self {
        Self {
            pal: 0,
            lookup_pal: 0,
            lookup_query: String::new(),
            lookup_filter: String::new(),
            lookup_element: String::new(),
            lookup_page: 0,
            lookup_size: 30,
            stock_page: 0,
            stock_size: 24,
            offspring_page: 0,
            offspring_size: 0,
            plan_name: String::new(),
            recalculate: false,
            query: String::new(),
            filter: String::new(),
            reverse: false,
            element: String::new(),
            traits: vec![],
            picker: false,
            random: false,
            surgery: false,
            result: Value::Null,
            busy: false,
            latency: 0.,
            max_steps: 1,
            edit: None,
            stock_filter: String::new(),
            receiver: None,
            epoch: 0,
            stamp: String::new(),
            notice: String::new(),
            recording: None,
            offspring_selected: None,
            new_only: true,
            active_world: String::new(),
            active_plan: String::new(),
            edit_world: String::new(),
            edit_query: String::new(),
            all_steps: false,
            mode: None,
            delete_plan: None,
            batch: String::new(),
            qa_started: false,
        }
    }
}
impl Breeding {
    pub fn activate_lookup(&mut self, pal: usize, reverse: bool) {
        if self.lookup_pal != pal || self.reverse != reverse {
            self.lookup_page = 0;
            self.lookup_query.clear();
        }
        self.lookup_pal = pal;
        self.reverse = reverse;
    }
    pub fn invalidate(&mut self) {
        self.epoch += 1;
        self.result = Value::Null;
        self.busy = false;
        self.receiver = None;
    }
    pub fn poll(&mut self, store: &Store) {
        let world = text(store.world(), "id");
        if self.active_world != world {
            self.active_world = world.into();
            self.active_plan.clear();
            self.edit = None;
            self.edit_world.clear();
            self.recording = None;
            self.offspring_selected = None;
            self.all_steps = store.world()["inventory"]["limit"] == "all";
            self.max_steps = store.world()["inventory"]["limit"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            self.invalidate();
        }
        let stamp = format!(
            "{}|{}|{}|{}|{:?}|{}|{}",
            world,
            store.world()["inventory"]["records"],
            store.world()["surgery"],
            self.pal,
            self.traits,
            self.random,
            self.surgery
        );
        if self.stamp != stamp {
            self.stamp = stamp;
            self.invalidate();
        }
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok((epoch, result, ms)) => {
                    if epoch == self.epoch {
                        self.result = result;
                        self.latency = ms;
                        self.busy = false;
                    }
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.busy = false;
                    self.receiver = None;
                    self.notice = "计算任务已中断，请重试。".into();
                }
                _ => {}
            }
        }
    }
    pub fn lookup(&mut self, ui: &mut egui::Ui, d: &Data, store: &mut Store) -> Option<String> {
        self.poll(store);
        let mut nav = None;
        let wide = ui.available_width() > 760.;
        if wide {
            ui.horizontal_top(|ui| {
                panel(ui).show(ui, |ui| {
                    ui.set_width(208.);
                    ui.strong("选择帕鲁");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.lookup_query)
                            .desired_width(200.)
                            .hint_text("中文名、英文名或编号"),
                    );
                    let rows: Vec<_> = d
                        .species
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| search(p, &self.lookup_query))
                        .collect();
                    egui::ScrollArea::vertical()
                        .id_salt("lookup-species")
                        .max_height((ui.available_height() - 35.).max(200.))
                        .show_rows(ui, 42., rows.len(), |ui, range| {
                            for i in range {
                                let (index, p) = rows[i];
                                ui.horizontal(|ui| {
                                    d.portrait(ui, text(p, "key"), 32.);
                                    if ui
                                        .selectable_label(
                                            self.lookup_pal == index,
                                            format!("{}  {}", text(p, "dex"), text(p, "zh")),
                                        )
                                        .clicked()
                                    {
                                        self.lookup_pal = index;
                                        self.lookup_page = 0;
                                    }
                                });
                            }
                        });
                    ui.small(format!("{} 个图鉴条目", rows.len()));
                });
                ui.vertical(|ui| {
                    self.lookup_results(ui, d, store, &mut nav);
                });
            });
        } else {
            pal_picker(ui, d, &mut self.lookup_pal, &mut self.lookup_query);
            self.lookup_results(ui, d, store, &mut nav);
        }
        self.edit_dialog(ui.ctx(), d, store);
        nav
    }
    fn lookup_results(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        _store: &mut Store,
        nav: &mut Option<String>,
    ) {
        panel(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.;
            ui.spacing_mut().interact_size.y = 25.;
            ui.spacing_mut().button_padding.y = 3.;
            ui.horizontal(|ui| {
                for (reverse, icon, label) in [
                    (false, "branch", "查配种伙伴"),
                    (true, "parents", "反查双亲"),
                ] {
                    if ui
                        .add(
                            egui::Button::image_and_text(
                                egui::Image::new(crate::design::icon_source(icon))
                                    .fit_to_exact_size(Vec2::splat(15.))
                                    .tint(ui.visuals().text_color()),
                                label,
                            )
                            .selected(self.reverse == reverse),
                        )
                        .clicked()
                    {
                        if self.reverse != reverse {
                            self.lookup_page = 0;
                        }
                        self.reverse = reverse;
                    }
                }
            });
            let p = &d.species[self.lookup_pal];
            let key = text(p, "key");
            ui.separator();
            ui.horizontal(|ui| {
                if d.portrait_link(ui, key, 44.) {
                    *nav = Some(format!("#paldex/{key}"));
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.;
                    ui.spacing_mut().interact_size.y = 14.;
                    ui.small(if self.reverse {
                        "目标子代"
                    } else {
                        "当前亲代"
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Link::new(
                                egui::RichText::new(text(p, "zh")).size(20.).strong(),
                            ))
                            .clicked()
                        {
                            *nav = Some(format!("#paldex/{key}"));
                        }
                        element_badges(ui, &p["types"]);
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.small(format!("#{} · {}", text(p, "dex"), text(p, "en")));
                        if ui
                            .add(egui::Link::new(egui::RichText::new("刷新位置").size(11.)))
                            .clicked()
                        {
                            *nav = Some(format!("#spawns/{key}"));
                        }
                        if ui
                            .add(egui::Link::new(egui::RichText::new("图鉴资料").size(11.)))
                            .clicked()
                        {
                            *nav = Some(format!("#paldex/{key}"));
                        }
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_action(ui, "plus", "加入仓库").clicked() {
                        self.edit = Some(new_record(key));
                    }
                });
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.strong("配种组合");
                let old = self.lookup_element.clone();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.lookup_filter)
                            .desired_width(180.)
                            .hint_text("筛选伙伴或子代"),
                    )
                    .changed()
                {
                    self.lookup_page = 0;
                }
                element_picker(ui, &mut self.lookup_element);
                if old != self.lookup_element {
                    self.lookup_page = 0;
                }
                page_size(
                    ui,
                    "lookup-size",
                    &mut self.lookup_size,
                    &mut self.lookup_page,
                );
            });
            let mut rows: Vec<Value> = d
                .pairs
                .iter()
                .filter(|r| {
                    if self.reverse {
                        r[2].as_u64() == Some(self.lookup_pal as u64)
                    } else {
                        r[0].as_u64() == Some(self.lookup_pal as u64)
                            || r[1].as_u64() == Some(self.lookup_pal as u64)
                    }
                })
                .filter(|r| {
                    let a = r[0].as_u64().unwrap() as usize;
                    let b = r[1].as_u64().unwrap() as usize;
                    let c = r[2].as_u64().unwrap() as usize;
                    let left = if self.reverse {
                        a
                    } else if a == self.lookup_pal {
                        b
                    } else {
                        a
                    };
                    let right = if self.reverse { b } else { c };
                    (self.lookup_element.is_empty()
                        || array(&d.species[right]["types"])
                            .iter()
                            .any(|t| t.as_str() == Some(&self.lookup_element))
                        || (self.reverse
                            && array(&d.species[left]["types"])
                                .iter()
                                .any(|t| t.as_str() == Some(&self.lookup_element))))
                        && (self.lookup_filter.is_empty()
                            || search(&d.species[left], &self.lookup_filter)
                            || search(&d.species[right], &self.lookup_filter))
                })
                .cloned()
                .collect();
            rows.sort_by_key(|r| {
                let a = r[0].as_u64().unwrap() as usize;
                let b = r[1].as_u64().unwrap() as usize;
                if self.reverse {
                    (a, b)
                } else {
                    (
                        if a == self.lookup_pal { b } else { a },
                        r[2].as_u64().unwrap() as usize,
                    )
                }
            });
            let total = rows.len();
            let range = page_range(total, self.lookup_size, &mut self.lookup_page);
            let rows = &rows[range];
            ui.small(if self.reverse {
                "亲代 A ＋ 亲代 B → 当前目标；特殊配方需满足标注性别"
            } else {
                "当前亲代 ＋ 配种伙伴 → 孵化子代；点击帕鲁查看资料与功能入口"
            });
            let width = ui.available_width();
            let cols = ((width / if self.reverse { 285. } else { 340. }) as usize).clamp(1, 4);
            let cell = (width - (cols - 1) as f32 * 6.) / cols as f32;
            egui::ScrollArea::vertical()
                .id_salt("pair-results")
                .max_height((ui.available_height() - 40.).max(170.))
                .show_rows(ui, 40., rows.len().div_ceil(cols), |ui, range| {
                    for row in range {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.;
                            for col in 0..cols {
                                let Some(r) = rows.get(row * cols + col) else {
                                    break;
                                };
                                compact_lookup_pair(
                                    ui,
                                    d,
                                    r,
                                    self.lookup_pal,
                                    self.reverse,
                                    cell,
                                    nav,
                                );
                            }
                        });
                    }
                });
            pagination(ui, total, self.lookup_size, &mut self.lookup_page);
        });
    }
    pub fn warehouse(&mut self, ui: &mut egui::Ui, d: &Data, store: &mut Store) -> Option<String> {
        self.poll(store);
        self.recover(store);
        egui::ScrollArea::vertical()
            .id_salt("warehouse-page")
            .show(ui, |ui| self.warehouse_content(ui, d, store))
            .inner
    }
    fn warehouse_content(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &mut Store,
    ) -> Option<String> {
        let mut nav = None;
        panel(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("仓库成员");
                let records = array(&store.world()["inventory"]["records"]);
                let kinds: std::collections::HashSet<_> =
                    records.iter().map(|r| text(r, "speciesKey")).collect();
                ui.small(format!(
                    "{} 只 · {} 种 · 保存在本机",
                    records.len(),
                    kinds.len()
                ));
                if icon_action(ui, "plus", "添加帕鲁").clicked() {
                    self.edit = Some(new_record(text(&d.species[self.pal], "key")));
                }
                if icon_action(ui, "egg", "可配出的帕鲁").clicked() {
                    nav = Some("#offspring".into());
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.stock_filter)
                            .desired_width(290.)
                            .hint_text("搜索物种、个体名或词条"),
                    )
                    .changed()
                {
                    self.stock_page = 0;
                }
                if !self.stock_filter.is_empty() && icon_action(ui, "reset", "清除").clicked() {
                    self.stock_filter.clear();
                    self.stock_page = 0;
                }
                page_size(
                    ui,
                    "stock-page-size",
                    &mut self.stock_size,
                    &mut self.stock_page,
                );
            });
        });
        ui.add_space(10.);
        let species_order: std::collections::HashMap<_, _> = d
            .species
            .iter()
            .enumerate()
            .map(|(i, p)| (text(p, "key"), i))
            .collect();
        let mut records: Vec<Value> = array(&store.world()["inventory"]["records"])
            .iter()
            .filter(|v| {
                self.stock_filter.is_empty()
                    || search(v, &self.stock_filter)
                    || d.pal_name(text(v, "speciesKey"))
                        .contains(&self.stock_filter)
                    || array(&v["traits"]).iter().any(|t| {
                        d.trait_name(t.as_str().unwrap_or(""))
                            .contains(&self.stock_filter)
                    })
            })
            .cloned()
            .collect();
        records.sort_by_key(|r| {
            species_order
                .get(text(r, "speciesKey"))
                .copied()
                .unwrap_or(usize::MAX)
        });
        let total = records.len();
        let range = page_range(total, self.stock_size, &mut self.stock_page);
        let records = &records[range];
        let cols = ((ui.available_width() / 225.) as usize).clamp(1, 7);
        let width = (ui.available_width() - (cols - 1) as f32 * 10.) / cols as f32;
        let mut remove = None;
        if total == 0 {
            panel(ui).show(ui, |ui| {
                ui.heading(if self.stock_filter.is_empty() {
                    "仓库里还没有帕鲁"
                } else {
                    "没有匹配的仓库成员"
                });
                ui.label("每只个体独立保留名字、性别和词条，同种帕鲁可以拥有多只。");
                if icon_action(ui, "plus", "添加第一只帕鲁").clicked() {
                    self.edit = Some(new_record(text(&d.species[self.pal], "key")));
                }
            });
        }
        egui::ScrollArea::vertical()
            .id_salt("warehouse-cards")
            .max_height((ui.available_height() - 42.).clamp(80., 620.))
            .show_rows(ui, 232., records.len().div_ceil(cols), |ui, range| {
                for row in range {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.;
                        for col in 0..cols {
                            let Some(p) = records.get(row * cols + col) else {
                                break;
                            };
                            let _card = panel(ui).inner_margin(10).show(ui, |ui| {
                                ui.set_width((width - 20.).max(180.));
                                ui.set_min_height(210.);
                                ui.spacing_mut().item_spacing.y = 4.;
                                let key = text(p, "speciesKey");
                                let species = d.species.iter().find(|s| text(s, "key") == key);
                                ui.horizontal(|ui| {
                                    if d.portrait_link(ui, key, 54.) {
                                        nav = Some(format!("#paldex/{key}"));
                                    }
                                    ui.vertical(|ui| {
                                        ui.small(format!(
                                            "{}  {}",
                                            species
                                                .map(|s| format!("#{}", text(s, "dex")))
                                                .unwrap_or_else(|| "图鉴待收录".into()),
                                            sex_label(text(p, "sex"))
                                        ));
                                        if ui
                                            .add(egui::Link::new(
                                                egui::RichText::new(record_species_name(d, p))
                                                    .size(16.)
                                                    .strong(),
                                            ))
                                            .clicked()
                                        {
                                            nav = Some(format!("#paldex/{key}"));
                                        }
                                        let name = text(p, "name");
                                        if !name.is_empty() && name != d.pal_name(key) {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(name).size(12.),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(name);
                                        }
                                    });
                                });
                                ui.separator();
                                ui.small("被动词条");
                                ui.allocate_ui(Vec2::new(width - 20., 78.), |ui| {
                                    // Four compact rows must fit the reserved area; otherwise
                                    // the virtual list's 232 px row stride skips card contents.
                                    ui.spacing_mut().interact_size.y = 16.;
                                    ui.spacing_mut().item_spacing.y = 3.;
                                    if array(&p["traits"]).is_empty() {
                                        ui.label(
                                            egui::RichText::new("尚未登记词条").weak().size(12.),
                                        );
                                    }
                                    for key in array(&p["traits"]).iter().filter_map(Value::as_str)
                                    {
                                        if trait_badge(ui, d, key).clicked() {
                                            nav = Some(format!("#details/passive/trait/{key}"));
                                        }
                                    }
                                });
                                ui.separator();
                                ui.horizontal(|ui| {
                                    if crate::design::small_action(ui, "edit", "编辑").clicked() {
                                        self.edit = Some(p.clone());
                                    }
                                    if crate::design::small_action(ui, "plus", "同种").clicked() {
                                        self.edit = Some(new_record(key));
                                    }
                                    if crate::design::small_action(ui, "trash", "移除").clicked()
                                    {
                                        remove = Some(text(p, "id").to_owned());
                                    }
                                });
                            });
                            #[cfg(test)]
                            track_test_rect(
                                ui,
                                &_card.response,
                                &format!("stock-card:{}", text(p, "id")),
                            );
                        }
                    });
                }
            });
        pagination(ui, total, self.stock_size, &mut self.stock_page);
        if let Some(id) = remove {
            let world = store.world_mut();
            let mut record = None;
            world["inventory"]["records"]
                .as_array_mut()
                .unwrap()
                .retain(|p| {
                    if text(p, "id") == id {
                        record = Some(p.clone());
                        false
                    } else {
                        true
                    }
                });
            if let Some(p) = record {
                if !world["inventory"]["archived"].is_array() {
                    world["inventory"]["archived"] = json!([]);
                }
                world["inventory"]["archived"]
                    .as_array_mut()
                    .unwrap()
                    .push(p);
            }
            if !world["inventory"]["removed"].is_array() {
                world["inventory"]["removed"] = json!([]);
            }
            world["inventory"]["removed"]
                .as_array_mut()
                .unwrap()
                .push(json!(id));
            self.invalidate();
        }
        egui::CollapsingHeader::new(format!(
            "已移出 / 存档中已失去（{}）",
            array(&store.world()["inventory"]["archived"]).len()
        ))
        .show(ui, |ui| {
            let mut restore = None;
            for p in array(&store.world()["inventory"]["archived"]) {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} · {}",
                        text(p, "name"),
                        d.pal_name(text(p, "speciesKey"))
                    ));
                    if p["terminalArchived"] != true
                        && crate::design::small_action(ui, "reset", "恢复手工记录").clicked()
                    {
                        restore = Some(p.clone());
                    }
                });
            }
            if let Some(p) = restore {
                let id = text(&p, "id").to_owned();
                let w = store.world_mut();
                w["inventory"]["archived"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|p| text(p, "id") != id);
                w["inventory"]["removed"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|v| v.as_str() != Some(&id));
                w["inventory"]["records"].as_array_mut().unwrap().push(p);
                self.invalidate();
            }
        });
        egui::CollapsingHeader::new("批量添加个体").show(ui, |ui| {
            ui.label(
                "输入准确的中文名、英文名或游戏编号，用逗号、顿号或换行分隔；重复名字会分别添加。",
            );
            ui.text_edit_multiline(&mut self.batch);
            if icon_action(ui, "plus", "添加已识别的个体").clicked() {
                let terms: Vec<_> = self
                    .batch
                    .split([',', '，', '、', ';', '；', '\n', '\r'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                let mut added = vec![];
                let mut unknown = vec![];
                for term in terms {
                    let matches: Vec<_> = d
                        .species
                        .iter()
                        .filter(|p| {
                            ["key", "zh", "en", "dex"].iter().any(|field| {
                                text(p, field).eq_ignore_ascii_case(term.trim_start_matches('#'))
                            })
                        })
                        .collect();
                    if matches.len() == 1 {
                        let mut row = new_record(text(matches[0], "key"));
                        row["name"] = matches[0]["zh"].clone();
                        added.push(row);
                    } else {
                        unknown.push(term.to_owned());
                    }
                }
                if !added.is_empty() {
                    let mut next = store.state.clone();
                    let i = store.world_index();
                    let count = added.len();
                    next["worlds"][i]["inventory"]["records"]
                        .as_array_mut()
                        .unwrap()
                        .extend(added);
                    match store.commit(next) {
                        Ok(()) => {
                            self.notice =
                                format!("已添加 {count} 只。未识别：{}", unknown.join("、"));
                            self.batch = unknown.join("、");
                            self.invalidate();
                        }
                        Err(e) => self.notice = e,
                    }
                } else {
                    self.notice = "没有准确匹配的帕鲁，请检查输入名称。".into();
                }
            }
        });
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        self.edit_dialog(ui.ctx(), d, store);
        nav
    }
    pub fn planner(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &mut Store,
        offspring: bool,
    ) -> Option<String> {
        if self.mode != Some(offspring) {
            self.mode = Some(offspring);
            self.invalidate();
        }
        self.poll(store);
        self.recover(store);
        egui::ScrollArea::vertical()
            .id_salt(if offspring {
                "offspring-page"
            } else {
                "planner-page"
            })
            .show(ui, |ui| self.planner_content(ui, d, store, offspring))
            .inner
    }
    fn planner_content(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &mut Store,
        offspring: bool,
    ) -> Option<String> {
        let mut nav = None;
        let data_ready =
            offspring || !self.surgery || d.get("pal-details-data")["passives"].is_object();
        let qa_run = !offspring
            && !self.qa_started
            && data_ready
            && std::env::var("JADE_QA_PLANNER").as_deref() == Ok("1");
        if qa_run {
            self.qa_started = true;
            let stock = array(&store.world()["inventory"]["records"]);
            if let Some(target) = d.pairs.iter().find_map(|pair| {
                let a = d.species.get(pair[0].as_u64()? as usize)?;
                let b = d.species.get(pair[1].as_u64()? as usize)?;
                let usable = stock.iter().any(|x| {
                    x["speciesKey"] == a["key"]
                        && stock.iter().any(|y| {
                            y["speciesKey"] == b["key"]
                                && x["id"] != y["id"]
                                && ((x["sex"] == "M" && y["sex"] == "F")
                                    || (x["sex"] == "F" && y["sex"] == "M"))
                        })
                });
                usable.then_some(pair[2].as_u64()? as usize)
            }) {
                self.pal = target;
                self.traits.clear();
                self.poll(store);
            }
        }
        if !offspring && ui.available_width() > 840. {
            ui.horizontal_top(|ui| {
                panel(ui).show(ui, |ui| {
                    ui.set_width(290.);
                    self.planner_controls(ui, d, store, offspring, data_ready, qa_run, &mut nav);
                });
                ui.vertical(|ui| {
                    self.planner_output(ui, d, store, offspring, &mut nav);
                });
            });
        } else {
            panel(ui).show(ui, |ui| {
                self.planner_controls(ui, d, store, offspring, data_ready, qa_run, &mut nav)
            });
            ui.add_space(10.);
            self.planner_output(ui, d, store, offspring, &mut nav);
        }
        if self.picker {
            let before = self.traits.clone();
            trait_picker(ui.ctx(), d, &mut self.picker, &mut self.traits);
            if before != self.traits {
                self.invalidate();
            }
        }
        self.edit_dialog(ui.ctx(), d, store);
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        nav
    }
    fn planner_controls(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &mut Store,
        offspring: bool,
        data_ready: bool,
        qa_run: bool,
        nav: &mut Option<String>,
    ) {
        if !offspring {
            crate::design::section(ui, "设定目标");
            ui.horizontal_wrapped(|ui| {
                if pal_picker(ui, d, &mut self.pal, &mut self.query) {
                    self.active_plan.clear();
                    self.invalidate();
                }
                if icon_action(
                    ui,
                    "list",
                    &format!("选择目标词条（{}/4）", self.traits.len()),
                )
                .clicked()
                {
                    self.picker = true;
                }
            });
            ui.horizontal_wrapped(|ui| {
                for t in self.traits.clone() {
                    if trait_remove_tag(ui, d, &t).clicked() {
                        self.traits.retain(|x| x != &t);
                        self.invalidate();
                    }
                }
            });
            ui.vertical(|ui| {
                if crate::design::checkbox(ui, &mut self.random, "允许随机孵出词条").changed()
                {
                    self.invalidate();
                }
                if crate::design::checkbox(ui, &mut self.surgery, "使用已拥有的手术台词条")
                    .changed()
                {
                    self.invalidate();
                }
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                crate::design::section(ui, "繁育结果");
                ui.label("最多配种代数");
                let mut changed =
                    crate::design::checkbox(ui, &mut self.all_steps, "不限步数").changed();
                changed |= ui
                    .add_enabled(
                        !self.all_steps,
                        egui::DragValue::new(&mut self.max_steps).range(1..=d.species.len()),
                    )
                    .changed();
                if changed {
                    store.world_mut()["inventory"]["limit"] = json!(if self.all_steps {
                        "all".into()
                    } else {
                        self.max_steps.to_string()
                    });
                }
            });
        }
        if ui
            .add_enabled_ui(!self.busy && data_ready, |ui| {
                icon_action(
                    ui,
                    "branch",
                    if offspring {
                        "计算可配出的帕鲁"
                    } else {
                        "计算配种路径"
                    },
                )
            })
            .inner
            .clicked()
            || qa_run
            || (offspring && !self.busy && self.result.is_null())
            || (self.recalculate && data_ready && !self.busy)
        {
            self.start_calculation(ui.ctx(), d, store, offspring);
        }

        if !offspring {
            let key = text(&d.species[self.pal], "key");
            ui.horizontal(|ui| {
                if d.portrait_link(ui, key, 58.) {
                    *nav = Some(format!("#paldex/{key}"));
                }
                ui.vertical(|ui| {
                    if ui.link(d.pal_name(key)).clicked() {
                        *nav = Some(format!("#paldex/{key}"));
                    }
                    element_badges(ui, &d.species[self.pal]["types"]);
                });
            });
            ui.horizontal_wrapped(|ui| {
                if icon_action(ui, "pin", "刷新位置").clicked() {
                    *nav = Some(format!("#spawns/{key}"));
                }
                if icon_action(ui, "surgery", "管理手术台词条").clicked() {
                    *nav = Some("#details/surgery".into());
                }
            });
            ui.small("最多选 4 个词条；留空可只规划物种。目标子代需同时具有选中的词条。");
        }
        ui.separator();
        let stock = array(&store.world()["inventory"]["records"]);
        let kinds: std::collections::HashSet<_> =
            stock.iter().map(|r| text(r, "speciesKey")).collect();
        let unknown = stock
            .iter()
            .filter(|r| !matches!(text(r, "sex"), "M" | "F"))
            .count();
        ui.horizontal_wrapped(|ui| {
            ui.strong("仓库亲代");
            ui.label(format!(
                "{} 只个体 · {} 种 · {} 只性别待确认",
                stock.len(),
                kinds.len(),
                unknown
            ));
            if offspring {
                ui.small(format!("当前世界：{}", text(store.world(), "name")));
            }
            if ui.link("打开配种仓库 →").clicked() {
                *nav = Some("#warehouse".into());
            }
        });
        if !offspring {
            ui.small(format!("当前世界：{}", text(store.world(), "name")));
        }
        if !data_ready {
            ui.label("正在准备手术台词条资料…");
        }
    }
    fn start_calculation(&mut self, ctx: &egui::Context, d: &Data, store: &Store, offspring: bool) {
        self.recalculate = false;
        self.poll(store);
        let stock: Vec<_> = array(&store.world()["inventory"]["records"])
            .iter()
            .filter_map(|r| {
                d.species
                    .iter()
                    .position(|p| p["key"] == r["speciesKey"])
                    .map(|index| {
                        let mut r = r.clone();
                        r["species"] = json!(index);
                        r
                    })
            })
            .collect();
        let options: Vec<_> = if self.surgery {
            self.traits
                .iter()
                .filter_map(|key| {
                    let mut op = d.get("pal-details-data")["passives"][key]["surgery"].clone();
                    if op.is_object()
                        && (op["mode"] == "coins"
                            || array(&store.world()["surgery"])
                                .iter()
                                .any(|v| v.as_str() == Some(key)))
                    {
                        op["key"] = json!(key);
                        Some(op)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        };
        let eligible: Vec<_> = self
            .traits
            .iter()
            .filter(|k| {
                array(&d.get("trait-data")["traits"])
                    .iter()
                    .any(|t| t["key"].as_str() == Some(k) && t["random"] == true)
            })
            .cloned()
            .collect();
        let request = json!({"pairs":d.pairs,"speciesCount":d.species.len(),"stock":stock,"target":self.pal,"traits":self.traits,"allowRandom":self.random,"randomEligible":eligible,"surgeryOptions":options,"preferHatch":true,"maxSteps":self.max_steps});
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.busy = true;
        self.result = Value::Null;
        let epoch = self.epoch;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let result = if offspring {
                jade_core::offspring_solve(&request)
            } else {
                jade_core::plan_request(&request)
            };
            let _ = tx.send((epoch, result, start.elapsed().as_secs_f64() * 1000.));
            ctx.request_repaint();
        });
    }
    fn planner_output(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &mut Store,
        offspring: bool,
        nav: &mut Option<String>,
    ) {
        if !offspring {
            panel(ui).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("我的配种计划");
                    let previous = self.active_plan.clone();
                    egui::ComboBox::from_id_salt("saved-plan-select")
                        .width(180.)
                        .selected_text(
                            array(&store.world()["plans"])
                                .iter()
                                .find(|p| p["id"] == self.active_plan)
                                .map(|p| text(p, "name"))
                                .unwrap_or("未保存的目标"),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.active_plan,
                                String::new(),
                                "未保存的目标",
                            );
                            for p in array(&store.world()["plans"]) {
                                ui.selectable_value(
                                    &mut self.active_plan,
                                    text(p, "id").into(),
                                    text(p, "name"),
                                );
                            }
                        });
                    if previous != self.active_plan {
                        if let Some(p) = array(&store.world()["plans"])
                            .iter()
                            .find(|p| p["id"] == self.active_plan)
                            .cloned()
                        {
                            self.load_plan(d, &p);
                        } else {
                            self.plan_name.clear();
                            self.invalidate();
                        }
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.plan_name)
                            .desired_width(180.)
                            .hint_text("计划名称（可选）"),
                    );
                    if icon_action(
                        ui,
                        "save",
                        if self.active_plan.is_empty() {
                            "保存为计划"
                        } else {
                            "更新计划"
                        },
                    )
                    .clicked()
                    {
                        self.save_plan(d, store);
                    }
                    if icon_action(ui, "plus", "新建计划").clicked() {
                        self.active_plan.clear();
                        self.plan_name.clear();
                        self.traits.clear();
                        self.random = false;
                        self.surgery = false;
                        self.invalidate();
                    }
                });
            });
            ui.add_space(10.);
        }
        if self.busy {
            ui.spinner();
            ui.label("正在计算，可继续使用其他页面。");
        }
        if !self.result.is_null() {
            ui.small(format!("计算耗时 {:.2} ms", self.latency));
            if offspring {
                self.offspring_results(ui, d, store, nav);
            } else {
                let status = text(&self.result, "status");
                ui.heading(match status {
                    "ok" => "找到配种路径",
                    "owned" => "仓库已有符合条件的帕鲁",
                    "missing-traits" => "仓库尚缺目标词条",
                    "unreachable-species" => "现有帕鲁暂时无法配出目标",
                    _ => "配种结果",
                });
                render_plan(ui, d, &self.result, store.world(), nav);
            }
        }
        if !offspring {
            self.saved_plans(ui, d, store, nav);
        }
        if self.result.is_null() && !self.busy && !offspring {
            panel(ui).show(ui, |ui| {
                ui.set_min_height(230.);
                crate::design::section(ui, "配种路线");
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.add_space(35.);
                    crate::design::icon(ui, "branch", 32.);
                    ui.strong("等待设定目标");
                    ui.label("选择目标帕鲁和词条后开始计算。");
                    ui.small("路线会列出仓库个体、逐代配对和手术方案。");
                });
            });
        }
    }
    fn load_plan(&mut self, d: &Data, p: &Value) {
        if let Some(index) = d.species.iter().position(|s| s["key"] == p["targetKey"]) {
            self.active_plan = text(p, "id").into();
            self.plan_name = text(p, "name").into();
            self.pal = index;
            self.traits = array(&p["traits"])
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            self.random = p["random"] == true;
            self.surgery = p["surgery"] == true;
            self.recalculate = true;
            self.invalidate();
        }
    }
    fn save_plan(&mut self, d: &Data, store: &mut Store) {
        let route = if matches!(text(&self.result, "status"), "ok" | "owned") {
            stable_route(d, &self.result, store.world())
        } else {
            Value::Null
        };
        let mut next = store.state.clone();
        let world = store.world_index();
        let plans = next["worlds"][world]["plans"].as_array_mut().unwrap();
        let name = if self.plan_name.trim().is_empty() {
            format!("{} 配种计划", text(&d.species[self.pal], "zh"))
        } else {
            self.plan_name.trim().to_owned()
        };
        if let Some(plan) = plans.iter_mut().find(|p| p["id"] == self.active_plan) {
            plan["currentRoute"] = route;
            plan["nativeResult"] = self.result.clone();
            plan["name"] = json!(name);
            plan["targetKey"] = d.species[self.pal]["key"].clone();
            plan["traits"] = json!(self.traits);
            plan["random"] = json!(self.random);
            plan["surgery"] = json!(self.surgery);
        } else {
            let id = format!("plan-{}", uuid::Uuid::new_v4());
            plans.push(json!({"id":id,"created":timestamp(),"name":name,"targetKey":d.species[self.pal]["key"],"traits":self.traits,"random":self.random,"surgery":self.surgery,"initialRoute":route,"nativeResult":self.result,"log":[],"pending":null}));
            self.active_plan = id;
        }
        match store.commit(next) {
            Ok(()) => self.notice = "计划已保存；可以登记实际孵化与手术结果。".into(),
            Err(e) => self.notice = e,
        }
    }
    fn saved_plans(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &mut Store,
        nav: &mut Option<String>,
    ) {
        egui::CollapsingHeader::new("保存的配种计划与进度").default_open(true).show(ui,|ui|{
            let plans=array(&store.world()["plans"]).to_vec();let mut delete=None;
            for p in &plans{ui.push_id(text(p,"id"),|ui|{
                let title=if text(p,"name").is_empty(){format!("{} · {} 条词条",d.pal_name(text(p,"targetKey")),array(&p["traits"]).len())}else{text(p,"name").into()};
                egui::CollapsingHeader::new(title).show(ui,|ui|{
                    ui.label(format!("已登记 {} 个实际结果",array(&p["log"]).len()));
                    let route=if p["currentRoute"].is_object(){&p["currentRoute"]}else{&p["initialRoute"]};
                    for (i,step) in array(&route["steps"]).iter().enumerate(){
                        ui.group(|ui|{ui.strong(format!("第 {} 代 · {}",step["generation"],d.pal_name(text(step,"speciesKey"))));
                            ui.horizontal_wrapped(|ui|{for parent in array(&step["parents"]){ui.label(format!("{} {} {}",d.pal_name(text(parent,"speciesKey")),sex_label(text(parent,"sex")),text(parent,"name")));}crate::design::icon(ui,"arrow",16.);for t in array(&step["traits"]){if trait_badge(ui,d,t.as_str().unwrap_or("")).clicked(){*nav=Some(format!("#details/passive/trait/{}",t.as_str().unwrap_or("")));}}});
                            let parents_ready=array(&step["parents"]).len()==2&&array(&step["parents"]).iter().all(|parent|array(&store.world()["inventory"]["records"]).iter().any(|r|r["id"]==parent["stockId"]));
                            if ui.add_enabled_ui(parents_ready,|ui|icon_action(ui,"egg",&format!("登记第 {} 步实际孵化结果",i+1))).inner.on_disabled_hover_text("请先登记前一步的实际子代，再重新计算并更新计划路线。").clicked(){let mut draft=new_record(text(step,"speciesKey"));let id=uuid::Uuid::new_v4().to_string();draft["id"]=json!(format!("hatch-{id}"));draft["name"]=json!(format!("{} 第{}只",d.pal_name(text(step,"speciesKey")),array(&p["log"]).len()+1));draft["traits"]=step["traits"].clone();self.edit=Some(draft);self.recording=Some((text(p,"id").into(),json!({"id":id,"stockId":format!("hatch-{id}"),"kind":"hatch","at":timestamp(),"speciesKey":step["speciesKey"],"generation":step["generation"],"expectedTraits":step["traits"],"expectedSexes":step["sexes"],"parents":step["parents"]})));}
                        });
                    }
                    if p["surgery"]==true{egui::CollapsingHeader::new("登记现有目标个体的实际手术结果").show(ui,|ui|{let targets:Vec<_>=array(&store.world()["inventory"]["records"]).iter().filter(|r|r["speciesKey"]==p["targetKey"]).cloned().collect();if targets.is_empty(){ui.label("请先登记配出的目标帕鲁。");}for r in targets{if icon_action(ui,"surgery",&format!("登记手术 · {} · {}",text(&r,"name"),sex_label(text(&r,"sex")))).clicked(){let id=uuid::Uuid::new_v4().to_string();self.recording=Some((text(p,"id").into(),json!({"id":id,"stockId":r["id"],"kind":"surgery","at":timestamp(),"speciesKey":r["speciesKey"],"generation":0,"expectedTraits":p["traits"],"expectedSexes":[],"parents":[]})));self.edit=Some(r);}}});}
                    egui::CollapsingHeader::new("实际孵化 / 手术记录").show(ui,|ui|render_log(ui,d,&p["log"],nav));
                    ui.horizontal(|ui|{if icon_action(ui, "reset", "载入目标并重新计算").clicked(){if let Some(index)=d.species.iter().position(|s|s["key"]==p["targetKey"]){self.active_plan=text(p,"id").into();self.plan_name=text(p,"name").into();self.recalculate=true;self.pal=index;self.traits=array(&p["traits"]).iter().filter_map(|v|v.as_str().map(str::to_owned)).collect();self.random=p["random"]==true;self.surgery=p["surgery"]==true;self.invalidate();self.notice="已载入计划，正在按最新仓库重新计算。".into();}}if icon_action(ui, "trash", "删除计划").clicked(){delete=Some(text(p,"id").to_owned());}});
                });
            });}
            if let Some(id)=delete{self.delete_plan=Some(id);}
        });
        if let Some(id) = self.delete_plan.clone() {
            let mut keep = true;
            egui::Window::new("确认删除配种计划")
                .title_bar(false)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    if crate::design::dialog_header(ui, "trash", "确认删除配种计划").clicked()
                    {
                        keep = false;
                    }
                    ui.label("删除该计划及执行记录？已经入仓的个体仍会保留。");
                    ui.horizontal(|ui| {
                        if icon_action(ui, "trash", "确认删除").clicked() {
                            let mut next = store.state.clone();
                            let i = store.world_index();
                            next["worlds"][i]["plans"]
                                .as_array_mut()
                                .unwrap()
                                .retain(|p| p["id"] != id);
                            match store.commit(next) {
                                Ok(()) => {
                                    keep = false;
                                    if self.active_plan == id {
                                        self.active_plan.clear();
                                    }
                                }
                                Err(e) => self.notice = e,
                            }
                        }
                        if icon_action(ui, "close", "取消").clicked() {
                            keep = false;
                        }
                    });
                });
            if !keep {
                self.delete_plan = None;
            }
        }
    }
    fn offspring_results(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        store: &Store,
        nav: &mut Option<String>,
    ) {
        panel(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let previous = self.element.clone();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .desired_width(210.)
                            .hint_text("筛选目标子代"),
                    )
                    .changed()
                {
                    self.offspring_page = 0;
                }
                element_picker(ui, &mut self.element);
                if previous != self.element {
                    self.offspring_page = 0;
                }
                if crate::design::checkbox(ui, &mut self.new_only, "仅看未拥有").changed() {
                    self.offspring_page = 0;
                }
                page_size(
                    ui,
                    "offspring-size",
                    &mut self.offspring_size,
                    &mut self.offspring_page,
                );
            });
            let owned: std::collections::HashSet<_> = array(&store.world()["inventory"]["records"])
                .iter()
                .map(|r| text(r, "speciesKey"))
                .collect();
            let rows: Vec<Value> = array(&self.result["items"])
                .iter()
                .filter(|r| {
                    self.all_steps
                        || r["depth"].as_u64().unwrap_or(u64::MAX) <= self.max_steps as u64
                })
                .filter(|r| {
                    let Some(p) = r["id"].as_u64().and_then(|i| d.species.get(i as usize)) else {
                        return false;
                    };
                    search(p, &self.filter)
                        && (self.element.is_empty()
                            || array(&p["types"])
                                .iter()
                                .any(|t| t.as_str() == Some(&self.element)))
                        && (!self.new_only || !owned.contains(text(p, "key")))
                })
                .cloned()
                .collect();
            let total = rows.len();
            let range = page_range(total, self.offspring_size, &mut self.offspring_page);
            let rows = &rows[range];
            ui.small(format!(
                "{} 种帕鲁 · {} · 点选组合查看完整路线",
                total,
                if self.all_steps {
                    "不限步数".into()
                } else {
                    format!("{} 步以内", self.max_steps)
                }
            ));
            if total == 0 {
                ui.add_space(15.);
                ui.strong("没有符合条件的繁育结果");
                ui.label("可放宽步数、取消「仅看未拥有」，或补齐仓库个体及性别后重算。");
                if icon_action(ui, "reset", "清除结果筛选").clicked() {
                    self.filter.clear();
                    self.element.clear();
                    self.new_only = false;
                    self.all_steps = true;
                }
            }
            let cols = ((ui.available_width() / 315.) as usize).clamp(1, 5);
            let width = (ui.available_width() - (cols - 1) as f32 * 8.) / cols as f32;
            egui::ScrollArea::vertical()
                .id_salt("offspring-results")
                .max_height((ui.available_height() - 42.).clamp(90., 550.))
                .show_rows(ui, 90., rows.len().div_ceil(cols), |ui, range| {
                    for row in range {
                        ui.horizontal_top(|ui| {
                            for col in 0..cols {
                                let Some(r) = rows.get(row * cols + col) else {
                                    break;
                                };
                                let id = r["id"].as_u64().unwrap() as usize;
                                let p = &d.species[id];
                                let mut open_route = false;
                                let _card = Card(
                                    egui::Frame::new()
                                        .fill(ui.visuals().faint_bg_color)
                                        .corner_radius(5)
                                        .inner_margin(8),
                                )
                                .show(ui, |ui| {
                                    ui.set_width((width - 16.).max(140.));
                                    ui.set_min_height(66.);
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width((width * 0.43).max(100.));
                                            ui.spacing_mut().item_spacing.y = 2.;
                                            let parents = array(&r["steps"])
                                                .last()
                                                .map(|s| array(&s["parents"]))
                                                .unwrap_or(&[]);
                                            for (index, parent) in parents.iter().enumerate() {
                                                if let Some(p) = parent["species"]
                                                    .as_u64()
                                                    .and_then(|i| d.species.get(i as usize))
                                                {
                                                    ui.horizontal(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 3.;
                                                        if d.portrait_link(ui, text(p, "key"), 22.)
                                                        {
                                                            *nav = Some(format!(
                                                                "#paldex/{}",
                                                                text(p, "key")
                                                            ));
                                                        }
                                                        let response = ui
                                                            .add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(text(
                                                                        p, "zh",
                                                                    ))
                                                                    .size(12.),
                                                                )
                                                                .truncate()
                                                                .sense(egui::Sense::click()),
                                                            )
                                                            .on_hover_cursor(
                                                                egui::CursorIcon::PointingHand,
                                                            )
                                                            .on_hover_text(format!(
                                                                "亲代{}：{} · 查看资料与功能入口",
                                                                index + 1,
                                                                text(p, "zh")
                                                            ));
                                                        if response.clicked() {
                                                            *nav = Some(format!(
                                                                "#paldex/{}",
                                                                text(p, "key")
                                                            ));
                                                        }
                                                    });
                                                }
                                            }
                                        });
                                        crate::design::icon(ui, "arrow", 16.);
                                        if d.portrait_link(ui, text(p, "key"), 36.) {
                                            *nav = Some(format!("#paldex/{}", text(p, "key")));
                                        }
                                        ui.vertical(|ui| {
                                            if ui
                                                .add(
                                                    egui::Label::new(
                                                        egui::RichText::new(text(p, "zh")).strong(),
                                                    )
                                                    .truncate()
                                                    .sense(egui::Sense::click()),
                                                )
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                .clicked()
                                            {
                                                *nav = Some(format!("#paldex/{}", text(p, "key")));
                                            }
                                            ui.small(format!(
                                                "#{} · {} 步",
                                                text(p, "dex"),
                                                r["depth"]
                                            ));
                                            if r["conditional"] == true {
                                                ui.small("需确认性别");
                                            }
                                            if crate::design::small_action(ui, "branch", "配种路线")
                                                .clicked()
                                            {
                                                open_route = true;
                                            }
                                        });
                                    });
                                })
                                .response;
                                if open_route {
                                    self.offspring_selected = Some(id);
                                }
                            }
                        });
                    }
                });
            pagination(ui, total, self.offspring_size, &mut self.offspring_page);
        });
        if let Some(id) = self.offspring_selected {
            if let Some(r) = array(&self.result["items"])
                .iter()
                .find(|r| r["id"].as_u64() == Some(id as u64))
            {
                let mut open = true;
                let mut requested_close = false;
                egui::Window::new("配种路线").title_bar(false).id(egui::Id::new("offspring-route")).open(&mut open).default_width(740.).vscroll(true).show(ui.ctx(),|ui|{if crate::design::dialog_header(ui,"branch","配种路线").clicked(){requested_close=true;}let p=&d.species[id];ui.horizontal(|ui|{if d.portrait_link(ui,text(p,"key"),56.){*nav=Some(format!("#paldex/{}",text(p,"key")));}ui.vertical(|ui|{if ui.link(text(p,"zh")).clicked(){*nav=Some(format!("#paldex/{}",text(p,"key")));}ui.label(format!("{} {} 代 · {} 组配对",if r["minimumCertain"]==true{"最少"}else{"此路线"},r["depth"],array(&r["steps"]).len()));});if icon_action(ui,"pin","刷新位置").clicked(){*nav=Some(format!("#spawns/{}",text(p,"key")));}});
            for step in array(&r["steps"]){panel(ui).show(ui,|ui|{ui.strong(format!("第 {} 步",step["depth"]));ui.horizontal_wrapped(|ui|{for (i,parent) in array(&step["parents"]).iter().enumerate(){if i>0{crate::design::icon(ui,"plus",16.);}if let Some(p)=parent["species"].as_u64().and_then(|i|d.species.get(i as usize)){if d.portrait_link(ui,text(p,"key"),34.){*nav=Some(format!("#paldex/{}",text(p,"key")));}ui.vertical(|ui|{if ui.link(format!("{} {}",text(p,"zh"),sex_label(text(parent,"sex")))).clicked(){*nav=Some(format!("#paldex/{}",text(p,"key")));}if let Some(stock)=array(&store.world()["inventory"]["records"]).iter().find(|r|r["id"]==parent["stockId"]){ui.small(format!("仓库：{}",text(stock,"name")));}if parent["unknown"]==true{ui.small("性别待确认");}});}}crate::design::icon(ui,"arrow",16.);if let Some(p)=step["child"].as_u64().and_then(|i|d.species.get(i as usize)){if d.portrait_link(ui,text(p,"key"),34.){*nav=Some(format!("#paldex/{}",text(p,"key")));}if ui.link(text(p,"zh")).clicked(){*nav=Some(format!("#paldex/{}",text(p,"key")));}}});});ui.add_space(7.);}
            ui.small("子代用于后续配对时，请留下路线要求的性别。标注待确认的仓库个体需要先核对性别。");});
                if requested_close {
                    open = false;
                }
                if !open {
                    self.offspring_selected = None;
                }
            }
        }
        if self.result["unresolved"].as_u64().unwrap_or(0) > 0 {
            ui.label("部分路线受未知性别影响，无法确认最短路径；补齐性别后会自动重新计算。");
        }
    }
    fn edit_dialog(&mut self, ctx: &egui::Context, d: &Data, store: &mut Store) {
        let Some(mut draft) = self.edit.clone() else {
            self.edit_world.clear();
            return;
        };
        if self.edit_world.is_empty() {
            self.edit_world = text(store.world(), "id").into();
            self.edit_query.clear();
        }
        if self.edit_world != text(store.world(), "id") {
            self.edit = None;
            self.recording = None;
            self.notice = "世界已切换，请重新打开个体。".into();
            return;
        }
        let mut open = true;
        let mut save = false;
        let mut requested_close = false;
        let title = if self
            .recording
            .as_ref()
            .is_some_and(|(_, e)| e["kind"] == "surgery")
        {
            "登记实际手术结果"
        } else if self.recording.is_some() {
            "登记实际孵化结果"
        } else {
            "帕鲁个体"
        };
        egui::Window::new(title)
            .title_bar(false)
            .id(egui::Id::new("edit-pal"))
            .open(&mut open)
            .default_width(1100.)
            .default_height((ctx.content_rect().height() - 80.).min(780.))
            .vscroll(true)
            .show(ctx, |ui| {
                if crate::design::dialog_header(ui, "paldex", title).clicked() {
                    requested_close = true;
                }
                let mut index = d
                    .species
                    .iter()
                    .position(|p| p["key"] == draft["speciesKey"])
                    .unwrap_or(0);
                let fixed = self.recording.is_some()
                    || array(&store.world()["inventory"]["records"])
                        .iter()
                        .any(|r| r["id"] == draft["id"]);
                if fixed {
                    ui.strong(d.pal_name(text(&draft, "speciesKey")));
                } else if pal_picker(ui, d, &mut index, &mut self.edit_query) {
                    draft["speciesKey"] = d.species[index]["key"].clone();
                }
                if let Some((_, entry)) = &self.recording {
                    ui.label(format!(
                        "本步期望词条：{}。请填写游戏中实际获得的词条和性别。",
                        array(&entry["expectedTraits"])
                            .iter()
                            .map(|k| d.trait_name(k.as_str().unwrap_or("")))
                            .collect::<Vec<_>>()
                            .join("、")
                    ));
                }
                let mut name = text(&draft, "name").to_owned();
                ui.horizontal(|ui| {
                    ui.label("名字");
                    if ui
                        .add(egui::TextEdit::singleline(&mut name).char_limit(
                            if text(&draft, "id").starts_with("terminal-") {
                                256
                            } else {
                                48
                            },
                        ))
                        .changed()
                    {
                        draft["name"] = json!(name);
                    }
                });
                let mut sex = text(&draft, "sex").to_owned();
                ui.horizontal(|ui| {
                    for (key, label) in [("M", "雄性"), ("F", "雌性"), ("unknown", "未知")] {
                        if ui.selectable_value(&mut sex, key.into(), label).changed() {
                            draft["sex"] = json!(sex);
                        }
                    }
                });
                let mut keys: Vec<String> = array(&draft["traits"])
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect();
                trait_grid(ui, d, &mut keys);
                draft["traits"] = json!(keys);
                if icon_action(ui, "save", "保存个体").clicked() {
                    save = true;
                }
            });
        if requested_close {
            open = false;
        }
        if save {
            if text(&draft, "name").trim().is_empty() {
                let count = array(&store.world()["inventory"]["records"])
                    .iter()
                    .filter(|r| r["speciesKey"] == draft["speciesKey"])
                    .count()
                    + 1;
                draft["name"] = json!(format!(
                    "{} {}",
                    d.pal_name(text(&draft, "speciesKey")),
                    count
                ));
            }
            let result = if let Some((plan_id, entry)) = &self.recording {
                let mut entry = entry.clone();
                for key in ["name", "sex", "traits"] {
                    entry[key] = draft[key].clone();
                }
                record_event(store, &self.edit_world, plan_id, entry)
            } else {
                let mut next = store.state.clone();
                let i = store.world_index();
                let rows = next["worlds"][i]["inventory"]["records"]
                    .as_array_mut()
                    .unwrap();
                if let Some(r) = rows.iter_mut().find(|r| r["id"] == draft["id"]) {
                    *r = draft.clone();
                } else {
                    rows.push(draft.clone());
                }
                store.commit(next)
            };
            match result {
                Ok(()) => {
                    let recorded_plan = self.recording.as_ref().map(|(id, _)| id.clone());
                    self.edit = None;
                    self.recording = None;
                    self.edit_world.clear();
                    self.active_plan = recorded_plan.clone().unwrap_or_default();
                    self.recalculate = recorded_plan.is_some();
                    self.invalidate();
                    self.notice = "个体与执行记录已保存。".into();
                }
                Err(e) => {
                    self.edit = Some(draft);
                    self.notice = e;
                }
            }
        } else if open {
            self.edit = Some(draft);
        } else {
            self.edit = None;
            self.recording = None;
            self.edit_world.clear();
        }
    }
    fn recover(&mut self, store: &mut Store) {
        if array(&store.world()["plans"])
            .iter()
            .any(|p| !p["pending"].is_null())
        {
            match settle_pending(store) {
                Ok(n) => {
                    if n > 0 {
                        self.notice = format!("已恢复 {n} 条中断前登记的执行记录。");
                    }
                }
                Err(e) => self.notice = e,
            }
        }
    }
}
pub fn pal_picker(ui: &mut egui::Ui, d: &Data, index: &mut usize, query: &mut String) -> bool {
    let before = *index;
    egui::ComboBox::from_id_salt("pal-picker")
        .width(220.)
        .height(480.)
        .selected_text(
            d.species
                .get(*index)
                .map(|p| format!("#{} {}", text(p, "dex"), text(p, "zh")))
                .unwrap_or_default(),
        )
        .show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(query).hint_text("搜索全部 300 个帕鲁"));
            for (i, p) in d.species.iter().enumerate() {
                if search(p, query) {
                    ui.selectable_value(index, i, format!("#{} {}", text(p, "dex"), text(p, "zh")));
                }
            }
        });
    *index != before
}
pub fn trait_grid(ui: &mut egui::Ui, d: &Data, keys: &mut Vec<String>) {
    ui.spacing_mut().interact_size.y = 23.;
    ui.spacing_mut().button_padding.y = 3.;
    ui.spacing_mut().item_spacing.y = 4.;
    let query_id = ui.id().with("trait-query");
    let mut query = ui
        .data_mut(|m| m.get_temp::<String>(query_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut query)
                .hint_text("模糊搜索词条名称或效果（留空显示全部）"),
        );
        if icon_action(ui, "reset", "清空搜索").clicked() {
            query.clear();
        }
    });
    ui.data_mut(|m| m.insert_temp(query_id, query.clone()));
    let mut traits: Vec<_> = array(&d.get("trait-data")["traits"])
        .iter()
        .filter(|t| crate::search::trait_matches(d, text(t, "key"), &query))
        .collect();
    traits.sort_by_key(|t| {
        (
            crate::search::trait_score(d, text(t, "key"), &query).unwrap_or(u32::MAX),
            std::cmp::Reverse(t["rank"].as_i64().unwrap_or(0)),
        )
    });
    ui.label(format!(
        "已选 {}/4 · {} · {} 项",
        keys.len(),
        if query.trim().is_empty() {
            "按稀有度从高到低"
        } else {
            "按匹配度排列"
        },
        traits.len()
    ));
    ui.horizontal_wrapped(|ui| {
        for key in keys.clone() {
            if trait_remove_tag(ui, d, &key).clicked() {
                keys.retain(|k| k != &key);
            }
        }
    });
    let cols = ((ui.available_width() / 145.) as usize).max(1);
    egui::Grid::new("traits-grid")
        .num_columns(cols)
        .spacing(Vec2::new(6., 5.))
        .show(ui, |ui| {
            for (i, t) in traits.iter().enumerate() {
                let key = text(t, "key");
                let mut selected = keys.iter().any(|k| k == key);
                let enabled = selected || keys.len() < 4;
                if trait_checkbox(ui, d, key, &mut selected, enabled).changed() {
                    keys.retain(|k| k != key);
                    if selected {
                        keys.push(key.into());
                    }
                }
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
}
/// Official square rank emblem, with the same tier colours and downward
/// negative arrows as the original handbook. Paint directly: Frame's minimum
/// widget height must not turn a 15 px square into a tall pill.
pub fn passive_rank(ui: &mut egui::Ui, d: &Data, rank: i64, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint_passive_rank(ui, d, rank, rect);
    response.on_hover_text(format!("词条等级 {rank}"))
}
fn paint_passive_rank(ui: &egui::Ui, d: &Data, rank: i64, rect: egui::Rect) {
    if let Some(uri) = d.get("trait-ranks")[rank.unsigned_abs() as usize]
        .as_str()
        .filter(|uri| !uri.is_empty())
    {
        ui.painter()
            .rect_filled(rect, 3, egui::Color32::from_rgb(51, 59, 75));
        let colour = if rank < 0 {
            egui::Color32::from_rgb(255, 137, 119)
        } else if rank >= 4 {
            egui::Color32::from_rgb(79, 255, 222)
        } else if rank >= 2 {
            egui::Color32::from_rgb(255, 221, 67)
        } else {
            egui::Color32::WHITE
        };
        let mut image = egui::Image::new(d.image_source(uri)).tint(colour);
        if rank < 0 {
            image = image.uv(egui::Rect::from_min_max(
                egui::pos2(0., 1.),
                egui::pos2(1., 0.),
            ));
        }
        image.paint_at(ui, rect.shrink(1.));
    }
}

pub fn trait_remove_tag(ui: &mut egui::Ui, d: &Data, key: &str) -> egui::Response {
    let name = d.trait_name(key);
    let rank = array(&d.get("trait-data")["traits"])
        .iter()
        .find(|t| text(t, "key") == key)
        .and_then(|t| t["rank"].as_i64())
        .unwrap_or_default();
    let palette = crate::design::palette(ui.ctx());
    let galley =
        ui.painter()
            .layout_no_wrap(name.clone(), egui::FontId::proportional(12.), palette.ink);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 49., 24.), egui::Sense::click());
    ui.painter().rect_filled(
        rect,
        5,
        if response.hovered() {
            palette.selected
        } else {
            palette.soft
        },
    );
    ui.painter().rect_stroke(
        rect,
        5,
        egui::Stroke::new(1., palette.line),
        egui::StrokeKind::Inside,
    );
    paint_passive_rank(
        ui,
        d,
        rank,
        egui::Rect::from_center_size(rect.left_center() + Vec2::new(11., 0.), Vec2::splat(15.)),
    );
    ui.painter().galley(
        rect.left_center() + Vec2::new(23., -galley.size().y * 0.5),
        galley,
        palette.ink,
    );
    egui::Image::new(crate::design::icon_source("close"))
        .tint(palette.muted)
        .paint_at(
            ui,
            egui::Rect::from_center_size(
                rect.right_center() - Vec2::new(11., 0.),
                Vec2::splat(12.),
            ),
        );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(format!("移除词条：{name}"))
}

/// Keep the inline work picker and the full picker consistent, including the game's rank art.
pub fn trait_checkbox(
    ui: &mut egui::Ui,
    d: &Data,
    key: &str,
    selected: &mut bool,
    enabled: bool,
) -> egui::Response {
    trait_checkbox_with_suffix(ui, d, key, selected, enabled, "")
}
pub fn trait_checkbox_with_suffix(
    ui: &mut egui::Ui,
    d: &Data,
    key: &str,
    selected: &mut bool,
    enabled: bool,
    suffix: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        let rank = array(&d.get("trait-data")["traits"])
            .iter()
            .find(|t| text(t, "key") == key)
            .and_then(|t| t["rank"].as_i64())
            .unwrap_or_default();
        let emblem = passive_rank(ui, d, rank, 15.).interact(egui::Sense::click());
        track_test_rect(ui, &emblem, &format!("trait-emblem:{key}"));
        let mut response = ui
            .add_enabled(
                enabled,
                egui::Checkbox::new(selected, format!("{}{suffix}", d.trait_name(key))),
            )
            .on_hover_text(text(&d.get("pal-details-data")["passives"][key], "effect"));
        if enabled && emblem.clicked() {
            *selected = !*selected;
            response.mark_changed();
        }
        track_test_rect(ui, &response, &format!("trait:{key}"));
        response
    })
    .inner
}
pub fn trait_picker(ctx: &egui::Context, d: &Data, open: &mut bool, keys: &mut Vec<String>) {
    let mut done = false;
    egui::Window::new("选择词条")
        .title_bar(false)
        .id(egui::Id::new("traits-picker"))
        .open(open)
        .default_width(1120.)
        .default_height(720.)
        .resizable(true)
        .show(ctx, |ui| {
            if crate::design::dialog_header(ui, "layers", "选择词条").clicked() {
                done = true;
            }
            egui::ScrollArea::vertical().show(ui, |ui| trait_grid(ui, d, keys));
            if icon_action(ui, "check", "完成").clicked() {
                done = true;
            }
        });
    if done {
        *open = false;
    }
}
fn track_test_rect(_ui: &egui::Ui, _response: &egui::Response, _name: &str) {
    #[cfg(test)]
    {
        _ui.ctx().data_mut(|m| {
            m.insert_temp(
                egui::Id::new(format!("test-widget:{}", _name)),
                _response.rect,
            )
        });
    }
}
pub struct Card(egui::Frame);
impl Card {
    pub fn inner_margin(mut self, margin: i8) -> Self {
        self.0 = self.0.inner_margin(margin);
        self
    }
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        self.0.show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), body)
                .inner
        })
    }
}
pub fn panel(ui: &egui::Ui) -> Card {
    Card(crate::design::card(ui))
}
fn compact_lookup_pair(
    ui: &mut egui::Ui,
    d: &Data,
    pair: &Value,
    selected: usize,
    reverse: bool,
    width: f32,
    nav: &mut Option<String>,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 40.), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 4, ui.visuals().faint_bg_color);
    let a = pair[0].as_u64().unwrap() as usize;
    let b = pair[1].as_u64().unwrap() as usize;
    let c = pair[2].as_u64().unwrap() as usize;
    let partner = if a == selected { b } else { a };
    let members = if reverse {
        [a, b, c]
    } else {
        [selected, partner, c]
    };
    let count = if reverse { 2 } else { 3 };
    let parent_sides = if reverse || a == selected {
        [0, 1]
    } else {
        [1, 0]
    };
    let special = pair.as_array().is_some_and(|p| p.len() == 5);
    let member = (width - 8. - (count - 1) as f32 * 18.) / count as f32;
    for i in 0..count - 1 {
        egui::Image::new(crate::design::icon_source(if reverse || i == 0 {
            "plus"
        } else {
            "arrow"
        }))
        .tint(ui.visuals().text_color())
        .paint_at(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(
                    rect.left() + 4. + (i + 1) as f32 * member + i as f32 * 18. + 9.,
                    rect.center().y,
                ),
                Vec2::splat(14.),
            ),
        );
    }
    for (i, index) in members.into_iter().take(count).enumerate() {
        let pal = &d.species[index];
        let key = text(pal, "key");
        let left = rect.left() + 4. + i as f32 * (member + 18.);
        let member_rect =
            egui::Rect::from_min_size(egui::pos2(left, rect.top() + 3.), Vec2::new(member, 34.));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(member_rect));
        child.set_clip_rect(ui.clip_rect().intersect(rect));
        child.spacing_mut().interact_size.y = 12.;
        let uri = d.get("portraits")[text(pal, "en")].as_str().unwrap_or("");
        if !uri.is_empty() {
            child.put(
                egui::Rect::from_min_size(egui::pos2(left, rect.top() + 6.), Vec2::splat(28.)),
                egui::Image::new(d.image_source(uri)).fit_to_exact_size(Vec2::splat(28.)),
            );
        }
        let sex = if special && i < 2 {
            let side = parent_sides[i];
            match pair[3 + side].as_str().unwrap_or("") {
                "M" => " ♂",
                "F" => " ♀",
                _ => "",
            }
        } else {
            ""
        };
        let name = format!("{}{sex}", text(pal, "zh"));
        let _name = child
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(egui::Rect::from_min_size(
                        egui::pos2(left + 31., rect.top() + 3.),
                        Vec2::new((member - 31.).max(15.), 17.),
                    ))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&name).size(12.).strong()).truncate(),
                    )
                },
            )
            .inner;
        let link = child
            .interact(
                member_rect,
                child.id().with((a, b, c, i)),
                egui::Sense::click(),
            )
            .on_hover_text(format!("{name} · 查看资料与功能入口"))
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        #[cfg(test)]
        track_test_rect(ui, &link, &format!("lookup-pal:{i}"));
        if link.clicked() {
            *nav = Some(format!("#paldex/{key}"));
        }
        let number = ui.painter().text(
            egui::pos2(left + 31., rect.top() + 23.),
            egui::Align2::LEFT_TOP,
            format!("#{}", text(pal, "dex")),
            egui::FontId::proportional(10.),
            ui.visuals().weak_text_color(),
        );
        if !reverse && i == 2 {
            let mut x = number.right() + 3.;
            let types: Vec<_> = array(&pal["types"])
                .iter()
                .filter_map(Value::as_str)
                .collect();
            let full_badges = rect.right() - 4. - x >= types.len() as f32 * 32. - 3.;
            for kind in types {
                if full_badges {
                    paint_element_badge(
                        ui,
                        kind,
                        egui::Rect::from_min_size(
                            egui::pos2(x, rect.top() + 22.),
                            Vec2::new(29., 14.),
                        ),
                    );
                    x += 32.;
                } else if let Some(source) = crate::elements::source(kind) {
                    egui::Image::new(source).paint_at(
                        ui,
                        egui::Rect::from_min_size(
                            egui::pos2(x, rect.top() + 22.),
                            Vec2::splat(13.),
                        ),
                    );
                    x += 15.;
                }
            }
        }
    }
    if special {
        response.on_hover_text(format!(
            "特殊配方：{} {} ＋ {} {}",
            text(&d.species[a], "zh"),
            sex_label(pair[3].as_str().unwrap_or("")),
            text(&d.species[b], "zh"),
            sex_label(pair[4].as_str().unwrap_or(""))
        ));
    }
}

fn page_size(ui: &mut egui::Ui, id: &str, size: &mut usize, page: &mut usize) {
    ui.label("每页");
    egui::ComboBox::from_id_salt(id)
        .width(66.)
        .selected_text(if *size == 0 {
            "全部".into()
        } else {
            size.to_string()
        })
        .show_ui(ui, |ui| {
            for count in if id == "lookup-size" {
                [30, 60, 120, 0]
            } else {
                [24, 48, 96, 0]
            } {
                if ui
                    .selectable_value(
                        size,
                        count,
                        if count == 0 {
                            "全部".into()
                        } else {
                            count.to_string()
                        },
                    )
                    .changed()
                {
                    *page = 0;
                }
            }
        });
}
fn page_range(total: usize, size: usize, page: &mut usize) -> std::ops::Range<usize> {
    let size = if size == 0 { total.max(1) } else { size };
    *page = (*page).min(total.saturating_sub(1) / size);
    let start = *page * size;
    start..(start + size).min(total)
}
fn pagination(ui: &mut egui::Ui, total: usize, size: usize, page: &mut usize) {
    let range = page_range(total, size, page);
    let pages = if size == 0 {
        1
    } else {
        total.div_ceil(size).max(1)
    };
    ui.horizontal(|ui| {
        ui.small(format!(
            "显示 {}–{} 项，共 {} 项",
            if total == 0 { 0 } else { range.start + 1 },
            range.end,
            total
        ));
        if pages > 1 {
            if ui
                .add_enabled_ui(*page > 0, |ui| icon_button(ui, "first", "第一页"))
                .inner
                .on_hover_text("第一页")
                .clicked()
            {
                *page = 0;
            }
            if ui
                .add_enabled_ui(*page > 0, |ui| icon_button(ui, "left", "上一页"))
                .inner
                .on_hover_text("上一页")
                .clicked()
            {
                *page -= 1;
            }
            ui.label(format!("{} / {pages}", *page + 1));
            if ui
                .add_enabled_ui(*page + 1 < pages, |ui| icon_button(ui, "right", "下一页"))
                .inner
                .on_hover_text("下一页")
                .clicked()
            {
                *page += 1;
            }
            if ui
                .add_enabled_ui(*page + 1 < pages, |ui| icon_button(ui, "last", "最后一页"))
                .inner
                .on_hover_text("最后一页")
                .clicked()
            {
                *page = pages - 1;
            }
        }
    });
}
pub fn element_color(key: &str) -> egui::Color32 {
    match key {
        "fire" => egui::Color32::from_rgb(186, 75, 45),
        "water" => egui::Color32::from_rgb(42, 118, 162),
        "grass" => egui::Color32::from_rgb(62, 117, 54),
        "electric" => egui::Color32::from_rgb(141, 106, 26),
        "ice" => egui::Color32::from_rgb(37, 128, 139),
        "ground" => egui::Color32::from_rgb(141, 91, 51),
        "dark" => egui::Color32::from_rgb(113, 78, 149),
        "dragon" => egui::Color32::from_rgb(126, 75, 163),
        _ => egui::Color32::from_rgb(128, 113, 71),
    }
}
fn paint_element_badge(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    let color = element_color(key);
    ui.painter()
        .rect_filled(rect, 3, color.gamma_multiply(0.12));
    if let Some(source) = crate::elements::source(key) {
        egui::Image::new(source).paint_at(
            ui,
            egui::Rect::from_center_size(rect.left_center() + Vec2::new(8., 0.), Vec2::splat(14.)),
        );
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(17., 0.),
        egui::Align2::LEFT_CENTER,
        element_label(key),
        egui::FontId::proportional(11.),
        color,
    );
}
pub fn element_badges(ui: &mut egui::Ui, types: &Value) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.;
        for t in array(types).iter().filter_map(Value::as_str) {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(33., 18.), egui::Sense::hover());
            paint_element_badge(ui, t, rect);
            response.on_hover_text(format!("{}属性", element_label(t)));
        }
    });
}
pub fn trait_badge(ui: &mut egui::Ui, d: &Data, key: &str) -> egui::Response {
    let rank = array(&d.get("trait-data")["traits"])
        .iter()
        .find(|t| text(t, "key") == key)
        .and_then(|t| t["rank"].as_i64())
        .unwrap_or(0);
    let color = if rank < 0 {
        egui::Color32::from_rgb(175, 86, 99)
    } else if rank >= 4 {
        egui::Color32::from_rgb(136, 91, 179)
    } else if rank >= 3 {
        egui::Color32::from_rgb(158, 119, 38)
    } else {
        ui.visuals().text_color()
    };
    let inner = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.;
        passive_rank(ui, d, rank, 15.);
        ui.add(egui::Link::new(
            egui::RichText::new(d.trait_name(key))
                .size(12.)
                .color(color)
                .background_color(color.gamma_multiply(0.10)),
        ))
        .on_hover_text(text(&d.get("pal-details-data")["passives"][key], "effect"))
    });
    ui.interact(
        inner.response.rect,
        ui.id().with(("trait-badge", key)),
        egui::Sense::click(),
    )
    .union(inner.inner)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn new_record(key: &str) -> Value {
    json!({"id":uuid::Uuid::new_v4().to_string(),"speciesKey":key,"name":"","sex":"unknown","traits":[],"legacy":false})
}
fn record_species_name(d: &Data, record: &Value) -> String {
    let game_key = text(record, "terminalSpeciesKey").to_lowercase();
    d.get("trait-data")["terminalNames"][&game_key]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| d.pal_name(text(record, "speciesKey")))
}
fn sex_label(s: &str) -> &str {
    match s {
        "M" => "♂ 雄性",
        "F" => "♀ 雌性",
        _ => "性别未知",
    }
}
fn element_label(key: &str) -> &str {
    match key {
        "neutral" => "无",
        "fire" => "火",
        "water" => "水",
        "grass" => "草",
        "electric" => "雷",
        "ice" => "冰",
        "ground" => "地",
        "dark" => "暗",
        "dragon" => "龙",
        _ => key,
    }
}
pub fn element_picker(ui: &mut egui::Ui, current: &mut String) {
    egui::ComboBox::from_id_salt("element")
        .selected_text(if current.is_empty() {
            "全部属性"
        } else {
            element_label(current)
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(current, String::new(), "全部属性");
            for (key, label) in [
                ("neutral", "无"),
                ("fire", "火"),
                ("water", "水"),
                ("grass", "草"),
                ("electric", "雷"),
                ("ice", "冰"),
                ("ground", "地"),
                ("dark", "暗"),
                ("dragon", "龙"),
            ] {
                ui.selectable_value(current, key.into(), label);
            }
        });
}
fn render_plan(
    ui: &mut egui::Ui,
    d: &Data,
    result: &Value,
    world: &Value,
    nav: &mut Option<String>,
) {
    if result["conditional"] == true {
        ui.label("此路线包含随机孵出的词条，成功率并非保证。");
    }
    if result["conditionalSex"] == true {
        ui.label("请先核对路线标注的未知个体性别。");
    }
    if result["minimumUnconfirmed"] == true {
        ui.label("找到可用路线，但尚未确认是最少代数。");
    }
    if result["hatchUnavailable"] == true {
        ui.label("已有符合目标的个体，但现有仓库找不到额外孵化路线。");
    }
    if text(result, "status") == "unconfirmed-sex" {
        ui.label("未知性别组合较多，暂不能确认可达性。请记录性别后重算。");
    }
    for step in array(&result["steps"]) {
        ui.group(|ui| {
            ui.strong(format!("第 {} 代", step["depth"]));
            ui.horizontal_wrapped(|ui| {
                for (i, parent) in array(&step["parents"]).iter().enumerate() {
                    if i > 0 {
                        crate::design::icon(ui, "plus", 16.);
                    }
                    if let Some(p) = parent["species"]
                        .as_u64()
                        .and_then(|i| d.species.get(i as usize))
                    {
                        if d.portrait_link(ui, text(p, "key"), 32.) {
                            *nav = Some(format!("#paldex/{}", text(p, "key")));
                        }
                        if ui
                            .link(format!(
                                "{} {}",
                                text(p, "zh"),
                                sex_label(text(parent, "sex"))
                            ))
                            .clicked()
                        {
                            *nav = Some(format!("#paldex/{}", text(p, "key")));
                        }
                        if let Some(founder) = array(&result["founders"])
                            .iter()
                            .find(|f| f["id"] == parent["id"])
                        {
                            let id = text(founder, "stockId");
                            let name = array(&world["inventory"]["records"])
                                .iter()
                                .find(|r| text(r, "id") == id)
                                .map(|r| text(r, "name"))
                                .filter(|n| !n.is_empty());
                            let short: String = id
                                .chars()
                                .rev()
                                .take(6)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect();
                            ui.small(format!("仓库 · {}", name.unwrap_or(&short)))
                                .on_hover_text(id);
                        }
                    }
                }
                crate::design::icon(ui, "arrow", 16.);
                if let Some(p) = step["species"]
                    .as_u64()
                    .and_then(|i| d.species.get(i as usize))
                {
                    if d.portrait_link(ui, text(p, "key"), 36.) {
                        *nav = Some(format!("#paldex/{}", text(p, "key")));
                    }
                    if ui.link(text(p, "zh")).clicked() {
                        *nav = Some(format!("#paldex/{}", text(p, "key")));
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("保留词条：");
                for t in array(&step["traits"]) {
                    if trait_badge(ui, d, t.as_str().unwrap_or("")).clicked() {
                        *nav = Some(format!(
                            "#details/passive/trait/{}",
                            t.as_str().unwrap_or("")
                        ));
                    }
                }
                if !array(&step["sexes"]).is_empty() {
                    ui.label(format!(
                        "后续需要性别：{}",
                        array(&step["sexes"])
                            .iter()
                            .filter_map(Value::as_str)
                            .map(sex_label)
                            .collect::<Vec<_>>()
                            .join("、")
                    ));
                }
            });
            if !array(&step["randomTraits"]).is_empty() {
                ui.label(format!(
                    "此步随机孵出：{}",
                    array(&step["randomTraits"])
                        .iter()
                        .map(|v| d.trait_name(v.as_str().unwrap_or("")))
                        .collect::<Vec<_>>()
                        .join("、")
                ));
            }
        });
    }
    if !array(&result["surgery"]).is_empty() {
        ui.strong("完成配种后使用手术台");
        for op in array(&result["surgery"]) {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} · {} 金币",
                    d.trait_name(text(op, "key")),
                    op["cost"]
                ));
                if ui.link(d.item_name(text(op, "item"))).clicked() {
                    *nav = Some(format!("#items/{}", text(op, "item")));
                }
                ui.label(if op["mode"] == "consume" {
                    "消耗植入体"
                } else if op["mode"] == "coins" {
                    "金币手术"
                } else {
                    "已解锁手术"
                });
            });
        }
    }
    if !array(&result["founders"]).is_empty() {
        ui.add_space(8.);
        ui.strong("需要的仓库个体");
        for founder in array(&result["founders"]) {
            if let Some(p) = founder["species"]
                .as_u64()
                .and_then(|i| d.species.get(i as usize))
            {
                let source = array(&world["inventory"]["records"])
                    .iter()
                    .find(|r| r["id"] == founder["stockId"]);
                panel(ui).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if d.portrait_link(ui, text(p, "key"), 36.) {
                            *nav = Some(format!("#paldex/{}", text(p, "key")));
                        }
                        if ui.link(text(p, "zh")).clicked() {
                            *nav = Some(format!("#paldex/{}", text(p, "key")));
                        }
                        ui.label(source.map(|r| text(r, "name")).unwrap_or("仓库个体"));
                        ui.label(if !text(founder, "assumedSex").is_empty() {
                            format!("须确认是{}", sex_label(text(founder, "assumedSex")))
                        } else {
                            sex_label(text(founder, "sex")).into()
                        });
                        for key in array(&founder["requiredTraits"])
                            .iter()
                            .filter_map(Value::as_str)
                        {
                            if trait_badge(ui, d, key).clicked() {
                                *nav = Some(format!("#details/passive/trait/{key}"));
                            }
                        }
                    });
                });
            }
        }
    }
    if !array(&result["missing"]).is_empty() {
        ui.strong("缺少词条来源");
        ui.horizontal_wrapped(|ui| {
            for key in array(&result["missing"]).iter().filter_map(Value::as_str) {
                if trait_badge(ui, d, key).clicked() {
                    *nav = Some(format!("#details/passive/trait/{key}"));
                }
            }
        });
        ui.label("补录携带这些词条的个体，或允许随机词条、使用已经拥有的手术台词条后重新计算。");
    }
    match text(result, "status") {
        "unreachable-species" => {
            ui.label("请检查亲代的雌雄性别，或加入能通往目标物种的亲代。");
        }
        "unreachable-traits" => {
            ui.label("目标物种可以获得，但现有亲代无法把这些词条集中到目标上。可以补入异性伙伴或其他携带者。");
        }
        _ => {}
    }
}
fn render_log(ui: &mut egui::Ui, d: &Data, log: &Value, nav: &mut Option<String>) {
    if array(log).is_empty() {
        ui.label("尚未登记实际结果。");
    }
    for (i, event) in array(log).iter().enumerate() {
        panel(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                if d.portrait_link(ui, text(event, "speciesKey"), 40.) {
                    *nav = Some(format!("#paldex/{}", text(event, "speciesKey")));
                }
                ui.vertical(|ui| {
                    ui.strong(format!(
                        "{}. {} · {}",
                        i + 1,
                        if event["kind"] == "surgery" {
                            "手术"
                        } else {
                            "孵化"
                        },
                        d.pal_name(text(event, "speciesKey"))
                    ));
                    ui.label(format!(
                        "{} · {}",
                        text(event, "name"),
                        sex_label(text(event, "sex"))
                    ));
                });
            });
            ui.horizontal_wrapped(|ui| {
                for key in array(&event["traits"]).iter().filter_map(Value::as_str) {
                    if trait_badge(ui, d, key).clicked() {
                        *nav = Some(format!("#details/passive/trait/{key}"));
                    }
                }
            });
            let missing: Vec<_> = array(&event["expectedTraits"])
                .iter()
                .filter(|t| !array(&event["traits"]).contains(t))
                .filter_map(Value::as_str)
                .map(|t| d.trait_name(t))
                .collect();
            if !missing.is_empty() {
                ui.small(format!("还缺词条：{}", missing.join("、")));
            }
            if !array(&event["expectedSexes"]).is_empty()
                && !array(&event["expectedSexes"]).contains(&event["sex"])
            {
                ui.small("实际性别与本步要求不同，请继续保留合适的个体。");
            }
        });
    }
}
fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn stable_route(d: &Data, result: &Value, world: &Value) -> Value {
    let steps:Vec<_>=array(&result["steps"]).iter().filter_map(|step|{
        let species=d.species.get(step["species"].as_u64()?as usize)?;let parents:Vec<_>=array(&step["parents"]).iter().filter_map(|p|{
            let species=d.species.get(p["species"].as_u64()?as usize)?;let founder=array(&result["founders"]).iter().find(|f|f["id"]==p["id"]);let stock_id=founder.map(|f|f["stockId"].clone()).unwrap_or(Value::Null);let name=array(&world["inventory"]["records"]).iter().find(|r|r["id"]==stock_id).map(|r|text(r,"name")).unwrap_or("");
            Some(json!({"speciesKey":species["key"],"sex":p["sex"],"stockId":stock_id,"name":name}))
        }).collect();Some(json!({"speciesKey":species["key"],"generation":step["depth"],"traits":step["traits"],"sexes":step["sexes"],"parents":parents}))
    }).collect();
    json!({"generations":result["generations"],"steps":steps,"surgery":array(&result["surgery"]).iter().map(|o|o["key"].clone()).collect::<Vec<_>>()})
}

// Records and plan progress share one durable commit. Old interrupted events are replayed by stable IDs.
fn apply_event(world: &mut Value, plan_id: &str, mut event: Value) -> Result<(), String> {
    let plan_index = array(&world["plans"])
        .iter()
        .position(|p| p["id"] == plan_id)
        .ok_or("配种计划已删除。")?;
    let id = text(&event, "id").to_owned();
    let stock_id = text(&event, "stockId").to_owned();
    if id.is_empty()
        || stock_id.is_empty()
        || !matches!(event["kind"].as_str(), Some("hatch" | "surgery"))
    {
        return Err("执行记录格式无效。".into());
    }
    if array(&world["plans"][plan_index]["log"])
        .iter()
        .any(|entry| entry["id"] == id)
    {
        world["plans"][plan_index]["pending"] = Value::Null;
        return Ok(());
    }
    if event["kind"] == "hatch" && stock_id != format!("hatch-{id}") {
        return Err("孵化记录与个体编号不一致。".into());
    }
    let existing = array(&world["inventory"]["records"])
        .iter()
        .position(|r| r["id"] == stock_id);
    if let Some(i) = existing {
        let row = &mut world["inventory"]["records"][i];
        if row["speciesKey"] != event["speciesKey"] {
            return Err("执行记录与仓库个体物种不匹配。".into());
        }
        if event["kind"] == "surgery" {
            for key in ["name", "sex", "traits"] {
                row[key] = event[key].clone();
            }
        } else {
            event["name"] = row["name"].clone();
        }
    } else if event["kind"] == "hatch"
        && !array(&world["inventory"]["removed"])
            .iter()
            .any(|id| id.as_str() == Some(&stock_id))
        && !array(&world["inventory"]["archived"])
            .iter()
            .any(|r| r["id"] == stock_id)
    {
        world["inventory"]["records"].as_array_mut().unwrap().push(json!({"id":stock_id,"speciesKey":event["speciesKey"],"name":event["name"],"sex":event["sex"],"traits":event["traits"],"legacy":false}));
    }
    world["plans"][plan_index]["log"]
        .as_array_mut()
        .unwrap()
        .push(event);
    world["plans"][plan_index]["pending"] = Value::Null;
    Ok(())
}
fn settle_world(world: &mut Value) -> Result<usize, String> {
    let events: Vec<_> = array(&world["plans"])
        .iter()
        .filter(|p| !p["pending"].is_null())
        .map(|p| (text(p, "id").to_owned(), p["pending"].clone()))
        .collect();
    let count = events.len();
    for (id, event) in events {
        apply_event(world, &id, event)?;
    }
    Ok(count)
}
fn settle_pending(store: &mut Store) -> Result<usize, String> {
    let mut next = store.state.clone();
    let i = store.world_index();
    let n = settle_world(&mut next["worlds"][i])?;
    if n > 0 {
        store.commit(next)?;
    }
    Ok(n)
}
fn record_event(
    store: &mut Store,
    world_id: &str,
    plan_id: &str,
    event: Value,
) -> Result<(), String> {
    if text(store.world(), "id") != world_id {
        return Err("世界已切换，请在当前世界重新打开计划。".into());
    }
    let mut next = store.state.clone();
    let i = store.world_index();
    settle_world(&mut next["worlds"][i])?;
    apply_event(&mut next["worlds"][i], plan_id, event)?;
    store.commit(next)
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::store::tests::{Temp, record};
    fn event() -> Value {
        json!({"id":"event-one","stockId":"hatch-event-one","kind":"hatch","at":1,"speciesKey":"SheepBall","generation":1,"name":"实际幼崽","sex":"F","traits":["Legend"],"expectedTraits":["Legend"],"expectedSexes":["F"],"parents":[{"speciesKey":"SheepBall","stockId":"a","name":"A","sex":"M"},{"speciesKey":"SheepBall","stockId":"b","name":"B","sex":"F"}]})
    }
    fn plan() -> Value {
        json!({"id":"plan-one","created":1,"targetKey":"SheepBall","traits":["Legend"],"random":false,"surgery":true,"initialRoute":null,"log":[],"pending":null})
    }
    fn store(temp: &Temp) -> Store {
        let mut s = Store::open(temp.0.clone());
        s.world_mut()["plans"] = json!([plan()]);
        s
    }
    fn ui_fixture() -> (Data, Temp) {
        let temp = Temp::new();
        for (name, value) in [
            (
                "db",
                json!({"species":[{"key":"SheepBall","zh":"棉悠悠","dex":"001"}]}),
            ),
            ("pairs", json!([[0, 0, 0]])),
            ("portraits", json!({})),
            ("trait-ranks", json!({})),
            (
                "trait-data",
                json!({"traits":[{"key":"Legend","zh":"传说","rank":4},{"key":"Artisan","zh":"工匠","rank":3}]}),
            ),
        ] {
            std::fs::write(
                temp.0.join(format!("{name}.json")),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();
        }
        let data = Data::new(temp.0.clone(), egui::Context::default()).unwrap();
        (data, temp)
    }
    #[test]
    fn pair_parent_and_child_identity_clicks_all_open_the_overview() {
        let (mut d, _temp) = ui_fixture();
        d.species.extend([
            json!({"key":"PinkCat","zh":"捣蛋猫","dex":"002"}),
            json!({"key":"Ganesha","zh":"壶小象","dex":"011"}),
        ]);
        let ctx = egui::Context::default();
        let pair = json!([0, 1, 2]);
        let render = |events: Vec<egui::Event>| {
            let mut nav = None;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(600., 160.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        compact_lookup_pair(ui, &d, &pair, 0, false, 500., &mut nav)
                    });
                },
            );
            nav
        };
        for (i, key) in ["SheepBall", "PinkCat", "Ganesha"].iter().enumerate() {
            render(vec![]);
            render(vec![]);
            let rect = ctx
                .data(|m| {
                    m.get_temp::<egui::Rect>(egui::Id::new(format!("test-widget:lookup-pal:{i}")))
                })
                .unwrap();
            // Both avatar and name belong to this same identity target.
            for pos in [egui::pos2(rect.left()+14.,rect.center().y), egui::pos2(rect.left()+50.,rect.top()+8.)] {
                let event = |pressed| {
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ]
                };
                render(event(true));
                assert_eq!(render(event(false)), Some(format!("#paldex/{key}")));
            }
        }
    }
    #[test]
    fn pagination_keeps_all_records_accessible_after_filtering() {
        let mut page = 0;
        assert_eq!(page_range(467, 24, &mut page), 0..24);
        page = 19;
        assert_eq!(page_range(467, 24, &mut page), 456..467);
        assert_eq!(page_range(10, 24, &mut page), 0..10);
        page = 15;
        assert_eq!(page_range(467, 0, &mut page), 0..467);
        assert_eq!(page, 0);
    }
    #[test]
    fn warehouse_cards_fit_virtual_row_with_four_traits_and_long_nickname() {
        let (d, temp) = ui_fixture();
        let mut store = Store::open(temp.0.join("profile"));
        store.world_mut()["inventory"]["records"] = json!([
            {"id":"one","speciesKey":"SheepBall","name":"这是游戏内一个很长很长的自定义昵称","sex":"M","traits":["Legend","Artisan","Legend","Artisan"]},
            {"id":"two","speciesKey":"SheepBall","name":"另一只","sex":"F","traits":[]}
        ]);
        let ctx = egui::Context::default();
        crate::design::apply(&ctx, "moon");
        let mut breeding = Breeding::default();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(360., 900.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    breeding.warehouse_content(ui, &d, &mut store);
                });
            },
        );
        let rect = ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new("test-widget:stock-card:one")))
            .unwrap();
        assert!(
            rect.height() <= 232. + 1.,
            "card {} exceeds the virtual row",
            rect.height()
        );
    }
    #[test]
    fn rank_emblems_are_square_tinted_and_negative_arrows_point_down() {
        let (mut d, temp) = ui_fixture();
        std::fs::create_dir_all(temp.0.join("assets")).unwrap();
        let real = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/assets/passive-rank-1.webp");
        std::fs::copy(real, temp.0.join("assets/rank.webp")).unwrap();
        d.tables.insert(
            "trait-ranks".into(),
            std::sync::Arc::new(json!([
                "",
                "assets/rank.webp",
                "assets/rank.webp",
                "assets/rank.webp",
                "assets/rank.webp"
            ])),
        );
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        crate::design::apply(&ctx, "moon");
        let uri = d.image_uri("assets/rank.webp");
        let start = std::time::Instant::now();
        let mut texture = None;
        while texture.is_none() && start.elapsed().as_secs() < 5 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                if let Ok(egui::load::TexturePoll::Ready { texture: ready }) = ctx.try_load_texture(
                    &uri,
                    egui::TextureOptions::LINEAR,
                    egui::load::SizeHint::default(),
                ) {
                    texture = Some(ready.id);
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        texture.expect("real rank texture should load");
        let mut bounds = Vec::new();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300., 100.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for rank in [1, 2, 4, -1] {
                            bounds.push(passive_rank(ui, &d, rank, 15.).rect);
                        }
                    });
                });
            },
        );
        assert!(
            bounds.iter().all(|r| r.size() == Vec2::splat(15.)),
            "rank background must not expand with minimum widget height"
        );
        // egui 0.33 paints unrotated images as textured RectShape values;
        // Mesh is only used for rotated images. The image widget can also use
        // a different texture sampler than preloading, so inspect its real draw
        // rather than assuming the preload's texture ID is the same.
        let images: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.brush.is_some() => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(images.len(), 4);
        assert!(
            images
                .iter()
                .all(|r| r.brush.as_ref().unwrap().fill_texture_id
                    == images[0].brush.as_ref().unwrap().fill_texture_id)
        );
        assert!(images.iter().all(|r| r.rect.size() == Vec2::splat(13.)));
        assert_eq!(images[0].brush.as_ref().unwrap().uv.min.y, 0.);
        assert_eq!(images[3].brush.as_ref().unwrap().uv.min.y, 1.);
        assert_eq!(images[3].brush.as_ref().unwrap().uv.max.y, 0.);
        assert_ne!(images[0].fill, images[1].fill);
        assert_ne!(images[1].fill, images[2].fill);
        assert_ne!(images[2].fill, images[3].fill);
    }
    #[test]
    fn trait_emblem_click_toggles_the_same_selection_as_its_checkbox() {
        let (d, _temp) = ui_fixture();
        let ctx = egui::Context::default();
        let mut keys = Vec::new();
        let render = |events: Vec<egui::Event>, keys: &mut Vec<String>| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700., 500.),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| trait_grid(ui, &d, keys));
                },
            )
        };
        let _ = render(vec![], &mut keys);
        let _ = render(vec![], &mut keys);
        let rect = ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new("test-widget:trait-emblem:Legend")))
            .unwrap();
        for expected in [true, false] {
            for pressed in [true, false] {
                let _ = render(
                    vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    &mut keys,
                );
            }
            assert_eq!(keys.iter().any(|key| key == "Legend"), expected);
        }
    }
    #[test]
    fn trait_checkboxes_can_select_and_remove_by_pointer() {
        let (d, _temp) = ui_fixture();
        let ctx = egui::Context::default();
        let mut keys = vec![];
        let frame = |ctx: &egui::Context, keys: &mut Vec<String>, events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900., 650.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| trait_grid(ui, &d, keys));
                },
            );
        };
        frame(&ctx, &mut keys, vec![]);
        let rect = ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new("test-widget:trait:Legend")))
            .unwrap();
        let pos = rect.center();
        for expected in [1, 0] {
            frame(
                &ctx,
                &mut keys,
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
            frame(
                &ctx,
                &mut keys,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            assert_eq!(keys.len(), expected);
        }
    }
    #[test]
    fn target_trait_click_invalidates_route_and_discards_old_worker() {
        let (d, temp) = ui_fixture();
        let mut store = Store::open(temp.0.join("profile"));
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut breeding = Breeding::default();
        breeding.picker = true;
        let frame = |breeding: &mut Breeding, store: &mut Store, events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        breeding.planner_content(ui, &d, store, false);
                    });
                },
            );
        };
        frame(&mut breeding, &mut store, vec![]);
        frame(&mut breeding, &mut store, vec![]);
        let pos = ctx
            .data(|m| m.get_temp::<egui::Rect>(egui::Id::new("test-widget:trait:Legend")))
            .unwrap()
            .center();
        frame(
            &mut breeding,
            &mut store,
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
        let old_epoch = breeding.epoch;
        let old_result = json!({"status":"ok","steps":[],"founders":[]});
        let (sender, receiver) = mpsc::channel();
        breeding.receiver = Some(receiver);
        breeding.busy = true;
        breeding.result = old_result.clone();
        frame(
            &mut breeding,
            &mut store,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(breeding.traits, vec!["Legend"]);
        assert!(
            breeding.result.is_null(),
            "the old target's route must disappear on the click frame"
        );
        assert!(breeding.epoch > old_epoch);
        assert!(!breeding.busy);
        assert!(
            sender.send((old_epoch, old_result, 1.)).is_err(),
            "a previous solver cannot restore its route"
        );
    }
    #[test]
    fn interrupted_hatch_is_restored_once_and_tombstones_are_respected() {
        let temp = Temp::new();
        let mut s = store(&temp);
        s.world_mut()["plans"][0]["pending"] = event();
        s.flush();
        assert!(s.error.is_none());
        let mut s = Store::open(temp.0.clone());
        assert_eq!(settle_pending(&mut s).unwrap(), 1);
        assert_eq!(array(&s.world()["inventory"]["records"]).len(), 1);
        assert_eq!(array(&s.world()["plans"][0]["log"]).len(), 1);
        record_event(&mut s, "default", "plan-one", event()).unwrap();
        assert_eq!(array(&s.world()["inventory"]["records"]).len(), 1);
        assert_eq!(array(&s.world()["plans"][0]["log"]).len(), 1);
        let mut w = Store::new_world("tombstone", "删除过的");
        w["plans"] = json!([plan()]);
        w["plans"][0]["pending"] = event();
        w["inventory"]["removed"] = json!(["hatch-event-one"]);
        settle_world(&mut w).unwrap();
        assert!(array(&w["inventory"]["records"]).is_empty());
        assert_eq!(array(&w["plans"][0]["log"]).len(), 1);
    }
    #[test]
    fn surgery_changes_existing_individual_and_deduplicates_log() {
        let temp = Temp::new();
        let mut s = store(&temp);
        s.world_mut()["inventory"]["records"] = json!([record("existing")]);
        let mut op = event();
        op["id"] = json!("operation");
        op["kind"] = json!("surgery");
        op["stockId"] = json!("existing");
        op["generation"] = json!(0);
        op["parents"] = json!([]);
        op["traits"] = json!(["Legend", "Rare"]);
        record_event(&mut s, "default", "plan-one", op.clone()).unwrap();
        assert_eq!(s.world()["inventory"]["records"][0]["traits"], op["traits"]);
        record_event(&mut s, "default", "plan-one", op).unwrap();
        assert_eq!(array(&s.world()["plans"][0]["log"]).len(), 1);
        assert_eq!(array(&s.world()["inventory"]["records"]).len(), 1);
    }
    #[test]
    fn wrong_world_species_or_disk_failure_never_partially_updates_progress() {
        let temp = Temp::new();
        let mut s = store(&temp);
        let before = s.state.clone();
        assert!(record_event(&mut s, "other", "plan-one", event()).is_err());
        assert_eq!(s.state, before);
        let mut wrong = record("hatch-event-one");
        wrong["speciesKey"] = json!("PinkCat");
        s.world_mut()["inventory"]["records"] = json!([wrong]);
        let before = s.state.clone();
        assert!(record_event(&mut s, "default", "plan-one", event()).is_err());
        assert_eq!(s.state, before);
        let blocked = temp.0.join("blocked");
        std::fs::write(&blocked, b"blocked").unwrap();
        s.root = blocked;
        s.world_mut()["inventory"]["records"] = json!([]);
        let before = s.state.clone();
        assert!(record_event(&mut s, "default", "plan-one", event()).is_err());
        assert_eq!(s.state, before);
    }
}
