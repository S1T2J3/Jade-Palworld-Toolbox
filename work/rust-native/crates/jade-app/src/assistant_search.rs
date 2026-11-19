use crate::{
    breeding_ui,
    catalog::Catalog,
    data::{self, Data},
    design, search, search_rows,
};
use eframe::egui::{self, RichText, Vec2};

const RESULT_ROW_HEIGHT: f32 = 45.;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SearchAction {
    Ask(String),
    Open(String),
}

#[derive(Default)]
pub(crate) struct SearchBox {
    query: String,
    kind: String,
    index: Option<search::GlobalIndex>,
    indexed_tables: usize,
    kinds: Vec<String>,
    hits: Vec<search::SearchHit>,
    cache_key: Option<(String, String)>,
    selected: Option<usize>,
    fresh_editor: bool,
    composing: bool,
}

impl SearchBox {
    pub(crate) fn reset(&mut self) {
        self.kind.clear();
        self.clear_input();
    }

    pub(crate) fn clear_input(&mut self) {
        self.query.clear();
        self.selected = None;
        self.hits.clear();
        self.cache_key = None;
        self.fresh_editor = true;
        self.composing = false;
    }

    pub(crate) fn prefill(&mut self, question: String) {
        self.reset();
        self.query = question;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.query.trim().is_empty()
    }

