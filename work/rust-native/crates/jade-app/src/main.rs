#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod breeding_ui;
mod calculators_ui;
mod catalog;
mod catalog_views;
mod chrome;
mod data;
mod design;
mod elements;
mod external;
mod game_art;
mod instance;
mod links;
mod map;
mod navigation;
mod qa;
mod search;
mod search_rows;
mod skins;
mod store;
mod terrain;
mod updates;
mod work_views;
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
fn map_origin<'a>(
    history: impl DoubleEndedIterator<Item = &'a str>,
) -> Option<(&'a str, &'static str)> {
    let route = history
        .rev()
        .find(|route| !matches!(page_for(route).as_str(), "map" | "spawns"))?;
    if !route.trim_start_matches('#').contains('/') {
        return None;
    }
    let source = page_for(route);
    let label = SECTIONS
        .iter()
        .flat_map(|(_, _, tabs)| tabs.iter())
        .find(|(_, id)| *id == source)
        .map(|(label, _)| *label)?;
    Some((route, label))
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

fn page_header(
    ui: &mut egui::Ui,
    page: &str,
    heading: &str,
    d: &Data,
    origin: Option<(&str, &str)>,
) -> Option<String> {
    let p = design::palette(ui.ctx());
    if ["lookup", "warehouse", "offspring", "planner"].contains(&page) {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 100.), egui::Sense::hover());
        ui.painter().rect_filled(rect, 14, p.soft);
        let artwork = egui::Rect::from_min_max(
            egui::pos2(rect.left() + rect.width() * 0.36, rect.top()),
            rect.max,
        );
        let crop = (artwork.height() / artwork.width() * (1536. / 864.)).min(1.);
        let top = (1. - crop) * 0.2;
        egui::Image::new(egui::include_image!("../assets/hero.webp"))
            .uv(egui::Rect::from_min_max(
                egui::pos2(0., top),
                egui::pos2(1., top + crop),
            ))
            .corner_radius(14)
            .tint(Color32::from_white_alpha(165))
            .paint_at(ui, artwork);
        let fade = egui::Rect::from_min_max(
            artwork.min,
            egui::pos2(artwork.left() + artwork.width() * 0.25, artwork.bottom()),
        );
        design::gradient(ui.painter(), fade, &[p.soft, Color32::TRANSPARENT], true);
        ui.painter().rect_stroke(
            rect,
            14,
            egui::Stroke::new(1., p.line),
            egui::StrokeKind::Inside,
        );
        let title = if page == "lookup" {
            "帕鲁配种查询"
        } else {
            heading
        };
        let subtitle = match page {
            "warehouse" => "查看每只帕鲁的词条、性别和个体名称。",
            "offspring" => "从仓库出发，查找指定步数内可配出的帕鲁。",
            "planner" => "选择目标和词条，规划仓库帕鲁的配种路线。",
            _ => "选一只帕鲁，查找配种伙伴，或反查它的双亲。",
        };
        let mut lines = vec![(15., 28., title.to_owned()), (51., 13., subtitle.into())];
        if page == "lookup" {
            lines.push((
                74.,
                12.,
                format!(
                    "{} 个图鉴条目    ·    {} 条配方",
                    d.species.len(),
                    d.pairs.len()
                ),
            ));
        }
        for (dy, size, text) in lines {
            ui.painter().text(
                rect.min + Vec2::new(22., dy),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(size),
                p.ink,
            );
        }
        ui.add_space(6.);
        return None;
    }
    let description = match page {
        "warehouse" => "逐只管理你的帕鲁，保存昵称、性别和词条；数据按世界独立保存。",
        "offspring" => "从当前世界的仓库出发，查看指定步数内能配出的帕鲁与路线。",
        "planner" => "选择目标帕鲁与词条，使用仓库中的帕鲁规划配种并记录孵化进度。",
        "map" => "筛选点位、查看等级与奖励，标记完成情况或添加自己的点位。",
        "spawns" => "查找帕鲁的昼夜分布，查看刷新等级与相关资料。",
        "paldex" => "按属性、工作适应性和编号筛选，查看帕鲁的完整资料。",
        "items" => "查询获取方式、合成表和制作路径，跳转到对应地图点位与资料。",
        "details/active" => "查找技能威力、冷却与学习方式，也可以按具体帕鲁查看。",
        "details/passive" => "查询词条效果，或选择帕鲁查看其固定被动。",
        "details/partner" => "按图鉴编号排列，筛选技能类型并查看各等级效果。",
        "details/surgery" => "查看手术条件，标记已有植入体，并在目标配种中使用。",
        "details/fruits" => "查看技能果实效果，以及能够获取果实的位置与来源。",
        "details/stats" => "计算帕鲁的生命、攻击、防御与远征属性。",
        "details/capture" => "根据目标、帕鲁球与增益条件计算捕捉概率。",
        "work/suitability" => "按工作种类查看帕鲁，工作等级从高到低排列。",
        "work/efficiency" => "按工作适应性等级、帕鲁词条与当前条件计算工作效率。",
        "work/facilities" => "查询设施要求、建造材料与可生产的物品。",
        "work/production" => "查找据点设施、生产配方与相关工作适应性。",
        "work/ranch" => "按帕鲁查看牧场产物，或从材料反查生产它的帕鲁。",
        "work/relic" => "查看遗物转换各档奖励，反查所需材料与获取方式。",
        "work/expeditions" => "查看远征要求、推荐属性与可能获得的物品。",
        "guide/journals" => "阅读探索手记原文，并在世界地图中定位。",
        "guide/quests" => "查看任务条件、前后任务与奖励。",
        "guide/gathering" => "查看采集产物与等级要求，定位对应的野外资源。",
        "guide/loot" => "按地点查看探索奖励与各栏位掉落。",
        "guide/elements" => "查看游戏中的属性克制关系。",
        "guide/status" => "查询异常状态效果与相关主动技能。",
        "enemies" => "查找敌人等级、掉落物与地图位置。",
        "merchants" => "查询商人出售的物品或帕鲁，定位其所在位置。",
        "neutrals" => "查看其他中立单位的相关资料。",
        "settings/skins" => "选择辉月伊配色，或使用普通颜色主题。",
        _ => "",
    };
    let mut nav = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(heading)
                .size(if page == "map" || page == "spawns" {
                    24.
                } else {
                    30.
                })
                .strong(),
        );
        if let Some((route, label)) = origin {
            if design::small_action(ui, "left", &format!("返回{label}")).clicked() {
                nav = Some(route.to_owned());
            }
        }
    });
    if !description.is_empty() {
        ui.label(RichText::new(description).size(13.).color(p.muted));
    }
    ui.add_space(5.);
    nav
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
    history: navigation::History<ViewBookmark>,
    ui_ctx: egui::Context,
    search_open: bool,
    search: String,
    search_kind: String,
    search_selected: usize,
    search_index: Option<search::GlobalIndex>,
    search_index_tables: usize,
    search_hits: Vec<search::SearchHit>,
    search_cache_key: Option<(String, String)>,
    search_kinds: Vec<String>,
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
    chrome: chrome::WindowChrome,
    external: external::Directory,
}
#[derive(Clone)]
struct ViewBookmark {
    world_id: String,
    view: ViewState,
    scrolls: Vec<navigation::ScrollBookmark>,
}
#[derive(Clone)]
enum ViewState {
    Breeding(breeding_ui::BreedingViewState),
    Catalog(catalog::CatalogViewState),
    Calculators(calculators_ui::ViewState),
    Map(map::ViewState),
    Spawns(map::ViewState),
    Elements(Option<String>),
    Other,
}
#[derive(Clone, Copy)]
enum HistoryRequest {
    Dismissed,
    Back,
    Forward,
    BackSteps(usize),
    ForwardSteps(usize),
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
        Ok(Self::from_components(
            cc.egui_ctx.clone(),
            data,
            store,
            updates,
            qa_output,
        ))
    }
    fn from_components(
        ctx: egui::Context,
        data: Data,
        store: store::Store,
        updates: updates::UpdateState,
        qa_output: Option<PathBuf>,
    ) -> Self {
        Self {
            data,
            store,
            catalog: catalog::Catalog::default(),
            breeding: breeding_ui::Breeding::default(),
            calculators: calculators_ui::Calculators::default(),
            worlds: worlds_ui::Worlds::default(),
            map: map::MapState::default(),
            spawns: map::MapState::default(),
            route: std::env::var("JADE_QA_ROUTE").unwrap_or("#lookup".into()),
            history: navigation::History::default(),
            ui_ctx: ctx,
            search_open: false,
            search: String::new(),
            search_kind: String::new(),
            search_selected: 0,
            search_index: None,
            search_index_tables: 0,
            search_hits: vec![],
            search_cache_key: None,
            search_kinds: vec![],
            search_focus: false,
            about: false,
            last_tables: 0,
            last_save: Instant::now(),
            theme: String::new(),
            frames: 0,
            started: Instant::now(),
            qa_output,
            qa: json!({"runtime":"Rust native egui/glow","version":env!("CARGO_PKG_VERSION"),"createdAt":now_ms()}),
            updates,
            probe: qa::Probe::default(),
            chrome: chrome::WindowChrome::default(),
            external: external::Directory::default(),
        }
    }
    fn canonical_route(&self) -> String {
        let page = page_for(&self.route);
        let pal = match page.as_str() {
            "lookup" => Some(self.breeding.lookup_pal),
            "planner" => Some(self.breeding.pal),
            "details/stats" | "details/capture" => Some(self.calculators.pal),
            _ => None,
        };
        if let Some(p) = pal.and_then(|i| self.data.species.get(i)) {
            return format!(
                "#{page}/{}{}",
                if page == "lookup" && self.breeding.reverse {
                    "parents/"
                } else {
                    ""
                },
                text(p, "key")
            );
        }
        if page == "spawns" {
            return format!("#spawns/{}", self.spawns.current_species());
        }
        self.route.clone()
    }
    fn navigation_label(&self, route: &str) -> String {
        let page = page_for(route);
        let base = SECTIONS
            .iter()
            .flat_map(|(_, _, tabs)| *tabs)
            .find(|(_, id)| *id == page)
            .map(|(name, _)| *name)
            .unwrap_or("工具箱");
        let tail = route
            .trim_start_matches('#')
            .strip_prefix(&page)
            .unwrap_or("")
            .trim_start_matches('/');
        let key = tail.strip_prefix("parents/").unwrap_or(tail);
        let base = if page == "lookup" {
            if tail.starts_with("parents/") {
                "反查双亲"
            } else {
                "查配种伙伴"
            }
        } else {
            base
        };
        if let Some(pal) = self.data.pal(key) {
            return format!("{} · {base}", text(pal, "zh"));
        }
        if page == "map" {
            return self
                .map
                .selected_title()
                .map(|name| format!("{name} · {base}"))
                .unwrap_or(base.into());
        }
        let id = key
            .strip_prefix("move/")
            .or_else(|| key.strip_prefix("trait/"))
            .unwrap_or(key);
        let id = id.strip_suffix("/shops").unwrap_or(id);
        let id = if ["details/active", "details/passive"].contains(&page.as_str()) {
            id.split_once('/')
                .filter(|(pal, _)| self.data.pal(pal).is_some())
                .map(|(_, id)| id)
                .unwrap_or(id)
        } else {
            id
        };
        if let Some(entry) = self
            .catalog
            .pages
            .get(&page)
            .and_then(|rows| rows.iter().find(|e| e.id == id))
        {
            return format!("{} · {base}", entry.name);
        }
        base.into()
    }
    fn navigation_snapshot(&self) -> navigation::Entry<ViewBookmark> {
        let route = self.canonical_route();
        let page = page_for(&route);
        let view = match page.as_str() {
            "lookup" | "warehouse" | "planner" | "offspring" => {
                ViewState::Breeding(self.breeding.navigation_state(&page))
            }
            "map" => ViewState::Map(self.map.navigation_state()),
            "spawns" => ViewState::Spawns(self.spawns.navigation_state()),
            "details/stats" | "details/capture" | "work/efficiency" | "work/relic"
            | "work/suitability" => {
                ViewState::Calculators(self.calculators.navigation_state(&page))
            }
            "guide/elements" => ViewState::Elements(
                self.ui_ctx
                    .data(|m| m.get_temp::<String>(egui::Id::new("element-guide-selection"))),
            ),
            "settings/skins" | "settings/updates" => ViewState::Other,
            _ => ViewState::Catalog(self.catalog.navigation_state(&page)),
        };
        navigation::Entry {
            label: self.navigation_label(&route),
            route,
            state: ViewBookmark {
                world_id: text(self.store.world(), "id").into(),
                view,
                scrolls: navigation::capture_scrolls(&self.ui_ctx),
            },
        }
    }
    fn go(&mut self, route: String) {
        let target = decode_route(&route);
        let current = self.navigation_snapshot();
        self.go_from(target, current);
    }
    fn go_from(&mut self, target: String, current: navigation::Entry<ViewBookmark>) {
        let page = page_for(&current.route);
        let same_tool = target == format!("#{page}")
            && [
                "lookup",
                "planner",
                "details/stats",
                "details/capture",
                "work/efficiency",
                "work/relic",
                "work/suitability",
                "work/ranch",
                "map",
                "spawns",
            ]
            .contains(&page.as_str());
        if same_tool || !self.history.record(current, &target) {
            return;
        }
        self.activate(target);
    }
    fn restore_entry(&mut self, entry: navigation::Entry<ViewBookmark>) {
        self.route = entry.route;
        self.search_open = false;
        let same_world = entry.state.world_id == text(self.store.world(), "id");
        let world_scoped = matches!(
            &entry.state.view,
            ViewState::Breeding(_) | ViewState::Map(_) | ViewState::Spawns(_)
        ) || page_for(&self.route) == "work/efficiency";
        if same_world || !world_scoped {
            match &entry.state.view {
                ViewState::Breeding(view) => self.breeding.restore_navigation(view, &self.route),
                ViewState::Map(view) => {
                    self.map.set_world_id(text(self.store.world(), "id"));
                    self.map.restore_navigation(view, &self.route);
                }
                ViewState::Spawns(view) => {
                    self.spawns.set_world_id(text(self.store.world(), "id"));
                    self.spawns.restore_navigation(view, &self.route);
                }
                ViewState::Calculators(view) => {
                    self.calculators.restore_navigation(view, &self.route)
                }
                ViewState::Catalog(view) => self.catalog.restore_navigation(view, &self.route),
                ViewState::Elements(value) => self.ui_ctx.data_mut(|m| {
                    if let Some(value) = value {
                        m.insert_temp(egui::Id::new("element-guide-selection"), value.clone());
                    }
                }),
                ViewState::Other => {}
            }
            navigation::restore_scrolls(&self.ui_ctx, &entry.state.scrolls);
        } else {
            self.activate(self.route.clone());
        }
        self.ui_ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        self.ui_ctx.request_repaint();
    }
    fn has_overlay(&self) -> bool {
        let page = page_for(&self.route);
        egui::Popup::is_any_open(&self.ui_ctx)
            || self.search_open
            || self.about
            || self.worlds.open
            || self.updates.has_overlay()
            || (["lookup", "warehouse", "planner", "offspring"].contains(&page.as_str())
                && self.breeding.has_overlay_for(&page))
            || ([
                "details/stats",
                "details/capture",
                "work/efficiency",
                "work/relic",
            ]
            .contains(&page.as_str())
                && self.calculators.picker)
            || (page == "map" && self.map.has_overlay())
            || (page == "spawns" && self.spawns.has_overlay())
    }
    fn dismiss_overlay(&mut self) -> bool {
        if egui::Popup::is_any_open(&self.ui_ctx) {
            egui::Popup::close_all(&self.ui_ctx);
            return true;
        }
        if self.worlds.dismiss_overlay() {
            return true;
        }
        if self.updates.dismiss_overlay() {
            return true;
        }
        if self.about {
            self.about = false;
            return true;
        }
        if self.search_open {
            self.search_open = false;
            return true;
        }
        let page = page_for(&self.route);
        if ["lookup", "warehouse", "planner", "offspring"].contains(&page.as_str())
            && self.breeding.dismiss_overlay_for(&page)
        {
            return true;
        }
        if [
            "details/stats",
            "details/capture",
            "work/efficiency",
            "work/relic",
        ]
        .contains(&page.as_str())
            && self.calculators.picker
        {
            self.calculators.picker = false;
            return true;
        }
        if page == "map" {
            return self.map.dismiss_overlay();
        }
        if page == "spawns" {
            return self.spawns.dismiss_overlay();
        }
        false
    }
    fn back(&mut self) {
        if self.dismiss_overlay() {
            self.ui_ctx.request_repaint();
            return;
        }
        self.travel_back(1);
    }
    fn travel_back(&mut self, steps: usize) {
        if !self.history_travel_allowed() {
            return;
        }
        let current = self.navigation_snapshot();
        if let Some(entry) = self.history.back_steps(current, steps) {
            self.restore_entry(entry);
        }
    }
    fn next(&mut self) {
        if self.dismiss_overlay() {
            self.ui_ctx.request_repaint();
            return;
        }
        self.travel_forward(1);
    }
    fn travel_forward(&mut self, steps: usize) {
        if !self.history_travel_allowed() {
            return;
        }
        let current = self.navigation_snapshot();
        if let Some(entry) = self.history.forward_steps(current, steps) {
            self.restore_entry(entry);
        }
    }
    fn history_travel_allowed(&mut self) -> bool {
        if self.has_overlay() {
            self.dismiss_overlay();
            if self.has_overlay() {
                self.breeding.notice = "已有暂存草稿，请先保存或关闭当前编辑窗口后再返回。".into();
                self.ui_ctx.request_repaint();
                return false;
            }
        }
        true
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
        let (key, reverse) = if page == "lookup" {
            key.strip_prefix("parents/")
                .map(|key| (key, true))
                .unwrap_or((key, false))
        } else {
            (key, false)
        };
        if let Some(i) = self.data.species.iter().position(|p| text(p, "key") == key) {
            if page == "lookup" {
                self.breeding.activate_lookup(i, reverse);
            } else if page == "planner" {
                self.breeding.pal = i;
            }
            if page == "details/stats" || page == "details/capture" {
                self.calculators.activate_pal(&page, i);
            }
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
        design::apply(ctx, &id);
    }
    fn search_dialog(&mut self, ctx: &egui::Context) -> Option<String> {
        let mut nav = None;
        if !self.search_open {
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.search_open = false;
            return None;
        }
        let mut open = true;
        let mut close_search = false;
        let search_size = Vec2::new(
            (ctx.content_rect().width() - 48.).clamp(280., 960.),
            (ctx.content_rect().height() - 80.).clamp(240., 640.),
        );
        egui::Window::new("全局搜索")
            .open(&mut open)
            .title_bar(false)
            .default_size(search_size)
            .min_size(Vec2::new(280., 200.))
            .max_size(search_size)
            .constrain_to(ctx.content_rect().shrink(16.))
            .show(ctx, |ui| {
                if design::dialog_header(ui, "search", "全局搜索").clicked() {
                    close_search = true;
                }
                ui.separator();
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("名称、编号、别名或近似说法，例如防空炮")
                        .desired_width(f32::INFINITY),
                );
                if self.search_focus {
                    input.request_focus();
                    self.search_focus = false;
                }
                if input.changed() {
                    self.search_selected = 0;
                }
                // Build only when the search is opened, then reuse its prepared rows
                // and results while the user scrolls or moves the pointer.
                if self.search_index.is_none() || self.search_index_tables != self.data.tables.len()
                {
                    let rows = search_rows::all(&self.data);
                    let index = search::GlobalIndex::new(&rows);
                    self.search_kinds = index
                        .rows()
                        .iter()
                        .map(|r| r.kind.clone())
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    self.search_index = Some(index);
                    self.search_index_tables = self.data.tables.len();
                    self.search_cache_key = None;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.search_kind, String::new(), "全部");
                    for kind in &self.search_kinds {
                        ui.selectable_value(&mut self.search_kind, kind.clone(), kind.as_str());
                    }
                });
                let cache_key = (
                    data::normalize(self.search.trim()),
                    self.search_kind.clone(),
                );
                if self.search_cache_key.as_ref() != Some(&cache_key) {
                    self.search_hits = self.search_index.as_ref().unwrap().search(
                        &cache_key.0,
                        &cache_key.1,
                        usize::MAX,
                    );
                    self.search_cache_key = Some(cache_key);
                    self.search_selected = 0;
                }
                let index = self.search_index.as_ref().unwrap();
                let found = &self.search_hits;
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
                        nav = Some(index.rows()[r.row_index].href.clone());
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} 条结果", found.len()));
                    if !self.search.trim().is_empty()
                        && found
                            .iter()
                            .any(|h| h.match_kind != search::MatchKind::Exact)
                    {
                        ui.weak("包含别名或近似匹配，请按名称、类别和坐标选择。");
                    }
                });
                let mut scroll = egui::ScrollArea::vertical();
                if up || down {
                    scroll = scroll.vertical_scroll_offset(self.search_selected as f32 * 45.);
                }
                scroll.show_rows(ui, 45., found.len(), |ui, range| {
                    for i in range {
                        let hit = &found[i];
                        let r = &index.rows()[hit.row_index];
                        let href = r.href.as_str();
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 41.),
                            egui::Sense::click(),
                        );
                        let p = design::palette(ui.ctx());
                        if i == self.search_selected || response.hovered() {
                            ui.painter().rect_filled(rect, 5, p.selected);
                        }
                        let mut row = ui.new_child(
                            egui::UiBuilder::new()
                                .id_salt(("search-result", href))
                                .max_rect(rect.shrink(3.))
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        row.set_clip_rect(rect.intersect(ui.clip_rect()));
                        if href.starts_with("#details/passive/")
                            || href.starts_with("#details/surgery/")
                        {
                            let key = href.rsplit('/').next().unwrap_or("");
                            let rank = self.data.get("pal-details-data")["passives"][key]["rank"]
                                .as_i64()
                                .unwrap_or(0);
                            breeding_ui::passive_rank(&mut row, &self.data, rank, 22.);
                        } else if !r.image.is_empty() {
                            row.add(
                                egui::Image::new(self.data.image_source(&r.image))
                                    .fit_to_exact_size(Vec2::splat(30.)),
                            );
                        } else if let Some(source) = self.catalog.image_for_route(&self.data, href)
                        {
                            let image =
                                egui::Image::new(source).fit_to_exact_size(Vec2::splat(30.));
                            row.add(image);
                        } else {
                            row.allocate_space(Vec2::splat(30.));
                        }
                        row.add_sized(
                            Vec2::new(72., 32.),
                            egui::Label::new(
                                RichText::new(r.kind.as_str()).size(12.).color(p.muted),
                            )
                            .halign(egui::Align::Min)
                            .truncate(),
                        );
                        let name_width = (row.available_width() * 0.47).max(90.);
                        row.add_sized(
                            Vec2::new(name_width, 32.),
                            egui::Label::new(RichText::new(r.name.as_str()).size(14.))
                                .halign(egui::Align::Min)
                                .truncate(),
                        );
                        row.add_sized(
                            Vec2::new(row.available_width(), 32.),
                            egui::Label::new(
                                RichText::new(if hit.match_kind == search::MatchKind::Exact {
                                    r.hint.clone()
                                } else {
                                    format!("{} · {}", r.hint, hit.match_kind.label())
                                })
                                .size(11.)
                                .color(p.muted),
                            )
                            .halign(egui::Align::Min)
                            .truncate(),
                        );
                        if response
                            .on_hover_text(format!("{}\n{}\n{}", r.name, r.hint, hit.explanation))
                            .clicked()
                        {
                            nav = Some(href.into());
                        }
                    }
                });
            });
        self.search_open = open && !close_search;
        nav
    }
    fn settings(&mut self, ui: &mut egui::Ui, page: &str) {
        if page == "settings/skins" {
            egui::ScrollArea::vertical()
                .id_salt("skin-settings-page")
                .show(ui, |ui| {
                    if let Some(theme) = skins::show(ui, &self.theme) {
                        self.store.state["theme"] = json!(theme);
                        self.store.changed();
                    }
                    ui.add_space(18.);
                    ui.separator();
                    self.external.show(ui, &self.data);
                });
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.chrome.apply(ctx, frame);
        self.probe.begin_frame();
        self.frames += 1;
        if self.last_save.elapsed() > Duration::from_secs(1) {
            self.store.flush();
            self.last_save = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        self.data.poll();
        self.updates
            .seed_local_notices(self.data.get("announcements"));
        self.data.preload(ctx);
        self.worlds.tick(&mut self.store);
        self.map.set_world_id(text(self.store.world(), "id"));
        self.spawns.set_world_id(text(self.store.world(), "id"));
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
            if !self.worlds.open && !self.updates.has_overlay() {
                self.search_open = true;
                self.search_focus = true;
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft))
            || ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Extra1))
        {
            self.back();
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight))
            || ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Extra2))
        {
            self.next();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) {
            self.new_window();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.dismiss_overlay();
        }
        let page = page_for(&self.route);
        let section = SECTIONS
            .iter()
            .position(|(_, _, tabs)| tabs.iter().any(|(_, p)| *p == page))
            .unwrap_or(0);
        let mut nav = None;
        let mut history_request = None;
        let mut departure_map = None;
        let palette = design::palette(ctx);
        egui::TopBottomPanel::top("titlebar")
            .exact_height(35.)
            .frame(
                egui::Frame::new()
                    .fill(palette.rail)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ctx, |ui| {
                if self.theme == "moon" {
                    design::gradient(
                        ui.painter(),
                        ui.max_rect(),
                        &[
                            Color32::from_rgb(155, 191, 217),
                            Color32::from_rgb(155, 173, 216),
                            Color32::from_rgb(183, 166, 212),
                        ],
                        true,
                    );
                }
                let caption_ink = if self.theme == "moon" {
                    palette.ink
                } else {
                    Color32::WHITE
                };
                ui.visuals_mut().override_text_color = Some(caption_ink);
                let mut drag_rect = ui.max_rect();
                drag_rect.max.x -= 138.;
                let drag = ui.interact(drag_rect, ui.id().with("window-drag"), egui::Sense::drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.add_space(12.);
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/jade-symbol.webp"))
                            .fit_to_exact_size(Vec2::splat(24.)),
                    );
                    ui.add_space(8.);
                    ui.label(RichText::new("Jade · 幻兽帕鲁工具箱").size(12.).strong());
                    ui.add_space(10.);
                    ui.label(
                        RichText::new("Rust 测试版")
                            .size(11.)
                            .color(caption_ink.gamma_multiply(0.8)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if design::window_button(ui, "close", "关闭", caption_ink).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        if design::window_button(
                            ui,
                            if max { "restore" } else { "maximize" },
                            "最大化 / 还原",
                            caption_ink,
                        )
                        .clicked()
                        {
                            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                        }
                        if design::window_button(ui, "minimize", "最小化", caption_ink).clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
        egui::SidePanel::left("navigation")
            .resizable(false)
            .exact_width(108.)
            .frame(
                egui::Frame::new()
                    .fill(palette.rail)
                    .inner_margin(egui::Margin::symmetric(9, 10)),
            )
            .show(ctx, |ui| {
                if self.theme == "moon" {
                    design::gradient(
                        ui.painter(),
                        ui.max_rect().expand2(Vec2::new(9., 10.)),
                        &[
                            palette.rail,
                            Color32::from_rgb(151, 168, 231),
                            Color32::from_rgb(191, 145, 223),
                            Color32::from_rgb(166, 154, 223),
                        ],
                        false,
                    );
                }
                let compact = ui.available_height() < 700.;
                let nav_height = if compact { 30. } else { 54. };
                ui.style_mut().visuals.override_text_color = Some(if self.theme == "moon" {
                    palette.ink
                } else {
                    Color32::from_rgb(241, 245, 231)
                });
                ui.vertical_centered(|ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/jade-symbol.webp"))
                            .fit_to_exact_size(Vec2::splat(if compact { 38. } else { 50. })),
                    );
                    ui.label(RichText::new("Jade").size(21.).strong());
                    ui.label(RichText::new("幻兽帕鲁工具箱").size(10.));
                });
                ui.add_space(5.);
                ui.separator();
                if ui
                    .add_sized(
                        [90., 34.],
                        egui::Button::image_and_text(
                            egui::Image::new(design::icon_source("search"))
                                .fit_to_exact_size(Vec2::splat(14.))
                                .tint(ui.visuals().text_color()),
                            RichText::new("全局搜索").size(12.),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend)
                        .fill(Color32::TRANSPARENT),
                    )
                    .on_hover_text("搜索全部资料 · Ctrl+K")
                    .clicked()
                {
                    self.search_open = true;
                    self.search_focus = true;
                }
                egui::ScrollArea::vertical()
                    .id_salt("sidebar")
                    .max_height((ui.available_height() - 113.).max(100.))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = if compact { 1. } else { 4. };
                        let icons = [
                            "egg", "worldmap", "paldex", "enemy", "book", "ranch", "items",
                            "compass", "settings",
                        ];
                        for (i, (name, route, _)) in SECTIONS.iter().enumerate() {
                            if design::nav_button(
                                ui,
                                name,
                                icons[i],
                                i == section,
                                nav_height,
                                self.theme == "moon",
                            )
                            .clicked()
                            {
                                nav = Some(format!("#{route}"));
                            }
                        }
                    });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(if self.data.pending > 0 {
                            "正在准备资料"
                        } else {
                            "● 离线可用"
                        })
                        .size(10.),
                    );
                    if ui
                        .add(
                            egui::Button::image_and_text(
                                egui::Image::new(design::icon_source("info"))
                                    .fit_to_exact_size(Vec2::splat(12.))
                                    .tint(ui.visuals().text_color()),
                                RichText::new("数据说明").size(11.),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.about = true;
                    }
                    if ui
                        .add_sized(
                            [88., 46.],
                            egui::Button::new(
                                RichText::new(format!(
                                    "世界档案\n{}",
                                    text(self.store.world(), "name")
                                ))
                                .size(11.),
                            )
                            .wrap()
                            .fill(Color32::from_white_alpha(28)),
                        )
                        .clicked()
                    {
                        self.worlds.open = true;
                    }
                });
            });
        egui::TopBottomPanel::top("tabs")
            .exact_height(58.)
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .inner_margin(egui::Margin::symmetric(20, 5)),
            )
            .show(ctx, |ui| {
                if self.theme == "moon" {
                    design::gradient(
                        ui.painter(),
                        ui.max_rect().expand2(Vec2::new(20., 5.)),
                        &[palette.surface, Color32::from_rgb(195, 186, 215)],
                        true,
                    );
                }
                ui.horizontal_centered(|ui| {
                    let current = self.canonical_route();
                    let overlay = self.has_overlay();
                    let back_hint = if overlay {
                        "关闭当前窗口或选择框".into()
                    } else {
                        self.history
                            .peek_back(&current)
                            .map(|e| format!("返回：{}", e.label))
                            .unwrap_or("已是最初页面".into())
                    };
                    let back = ui
                        .add_enabled_ui(
                            overlay || self.history.peek_back(&current).is_some(),
                            |ui| design::action(ui, "left", "返回"),
                        )
                        .inner
                        .on_hover_text(format!("{back_hint}\nAlt+← / 鼠标后退键 · 右键查看历史"));
                    if back.clicked() {
                        history_request = Some(if overlay {
                            self.dismiss_overlay();
                            HistoryRequest::Dismissed
                        } else {
                            HistoryRequest::Back
                        });
                    }
                    back.context_menu(|ui| {
                        ui.strong("返回到");
                        for (index, entry) in self
                            .history
                            .back_targets(&current, 12)
                            .into_iter()
                            .enumerate()
                        {
                            if ui.button(&entry.label).clicked() {
                                history_request = Some(HistoryRequest::BackSteps(index + 1));
                                ui.close();
                            }
                        }
                    });
                    let forward_hint = self
                        .history
                        .peek_forward(&current)
                        .map(|e| format!("前进：{}", e.label))
                        .unwrap_or("没有可前进的页面".into());
                    let forward = ui
                        .add_enabled_ui(self.history.peek_forward(&current).is_some(), |ui| {
                            design::icon_button(ui, "right", "前进")
                        })
                        .inner
                        .on_hover_text(format!(
                            "{forward_hint}\nAlt+→ / 鼠标前进键 · 右键查看历史"
                        ));
                    if forward.clicked() {
                        history_request = Some(if overlay {
                            self.dismiss_overlay();
                            HistoryRequest::Dismissed
                        } else {
                            HistoryRequest::Forward
                        });
                    }
                    forward.context_menu(|ui| {
                        ui.strong("前进到");
                        for (index, entry) in self
                            .history
                            .forward_targets(&current, 12)
                            .into_iter()
                            .enumerate()
                        {
                            if ui.button(&entry.label).clicked() {
                                history_request = Some(HistoryRequest::ForwardSteps(index + 1));
                                ui.close();
                            }
                        }
                    });
                    if self
                        .breeding
                        .has_suspended_edit_for(text(self.store.world(), "id"))
                        && design::small_action(ui, "edit", "继续编辑").clicked()
                    {
                        if self
                            .breeding
                            .resume_edit_for(text(self.store.world(), "id"))
                            && !["lookup", "warehouse", "planner", "offspring"]
                                .contains(&page.as_str())
                        {
                            nav = Some("#warehouse".into());
                        }
                    }
                    if (page == "map" || page == "spawns")
                        && (if page == "map" {
                            self.map.has_suspended_editor()
                        } else {
                            self.spawns.has_suspended_editor()
                        })
                        && design::small_action(ui, "edit", "继续编辑点位").clicked()
                    {
                        if page == "map" {
                            self.map.resume_editor();
                        } else {
                            self.spawns.resume_editor();
                        }
                    }
                    let unread = self.updates.unread_count(&self.store.state);
                    if design::announcement_button(ui, unread).clicked() {
                        nav = Some("#settings/updates".into());
                    }
                    egui::ScrollArea::horizontal()
                        .id_salt("tab-scroll")
                        .max_height(44.)
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (name, id) in SECTIONS[section].2 {
                                    let response = design::tab_with_icon(
                                        ui,
                                        name,
                                        design::tab_icon(id),
                                        page == *id,
                                    );
                                    if page == *id
                                        && ui
                                            .ctx()
                                            .data(|m| {
                                                m.get_temp::<String>(egui::Id::new(
                                                    "last-tabs-page",
                                                ))
                                            })
                                            .as_deref()
                                            != Some(page.as_str())
                                    {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if response.clicked() {
                                        nav = Some(format!("#{id}"));
                                    }
                                }
                            });
                        });
                    ui.ctx()
                        .data_mut(|m| m.insert_temp(egui::Id::new("last-tabs-page"), page.clone()));
                });
            });
        navigation::begin_scroll_frame(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(palette.bg)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                let heading = SECTIONS[section]
                    .2
                    .iter()
                    .find(|(_, id)| *id == page)
                    .map(|(name, _)| *name)
                    .unwrap_or("工具箱");
                let origin = if page == "map" || page == "spawns" {
                    map_origin(
                        self.history
                            .back_entries()
                            .map(|entry| entry.route.as_str()),
                    )
                } else {
                    None
                };
                if let Some(route) = page_header(ui, &page, heading, &self.data, origin) {
                    nav = Some(route);
                }
                if let Some(e) = &self.store.error {
                    ui.colored_label(Color32::DARK_RED, e);
                }
                for error in &self.data.errors {
                    ui.colored_label(Color32::DARK_RED, error);
                }
                if ["warehouse", "planner", "offspring"].contains(&page.as_str()) {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled_ui(!self.worlds.busy, |ui| {
                                design::action(ui, "refresh", "刷新现有帕鲁")
                            })
                            .inner
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
                        // Freeze only an input frame that can leave this view. Panning,
                        // hovering and idle frames do not copy even the small bookmark.
                        if ui.input(|i| {
                            i.pointer.any_click()
                                || i.events
                                    .iter()
                                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
                        }) {
                            departure_map = Some(self.navigation_snapshot());
                        }
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
                        let next =
                            self.catalog
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
        if std::mem::take(&mut self.worlds.new_window_requested) {
            self.new_window();
        }
        if self.about {
            let mut close_about = false;
            egui::Window::new("数据说明")
                .open(&mut self.about)
                .title_bar(false)
                .show(ctx, |ui| {
                    if design::dialog_header(ui, "info", "数据说明").clicked() {
                        close_about = true;
                    }
                    ui.separator();
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
            if close_about {
                self.about = false;
            }
        }
        if let Some(request) = history_request {
            match request {
                HistoryRequest::Dismissed => {}
                HistoryRequest::Back => self.back(),
                HistoryRequest::Forward => self.next(),
                HistoryRequest::BackSteps(n) => self.travel_back(n),
                HistoryRequest::ForwardSteps(n) => self.travel_forward(n),
            }
        } else if let Some(route) = nav {
            if let Some(mut origin) = departure_map {
                origin.state.scrolls = navigation::capture_scrolls(ctx);
                self.go_from(decode_route(&route), origin);
            } else {
                self.go(route);
            }
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
        let probe_route = self.route.clone();
        if self.probe.enabled() {
            let scrolls: Vec<_> = navigation::capture_scrolls(ctx)
                .into_iter()
                .map(|s| json!({"id":format!("{:?}",s.id),"x":s.offset.x,"y":s.offset.y}))
                .collect();
            let value = json!({"canonicalRoute":self.canonical_route(),"back":self.history.back_entries().map(|e|e.route.clone()).collect::<Vec<_>>(),
                "forward":self.history.forward_targets(&self.canonical_route(),64).iter().map(|e|e.route.clone()).collect::<Vec<_>>(),
                "scrolls":scrolls,"searchOpen":self.search_open,"worldsOpen":self.worlds.open,"aboutOpen":self.about,
                "editorOpen":self.breeding.has_edit_draft(),"suspendedEditor":self.breeding.has_suspended_edit_for(text(self.store.world(),"id")),
                "calculator":{"pal":text(&self.data.species[self.calculators.pal],"key"),"level":self.calculators.level,"traits":self.calculators.traits},
                "mapState":format!("{:?}",self.map.navigation_state()),"spawnState":format!("{:?}",self.spawns.navigation_state())});
            ctx.data_mut(|m| m.insert_temp(egui::Id::new("jade-navigation-probe"), value));
        }
        if self
            .probe
            .tick(ctx, &mut self.route, &self.data, &self.store)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.route != probe_route {
            self.activate(self.route.clone());
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
        if adjacent.exists()
            || !(cfg!(debug_assertions)
                || arg("--qa-output").is_some()
                || std::env::var_os("JADE_QA_OUTPUT").is_some())
        {
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
    let qa_size = if std::env::var_os("JADE_QA_OUTPUT").is_some() {
        std::env::var("JADE_QA_SIZE")
            .ok()
            .and_then(|s| {
                s.split_once('x')
                    .map(|(a, b)| (a.parse::<f32>(), b.parse::<f32>()))
            })
            .and_then(|(a, b)| Some([a.ok()?.max(960.), b.ok()?.max(620.)]))
    } else {
        None
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(qa_size.unwrap_or([1440., 900.]))
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
    fn app() -> (App, store::tests::Temp) {
        let temp = store::tests::Temp::new();
        let ctx = egui::Context::default();
        let data = Data::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
            ctx.clone(),
        )
        .unwrap();
        let updates = updates::UpdateState::new(temp.0.clone(), std::env::current_exe().unwrap());
        let store = store::Store::open(temp.0.clone());
        (App::from_components(ctx, data, store, updates, None), temp)
    }
    #[test]
    fn back_tracks_the_species_edited_inside_a_calculator_and_restores_inputs() {
        let (mut app, _temp) = app();
        app.activate("#details/stats/SheepBall".into());
        app.calculators.pal = app
            .data
            .species
            .iter()
            .position(|p| text(p, "key") == "Anubis")
            .unwrap();
        app.calculators.level = 48;
        app.calculators.traits = vec!["Legend".into()];
        app.go("#paldex/Anubis".into());
        app.calculators.level = 2;
        app.back();
        assert_eq!(app.route, "#details/stats/Anubis");
        assert_eq!(app.calculators.level, 48);
        assert_eq!(app.data.species[app.calculators.pal]["key"], "Anubis");
        assert_eq!(app.calculators.traits, vec!["Legend".to_owned()]);
        app.next();
        assert_eq!(app.route, "#paldex/Anubis");
    }
    #[test]
    fn back_ignores_equivalent_tool_routes_and_retains_forward_history() {
        let (mut app, _temp) = app();
        app.activate("#lookup/SheepBall".into());
        app.breeding.reverse = true;
        app.go("#paldex/SheepBall".into());
        app.back();
        assert_eq!(app.route, "#lookup/parents/SheepBall");
        let n = app.history.back_entries().len();
        app.go("#lookup".into());
        assert_eq!(app.history.back_entries().len(), n);
        assert!(app.history.peek_forward(&app.canonical_route()).is_some());
        app.next();
        assert_eq!(app.route, "#paldex/SheepBall");
    }
    #[test]
    fn back_closes_the_foreground_window_without_advancing_history() {
        let (mut app, _temp) = app();
        app.go("#paldex/SheepBall".into());
        app.search_open = true;
        app.worlds.open = true;
        app.back();
        assert!(!app.worlds.open && app.search_open);
        assert_eq!(app.route, "#paldex/SheepBall");
        app.back();
        assert!(!app.search_open);
        assert_eq!(app.route, "#paldex/SheepBall");
        app.back();
        assert!(app.route.starts_with("#lookup/"));
    }
    #[test]
    fn back_and_forward_do_not_leave_a_second_unsaved_editor() {
        let (mut app, _temp) = app();
        let started = Instant::now();
        while app.data.pending > 0 && started.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
            app.data.poll();
        }
        assert_eq!(app.data.pending, 0);
        assert!(app.data.errors.is_empty());
        app.activate("#map/coords/worldtree/0/800".into());
        app.map.set_world_id(text(app.store.world(), "id"));
        app.map.route = app.route.clone();
        let ctx = app.ui_ctx.clone();
        let mut notes = json!({"completed":[],"custom":[]});
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1200., 900.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    map::show(ui, &app.data, &mut app.map, &mut notes);
                });
            },
        );
        let camera = format!("{:?}", app.map.navigation_state());
        app.go("#warehouse".into());
        app.go("#paldex/Anubis".into());
        app.back();
        app.breeding.poll(&app.store);
        let first = json!({"id":"draft-first","speciesKey":"SheepBall","name":"第一份未保存草稿"});
        let second = json!({"id":"draft-second","speciesKey":"Anubis","name":"第二份未保存草稿"});
        app.breeding.edit = Some(first.clone());
        app.back(); // The first editor is safely suspended, with no navigation.
        assert_eq!(app.route, "#warehouse");
        app.breeding.edit = Some(second.clone());
        let world = app.store.world().clone();
        let back: Vec<_> = app
            .history
            .back_entries()
            .map(|e| e.route.clone())
            .collect();
        let forward: Vec<_> = app
            .history
            .forward_targets(&app.canonical_route(), 64)
            .iter()
            .map(|e| e.route.clone())
            .collect();
        for action in 0..4 {
            match action {
                0 => app.back(),
                1 => app.next(),
                2 => app.travel_back(1),
                _ => app.travel_forward(1),
            }
            assert_eq!(app.route, "#warehouse");
            assert_eq!(app.breeding.edit, Some(second.clone()));
            assert_eq!(
                app.history
                    .back_entries()
                    .map(|e| e.route.clone())
                    .collect::<Vec<_>>(),
                back
            );
            assert_eq!(
                app.history
                    .forward_targets(&app.canonical_route(), 64)
                    .iter()
                    .map(|e| e.route.clone())
                    .collect::<Vec<_>>(),
                forward
            );
            assert_eq!(format!("{:?}", app.map.navigation_state()), camera);
            assert_eq!(app.store.world(), &world);
        }
        app.breeding.edit = None; // Explicitly close the second editor.
        assert!(app.breeding.resume_edit_for(text(app.store.world(), "id")));
        assert_eq!(app.breeding.edit, Some(first));
    }

    use super::*;
    #[test]
    fn map_source_survives_map_navigation_without_reusing_stale_item_history() {
        let routes = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let from_item = routes(&[
            "#lookup",
            "#items/Wool",
            "#map/point/example",
            "#spawns/SheepBall",
        ]);
        assert_eq!(
            map_origin(from_item.iter().map(String::as_str)),
            Some(("#items/Wool", "物品查询"))
        );
        assert_eq!(
            map_origin(
                routes(&["#items/Wool", "#lookup", "#map"])
                    .iter()
                    .map(String::as_str)
            ),
            None
        );
        let from_journal = routes(&["#guide/journals/one", "#map"]);
        assert_eq!(
            map_origin(from_journal.iter().map(String::as_str)),
            Some(("#guide/journals/one", "探索手记"))
        );
    }
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
