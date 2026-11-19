use crate::{
    data::{Data, array, normalize, text},
    design,
};
use eframe::egui::{self, Align2, FontId, Id, Rect, Sense, Stroke, StrokeKind, Vec2};
use serde_json::Value;

#[derive(Default)]
pub(crate) struct Picker {
    open: bool,
    focus_search: bool,
    terms: String,
    species_count: usize,
    matches: Vec<usize>,
}

impl Picker {
    pub(crate) fn open(&mut self) {
        self.open = true;
        self.focus_search = true;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn dismiss(&mut self) -> bool {
        self.focus_search = false;
        std::mem::take(&mut self.open)
    }

    pub(crate) fn choose(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        selected: &mut usize,
        query: &mut String,
        title: &str,
    ) -> bool {
        let trigger = if let Some(pal) = data.species.get(*selected) {
            let image =
                egui::Image::new(portrait_source(data, pal)).fit_to_exact_size(Vec2::splat(29.));
            let label = format!("#{} {}  ·  更换", text(pal, "dex"), text(pal, "zh"));
            ui.add(egui::Button::image_and_text(image, label))
                .on_hover_text(title)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            design::action(ui, "search", title)
        };
        if trigger.clicked() {
            self.open();
        }
        if let Some(choice) = self.show(ui.ctx(), data, Some(*selected), query, title) {
            return std::mem::replace(selected, choice) != choice;
        }
        false
    }

    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        data: &Data,
        selected: Option<usize>,
        query: &mut String,
        title: &str,
    ) -> Option<usize> {
        if !self.is_open() {
            return None;
        }
        let id = Id::new(("pal-portrait-picker", title));
        let palette = design::palette(ctx);
        let viewport = ctx.content_rect();
        let width = (viewport.width() - 62.).clamp(280., 940.);
        // Keep the modal header below the native window controls at minimum size.
        let height = (viewport.height() - 248.).clamp(180., 630.);
        let mut chosen = None;
        let mut close = false;
        let modal = egui::Modal::new(id)
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(Stroke::new(1., palette.line))
                    .corner_radius(14)
                    .inner_margin(16),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                close = design::dialog_header(ui, "paldex", title).clicked();
                let search = design::clearable_text_edit(ui, query, f32::INFINITY, |edit| {
                    edit.id(id.with("search"))
                        .hint_text("搜索中文名、英文名或图鉴编号")
                });
                let just_opened = std::mem::take(&mut self.focus_search);
                if just_opened {
                    search.request_focus();
                }

                let terms = normalize(query);
                let updated = self.terms != terms || self.species_count != data.species.len();
                if updated {
                    self.matches = matching_species(&data.species, &terms);
                    self.terms = terms;
                    self.species_count = data.species.len();
                }
                ui.small(format!(
                    "{} / {} 只帕鲁",
                    self.matches.len(),
                    self.species_count
                ));

                // Enter in the search field accepts its best match. Leave IME commits to the editor.
                if search.lost_focus()
                    && ui.input(|input| {
                        input.key_pressed(egui::Key::Enter)
                            && !input
                                .events
                                .iter()
                                .any(|event| matches!(event, egui::Event::Ime(_)))
                    })
                {
                    chosen = if self.terms.trim().is_empty() {
                        selected.filter(|index| self.matches.contains(index))
                    } else {
                        None
                    }
                    .or_else(|| self.matches.first().copied());
                }

                let gap = ui.spacing().item_spacing;
                let grid_width = (width - 14.).max(1.);
                let columns = ((grid_width + gap.x) / (105. + gap.x)).floor().max(1.) as usize;
                let tile_width = (grid_width + gap.x) / columns as f32 - gap.x;
                let tile_height = 99.;
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt(id.with("grid"))
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false]);
                if just_opened || updated {
                    let row = if just_opened && self.terms.trim().is_empty() {
                        selected
                            .and_then(|index| self.matches.iter().position(|i| *i == index))
                            .unwrap_or_default()
                            / columns
                    } else {
                        0
                    };
                    scroll = scroll.vertical_scroll_offset(row as f32 * (tile_height + gap.y));
                }
                if self.matches.is_empty() {
                    scroll.show(ui, |ui| {
                        ui.centered_and_justified(|ui| ui.weak("找不到符合条件的帕鲁"));
                    });
                } else {
                    scroll.show_rows(
                        ui,
                        tile_height,
                        self.matches.len().div_ceil(columns),
                        |ui, rows| {
                            for row in rows {
                                ui.horizontal(|ui| {
                                    for &index in
                                        self.matches.iter().skip(row * columns).take(columns)
                                    {
                                        if portrait_tile(
                                            ui,
                                            id.with(index),
                                            data,
                                            &data.species[index],
                                            Vec2::new(tile_width, tile_height),
                                            selected == Some(index),
                                        )
                                        .clicked()
                                        {
                                            chosen = Some(index);
                                        }
                                    }
                                });
                            }
                        },
                    );
                }
            });
        if close || modal.should_close() {
            self.dismiss();
            return None;
        }
        if chosen.is_some() {
            self.dismiss();
        }
        chosen
    }
}

