//! Purpose-built native catalogue details, matching the handbook's information hierarchy.
use crate::{
    catalog::Entry,
    data::{Data, array, text},
};
use eframe::egui::{self, RichText, Vec2};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "catalog_views/items.rs"]
mod items;
#[path = "catalog_views/pals.rs"]
mod pals;
#[cfg(test)]
#[path = "catalog_views/tests.rs"]
mod tests;
#[path = "catalog_views/world.rs"]
mod world;

pub struct DetailState {
    pub quantity: u64,
    pub choices: Value,
    pub partner_level: usize,
    pub drop_tier: usize,
    pub product_query: String,
    last: String,
    craft_key: String,
    craft_plan: Value,
    methods_key: String,
    methods: BTreeMap<String, Vec<(String, String)>>,
    craft_settings: BTreeMap<String, (u64, Value)>,
}
impl Default for DetailState {
    fn default() -> Self {
        Self {
            quantity: 1,
            choices: json!({}),
            partner_level: 0,
            drop_tier: 0,
            product_query: String::new(),
            last: String::new(),
            craft_key: String::new(),
            craft_plan: Value::Null,
            methods_key: String::new(),
            methods: BTreeMap::new(),
            craft_settings: BTreeMap::new(),
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    d: &Data,
    page: &str,
    e: &Entry,
    route: &str,
    surgery: &mut Value,
    state: &mut DetailState,
) -> Option<String> {
    let mut nav = None;
    let identity = format!("{page}/{}", e.id);
    if state.last != identity {
        if let Some(id) = state.last.strip_prefix("items/") {
            state
                .craft_settings
                .insert(id.into(), (state.quantity, state.choices.clone()));
        }
        if page == "items" {
            let (quantity, choices) = state
                .craft_settings
                .get(&e.id)
                .cloned()
                .unwrap_or((1, json!({})));
            state.quantity = quantity;
            state.choices = choices;
        }
        state.last = identity;
        state.drop_tier = 0;
        state.product_query.clear();
    }
    ui.push_id((&e.route, "content"), |ui| {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            match page {
                "paldex" => pals::paldex(ui, d, e, state, &mut nav),
                "details/partner" => pals::partner(ui, d, e, state, &mut nav),
                "details/active" | "details/fruits" => pals::active(ui, d, e, page, &mut nav),
                "details/passive" | "details/surgery" => {
                    pals::passive(ui, d, e, page, surgery, &mut nav)
                }
                "items" => items::show(ui, d, e, route, state, &mut nav),
                "enemies" | "merchants" | "neutrals" => world::unit(ui, d, e, page, &mut nav),
                "work/facilities" => world::facility(ui, d, e, state, &mut nav),
                "work/expeditions" => world::expedition(ui, d, e, &mut nav),
                "guide/journals" => world::journal(ui, d, e, &mut nav),
                "guide/status" => world::status(ui, d, e, &mut nav),
                "guide/quests" => world::quest(ui, d, e, &mut nav),
                "guide/loot" => world::loot(ui, d, e, route, &mut nav),
                "guide/gathering" => world::gathering(ui, d, e, &mut nav),
                "work/production" => world::production(ui, d, e, &mut nav),
                _ => {
                    ui.label("请选择对应资料分类。");
                }
            }
        })
    });
    nav
}

