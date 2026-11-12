//! Native map camera, spatial clustering, annotations and Pal spawn lookup.
use crate::data::{Data, array, normalize, text};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Vec2};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone)]
struct Pin {
    id: String,
    region: String,
    category: String,
    name: String,
    detail: String,
    uv: Vec2,
    x: f64,
    y: f64,
    levels: String,
    art: String,
    raw: Value,
    search: String,
}
#[derive(Default)]
struct MapSearchCache {
    query: String,
    points: usize,
    custom_signature: String,
    scores: Vec<Option<u32>>,
    texts: Vec<crate::search::CachedText>,
}
#[derive(Clone)]
struct Editor {
    id: String,
    name: String,
    note: String,
    x: f64,
    y: f64,
    error: String,
}
pub struct MapState {
    pub route: String,
    world_id: String,
    last_route: String,
    region: String,
    categories: HashSet<String>,
    points: Arc<Vec<Pin>>,
    merged_points: Arc<Vec<Pin>>,
    custom_signature: String,
    category_counts: HashMap<String, HashMap<String, usize>>,
    search: String,
    completion: u8,
    center: Vec2,
    zoom: f32,
    selected: String,
    cluster: Vec<String>,
    coordinate: Option<Vec2>,
    picking: bool,
    pick_draft: Option<Editor>,
    editor: Option<Editor>,
    delete_confirm: bool,
    x: f64,
    y: f64,
    spawn_key: String,
    spawn_search: String,
    spawn_time: u8,
    spawn_kind: u8,
    spawn_cache: (String, u8, u8),
    spawn_points: Vec<Pin>,
    categories_expanded: bool,
    viewport: Rect,
    detail_height: f32,
    search_cache: RefCell<MapSearchCache>,
}
impl Default for MapState {
    fn default() -> Self {
        Self {
            route: String::new(),
            world_id: String::new(),
            last_route: String::new(),
            region: "palpagos".into(),
            categories: HashSet::new(),
            points: Arc::new(vec![]),
            merged_points: Arc::new(vec![]),
            custom_signature: String::new(),
            category_counts: HashMap::new(),
            search: String::new(),
            completion: 0,
            center: Vec2::splat(0.5),
            zoom: 1.,
            selected: String::new(),
            cluster: vec![],
            coordinate: None,
            picking: false,
            pick_draft: None,
            editor: None,
            delete_confirm: false,
            x: 0.,
            y: 0.,
            spawn_key: "SheepBall".into(),
            spawn_search: String::new(),
            spawn_time: 0,
            spawn_kind: 0,
            spawn_cache: (String::new(), 0, 0),
            spawn_points: vec![],
            categories_expanded: false,
            viewport: Rect::NOTHING,
            detail_height: 220.,
            search_cache: RefCell::default(),
        }
    }
}
impl MapState {
    /// A draft belongs to its original world; never carry an editor into another save.
    pub fn set_world_id(&mut self, id: &str) {
        if self.world_id != id {
            self.world_id = id.into();
            self.editor = None;
            self.picking = false;
            self.pick_draft = None;
            self.delete_confirm = false;
            self.selected.clear();
            self.cluster.clear();
            self.completion = 0;
            self.custom_signature.clear();
        }
    }
}
fn start_picking(state: &mut MapState) {
    state.pick_draft = state.editor.take();
    state.picking = true;
}
fn cancel_picking(state: &mut MapState) {
    state.picking = false;
    if let Some(draft) = state.pick_draft.take() {
        state.editor = Some(draft);
    }
}
fn accept_picked_position(state: &mut MapState, x: f64, y: f64) {
    let mut draft = state.pick_draft.take().unwrap_or(Editor {
        id: String::new(),
        name: String::new(),
        note: String::new(),
        x,
        y,
        error: String::new(),
    });
    draft.x = x;
    draft.y = y;
    draft.error.clear();
    state.editor = Some(draft);
    state.picking = false;
}
fn number(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_default()
}
fn bounds(region: &str) -> (f64, f64, f64) {
    if region == "worldtree" {
        (-127.7, 648.7 + 341797. / 1335.144531, 341797. / 1335.144531)
    } else {
        (
            (-724400. - 158000.) / 459.,
            (349400. + 123888.) / 459.,
            1448800. / 459.,
        )
    }
}
fn coordinates(region: &str, uv: Vec2) -> (f64, f64) {
    let (left, top, span) = bounds(region);
    (
        (left + uv.x as f64 * span).round(),
        (top - uv.y as f64 * span).round(),
    )
}
fn position(region: &str, x: f64, y: f64) -> Option<Vec2> {
    let (left, top, span) = bounds(region);
    let u = (x - left) / span;
    let v = (top - y) / span;
    if u.is_finite() && v.is_finite() && (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
        Some(Vec2::new(u as f32, v as f32))
    } else {
        None
    }
}
fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(n) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn level(v: &Value) -> String {
    if let Some(a) = v["guardLevels"].as_array() {
        format!(
            "守军等级 {}",
            a.iter().map(Value::to_string).collect::<Vec<_>>().join("–")
        )
    } else if let Some(n) = v["level"].as_u64() {
        format!("等级 {n}")
    } else {
        String::new()
    }
}
fn icon(data: &Data, p: &Value) -> String {
    let images = data.get("map-images");
    let cat = text(p, "category");
    if cat == "alpha_pals" {
        if let Some(pal) = data.pal(text(p, "palKey")) {
            if let Some(s) = data.get("portraits")[text(pal, "en")].as_str() {
                return s.into();
            }
        }
    }
    if let Some(s) = images["minerals"][cat].as_str() {
        return s.into();
    }
    let key = p["iconKey"]
        .as_str()
        .or_else(|| images["mapIcons"]["defaults"][cat].as_str())
        .unwrap_or("");
    images["mapIcons"]["art"][key].as_str().unwrap_or("").into()
}
fn pin(data: &Data, p: &Value) -> Pin {
    let uv = Vec2::new(number(p, "u") as f32, number(p, "v") as f32);
    let region = text(p, "map");
    let (x, y) = coordinates(region, uv);
    let category = text(p, "category");
    let category_name = array(&data.get("map-data")["categories"])
        .iter()
        .find(|c| text(c, "id") == category)
        .map(|c| text(c, "name"))
        .unwrap_or("我的点位");
    Pin {
        id: text(p, "id").into(),
        region: region.into(),
        category: category.into(),
        name: text(p, "name").into(),
        detail: p["detail"]
            .as_str()
            .or_else(|| p["note"].as_str())
            .unwrap_or("")
            .into(),
        uv,
        x: p["x"].as_f64().unwrap_or(x),
        y: p["y"].as_f64().unwrap_or(y),
        levels: level(p),
        art: icon(data, p),
        search: normalize(&format!(
            "{category_name} {} {} {} {} {} {}",
            text(p, "name"),
            text(p, "detail"),
            text(p, "note"),
            text(p, "nearest"),
            text(p, "en"),
            category
        )),
        raw: p.clone(),
    }
}
fn defaults(data: &Data) -> HashSet<String> {
    array(&data.get("map-data")["categories"])
        .iter()
        .filter(|c| c["default"].as_bool() == Some(true))
        .map(|c| text(c, "id").into())
        .chain(std::iter::once("custom".into()))
        .collect()
}
fn region_name(region: &str) -> &str {
    if region == "worldtree" {
        "世界树"
    } else {
        "帕洛斯群岛"
    }
}
fn done_set(notes: &Value) -> HashSet<String> {
    array(&notes["completed"])
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .chain(
            array(&notes["custom"])
                .iter()
                .filter(|p| p["done"].as_bool() == Some(true))
                .map(|p| text(p, "id").into()),
        )
        .collect()
}
fn set_done(notes: &mut Value, id: &str, done: bool) {
    let mut ids = done_set(notes);
    if done {
        ids.insert(id.into());
    } else {
        ids.remove(id);
    }
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    notes["completed"] = json!(ids);
    if let Some(custom) = notes["custom"].as_array_mut() {
        for p in custom {
            if text(p, "id") == id {
                p["done"] = json!(done)
            }
        }
    }
}
fn all_points(data: &Data, state: &mut MapState, notes: &Value) -> Arc<Vec<Pin>> {
    let signature = notes["custom"].to_string();
    let changed = state.custom_signature != signature || state.merged_points.is_empty();
    if changed {
        state.custom_signature = signature;
        if array(&notes["custom"]).is_empty() {
            state.merged_points = state.points.clone();
        } else {
            let mut p = state.points.as_ref().clone();
            p.extend(array(&notes["custom"]).iter().map(|v| {
                let mut value = v.clone();
                value["category"] = json!("custom");
                pin(data, &value)
            }));
            state.merged_points = Arc::new(p);
        }
    }
    if changed || state.category_counts.is_empty() {
        state.category_counts.clear();
        for p in state.merged_points.iter() {
            let counts = state.category_counts.entry(p.region.clone()).or_default();
            *counts.entry(p.category.clone()).or_default() += 1;
            let also = text(&p.raw, "alsoCategory");
            if !also.is_empty() {
                *counts.entry(also.into()).or_default() += 1;
            }
        }
    }
    state.merged_points.clone()
}
fn reset_camera(state: &mut MapState) {
    state.center = Vec2::splat(0.5);
    state.zoom = 1.;
    state.coordinate = None;
}
fn focus(state: &mut MapState, p: &Pin) {
    state.region = p.region.clone();
    state.center = p.uv;
    state.zoom = state.zoom.max(4.);
    state.selected = p.id.clone();
    state.delete_confirm = false;
    state.coordinate = None;
    state.cluster.clear();
    state.categories.insert(p.category.clone());
    state.search.clear();
    state.completion = 0;
}
fn routes(data: &Data, state: &mut MapState, points: &[Pin]) {
    if state.route == state.last_route {
        return;
    }
    state.last_route = state.route.clone();
    let route = state.route.clone();
    let parts = route
        .trim_start_matches('#')
        .split('/')
        .map(decode)
        .collect::<Vec<_>>();
    if parts.first().map(String::as_str) != Some("map") {
        return;
    }
    match parts.get(1).map(String::as_str) {
        Some("point") => {
            if let Some(p) = points.iter().find(|p| Some(&p.id) == parts.get(2)) {
                focus(state, p)
            }
        }
        Some("category") => {
            if let Some(cat) = parts.get(2) {
                state.region = parts
                    .get(3)
                    .filter(|s| s.as_str() == "worldtree")
                    .cloned()
                    .unwrap_or_else(|| "palpagos".into());
                state.categories = if cat == "all" {
                    points.iter().map(|p| p.category.clone()).collect()
                } else {
                    HashSet::from([cat.clone()])
                };
                state.search = parts.get(4).cloned().unwrap_or_default();
                state.completion = 0;
                reset_camera(state);
            }
        }
        Some("coords") => {
            if let (Some(region), Some(x), Some(y)) = (
                parts.get(2),
                parts.get(3).and_then(|s| s.parse::<f64>().ok()),
                parts.get(4).and_then(|s| s.parse::<f64>().ok()),
            ) {
                if ["palpagos", "worldtree"].contains(&region.as_str()) {
                    if let Some(uv) = position(region, x, y) {
                        state.region = region.clone();
                        state.center = uv;
                        state.coordinate = Some(uv);
                        state.zoom = 5.;
                        state.x = x;
                        state.y = y;
                        state.selected.clear();
                    }
                }
            }
        }
        _ => {}
    }
    if state.categories.is_empty() && state.points.is_empty() {
        state.categories = defaults(data);
    }
}
fn map_header(ui: &mut egui::Ui, state: &mut MapState) {
    let old = state.region.clone();
    egui::ComboBox::from_id_salt("map-region")
        .width(140.)
        .selected_text(region_name(&state.region))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut state.region, "palpagos".into(), "帕洛斯群岛");
            ui.selectable_value(&mut state.region, "worldtree".into(), "世界树");
        });
    if old != state.region {
        reset_camera(state);
        state.selected.clear();
        state.cluster.clear();
        state.picking = false;
        state.pick_draft = None;
        state.editor = None;
        state.delete_confirm = false;
    }
}
fn map_tools(ui: &mut egui::Ui, state: &mut MapState) {
    let clip = state.viewport;
    if !clip.is_positive() {
        return;
    }
    let mut tools = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("map-floating-tools")
            .max_rect(Rect::from_min_size(
                clip.right_top() + Vec2::new(-48., 12.),
                Vec2::new(36., 112.),
            ))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    tools.spacing_mut().item_spacing = Vec2::new(0., 2.);
    tools.spacing_mut().button_padding = Vec2::splat(4.);
    for (label, name, help) in [
        ("plus", "plus", "放大"),
        ("minus", "minimize", "缩小"),
        ("reset", "worldmap", "查看全图"),
    ] {
        if crate::design::icon_button(&mut tools, name, help).clicked() {
            match label {
                "plus" => state.zoom = (state.zoom * 1.5).min(32.),
                "minus" => state.zoom = (state.zoom / 1.5).max(1.),
                _ => reset_camera(state),
            }
        }
    }
    let readout = format!(
        "{} · {:.0}%  拖动平移 / 滚轮缩放",
        region_name(&state.region),
        state.zoom * 100.
    );
    let mut caption = ui.new_child(egui::UiBuilder::new().id_salt("map-readout").max_rect(
        Rect::from_min_size(
            clip.left_bottom() + Vec2::new(10., -29.),
            Vec2::new(320., 24.),
        ),
    ));
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(18, 33, 39, 215))
        .corner_radius(5)
        .inner_margin(5)
        .show(&mut caption, |ui| {
            ui.label(
                egui::RichText::new(readout)
                    .size(11.)
                    .color(Color32::from_rgb(233, 242, 242)),
            );
        });
}
fn floating_detail(
    ui: &mut egui::Ui,
    data: &Data,
    state: &mut MapState,
    points: &[Pin],
    notes: &mut Value,
) -> Option<String> {
    let clip = state.viewport;
    let Some(p) = points.iter().find(|p| p.id == state.selected) else {
        if state.cluster.is_empty() {
            return None;
        }
        let mut out = None;
        let rect = Rect::from_min_size(
            clip.left_bottom() + Vec2::new(11., -260.),
            Vec2::new(315., 222.),
        );
        let mut panel = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("map-floating-cluster")
                .max_rect(rect),
        );
        crate::design::card(&panel).show(&mut panel, |ui| {
            ui.set_width(289.);
            ui.horizontal(|ui| {
                ui.strong(format!("附近 {} 处点位", state.cluster.len()));
                if crate::design::small_icon_button(ui, "close", "关闭").clicked() {
                    state.cluster.clear();
                }
            });
            egui::ScrollArea::vertical()
                .max_height(170.)
                .show(ui, |ui| {
                    for id in state.cluster.clone() {
                        if let Some(p) = points.iter().find(|p| p.id == id) {
                            if ui.button(format!("{} · {}", p.name, p.levels)).clicked() {
                                focus(state, p);
                                out = Some(format!("#map/point/{}", encode(&p.id)));
                            }
                        }
                    }
                });
        });
        return out;
    };
    let width = 315f32.min(clip.width() - 24.);
    let height = state.detail_height.min(clip.height() - 68.).max(80.);
    let rect = Rect::from_min_size(
        clip.left_bottom() + Vec2::new(11., -38. - height),
        Vec2::new(width, clip.height() - 68.),
    );
    let mut panel = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("map-floating-detail")
            .max_rect(rect),
    );
    let result = crate::design::card(&panel).show(&mut panel, |ui| {
        ui.set_width(width - 26.);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("点位详情")
                    .size(11.)
                    .color(crate::design::palette(ui.ctx()).muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::design::small_icon_button(ui, "close", "关闭详情").clicked() {
                    state.selected.clear();
                }
            });
        });
        egui::ScrollArea::vertical()
            .id_salt("map-detail-scroll")
            .max_height(clip.height() - 128.)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| details(ui, data, state, p, notes))
            .inner
    });
    state.detail_height = result.response.rect.height();
    result.inner
}
fn category_button(
    ui: &mut egui::Ui,
    data: &Data,
    cat: &Value,
    count: usize,
    on: bool,
    width: f32,
) -> egui::Response {
    let palette = crate::design::palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 29.), Sense::click());
    ui.painter()
        .rect_filled(rect, 6, if on { palette.selected } else { palette.soft });
    ui.painter().rect_stroke(
        rect,
        6,
        Stroke::new(1.0_f32, if on { palette.accent } else { palette.line }),
        egui::StrokeKind::Inside,
    );
    let art = icon(data, &json!({"category":text(cat,"id")}));
    if !art.is_empty() {
        egui::Image::new(data.image_source(&art)).paint_at(
            ui,
            Rect::from_center_size(rect.left_center() + Vec2::new(15., 0.), Vec2::splat(19.)),
        );
    } else {
        ui.painter().circle_filled(
            rect.left_center() + Vec2::new(15., 0.),
            5.,
            color(text(cat, "id")),
        );
    }
    let name = text(cat, "name");
    let limit = ((width - 53.) / 12.).floor().max(3.) as usize;
    let display = if name.chars().count() > limit {
        format!("{}…", name.chars().take(limit - 1).collect::<String>())
    } else {
        name.into()
    };
    ui.painter().text(
        rect.left_center() + Vec2::new(29., 0.),
        egui::Align2::LEFT_CENTER,
        display,
        egui::FontId::proportional(12.),
        palette.ink,
    );
    ui.painter().text(
        rect.right_center() - Vec2::new(5., 0.),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        egui::FontId::proportional(10.),
        palette.muted,
    );
    response
        .on_hover_text(format!("{name} · {count} 处点位"))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn point_row(
    ui: &mut egui::Ui,
    data: &Data,
    p: &Pin,
    selected: bool,
    done: Option<bool>,
) -> (bool, Option<bool>) {
    let palette = crate::design::palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.), Sense::click());
    let painter = ui.painter().with_clip_rect(rect);
    if selected || response.hovered() {
        painter.rect_filled(rect, 4, palette.selected);
    }
    let mut x = rect.left() + 3.;
    let mut changed = None;
    if let Some(mut checked) = done {
        let mut check = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("point-done", &p.id))
                .max_rect(Rect::from_center_size(
                    egui::pos2(x + 9., rect.center().y),
                    Vec2::splat(18.),
                )),
        );
        check.spacing_mut().interact_size = Vec2::splat(18.);
        if crate::design::checkbox(&mut check, &mut checked, "").changed() {
            changed = Some(checked);
        }
        x += 24.;
    }
    if !p.art.is_empty() {
        egui::Image::new(data.image_source(&p.art)).paint_at(
            ui,
            Rect::from_center_size(egui::pos2(x + 13., rect.center().y), Vec2::splat(26.)),
        );
        x += 32.;
    }
    let label = if p.category == "wild" || p.category == "alpha" {
        format!("{:.0}，{:.0}", p.x, p.y)
    } else {
        p.name.clone()
    };
    painter.text(
        egui::pos2(x, rect.top() + 5.),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(12.),
        palette.ink,
    );
    painter.text(
        egui::pos2(x, rect.top() + 25.),
        egui::Align2::LEFT_TOP,
        if p.category == "wild" || p.category == "alpha" {
            p.levels.clone()
        } else {
            format!("{:.0}，{:.0} {}", p.x, p.y, p.levels)
        },
        egui::FontId::proportional(10.5),
        palette.muted,
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.5_f32, palette.line),
    );
    (
        response
            .on_hover_text(format!(
                "{}
{}
{:.0}，{:.0}",
                p.name, p.levels, p.x, p.y
            ))
            .clicked()
            && changed.is_none(),
        changed,
    )
}
fn coordinate_tools(ui: &mut egui::Ui, state: &mut MapState) {
    ui.horizontal(|ui| {
        ui.label("X");
        ui.add(egui::DragValue::new(&mut state.x));
        ui.label("Y");
        ui.add(egui::DragValue::new(&mut state.y));
        if crate::design::small_action(ui, "pin", "定位").clicked() {
            if let Some(uv) = position(&state.region, state.x, state.y) {
                state.center = uv;
                state.coordinate = Some(uv);
                state.zoom = 5.;
                state.selected.clear();
            }
        }
    });
    if crate::design::action(ui, "plus", "输入坐标添加").clicked() {
        let mut draft = state.pick_draft.take().unwrap_or(Editor {
            id: String::new(),
            name: String::new(),
            note: String::new(),
            x: state.x,
            y: state.y,
            error: String::new(),
        });
        draft.x = state.x;
        draft.y = state.y;
        draft.error.clear();
        state.editor = Some(draft);
        state.picking = false;
    }
}
fn filtered<'a>(points: &'a [Pin], state: &MapState, done: &HashSet<String>) -> Vec<&'a Pin> {
    let q = normalize(state.search.trim());
    let allowed = |p: &&Pin| {
        p.region == state.region
            && (state.categories.contains(&p.category)
                || state.categories.contains(text(&p.raw, "alsoCategory")))
            && (state.completion == 0
                || (state.completion == 1 && !done.contains(&p.id))
                || (state.completion == 2 && done.contains(&p.id)))
    };
    if q.is_empty() {
        return points.iter().filter(allowed).collect();
    }
    let mut cache = state.search_cache.borrow_mut();
    let identity = points.as_ptr() as usize;
    let changed = cache.points != identity
        || cache.texts.len() != points.len()
        || cache.custom_signature != state.custom_signature;
    if changed {
        // Ordinary map opening and panning keep the original light-weight pins.
        // Prepare approximate names only when the user actually searches.
        cache.texts=points.iter().map(|p|crate::search::CachedText::from_row(&json!({
            "name":p.name,"hint":p.search,"href":format!("#map/point/{}",p.id),
            "category":p.category,"aliases":[text(&p.raw,"en"),text(&p.raw,"nearest"),p.category]
        }))).collect();
    }
    if cache.query != q || changed {
        let query = crate::search::PreparedQuery::new(&q);
        cache.scores = points
            .iter()
            .zip(&cache.texts)
            .map(|(p, fuzzy)| {
                if q.split_whitespace().all(|word| p.search.contains(word)) {
                    Some(0)
                } else {
                    fuzzy.match_query(&query).map(|m| m.score)
                }
            })
            .collect();
        cache.query = q;
        cache.points = identity;
        cache.custom_signature = state.custom_signature.clone();
    }
    let mut results: Vec<_> = points
        .iter()
        .enumerate()
        .filter(|(_, p)| allowed(p))
        .filter_map(|(i, p)| cache.scores[i].map(|score| (score, p)))
        .collect();
    results.sort_by_key(|(score, _)| *score);
    results.into_iter().map(|(_, p)| p).collect()
}
fn color(cat: &str) -> Color32 {
    match cat {
        "ore" => Color32::from_rgb(190, 117, 68),
        "coal" => Color32::from_rgb(85, 89, 99),
        "sulfur" => Color32::from_rgb(225, 189, 54),
        "pure_quartz" => Color32::from_rgb(112, 211, 238),
        "hexolite_quartz" => Color32::from_rgb(218, 126, 216),
        "alpha_pals" => Color32::from_rgb(240, 159, 90),
        "custom" => Color32::from_rgb(94, 224, 168),
        "wild" => Color32::from_rgb(118, 211, 246),
        "alpha" => Color32::from_rgb(255, 180, 101),
        _ => Color32::from_rgb(154, 172, 236),
    }
}
/// Stable category priority: resource markers remain below destinations, bosses and user pins.
/// It must never depend on screen position, hash iteration or a cluster's current membership.
fn pin_layer(category: &str) -> u8 {
    match category {
        "custom" => 5,
        "alpha_pals" | "alpha" => 4,
        "fast_travel" | "warp_altar" => 3,
        "syndicate_tower" | "merchant" | "neutral_unit" | "bounty" => 2,
        "ore" | "coal" | "sulfur" | "pure_quartz" | "hexolite_quartz" | "chromite" | "soralite"
        | "paloxite" | "crude_oil" | "ancient_bark" | "ancient_bone" | "ancient_lava"
        | "nightstar_sand" | "beautiful_flower" | "kinship_peach" | "paldium" | "mushrooms"
        | "red_berries" | "wild" => 0,
        _ => 1,
    }
}
fn spatial_groups<'a>(visible: &[&'a Pin], map_rect: Rect, clip: Rect) -> Vec<Vec<&'a Pin>> {
    const CELL: f32 = 34.;
    let mut bins: HashMap<(&str, i32, i32), Vec<&Pin>> = HashMap::new();
    for p in visible {
        // Anchor the grid to the map, not the screen. Panning only translates a cluster;
        // it must not split/recombine it or change which marker is drawn above another.
        let local = p.uv * map_rect.width();
        let x = (local.x / CELL).floor() as i32;
        let y = (local.y / CELL).floor() as i32;
        let cell = Rect::from_min_size(
            map_rect.min + Vec2::new(x as f32 * CELL, y as f32 * CELL),
            Vec2::splat(CELL),
        );
        // Cull complete cells, preserving every member at viewport edges so that the
        // cluster center, count and identity cannot jump while it is entering the view.
        if !clip.expand(22.).intersects(cell) {
            continue;
        }
        bins.entry((p.category.as_str(), x, y))
            .or_default()
            .push(*p);
    }
    let mut groups = bins.into_values().collect::<Vec<_>>();
    for group in &mut groups {
        group.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    }
    groups.sort_unstable_by(|a, b| {
        (pin_layer(&a[0].category), &a[0].category, &a[0].id).cmp(&(
            pin_layer(&b[0].category),
            &b[0].category,
            &b[0].id,
        ))
    });
    groups
}
fn marker_geometry(group: &[&Pin], map_rect: Rect) -> (Vec2, egui::Pos2, Rect) {
    let uv = group.iter().fold(Vec2::ZERO, |sum, p| sum + p.uv) / group.len() as f32;
    let pos = map_rect.min + uv * map_rect.width();
    let rect = Rect::from_center_size(pos, Vec2::splat(if group.len() > 1 { 32. } else { 28. }));
    (uv, pos, rect)
}
/// Images and pins share a rectangular GPU clip. Cover the four small corner
/// sectors afterwards so zoomed terrain cannot paint over the rounded viewport.
fn rounded_map_mask(painter: &egui::Painter, clip: Rect, fill: Color32, border: Color32) {
    let radius = 15f32.min(clip.width() * 0.5).min(clip.height() * 0.5);
    let mut mesh = egui::Mesh::default();
    for (corner, sx, sy) in [
        (clip.left_top(), 1., 1.),
        (clip.right_top(), -1., 1.),
        (clip.right_bottom(), -1., -1.),
        (clip.left_bottom(), 1., -1.),
    ] {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, fill);
        for step in 0..=10 {
            let angle =
                -std::f32::consts::FRAC_PI_2 - step as f32 * std::f32::consts::FRAC_PI_2 / 10.;
            mesh.colored_vertex(
                corner
                    + Vec2::new(
                        sx * radius * (1. + angle.cos()),
                        sy * radius * (1. + angle.sin()),
                    ),
                fill,
            );
        }
        for step in 0..10 {
            mesh.add_triangle(base, base + step + 1, base + step + 2);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.rect_stroke(
        clip,
        radius,
        Stroke::new(1., border),
        egui::StrokeKind::Inside,
    );
}
fn overlapping_markers<'a>(
    groups: &[Vec<&'a Pin>],
    map_rect: Rect,
    pointer: Option<egui::Pos2>,
) -> Vec<&'a Pin> {
    let Some(pointer) = pointer else {
        return vec![];
    };
    let mut seen = HashSet::new();
    groups
        .iter()
        .rev()
        .filter(|group| marker_geometry(group, map_rect).2.contains(pointer))
        .flat_map(|group| group.iter().copied())
        .filter(|p| seen.insert(p.id.as_str()))
        .collect()
}
fn scene(
    ui: &mut egui::Ui,
    data: &Data,
    state: &mut MapState,
    visible: &[&Pin],
    done: &HashSet<String>,
    spawn: bool,
) -> Option<String> {
    let size = Vec2::new(
        ui.available_width().max(120.),
        ui.available_height().max(300.),
    );
    let (clip, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    state.viewport = clip;
    let mut route = None;
    if response.dragged() {
        let delta = ui.input(|i| i.pointer.delta());
        state.center -= delta / (clip.width().min(clip.height()) * state.zoom);
    }
    if response.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        if scroll.abs() > 0.1 || (pinch - 1.).abs() > 0.001 {
            let old = state.zoom;
            state.zoom = (state.zoom * (scroll * 0.002).exp() * pinch).clamp(1., 32.);
            if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
                let offset = cursor - clip.center();
                let fit = clip.width().min(clip.height());
                state.center += offset / (fit * old) - offset / (fit * state.zoom);
            }
        }
        ui.input(|i| {
            let step = 0.05 / state.zoom;
            if i.key_pressed(egui::Key::ArrowLeft) {
                state.center.x -= step
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                state.center.x += step
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                state.center.y -= step
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                state.center.y += step
            }
            if i.key_pressed(egui::Key::Plus) {
                state.zoom = (state.zoom * 1.5).min(32.);
            }
            if i.key_pressed(egui::Key::Minus) {
                state.zoom = (state.zoom / 1.5).max(1.);
            }
            if i.key_pressed(egui::Key::Home) {
                reset_camera(state)
            }
        });
    }
    state.center = state.center.clamp(Vec2::ZERO, Vec2::splat(1.));
    let side = clip.width().min(clip.height()) * state.zoom;
    let map_rect = Rect::from_min_size(clip.center() - state.center * side, Vec2::splat(side));
    let painter = ui.painter().with_clip_rect(clip);
    painter.rect_filled(clip, 15., Color32::from_rgb(12, 22, 28));
    let old_clip = ui.clip_rect();
    ui.set_clip_rect(old_clip.intersect(clip));
    if let Some(uri) = data.get("map-images")[&state.region].as_str() {
        egui::Image::new(data.image_source(uri)).paint_at(ui, map_rect);
    }
    let mut groups = spatial_groups(visible, map_rect, clip);
    // A selected marker has one explicit top layer, independent of hover and pan.
    groups.sort_by_key(|group| group.iter().any(|p| p.id == state.selected));
    // Hit-test the exact same rectangles as painting. Cross-category overlaps stay
    // visually separate, but clicking any shared part must offer every obscured point.
    let click_pos = ui.input(|i| {
        i.pointer
            .any_click()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    let overlaps = overlapping_markers(&groups, map_rect, click_pos);
    let cross_category_overlap = overlaps
        .iter()
        .map(|p| p.category.as_str())
        .collect::<HashSet<_>>()
        .len()
        > 1;
    for group in groups {
        let (uv, pos, rect) = marker_geometry(&group, map_rect);
        let p = group
            .iter()
            .find(|p| p.id == state.selected)
            .copied()
            .unwrap_or(group[0]);
        let id = ui.id().with(("map-pin", spawn, &p.id));
        let mark = ui.interact(rect, id, Sense::click());
        if group.len() > 1 {
            let fill = if spawn {
                Color32::from_rgb(139, 116, 57)
            } else {
                Color32::from_rgb(46, 80, 73)
            };
            painter.circle_filled(pos, 13., fill);
            painter.circle_stroke(
                pos,
                13.,
                Stroke::new(1.3_f32, Color32::from_rgb(237, 236, 211)),
            );
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                group.len().to_string(),
                egui::FontId::proportional(12.),
                Color32::WHITE,
            );
        } else if spawn {
            painter.circle_filled(
                pos,
                6.,
                if p.category == "alpha" {
                    Color32::from_rgb(218, 122, 73)
                } else {
                    Color32::from_rgb(211, 166, 66)
                },
            );
            painter.circle_stroke(
                pos,
                6.,
                Stroke::new(1.0_f32, Color32::from_rgb(245, 225, 176)),
            );
        } else if !p.art.is_empty() {
            if p.category == "alpha_pals" {
                painter.circle_filled(pos, 15., Color32::from_rgb(246, 240, 225));
                painter.circle_stroke(
                    pos,
                    15.,
                    Stroke::new(1.3_f32, Color32::from_rgb(178, 112, 156)),
                );
                egui::Image::new(data.image_source(&p.art))
                    .corner_radius(14)
                    .paint_at(ui, Rect::from_center_size(pos, Vec2::splat(28.)));
            } else {
                egui::Image::new(data.image_source(&p.art))
                    .paint_at(ui, Rect::from_center_size(pos, Vec2::splat(23.)));
            }
        } else {
            painter.circle_filled(pos, 7., color(&p.category));
        }
        if state.selected == p.id {
            painter.circle_stroke(
                pos,
                17.,
                Stroke::new(2.0_f32, Color32::from_rgb(255, 235, 119)),
            );
        }
        if done.contains(&p.id) {
            painter.text(
                pos + Vec2::new(11., 12.),
                egui::Align2::CENTER_CENTER,
                "✓",
                egui::FontId::proportional(14.),
                Color32::LIGHT_GREEN,
            );
        }
        if mark.clicked() {
            if cross_category_overlap {
                state.cluster = overlaps.iter().map(|p| p.id.clone()).collect();
                state.selected.clear();
                state.coordinate = None;
                state.delete_confirm = false;
            } else if group.len() == 1 {
                state.selected = p.id.clone();
                state.delete_confirm = false;
                state.cluster.clear();
                state.coordinate = None;
                if !spawn {
                    route = Some(format!("#map/point/{}", encode(&p.id)));
                }
            } else if state.zoom < 31.9
                && group
                    .iter()
                    .any(|other| (other.uv - p.uv).length() > 0.0001)
            {
                state.center = uv;
                state.zoom = (state.zoom * 2.).min(32.);
            } else {
                state.cluster = group.iter().map(|p| p.id.clone()).collect();
                state.selected.clear();
            }
        }
        mark.on_hover_ui(|ui| {
            if group.len() > 1 {
                ui.strong(format!("附近 {} 处点位", group.len()));
            }
            for p in group.iter().take(6) {
                ui.label(&p.name);
                if !p.levels.is_empty() {
                    ui.label(&p.levels);
                }
                ui.small(format!("{:.0}，{:.0}", p.x, p.y));
            }
            if group.len() > 6 {
                ui.small("点击放大或查看全部");
            }
        });
    }
    if let Some(uv) = state.coordinate {
        let pos = map_rect.min + uv * side;
        painter.circle_stroke(pos, 12., Stroke::new(3.0_f32, Color32::YELLOW));
        painter.line_segment(
            [pos - Vec2::new(18., 0.), pos + Vec2::new(18., 0.)],
            Stroke::new(2.0_f32, Color32::YELLOW),
        );
        painter.line_segment(
            [pos - Vec2::new(0., 18.), pos + Vec2::new(0., 18.)],
            Stroke::new(2.0_f32, Color32::YELLOW),
        );
    }
    if state.picking {
        painter.text(
            clip.center_top() + Vec2::new(0., 20.),
            egui::Align2::CENTER_TOP,
            "点击地图添加点位",
            egui::FontId::proportional(18.),
            Color32::YELLOW,
        );
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let uv = (pos - map_rect.min) / side;
                if (0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y) {
                    let (x, y) = coordinates(&state.region, uv);
                    accept_picked_position(state, x, y);
                }
            }
        }
    }
    let palette = crate::design::palette(ui.ctx());
    rounded_map_mask(&painter, clip, palette.bg, palette.line);
    ui.set_clip_rect(old_clip);
    route
}
fn nav(
    ui: &mut egui::Ui,
    out: &mut Option<String>,
    label: impl Into<egui::WidgetText>,
    route: String,
) {
    let icon = if route.starts_with("#paldex/") {
        "paldex"
    } else if route.starts_with("#spawns/") || route.starts_with("#map/") {
        "pin"
    } else if route.starts_with("#items/") {
        "items"
    } else if route.starts_with("#enemies/") {
        "enemy"
    } else if route.starts_with("#work/") {
        "ranch"
    } else {
        "book"
    };
    let color = crate::design::palette(ui.ctx()).accent;
    if ui
        .add(
            egui::Button::image_and_text(
                egui::Image::new(crate::design::icon_source(icon))
                    .fit_to_exact_size(Vec2::splat(14.))
                    .tint(color),
                label,
            )
            .frame(false)
            .wrap_mode(egui::TextWrapMode::Wrap),
        )
        .clicked()
    {
        *out = Some(route)
    }
}
fn details(
    ui: &mut egui::Ui,
    data: &Data,
    state: &mut MapState,
    p: &Pin,
    notes: &mut Value,
) -> Option<String> {
    let mut out = None;
    ui.horizontal_wrapped(|ui| {
        if !p.art.is_empty() {
            data.image(ui, &p.art, Vec2::splat(32.));
        }
        ui.label(egui::RichText::new(&p.name).size(17.).strong());
    });
    ui.label(format!("坐标 {:.0}，{:.0}", p.x, p.y));
    if !p.levels.is_empty() {
        ui.label(&p.levels);
    }
    if !p.detail.is_empty() {
        ui.label(&p.detail);
    }
    let mut done = done_set(notes).contains(&p.id);
    if crate::design::checkbox(ui, &mut done, "已完成").changed() {
        set_done(notes, &p.id, done);
    }
    let key = text(&p.raw, "palKey");
    if !key.is_empty() {
        nav(
            ui,
            &mut out,
            "查看帕鲁图鉴",
            format!("#paldex/{}", encode(key)),
        );
        nav(
            ui,
            &mut out,
            "查看刷新位置",
            format!("#spawns/{}", encode(key)),
        );
    }
    if let Some(cat) = array(&data.get("map-data")["categories"])
        .iter()
        .find(|c| text(c, "id") == p.category)
    {
        for (field, prefix, label) in [
            ("itemId", "#items/", "查看矿物 / 物品"),
            ("guide", "", "查看野外采集方式"),
        ] {
            let v = text(cat, field);
            if !v.is_empty() {
                nav(ui, &mut out, label, format!("{prefix}{v}"));
            }
        }
        if let Some(a) = cat["lootChapter"].as_array() {
            if let Some(id) = a.first().and_then(Value::as_str) {
                nav(
                    ui,
                    &mut out,
                    "查看探索奖励表",
                    format!("#guide/loot/{}", encode(id)),
                );
            }
        }
    }
    if p.category == "journal" {
        if let Some(journal) = array(&data.get("journals-data")["catalog"])
            .iter()
            .find(|j| text(j, "pointId") == p.id)
        {
            nav(
                ui,
                &mut out,
                "阅读手记",
                format!("#guide/journals/{}", encode(text(journal, "id"))),
            );
        }
    }
    let reward = text(&p.raw, "rewardId");
    if !reward.is_empty() {
        nav(
            ui,
            &mut out,
            format!("奖励 · {}", text(&p.raw, "rewardName")),
            format!("#items/{}", encode(reward)),
        );
    }
    for (field, prefix) in [
        ("enemyLinks", "#enemies/"),
        ("unitLinks", ""),
        ("questLinks", "#guide/quests/"),
    ] {
        for pair in array(&data.get("map-data")[field][&p.id]) {
            if let (Some(id), Some(label)) = (pair[0].as_str(), pair[1].as_str()) {
                nav(ui, &mut out, label, format!("{prefix}{id}"));
            }
        }
    }
    let nearest = text(&p.raw, "nearestId");
    if !nearest.is_empty() {
        nav(
            ui,
            &mut out,
            format!("最近传送点 · {}", text(&p.raw, "nearest")),
            format!("#map/point/{}", encode(nearest)),
        );
    }
    if p.category == "custom" {
        ui.horizontal(|ui| {
            if crate::design::action(ui, "edit", "编辑点位").clicked() {
                state.editor = Some(Editor {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    note: p.detail.clone(),
                    x: p.x,
                    y: p.y,
                    error: String::new(),
                });
            }
            if crate::design::action(ui, "trash", "删除点位").clicked() {
                state.delete_confirm = true;
            }
        });
        if state.delete_confirm {
            ui.label("确认删除这个自定义点位？");
            ui.horizontal(|ui| {
                if crate::design::action(ui, "trash", "确认删除").clicked() {
                    if let Some(a) = notes["custom"].as_array_mut() {
                        a.retain(|v| text(v, "id") != p.id);
                    }
                    set_done(notes, &p.id, false);
                    state.selected.clear();
                    state.delete_confirm = false;
                }
                if crate::design::action(ui, "close", "取消").clicked() {
                    state.delete_confirm = false;
                }
            });
        }
    }
    out
}
fn edit_window(ctx: &egui::Context, state: &mut MapState, notes: &mut Value) {
    let Some(editor) = state.editor.as_mut() else {
        return;
    };
    let mut save = false;
    let mut close = false;
    let mut repick = false;
    let title = if editor.id.is_empty() {
        "添加我的点位"
    } else {
        "编辑我的点位"
    };
    egui::Window::new(title)
        .id(egui::Id::new("map-editor"))
        .title_bar(false)
        .resizable(false)
        .show(ctx, |ui| {
            if crate::design::dialog_header(ui, "pin", title).clicked() {
                close = true;
            }
            ui.separator();
            ui.label("名称");
            ui.add(
                egui::TextEdit::singleline(&mut editor.name)
                    .char_limit(80)
                    .desired_width(340.),
            );
            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut editor.x));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut editor.y));
            });
            ui.small(region_name(&state.region));
            ui.label("备注");
            ui.add(
                egui::TextEdit::multiline(&mut editor.note)
                    .char_limit(600)
                    .desired_width(340.),
            );
            if !editor.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &editor.error);
            }
            ui.horizontal_wrapped(|ui| {
                repick = crate::design::action(ui, "pin", "在地图上选位置").clicked();
                save = crate::design::action(ui, "save", "保存点位").clicked();
                close = crate::design::action(ui, "close", "取消").clicked();
            });
        });
    if save {
        if editor.name.trim().is_empty() {
            editor.error = "请输入点位名称".into();
        } else if let Some(uv) = position(&state.region, editor.x, editor.y) {
            if !notes["custom"].is_array() {
                notes["custom"] = json!([]);
            }
            let id = if editor.id.is_empty() {
                format!("custom-{}", uuid::Uuid::new_v4())
            } else {
                editor.id.clone()
            };
            let value = json!({"id":id,"map":state.region,"u":uv.x,"v":uv.y,"name":editor.name.trim(),"note":editor.note.trim(),"done":done_set(notes).contains(&id)});
            let custom = notes["custom"].as_array_mut().unwrap();
            if let Some(old) = custom.iter_mut().find(|p| text(p, "id") == id) {
                *old = value;
            } else {
                custom.push(value);
            }
            state.selected = id;
            state.categories.insert("custom".into());
            state.search.clear();
            state.completion = 0;
            close = true;
        } else {
            editor.error = "坐标超出当前地图范围".into();
        }
    }
    if repick {
        start_picking(state);
    } else if close {
        state.editor = None;
    }
}
pub fn show(
    ui: &mut egui::Ui,
    data: &Data,
    state: &mut MapState,
    notes: &mut Value,
) -> Option<String> {
    if !data.ready("map-data") || !data.ready("map-images") {
        ui.spinner();
        ui.label("正在准备地图资料…");
        return None;
    }
    if state.points.is_empty() {
        state.points = Arc::new(
            array(&data.get("map-data")["points"])
                .iter()
                .map(|p| pin(data, p))
                .collect(),
        );
        state.categories = defaults(data);
    }
    if !notes.is_object() {
        *notes = json!({"completed":[],"custom":[]});
    }
    let points = all_points(data, state, notes);
    routes(data, state, &points);
    let done = done_set(notes);
    let mut out = None;
    ui.horizontal(|ui| {
        ui.small("地图区域");
        map_header(ui, state);
        ui.menu_button("坐标定位", |ui| coordinate_tools(ui, state));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::design::action(
                ui,
                if state.picking { "close" } else { "plus" },
                if state.picking {
                    "取消选点"
                } else {
                    "添加点位"
                },
            )
            .clicked()
            {
                if state.picking {
                    cancel_picking(state);
                } else {
                    start_picking(state);
                }
            }
            ui.small("点位进度随当前世界保存");
        });
    });
    let height = ui.available_height().max(300.);
    let sidebar = (ui.available_width() * 0.24).clamp(225., 292.);
    ui.spacing_mut().item_spacing.x = 13.;
    ui.horizontal_top(|ui| {
        let map_width = (ui.available_width() - sidebar - 13.).max(200.);
        ui.allocate_ui_with_layout(
            Vec2::new(map_width, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let visible = filtered(&points, state, &done);
                if let Some(route) = scene(ui, data, state, &visible, &done, false) {
                    out = Some(route);
                }
                map_tools(ui, state);
                if let Some(route) = floating_detail(ui, data, state, &points, notes) {
                    out = Some(route);
                }
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(sidebar, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                crate::design::card(ui).show(ui, |ui| {
                    ui.set_width(sidebar - 26.);
                    ui.set_height(height - 26.);
                    ui.spacing_mut().item_spacing = Vec2::new(6., 6.);
                    ui.spacing_mut().interact_size.y = 25.;
                    ui.spacing_mut().button_padding = Vec2::new(7., 4.);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("点位筛选").size(16.).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("清空").clicked() {
                                state.categories.clear();
                            }
                            if ui.small_button("全选").clicked() {
                                state.categories = array(&data.get("map-data")["categories"])
                                    .iter()
                                    .map(|c| text(c, "id").into())
                                    .chain(std::iter::once("custom".into()))
                                    .collect();
                            }
                        });
                    });
                    ui.add(
                        egui::TextEdit::singleline(&mut state.search)
                            .hint_text("搜索名称、资源或别名…")
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("map-completion")
                            .width(102.)
                            .selected_text(
                                ["全部状态", "未完成", "已完成"][state.completion as usize],
                            )
                            .show_ui(ui, |ui| {
                                for (i, label) in
                                    ["全部状态", "未完成", "已完成"].iter().enumerate()
                                {
                                    ui.selectable_value(&mut state.completion, i as u8, *label);
                                }
                            });
                        ui.small(format!(
                            "已完成 {} 处",
                            points
                                .iter()
                                .filter(|p| p.region == state.region && done.contains(&p.id))
                                .count()
                        ));
                    });
                    let counts = state
                        .category_counts
                        .get(&state.region)
                        .cloned()
                        .unwrap_or_default();
                    let cats = array(&data.get("map-data")["categories"])
                        .iter()
                        .cloned()
                        .chain(std::iter::once(json!({"id":"custom","name":"我的点位"})))
                        .filter(|c| {
                            counts.get(text(c, "id")).copied().unwrap_or(0) > 0
                                || text(c, "id") == "custom"
                        })
                        .collect::<Vec<_>>();
                    let limit = if state.categories_expanded {
                        cats.len()
                    } else {
                        cats.len().min(24)
                    };
                    let category_height = (height - 290.).clamp(130., 500.);
                    let cell = (ui.available_width() - 6.) / 2.;
                    egui::ScrollArea::vertical()
                        .id_salt("map-category-scroll")
                        .max_height(category_height)
                        .show(ui, |ui| {
                            egui::Grid::new("map-categories")
                                .num_columns(2)
                                .spacing(Vec2::new(6., 4.))
                                .show(ui, |ui| {
                                    for (i, cat) in cats.iter().take(limit).enumerate() {
                                        let id = text(cat, "id");
                                        let on = state.categories.contains(id);
                                        if category_button(
                                            ui,
                                            data,
                                            cat,
                                            counts.get(id).copied().unwrap_or(0),
                                            on,
                                            cell,
                                        )
                                        .clicked()
                                        {
                                            if on {
                                                state.categories.remove(id);
                                            } else {
                                                state.categories.insert(id.into());
                                            }
                                        }
                                        if i % 2 == 1 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                    if cats.len() > 24
                        && ui
                            .small_button(if state.categories_expanded {
                                "收起类别".into()
                            } else {
                                format!("展开全部 {} 类", cats.len())
                            })
                            .clicked()
                    {
                        state.categories_expanded = !state.categories_expanded;
                    }
                    let list = filtered(&points, state, &done);
                    ui.separator();
                    ui.small(format!("显示 {} 处点位 · 点击定位", list.len()));
                    if list.is_empty()
                        && !state.search.is_empty()
                        && ui.button("在全部类别中搜索").clicked()
                    {
                        state.categories = counts.keys().cloned().collect();
                    }
                    let remaining =
                        (height - (ui.cursor().top() - ui.min_rect().top()) - 48.).max(60.);
                    egui::ScrollArea::vertical()
                        .id_salt("map-points")
                        .max_height(remaining)
                        .auto_shrink([false, false])
                        .show_rows(ui, 45., list.len(), |ui, range| {
                            for i in range {
                                let p = list[i];
                                let (clicked, changed) = point_row(
                                    ui,
                                    data,
                                    p,
                                    state.selected == p.id,
                                    Some(done.contains(&p.id)),
                                );
                                if let Some(checked) = changed {
                                    set_done(notes, &p.id, checked);
                                }
                                if clicked {
                                    focus(state, p);
                                    out = Some(format!("#map/point/{}", encode(&p.id)));
                                }
                            }
                        });
                    egui::CollapsingHeader::new("资料来源").show(ui, |ui| {
                        for s in array(&data.get("map-data")["sources"]) {
                            ui.hyperlink_to(text(s, "name"), text(s, "url"));
                        }
                    });
                });
            },
        );
    });
    edit_window(ui.ctx(), state, notes);
    out
}