    #[cfg(test)]
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        can_ask: bool,
    ) -> Option<SearchAction> {
        self.ui_with_catalog(ui, data, can_ask, None)
    }

    pub(crate) fn ui_with_catalog(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        can_ask: bool,
        catalog: Option<&Catalog>,
    ) -> Option<SearchAction> {
        let input_id = ui.make_persistent_id("assistant-search-question");
        if std::mem::take(&mut self.fresh_editor) {
            ui.ctx()
                .data_mut(|m| m.remove::<egui::text_edit::TextEditState>(input_id));
            ui.memory_mut(|m| m.request_focus(input_id));
        }
        // Inspect composition before TextEdit consumes its key events. Enter can
        // commit a Chinese IME candidate on the same frame as the commit event.
        let focused = ui.memory(|m| m.has_focus(input_id));
        let ime_frame =
            focused && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
        let composing = self.composing || ime_frame;
        if focused && ime_frame {
            ui.input(|i| {
                for event in &i.events {
                    if let egui::Event::Ime(ime) = event {
                        self.composing = match ime {
                            egui::ImeEvent::Preedit(value) => !value.is_empty(),
                            egui::ImeEvent::Enabled => true,
                            egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled => false,
                        };
                    }
                }
            });
        } else if !focused {
            self.composing = false;
        }
        let (up, down) = if focused && !composing && !self.hits.is_empty() {
            ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            })
        } else {
            (false, false)
        };
        if up || down {
            // egui queues directional focus changes before widgets see the first
            // arrow after focus was requested. Keep the caret in the composer.
            ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        let mut ask = false;
        let enter = ui
            .horizontal(|ui| {
                let width = (ui.available_width() - 76.).max(40.);
                let edit = design::clearable_text_edit(ui, &mut self.query, width, |editor| {
                    let editor = editor
                        .id(input_id)
                        .hint_text("搜索名称、编号或别名，也可以直接提问");
                    if composing {
                        editor.return_key(None)
                    } else {
                        editor
                    }
                });
                if edit.changed() {
                    self.selected = None;
                }
                let enter = !composing
                    && edit.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ask = ui
                    .add_enabled(can_ask && !self.is_empty(), egui::Button::new("问助手"))
                    .clicked();
                enter
            })
            .inner;
        // Keep the displayed rows in place until an in-flight click or key selection
        // is handled. Background tables can change both the ordering and the scroll offset.
        let index_outdated = self.indexed_tables != data.tables.len();
        let (pointer_down, pointer_released) =
            ui.input(|i| (i.pointer.primary_down(), i.pointer.primary_released()));
        let selecting = pointer_down || pointer_released || up || down || enter;
        if self.index.is_none() || (index_outdated && !selecting) {
            let index = search::GlobalIndex::new(&search_rows::all(data));
            self.kinds = index
                .rows()
                .iter()
                .map(|r| r.kind.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            self.index = Some(index);
            self.indexed_tables = data.tables.len();
            self.cache_key = None;
        } else if index_outdated && !pointer_down {
            ui.ctx().request_repaint();
        }
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("assistant-search-kind")
                .selected_text(if self.kind.is_empty() {
                    "全部类别"
                } else {
                    &self.kind
                })
                .show_ui(ui, |ui| {
                    let mut changed = ui
                        .selectable_value(&mut self.kind, String::new(), "全部类别")
                        .changed();
                    for kind in &self.kinds {
                        changed |= ui
                            .selectable_value(&mut self.kind, kind.clone(), kind.as_str())
                            .changed();
                    }
                    if changed {
                        ui.memory_mut(|m| m.request_focus(input_id));
                    }
                });
            if !self.is_empty() || !self.kind.is_empty() {
                ui.weak("↑ ↓ 选择结果，Enter 打开或提问");
            }
        });
        if self.is_empty() && self.kind.is_empty() {
            self.hits.clear();
            self.cache_key = None;
            if enter {
                ui.memory_mut(|m| m.request_focus(input_id));
            }
            return None;
        }
        let cache_key = (data::normalize(self.query.trim()), self.kind.clone());
        let index = self.index.as_ref().unwrap();
        let query_changed = self.cache_key.as_ref() != Some(&cache_key);
        if query_changed {
            self.hits = index.search(&cache_key.0, &cache_key.1, usize::MAX);
            self.cache_key = Some(cache_key);
            self.selected = None;
        }
        let found = &self.hits;
        if !found.is_empty() && (up || down) {
            let last = found.len() - 1;
            self.selected = Some(match (self.selected, up) {
                (Some(i), true) => i.saturating_sub(1),
                (Some(i), false) => (i + 1).min(last),
                (None, true) => last,
                (None, false) => 0,
            });
        }
        let mut action = None;
        if ask {
            action = Some(SearchAction::Ask(self.query.trim().to_owned()));
        } else if enter {
            let hit = self.selected.and_then(|i| found.get(i)).or_else(|| {
                found.first().filter(|hit| {
                    !self.is_empty() && hit.score <= 10 && !self.query.trim().ends_with(['?', '？'])
                })
            });
            if let Some(hit) = hit {
                action = Some(SearchAction::Open(index.rows()[hit.row_index].href.clone()));
            } else if can_ask && !self.is_empty() {
                action = Some(SearchAction::Ask(self.query.trim().to_owned()));
            } else {
                ui.memory_mut(|m| m.request_focus(input_id));
            }
        }
        if found.is_empty() {
            ui.weak("没有匹配的资料");
        } else {
            ui.weak(format!("{} 条结果", found.len()));
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("assistant-search-results")
                .max_height((ui.available_height() * 0.42).clamp(45., 200.));
            if query_changed {
                scroll = scroll.vertical_scroll_offset(0.);
            } else if up || down {
                scroll = scroll.vertical_scroll_offset(
                    self.selected.unwrap_or_default() as f32
                        * (RESULT_ROW_HEIGHT + ui.spacing().item_spacing.y),
                );
            }
            scroll.show_rows(ui, RESULT_ROW_HEIGHT, found.len(), |ui, range| {
                let p = design::palette(ui.ctx());
                for i in range {
                    let hit = &found[i];
                    let r = &index.rows()[hit.row_index];
                    let href = r.href.as_str();
                    let (_, rect) =
                        ui.allocate_space(Vec2::new(ui.available_width(), RESULT_ROW_HEIGHT));
                    let id = ui.make_persistent_id(("search-result", hit.row_index, href));
                    let response = ui.interact(rect, id, egui::Sense::click());
                    if Some(i) == self.selected || response.hovered() {
                        ui.painter().rect_filled(rect, 5, p.selected);
                    }
                    let mut row = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(("search-result-content", hit.row_index))
                            .max_rect(rect.shrink(3.))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    row.set_clip_rect(rect.intersect(ui.clip_rect()));
                    // All text belongs to the clickable row, including its hint.
                    row.style_mut().interaction.selectable_labels = false;
                    if href.starts_with("#details/passive/")
                        || href.starts_with("#details/surgery/")
                    {
                        let key = href.rsplit('/').next().unwrap_or("");
                        let rank = data.get("pal-details-data")["passives"][key]["rank"]
                            .as_i64()
                            .unwrap_or(0);
                        breeding_ui::passive_rank(&mut row, data, rank, 22.);
                    } else if !r.image.is_empty() {
                        row.add(
                            egui::Image::new(data.image_source(&r.image))
                                .fit_to_exact_size(Vec2::splat(30.)),
                        );
                    } else if let Some(source) = catalog.and_then(|c| c.image_for_route(data, href))
                    {
                        let image = egui::Image::new(source).fit_to_exact_size(Vec2::splat(30.));
                        row.add(image);
                    } else {
                        row.allocate_space(Vec2::splat(30.));
                    }
                    row.add_sized(
                        Vec2::new(72., 32.),
                        egui::Label::new(RichText::new(r.kind.as_str()).size(12.).color(p.muted))
                            .sense(egui::Sense::hover())
                            .halign(egui::Align::Min)
                            .truncate(),
                    );
                    let name_width = (row.available_width() * 0.47).max(90.);
                    row.add_sized(
                        Vec2::new(name_width, 32.),
                        egui::Label::new(RichText::new(r.name.as_str()).size(14.))
                            .sense(egui::Sense::hover())
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
                        .sense(egui::Sense::hover())
                        .halign(egui::Align::Min)
                        .truncate(),
                    );
                    if response
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!("{}\n{}\n{}", r.name, r.hint, hit.explanation))
                        .clicked()
                    {
                        action = Some(SearchAction::Open(href.into()));
                    }
                }
            });
        }
        if action.is_some() {
            self.clear_input();
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(ctx: &egui::Context) -> Data {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root, ctx.clone()).unwrap();
        data.tables.insert(
            "search-index-data".into(),
            std::sync::Arc::new(serde_json::json!([
                {"name":"蛋糕", "kind":"物品", "hint":"食物", "href":"#items/Cake", "aliases":["Cake"]},
                {"name":"蛋糕材料", "kind":"配方", "href":"#items/Ingredients"}
            ])),
        );
        data
    }

    fn frame(
        ctx: &egui::Context,
        entry: &mut SearchBox,
        data: &Data,
        events: Vec<egui::Event>,
    ) -> Option<SearchAction> {
        render(ctx, entry, data, events).0
    }

    fn render(
        ctx: &egui::Context,
        entry: &mut SearchBox,
        data: &Data,
        events: Vec<egui::Event>,
    ) -> (Option<SearchAction>, egui::FullOutput) {
        let mut action = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(820., 620.),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = entry.ui(ui, data, true);
                });
            },
        );
        (action, output)
    }

    fn result_label_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && text.galley.job.text.starts_with(label)
                {
                    let center = text.pos + text.galley.size() * 0.5;
                    shape.clip_rect.contains(center).then_some(center)
                } else {
                    None
                }
            })
            .expect("expected a visible result label")
    }

    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn clicking_the_result_name_category_or_hint_opens_the_same_entry() {
        let ctx = egui::Context::default();
        let data = data(&ctx);
        let mut entry = SearchBox::default();
        for label in ["蛋糕", "物品", "食物"] {
            entry.prefill("Cake".into());
            let (_, output) = render(&ctx, &mut entry, &data, vec![]);
            let center = result_label_center(&output, label);
            frame(
                &ctx,
                &mut entry,
                &data,
                vec![
                    egui::Event::PointerMoved(center),
                    pointer_button(center, true),
                ],
            );
            assert_eq!(
                frame(&ctx, &mut entry, &data, vec![pointer_button(center, false)]),
                Some(SearchAction::Open("#items/Cake".into())),
                "clicked {label}"
            );
            assert!(entry.is_empty());
        }
    }

    #[test]
    fn a_background_index_update_does_not_replace_a_result_during_a_click() {
        let ctx = egui::Context::default();
        let mut data = data(&ctx);
        let mut entry = SearchBox::default();
        entry.prefill("Cake".into());
        let (_, output) = render(&ctx, &mut entry, &data, vec![]);
        let center = result_label_center(&output, "蛋糕");
        frame(
            &ctx,
            &mut entry,
            &data,
            vec![
                egui::Event::PointerMoved(center),
                pointer_button(center, true),
            ],
        );

        data.tables.insert(
            "search-index-data".into(),
            std::sync::Arc::new(serde_json::json!([
                {"name":"新蛋糕", "kind":"物品", "href":"#items/NewCake", "aliases":["Cake"]}
            ])),
        );
        data.tables.insert(
            "loaded-table".into(),
            std::sync::Arc::new(serde_json::Value::Null),
        );
        frame(&ctx, &mut entry, &data, vec![]);
        assert_eq!(
            frame(&ctx, &mut entry, &data, vec![pointer_button(center, false)]),
            Some(SearchAction::Open("#items/Cake".into()))
        );

        entry.prefill("Cake".into());
        let (_, output) = render(&ctx, &mut entry, &data, vec![]);
        result_label_center(&output, "新蛋糕");
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn enter_opens_exact_alias_or_selected_result_and_asks_questions() {
        let ctx = egui::Context::default();
        let data = data(&ctx);
        let mut entry = SearchBox::default();
        let enter = || vec![key(egui::Key::Enter, egui::Modifiers::NONE)];
        for query in ["蛋糕", "Cake"] {
            entry.prefill(query.into());
            frame(&ctx, &mut entry, &data, vec![]);
            assert_eq!(
                frame(&ctx, &mut entry, &data, enter()),
                Some(SearchAction::Open("#items/Cake".into()))
            );
            assert!(entry.is_empty());
        }
        entry.prefill("蛋".into());
        frame(&ctx, &mut entry, &data, vec![]);
        assert_eq!(entry.hits.len(), 2);
        for expected in 0..2 {
            frame(
                &ctx,
                &mut entry,
                &data,
                vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            );
            assert_eq!(entry.selected, Some(expected));
        }
        assert_eq!(
            frame(&ctx, &mut entry, &data, enter()),
            Some(SearchAction::Open("#items/Ingredients".into()))
        );
        assert!(entry.is_empty());
        entry.prefill("蛋糕怎么做？".into());
        frame(&ctx, &mut entry, &data, vec![]);
        assert_eq!(
            frame(&ctx, &mut entry, &data, enter()),
            Some(SearchAction::Ask("蛋糕怎么做？".into()))
        );
        assert!(entry.is_empty());
    }

    #[test]
    fn committing_an_ime_candidate_keeps_the_editor_and_reset_discards_undo() {
        let ctx = egui::Context::default();
        let data = data(&ctx);
        let mut entry = SearchBox::default();
        entry.reset();
        frame(&ctx, &mut entry, &data, vec![]);
        frame(
            &ctx,
            &mut entry,
            &data,
            vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit("dangao".into())),
            ],
        );
        assert_eq!(
            frame(
                &ctx,
                &mut entry,
                &data,
                vec![
                    egui::Event::Ime(egui::ImeEvent::Commit("蛋糕".into())),
                    key(egui::Key::Enter, egui::Modifiers::NONE),
                ]
            ),
            None
        );
        assert_eq!(entry.query, "蛋糕");
        assert_eq!(
            frame(
                &ctx,
                &mut entry,
                &data,
                vec![key(egui::Key::Enter, egui::Modifiers::NONE)]
            ),
            Some(SearchAction::Open("#items/Cake".into()))
        );
        frame(&ctx, &mut entry, &data, vec![]);
        frame(
            &ctx,
            &mut entry,
            &data,
            vec![egui::Event::Text("旧草稿".into())],
        );
        assert_eq!(entry.query, "旧草稿");
        entry.kind = "物品".into();
        entry.reset();
        frame(&ctx, &mut entry, &data, vec![]);
        frame(
            &ctx,
            &mut entry,
            &data,
            vec![key(egui::Key::Z, egui::Modifiers::COMMAND)],
        );
        assert!(entry.is_empty());
        assert!(entry.kind.is_empty());
        assert!(entry.hits.is_empty());
        assert!(entry.selected.is_none());
    }
}
