//! Dedicated work pages, matching the original handbook's compact picture cards.
use crate::data::{Data, array, normalize, text};
use eframe::egui::{self, Align, Color32, Rect, RichText, Sense, Vec2};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct RanchState {
    route: String,
    pal: String,
    item: String,
    pal_query: String,
    item_query: String,
    pal_level: usize,
    item_level: usize,
}
#[derive(Clone, Debug)]
pub struct RanchViewState {
    pal: String,
    item: String,
    pal_query: String,
    item_query: String,
    pal_level: usize,
    item_level: usize,
}

fn tracked(response: egui::Response, key: &str) -> egui::Response {
    #[cfg(test)]
    response
        .ctx
        .data_mut(|m| m.insert_temp(egui::Id::new(("work-control", key)), response.rect));
    let _ = key;
    response
}

fn image(d: &Data, pal: bool, key: &str) -> Option<egui::ImageSource<'static>> {
    if pal {
        let p = d.pal(key)?;
        d.get("portraits")[text(p, "en")]
            .as_str()
            .map(|uri| d.image_source(uri))
    } else {
        let icons = d.get("item-icons");
        let index = icons["ids"][key].as_u64()? as usize;
        icons["images"][index]
            .as_str()
            .map(|uri| d.image_source(uri))
    }
}

fn picture_choice(
    ui: &mut egui::Ui,
    d: &Data,
    pal: bool,
    key: &str,
    meta: &str,
    width: f32,
    selected: bool,
) -> egui::Response {
    let p = crate::design::palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 44.), Sense::click());
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
    painter.rect_filled(
        rect,
        6,
        if selected || response.hovered() {
            p.selected
        } else {
            p.soft
        },
    );
    painter.rect_stroke(
        rect,
        6,
        egui::Stroke::new(1., if selected { p.accent } else { p.line }),
        egui::StrokeKind::Inside,
    );
    if let Some(source) = image(d, pal, key) {
        egui::Image::new(source).paint_at(
            ui,
            Rect::from_center_size(rect.left_center() + Vec2::new(19., 0.), Vec2::splat(30.)),
        );
    }
    if meta.contains("暗属性") {
        if let Some(source) = crate::elements::source("dark") {
            egui::Image::new(source).paint_at(
                ui,
                Rect::from_center_size(rect.right_center() - Vec2::new(14., 0.), Vec2::splat(16.)),
            );
        }
    }
    let name = if pal {
        d.pal_name(key)
    } else {
        d.item_name(key)
    };
    let mut cell = ui.new_child(
        egui::UiBuilder::new()
            .layout(egui::Layout::top_down(Align::Min))
            .max_rect(Rect::from_min_max(
                rect.min + Vec2::new(39., 4.),
                rect.max - Vec2::new(if meta.contains("暗属性") { 26. } else { 4. }, 4.),
            )),
    );
    cell.set_clip_rect(rect.intersect(ui.clip_rect()));
    cell.spacing_mut().item_spacing.y = 2.;
    cell.add(
        egui::Label::new(RichText::new(&name).size(13.).strong())
            .selectable(false)
            .truncate(),
    );
    cell.add(
        egui::Label::new(RichText::new(meta).size(10.).color(p.muted))
            .selectable(false)
            .truncate(),
    );
    tracked(
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("{name}\n{meta}")),
        &format!("choice:{pal}:{key}"),
    )
}

fn mode_action(ui: &mut egui::Ui, icon: &str, label: &str, active: bool) -> egui::Response {
    let response = crate::design::action(ui, icon, label);
    if active {
        ui.painter().line_segment(
            [response.rect.left_bottom(), response.rect.right_bottom()],
            egui::Stroke::new(3., crate::design::palette(ui.ctx()).accent),
        );
    }
    response
}

fn picture_link(
    ui: &mut egui::Ui,
    d: &Data,
    pal: bool,
    key: &str,
    subtitle: &str,
    width: f32,
) -> egui::Response {
    picture_choice(ui, d, pal, key, subtitle, width, false)
}

