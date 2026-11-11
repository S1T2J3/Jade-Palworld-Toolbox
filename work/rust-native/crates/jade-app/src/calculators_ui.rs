use crate::{
    breeding_ui::{element_badges, pal_picker, panel, trait_badge, trait_checkbox, trait_picker},
    data::{Data, array, search, text},
};
use eframe::egui;
use serde_json::{Value, json};
pub struct Calculators {
    pub pal: usize,
    pub query: String,
    pub traits: Vec<String>,
    pub picker: bool,
    pub level: u32,
    pub stars: u32,
    pub trust: u32,
    pub awaken: bool,
    pub alpha: bool,
    pub iv: [u32; 3],
    pub souls: [u32; 4],
    pub hp: f64,
    pub world: f64,
    pub effigy: u32,
    pub partner: u32,
    pub module: usize,
    pub poison: bool,
    pub trap: bool,
    pub critical: bool,
    pub throws: u32,
    pub job: String,
    pub work_level: usize,
    pub relic: usize,
    pub simulated: Value,
    pub relic_target: String,
    capture_level: u32,
    encounter: String,
    status: String,
    sphere: usize,
    work_origin: String,
    work_source: String,
    work_query: String,
    work_signature: String,
    work_route: String,
    relic_reverse: bool,
    relic_output: String,
    relic_category: String,
    relic_route: String,
    suitability_route: String,
    active_page: String,
    page_inputs: std::collections::HashMap<String, SharedInput>,
    work_relevant: bool,
    work_trait_query: String,
}
#[derive(Clone)]
struct SharedInput {
    pal: usize,
    traits: Vec<String>,
    query: String,
    job: String,
}
impl Default for Calculators {
    fn default() -> Self {
        Self {
            pal: 0,
            query: String::new(),
            traits: vec![],
            picker: false,
            level: 1,
            stars: 0,
            trust: 0,
            awaken: false,
            alpha: false,
            iv: [0; 3],
            souls: [0; 4],
            hp: 25.,
            world: 1.,
            effigy: 0,
            partner: 0,
            module: 0,
            poison: false,
            trap: false,
            critical: false,
            throws: 5,
            job: "生火".into(),
            work_level: 1,
            relic: 0,
            simulated: Value::Null,
            relic_target: String::new(),
            capture_level: 10,
            encounter: "wild".into(),
            status: String::new(),
            sphere: 0,
            work_origin: "manual".into(),
            work_source: String::new(),
            work_query: String::new(),
            work_signature: String::new(),
            work_route: String::new(),
            relic_reverse: false,
            relic_output: String::new(),
            relic_category: String::new(),
            relic_route: String::new(),
            suitability_route: String::new(),
            active_page: String::new(),
            page_inputs: std::collections::HashMap::new(),
            work_relevant: true,
            work_trait_query: String::new(),
        }
    }
}
impl Calculators {
    fn enter_page(&mut self, page: &str) {
        if self.active_page == page {
            return;
        }
        self.picker = false;
        if !self.active_page.is_empty() {
            self.page_inputs.insert(
                self.active_page.clone(),
                SharedInput {
                    pal: self.pal,
                    traits: self.traits.clone(),
                    query: self.query.clone(),
                    job: self.job.clone(),
                },
            );
        }
        if let Some(saved) = self.page_inputs.get(page) {
            self.pal = saved.pal;
            self.traits = saved.traits.clone();
            self.query = saved.query.clone();
            self.job = saved.job.clone();
        } else if !self.active_page.is_empty() {
            self.pal = 0;
            self.traits.clear();
            self.query.clear();
            self.job = "生火".into();
        }
        self.active_page = page.into();
    }
    pub fn activate_pal(&mut self, page: &str, index: usize) {
        self.enter_page(page);
        self.pal = index;
        self.query.clear();
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        page: &str,
        world: &Value,
        route: &str,
    ) -> Option<String> {
        self.enter_page(page);
        let mut nav = None;
        egui::ScrollArea::vertical()
            .id_salt(("calculator-page", page))
            .show(ui, |ui| {
                if page == "details/stats" || page == "details/capture" {
                    panel(ui).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            d.portrait(ui, text(&d.species[self.pal], "key"), 50.);
                            pal_picker(ui, d, &mut self.pal, &mut self.query);
                            element_badges(ui, &d.species[self.pal]["types"]);
                            if ui.link("图鉴资料").clicked() {
                                nav =
                                    Some(format!("#paldex/{}", text(&d.species[self.pal], "key")));
                            }
                        });
                    });
                    ui.add_space(10.);
                }
                match page {
                    "details/stats" => self.stats(ui, d, &mut nav),
                    "details/capture" => self.capture(ui, d, &mut nav),
                    "work/efficiency" => self.work(ui, d, world, route, &mut nav),
                    "work/relic" => self.relic(ui, d, route, &mut nav),
                    _ => {}
                }
            });
        if self.picker {
            trait_picker(ui.ctx(), d, &mut self.picker, &mut self.traits);
        }
        nav
    }
    fn stat_result(&self, d: &Data) -> Value {
        let mut passive = json!({});
        for key in &self.traits {
            if let Some(mods) = d.get("stat-data")["modifiers"][key].as_object() {
                for (k, v) in mods {
                    passive[k] =
                        json!(passive[k].as_f64().unwrap_or(0.) + v.as_f64().unwrap_or(0.));
                }
            }
        }
        let config = json!({"level":self.level,"stars":self.stars,"trust":self.trust,"awaken":self.awaken,"iv":{"hp":self.iv[0],"attack":self.iv[1],"defense":self.iv[2]},"souls":{"hp":self.souls[0],"attack":self.souls[1],"defense":self.souls[2],"work":self.souls[3]}});
        let key = text(&d.species[self.pal], "key");
        let profile =
            &d.get("stat-data")["profiles"][key][if self.alpha { "alpha" } else { "normal" }];
        if profile.is_null() {
            Value::Null
        } else {
            jade_core::calculate_pal_stats(profile, &config, &passive)
        }
    }
    fn stats(&mut self, ui: &mut egui::Ui, d: &Data, nav: &mut Option<String>) {
        panel(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::design::section(ui, "培养参数");
                if ui
                    .button("满练参数")
                    .on_hover_text("等级80、浓缩4星、信赖10、个体值和帕鲁之魂升满")
                    .clicked()
                {
                    self.level = 80;
                    self.stars = 4;
                    self.trust = 10;
                    self.iv = [100; 3];
                    self.souls = [20; 4];
                }
                if ui.button("重置").clicked() {
                    self.level = 1;
                    self.stars = 0;
                    self.trust = 0;
                    self.iv = [0; 3];
                    self.souls = [0; 4];
                    self.traits.clear();
                    self.alpha = false;
                    self.awaken = false;
                }
            });
            ui.horizontal_wrapped(|ui| {
                number_field(ui, "帕鲁等级", &mut self.level, 1..=80);
                number_field(ui, "浓缩星级", &mut self.stars, 0..=4);
                number_field(ui, "信赖等级", &mut self.trust, 0..=10);
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.alpha, "头目 / 稀有个体");
                    ui.checkbox(&mut self.awaken, "已觉醒");
                });
            });
            ui.add_space(10.);
            let unavailable = text(&d.species[self.pal], "key") == "WorldTreeDragon";
            egui::Grid::new("stat-cultivation-grid")
                .num_columns(5)
                .striped(true)
                .min_col_width((ui.available_width() / 6.).min(120.))
                .spacing(egui::vec2(16., 10.))
                .show(ui, |ui| {
                    for h in ["属性", "个体值 0–100", "魂强化 0–20", "面板值", "远征值"]
                    {
                        ui.add_sized(
                            egui::vec2(112., 24.),
                            egui::Label::new(egui::RichText::new(h).strong()),
                        );
                    }
                    ui.end_row();
                    for (i, (stat, name)) in [
                        ("hp", "生命"),
                        ("attack", "攻击"),
                        ("defense", "防御"),
                        ("work", "工作速度"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.strong(name);
                        if i < 3 {
                            ui.add(egui::DragValue::new(&mut self.iv[i]).range(0..=100));
                        } else {
                            ui.label("—");
                        }
                        ui.add(egui::DragValue::new(&mut self.souls[i]).range(0..=20));
                        let result = self.stat_result(d);
                        ui.label(
                            egui::RichText::new(number(&result["panel"][stat]))
                                .strong()
                                .color(crate::design::palette(ui.ctx()).accent),
                        );
                        ui.label(if unavailable || stat == "work" {
                            "—".into()
                        } else {
                            number(&result["expedition"][stat])
                        });
                        ui.end_row();
                    }
                });
            let result = self.stat_result(d);
            ui.add_space(10.);
            crate::design::stat(
                ui,
                "单只远征战力",
                if unavailable {
                    "—".into()
                } else {
                    number(&result["power"])
                },
            );
            if result.is_null() {
                ui.label("该帕鲁暂无可计算参数。");
            } else if unavailable {
                ui.label("枯星龙为无法捕获的塔主，面板只供查看种族参数，无法派去远征。");
            } else {
                ui.small(format!(
                    "(生命折算 {} + 攻击 {} + 防御 {}) × 星级倍率 {}",
                    (result["expedition"]["hp"].as_f64().unwrap_or(0.) / 5.).floor(),
                    number(&result["expedition"]["attack"]),
                    number(&result["expedition"]["defense"]),
                    number(&result["factor"])
                ));
            }
            if ui.link("查看远征要求与奖励 →").clicked() {
                *nav = Some("#work/expeditions".into());
            }
        });
        ui.add_space(10.);
        panel(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                crate::design::section(ui, "面板被动词条");
                crate::design::badge(ui, format!("{} / 4", self.traits.len()));
                if ui.button("选择词条").clicked() {
                    self.picker = true;
                }
                if !self.traits.is_empty() && ui.button("清空词条").clicked() {
                    self.traits.clear();
                }
            });
            ui.small("被动词条影响面板；远征战力使用不含被动加成的属性值。");
            ui.horizontal_wrapped(|ui| {
                for key in self.traits.clone() {
                    if trait_badge(ui, d, &key).clicked() {
                        *nav = Some(format!("#details/passive/trait/{key}"));
                    }
                    if ui.small_button("×").clicked() {
                        self.traits.retain(|k| k != &key);
                    }
                }
            });
            if self.traits.iter().any(|key| key == "MutationPal_Mutant") {
                ui.small("突变词条含条件效果；结果按所选词条已经生效计算。");
            }
        });
    }
    fn capture(&mut self, ui: &mut egui::Ui, d: &Data, nav: &mut Option<String>) {
        if ui.available_width() > 810. {
            ui.columns(2, |columns| {
                panel(&columns[0]).show(&mut columns[0], |ui| self.capture_controls(ui, d, nav));
                panel(&columns[1]).show(&mut columns[1], |ui| self.capture_results(ui, d, nav));
            });
        } else {
            panel(ui).show(ui, |ui| self.capture_controls(ui, d, nav));
            ui.add_space(10.);
            panel(ui).show(ui, |ui| self.capture_results(ui, d, nav));
        }
    }
    fn capture_controls(&mut self, ui: &mut egui::Ui, d: &Data, nav: &mut Option<String>) {
        let data = d.get("capture-data");
        let key = text(&d.species[self.pal], "key");
        let has_alpha = data["profiles"][key][1].is_number();
        if !has_alpha && self.encounter == "alpha" {
            self.encounter = "wild".into();
        }
        crate::design::section(ui, "捕捉条件");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("capture-encounter")
                .selected_text(match self.encounter.as_str() {
                    "alpha" => "野生头目",
                    "blocked" => "禁止捕捉的战斗",
                    _ => "普通野生帕鲁",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.encounter, "wild".into(), "普通野生帕鲁");
                    ui.add_enabled_ui(has_alpha, |ui| {
                        ui.selectable_value(&mut self.encounter, "alpha".into(), "野生头目");
                    });
                    ui.selectable_value(&mut self.encounter, "blocked".into(), "禁止捕捉的战斗");
                });
            ui.label("目标等级");
            ui.add(egui::DragValue::new(&mut self.capture_level).range(1..=80));
            ui.label("剩余生命 %");
            ui.add(egui::DragValue::new(&mut self.hp).range(0.0..=100.));
            ui.label("世界倍率");
            ui.add(
                egui::DragValue::new(&mut self.world)
                    .range(0.0..=100.)
                    .speed(0.1),
            );
        });
        ui.add_space(8.);
        let spheres = array(&data["spheres"]);
        let modules = array(&data["modules"]);
        self.sphere = self.sphere.min(spheres.len().saturating_sub(1));
        self.module = self.module.min(modules.len());
        crate::design::section(ui, "捕捉装备");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("capture-sphere")
                .selected_text(
                    spheres
                        .get(self.sphere)
                        .map(|s| text(s, "name"))
                        .unwrap_or("帕鲁球"),
                )
                .show_ui(ui, |ui| {
                    for (i, s) in spheres.iter().enumerate() {
                        ui.selectable_value(&mut self.sphere, i, text(s, "name"));
                    }
                });
            egui::ComboBox::from_id_salt("capture-module")
                .selected_text(if self.module == 0 {
                    "未装备模块"
                } else {
                    text(&modules[self.module - 1], "name")
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.module, 0, "未装备模块");
                    for (i, m) in modules.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.module,
                            i + 1,
                            format!("{}（+{}）", text(m, "name"), number(&m["power"])),
                        );
                    }
                });
            ui.label("力量石像等级");
            ui.add(egui::DragValue::new(&mut self.effigy).range(0..=15));
            ui.label("计划投球数");
            ui.add(egui::DragValue::new(&mut self.throws).range(1..=999));
        });
        ui.collapsing("异常状态与额外加成",|ui|{ui.horizontal_wrapped(|ui|{egui::ComboBox::from_id_salt("capture-status").selected_text(status_name(&self.status)).show_ui(ui,|ui|{for key in ["","burn","wet","freeze","shock","mud","ivy","blind"]{ui.selectable_value(&mut self.status,key.into(),status_name(key));}});ui.checkbox(&mut self.poison,"中毒");ui.checkbox(&mut self.trap,"被捕兽夹困住");ui.checkbox(&mut self.critical,"帕鲁球命中弱点");ui.label("已生效伙伴捕获加成");ui.add(egui::DragValue::new(&mut self.partner).range(0..=25));});
            if ui.link("异常状态说明").clicked(){*nav=Some(format!("#guide/status/{}",if self.status.is_empty()&&self.poison{"poison"}else{&self.status}));}
            ui.label("念影喵背击、雪绵啾冻结和蒲蒲飞芽缠绕的伙伴技能需满足对应条件。按已生效的加成填写，每只 +1～+5。");
        });
    }
    fn capture_results(&mut self, ui: &mut egui::Ui, d: &Data, nav: &mut Option<String>) {
        crate::design::section(ui, "捕捉结果");
        let data = d.get("capture-data");
        let key = text(&d.species[self.pal], "key");
        let spheres = array(&data["spheres"]);
        let modules = array(&data["modules"]);
        let raid = array(&data["raidEggOnly"])
            .iter()
            .any(|v| v.as_str() == Some(key));
        let blocked = key == "WorldTreeDragon" || raid || self.encounter == "blocked";
        if blocked {
            ui.strong(if raid {
                "常规世界中的团体战头目不能投球捕捉，击败后从奖励蛋孵出。"
            } else {
                "这场战斗禁止捕捉，或目标是无法捕捉的塔主。"
            });
            for row in array(&data["enemiesByPal"][key]) {
                if ui.link(row[1].as_str().unwrap_or("敌人图鉴")).clicked() {
                    *nav = Some(format!("#enemies/{}", row[0].as_str().unwrap_or("")));
                }
            }
        } else if self.hp == 0. {
            ui.label("目标已倒下，无法捕捉。");
        } else if self.world == 0. {
            ui.label("当前世界捕捉倍率为 0。");
        }
        let coefficient = data["profiles"][key][usize::from(self.encounter == "alpha")]
            .as_f64()
            .unwrap_or(0.);
        let module = if self.module == 0 {
            0.
        } else {
            modules[self.module - 1]["power"].as_f64().unwrap_or(0.)
        };
        let bonus = self.effigy as f64 / 2.
            + module
            + self.partner as f64
            + if self.critical { 2. } else { 0. };
        let status = (if self.status.is_empty() { 0. } else { 0.3 })
            + if self.poison { 0.3 } else { 0. }
            + if self.trap { 0.5 } else { 0. };
        let rows:Vec<Value>=spheres.iter().map(|sphere|{let power=sphere["power"].as_f64().unwrap_or(0.)+bonus;let mut row=jade_core::estimate_capture(&json!({"power":power,"level":self.capture_level,"hp":self.hp,"coefficient":coefficient,"world":self.world,"status":status,"blocked":blocked}));row["power"]=json!(power);row}).collect();
        if let Some(row) = rows.get(self.sphere) {
            ui.separator();
            crate::design::stat(
                ui,
                "单球成功率",
                percent(row["chance"].as_f64().unwrap_or(0.)),
            );
            ui.add(
                egui::ProgressBar::new(row["chance"].as_f64().unwrap_or(0.) as f32)
                    .desired_height(10.)
                    .fill(crate::design::palette(ui.ctx()).accent),
            );
            ui.label(format!(
                "准星预估 {} · {} 球内至少成功一次 {}",
                percent(row["reticle"].as_f64().unwrap_or(0.)),
                self.throws,
                percent(cumulative(
                    row["chance"].as_f64().unwrap_or(0.),
                    self.throws
                ))
            ));
            ui.small("多球概率假定每次投球条件相同；闪光帕鲁的有效捕捉等级可能与显示等级不同。");
        }
        egui::Grid::new("capture-results")
            .striped(true)
            .show(ui, |ui| {
                for h in ["帕鲁球", "总捕获力", "单球成功率", "准星预估", "物品"]
                {
                    ui.strong(h);
                }
                ui.end_row();
                for (i, sphere) in spheres.iter().enumerate() {
                    let row = &rows[i];
                    ui.selectable_value(&mut self.sphere, i, text(sphere, "name"));
                    ui.label(number(&row["power"]));
                    ui.label(percent(row["chance"].as_f64().unwrap_or(0.)));
                    ui.label(percent(row["reticle"].as_f64().unwrap_or(0.)));
                    if ui.link("获取方式").clicked() {
                        *nav = Some(format!("#items/{}", text(sphere, "id")));
                    }
                    ui.end_row();
                }
            });
    }
    fn pick_work(&mut self, d: &Data, world: &Value, requested_job: Option<&str>) {
        let (key, record) = if self.work_origin == "warehouse" {
            let record = array(&world["inventory"]["records"])
                .iter()
                .find(|r| text(r, "id") == self.work_source);
            (record.map(|r| text(r, "speciesKey")).unwrap_or(""), record)
        } else {
            (self.work_source.as_str(), None)
        };
        let key = key.to_string();
        let works = array(&d.get("paldex-index-data")["profiles"][&key]["work"]);
        self.traits = record
            .map(|r| {
                array(&r["traits"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(job) = requested_job {
            self.job = job.into()
        } else if !works.is_empty() && !works.iter().any(|r| r[0].as_str() == Some(&self.job)) {
            self.job = works[0][0].as_str().unwrap_or("生火").into();
        }
        let known = d.species.iter().any(|p| text(p, "key") == key);
        self.work_level = if self.work_source.is_empty() {
            if self.work_origin == "manual" { 1 } else { 0 }
        } else if known {
            natural_level(d, &key, &self.job)
        } else {
            1
        };
        self.work_signature = record
            .map(|r| format!("{}|{}|{}", text(world, "id"), r["speciesKey"], r["traits"]))
            .unwrap_or_default();
    }
    fn work(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        world: &Value,
        route: &str,
        nav: &mut Option<String>,
    ) {
        if ui.available_width() > 810. {
            ui.columns(2, |columns| {
                panel(&columns[0]).show(&mut columns[0], |ui| {
                    self.work_controls(ui, d, world, route, nav)
                });
                panel(&columns[1]).show(&mut columns[1], |ui| self.work_result(ui, d, world, nav));
            });
        } else {
            panel(ui).show(ui, |ui| self.work_controls(ui, d, world, route, nav));
            ui.add_space(10.);
            panel(ui).show(ui, |ui| self.work_result(ui, d, world, nav));
        }
    }
    fn work_controls(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        world: &Value,
        route: &str,
        nav: &mut Option<String>,
    ) {
        crate::design::section(ui, "工作条件");
        let data = d.get("work-efficiency-data");
        let Some(levels) = data["levels"].as_object() else {
            ui.label("工作资料正在准备中…");
            return;
        };
        if !d.get("paldex-index-data")["profiles"].is_object() {
            ui.label("帕鲁工作资料正在准备中…");
            return;
        }
        if self.work_route != route {
            self.work_route = route.into();
            let parts: Vec<String> = route
                .trim_start_matches('#')
                .split('/')
                .map(decode)
                .collect();
            if let Some(argument) = parts.get(2) {
                if d.species.iter().any(|p| text(p, "key") == argument) {
                    self.work_origin = "dex".into();
                    self.work_source = argument.clone();
                    let requested = parts
                        .get(3)
                        .filter(|job| levels.contains_key(job.as_str()))
                        .map(String::as_str);
                    self.pick_work(d, world, requested);
                } else if levels.contains_key(argument) {
                    self.work_origin = "manual".into();
                    self.work_source.clear();
                    self.job = argument.clone();
                    self.work_level = 1;
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("数据来源");
            let previous = self.work_origin.clone();
            for (id, label) in [
                ("manual", "手动填写"),
                ("dex", "帕鲁图鉴"),
                ("warehouse", "当前世界仓库"),
            ] {
                ui.selectable_value(&mut self.work_origin, id.into(), label);
            }
            if previous != self.work_origin {
                self.work_source.clear();
                self.work_query.clear();
                self.pick_work(d, world, None);
            }
        });
        if self.work_origin != "manual" {
            ui.add(
                egui::TextEdit::singleline(&mut self.work_query)
                    .hint_text("搜索帕鲁、编号或个体名字"),
            );
            let mut picked = false;
            if self.work_origin == "dex" {
                egui::ComboBox::from_id_salt("work-source-dex")
                    .width(330.)
                    .height(400.)
                    .selected_text(if self.work_source.is_empty() {
                        "请选择帕鲁".into()
                    } else {
                        d.pal_name(&self.work_source)
                    })
                    .show_ui(ui, |ui| {
                        for p in &d.species {
                            if search(p, &self.work_query) {
                                picked |= ui
                                    .selectable_value(
                                        &mut self.work_source,
                                        text(p, "key").into(),
                                        format!("#{} {}", text(p, "dex"), text(p, "zh")),
                                    )
                                    .changed();
                            }
                        }
                    });
            } else {
                let records = array(&world["inventory"]["records"]);
                let label = records
                    .iter()
                    .find(|r| text(r, "id") == self.work_source)
                    .map(|r| {
                        format!(
                            "{} · {}",
                            text(r, "name"),
                            d.pal_name(text(r, "speciesKey"))
                        )
                    })
                    .unwrap_or_else(|| {
                        if records.is_empty() {
                            "当前世界仓库为空".into()
                        } else {
                            "请选择仓库个体".into()
                        }
                    });
                egui::ComboBox::from_id_salt("work-source-stock")
                    .width(ui.available_width().min(440.))
                    .height(400.)
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for r in records {
                            let label = format!(
                                "{} · {} · {} · {}",
                                text(r, "name"),
                                d.pal_name(text(r, "speciesKey")),
                                match text(r, "sex") {
                                    "M" => "雄",
                                    "F" => "雌",
                                    _ => "性别未知",
                                },
                                text(r, "id")
                                    .chars()
                                    .rev()
                                    .take(8)
                                    .collect::<String>()
                                    .chars()
                                    .rev()
                                    .collect::<String>()
                            );
                            if self.work_query.is_empty()
                                || label.contains(&self.work_query)
                                || search(r, &self.work_query)
                            {
                                picked |= ui
                                    .selectable_value(
                                        &mut self.work_source,
                                        text(r, "id").into(),
                                        label,
                                    )
                                    .changed();
                            }
                        }
                    });
                let signature = records
                    .iter()
                    .find(|r| text(r, "id") == self.work_source)
                    .map(|r| format!("{}|{}|{}", text(world, "id"), r["speciesKey"], r["traits"]))
                    .unwrap_or_default();
                if !self.work_source.is_empty() && signature.is_empty() {
                    self.work_source.clear();
                    picked = true;
                } else if signature != self.work_signature {
                    picked = true;
                }
            }
            if picked {
                self.pick_work(d, world, None);
            }
        }
        let source_key = if self.work_origin == "warehouse" {
            array(&world["inventory"]["records"])
                .iter()
                .find(|r| text(r, "id") == self.work_source)
                .map(|r| text(r, "speciesKey"))
                .unwrap_or("")
        } else {
            &self.work_source
        };
        let source_key = source_key.to_string();
        let known = d.species.iter().any(|p| text(p, "key") == source_key);
        ui.horizontal_wrapped(|ui| {
            let old_job = self.job.clone();
            egui::ComboBox::from_id_salt("work-job")
                .selected_text(&self.job)
                .show_ui(ui, |ui| {
                    for job in levels.keys() {
                        ui.selectable_value(&mut self.job, job.clone(), job);
                    }
                });
            if old_job != self.job && self.work_origin != "manual" && known {
                self.work_level = natural_level(d, &source_key, &self.job);
            }
            ui.label("适应性等级");
            let can_edit = self.work_origin == "manual"
                || (!self.work_source.is_empty()
                    && (!known || natural_level(d, &source_key, &self.job) > 0));
            ui.add_enabled(
                can_edit,
                egui::DragValue::new(&mut self.work_level)
                    .range(0..=array(&data["levels"][&self.job]).len()),
            );
            if ui.button("选择被动词条").clicked() {
                self.picker = true;
            }
        });
        let selected = self.work_origin == "manual" || !self.work_source.is_empty();
        let natural = natural_level(d, &source_key, &self.job);
        if self.work_origin == "manual" {
            ui.small("可填写浓缩、研习或据点支援后的等级；牧场词条另行加上。");
        } else if !selected {
            ui.small("请先选择帕鲁。");
        } else if !known {
            ui.small("图鉴暂无此物种的工作资料，请按游戏内等级填写。");
        } else if natural == 0 {
            ui.small(format!("该物种没有{}适应性。", self.job));
        } else {
            ui.small(format!(
                "图鉴基础 Lv. {natural}，可填写该个体经过培养和据点支援后的等级。"
            ));
        }
        ui.horizontal_wrapped(|ui| {
            for key in self.traits.clone() {
                if trait_badge(ui, d, &key).clicked() {
                    *nav = Some(format!("#details/passive/trait/{key}"));
                }
                if ui.small_button("×").on_hover_text("移除此词条").clicked() {
                    self.traits.retain(|v| v != &key);
                }
            }
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong("被动词条");
            crate::design::badge(ui, format!("{} / 4", self.traits.len()));
            ui.selectable_value(&mut self.work_relevant, true, "与本项相关");
            ui.selectable_value(&mut self.work_relevant, false, "全部词条");
        });
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.work_trait_query)
                .hint_text("搜索词条；输入准确名称，按回车添加"),
        );
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            for term in self
                .work_trait_query
                .split([',', '，', '、', ';', '；'])
                .map(str::trim)
                .filter(|t| !t.is_empty())
            {
                if let Some(t) = array(&d.get("trait-data")["traits"]).iter().find(|t| {
                    ["key", "zh", "en"]
                        .iter()
                        .any(|field| text(t, field).eq_ignore_ascii_case(term))
                }) {
                    let key = text(t, "key").to_owned();
                    if self.traits.len() < 4 && !self.traits.contains(&key) {
                        self.traits.push(key);
                    }
                }
            }
        }
        let modifier = if self.job == "搬运" {
            "movement"
        } else if self.job == "牧场" {
            "ranch"
        } else {
            "work"
        };
        let mut choices: Vec<_> = array(&d.get("trait-data")["traits"])
            .iter()
            .filter(|t| {
                (!self.work_relevant
                    || data["modifiers"][text(t, "key")][modifier]
                        .as_f64()
                        .unwrap_or(0.)
                        != 0.)
                    && search(t, &self.work_trait_query)
            })
            .collect();
        choices.sort_by_key(|t| std::cmp::Reverse(t["rank"].as_i64().unwrap_or(0)));
        egui::ScrollArea::vertical()
            .id_salt("work-trait-options")
            .max_height(250.)
            .show(ui, |ui| {
                let columns = ((ui.available_width() / 155.) as usize).max(1);
                egui::Grid::new("work-trait-grid")
                    .num_columns(columns)
                    .show(ui, |ui| {
                        for (i, t) in choices.iter().enumerate() {
                            let key = text(t, "key");
                            let mut checked = self.traits.iter().any(|k| k == key);
                            let enabled = checked || self.traits.len() < 4;
                            if trait_checkbox(ui, d, key, &mut checked, enabled).changed() {
                                self.traits.retain(|k| k != key);
                                if checked {
                                    self.traits.push(key.into());
                                }
                            }
                            if (i + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }
    fn work_result(
        &mut self,
        ui: &mut egui::Ui,
        d: &Data,
        world: &Value,
        nav: &mut Option<String>,
    ) {
        crate::design::section(ui, "计算结果");
        let data = d.get("work-efficiency-data");
        let source_key = if self.work_origin == "warehouse" {
            array(&world["inventory"]["records"])
                .iter()
                .find(|r| text(r, "id") == self.work_source)
                .map(|r| text(r, "speciesKey"))
                .unwrap_or("")
        } else {
            &self.work_source
        };
        let known = d.species.iter().any(|p| text(p, "key") == source_key);
        let selected = self.work_origin == "manual" || !self.work_source.is_empty();
        let natural = natural_level(d, source_key, &self.job);
        match jade_core::calculate_work_efficiency(data, &self.job, self.work_level, &self.traits) {
            Ok(result) => {
                let title = if self.job == "搬运" {
                    "单趟搬运上限（件）"
                } else if self.job == "牧场" {
                    "牧场等级基础参数"
                } else {
                    "标准工作量"
                };
                crate::design::stat(
                    ui,
                    title,
                    if selected {
                        number(&result["amount"])
                    } else {
                        "—".into()
                    },
                );
                ui.label(format!(
                    "生效等级 Lv. {} · 基础值 {}",
                    result["rank"],
                    number(&result["base"])
                ));
                if self.job == "搬运" {
                    ui.label(format!(
                        "移动速度词条 {:+}%；影响往返用时，装载上限由等级决定。",
                        result["movement"].as_f64().unwrap_or(0.)
                    ));
                } else if self.job == "牧场" {
                    ui.label(format!(
                        "牧场等级词条 +{}；每次产物数量与概率取决于物种和生效等级。",
                        number(&result["ranch"])
                    ));
                    if ui.link("查看牧场产物与概率").clicked() {
                        *nav = Some(if known && natural > 0 {
                            format!("#work/ranch/{source_key}")
                        } else {
                            "#work/ranch".into()
                        });
                    }
                } else {
                    ui.label(format!(
                        "{} × {:.2} = {}（工作速度词条 {:+}%）",
                        number(&result["base"]),
                        (100. + result["work"].as_f64().unwrap_or(0.)).max(0.) / 100.,
                        number(&result["amount"]),
                        result["work"].as_f64().unwrap_or(0.)
                    ));
                    ui.small("只计适应性等级和自身词条；设施、研究、伙伴技能、帕鲁之魂、饮食及世界设置会影响实际产速。");
                    if result["work"].as_f64().unwrap_or(0.) <= -100. {
                        ui.label(
                            "词条合计为 −100% 或更低，公式按零显示；游戏最低工作速度尚未核实。",
                        );
                    }
                }
                let unknown: Vec<_> = self
                    .traits
                    .iter()
                    .filter(|key| {
                        !array(&d.get("trait-data")["traits"])
                            .iter()
                            .any(|t| text(t, "key") == key.as_str())
                    })
                    .cloned()
                    .collect();
                if !unknown.is_empty() {
                    ui.label(format!(
                        "以下词条暂无效果数据，仅计算已知加成：{}",
                        unknown.join("、")
                    ));
                }
            }
            Err(e) => {
                ui.label(e);
            }
        }
        ui.add_space(10.);
        if ui.link("查看工作类伙伴技能 →").clicked() {
            *nav = Some("#details/partner/work".into());
        }
        crate::design::section(ui, "各等级基础值");
        let columns = ((ui.available_width() / 88.) as usize).max(1);
        egui::Grid::new("work-level-scale")
            .num_columns(columns)
            .show(ui, |ui| {
                for (i, value) in array(&data["levels"][&self.job]).iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.set_min_width(72.);
                        crate::design::stat(ui, &format!("Lv. {}", i + 1), number(value));
                    });
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    }
    fn relic(&mut self, ui: &mut egui::Ui, d: &Data, route: &str, nav: &mut Option<String>) {
        panel(ui).show(ui,|ui|{crate::design::section(ui,"古代文明遗物转换");
        let data=d.get("relic-data");let tiers=array(&data["tiers"]);if tiers.is_empty(){ui.label("遗物资料正在准备中…");return}self.relic=self.relic.min(tiers.len()-1);
        let items:std::collections::HashMap<&str,&Value>=array(&d.get("item-catalog-data")["items"]).iter().map(|i|(text(i,"id"),i)).collect();
        let name=|id:&str|items.get(id).map(|i|text(i,"name")).unwrap_or(id).to_owned();
        if self.relic_route!=route{self.relic_route=route.into();let args:Vec<String>=route.trim_start_matches('#').split('/').map(decode).collect();if args.get(2).is_some_and(|a|a=="item"){
            self.relic_reverse=true;if let Some(id)=args.get(3){if array(&data["outputs"]).iter().any(|v|v.as_str()==Some(id)){self.relic_output=id.clone();}}
        }else{self.relic_reverse=false;if let Some(id)=args.get(2){if let Some(index)=tiers.iter().position(|t|text(t,"id")==id){if self.relic!=index{self.simulated=Value::Null;}self.relic=index;}}}}
        if self.relic_output.is_empty(){self.relic_output=array(&data["outputs"]).first().and_then(Value::as_str).unwrap_or("").into();}
        ui.horizontal(|ui|{if ui.selectable_label(!self.relic_reverse,"按遗物查产出").clicked(){*nav=Some(format!("#work/relic/{}",text(&tiers[self.relic],"id")));}if ui.selectable_label(self.relic_reverse,"按产物反查").clicked(){*nav=Some(format!("#work/relic/item/{}",self.relic_output));}});
        ui.add(egui::TextEdit::singleline(&mut self.relic_target).hint_text("搜索产物名称、英文名或物品 ID"));
        let query=self.relic_target.to_lowercase();let item_matches=|id:&str|query.is_empty()||name(id).to_lowercase().contains(&query)||id.to_lowercase().contains(&query)||items.get(id).is_some_and(|item|search(item,&query));
        if self.relic_reverse{
            egui::ComboBox::from_id_salt("relic-output").width(400.).height(400.).selected_text(name(&self.relic_output)).show_ui(ui,|ui|{for id in array(&data["outputs"]).iter().filter_map(Value::as_str){if item_matches(id)&&ui.selectable_value(&mut self.relic_output,id.into(),name(id)).clicked(){*nav=Some(format!("#work/relic/item/{id}"));}}});
            ui.heading(name(&self.relic_output));if ui.link("查看物品获取方式").clicked(){*nav=Some(format!("#items/{}",self.relic_output));}
            egui::Grid::new("relic-reverse").striped(true).show(ui,|ui|{for title in ["投入遗物","获得数量","每件至少获得一次"]{ui.strong(title);}ui.end_row();for tier in tiers{
                if ui.link(name(text(tier,"id"))).clicked(){*nav=Some(format!("#work/relic/{}",text(tier,"id")));}
                let odds=jade_core::relic_product_odds(tier,&data["pools"]);if let Some(row)=array(&odds).iter().find(|r|text(r,"id")==self.relic_output){ui.label(format!("{}–{}",number(&row["min"]),number(&row["max"])));ui.label(format!("{:.5}%",row["probability"].as_f64().unwrap_or(0.)*100.));}else{ui.label("—");ui.label("无此产物");}ui.end_row();
            }});
        }else{
            ui.horizontal_wrapped(|ui|{for (i,t) in tiers.iter().enumerate(){if ui.selectable_label(self.relic==i,name(text(t,"id"))).clicked(){*nav=Some(format!("#work/relic/{}",text(t,"id")));}}});
            let tier=&tiers[self.relic];ui.label(format!("每次消耗 1 件遗物 · 处理工作量 {}",number(&tier["work"])));if ui.link("查看该遗物获取方式").clicked(){*nav=Some(format!("#items/{}",text(tier,"id")));}
            let odds=jade_core::relic_product_odds(tier,&data["pools"]);
            ui.horizontal_wrapped(|ui|{ui.strong("必得材料");for row in array(&odds).iter().filter(|r|r["probability"].as_f64()==Some(1.)){if ui.link(format!("{} ×{}–{}",name(text(row,"id")),number(&row["min"]),number(&row["max"]))).clicked(){*nav=Some(format!("#items/{}",text(row,"id")));}}});
            if ui.button("模拟转换一次").clicked(){let mut x=uuid::Uuid::new_v4().as_u128()as u64;if x==0{x=1}self.simulated=jade_core::simulate_relic(tier,&data["pools"],move||{x^=x<<13;x^=x>>7;x^=x<<17;((x>>11)as f64)/(1u64<<53)as f64});}
            if !self.simulated.is_null(){ui.horizontal_wrapped(|ui|{ui.strong("模拟结果");for row in array(&self.simulated){let id=row[0].as_str().unwrap_or("");if ui.link(format!("{} ×{}",name(id),row[1])).clicked(){*nav=Some(format!("#items/{id}"));}}});}
            let mut categories:Vec<String>=array(&data["outputs"]).iter().filter_map(Value::as_str).filter_map(|id|items.get(id)).map(|i|text(i,"category").into()).collect();categories.sort();categories.dedup();egui::ComboBox::from_id_salt("relic-category").selected_text(if self.relic_category.is_empty(){"全部类别"}else{&self.relic_category}).show_ui(ui,|ui|{ui.selectable_value(&mut self.relic_category,String::new(),"全部类别");for category in categories{ui.selectable_value(&mut self.relic_category,category.clone(),category);}});
            let mut rows:Vec<_>=array(&odds).iter().filter(|r|item_matches(text(r,"id"))&&(self.relic_category.is_empty()||items.get(text(r,"id")).is_some_and(|i|text(i,"category")==self.relic_category))).collect();rows.sort_by(|a,b|b["probability"].as_f64().unwrap_or(0.).total_cmp(&a["probability"].as_f64().unwrap_or(0.)));
            ui.label(format!("{} 种产物",rows.len()));
            let columns=((ui.available_width()/290.)as usize).clamp(1,4);
            let width=(ui.available_width()-(columns-1)as f32*8.)/columns as f32;
            egui::ScrollArea::vertical().id_salt("relic-products").max_height(420.).show_rows(ui,61.,rows.len().div_ceil(columns),|ui,range|{for row_index in range{ui.horizontal_top(|ui|{for column in 0..columns{let Some(row)=rows.get(row_index*columns+column)else{break};let id=text(row,"id");panel(ui).inner_margin(5).show(ui,|ui|{ui.set_width(width-10.);ui.horizontal(|ui|{d.item_icon(ui,id,28.);ui.vertical(|ui|{if ui.link(name(id)).clicked(){*nav=Some(format!("#work/relic/item/{id}"));}ui.small(format!("{}–{} 个 · {:.5}%",number(&row["min"]),number(&row["max"]),row["probability"].as_f64().unwrap_or(0.)*100.));});});});}});}});
            ui.collapsing("各奖励槽的触发率",|ui|{let names=["神秘木材","帕鲁树晶矿","辉石①","辉石②","辉石③","辉石④","辉石⑤","耗材植入体","古代文明核心","果实及养成物品①","果实及养成物品②","果实及养成物品③","装备设计图","稀有耗材植入体"];for (i,slot) in array(&tier["slots"]).iter().enumerate(){ui.label(format!("{} · {}%",names.get(i).unwrap_or(&"奖励槽"),number(&slot["probability"])));}});
        }
        ui.collapsing("转换器与概率说明",|ui|{ui.label("科技 74 级解锁古代文明遗物转换器。接通电力并指派手工作业帕鲁，每次处理 1 件遗物。");ui.horizontal_wrapped(|ui|{ui.label("建造材料：");for (id,quantity) in [("WorldTreeIngot",50),("Wood_WorldTree",50),("PalCrystal_Ex",30),("AncientParts2",20)]{if ui.link(format!("{} ×{quantity}",name(id))).clicked(){*nav=Some(format!("#items/{id}"));}}});if ui.link("世界树圣水：处理效率变为 1.5 倍").clicked(){*nav=Some("#items/WorldTreeHolyWater".into());}ui.label("每个奖励槽独立判定，触发后按权重抽取。这里显示每次转换至少获得一次的概率，同一物品可能来自多个槽。");});
        });
    }
    pub fn suitability(&mut self, ui: &mut egui::Ui, d: &Data, route: &str) -> Option<String> {
        self.enter_page("work/suitability");
        if !d.get("paldex-index-data")["profiles"].is_object() {
            ui.label("帕鲁工作资料正在准备中…");
            return None;
        }
        if self.suitability_route != route {
            self.suitability_route = route.into();
            if let Some(key) = route.trim_start_matches('#').split('/').nth(2).map(decode) {
                if let Some(index) = d.species.iter().position(|p| text(p, "key") == key) {
                    self.pal = index;
                    self.query = text(&d.species[index], "zh").into();
                    if let Some(job) = array(&d.get("paldex-index-data")["profiles"][&key]["work"])
                        .first()
                        .and_then(|row| row[0].as_str())
                    {
                        self.job = job.into();
                    }
                }
            }
        }
        let mut nav = None;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("job")
                .selected_text(&self.job)
                .show_ui(ui, |ui| {
                    if let Some(jobs) = d.get("work-efficiency-data")["levels"].as_object() {
                        for job in jobs.keys() {
                            ui.selectable_value(&mut self.job, job.clone(), job);
                        }
                    }
                });
            ui.text_edit_singleline(&mut self.query);
            if !self.query.is_empty() && ui.button("清除筛选").clicked() {
                self.query.clear();
            }
        });
        let mut rows: Vec<_> = d
            .species
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let key = text(p, "key");
                let profile = &d.get("paldex-index-data")["profiles"][key];
                let level = array(&profile["work"])
                    .iter()
                    .find(|x| x[0].as_str() == Some(&self.job))
                    .and_then(|x| x[1].as_u64())
                    .unwrap_or(0);
                if level > 0 && (self.query.is_empty() || text(p, "zh").contains(&self.query)) {
                    Some((i, p, level, profile))
                } else {
                    None
                }
            })
            .collect();
        rows.sort_by_key(|(i, _, level, _)| (std::cmp::Reverse(*level), *i));
        egui::ScrollArea::vertical().show_rows(ui, 52., rows.len(), |ui, range| {
            for i in range {
                let (_, p, level, profile) = rows[i];
                ui.horizontal(|ui| {
                    d.portrait(ui, text(p, "key"), 38.);
                    if ui.link(text(p, "zh")).clicked() {
                        nav = Some(format!("#paldex/{}", text(p, "key")));
                    }
                    ui.strong(format!("{} {level}", self.job));
                    if array(&p["types"]).iter().any(|t| t == "dark") {
                        ui.label("☾ 暗系");
                    }
                    if profile["nocturnal"] == true {
                        ui.label("夜间工作");
                    }
                    if ui.link("计算效率").clicked() {
                        nav = Some(format!("#work/efficiency/{}/{}", text(p, "key"), self.job));
                    }
                });
            }
        });
        nav
    }
}
fn number_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) {
    ui.vertical(|ui| {
        ui.small(label);
        ui.add(egui::DragValue::new(value).range(range));
    });
}
fn status_name(id: &str) -> &str {
    match id {
        "burn" => "燃烧",
        "wet" => "浸湿",
        "freeze" => "冻结",
        "shock" => "感电",
        "mud" => "泥泞",
        "ivy" => "缠绕",
        "blind" => "黑暗",
        _ => "无主要异常状态",
    }
}
fn decode(s: &str) -> String {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.into())
}
fn percent(v: f64) -> String {
    if v == 0. || v == 1. {
        format!("{:.0}%", v * 100.)
    } else if v < 0.0001 {
        "<0.01%".into()
    } else if v > 0.9999 {
        ">99.99%".into()
    } else {
        format!("{:.2}%", v * 100.)
    }
}
fn number(v: &Value) -> String {
    v.as_f64()
        .map(|v| {
            if v.fract() == 0. {
                format!("{v:.0}")
            } else {
                format!("{v:.2}")
            }
        })
        .unwrap_or_else(|| "—".into())
}
fn cumulative(chance: f64, throws: u32) -> f64 {
    if chance >= 1. {
        1.
    } else {
        (-((throws as f64) * (-chance).ln_1p()).exp_m1()).min(1. - f64::EPSILON)
    }
}
fn natural_level(d: &Data, key: &str, job: &str) -> usize {
    array(&d.get("paldex-index-data")["profiles"][key]["work"])
        .iter()
        .find(|r| r[0].as_str() == Some(job))
        .and_then(|r| r[1].as_u64())
        .unwrap_or(0) as usize
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Data {
        let root =
            std::env::temp_dir().join(format!("jade-calculator-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        for (name, value) in [
            (
                "db",
                json!({"species":[{"key":"SheepBall","zh":"棉悠悠","dex":"001"},{"key":"Fox","zh":"火绒狐","dex":"005"}]}),
            ),
            ("pairs", json!([[0, 0, 0], [0, 1, 0], [1, 1, 1]])),
            (
                "trait-data",
                json!({"traits":[{"key":"Artisan"},{"key":"Swift"}]}),
            ),
            ("portraits", json!({})),
            ("trait-ranks", json!({})),
        ] {
            std::fs::write(
                root.join(format!("{name}.json")),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();
        }
        let mut data = Data::new(root, egui::Context::default()).unwrap();
        data.tables.insert("paldex-index-data".into(),std::sync::Arc::new(json!({"profiles":{"SheepBall":{"work":[["牧场",1],["搬运",1]]},"Fox":{"work":[["生火",2]]}}})));
        data.tables.insert("work-efficiency-data".into(),std::sync::Arc::new(json!({"levels":{"生火":[50,100,200],"牧场":[1,2,3],"搬运":[1,2,3]},"modifiers":{"Artisan":{"work":50},"Swift":{"movement":30}}})));
        data
    }
    fn frame(calc: &mut Calculators, d: &Data, page: &str, world: &Value, route: &str) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                calc.show(ui, d, page, world, route);
            });
        });
    }
    fn remove_fixture(data: Data) {
        let root = data.root.clone();
        drop(data);
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn calculators_keep_page_specific_species_traits_and_job() {
        let mut calc = Calculators::default();
        calc.activate_pal("details/stats", 1);
        calc.traits = vec!["Artisan".into()];
        calc.level = 72;
        calc.enter_page("work/efficiency");
        assert!(calc.traits.is_empty());
        calc.traits = vec!["Swift".into()];
        calc.job = "搬运".into();
        calc.activate_pal("details/capture", 0);
        calc.capture_level = 23;
        calc.enter_page("details/stats");
        assert_eq!(calc.pal, 1);
        assert_eq!(calc.traits, vec!["Artisan"]);
        assert_eq!(calc.level, 72);
        calc.enter_page("work/efficiency");
        assert_eq!(calc.traits, vec!["Swift"]);
        assert_eq!(calc.job, "搬运");
        calc.enter_page("details/capture");
        assert_eq!(calc.pal, 0);
        assert!(calc.traits.is_empty());
        assert_eq!(calc.capture_level, 23);
    }
    #[test]
    fn inline_work_trait_choices_preserve_four_trait_limit_and_removal() {
        let mut d = fixture();
        d.tables.insert(
            "trait-data".into(),
            std::sync::Arc::new(json!({"traits":[
                {"key":"One","zh":"甲","rank":5}, {"key":"Two","zh":"乙","rank":4},
                {"key":"Three","zh":"丙","rank":3}, {"key":"Four","zh":"丁","rank":2},
                {"key":"Five","zh":"戊","rank":1}
            ]})),
        );
        let ctx = egui::Context::default();
        crate::design::apply(&ctx, "moon");
        let mut calc = Calculators::default();
        calc.work_relevant = false;
        calc.traits = ["One", "Two", "Three", "Four"].map(str::to_owned).to_vec();
        let frame = |calc: &mut Calculators, events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(760., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        calc.work_controls(ui, &d, &json!({}), "#work/efficiency", &mut None);
                    });
                },
            );
        };
        let click = |calc: &mut Calculators, key: &str| {
            frame(calc, vec![]);
            let pos = ctx
                .data(|m| {
                    m.get_temp::<egui::Rect>(egui::Id::new(format!("test-widget:trait:{key}")))
                })
                .unwrap()
                .center();
            for pressed in [true, false] {
                frame(
                    calc,
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
            }
        };
        click(&mut calc, "Five");
        assert_eq!(calc.traits.len(), 4);
        assert!(!calc.traits.iter().any(|k| k == "Five"));
        click(&mut calc, "One");
        assert_eq!(calc.traits.len(), 3);
        click(&mut calc, "Five");
        assert_eq!(calc.traits.len(), 4);
        assert!(calc.traits.iter().any(|k| k == "Five"));
        remove_fixture(d);
    }
    #[test]
    fn cumulative_keeps_tiny_probabilities() {
        let p = 1e-20;
        assert!((cumulative(p, 999) - 999. * p).abs() < 1e-30);
        assert_eq!(cumulative(0., 5), 0.);
        assert_eq!(cumulative(1., 5), 1.);
        assert!((cumulative(0.2, 5) - (1. - 0.8_f64.powi(5))).abs() < 1e-12);
    }
    #[test]
    fn route_segments_and_small_probabilities() {
        assert_eq!(decode("%E7%94%9F%E7%81%AB"), "生火");
        assert_eq!(decode("%生火"), "%生火");
        assert_eq!(decode("SheepBall"), "SheepBall");
        assert_eq!(percent(1e-10), "<0.01%");
        assert_eq!(percent(0.), "0%");
    }
    #[test]
    fn stock_sources_refresh_individual_traits_and_drop_removed_pals() {
        let d = fixture();
        let mut world = json!({"id":"world-a","inventory":{"records":[{"id":"one","speciesKey":"SheepBall","traits":["Artisan"]},{"id":"two","speciesKey":"SheepBall","traits":["Swift"]}]}});
        let mut calc = Calculators::default();
        calc.work_origin = "warehouse".into();
        calc.work_source = "two".into();
        calc.pick_work(&d, &world, Some("牧场"));
        assert_eq!(calc.work_level, 1);
        assert_eq!(calc.traits, vec!["Swift"]);
        world["inventory"]["records"][1]["traits"] = json!(["Artisan"]);
        frame(&mut calc, &d, "work/efficiency", &world, "#work/efficiency");
        assert_eq!(calc.traits, vec!["Artisan"]);
        world["inventory"]["records"] = json!([world["inventory"]["records"][0].clone()]);
        frame(&mut calc, &d, "work/efficiency", &world, "#work/efficiency");
        assert!(calc.work_source.is_empty());
        assert!(calc.traits.is_empty());
        assert_eq!(calc.work_level, 0);
        remove_fixture(d);
    }
    #[test]
    fn work_deep_link_waits_for_profiles_and_uses_requested_job() {
        let mut d = fixture();
        let profile = d.tables.remove("paldex-index-data").unwrap();
        let mut calc = Calculators::default();
        let route = "#work/efficiency/Fox/%E7%94%9F%E7%81%AB";
        frame(&mut calc, &d, "work/efficiency", &json!({}), route);
        assert!(calc.work_route.is_empty());
        d.tables.insert("paldex-index-data".into(), profile);
        frame(&mut calc, &d, "work/efficiency", &json!({}), route);
        assert_eq!(calc.work_origin, "dex");
        assert_eq!(calc.work_source, "Fox");
        assert_eq!(calc.job, "生火");
        assert_eq!(calc.work_level, 2);
        remove_fixture(d);
    }
    #[test]
    fn suitability_deep_link_selects_the_species_first_job() {
        let d = fixture();
        let mut calc = Calculators::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                calc.suitability(ui, &d, "#work/suitability/SheepBall");
            });
        });
        assert_eq!(calc.job, "牧场");
        assert_eq!(calc.query, "棉悠悠");
        remove_fixture(d);
    }
    #[test]
    fn relic_reverse_and_tier_deep_links_set_mode() {
        let mut d = fixture();
        d.tables.insert("relic-data".into(),std::sync::Arc::new(json!({"tiers":[{"id":"Relic1","slots":[]},{"id":"Relic2","slots":[]},{"id":"Relic3","slots":[]},{"id":"Relic4","slots":[]},{"id":"Relic5","slots":[]}],"outputs":["Wood","Stone"],"pools":[]})));
        let mut calc = Calculators::default();
        frame(
            &mut calc,
            &d,
            "work/relic",
            &json!({}),
            "#work/relic/item/Stone",
        );
        assert!(calc.relic_reverse);
        assert_eq!(calc.relic_output, "Stone");
        frame(
            &mut calc,
            &d,
            "work/relic",
            &json!({}),
            "#work/relic/Relic5",
        );
        assert!(!calc.relic_reverse);
        assert_eq!(calc.relic, 4);
        remove_fixture(d);
    }
}
