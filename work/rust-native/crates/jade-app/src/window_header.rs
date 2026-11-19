//! Window controls and a dedicated drag target inside the application toolbar.
use crate::design;
use eframe::egui::{self, Align, Layout as UiLayout, Pos2, Rect, Sense, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub content: Rect,
    pub drag: Rect,
    pub controls: Rect,
}

pub struct TabStrip {
    pub clicked: Option<String>,
    pub selected_rect: Option<Rect>,
    pub viewport: Rect,
    pub offset: f32,
    pub trailing_space: Option<Rect>,
}

/// Reveal the current page when navigation or the available width changes.
/// Explicit non-animated scrolling also works before a real-time animation has
/// advanced; retaining the key leaves subsequent manual scrolling untouched.
pub fn show_tabs(ui: &mut egui::Ui, tabs: &[(&str, &str)], page: &str) -> TabStrip {
    let width = ui.available_width().max(1.);
    let reveal_id = ui.make_persistent_id("tab-strip-reveal");
    let previous = ui
        .ctx()
        .data(|data| data.get_temp::<(String, f32)>(reveal_id));
    let reveal = previous.as_ref().is_none_or(|(last_page, last_width)| {
        last_page != page || (last_width - width).abs() > 0.5
    });
    let mut clicked = None;
    let output = egui::ScrollArea::horizontal()
        .id_salt("tab-scroll")
        .animated(false)
        .max_width(width)
        .min_scrolled_width(1.)
        .max_height(44.)
        .auto_shrink([false, true])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            let mut selected_rect = None;
            let mut last_right = ui.next_widget_position().x;
            ui.horizontal(|ui| {
                for (name, id) in tabs {
                    let selected = page == *id;
                    let response = design::tab_with_icon(ui, name, design::tab_icon(id), selected);
                    last_right = response.rect.right();
                    if selected {
                        selected_rect = Some(response.rect);
                        if reveal {
                            response.scroll_to_me(Some(Align::Center));
                        }
                    }
                    if response.clicked() {
                        clicked = Some(format!("#{id}"));
                    }
                }
            });
            (selected_rect, last_right)
        });
    ui.ctx()
        .data_mut(|data| data.insert_temp(reveal_id, (page.to_owned(), width)));
    let (selected_rect, last_right) = output.inner;
    let mut trailing_space = output.inner_rect;
    trailing_space.min.x = trailing_space
        .left()
        .max(last_right + ui.spacing().item_spacing.x);
    TabStrip {
        clicked,
        selected_rect,
        viewport: output.inner_rect,
        offset: output.state.offset.x,
        trailing_space: (trailing_space.width() >= 8.).then_some(trailing_space),
    }
}

/// Reserve native controls before allocating any tabs or conditional actions.
/// At the supported minimum window width, even a long tab list cannot cover
/// these areas. The narrow-window fallback also keeps every rectangle ordered.
pub fn geometry(rect: Rect) -> Layout {
    let controls_left = (rect.right() - 138.).max(rect.left());
    let drag_left = (controls_left - 40.).max(rect.left());
    Layout {
        content: Rect::from_min_max(rect.min, Pos2::new(drag_left, rect.bottom())),
        drag: Rect::from_min_max(
            Pos2::new(drag_left, rect.top()),
            Pos2::new(controls_left, rect.bottom()),
        ),
        controls: Rect::from_min_max(Pos2::new(controls_left, rect.top()), rect.max),
    }
}

pub fn show_controls(ui: &mut egui::Ui, rect: Rect) {
    show_controls_with_close(ui, rect, false);
}