fn pal_products(p: &Value) -> BTreeSet<String> {
    array(&p["tiers"])
        .iter()
        .flat_map(array)
        .filter_map(|r| r["item"].as_str().map(str::to_owned))
        .collect()
}
fn has_product(p: &Value, item: &str) -> bool {
    array(&p["tiers"])
        .iter()
        .flat_map(array)
        .any(|r| text(r, "item") == item)
}
fn count_suppliers(d: &Data, item: &str) -> usize {
    d.get("ranch-data")["pals"]
        .as_object()
        .map(|p| p.values().filter(|p| has_product(p, item)).count())
        .unwrap_or(0)
}
fn quantity(v: &Value) -> String {
    if v["min"] == v["max"] {
        v["min"].to_string()
    } else {
        format!("{}～{}", v["min"], v["max"])
    }
}

impl RanchState {
    pub fn navigation_state(&self) -> RanchViewState {
        RanchViewState {
            pal: self.pal.clone(),
            item: self.item.clone(),
            pal_query: self.pal_query.clone(),
            item_query: self.item_query.clone(),
            pal_level: self.pal_level,
            item_level: self.item_level,
        }
    }
    pub fn restore_navigation(&mut self, state: &RanchViewState, route: &str) {
        self.pal.clone_from(&state.pal);
        self.item.clone_from(&state.item);
        self.pal_query.clone_from(&state.pal_query);
        self.item_query.clone_from(&state.item_query);
        self.pal_level = state.pal_level;
        self.item_level = state.item_level;
        self.route = route.into();
    }
    pub fn show(&mut self, ui: &mut egui::Ui, d: &Data, route: &str) -> Option<String> {
        let Some(pals) = d.get("ranch-data")["pals"].as_object() else {
            ui.spinner();
            ui.label("正在准备牧场资料…");
            return None;
        };
        let producers: Vec<String> = d
            .species
            .iter()
            .map(|p| text(p, "key"))
            .filter(|key| pals.contains_key(*key))
            .map(str::to_owned)
            .collect();
        let mut products: Vec<String> = pals
            .values()
            .flat_map(pal_products)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        products.sort_by_key(|key| d.item_name(key));
        if self.pal.is_empty() {
            self.pal = producers.first().cloned().unwrap_or_default();
        }
        if self.item.is_empty() {
            self.item = if products.iter().any(|id| id == "Egg") {
                "Egg".into()
            } else {
                products.first().cloned().unwrap_or_default()
            };
        }
        let path = route.strip_prefix("#work/ranch/").unwrap_or("");
        let reverse = path.starts_with("item/");
        let id = path.strip_prefix("item/").unwrap_or(path);
        let candidates = if reverse { &products } else { &producers };
        if self.route != route {
            self.route = route.into();
            if candidates.iter().any(|candidate| candidate == id) {
                if reverse {
                    self.item = id.into();
                } else {
                    self.pal = id.into();
                }
            }
        }
        let selected = if reverse {
            self.item.clone()
        } else {
            self.pal.clone()
        };
        let query = if reverse {
            &mut self.item_query
        } else {
            &mut self.pal_query
        };
        let mut nav = None;
        let available = ui.available_width();
        let left_width = (available * 0.43).clamp(260., 700.);
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(left_width, height), egui::Layout::top_down(Align::Min), |ui| {
                crate::design::card(ui).show(ui, |ui| {
                    ui.set_width((left_width-24.).max(210.));
                    ui.horizontal_wrapped(|ui| {
                        if mode_action(ui,"ranch","按帕鲁查",!reverse).clicked() { nav=Some(format!("#work/ranch/{}",self.pal)); }
                        if mode_action(ui,"items","按材料查",reverse).clicked() { nav=Some(format!("#work/ranch/item/{}",self.item)); }
                    });
                    ui.separator();
                    ui.small(if reverse {"查找材料"} else {"查找帕鲁"});
                    ui.add(egui::TextEdit::singleline(query).desired_width(f32::INFINITY).hint_text(if reverse {"中文名、英文名或物品 ID"} else {"中文名、英文名或图鉴编号"}));
                    let q = normalize(query);
                    let matches: Vec<_> = candidates.iter().filter(|key| {
                        let name = if reverse {d.item_name(key)} else {d.pal_name(key)};
                        q.split_whitespace().all(|word| normalize(&format!("{name} {key} {}", if reverse {String::new()} else {d.pal(key).map(|p|format!("{} {}",text(p,"dex"),text(p,"en"))).unwrap_or_default()})).contains(word))
                    }).collect();
                    ui.small(format!("共 {} {}", matches.len(), if reverse {"种产物"} else {"种帕鲁"}));
                    let columns = ((ui.available_width()+6.)/124.).floor().max(1.) as usize;
                    let width = (ui.available_width()-6.*(columns-1) as f32)/columns as f32;
                    crate::navigation::ScrollArea::vertical().id_salt(("ranch-picture-list",reverse)).max_height((height-175.).max(140.)).auto_shrink([false,false]).show_rows(ui,44.,matches.len().div_ceil(columns),|ui,range| {
                        for row in range { ui.horizontal_top(|ui| { ui.spacing_mut().item_spacing.x=6.;
                            for key in matches.iter().skip(row*columns).take(columns) {
                                let meta = if reverse {format!("{} 种帕鲁",count_suppliers(d,key))} else {format!("#{} · {} 种产物",d.pal(key).map(|p|text(p,"dex")).unwrap_or(""),pal_products(&pals[*key]).len())};
                                if picture_choice(ui,d,!reverse,key,&meta,width,key.as_str()==selected).clicked() {nav=Some(format!("#work/ranch/{}{key}",if reverse {"item/"} else {""}));}
                            }
                        }); }
                    });
                    if matches.is_empty() {ui.weak("没有匹配的条目。");}
                });
            });
            ui.allocate_ui_with_layout(Vec2::new(ui.available_width(),height),egui::Layout::top_down(Align::Min),|ui| {
                crate::navigation::ScrollArea::vertical().id_salt(("ranch-picture-detail",reverse,&selected)).auto_shrink([false,false]).show(ui,|ui| {
                    crate::design::card(ui).show(ui,|ui| {
                        ui.set_width((ui.available_width()-1.).max(200.));
                        let empty = Value::Null;
                        let p = pals.get(&selected).unwrap_or(&empty);
                        ui.horizontal_wrapped(|ui| {
                            if let Some(source)=image(d,!reverse,&selected){let response=ui.add(egui::Image::new(source).fit_to_exact_size(Vec2::splat(54.)).sense(if reverse {egui::Sense::hover()} else {egui::Sense::click()}));if !reverse && response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked(){nav=Some(format!("#paldex/{selected}"));}}
                            ui.vertical(|ui| {let name=RichText::new(if reverse {d.item_name(&selected)} else {d.pal_name(&selected)}).size(22.).strong(); if reverse {ui.label(name);} else if ui.add(egui::Link::new(name)).clicked(){nav=Some(format!("#paldex/{selected}"));}
                                ui.small(if reverse {format!("{} 种帕鲁可放牧产出",count_suppliers(d,&selected))} else {format!("基础牧场 Lv.{} · 满浓缩 Lv.{}",p["base"],p["condensed"])});
                            });
                            if tracked(crate::design::action(ui,if reverse {"items"} else {"paldex"},if reverse {"物品图鉴"} else {"帕鲁图鉴"}),"ranch-dex").clicked(){nav=Some(format!("#{}{selected}",if reverse {"items/"} else {"paldex/"}));}
                        });
                        ui.separator();
                        let level = if reverse {&mut self.item_level} else {&mut self.pal_level};
                        let base = p["base"].as_u64().unwrap_or(1) as usize;
                        if !reverse && *level < base {*level=0;}
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(if reverse {"产出帕鲁"} else {"放牧产物"});
                            if let Some(uri)=d.get("pal-details-data")["workIcons"]["牧场"].as_str(){ui.add(egui::Image::new(d.image_source(uri)).fit_to_exact_size(Vec2::splat(18.)));}
                            ui.small("牧场工作适应性");
                            egui::ComboBox::from_id_salt("ranch-production-level").selected_text(if *level==0 {if reverse {"各自基础等级".into()} else {format!("基础 Lv.{base}")}} else {format!("Lv.{}",*level)}).show_ui(ui,|ui|{
                                ui.selectable_value(level,0,if reverse {"各自基础等级".into()} else {format!("基础 Lv.{base}")});
                                for i in (if reverse {1} else {base})..=10 {ui.selectable_value(level,i,format!("Lv.{i}"));}
                            });
                        });
                        egui::Grid::new(("ranch-yields",reverse,&selected)).striped(true).min_col_width(38.).spacing(Vec2::new(8.,7.)).show(ui,|ui| {
                            ui.strong(if reverse {"放牧帕鲁"} else {"材料"});ui.strong("数量");ui.strong("概率");ui.strong("图鉴");ui.end_row();
                            let others:Vec<String> = if reverse {producers.iter().filter(|key|has_product(&pals[*key],&selected)).cloned().collect()} else {pal_products(p).into_iter().collect()};
                            for other in others {
                                let (pal,item) = if reverse {(&other,&selected)} else {(&selected,&other)};
                                let data=&pals[pal];let natural=data["base"].as_u64().unwrap_or(1) as usize;
                                let effective=if *level==0 {natural} else {*level};
                                let drop=if effective<natural {None} else {array(&data["tiers"][effective.saturating_sub(1)]).iter().find(|row|text(row,"item")==item)};
                                let first=array(&data["tiers"]).iter().enumerate().find(|(i,t)|i+1>=natural&&array(t).iter().any(|r|text(r,"item")==item)).map(|(i,_)|i+1).unwrap_or(0);
                                let subtitle=if reverse {format!("牧场 Lv.{effective}")} else {"反查放牧来源".into()};
                                if picture_link(ui,d,reverse,&other,&subtitle,165.).clicked(){nav=Some(if reverse {format!("#paldex/{other}")} else {format!("#work/ranch/item/{other}")});}
                                if let Some(drop)=drop {ui.label(quantity(drop));ui.label(format!("{}%",drop["chance"]));} else {ui.weak(if effective<natural {format!("基础 Lv.{natural}")} else {format!("Lv.{first} 起")});ui.weak("—");}
                                if tracked(crate::design::icon_button(ui,if reverse {"paldex"} else {"items"},if reverse {"帕鲁图鉴"} else {"物品图鉴"}),&format!("ranch-wiki:{reverse}:{other}")).clicked(){nav=Some(format!("#{}{other}",if reverse {"paldex/"} else {"items/"}));}
                                ui.end_row();
                            }
                        });
                        ui.add_space(8.);ui.small("按帕鲁面板上的牧场工作适应性选择等级，包含浓缩和适应性手册带来的提升。每次产出抽取一种物品，数量为抽中后的个数。");
                        if !reverse {ui.horizontal_wrapped(|ui|{
                            if crate::design::action(ui,"branch","伙伴技能").clicked(){nav=Some(format!("#details/partner/{selected}"));}
                            if crate::design::action(ui,"ranch","工作适应性").clicked(){nav=Some(format!("#work/suitability/{selected}"));}
                        });}
                    });
                });
            });
        });
        nav
    }
}