/// The original surgery page starts with the actual facility and gender medicine,
/// before individual passive operations. Keep these visible in the list view too.
pub fn surgery_overview(ui: &mut egui::Ui, d: &Data) -> Option<String> {
    let mut nav = None;
    let table = |ui: &mut egui::Ui, nav: &mut Option<String>| {
        crate::design::card(ui).inner_margin(9).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if let Some(uri) = d.get("pal-details-data")["tableIcon"].as_str() {
                    if ui
                        .add(
                            egui::Image::new(d.image_source(uri))
                                .fit_to_exact_size(Vec2::splat(56.))
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text("建造帕鲁手术台")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *nav = Some("#work/facilities/Pal_Surgery_Table".into());
                    }
                }
                ui.vertical(|ui| {
                    action(
                        ui,
                        "#work/facilities/Pal_Surgery_Table",
                        "建造帕鲁手术台",
                        nav,
                    );
                    ui.small("科技 Lv. 38");
                });
            });
            ui.horizontal_wrapped(|ui| {
                for (id, label) in [
                    ("IronIngot", "精炼金属锭 ×30"),
                    ("Cloth2", "优质的布 ×20"),
                    ("Corrosive_Solvent", "腐蚀溶剂 ×10"),
                ] {
                    route(ui, format!("#items/{id}"), label, nav);
                }
            });
        });
    };
    let gender = |ui: &mut egui::Ui, nav: &mut Option<String>| {
        crate::design::card(ui).inner_margin(9).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if let Some(source) = item_image(d, "PalGenderReverse") {
                    if ui
                        .add(
                            egui::Image::new(source)
                                .fit_to_exact_size(Vec2::splat(56.))
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text("帕鲁性转药")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *nav = Some("#items/PalGenderReverse".into());
                    }
                }
                ui.vertical(|ui| {
                    action(ui, "#items/PalGenderReverse", "更改性别", nav);
                    ui.small("在手术台使用帕鲁性转药。");
                });
            });
            ui.small("性转药的获取方式可在物品图鉴中查看。");
        });
    };
    if ui.available_width() > 620. {
        ui.columns(2, |cols| {
            table(&mut cols[0], &mut nav);
            gender(&mut cols[1], &mut nav);
        });
    } else {
        table(ui, &mut nav);
        gender(ui, &mut nav);
    }
    ui.horizontal_wrapped(|ui| {
        ui.small("勾选已获取的植入体或持有耗材后，可在目标配种中使用。");
        action(ui, "#planner", "前往目标配种", &mut nav);
    });
    nav
}

fn number(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => "—".into(),
        _ => String::new(),
    }
}
fn quantity(lo: &Value, hi: &Value) -> String {
    if hi.is_null() || lo == hi {
        format!("× {}", number(lo))
    } else {
        format!("× {}–{}", number(lo), number(hi))
    }
}
fn percentage(v: &Value) -> String {
    if let Some(n) = v.as_f64() {
        if n != 0. && n.abs() < 0.001 {
            format!("{n:.2e}%")
        } else {
            format!(
                "{}%",
                format!("{n:.3}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
            )
        }
    } else {
        "—".into()
    }
}
fn section<R>(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(12.);
    crate::design::card(ui)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(18.).strong());
            ui.add_space(7.);
            body(ui)
        })
        .inner
}
fn hero(ui: &mut egui::Ui, d: &Data, e: &Entry, kind: &str, subtitle: &str) {
    ui.horizontal_top(|ui| {
        let page = entry_page(e);
        if matches!(page, "details/passive" | "details/surgery") {
            crate::breeding_ui::passive_rank(ui, d, e.value["rank"].as_i64().unwrap_or(0), 40.);
        } else if let Some(source) = entry_image(d, page, e) {
            let size = if kind == "pal" { 76. } else { 64. };
            ui.add(egui::Image::new(source).fit_to_exact_size(Vec2::splat(size)));
        }
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            if !subtitle.is_empty() {
                ui.weak(subtitle);
            }
            ui.label(RichText::new(&e.name).size(25.).strong());
        });
    });
}