fn portrait_tile(
    ui: &mut egui::Ui,
    id: Id,
    data: &Data,
    pal: &Value,
    size: Vec2,
    selected: bool,
) -> egui::Response {
    let (_, rect) = ui.allocate_space(size);
    let response = ui.interact(rect, id, Sense::click());
    let name = text(pal, "zh");
    let dex = format!("#{}", text(pal, "dex"));
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            format!("{dex} {name}"),
        )
    });
    let palette = design::palette(ui.ctx());
    let highlight = selected || response.hovered() || response.has_focus();
    ui.painter().rect(
        rect,
        9.,
        if highlight {
            palette.selected
        } else {
            palette.soft
        },
        Stroke::new(
            1.,
            if highlight {
                palette.accent
            } else {
                palette.line
            },
        ),
        StrokeKind::Inside,
    );
    let portrait = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 31.),
        Vec2::splat(52.),
    );
    egui::Image::new(portrait_source(data, pal)).paint_at(ui, portrait);
    let painter = ui.painter().with_clip_rect(rect.shrink(4.));
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 69.),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(14.),
        palette.ink,
    );
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 86.),
        Align2::CENTER_CENTER,
        &dex,
        FontId::proportional(11.),
        palette.muted,
    );
    response
        .on_hover_text(format!("{dex} {name} · {}", text(pal, "en")))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn portrait_source(data: &Data, pal: &Value) -> egui::ImageSource<'static> {
    data.get("portraits")[text(pal, "en")]
        .as_str()
        .map(|uri| data.image_source(uri))
        .unwrap_or_else(|| design::icon_source("paldex"))
}

fn compact(text: &str) -> String {
    text.chars().filter(|ch| ch.is_alphanumeric()).collect()
}

fn without_dex_padding(dex: &str) -> &str {
    let normalized = dex.trim_start_matches('0');
    if normalized.is_empty() {
        dex
    } else {
        normalized
    }
}

fn matching_species(species: &[Value], normalized_query: &str) -> Vec<usize> {
    let query_key = compact(normalized_query);
    if query_key.is_empty() {
        return (0..species.len()).collect();
    }
    let words: Vec<_> = normalized_query
        .split_whitespace()
        .map(compact)
        .filter(|word| !word.is_empty())
        .collect();
    let mut ranked: Vec<_> = species
        .iter()
        .enumerate()
        .filter_map(|(index, pal)| {
            let mut names: Vec<_> = ["zh", "en", "key"]
                .iter()
                .map(|field| compact(&normalize(text(pal, field))))
                .collect();
            names.extend(
                array(&pal["aliases"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|name| compact(&normalize(name))),
            );
            let dex = compact(&normalize(text(pal, "dex")));
            let dex_unpadded = without_dex_padding(&dex);
            let query_unpadded = without_dex_padding(&query_key);
            let rank = if names.contains(&query_key) || dex_unpadded == query_unpadded {
                0
            } else if names.iter().any(|name| name.starts_with(&query_key))
                || dex.starts_with(&query_key)
                || dex_unpadded.starts_with(query_unpadded)
            {
                1
            } else if words.iter().all(|word| {
                names.iter().any(|name| name.contains(word))
                    || dex.starts_with(word)
                    || dex_unpadded.starts_with(without_dex_padding(word))
            }) {
                2
            } else {
                return None;
            };
            Some((rank, index))
        })
        .collect();
    ranked.sort_unstable();
    ranked.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn portrait_picker_searches_names_aliases_and_dex_numbers() {
        let species = [
            json!({"zh":"棉悠悠","en":"Lamball","key":"SheepBall","dex":"001","aliases":["绵悠悠"]}),
            json!({"zh":"雷米儿","en":"Sparkit Lux","key":"ExampleLux","dex":"007B"}),
            json!({"zh":"鲁米儿","en":"Sparkit","key":"Example","dex":"007"}),
            json!({"zh":"其他","en":"Other","key":"ExampleOther","dex":"107B"}),
        ];
        let find = |query: &str| matching_species(&species, &normalize(query));
        assert_eq!(find(""), [0, 1, 2, 3]);
        assert_eq!(find("悠悠"), [0]);
        assert_eq!(find("绵悠悠"), [0]);
        assert_eq!(find("SPARKIT LUX"), [1]);
        assert_eq!(find("sparkit"), [2, 1]);
        assert_eq!(find("＃００７ｂ"), [1]);
        assert_eq!(find("#7"), [2, 1]);
        assert_eq!(find("7b 雷"), [1]);
        assert!(find("没有匹配").is_empty());
    }
}