fn spawn_accept(time: u8, kind: u8, filter_time: u8, filter_kind: u8) -> bool {
    (filter_time == 0
        || (filter_time == 1 && time == 0)
        || (filter_time == 2 && (time == 0 || time == 1))
        || (filter_time == 3 && time == 1))
        && (filter_kind == 0 || (filter_kind == 1 && kind == 0) || (filter_kind == 2 && kind == 1))
}
fn load_spawns(data: &Data, state: &mut MapState) {
    let cache = (state.spawn_key.clone(), state.spawn_time, state.spawn_kind);
    if cache == state.spawn_cache {
        return;
    }
    state.spawn_cache = cache;
    let db = data.get("spawn-data");
    let mut sites: HashMap<usize, Pin> = HashMap::new();
    for record in array(&db["bySpecies"][&state.spawn_key]) {
        let Some(id) = record[0].as_u64().map(|n| n as usize) else {
            continue;
        };
        let time = record[3].as_u64().unwrap_or(2) as u8;
        let kind = record[4].as_u64().unwrap_or_default() as u8;
        if !spawn_accept(time, kind, state.spawn_time, state.spawn_kind) {
            continue;
        }
        let pos = &db["sites"][id];
        let region = db["mapIds"][pos[0].as_u64().unwrap_or_default() as usize]
            .as_str()
            .unwrap_or("palpagos");
        let line = format!(
            "Lv. {}–{} · {} · {}",
            record[1],
            record[2],
            if kind == 1 { "头目" } else { "野外" },
            match time {
                0 => "昼夜均可",
                1 => "仅夜间",
                _ => "时段未标注",
            }
        );
        let p = sites.entry(id).or_insert_with(|| Pin {
            id: format!("spawn-{id}"),
            region: region.into(),
            category: if kind == 1 {
                "alpha".into()
            } else {
                "wild".into()
            },
            name: data.pal_name(&state.spawn_key),
            detail: String::new(),
            uv: Vec2::new(
                pos[1].as_f64().unwrap_or_default() as f32,
                pos[2].as_f64().unwrap_or_default() as f32,
            ),
            x: pos[3].as_f64().unwrap_or_default(),
            y: pos[4].as_f64().unwrap_or_default(),
            levels: String::new(),
            art: String::new(),
            raw: Value::Null,
            search: String::new(),
        });
        if !p.levels.is_empty() {
            p.levels.push('\n');
        }
        p.levels.push_str(&line);
        if kind == 1 {
            p.category = "alpha".into();
        }
    }
    state.spawn_points = sites.into_values().collect();
    state.spawn_points.sort_by(|a, b| a.id.cmp(&b.id));
}
pub fn show_spawns(
    ui: &mut egui::Ui,
    data: &Data,
    state: &mut MapState,
    _notes: &mut Value,
) -> Option<String> {
    if !data.ready("spawn-data") || !data.ready("map-images") {
        ui.spinner();
        ui.label("正在准备刷新位置…");
        return None;
    }
    if state.route != state.last_route {
        state.last_route = state.route.clone();
        if let Some(key) = state.route.strip_prefix("#spawns/") {
            let key = decode(key);
            if data.pal(&key).is_some() {
                state.spawn_key = key;
                state.spawn_time = 0;
                state.spawn_kind = 0;
                state.selected.clear();
                reset_camera(state);
            }
        }
        load_spawns(data, state);
        if !state.spawn_points.iter().any(|p| p.region == state.region) {
            if let Some(p) = state.spawn_points.first() {
                state.region = p.region.clone();
            }
        }
    }
    let mut out = None;
    let previous = state.spawn_key.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("查找帕鲁");
        ui.add(
            egui::TextEdit::singleline(&mut state.spawn_search)
                .hint_text("名称 / 编号")
                .desired_width(160.),
        );
        egui::ComboBox::from_id_salt("spawn-species")
            .selected_text(data.pal_name(&state.spawn_key))
            .height(400.)
            .show_ui(ui, |ui| {
                let q = normalize(&state.spawn_search);
                for p in &data.species {
                    if crate::data::search(p, &q) {
                        ui.selectable_value(
                            &mut state.spawn_key,
                            text(p, "key").into(),
                            format!("{} · {}", text(p, "zh"), text(p, "dex")),
                        );
                    }
                }
            });
        egui::ComboBox::from_id_salt("spawn-time")
            .selected_text(
                ["全部时段", "白天可遇", "夜间可遇", "仅夜间出现"][state.spawn_time as usize],
            )
            .show_ui(ui, |ui| {
                for (i, s) in ["全部时段", "白天可遇", "夜间可遇", "仅夜间出现"]
                    .iter()
                    .enumerate()
                {
                    ui.selectable_value(&mut state.spawn_time, i as u8, *s);
                }
            });
        egui::ComboBox::from_id_salt("spawn-kind")
            .selected_text(["全部类型", "野外", "头目"][state.spawn_kind as usize])
            .show_ui(ui, |ui| {
                for (i, s) in ["全部类型", "野外", "头目"].iter().enumerate() {
                    ui.selectable_value(&mut state.spawn_kind, i as u8, *s);
                }
            });
    });
    if previous != state.spawn_key {
        reset_camera(state);
        state.selected.clear();
        out = Some(format!("#spawns/{}", encode(&state.spawn_key)));
    }
    load_spawns(data, state);
    ui.horizontal(|ui| {
        ui.small("地图区域");
        map_header(ui, state);
        ui.small("白天 / 夜间与头目位置可分别筛选，悬停查看等级。");
    });
    let shown = state
        .spawn_points
        .iter()
        .filter(|p| p.region == state.region)
        .cloned()
        .collect::<Vec<_>>();
    let height = ui.available_height().max(300.);
    let sidebar = (ui.available_width() * 0.24).clamp(250., 304.);
    ui.spacing_mut().item_spacing.x = 13.;
    ui.horizontal_top(|ui| {
        let map_width = (ui.available_width() - sidebar - 13.).max(200.);
        ui.allocate_ui_with_layout(
            Vec2::new(map_width, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                scene(
                    ui,
                    data,
                    state,
                    &shown.iter().collect::<Vec<_>>(),
                    &HashSet::new(),
                    true,
                );
                map_tools(ui, state);
                if let Some(p) = shown.iter().find(|p| p.id == state.selected) {
                    let clip = state.viewport;
                    let mut detail = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("spawn-floating-detail")
                            .max_rect(Rect::from_min_size(
                                clip.left_bottom() + Vec2::new(11., -168.),
                                Vec2::new(300., 130.),
                            )),
                    );
                    crate::design::card(&detail).show(&mut detail, |ui| {
                        ui.set_width(274.);
                        ui.horizontal(|ui| {
                            ui.strong(&p.name);
                            if crate::design::small_icon_button(ui, "close", "关闭").clicked() {
                                state.selected.clear();
                            }
                        });
                        ui.label(format!("坐标 {:.0}，{:.0}", p.x, p.y));
                        ui.label(&p.levels);
                        nav(
                            ui,
                            &mut out,
                            "在世界地图上定位",
                            format!("#map/coords/{}/{}/{}", p.region, p.x, p.y),
                        );
                    });
                }
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(sidebar, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                crate::design::card(ui).show(ui, |ui| {
                    ui.set_width(sidebar - 26.);
                    egui::ScrollArea::vertical()
                        .id_salt("spawn-profile")
                        .max_height((height * 0.39).clamp(155., 260.))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                data.portrait(ui, &state.spawn_key, 64.);
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(data.pal_name(&state.spawn_key))
                                            .size(21.)
                                            .strong(),
                                    );
                                    if let Some(p) = data.pal(&state.spawn_key) {
                                        ui.small(format!(
                                            "#{} · {}",
                                            text(p, "dex"),
                                            text(p, "en")
                                        ));
                                        ui.horizontal_wrapped(|ui| {
                                            for element in
                                                array(&p["types"]).iter().filter_map(Value::as_str)
                                            {
                                                ui.horizontal(|ui| {
                                                    crate::elements::image(ui, element, 17.);
                                                    ui.small(match element {
                                                        "neutral" => "无",
                                                        "fire" => "火",
                                                        "water" => "水",
                                                        "grass" => "草",
                                                        "electric" => "雷",
                                                        "ice" => "冰",
                                                        "ground" => "地",
                                                        "dark" => "暗",
                                                        "dragon" => "龙",
                                                        other => other,
                                                    });
                                                });
                                            }
                                        });
                                    }
                                });
                            });
                            let facts = &data.get("spawn-data")["profiles"][&state.spawn_key];
                            ui.horizontal_wrapped(|ui| {
                                for (key, label) in
                                    [("hp", "生命"), ("attack", "攻击"), ("defense", "防御")]
                                {
                                    if !facts[key].is_null() {
                                        crate::design::badge(ui, format!("{label} {}", facts[key]));
                                    }
                                }
                            });
                            ui.horizontal_wrapped(|ui| {
                                for work in array(&facts["work"]) {
                                    let name = work[0].as_str().unwrap_or("");
                                    let image =
                                        data.get("pal-details-data")["workIcons"][name].as_str();
                                    let label = format!("{} {}", name, work[1]);
                                    let response = if let Some(uri) = image {
                                        ui.add(
                                            egui::Button::image_and_text(
                                                egui::Image::new(data.image_source(uri))
                                                    .fit_to_exact_size(Vec2::splat(16.)),
                                                label,
                                            )
                                            .frame(false)
                                            .wrap_mode(egui::TextWrapMode::Wrap),
                                        )
                                    } else {
                                        ui.link(label)
                                    };
                                    if response.clicked() {
                                        out = Some(format!(
                                            "#work/efficiency/{}/{}",
                                            encode(&state.spawn_key),
                                            encode(name)
                                        ));
                                    }
                                }
                            });
                            if let Some(partner) = facts["partner"].as_str() {
                                nav(
                                    ui,
                                    &mut out,
                                    format!("伙伴技能 · {partner}"),
                                    format!("#details/partner/{}", encode(&state.spawn_key)),
                                );
                            }
                            ui.horizontal_wrapped(|ui| {
                                for (name, prefix) in [
                                    ("帕鲁图鉴", "#paldex/"),
                                    ("捕捉概率", "#details/capture/"),
                                    ("属性计算", "#details/stats/"),
                                ] {
                                    nav(
                                        ui,
                                        &mut out,
                                        name,
                                        format!("{prefix}{}", encode(&state.spawn_key)),
                                    );
                                }
                            });
                        });
                });
                let list_height = ui.available_height().max(130.);
                crate::design::card(ui).show(ui, |ui| {
                    ui.set_width(sidebar - 26.);
                    ui.label(
                        egui::RichText::new(format!("刷新位置 · {}", shown.len()))
                            .size(16.)
                            .strong(),
                    );
                    ui.small("点击坐标定位，鼠标悬停查看等级");
                    if shown.is_empty() {
                        if state.spawn_points.is_empty() {
                            ui.label("本版资料没有符合筛选条件的刷新记录。");
                        } else if ui.button("切换到有记录的地图").clicked() {
                            state.region = state.spawn_points[0].region.clone();
                            reset_camera(state);
                        }
                    }
                    if !state.cluster.is_empty() {
                        egui::CollapsingHeader::new("附近刷新记录")
                            .default_open(true)
                            .show(ui, |ui| {
                                for id in state.cluster.clone() {
                                    if let Some(p) = shown.iter().find(|p| p.id == id) {
                                        if ui
                                            .button(format!("{:.0}，{:.0} {}", p.x, p.y, p.levels))
                                            .clicked()
                                        {
                                            state.selected = p.id.clone();
                                            state.cluster.clear();
                                        }
                                    }
                                }
                            });
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("spawn-site-list")
                        .max_height((list_height - 105.).max(65.))
                        .auto_shrink([false, false])
                        .show_rows(ui, 45., shown.len(), |ui, range| {
                            for i in range {
                                let p = &shown[i];
                                if point_row(ui, data, p, state.selected == p.id, None).0 {
                                    state.center = p.uv;
                                    state.zoom = 5.;
                                    state.selected = p.id.clone();
                                }
                            }
                        });
                    egui::CollapsingHeader::new("资料来源").show(ui, |ui| {
                        for s in array(&data.get("spawn-data")["sources"]) {
                            ui.hyperlink_to(text(s, "name"), text(s, "url"));
                        }
                    });
                });
            },
        );
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approximate_map_search_keeps_all_locations_filters_and_cached_text() {
        let data = fixture();
        let points: Vec<_> = array(&data.get("map-data")["points"])
            .iter()
            .filter(|p| text(p, "category") == "anti_air")
            .map(|p| pin(&data, p))
            .collect();
        let mut state = MapState::default();
        state.categories = HashSet::from(["anti_air".into()]);
        assert_eq!(filtered(&points, &state, &HashSet::new()).len(), 11);
        assert!(
            state.search_cache.borrow().texts.is_empty(),
            "ordinary map display must not prepare fuzzy rows"
        );
        state.search = "天坠之地炮台".into();
        assert_eq!(filtered(&points, &state, &HashSet::new()).len(), 11);
        let prepared = state.search_cache.borrow().texts.as_ptr();
        assert_eq!(filtered(&points, &state, &HashSet::new()).len(), 11);
        assert_eq!(prepared, state.search_cache.borrow().texts.as_ptr());
        state.search = "防空袍".into();
        assert_eq!(filtered(&points, &state, &HashSet::new()).len(), 11);
        assert_eq!(prepared, state.search_cache.borrow().texts.as_ptr());
        let completed = HashSet::from([points[0].id.clone()]);
        state.completion = 1;
        assert_eq!(filtered(&points, &state, &completed).len(), 10);
        state.completion = 2;
        assert_eq!(filtered(&points, &state, &completed)[0].id, points[0].id);
        state.categories.clear();
        assert!(filtered(&points, &state, &completed).is_empty());
    }
    #[test]
    fn native_editor_repick_keeps_the_original_record_and_draft_text() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut state = MapState::default();
        state.editor = Some(Editor {
            id: "custom-kept".into(),
            name: "我的矿点".into(),
            note: "保留备注".into(),
            x: 10.,
            y: 20.,
            error: String::new(),
        });
        let mut notes = json!({"completed":[],"custom":[]});
        let input = |events| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900., 700.))),
            events,
            ..Default::default()
        };
        for _ in 0..2 {
            let _ = ctx.run(input(vec![]), |ctx| {
                edit_window(ctx, &mut state, &mut notes)
            });
        }
        let output = ctx.run(input(vec![]), |ctx| {
            edit_window(ctx, &mut state, &mut notes)
        });
        let pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "在地图上选位置" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("reposition action is visible");
        for pressed in [true, false] {
            let _ = ctx.run(
                input(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]),
                |ctx| edit_window(ctx, &mut state, &mut notes),
            );
        }
        assert!(state.picking && state.editor.is_none());
        assert_eq!(state.pick_draft.as_ref().unwrap().id, "custom-kept");
        accept_picked_position(&mut state, 120., -220.);
        let edited = state.editor.as_ref().unwrap();
        assert_eq!(
            (&*edited.id, &*edited.name, &*edited.note),
            ("custom-kept", "我的矿点", "保留备注")
        );
        assert_eq!((edited.x, edited.y), (120., -220.));
        start_picking(&mut state);
        cancel_picking(&mut state);
        assert!(!state.picking && state.pick_draft.is_none());
        assert_eq!(state.editor.as_ref().unwrap().note, "保留备注");
        assert_eq!(notes, json!({"completed":[],"custom":[]}));
    }
    #[test]
    fn rounded_viewport_covers_each_corner_but_keeps_the_map_center_clear() {
        let ctx = egui::Context::default();
        let clip = Rect::from_min_size(egui::pos2(20., 20.), Vec2::new(250., 180.));
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rounded_map_mask(ui.painter(), clip, Color32::RED, Color32::GRAY);
            });
        });
        let masks: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Mesh(m) => Some(m),
                _ => None,
            })
            .collect();
        let covered = |p: egui::Pos2| {
            masks.iter().any(|mesh| {
                mesh.indices.chunks_exact(3).any(|ids| {
                    let a = mesh.vertices[ids[0] as usize].pos;
                    let b = mesh.vertices[ids[1] as usize].pos;
                    let c = mesh.vertices[ids[2] as usize].pos;
                    let cross = |x: Vec2, y: Vec2| x.x * y.y - x.y * y.x;
                    let signs = [
                        cross(b - a, p - a),
                        cross(c - b, p - b),
                        cross(a - c, p - c),
                    ];
                    signs.iter().all(|s| *s >= -0.001) || signs.iter().all(|s| *s <= 0.001)
                })
            })
        };
        for p in [
            clip.left_top() + Vec2::splat(0.5),
            clip.right_top() + Vec2::new(-0.5, 0.5),
            clip.left_bottom() + Vec2::new(0.5, -0.5),
            clip.right_bottom() - Vec2::splat(0.5),
        ] {
            assert!(covered(p), "zoomed imagery must be covered at {p:?}");
        }
        assert!(
            !covered(clip.center()),
            "rounded corners must not cover map contents"
        );
    }
    fn marker(id: &str, category: &str, x: f32, y: f32) -> Pin {
        Pin {
            id: id.into(),
            region: "palpagos".into(),
            category: category.into(),
            name: id.into(),
            detail: String::new(),
            uv: Vec2::new(x / 1000., y / 1000.),
            x: x.into(),
            y: y.into(),
            levels: String::new(),
            art: String::new(),
            raw: Value::Null,
            search: String::new(),
        }
    }
    fn group_ids(groups: &[Vec<&Pin>]) -> Vec<Vec<String>> {
        groups
            .iter()
            .map(|g| g.iter().map(|p| p.id.clone()).collect())
            .collect()
    }
    #[test]
    fn panning_keeps_cluster_members_identity_and_layer_order() {
        let points = [
            marker("z-ore", "ore", 410., 410.),
            marker("a-ore", "ore", 430., 416.),
            marker("travel", "fast_travel", 420., 420.),
            marker("tower", "syndicate_tower", 422., 422.),
            marker("boss", "alpha_pals", 425., 425.),
            marker("custom", "custom", 427., 427.),
        ];
        let mut visible = points.iter().collect::<Vec<_>>();
        let clip = Rect::from_min_max(egui::pos2(-1000., -1000.), egui::pos2(2000., 2000.));
        let origin = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(1000.));
        let baseline = group_ids(&spatial_groups(&visible, origin, clip));
        assert_eq!(baseline.last().unwrap(), &vec!["custom"]);
        assert!(baseline.iter().any(|g| g == &vec!["a-ore", "z-ore"]));
        // Reverse data order and cross many former screen-grid boundaries in both axes.
        visible.reverse();
        for offset in [
            Vec2::new(7., 29.),
            Vec2::new(35., -46.),
            Vec2::new(-105., 73.),
        ] {
            let groups = spatial_groups(&visible, origin.translate(offset), clip);
            assert_eq!(group_ids(&groups), baseline);
            assert!(
                groups
                    .iter()
                    .all(|g| g.iter().all(|p| p.category == g[0].category))
            );
        }
    }
    #[test]
    fn clipping_preserves_complete_edge_clusters() {
        let points = [
            marker("left", "ore", 1., 20.),
            marker("right", "ore", 30., 20.),
        ];
        let visible = points.iter().collect::<Vec<_>>();
        let clip = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(300.));
        let origin = Rect::from_min_size(egui::pos2(-30., 0.), Vec2::splat(1000.));
        let groups = spatial_groups(&visible, origin, clip);
        // One member is outside the old per-point culling margin, but belongs to the
        // same visible cell. It must not disappear and move the center/count/ID.
        assert_eq!(group_ids(&groups), vec![vec!["left", "right"]]);
    }
    #[test]
    fn native_cross_category_overlap_lists_every_point_and_preserves_selected_layer() {
        let data = fixture();
        let ctx = egui::Context::default();
        let mut points = vec![
            marker("custom", "custom", 500., 500.),
            marker("merchant", "merchant", 500., 500.),
        ];
        for p in &mut points {
            p.levels = "Lv.1".into();
        }
        let mut state = MapState::default();
        state.zoom = 4.;
        let mut notes = json!({"completed":[],"custom":[]});
        fn render(
            ctx: &egui::Context,
            data: &Data,
            points: &[Pin],
            state: &mut MapState,
            notes: &mut Value,
            events: Vec<egui::Event>,
        ) -> (egui::FullOutput, Option<String>) {
            let mut route = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1000., 750.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        route = scene(
                            ui,
                            data,
                            state,
                            &points.iter().collect::<Vec<_>>(),
                            &HashSet::new(),
                            false,
                        );
                        if let Some(next) = floating_detail(ui, data, state, points, notes) {
                            route = Some(next);
                        }
                    });
                },
            );
            (output, route)
        }
        fn locate(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| locate(s, label)),
                _ => None,
            }
        }
        let click = |pos, state: &mut MapState, notes: &mut Value| {
            let mut result = None;
            for pressed in [true, false] {
                result = render(
                    &ctx,
                    &data,
                    &points,
                    state,
                    notes,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                )
                .1
                .or(result);
            }
            result
        };
        for target in ["merchant", "custom"] {
            for _ in 0..3 {
                render(&ctx, &data, &points, &mut state, &mut notes, vec![]);
            }
            let center = state.viewport.center();
            click(center, &mut state, &mut notes);
            assert!(state.selected.is_empty());
            assert_eq!(state.cluster.len(), 2);
            assert!(state.cluster.iter().any(|id| id == "merchant"));
            assert!(state.cluster.iter().any(|id| id == "custom"));
            render(&ctx, &data, &points, &mut state, &mut notes, vec![]);
            let output = render(&ctx, &data, &points, &mut state, &mut notes, vec![]).0;
            let label = format!("{target} · Lv.1");
            let pos = output
                .shapes
                .iter()
                .find_map(|s| locate(&s.shape, &label))
                .unwrap();
            let route = click(pos, &mut state, &mut notes);
            assert_eq!(state.selected, target);
            assert_eq!(
                route.as_deref(),
                Some(format!("#map/point/{target}").as_str())
            );
            assert!(state.cluster.is_empty());
            for offset in [Vec2::ZERO, Vec2::new(0.01, -0.01), Vec2::new(-0.01, 0.01)] {
                state.center = Vec2::splat(0.5) + offset;
                let output = render(&ctx, &data, &points, &mut state, &mut notes, vec![]).0;
                let top = output.shapes.iter().rev().find_map(|s| match &s.shape {
                    egui::Shape::Circle(c)
                        if c.fill == color("custom") || c.fill == color("merchant") =>
                    {
                        Some(c.fill)
                    }
                    _ => None,
                });
                assert_eq!(
                    top,
                    Some(color(target)),
                    "selected marker stays above the other category while panning"
                );
                assert_eq!(state.selected, target);
            }
            state.center = Vec2::splat(0.5);
        }
    }
    fn fixture() -> Data {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = [base.join("../../data"), base.join("../../rust-native/data")]
            .into_iter()
            .find(|p| p.join("map-data.json").exists())
            .expect("native data fixture");
        let mut data = Data::new(root, egui::Context::default()).unwrap();
        let start = std::time::Instant::now();
        while data.pending > 0 && start.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            data.poll();
        }
        assert!(data.errors.is_empty(), "{:?}", data.errors);
        assert_eq!(data.pending, 0);
        data
    }
    #[test]
    fn coordinate_roundtrip() {
        for region in ["palpagos", "worldtree"] {
            for uv in [Vec2::new(0.2, 0.3), Vec2::new(0.8, 0.7)] {
                let (x, y) = coordinates(region, uv);
                let back = position(region, x, y).unwrap();
                assert!((back - uv).length() < 0.003);
            }
        }
        assert!(position("palpagos", f64::NAN, 0.).is_none());
    }
    #[test]
    fn day_and_night_semantics() {
        assert!(spawn_accept(0, 0, 1, 1));
        assert!(!spawn_accept(1, 0, 1, 1));
        assert!(spawn_accept(1, 1, 2, 2));
        assert!(!spawn_accept(0, 1, 3, 0));
        assert!(spawn_accept(2, 1, 0, 0));
    }
    #[test]
    fn completed_custom_consistency() {
        let mut notes = json!({"completed":["a"],"custom":[{"id":"custom-1","done":true}]});
        set_done(&mut notes, "custom-1", false);
        assert_eq!(done_set(&notes), HashSet::from(["a".into()]));
        set_done(&mut notes, "a", false);
        assert!(done_set(&notes).is_empty());
    }
    #[test]
    fn routes_unicode() {
        let s = "叶泥泥（花朵）/12";
        assert_eq!(decode(&encode(s)), s);
    }
    #[test]
    fn switching_world_cancels_drafts_and_selection() {
        let mut s = MapState::default();
        s.set_world_id("a");
        s.picking = true;
        s.editor = Some(Editor {
            id: "custom-a".into(),
            name: "draft".into(),
            note: String::new(),
            x: 0.,
            y: 0.,
            error: String::new(),
        });
        s.selected = "custom-a".into();
        s.set_world_id("a");
        assert!(s.editor.is_some());
        s.set_world_id("b");
        assert!(s.editor.is_none());
        assert!(!s.picking);
        assert!(s.selected.is_empty());
    }
    #[test]
    fn shipped_points_spawns_and_inline_images() {
        let data = fixture();
        let mut state = MapState::default();
        state.points = Arc::new(
            array(&data.get("map-data")["points"])
                .iter()
                .map(|p| pin(&data, p))
                .collect(),
        );
        assert_eq!(state.points.len(), 12942);
        let mut notes = json!({"completed":[],"custom":[]});
        let points = all_points(&data, &mut state, &notes);
        assert!(Arc::ptr_eq(&points, &state.points));
        assert!(Arc::ptr_eq(&points, &all_points(&data, &mut state, &notes)));
        let rect = Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(900.));
        let mut count = 0;
        for region in ["palpagos", "worldtree"] {
            let visible = points
                .iter()
                .filter(|p| p.region == region)
                .collect::<Vec<_>>();
            let groups = spatial_groups(&visible, rect, rect);
            assert_eq!(groups.iter().map(Vec::len).sum::<usize>(), visible.len());
            count += visible.len();
        }
        assert_eq!(count, 12942);
        state.route = "#map/point/palpagos-alpha_pals-1".into();
        routes(&data, &mut state, &points);
        assert_eq!(state.selected, "palpagos-alpha_pals-1");
        assert!(state.categories.contains("alpha_pals"));
        set_done(&mut notes, &state.selected, true);
        state.completion = 2;
        assert_eq!(filtered(&points, &state, &done_set(&notes)).len(), 1);
        load_spawns(&data, &mut state);
        assert_eq!(
            state
                .spawn_points
                .iter()
                .filter(|p| p.region == "palpagos")
                .count(),
            350
        );
        let raw = data.get("portraits")["Lamball"].as_str().unwrap();
        let source = data.image_source(raw);
        if let egui::ImageSource::Bytes { bytes, .. } = source {
            assert!(image::load_from_memory(&bytes).is_ok());
        } else {
            panic!("inline portrait not decoded");
        }
        assert!(data.resolve("棉悠悠").is_some());
    }
    #[test]
    fn native_pointer_pan_and_wheel_preserve_annotations() {
        let data = fixture();
        let ctx = egui::Context::default();
        let mut state = MapState::default();
        state.route = "#map".into();
        let mut notes = json!({"completed":["palpagos-alpha_pals-1"],"custom":[]});
        let original = notes.clone();
        let mut render = |events: Vec<egui::Event>, state: &mut MapState| {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1440., 900.),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, &data, state, &mut notes);
                });
            });
        };
        render(vec![], &mut state);
        let initial = state.center;
        let a = egui::Pos2::new(80., 300.);
        let b = a + Vec2::new(90., 20.);
        render(
            vec![
                egui::Event::PointerMoved(a),
                egui::Event::PointerButton {
                    pos: a,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut state,
        );
        render(vec![egui::Event::PointerMoved(b)], &mut state);
        render(
            vec![egui::Event::PointerButton {
                pos: b,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut state,
        );
        assert!((state.center - initial).length() > 0.01);
        render(
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0., 120.),
                modifiers: egui::Modifiers::NONE,
            }],
            &mut state,
        );
        assert!(state.zoom > 1.);
        assert_eq!(notes, original);
    }
    #[test]
    fn native_custom_point_buttons_create_complete_edit_delete() {
        let data = fixture();
        let ctx = egui::Context::default();
        let mut state = MapState::default();
        let mut notes = json!({"completed":[],"custom":[]});
        let (x, y) = coordinates("palpagos", Vec2::splat(0.5));
        state.editor = Some(Editor {
            id: String::new(),
            name: "我的矿点".into(),
            note: "第一条备注".into(),
            x,
            y,
            error: String::new(),
        });
        fn render(
            ctx: &egui::Context,
            data: &Data,
            state: &mut MapState,
            notes: &mut Value,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1440., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        if let Some(v) = array(&notes["custom"])
                            .iter()
                            .find(|v| text(v, "id") == state.selected)
                            .cloned()
                        {
                            let mut v = v;
                            v["category"] = json!("custom");
                            let p = pin(data, &v);
                            details(ui, data, state, &p, notes);
                        }
                    });
                    edit_window(ctx, state, notes);
                },
            )
        }
        fn locate(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| locate(s, label)),
                _ => None,
            }
        }
        let click = |label: &str, state: &mut MapState, notes: &mut Value| {
            render(&ctx, &data, state, notes, vec![]);
            let output = render(&ctx, &data, state, notes, vec![]);
            let pos = output
                .shapes
                .iter()
                .find_map(|s| locate(&s.shape, label))
                .unwrap_or_else(|| panic!("missing button {label}"));
            for pressed in [true, false] {
                render(
                    &ctx,
                    &data,
                    state,
                    notes,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
        };
        click("保存点位", &mut state, &mut notes);
        assert_eq!(array(&notes["custom"]).len(), 1);
        let id = state.selected.clone();
        assert!(!id.is_empty());
        click("已完成", &mut state, &mut notes);
        assert!(done_set(&notes).contains(&id));
        click("编辑点位", &mut state, &mut notes);
        state.editor.as_mut().unwrap().name = "改名矿点".into();
        click("保存点位", &mut state, &mut notes);
        assert_eq!(array(&notes["custom"]).len(), 1);
        assert_eq!(notes["custom"][0]["name"], "改名矿点");
        assert_eq!(notes["custom"][0]["done"], true);
        click("删除点位", &mut state, &mut notes);
        assert_eq!(array(&notes["custom"]).len(), 1);
        click("确认删除", &mut state, &mut notes);
        assert!(array(&notes["custom"]).is_empty());
        assert!(!done_set(&notes).contains(&id));
    }
}
