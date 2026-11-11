#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod breeding_ui;
mod calculators_ui;
mod catalog;
mod data;
mod instance;
mod links;
mod map;
mod qa;
mod store;
mod updates;
mod worlds_ui;
use data::{Data, array, text};
use eframe::egui::{self, Color32, RichText, Vec2};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SECTIONS: &[(&str, &str, &[(&str, &str)])] = &[
    (
        "配种手册",
        "lookup",
        &[
            ("配种查询", "lookup"),
            ("配种仓库", "warehouse"),
            ("可配子代", "offspring"),
            ("目标配种", "planner"),
        ],
    ),
    (
        "世界地图",
        "map",
        &[("地图点位", "map"), ("帕鲁刷新位置", "spawns")],
    ),
    ("帕鲁图鉴", "paldex", &[("帕鲁图鉴", "paldex")]),
    (
        "单位图鉴",
        "enemies",
        &[
            ("敌人图鉴", "enemies"),
            ("商人图鉴", "merchants"),
            ("其他中立单位", "neutrals"),
        ],
    ),
    (
        "帕鲁详情",
        "details/active",
        &[
            ("主动技能", "details/active"),
            ("被动技能", "details/passive"),
            ("伙伴技能", "details/partner"),
            ("技能果实", "details/fruits"),
            ("帕鲁手术台", "details/surgery"),
            ("属性值计算", "details/stats"),
            ("捕捉概率", "details/capture"),
        ],
    ),
    (
        "帕鲁工作",
        "work/suitability",
        &[
            ("工作适应性", "work/suitability"),
            ("设施", "work/facilities"),
            ("据点生产", "work/production"),
            ("工作效率", "work/efficiency"),
            ("牧场产物", "work/ranch"),
            ("遗物转换", "work/relic"),
            ("远征所", "work/expeditions"),
        ],
    ),
    ("物品图鉴", "items", &[("物品查询", "items")]),
    (
        "冒险指南",
        "guide/elements",
        &[
            ("属性克制", "guide/elements"),
            ("异常状态", "guide/status"),
            ("野外采集", "guide/gathering"),
            ("探索奖励表", "guide/loot"),
            ("任务详情", "guide/quests"),
            ("探索手记", "guide/journals"),
        ],
    ),
    (
        "设置",
        "settings/skins",
        &[
            ("皮肤", "settings/skins"),
            ("公告与更新", "settings/updates"),
        ],
    ),
];
fn page_for(route: &str) -> String {
    let raw = route.trim_start_matches('#');
    SECTIONS
        .iter()
        .flat_map(|(_, _, tabs)| *tabs)
        .map(|(_, id)| *id)
        .filter(|p| raw == *p || raw.starts_with(&format!("{p}/")))
        .max_by_key(|p| p.len())
        .unwrap_or("lookup")
        .into()
}
fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.
}
fn decode_route(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
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

struct App {
    data: Data,
    store: store::Store,
    catalog: catalog::Catalog,
    breeding: breeding_ui::Breeding,
    calculators: calculators_ui::Calculators,
    worlds: worlds_ui::Worlds,
    map: map::MapState,
    spawns: map::MapState,
    route: String,
    history: Vec<String>,
    forward: Vec<String>,
    search_open: bool,
    search: String,
    search_kind: String,
    search_selected: usize,
    search_terms: Vec<String>,
    search_names: Vec<String>,
    search_focus: bool,
    about: bool,
    last_tables: usize,
    last_save: Instant,
    theme: String,
    frames: u64,
    started: Instant,
    qa_output: Option<PathBuf>,
    qa: Value,
    updates: updates::UpdateState,
    probe: qa::Probe,
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        root: PathBuf,
        data_root: PathBuf,
        qa_output: Option<PathBuf>,
    ) -> Result<Self, String> {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut fonts = egui::FontDefinitions::default();
        let windows = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:/Windows"));
        for file in ["msyh.ttc", "simhei.ttf"] {
            if let Ok(bytes) = std::fs::read(windows.join("Fonts").join(file)) {
                fonts
                    .font_data
                    .insert("chinese".into(), egui::FontData::from_owned(bytes).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "chinese".into());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("chinese".into());
                break;
            }
        }
        cc.egui_ctx.set_fonts(fonts);
        let data = Data::new(data_root, cc.egui_ctx.clone())?;
        let updates = updates::UpdateState::new(
            root.clone(),
            std::env::current_exe().map_err(|e| e.to_string())?,
        );
        let store = store::Store::open(root);
        Ok(Self {
            data,
            store,
            catalog: catalog::Catalog::default(),
            breeding: breeding_ui::Breeding::default(),
            calculators: calculators_ui::Calculators::default(),
            worlds: worlds_ui::Worlds::default(),
            map: map::MapState::default(),
            spawns: map::MapState::default(),
            route: std::env::var("JADE_QA_ROUTE").unwrap_or("#lookup".into()),
            history: vec![],
            forward: vec![],
            search_open: false,
            search: String::new(),
            search_kind: String::new(),
            search_selected: 0,
            search_terms: vec![],
            search_names: vec![],
            search_focus: false,
            about: false,
            last_tables: 0,
            last_save: Instant::now(),
            theme: String::new(),
            frames: 0,
            started: Instant::now(),
            qa_output,
            qa: json!({"runtime":"Rust native egui/glow","version":"2.0.0-beta.1","createdAt":now_ms()}),
            updates,
            probe: qa::Probe::default(),
        })
    }
    fn go(&mut self, route: String) {
        if route == self.route {
            return;
        }
        self.history.push(self.route.clone());
        self.forward.clear();
        self.activate(route);
    }
    fn activate(&mut self, route: String) {
        self.route = decode_route(&route);
        self.search_open = false;
        let page = page_for(&self.route);
        let key = self
            .route
            .trim_start_matches('#')
            .strip_prefix(&page)
            .unwrap_or("")
            .trim_start_matches('/');
        if let Some(i) = self.data.species.iter().position(|p| text(p, "key") == key) {
            if page == "lookup" {
                self.breeding.lookup_pal = i;
            } else if page == "planner" {
                self.breeding.pal = i;
            }
            if page == "details/stats" || page == "details/capture" {
                self.calculators.pal = i;
            }
        }
    }
    fn back(&mut self) {
        if let Some(r) = self.history.pop() {
            self.forward.push(self.route.clone());
            self.activate(r);
        }
    }
    fn next(&mut self) {
        if let Some(r) = self.forward.pop() {
            self.history.push(self.route.clone());
            self.activate(r);
        }
    }
    fn new_window(&self) {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--new-window")
            .arg("--data-dir")
            .arg(&self.store.root)
            .arg("--resources")
            .arg(&self.data.root)
            .creation_flags(0x08000000)
            .spawn();
    }
    fn apply_theme(&mut self, ctx: &egui::Context) {
        let id = self.store.state["theme"]
            .as_str()
            .unwrap_or("moon")
            .to_owned();
        if id == self.theme {
            return;
        }
        self.theme = id.clone();
        let accent = match id.as_str() {
            "jade" => Color32::from_rgb(60, 139, 115),
            "rose" => Color32::from_rgb(181, 106, 143),
            "amber" => Color32::from_rgb(164, 126, 50),
            "slate" => Color32::from_rgb(104, 119, 144),
            "blue" => Color32::from_rgb(62, 132, 186),
            _ => Color32::from_rgb(131, 108, 202),
        };
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = if id == "moon" {
            Color32::from_rgb(220, 219, 239)
        } else {
            Color32::from_rgb(228, 234, 235)
        };
        visuals.window_fill = Color32::from_rgb(237, 236, 246);
        visuals.extreme_bg_color = Color32::from_rgb(229, 229, 241);
        visuals.selection.bg_fill = accent;
        visuals.hyperlink_color = accent;
        visuals.widgets.active.bg_fill = accent;
        visuals.override_text_color = Some(Color32::from_rgb(40, 48, 69));
        ctx.set_visuals(visuals);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = Vec2::new(10., 9.);
            s.spacing.button_padding = Vec2::new(12., 8.);
            s.text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(16.));
            s.text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(16.));
            s.text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.));
        });
    }
    fn search_dialog(&mut self, ctx: &egui::Context) -> Option<String> {
        let mut nav = None;
        if !self.search_open {
            return None;
        }
        let mut open = true;
        egui::Window::new("全局搜索")
            .open(&mut open)
            .default_width(840.)
            .default_height(640.)
            .show(ctx, |ui| {
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("中文名、英文名或游戏编号")
                        .desired_width(f32::INFINITY),
                );
                if self.search_focus {
                    input.request_focus();
                    self.search_focus = false;
                }
                if input.changed() {
                    self.search_selected = 0;
                }
                let rows = array(self.data.get("search-index-data"));
                if self.search_terms.len() != rows.len() {
                    self.search_terms = rows
                        .iter()
                        .map(|r| data::normalize(&r.to_string()))
                        .collect();
                    self.search_names = rows
                        .iter()
                        .map(|r| data::normalize(text(r, "name")))
                        .collect();
                }
                let kinds: std::collections::BTreeSet<_> =
                    rows.iter().map(|r| text(r, "kind")).collect();
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.search_kind, String::new(), "全部");
                    for k in kinds {
                        ui.selectable_value(&mut self.search_kind, k.into(), k);
                    }
                });
                let q = data::normalize(&self.search);
                let mut found: Vec<_> = rows
                    .iter()
                    .enumerate()
                    .filter(|(index, r)| {
                        (self.search_kind.is_empty() || text(r, "kind") == self.search_kind)
                            && q.split_whitespace()
                                .all(|word| self.search_terms[*index].contains(word))
                    })
                    .collect();
                found.sort_by_cached_key(|(index, _)| {
                    let name = &self.search_names[*index];
                    if *name == q {
                        0
                    } else if name.starts_with(&q) {
                        1
                    } else {
                        2
                    }
                });
                let found: Vec<_> = found.into_iter().map(|(_, r)| r).collect();
                self.search_selected = self.search_selected.min(found.len().saturating_sub(1));
                let down =
                    ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                let up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                if down {
                    self.search_selected =
                        (self.search_selected + 1).min(found.len().saturating_sub(1));
                }
                if up {
                    self.search_selected = self.search_selected.saturating_sub(1);
                }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                    if let Some(r) = found.get(self.search_selected) {
                        nav = Some(text(r, "href").into());
                    }
                }
                ui.label(format!("{} 条结果", found.len()));
                let mut scroll = egui::ScrollArea::vertical();
                if up || down {
                    scroll = scroll.vertical_scroll_offset(self.search_selected as f32 * 45.);
                }
                scroll.show_rows(ui, 45., found.len(), |ui, range| {
                    for i in range {
                        let r = found[i];
                        ui.horizontal(|ui| {
                            ui.weak(text(r, "kind"));
                            if ui
                                .selectable_label(i == self.search_selected, text(r, "name"))
                                .clicked()
                            {
                                nav = Some(text(r, "href").into());
                            }
                            ui.small(text(r, "hint"));
                        });
                    }
                });
            });
        self.search_open = open;
        nav
    }
    fn settings(&mut self, ui: &mut egui::Ui, page: &str) {
        if page == "settings/skins" {
            ui.heading("选择界面皮肤");
            ui.horizontal_wrapped(|ui| {
                for (id, label) in [
                    ("moon", "辉月伊"),
                    ("jade", "玉绿"),
                    ("blue", "海蓝"),
                    ("rose", "玫瑰"),
                    ("amber", "琥珀"),
                    ("slate", "岩灰"),
                ] {
                    if ui.selectable_label(self.theme == id, label).clicked() {
                        self.store.state["theme"] = json!(id);
                        self.store.changed();
                    }
                }
            });
            self.data.portrait(ui, "MoonQueen", 180.);
        } else {
            let before = self.store.state.clone();
            if self.updates.show(ui, &mut self.store.state) == Some(updates::UpdateAction::Restart)
            {
                self.store.changed();
                self.store.flush();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if before != self.store.state {
                self.store.changed();
            }
        }
    }
}
impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        self.probe.raw_input_hook(input);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.probe.begin_frame();
        self.frames += 1;
        if self.last_save.elapsed()>Duration::from_secs(1){self.store.flush();self.last_save=Instant::now();}
        ctx.request_repaint_after(Duration::from_secs(1));
        self.data.poll();
        self.data.preload(ctx);
        self.worlds.tick(&mut self.store);
        if self.frames == 1 && std::env::var_os("JADE_QA_WORLDS").is_some() {
            self.worlds.open = true;
        }
        self.apply_theme(ctx);
        if self.qa_output.is_none() && std::env::var_os("JADE_QA_OUTPUT").is_none() {
            let read_before = array(&self.store.state["readAnnouncements"]).len();
            if self.updates.tick(ctx, &mut self.store.state) == Some(updates::UpdateAction::Restart)
            {
                self.store.changed();
                self.store.flush();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if array(&self.store.state["readAnnouncements"]).len() != read_before {
                self.store.changed();
            }
        }
        if self.last_tables != self.data.tables.len() && self.data.pending == 0 {
            self.catalog.rebuild(&self.data);
            self.last_tables = self.data.tables.len();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::K)) {
            self.search_open = true;
            self.search_focus = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft)) {
            self.back();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight)) {
            self.next();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) {
            self.new_window();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.search_open = false;
        }
        let page = page_for(&self.route);
        let section = SECTIONS
            .iter()
            .position(|(_, _, tabs)| tabs.iter().any(|(_, p)| *p == page))
            .unwrap_or(0);
        let mut nav = None;
        egui::TopBottomPanel::top("titlebar")
            .exact_height(40.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let title =
                        ui.label(RichText::new("Jade · 幻兽帕鲁工具箱  |  Rust 测试版").strong());
                    if title.interact(egui::Sense::drag()).drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("×").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("□").clicked() {
                            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                        }
                        if ui.button("—").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
        egui::SidePanel::left("navigation")
            .resizable(false)
            .exact_width(190.)
            .show(ctx, |ui| {
                ui.add_space(12.);
                ui.heading("Jade");
                ui.small("幻兽帕鲁工具箱");
                ui.add_space(12.);
                if ui.button("全局搜索   Ctrl+K").clicked() {
                    self.search_open = true;
                    self.search_focus = true;
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("sidebar")
                    .max_height((ui.available_height() - 145.).max(100.))
                    .show(ui, |ui| {
                        for (i, (name, route, _)) in SECTIONS.iter().enumerate() {
                            if ui
                                .add_sized(
                                    [170., 40.],
                                    egui::Button::new(*name).selected(i == section),
                                )
                                .clicked()
                            {
                                nav = Some(format!("#{route}"));
                            }
                        }
                    });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if ui.button("数据说明").clicked() {
                        self.about = true;
                    }
                    if ui
                        .button(format!("世界档案 · {}", text(self.store.world(), "name")))
                        .clicked()
                    {
                        self.worlds.open = true;
                    }
                    if self.data.pending > 0 {
                        ui.small(format!("后台准备资料 · 剩余 {} 项", self.data.pending));
                    } else {
                        ui.small("● 离线资料已就绪");
                    }
                });
            });
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.history.is_empty(), egui::Button::new("← 返回"))
                    .clicked()
                {
                    self.back();
                }
                if ui
                    .add_enabled(!self.forward.is_empty(), egui::Button::new("前进 →"))
                    .clicked()
                {
                    self.next();
                }
                for (name, id) in SECTIONS[section].2 {
                    if ui.selectable_label(page == *id, *name).clicked() {
                        nav = Some(format!("#{id}"));
                    }
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let heading = SECTIONS[section]
                .2
                .iter()
                .find(|(_, id)| *id == page)
                .map(|(name, _)| *name)
                .unwrap_or("工具箱");
            ui.heading(heading);
            ui.add_space(6.);
            if let Some(e) = &self.store.error {
                ui.colored_label(Color32::DARK_RED, e);
            }
            for error in &self.data.errors {
                ui.colored_label(Color32::DARK_RED, error);
            }
            if ["warehouse", "planner", "offspring"].contains(&page.as_str()) {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.worlds.busy, egui::Button::new("↻ 刷新现有帕鲁"))
                        .clicked()
                    {
                        self.worlds.refresh(ctx, &self.store);
                    }
                    ui.small(&self.worlds.message);
                });
            }
            let next = match page.as_str() {
                "lookup" => self.breeding.lookup(ui, &self.data, &mut self.store),
                "warehouse" => self.breeding.warehouse(ui, &self.data, &mut self.store),
                "planner" | "offspring" => {
                    self.breeding
                        .planner(ui, &self.data, &mut self.store, page == "offspring")
                }
                "map" | "spawns" => {
                    self.map.set_world_id(text(self.store.world(), "id"));
                    self.spawns.set_world_id(text(self.store.world(), "id"));
                    let notes = &self.store.world()["mapNotes"];
                    let mut edited = notes.clone();
                    let next = if page == "map" {
                        self.map.route = self.route.clone();
                        map::show(ui, &self.data, &mut self.map, &mut edited)
                    } else {
                        self.spawns.route = self.route.clone();
                        map::show_spawns(ui, &self.data, &mut self.spawns, &mut edited)
                    };
                    if &edited != notes {
                        self.store.world_mut()["mapNotes"] = edited;
                    }
                    next
                }
                "details/stats" | "details/capture" | "work/efficiency" | "work/relic" => self
                    .calculators
                    .show(ui, &self.data, &page, self.store.world(), &self.route),
                "work/suitability" => self.calculators.suitability(ui, &self.data, &self.route),
                "guide/elements" => catalog::elements(ui, &self.data),
                "settings/skins" | "settings/updates" => {
                    self.settings(ui, &page);
                    None
                }
                _ => {
                    let mut surgery = self.store.world()["surgery"].clone();
                    let next = self
                        .catalog
                        .show(ui, &self.data, &page, &self.route, &mut surgery);
                    if surgery != self.store.world()["surgery"] {
                        self.store.world_mut()["surgery"] = surgery;
                    }
                    next
                }
            };
            if next.is_some() {
                nav = next;
            }
        });
        if let Some(route) = self.search_dialog(ctx) {
            nav = Some(route);
        }
        self.worlds.show(ctx, &mut self.store, &self.data);
        if self.about {
            egui::Window::new("数据说明")
                .open(&mut self.about)
                .show(ctx, |ui| {
                    ui.label("非官方玩家工具。游戏美术与文本归 Pocketpair 等原权利人所有。");
                    ui.label("原生 Rust 界面与计算；测试版独立保存数据。");
                    catalog::render_value(
                        ui,
                        &self.data,
                        &self.data.get("db")["meta"]["sources"],
                        0,
                        &mut nav,
                    );
                });
        }
        if let Some(route) = nav {
            self.go(route);
        }
        if self.last_save.elapsed() > Duration::from_secs(1) {
            self.store.flush();
            self.last_save = Instant::now();
        }
        if self.frames == 2 {
            updates::mark_ready();
            self.qa["firstUiAt"] = json!(now_ms());
            self.qa["palCount"] = json!(array(&self.store.world()["inventory"]["records"]).len());
            self.qa["dataSpecies"] = json!(self.data.species.len());
            self.qa["pairs"] = json!(self.data.pairs.len());
        }
        if let Some(output) = &self.qa_output {
            ctx.request_repaint_after(Duration::from_millis(16));
            if self.started.elapsed() > Duration::from_secs(7) {
                self.qa["tables"] = json!(self.data.tables.len());
                self.qa["errors"] = json!(self.data.errors);
                let _ = std::fs::write(output, serde_json::to_vec_pretty(&self.qa).unwrap());
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self
            .probe
            .tick(ctx, &mut self.route, &self.data, &self.store)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.store.flush();
    }
}
fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = updates::handle_cli(&args) {
        std::process::exit(code);
    }
    let arg = |name: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let exe = std::env::current_exe().unwrap();
    let root = arg("--data-dir").unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
            .join("JadePalworldRustBeta")
    });
    if updates::apply_pending_on_start(&root, &exe).unwrap_or(false) {
        return Ok(());
    }
    let Some(_instance) = instance::acquire(&root, args.iter().any(|a| a == "--new-window")) else {
        return Ok(());
    };
    let resources = arg("--resources").unwrap_or_else(|| {
        let adjacent = exe.parent().unwrap().join("data");
        if adjacent.exists() || !(cfg!(debug_assertions)||arg("--qa-output").is_some()||std::env::var_os("JADE_QA_OUTPUT").is_some()) {
            adjacent
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
        }
    });
    let qa = arg("--qa-output");
    let icon = image::load_from_memory(include_bytes!("../assets/atlas.ico"))
        .ok()
        .map(|i| {
            let i = i.into_rgba8();
            egui::IconData {
                width: i.width(),
                height: i.height(),
                rgba: i.into_raw(),
            }
        })
        .unwrap_or_default();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440., 900.])
            .with_min_inner_size([960., 620.])
            .with_decorations(false)
            .with_icon(std::sync::Arc::new(icon))
            .with_app_id("JadePalworldRustBeta"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let result = eframe::run_native(
        instance::TITLE,
        options,
        Box::new(move |cc| {
            App::new(cc, root, resources, qa)
                .map(|a| Box::new(a) as Box<dyn eframe::App>)
                .map_err(|e| e.into())
        }),
    );
    if let Err(e) = &result {
        instance::show_error(&e.to_string());
    }
    result
}
#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn unicode_routes_and_all_search_destinations() {
        assert_eq!(
            decode_route("#guide/loot/%E7%94%9F%E7%81%AB"),
            "#guide/loot/生火"
        );
        assert_eq!(decode_route("#items/%生火"), "#items/%生火");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root, egui::Context::default()).unwrap();
        let start = Instant::now();
        while d.pending > 0 && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
            d.poll();
        }
        assert!(d.errors.is_empty());
        assert_eq!(d.pending, 0);
        let mut c = catalog::Catalog::default();
        c.rebuild(&d);
        let mut missed = vec![];
        for r in array(d.get("search-index-data")) {
            let route = decode_route(text(r, "href"));
            let page = page_for(&route);
            let wanted = route
                .trim_start_matches('#')
                .strip_prefix(&page)
                .unwrap_or("")
                .trim_start_matches('/');
            let wanted = wanted
                .strip_prefix("move/")
                .or_else(|| wanted.strip_prefix("trait/"))
                .unwrap_or(wanted);
            if !c.pages.get(&page).is_some_and(|entries| {
                entries.iter().any(|e| {
                    e.id == wanted
                        || page == "guide/loot" && e.id.starts_with(&format!("{wanted}/"))
                })
            }) {
                missed.push(route);
            }
        }
        assert!(missed.is_empty(), "Missing destinations: {missed:?}");
        assert_eq!(array(d.get("search-index-data")).len(), 5110);
        assert_eq!(c.pages["paldex"].len(), 300);
        assert_eq!(c.pages["items"].len(), 2466);
        assert_eq!(c.pages["merchants"].len(), 43);
        assert_eq!(c.pages["details/partner"].len(), 298);
        assert_eq!(c.pages["guide/journals"].len(), 64);
    }
}