/// The game overlay closes back into the game; the ordinary app still exits.
/// Native Alt+F4 and updater shutdown continue through their original paths.
pub fn show_controls_with_close(ui: &mut egui::Ui, rect: Rect, hide_on_close: bool) -> bool {
    let ctx = ui.ctx().clone();
    let modal_open = ctx.memory(|mem| mem.top_modal_layer().is_some());
    let palette = design::palette(&ctx);
    let mut hide_requested = false;
    // A separate registered Area remains above modal backdrops and is not
    // disabled together with the page. Only these three controls are exposed;
    // tabs, drag handles and the rest of the toolbar stay behind the modal.
    egui::Area::new(egui::Id::new("integrated-window-controls"))
        .order(egui::Order::Tooltip)
        .fixed_pos(rect.min)
        .default_size(Vec2::new(138., 48.))
        .movable(false)
        .constrain(false)
        .interactable(true)
        .sense(Sense::hover())
        .fade_in(false)
        .layout(UiLayout::right_to_left(Align::Center))
        .show(&ctx, |controls| {
            controls.set_min_size(Vec2::new(138., 48.));
            controls.set_max_size(Vec2::new(138., 48.));
            controls.set_clip_rect(rect.intersect(ctx.content_rect()));
            controls.spacing_mut().item_spacing = Vec2::ZERO;
            if modal_open {
                controls.painter().rect_filled(rect, 8, palette.surface);
            }
            if design::window_button(
                controls,
                "close",
                if hide_on_close {
                    "收起并返回游戏"
                } else {
                    "关闭"
                },
                palette.ink,
            )
            .clicked()
            {
                if hide_on_close {
                    hide_requested = true;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            let maximized = controls.input(|input| input.viewport().maximized.unwrap_or(false));
            if design::window_button(
                controls,
                if maximized { "restore" } else { "maximize" },
                if maximized { "还原" } else { "最大化" },
                palette.ink,
            )
            .clicked()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if design::window_button(controls, "minimize", "最小化", palette.ink).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        });
    hide_requested
}

/// Register only a dedicated drag area, never the tab/button rectangles.
/// The same interaction also makes the sidebar branding draggable without a
/// second title bar. Decorative grip dots are optional for the branding area.
pub fn drag_handle(ui: &mut egui::Ui, rect: Rect, id_salt: &str, paint_grip: bool) {
    let target = if paint_grip {
        rect.shrink2(Vec2::new(8., 0.))
    } else {
        rect
    };
    if !target.is_positive() {
        return;
    }
    let response = ui.interact(target, ui.id().with(id_salt), Sense::click_and_drag());
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    if paint_grip {
        let ink = design::palette(ui.ctx()).muted;
        let color = ink.gamma_multiply(if response.hovered() { 0.85 } else { 0.4 });
        for x in [-3., 3.] {
            for y in [-5., 0., 5.] {
                ui.painter()
                    .circle_filled(rect.center() + Vec2::new(x, y), 1., color);
            }
        }
    }
    response.on_hover_text("拖动窗口 · 双击最大化或还原");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        maximized: bool,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960., 620.))),
            events,
            time: Some(time),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .maximized = Some(maximized);
        ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let parts = geometry(Rect::from_min_size(
                        Pos2::new(108., 0.),
                        Vec2::new(852., 48.),
                    ));
                    show_controls(ui, parts.controls);
                    drag_handle(ui, parts.drag, "test-drag", true);
                });
        })
    }

    fn pointer(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
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

    #[test]
    fn game_overlay_close_requests_hide_without_exiting_the_toolbox() {
        let ctx = egui::Context::default();
        design::apply(&ctx, "moon");
        let mut hidden = false;
        let mut frame = |time, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960., 620.))),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ctx, |ui| {
                            hidden = show_controls_with_close(
                                ui,
                                geometry(Rect::from_min_size(
                                    Pos2::new(108., 0.),
                                    Vec2::new(852., 48.),
                                ))
                                .controls,
                                true,
                            );
                        });
                },
            )
        };
        frame(0., vec![]);
        frame(0.02, vec![]);
        let pos = Pos2::new(937., 24.);
        frame(0.1, pointer(pos, true));
        let output = frame(0.15, pointer(pos, false));
        assert!(
            !commands(&output)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close))
        );
        assert!(
            hidden,
            "the close button must return to the game without ending the app"
        );
    }

    fn commands(output: &egui::FullOutput) -> &[egui::ViewportCommand] {
        &output.viewport_output[&egui::ViewportId::ROOT].commands
    }

    #[test]
    fn integrated_controls_stay_separate_and_clickable_in_a_small_window() {
        let parts = geometry(Rect::from_min_size(
            Pos2::new(108., 0.),
            Vec2::new(852., 48.),
        ));
        assert_eq!(parts.controls.width(), 138.);
        assert_eq!(parts.drag.width(), 40.);
        assert!(parts.content.width() >= 600.);
        assert_eq!(parts.content.right(), parts.drag.left());
        assert_eq!(parts.drag.right(), parts.controls.left());
        assert_eq!(parts.controls.right(), 960.);
        for (button, maximized) in [(0, false), (1, false), (1, true), (2, false)] {
            let ctx = egui::Context::default();
            design::apply(&ctx, "jade");
            let mut time = 0.;
            draw(&ctx, time, vec![], maximized);
            time += 1.;
            draw(&ctx, time, vec![], maximized);
            let pos = Pos2::new(960. - 23. - 46. * button as f32, 24.);
            time += 1.;
            draw(&ctx, time, pointer(pos, true), maximized);
            time += 0.05;
            let output = draw(&ctx, time, pointer(pos, false), maximized);
            let cmds = commands(&output);
            assert!(match button {
                0 => cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
                1 => cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Maximized(value) if *value != maximized)),
                _ => cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Minimized(true))),
            }, "button {button}: {cmds:?}");
            assert!(
                !cmds
                    .iter()
                    .any(|c| matches!(c, egui::ViewportCommand::StartDrag))
            );
        }
    }

    #[test]
    fn dedicated_drag_target_supports_double_click_and_drag_without_tabs() {
        let ctx = egui::Context::default();
        design::apply(&ctx, "moon");
        draw(&ctx, 0., vec![], false);
        draw(&ctx, 0.02, vec![], false);
        let pos = Pos2::new(802., 24.);
        draw(&ctx, 0.04, pointer(pos, true), false);
        draw(&ctx, 0.06, pointer(pos, false), false);
        draw(&ctx, 0.08, pointer(pos, true), false);
        let output = draw(&ctx, 0.10, pointer(pos, false), false);
        assert!(
            commands(&output)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Maximized(true)))
        );

        draw(&ctx, 1., pointer(pos, true), false);
        let output = draw(
            &ctx,
            1.02,
            vec![egui::Event::PointerMoved(pos + Vec2::new(10., 0.))],
            false,
        );
        assert!(
            commands(&output)
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::StartDrag))
        );
    }

    fn draw_with_modal(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        modal_first: bool,
    ) -> (egui::FullOutput, bool, bool) {
        let mut tab_clicked = false;
        let mut modal_closed = false;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960., 620.))),
                events,
                time: Some(time),
                ..Default::default()
            },
            |ctx| {
                let modal = |ctx: &egui::Context| {
                    egui::Modal::new(egui::Id::new("test-world-archive"))
                        .show(ctx, |ui| {
                            ui.set_min_size(Vec2::new(350., 200.));
                            ui.label("世界档案");
                        })
                        .should_close()
                };
                if modal_first {
                    modal_closed |= modal(ctx);
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        tab_clicked = ui
                            .put(
                                Rect::from_min_size(Pos2::new(150., 6.), Vec2::new(100., 36.)),
                                egui::Button::new("原页面页签"),
                            )
                            .clicked();
                        let parts = geometry(Rect::from_min_size(
                            Pos2::new(108., 0.),
                            Vec2::new(852., 48.),
                        ));
                        show_controls(ui, parts.controls);
                    });
                if !modal_first {
                    modal_closed |= modal(ctx);
                }
            },
        );
        (output, tab_clicked, modal_closed)
    }

    #[test]
    fn integrated_window_controls_work_above_modals_without_unlocking_tabs() {
        for modal_first in [false, true] {
            for button in 0..3 {
                let ctx = egui::Context::default();
                design::apply(&ctx, "jade");
                // Allow both the modal and the floating controls to complete
                // their first-frame sizing and register their actual layers.
                for frame in 0..3 {
                    draw_with_modal(&ctx, frame as f64 * 0.02, vec![], modal_first);
                }
                let pos = Pos2::new(960. - 23. - 46. * button as f32, 24.);
                draw_with_modal(&ctx, 0.10, pointer(pos, true), modal_first);
                let (output, tab_clicked, modal_closed) =
                    draw_with_modal(&ctx, 0.15, pointer(pos, false), modal_first);
                let cmds = commands(&output);
                assert!(
                    match button {
                        0 => cmds
                            .iter()
                            .any(|c| matches!(c, egui::ViewportCommand::Close)),
                        1 => cmds
                            .iter()
                            .any(|c| matches!(c, egui::ViewportCommand::Maximized(true))),
                        _ => cmds
                            .iter()
                            .any(|c| matches!(c, egui::ViewportCommand::Minimized(true))),
                    },
                    "modal_first={modal_first}, button={button}: {cmds:?}"
                );
                assert!(!tab_clicked && !modal_closed);

                let tab = Pos2::new(200., 24.);
                draw_with_modal(&ctx, 1., pointer(tab, true), modal_first);
                let (_, tab_clicked, _) =
                    draw_with_modal(&ctx, 1.05, pointer(tab, false), modal_first);
                assert!(!tab_clicked, "window-control area must not enable the page");
            }
        }
    }

    const DETAIL_TABS: &[(&str, &str)] = &[
        ("主动技能", "details/active"),
        ("被动技能", "details/passive"),
        ("伙伴技能", "details/partner"),
        ("技能果实", "details/fruits"),
        ("帕鲁手术台", "details/surgery"),
        ("属性值计算", "details/stats"),
        ("捕捉概率", "details/capture"),
    ];

    fn draw_tabs(
        ctx: &egui::Context,
        time: f64,
        width: f32,
        page: &str,
        events: Vec<egui::Event>,
    ) -> TabStrip {
        let mut result = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960., 620.))),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let mut toolbar = ui.new_child(
                            egui::UiBuilder::new()
                                .id_salt("constrained-tabs")
                                .max_rect(Rect::from_min_size(
                                    Pos2::new(200., 0.),
                                    Vec2::new(width, 48.),
                                ))
                                .layout(UiLayout::left_to_right(Align::Center)),
                        );
                        toolbar.set_clip_rect(toolbar.max_rect());
                        result = Some(show_tabs(&mut toolbar, DETAIL_TABS, page));
                    });
            },
        );
        result.unwrap()
    }

    fn fully_visible(tabs: &TabStrip) -> bool {
        let selected = tabs.selected_rect.unwrap();
        selected.left() >= tabs.viewport.left() - 0.5
            && selected.right() <= tabs.viewport.right() + 0.5
    }

    #[test]
    fn last_tab_is_revealed_on_entry_and_after_width_changes() {
        let ctx = egui::Context::default();
        design::apply(&ctx, "moon");
        draw_tabs(&ctx, 0., 440., "details/capture", vec![]);
        let entered = draw_tabs(&ctx, 0.01, 440., "details/capture", vec![]);
        assert!(entered.offset > 0.);
        assert!(fully_visible(&entered));

        draw_tabs(&ctx, 0.02, 180., "details/capture", vec![]);
        let narrowed = draw_tabs(&ctx, 0.03, 180., "details/capture", vec![]);
        assert!(narrowed.viewport.width() <= 180.5);
        assert!(fully_visible(&narrowed));

        draw_tabs(&ctx, 0.04, 180., "details/active", vec![]);
        let navigated = draw_tabs(&ctx, 0.05, 180., "details/active", vec![]);
        assert!(fully_visible(&navigated));
    }

    #[test]
    fn manually_scrolling_tabs_away_does_not_snap_back_to_the_selected_page() {
        let ctx = egui::Context::default();
        design::apply(&ctx, "moon");
        draw_tabs(&ctx, 0., 180., "details/capture", vec![]);
        let entered = draw_tabs(&ctx, 0.02, 180., "details/capture", vec![]);
        assert!(fully_visible(&entered));
        let hover = entered.viewport.center();
        draw_tabs(
            &ctx,
            0.04,
            180.,
            "details/capture",
            vec![
                egui::Event::PointerMoved(hover),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(1000., 0.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let mut settled = None;
        for frame in 1..=30 {
            settled = Some(draw_tabs(
                &ctx,
                0.04 + frame as f64 / 30.,
                180.,
                "details/capture",
                vec![],
            ));
        }
        let settled = settled.unwrap();
        assert!(settled.offset < entered.offset - 50.);
        assert!(!fully_visible(&settled));
    }
}