/// One image resolver for list cards, search results and detail headings. Many
/// catalogue records reference another table's image instead of carrying `icon`.
pub fn entry_image(d: &Data, page: &str, e: &Entry) -> Option<egui::ImageSource<'static>> {
    if page == "enemies" {
        if let Some(source) = crate::game_art::enemy_source(&e.id) {
            return Some(source);
        }
    }
    if !e.icon.is_empty() {
        if let Some(uri) = d.get("enemy-data")["icons"][&e.icon].as_str() {
            return Some(d.image_source(uri));
        }
        if e.icon.starts_with("assets/") || e.icon.starts_with("data:image/") {
            return Some(d.image_source(&e.icon));
        }
    }
    if matches!(page, "paldex" | "details/partner" | "work/ranch") {
        return pal_image(d, &e.id);
    }
    if let Some(key) = e.value["palKey"]
        .as_str()
        .or_else(|| e.value["speciesKey"].as_str())
    {
        if let Some(source) = pal_image(d, key) {
            return Some(source);
        }
    }
    match page {
        "items" | "guide/gathering" => item_image(d, &e.id),
        "work/production" => item_image(d, text(&e.value, "item")),
        "details/fruits" => item_image(d, &format!("SkillCard_{}", e.id)).or_else(|| {
            d.get("pal-details-data")["fruitIcons"][text(&e.value, "type")]
                .as_str()
                .filter(|uri| !uri.trim().is_empty())
                .map(|uri| d.image_source(uri))
        }),
        "details/active" => crate::elements::source(text(&e.value, "type")),
        "details/passive" | "details/surgery" => {
            let rank = e.value["rank"].as_i64()?.unsigned_abs() as usize;
            d.get("trait-ranks")[rank]
                .as_str()
                .filter(|uri| !uri.trim().is_empty())
                .map(|uri| d.image_source(uri))
        }
        "guide/journals" => {
            let icons = d.get("map-images");
            let key = icons["mapIcons"]["defaults"]["journal"].as_str()?;
            icons["mapIcons"]["art"][key]
                .as_str()
                .filter(|uri| !uri.trim().is_empty())
                .map(|uri| d.image_source(uri))
        }
        _ => None,
    }
}
fn item_image(d: &Data, id: &str) -> Option<egui::ImageSource<'static>> {
    let icons = d.get("item-icons");
    let index = icons["ids"][id].as_u64()? as usize;
    icons["images"][index]
        .as_str()
        .filter(|uri| !uri.trim().is_empty())
        .map(|uri| d.image_source(uri))
}
fn pal_image(d: &Data, key: &str) -> Option<egui::ImageSource<'static>> {
    let pal = d.pal(key)?;
    d.get("portraits")[text(pal, "en")]
        .as_str()
        .filter(|uri| !uri.trim().is_empty())
        .map(|uri| d.image_source(uri))
}
fn element_image(key: &str) -> Option<egui::ImageSource<'static>> {
    crate::elements::source(key)
}
fn entry_page(e: &Entry) -> &str {
    let path = e.route.trim_start_matches('#');
    let Some((first, rest)) = path.split_once('/') else {
        return path;
    };
    if matches!(first, "details" | "work" | "guide") {
        let end = rest
            .find('/')
            .map(|i| first.len() + 1 + i)
            .unwrap_or(path.len());
        &path[..end]
    } else {
        first
    }
}
fn stats(ui: &mut egui::Ui, values: &[(&str, String)]) {
    ui.horizontal_wrapped(|ui| {
        for (label, value) in values {
            crate::design::stat(ui, label, value);
        }
    });
}

