use crate::{
    catalog::render_value,
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
            query: String::new(),
            filter: String::new(),
            reverse: false,
            element: String::new(),
            traits: vec![],
            picker: false,
            random: false,
            surgery: true,
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
            new_only: false,
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
        ui.horizontal(|ui| {
            pal_picker(ui, d, &mut self.lookup_pal, &mut self.lookup_query);
            ui.selectable_value(&mut self.reverse, false, "查配种伙伴");
            ui.selectable_value(&mut self.reverse, true, "反查双亲");
        });
        let p = &d.species[self.lookup_pal];
        let key = text(p, "key");
        ui.horizontal(|ui| {
            d.portrait(ui, key, 62.);
            ui.heading(text(p, "zh"));
            if ui.link("图鉴").clicked() {
                nav = Some(format!("#paldex/{key}"));
            }
            if ui.link("刷新位置").clicked() {
                nav = Some(format!("#spawns/{key}"));
            }
            if ui.button("＋ 加入仓库").clicked() {
                self.edit = Some(new_record(key));
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.lookup_filter).hint_text("筛选伙伴或子代"));
            element_picker(ui, &mut self.lookup_element);
        });
        let rows: Vec<_> = d
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
                let a = &d.species[r[0].as_u64().unwrap() as usize];
                let b = &d.species[r[1].as_u64().unwrap() as usize];
                let c = &d.species[r[2].as_u64().unwrap() as usize];
                (self.lookup_element.is_empty()
                    || array(&c["types"])
                        .iter()
                        .any(|x| x.as_str() == Some(&self.lookup_element)))
                    && (self.lookup_filter.is_empty()
                        || [a, b, c].iter().any(|p| search(p, &self.lookup_filter)))
            })
            .collect();
        ui.label(format!("{} 个组合", rows.len()));
        let cols = ((ui.available_width() / 430.) as usize).clamp(1, 3);
        let count = rows.len().div_ceil(cols);
        egui::ScrollArea::vertical()
            .id_salt("pair-results")
            .show_rows(ui, 66., count, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        for col in 0..cols {
                            let Some(r) = rows.get(row * cols + col) else {
                                break;
                            };
                            ui.allocate_ui(
                                Vec2::new(
                                    (ui.available_width() / ((cols - col) as f32)).max(200.),
                                    60.,
                                ),
                                |ui| {
                                    ui.horizontal(|ui| {
                                        let a = r[0].as_u64().unwrap() as usize;
                                        let b = r[1].as_u64().unwrap() as usize;
                                        let c = r[2].as_u64().unwrap() as usize;
                                        for (i, index) in if self.reverse {
                                            vec![a, b]
                                        } else {
                                            vec![if a == self.lookup_pal { b } else { a }, c]
                                        }
                                        .iter()
                                        .enumerate()
                                        {
                                            if i == 1 {
                                                ui.label(if self.reverse { "＋" } else { "→" });
                                            }
                                            let p = &d.species[*index];
                                            d.portrait(ui, text(p, "key"), 36.);
                                            if ui.link(text(p, "zh")).clicked() {
                                                self.lookup_pal = *index;
                                            }
                                        }
                                        if array(r).len() > 3 {
                                            ui.small("指定亲代性别");
                                        }
                                    });
                                },
                            );
                        }
                    });
                }
            });
        self.edit_dialog(ui.ctx(), d, store);
        nav
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
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.stock_filter)
                    .hint_text("搜索名字、物种、词条"),
            );
            if ui.button("＋ 添加个体").clicked() {
                self.edit = Some(new_record(text(&d.species[self.pal], "key")));
            }
        });
        let records: Vec<Value> = array(&store.world()["inventory"]["records"])
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
        ui.label(format!("{} 只个体 · 同种帕鲁分别保留", records.len()));
        let cols = ((ui.available_width() / 240.) as usize).clamp(1, 5);
        let mut remove = None;
        egui::ScrollArea::vertical()
            .id_salt("warehouse-cards")
            .max_height(460.)
            .show_rows(ui, 174., records.len().div_ceil(cols), |ui, range| {
                for row in range {
                    ui.horizontal_top(|ui| {
                        for col in 0..cols {
                            let Some(p) = records.get(row * cols + col) else {
                                break;
                            };
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.set_width(210.);
                                ui.horizontal(|ui| {
                                    d.portrait(ui, text(p, "speciesKey"), 54.);
                                    ui.vertical(|ui| {
                                        ui.strong(if text(p, "name").is_empty() {
                                            d.pal_name(text(p, "speciesKey"))
                                        } else {
                                            text(p, "name").to_owned()
                                        });
                                        if ui.link(d.pal_name(text(p, "speciesKey"))).clicked() {
                                            nav =
                                                Some(format!("#paldex/{}", text(p, "speciesKey")));
                                        }
                                        ui.small(sex_label(text(p, "sex")));
                                    });
                                });
                                ui.horizontal_wrapped(|ui| {
                                    for t in array(&p["traits"]) {
                                        let key = t.as_str().unwrap_or("");
                                        if ui.link(d.trait_name(key)).clicked() {
                                            nav = Some(format!("#details/passive/trait/{key}"));
                                        }
                                    }
                                });
                                ui.horizontal_wrapped(|ui| {
                                    if ui.small_button("编辑").clicked() {
                                        self.edit = Some(p.clone());
                                    }
                                    if ui.small_button("添加同种").clicked() {
                                        self.edit = Some(new_record(text(p, "speciesKey")));
                                    }
                                    if ui.small_button("移出仓库").clicked() {
                                        remove = Some(text(p, "id").to_owned());
                                    }
                                });
                            });
                        }
                    });
                }
            });
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
                    if p["terminalArchived"] != true && ui.small_button("恢复手工记录").clicked()
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
            if ui.button("添加已识别的个体").clicked() {
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
        if !offspring {
            ui.horizontal(|ui| {
                if pal_picker(ui, d, &mut self.pal, &mut self.query) {
                    self.active_plan.clear();
                    self.invalidate();
                }
                if ui
                    .button(format!("选择目标词条（{}/4）", self.traits.len()))
                    .clicked()
                {
                    self.picker = true;
                }
            });
            ui.horizontal_wrapped(|ui| {
                for t in self.traits.clone() {
                    if ui.button(format!("{} ×", d.trait_name(&t))).clicked() {
                        self.traits.retain(|x| x != &t);
                        self.invalidate();
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.random, "允许随机孵出词条").changed() {
                    self.invalidate();
                }
                if ui
                    .checkbox(&mut self.surgery, "使用已拥有的手术台词条")
                    .changed()
                {
                    self.invalidate();
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label("最多配种代数");
                let mut changed = ui.checkbox(&mut self.all_steps, "不限步数").changed();
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
            .add_enabled(
                !self.busy && data_ready,
                egui::Button::new(if offspring {
                    "计算可配出的帕鲁"
                } else {
                    "计算配种路径"
                }),
            )
            .clicked()
            || qa_run
        {
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
            let ctx = ui.ctx().clone();
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
        if !data_ready {
            ui.label("正在准备手术台词条资料，完成后即可计算。");
        }
        if self.busy {
            ui.spinner();
            ui.label("正在计算，可继续使用其他页面。");
        }
        if !self.result.is_null() {
            ui.small(format!("计算耗时 {:.2} ms", self.latency));
            if offspring {
                self.offspring_results(ui, d, store, &mut nav);
            } else {
                let status = text(&self.result, "status");
                ui.heading(match status {
                    "ok" => "找到配种路径",
                    "owned" => "仓库已有符合条件的帕鲁",
                    "missing-traits" => "仓库尚缺目标词条",
                    "unreachable-species" => "现有帕鲁暂时无法配出目标",
                    _ => "配种结果",
                });
                render_plan(ui, d, &self.result, store.world(), &mut nav);
                if ui
                    .button(if self.active_plan.is_empty() {
                        "保存为配种计划"
                    } else {
                        "更新此计划当前路线"
                    })
                    .clicked()
                {
                    let initial = if ["ok", "owned"].contains(&status) {
                        stable_route(d, &self.result, store.world())
                    } else {
                        Value::Null
                    };
                    let mut next = store.state.clone();
                    let world = store.world_index();
                    let plans = next["worlds"][world]["plans"].as_array_mut().unwrap();
                    if let Some(plan) = plans.iter_mut().find(|p| p["id"] == self.active_plan) {
                        plan["currentRoute"] = initial;
                        plan["nativeResult"] = self.result.clone();
                        plan["targetKey"] = d.species[self.pal]["key"].clone();
                        plan["traits"] = json!(self.traits);
                        plan["random"] = json!(self.random);
                        plan["surgery"] = json!(self.surgery);
                    } else {
                        let id = format!("plan-{}", uuid::Uuid::new_v4());
                        plans.push(json!({"id":id,"created":timestamp(),"name":format!("{} 配种计划",text(&d.species[self.pal],"zh")),"targetKey":d.species[self.pal]["key"],"traits":self.traits,"random":self.random,"surgery":self.surgery,"initialRoute":initial,"nativeResult":self.result,"log":[],"pending":null}));
                        self.active_plan = id;
                    }
                    match store.commit(next) {
                        Ok(()) => {
                            self.notice = "已保存计划；实际孵化和手术结果可在下方登记。".into()
                        }
                        Err(e) => self.notice = e,
                    }
                }
            }
        }
        if !offspring {
            self.saved_plans(ui, d, store, &mut nav);
        }
        if self.picker {
            trait_picker(ui.ctx(), d, &mut self.picker, &mut self.traits);
        }
        self.edit_dialog(ui.ctx(), d, store);
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        nav
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
                            ui.horizontal_wrapped(|ui|{for parent in array(&step["parents"]){ui.label(format!("{} {} {}",d.pal_name(text(parent,"speciesKey")),sex_label(text(parent,"sex")),text(parent,"name")));}ui.label("→");for t in array(&step["traits"]){ui.label(d.trait_name(t.as_str().unwrap_or("")));}});
                            let parents_ready=array(&step["parents"]).len()==2&&array(&step["parents"]).iter().all(|parent|array(&store.world()["inventory"]["records"]).iter().any(|r|r["id"]==parent["stockId"]));
                            if ui.add_enabled(parents_ready,egui::Button::new(format!("登记第 {} 步实际孵化结果",i+1))).on_disabled_hover_text("请先登记前一步的实际子代，再重新计算并更新计划路线。").clicked(){let mut draft=new_record(text(step,"speciesKey"));let id=uuid::Uuid::new_v4().to_string();draft["id"]=json!(format!("hatch-{id}"));draft["name"]=json!(format!("{} 第{}只",d.pal_name(text(step,"speciesKey")),array(&p["log"]).len()+1));draft["traits"]=step["traits"].clone();self.edit=Some(draft);self.recording=Some((text(p,"id").into(),json!({"id":id,"stockId":format!("hatch-{id}"),"kind":"hatch","at":timestamp(),"speciesKey":step["speciesKey"],"generation":step["generation"],"expectedTraits":step["traits"],"expectedSexes":step["sexes"],"parents":step["parents"]})));}
                        });
                    }
                    if p["surgery"]==true{egui::CollapsingHeader::new("登记现有目标个体的实际手术结果").show(ui,|ui|{let targets:Vec<_>=array(&store.world()["inventory"]["records"]).iter().filter(|r|r["speciesKey"]==p["targetKey"]).cloned().collect();if targets.is_empty(){ui.label("请先登记配出的目标帕鲁。");}for r in targets{if ui.button(format!("登记手术 · {} · {}",text(&r,"name"),sex_label(text(&r,"sex")))).clicked(){let id=uuid::Uuid::new_v4().to_string();self.recording=Some((text(p,"id").into(),json!({"id":id,"stockId":r["id"],"kind":"surgery","at":timestamp(),"speciesKey":r["speciesKey"],"generation":0,"expectedTraits":p["traits"],"expectedSexes":[],"parents":[]})));self.edit=Some(r);}}});}
                    egui::CollapsingHeader::new("实际孵化 / 手术记录").show(ui,|ui|render_value(ui,d,&p["log"],0,nav));
                    ui.horizontal(|ui|{if ui.button("载入目标并重新计算").clicked(){if let Some(index)=d.species.iter().position(|s|s["key"]==p["targetKey"]){self.active_plan=text(p,"id").into();self.pal=index;self.traits=array(&p["traits"]).iter().filter_map(|v|v.as_str().map(str::to_owned)).collect();self.random=p["random"]==true;self.surgery=p["surgery"]==true;self.invalidate();self.notice="已载入计划，点击计算配种路径；登记新子代后请更新当前路线。".into();}}if ui.small_button("删除计划").clicked(){delete=Some(text(p,"id").to_owned());}});
                });
            });}
            if let Some(id)=delete{self.delete_plan=Some(id);}
        });
        if let Some(id) = self.delete_plan.clone() {
            let mut keep = true;
            egui::Window::new("确认删除配种计划")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label("删除该计划及执行记录？已经入仓的个体仍会保留。");
                    ui.horizontal(|ui| {
                        if ui.button("确认删除").clicked() {
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
                        if ui.button("取消").clicked() {
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
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("搜索子代"));
            element_picker(ui, &mut self.element);
            ui.checkbox(&mut self.new_only, "仅看未拥有");
        });
        let rows: Vec<_> = array(&self.result["items"])
            .iter()
            .filter(|r| {
                self.all_steps || r["depth"].as_u64().unwrap_or(u64::MAX) <= self.max_steps as u64
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
                    && (!self.new_only
                        || !array(&store.world()["inventory"]["records"])
                            .iter()
                            .any(|r| r["speciesKey"] == p["key"]))
            })
            .collect();
        ui.label(format!(
            "{} 种帕鲁 · {}",
            rows.len(),
            if self.all_steps {
                "不限步数".into()
            } else {
                format!("{} 步以内", self.max_steps)
            }
        ));
        if rows.is_empty() {
            ui.label("没有符合条件的结果。可放宽筛选，或补齐仓库帕鲁及其性别后重算。");
        }
        egui::ScrollArea::vertical()
            .id_salt("offspring-results")
            .max_height(420.)
            .show_rows(ui, 48., rows.len(), |ui, range| {
                for i in range {
                    let r = rows[i];
                    let id = r["id"].as_u64().unwrap() as usize;
                    let p = &d.species[id];
                    ui.horizontal(|ui| {
                        d.portrait(ui, text(p, "key"), 36.);
                        if ui
                            .selectable_label(self.offspring_selected == Some(id), text(p, "zh"))
                            .clicked()
                        {
                            self.offspring_selected = Some(id);
                        }
                        ui.label(format!("{} 代", r["depth"]));
                        if r["conditional"] == true {
                            ui.small("需确认亲代性别");
                        }
                    });
                }
            });
        if let Some(r) = array(&self.result["items"])
            .iter()
            .find(|r| r["id"].as_u64() == self.offspring_selected.map(|i| i as u64))
        {
            for step in array(&r["steps"]) {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("第 {} 代", step["depth"]));
                    for parent in array(&step["parents"]) {
                        if let Some(p) = parent["species"]
                            .as_u64()
                            .and_then(|i| d.species.get(i as usize))
                        {
                            if ui
                                .link(format!(
                                    "{} {}{}",
                                    text(p, "zh"),
                                    sex_label(text(parent, "sex")),
                                    if parent["unknown"] == true {
                                        "（待确认）"
                                    } else {
                                        ""
                                    }
                                ))
                                .clicked()
                            {
                                *nav = Some(format!("#paldex/{}", text(p, "key")));
                            }
                        }
                    }
                    ui.label("→");
                    if let Some(p) = step["child"]
                        .as_u64()
                        .and_then(|i| d.species.get(i as usize))
                    {
                        if ui.link(text(p, "zh")).clicked() {
                            *nav = Some(format!("#paldex/{}", text(p, "key")));
                        }
                    }
                });
            }
        }
        if self.result["unresolved"].as_u64().unwrap_or(0) > 0 {
            ui.label("部分路线受未知性别影响，无法确认最短路径，请补齐性别后重算。");
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
            .id(egui::Id::new("edit-pal"))
            .open(&mut open)
            .default_width(1100.)
            .vscroll(true)
            .show(ctx, |ui| {
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
                if ui.button("保存个体").clicked() {
                    save = true;
                }
            });
        if save {
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
                    self.edit = None;
                    self.recording = None;
                    self.edit_world.clear();
                    self.active_plan.clear();
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
    let query_id = ui.id().with("trait-query");
    let mut query = ui
        .data_mut(|m| m.get_temp::<String>(query_id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut query).hint_text("搜索词条名称或效果（留空显示全部）"),
        );
        if ui.small_button("清空搜索").clicked() {
            query.clear();
        }
    });
    ui.data_mut(|m| m.insert_temp(query_id, query.clone()));
    let mut traits: Vec<_> = array(&d.get("trait-data")["traits"])
        .iter()
        .filter(|t| {
            query.is_empty()
                || search(t, &query)
                || search(
                    &d.get("pal-details-data")["passives"][text(t, "key")],
                    &query,
                )
        })
        .collect();
    traits.sort_by_key(|t| std::cmp::Reverse(t["rank"].as_i64().unwrap_or(0)));
    ui.label(format!(
        "已选 {}/4 · 按稀有度从高到低 · {} 项",
        keys.len(),
        traits.len()
    ));
    ui.horizontal_wrapped(|ui| {
        for key in keys.clone() {
            if ui
                .small_button(format!("{} ×", d.trait_name(&key)))
                .clicked()
            {
                keys.retain(|k| k != &key);
            }
        }
    });
    let cols = ((ui.available_width() / 165.) as usize).max(1);
    egui::Grid::new("traits-grid")
        .num_columns(cols)
        .spacing(Vec2::new(6., 5.))
        .show(ui, |ui| {
            for (i, t) in traits.iter().enumerate() {
                let key = text(t, "key");
                let mut selected = keys.iter().any(|k| k == key);
                ui.horizontal(|ui| {
                    let rank = t["rank"].as_i64().unwrap_or(0);
                    if let Some(icon) = d.get("trait-ranks")[rank.unsigned_abs() as usize].as_str()
                    {
                        d.image(ui, icon, Vec2::splat(16.));
                    }
                    let response = ui
                        .add_enabled(
                            selected || keys.len() < 4,
                            egui::Checkbox::new(&mut selected, text(t, "zh")),
                        )
                        .on_hover_text(text(&d.get("pal-details-data")["passives"][key], "effect"));
                    if response.changed() {
                        keys.retain(|k| k != key);
                        if selected {
                            keys.push(key.into());
                        }
                    }
                });
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
}
pub fn trait_picker(ctx: &egui::Context, d: &Data, open: &mut bool, keys: &mut Vec<String>) {
    egui::Window::new("选择词条")
        .id(egui::Id::new("traits-picker"))
        .open(open)
        .default_width(1120.)
        .default_height(720.)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| trait_grid(ui, d, keys));
        });
}
fn new_record(key: &str) -> Value {
    json!({"id":uuid::Uuid::new_v4().to_string(),"speciesKey":key,"name":"个体 1","sex":"unknown","traits":[],"legacy":false})
}
fn sex_label(s: &str) -> &str {
    match s {
        "M" => "♂ 雄性",
        "F" => "♀ 雌性",
        _ => "性别未知",
    }
}
pub fn element_picker(ui: &mut egui::Ui, current: &mut String) {
    egui::ComboBox::from_id_salt("element")
        .selected_text(if current.is_empty() {
            "全部属性"
        } else {
            current.as_str()
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
                        ui.label("＋");
                    }
                    if let Some(p) = parent["species"]
                        .as_u64()
                        .and_then(|i| d.species.get(i as usize))
                    {
                        d.portrait(ui, text(p, "key"), 32.);
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
                ui.label("→");
                if let Some(p) = step["species"]
                    .as_u64()
                    .and_then(|i| d.species.get(i as usize))
                {
                    d.portrait(ui, text(p, "key"), 36.);
                    if ui.link(text(p, "zh")).clicked() {
                        *nav = Some(format!("#paldex/{}", text(p, "key")));
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("保留词条：");
                for t in array(&step["traits"]) {
                    ui.label(d.trait_name(t.as_str().unwrap_or("")));
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
    for field in ["founders", "missing"] {
        if !result[field].is_null() && !array(&result[field]).is_empty() {
            egui::CollapsingHeader::new(if field == "founders" {
                "需要的仓库个体"
            } else {
                "缺少词条"
            })
            .show(ui, |ui| render_value(ui, d, &result[field], 0, nav));
        }
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