pub fn suitability(
    ui: &mut egui::Ui,
    d: &Data,
    job: &mut String,
    query: &mut String,
    dark: &mut bool,
    context: Option<&str>,
) -> Option<String> {
    let mut nav = None;
    let jobs = d.get("pal-details-data")["workIcons"].as_object();
    let Some(jobs) = jobs else {
        ui.spinner();
        ui.label("正在准备工作图标…");
        return None;
    };
    if !jobs.contains_key(job.as_str()) {
        *job = if jobs.contains_key("手工作业") {
            "手工作业".into()
        } else {
            jobs.keys().next().cloned().unwrap_or_default()
        };
    }
    crate::design::card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(query)
                    .desired_width(230.)
                    .hint_text("搜索帕鲁名称、英文或编号"),
            );
            egui::ComboBox::from_id_salt("suitability-element")
                .selected_text(if *dark {
                    "仅暗属性"
                } else {
                    "全部属性"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(dark, false, "全部属性");
                    ui.selectable_value(dark, true, "仅暗属性");
                });
            if crate::design::action(ui, "close", "清除筛选").clicked() {
                query.clear();
                *dark = false;
            }
        });
        ui.add_space(5.);
        ui.horizontal_wrapped(|ui| {
            for (name, uri) in jobs {
                let mut label = name.clone();
                if let Some(key) = context {
                    if let Some(level) = array(&d.get("paldex-index-data")["profiles"][key]["work"])
                        .iter()
                        .find(|r| r[0].as_str() == Some(name))
                        .and_then(|r| r[1].as_u64())
                    {
                        label = format!("{name} Lv.{level}");
                    }
                }
                let image = egui::Image::new(d.image_source(uri.as_str().unwrap_or("")))
                    .fit_to_exact_size(Vec2::splat(20.));
                let mut button = egui::Button::image_and_text(
                    image,
                    RichText::new(label).color(if *job == *name {
                        Color32::WHITE
                    } else {
                        crate::design::palette(ui.ctx()).accent
                    }),
                )
                .selected(*job == *name);
                if *job == *name {
                    button = button.fill(crate::design::palette(ui.ctx()).accent);
                }
                let response = tracked(ui.add(button), &format!("job:{name}"));
                if response.clicked() {
                    *job = name.clone();
                }
            }
        });
    });
    if let Some(key) = context.filter(|key| d.pal(key).is_some()) {
        crate::design::card(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if d.portrait_link(ui, key, 42.) {
                    nav = Some(format!("#paldex/{key}"));
                }
                if ui.link(d.pal_name(key)).clicked() {
                    nav = Some(format!("#paldex/{key}"));
                }
                if crate::design::action(ui, "paldex", "帕鲁图鉴").clicked() {
                    nav = Some(format!("#paldex/{key}"));
                }
                if crate::design::action(ui, "pin", "刷新位置").clicked() {
                    nav = Some(format!("#spawns/{key}"));
                }
                if crate::design::action(ui, "list", "查看全部帕鲁").clicked() {
                    nav = Some("#work/suitability".into());
                }
            });
        });
    }
    let q = normalize(query);
    let mut rows: Vec<_> = d
        .species
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let profile = &d.get("paldex-index-data")["profiles"][text(p, "key")];
            let level = array(&profile["work"])
                .iter()
                .find(|r| r[0].as_str() == Some(job.as_str()))
                .and_then(|r| r[1].as_u64())
                .unwrap_or(0);
            let is_dark = array(&p["types"]).iter().any(|v| v == "dark");
            let matches = q.split_whitespace().all(|word| {
                normalize(&format!(
                    "{} {} {} {}",
                    text(p, "zh"),
                    text(p, "en"),
                    text(p, "dex"),
                    text(p, "key")
                ))
                .contains(word)
            });
            if level > 0 && matches && (!*dark || is_dark) {
                Some((i, p, level, is_dark))
            } else {
                None
            }
        })
        .collect();
    rows.sort_by_key(|(i, _, level, _)| (std::cmp::Reverse(*level), *i));
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{} 种帕鲁", rows.len()));
        if let Some(key) = context {
            if let Some(level) = array(&d.get("paldex-index-data")["profiles"][key]["work"])
                .iter()
                .find(|r| r[0].as_str() == Some(job.as_str()))
                .and_then(|r| r[1].as_u64())
            {
                ui.small(format!(
                    "{}的{}基础等级为 Lv.{level}。",
                    d.pal_name(key),
                    job
                ));
            }
        }
        if tracked(
            crate::design::action(ui, "ranch", &format!("计算{job}效率")),
            "work-efficiency",
        )
        .clicked()
        {
            nav = Some(format!(
                "#work/efficiency/{}{job}",
                context.map(|key| format!("{key}/")).unwrap_or_default()
            ));
        }
    });
    crate::navigation::ScrollArea::vertical()
        .id_salt(("work-rankings", job.as_str(), &q, *dark))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let levels: BTreeSet<_> = rows.iter().map(|(_, _, level, _)| *level).collect();
            for level in levels.into_iter().rev() {
                ui.add_space(7.);
                ui.horizontal(|ui| {
                    crate::design::badge(ui, format!("Lv. {level}"));
                    if let Some(uri) = jobs.get(job.as_str()).and_then(Value::as_str) {
                        ui.add(
                            egui::Image::new(d.image_source(uri))
                                .fit_to_exact_size(Vec2::splat(20.)),
                        );
                    }
                    ui.strong(job.as_str());
                    ui.small(format!(
                        "{} 种",
                        rows.iter().filter(|(_, _, lv, _)| *lv == level).count()
                    ));
                });
                let group: Vec<_> = rows.iter().filter(|(_, _, lv, _)| *lv == level).collect();
                let columns = ((ui.available_width() + 7.) / 150.).floor().max(1.) as usize;
                let width = (ui.available_width() - 7. * (columns - 1) as f32) / columns as f32;
                for group_row in group.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 7.;
                        for (_, p, _, is_dark) in group_row {
                            let key = text(p, "key");
                            let meta = format!(
                                "#{}{}",
                                text(p, "dex"),
                                if *is_dark {
                                    " · 暗属性 · 夜间工作"
                                } else {
                                    ""
                                }
                            );
                            if picture_choice(ui, d, true, key, &meta, width, context == Some(key))
                                .clicked()
                            {
                                nav = Some(format!("#paldex/{key}"));
                            }
                        }
                    });
                }
            }
            if rows.is_empty() {
                ui.weak("该工作下没有匹配的帕鲁，清除筛选可查看全部。");
            }
        });
    nav
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_restore_keeps_both_ranch_queries_and_production_levels() {
        let mut ranch = RanchState {
            route: "#work/ranch/item/Wool".into(),
            pal: "SheepBall".into(),
            item: "Wool".into(),
            pal_query: "绵".into(),
            item_query: "羊毛".into(),
            pal_level: 6,
            item_level: 8,
        };
        let snapshot = ranch.navigation_state();
        ranch.pal = "ChickenPal".into();
        ranch.item = "Egg".into();
        ranch.pal_level = 1;
        ranch.item_level = 1;
        ranch.item_query.clear();
        ranch.restore_navigation(&snapshot, "#work/ranch/item/Wool");
        assert_eq!(ranch.route, "#work/ranch/item/Wool");
        assert_eq!(ranch.pal, "SheepBall");
        assert_eq!(ranch.item, "Wool");
        assert_eq!(ranch.pal_query, "绵");
        assert_eq!(ranch.item_query, "羊毛");
        assert_eq!((ranch.pal_level, ranch.item_level), (6, 8));
    }
    struct Harness {
        ctx: egui::Context,
        d: Data,
        time: f64,
    }
    impl Harness {
        fn new() -> Self {
            let ctx = egui::Context::default();
            crate::design::apply(&ctx, "moon");
            egui_extras::install_image_loaders(&ctx);
            let mut d = Data::new(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
                ctx.clone(),
            )
            .unwrap();
            let start = std::time::Instant::now();
            while d.pending > 0 && start.elapsed().as_secs() < 10 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                d.poll();
            }
            assert!(d.errors.is_empty());
            assert_eq!(d.pending, 0);
            Self { ctx, d, time: 0. }
        }
        fn frame(
            &mut self,
            events: Vec<egui::Event>,
            mut body: impl FnMut(&mut egui::Ui, &Data) -> Option<String>,
        ) -> Option<String> {
            self.time += 0.04;
            let mut nav = None;
            let _ = self.ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1200., 900.),
                    )),
                    time: Some(self.time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| nav = body(ui, &self.d));
                },
            );
            nav
        }
        fn pos(&self, key: &str) -> egui::Pos2 {
            self.ctx
                .data(|m| m.get_temp::<Rect>(egui::Id::new(("work-control", key))))
                .unwrap_or_else(|| panic!("missing {key}"))
                .center()
        }
        fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        }
    }
    #[test]
    fn ranch_picture_rows_switch_direction_and_open_real_dex_destinations() {
        let mut h = Harness::new();
        let mut state = RanchState::default();
        let route = "#work/ranch/SheepBall";
        for _ in 0..3 {
            h.frame(vec![], |ui, d| state.show(ui, d, route));
        }
        let item = text(
            &h.d.get("ranch-data")["pals"]["SheepBall"]["tiers"][0][0],
            "item",
        )
        .to_owned();
        assert!(!item.is_empty());
        let pos = h.pos(&format!("choice:false:{item}"));
        h.frame(Harness::pointer(pos, true), |ui, d| {
            state.show(ui, d, route)
        });
        assert_eq!(
            h.frame(Harness::pointer(pos, false), |ui, d| state
                .show(ui, d, route)),
            Some(format!("#work/ranch/item/{item}"))
        );
        let reverse = format!("#work/ranch/item/{item}");
        for _ in 0..3 {
            h.frame(vec![], |ui, d| state.show(ui, d, &reverse));
        }
        assert_eq!(state.item, item);
        let pos = h.pos("choice:true:SheepBall");
        h.frame(Harness::pointer(pos, true), |ui, d| {
            state.show(ui, d, &reverse)
        });
        assert_eq!(
            h.frame(Harness::pointer(pos, false), |ui, d| state
                .show(ui, d, &reverse)),
            Some("#paldex/SheepBall".into())
        );
        let pos = h.pos("ranch-dex");
        h.frame(Harness::pointer(pos, true), |ui, d| {
            state.show(ui, d, &reverse)
        });
        assert_eq!(
            h.frame(Harness::pointer(pos, false), |ui, d| state
                .show(ui, d, &reverse)),
            Some(format!("#items/{item}"))
        );
        assert!(image(&h.d, true, "SheepBall").is_some());
        assert!(image(&h.d, false, &item).is_some());
    }
    #[test]
    fn suitability_job_icons_switch_rankings_and_worker_picture_opens_dex() {
        let mut h = Harness::new();
        let (mut job, mut query, mut dark) = ("生火".to_owned(), String::new(), false);
        for _ in 0..3 {
            h.frame(vec![], |ui, d| {
                suitability(ui, d, &mut job, &mut query, &mut dark, None)
            });
        }
        let pos = h.pos("job:浇水");
        h.frame(Harness::pointer(pos, true), |ui, d| {
            suitability(ui, d, &mut job, &mut query, &mut dark, None)
        });
        h.frame(Harness::pointer(pos, false), |ui, d| {
            suitability(ui, d, &mut job, &mut query, &mut dark, None)
        });
        assert_eq!(job, "浇水");
        let key =
            h.d.species
                .iter()
                .filter_map(|p| {
                    let key = text(p, "key");
                    let level = array(&h.d.get("paldex-index-data")["profiles"][key]["work"])
                        .iter()
                        .find(|r| r[0].as_str() == Some("浇水"))
                        .and_then(|r| r[1].as_u64())
                        .unwrap_or(0);
                    (level > 0).then_some((key.to_owned(), level))
                })
                .max_by_key(|(_, lv)| *lv)
                .unwrap()
                .0;
        query = key.clone();
        for _ in 0..3 {
            h.frame(vec![], |ui, d| {
                suitability(ui, d, &mut job, &mut query, &mut dark, None)
            });
        }
        let pos = h.pos(&format!("choice:true:{key}"));
        h.frame(Harness::pointer(pos, true), |ui, d| {
            suitability(ui, d, &mut job, &mut query, &mut dark, None)
        });
        assert_eq!(
            h.frame(Harness::pointer(pos, false), |ui, d| suitability(
                ui, d, &mut job, &mut query, &mut dark, None
            )),
            Some(format!("#paldex/{key}"))
        );
        let pos = h.pos("work-efficiency");
        h.frame(Harness::pointer(pos, true), |ui, d| {
            suitability(ui, d, &mut job, &mut query, &mut dark, None)
        });
        assert_eq!(
            h.frame(Harness::pointer(pos, false), |ui, d| suitability(
                ui, d, &mut job, &mut query, &mut dark, None
            )),
            Some("#work/efficiency/浇水".into())
        );
    }
}