pub fn summary(page: &str, v: &Value) -> String {
    match page {
        "paldex" => format!(
            "#{} · {}",
            text(v, "dex"),
            array(&v["types"])
                .iter()
                .filter_map(Value::as_str)
                .map(element_name)
                .collect::<Vec<_>>()
                .join(" / ")
        ),
        "details/active" | "details/fruits" => format!(
            "{} · 威力 {} · {} 秒",
            element_name(text(v, "type")),
            number(&v["power"]),
            number(&v["cooldown"])
        ),
        "details/passive" | "details/surgery" => text(v, "effect").replace('\n', " · "),
        "details/partner" => text(v, "effect").into(),
        "items" => text(v, "category").into(),
        "work/facilities" => format!(
            "{}{}",
            text(v, "group"),
            if v["level"].as_i64().unwrap_or(0) > 0 {
                format!(" · Lv. {}", number(&v["level"]))
            } else {
                String::new()
            }
        ),
        "work/expeditions" => format!(
            "{} · {} 分钟 · 战力 {}",
            if v["hard"] == true {
                "高难"
            } else {
                "普通"
            },
            number(&v["minutes"]),
            number(&v["power"])
        ),
        "guide/journals" => format!(
            "{} · {}，{}",
            if v["map"] == "worldtree" {
                "世界树"
            } else {
                "帕洛斯群岛"
            },
            number(&v["x"]),
            number(&v["y"])
        ),
        "guide/status" => text(v, "group").into(),
        "guide/quests" => text(v, "summary").into(),
        "guide/loot" => format!("{} · {} 项奖励", text(v, "group"), array(&v["rows"]).len()),
        "guide/gathering" => text(v, "group").into(),
        "work/production" => format!("产出 · {}", text(v, "itemName")),
        "enemies" | "merchants" | "neutrals" => format!(
            "{}{}",
            text(v, "group"),
            if v["level"].is_null() {
                String::new()
            } else {
                format!(" · Lv. {}", number(&v["level"]))
            }
        ),
        _ => String::new(),
    }
}
fn route(
    ui: &mut egui::Ui,
    href: impl Into<String>,
    label: impl Into<String>,
    nav: &mut Option<String>,
) {
    let href = href.into();
    let label = label.into();
    if href.starts_with("https://") {
        ui.hyperlink_to(label, href);
        return;
    }
    let response = ui.link(&label);
    #[cfg(test)]
    track(ui, &response, &href);
    if response.clicked() {
        *nav = Some(href);
    }
}
fn action(ui: &mut egui::Ui, href: impl Into<String>, label: &str, nav: &mut Option<String>) {
    let href = href.into();
    let icon = if href.starts_with("#spawns") || href.starts_with("#map") {
        "pin"
    } else if href.starts_with("#planner") {
        "target"
    } else if href.starts_with("#lookup") {
        "branch"
    } else if href.starts_with("#work/ranch") {
        "ranch"
    } else if href.starts_with("#work/suitability") || href.starts_with("#work/efficiency") {
        "settings"
    } else if href.starts_with("#details/surgery") {
        "surgery"
    } else {
        "book"
    };
    let response = crate::design::action(ui, icon, label);
    #[cfg(test)]
    track(ui, &response, &href);
    if response.clicked() {
        *nav = Some(href);
    }
}
fn item(ui: &mut egui::Ui, d: &Data, id: &str, note: &str, nav: &mut Option<String>) {
    if id.is_empty() || id == "None" {
        return;
    }
    ui.horizontal(|ui| {
        if let Some(source) = item_image(d, id) {
            let response = ui
                .add(
                    egui::Image::new(source)
                        .fit_to_exact_size(Vec2::splat(26.))
                        .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(d.item_name(id));
            if response.clicked() {
                *nav = Some(format!("#items/{id}"));
            }
        }
        route(ui, format!("#items/{id}"), d.item_name(id), nav);
        if !note.is_empty() {
            ui.weak(note);
        }
    });
}
fn pal(ui: &mut egui::Ui, d: &Data, key: &str, note: &str, nav: &mut Option<String>) {
    ui.horizontal(|ui| {
        if let Some(source) = pal_image(d, key) {
            if ui
                .add(
                    egui::Image::new(source)
                        .fit_to_exact_size(Vec2::splat(30.))
                        .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(d.pal_name(key))
                .clicked()
            {
                *nav = Some(format!("#paldex/{key}"));
            }
        }
        route(ui, format!("#paldex/{key}"), d.pal_name(key), nav);
        if !note.is_empty() {
            ui.weak(note);
        }
    });
}
fn routes(ui: &mut egui::Ui, rows: &Value, nav: &mut Option<String>) {
    ui.horizontal_wrapped(|ui| {
        for r in array(rows) {
            if let (Some(h), Some(n)) = (r[0].as_str(), r[1].as_str()) {
                route(ui, h, n, nav);
            }
        }
    });
}
fn element_name(key: &str) -> &str {
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
fn element(ui: &mut egui::Ui, key: &str) {
    let color = match key {
        "fire" => egui::Color32::from_rgb(169, 72, 36),
        "water" => egui::Color32::from_rgb(44, 110, 161),
        "grass" => egui::Color32::from_rgb(75, 117, 49),
        "electric" => egui::Color32::from_rgb(140, 108, 25),
        "ice" => egui::Color32::from_rgb(45, 127, 138),
        "dark" => egui::Color32::from_rgb(113, 77, 151),
        _ => ui.visuals().text_color(),
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.;
        if let Some(source) = element_image(key) {
            ui.add(egui::Image::new(source).fit_to_exact_size(Vec2::splat(17.)));
        }
        ui.label(RichText::new(element_name(key)).color(color).strong());
    });
}
fn rank(ui: &mut egui::Ui, d: &Data, v: &Value) {
    let value = v.as_i64().unwrap_or(0);
    crate::breeding_ui::passive_rank(ui, d, value, 20.);
    ui.weak(if value < 0 {
        format!("负面词条 · 等级 {value}")
    } else {
        format!("词条等级 {value}")
    });
}
fn jobs(ui: &mut egui::Ui, d: &Data, rows: &Value, key: Option<&str>, nav: &mut Option<String>) {
    ui.horizontal_wrapped(|ui| {
        for r in array(rows) {
            let job = r[0].as_str().unwrap_or("");
            if job == "任意" {
                ui.label("任意种类帕鲁");
                continue;
            }
            if let Some(icon) = d.get("pal-details-data")["workIcons"][job].as_str() {
                d.image(ui, icon, Vec2::splat(23.));
            }
            let href = key
                .map(|key| format!("#work/efficiency/{key}/{job}"))
                .unwrap_or_else(|| format!("#work/efficiency/{job}"));
            route(ui, href, format!("{job} Lv. {}", number(&r[1])), nav);
        }
    });
}
fn rewards(ui: &mut egui::Ui, d: &Data, rows: &[Value], nav: &mut Option<String>) {
    let item_width = (ui.available_width() - 180.).clamp(150., 320.);
    egui::Grid::new("reward-table")
        .num_columns(3)
        .striped(true)
        .spacing([14., 8.])
        .show(ui, |ui| {
            ui.allocate_ui(egui::vec2(item_width, 20.), |ui| {
                ui.strong("物品");
            });
            ui.strong("数量");
            ui.strong("获得率");
            ui.end_row();
            for (i, r) in rows.iter().enumerate() {
                ui.push_id(i, |ui| {
                    item(
                        ui,
                        d,
                        r[0].as_str().unwrap_or(""),
                        r.get(4).and_then(Value::as_str).unwrap_or(""),
                        nav,
                    );
                });
                ui.label(quantity(&r[1], &r[2]));
                ui.label(percentage(&r[3]));
                ui.end_row();
            }
        });
}
fn locations(ui: &mut egui::Ui, points: &Value, nav: &mut Option<String>) {
    for p in array(points) {
        let suffix = if p["level"].is_null() {
            String::new()
        } else {
            format!(" · Lv. {}", number(&p["level"]))
        };
        route(
            ui,
            format!("#map/point/{}", text(p, "id")),
            format!(
                "{}（{}，{}）{suffix}",
                text(p, "name"),
                number(&p["x"]),
                number(&p["y"])
            ),
            nav,
        );
    }
}
fn resolved(ui: &mut egui::Ui, d: &Data, label: &str, nav: &mut Option<String>) {
    if let Some((href, name)) = d.resolve(label) {
        route(ui, href, name, nav);
    } else {
        ui.label(label);
    }
}
fn contextual(ui: &mut egui::Ui, d: &Data, page: &str, e: &Entry, nav: &mut Option<String>) {
    let rows = crate::links::context_routes(d, page, &e.id, &e.value);
    if !rows.is_empty() {
        section(ui, "相关地点与工具", |ui| {
            ui.horizontal_wrapped(|ui| {
                for (h, n) in rows {
                    route(ui, h, n, nav);
                }
            });
        });
    }
}

#[cfg(test)]
fn track(ui: &egui::Ui, response: &egui::Response, key: &str) {
    ui.ctx().data_mut(|data| {
        let mut rows = data
            .get_temp::<Vec<(String, egui::Rect)>>(egui::Id::new("catalog-controls"))
            .unwrap_or_default();
        rows.push((key.into(), response.rect));
        data.insert_temp(egui::Id::new("catalog-controls"), rows);
    });
}
