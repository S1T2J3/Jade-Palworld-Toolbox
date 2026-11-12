//! Theme choices keep the handbook's previews, rather than showing color names alone.
use crate::design;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Vec2};

const PLAIN: [(&str, &str); 5] = [
    ("jade", "玉绿"),
    ("blue", "海蓝"),
    ("rose", "玫瑰"),
    ("amber", "琥珀"),
    ("slate", "岩灰"),
];

/// Returns a choice only after the user selects a card. Persistence stays with the app.
pub fn show(ui: &mut egui::Ui, current: &str) -> Option<String> {
    let mut selected = None;
    design::section(ui, "界面皮肤");
    let width = ui.available_width();
    if width >= 700. {
        let left = (width * 0.255).clamp(230., 316.);
        let right = (width - left - 18.).max(0.);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 18.;
            if moon_card(ui, left, current == "moon").clicked() {
                selected = Some("moon".into());
            }
            ui.vertical(|ui| {
                ui.set_width(right);
                ordinary(ui, right, current, &mut selected);
            });
        });
    } else {
        if moon_card(ui, width.min(316.), current == "moon").clicked() {
            selected = Some("moon".into());
        }
        ui.add_space(12.);
        ordinary(ui, width, current, &mut selected);
    }
    selected
}

fn ordinary(ui: &mut egui::Ui, width: f32, current: &str, selected: &mut Option<String>) {
    design::section(ui, "纯色皮肤");
    let columns = if width >= 660. {
        3
    } else if width >= 380. {
        2
    } else {
        1
    };
    let gap = 10.;
    let card_width = ((width - gap * (columns - 1) as f32) / columns as f32).max(0.);
    for row in PLAIN.chunks(columns) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for &(id, label) in row {
                if plain_card(ui, card_width, id, label, current == id).clicked() {
                    *selected = Some(id.into());
                }
            }
        });
    }
}

fn shell(ui: &mut egui::Ui, size: Vec2, selected: bool, id: &str) -> (Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let palette = design::palette(ui.ctx());
    ui.painter().rect_filled(
        rect,
        9,
        if response.hovered() {
            palette.soft
        } else {
            palette.surface
        },
    );
    ui.painter().rect_stroke(
        rect,
        9,
        Stroke::new(
            if selected { 1.7 } else { 1. },
            if selected {
                palette.accent
            } else {
                palette.line
            },
        ),
        egui::StrokeKind::Inside,
    );
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("skin-card", id)), rect));
    #[cfg(not(test))]
    let _ = id;
    (
        rect,
        response.on_hover_cursor(egui::CursorIcon::PointingHand),
    )
}

fn selection_dot(ui: &egui::Ui, center: Pos2, selected: bool) {
    let palette = design::palette(ui.ctx());
    ui.painter().circle_filled(center, 7., Color32::WHITE);
    ui.painter().circle_stroke(
        center,
        7.,
        Stroke::new(
            0.8,
            if selected {
                palette.accent
            } else {
                palette.line
            },
        ),
    );
    if selected {
        ui.painter().circle_filled(center, 4.3, palette.accent);
    }
}

fn moon_card(ui: &mut egui::Ui, width: f32, selected: bool) -> Response {
    let preview_height = width * 0.57;
    let (rect, response) = shell(ui, Vec2::new(width, preview_height + 55.), selected, "moon");
    let preview = Rect::from_min_size(
        rect.min + Vec2::splat(4.),
        Vec2::new(width - 8., preview_height),
    );
    moon_gradient(ui.painter(), preview);
    let star = |u: f32, v: f32, size: f32, color: Color32| {
        let c = preview.min + Vec2::new(preview.width() * u, preview.height() * v);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c - Vec2::new(0., size),
                c + Vec2::new(size * 0.35, 0.),
                c + Vec2::new(0., size),
                c - Vec2::new(size * 0.35, 0.),
            ],
            color,
            Stroke::NONE,
        ));
    };
    star(
        0.15,
        0.26,
        8.,
        Color32::from_rgba_unmultiplied(255, 246, 250, 185),
    );
    star(
        0.88,
        0.77,
        6.,
        Color32::from_rgba_unmultiplied(255, 245, 178, 225),
    );
    star(
        0.76,
        0.15,
        3.,
        Color32::from_rgba_unmultiplied(244, 251, 255, 170),
    );
    let image_size = (preview.height() - 10.).max(0.);
    egui::Image::new(egui::include_image!("../assets/jade-selyne.webp")).paint_at(
        ui,
        Rect::from_center_size(preview.center(), Vec2::splat(image_size)),
    );
    let palette = design::palette(ui.ctx());
    ui.painter().text(
        Pos2::new(rect.left() + 12., preview.bottom() + 11.),
        egui::Align2::LEFT_TOP,
        "辉月伊",
        egui::FontId::proportional(15.),
        palette.ink,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 12., preview.bottom() + 31.),
        egui::Align2::LEFT_TOP,
        "默认皮肤",
        egui::FontId::proportional(11.),
        palette.muted,
    );
    selection_dot(
        ui,
        Pos2::new(rect.right() - 18., preview.bottom() + 26.),
        selected,
    );
    response.on_hover_text("辉月伊 · 默认皮肤")
}

fn plain_card(ui: &mut egui::Ui, width: f32, id: &str, label: &str, selected: bool) -> Response {
    let (rect, response) = shell(ui, Vec2::new(width, 94.), selected, id);
    let colors = design::colors(id);
    let preview = Rect::from_min_size(rect.min + Vec2::splat(4.), Vec2::new(width - 8., 53.));
    let painter = ui.painter();
    painter.rect_filled(preview, 6, colors.bg);
    let rail = Rect::from_min_size(
        preview.min + Vec2::splat(3.),
        Vec2::new(29., preview.height() - 6.),
    );
    painter.rect_filled(rail, 3, colors.rail);
    painter.rect_filled(
        Rect::from_min_size(rail.min + Vec2::new(7., 10.), Vec2::new(16., 5.)),
        1,
        colors.selected,
    );
    let body = Rect::from_min_max(
        preview.min + Vec2::new(44., 10.),
        preview.max - Vec2::new(9., 9.),
    );
    painter.rect_filled(
        Rect::from_min_size(body.min, Vec2::new(body.width() * 0.46, 4.)),
        1,
        colors.accent,
    );
    let line = Rect::from_min_max(body.min + Vec2::new(0., 12.), body.max);
    painter.rect_filled(line, 2, colors.surface);
    painter.rect_filled(
        Rect::from_min_size(line.min, Vec2::new(line.width() * 0.4, line.height())),
        2,
        colors.selected,
    );
    painter.text(
        Pos2::new(rect.left() + 12., preview.bottom() + 12.),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(14.),
        design::palette(ui.ctx()).ink,
    );
    selection_dot(
        ui,
        Pos2::new(rect.right() - 18., preview.bottom() + 18.),
        selected,
    );
    response.on_hover_text(format!("使用{label}皮肤"))
}

fn moon_gradient(painter: &egui::Painter, rect: Rect) {
    // A rounded mesh keeps the gradient inside the preview without covering its corners.
    let color = |p: Pos2| {
        let t = (((p.x - rect.left()) / rect.width()) * 0.65
            + ((p.y - rect.top()) / rect.height()) * 0.35)
            .clamp(0., 1.);
        let from = [157., 216., 235.];
        let to = [184., 139., 221.];
        Color32::from_rgb(
            (from[0] + (to[0] - from[0]) * t) as u8,
            (from[1] + (to[1] - from[1]) * t) as u8,
            (from[2] + (to[2] - from[2]) * t) as u8,
        )
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.center(), color(rect.center()));
    let radius = 6.;
    for (center, start) in [
        (rect.left_top() + Vec2::splat(radius), std::f32::consts::PI),
        (
            rect.right_top() + Vec2::new(-radius, radius),
            std::f32::consts::PI * 1.5,
        ),
        (rect.right_bottom() - Vec2::splat(radius), 0.),
        (
            rect.left_bottom() + Vec2::new(radius, -radius),
            std::f32::consts::FRAC_PI_2,
        ),
    ] {
        for step in 0..=6 {
            let angle = start + std::f32::consts::FRAC_PI_2 * step as f32 / 6.;
            let p = center + Vec2::new(angle.cos(), angle.sin()) * radius;
            mesh.colored_vertex(p, color(p));
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for i in 1..=count {
        mesh.add_triangle(0, i, if i == count { 1 } else { i + 1 });
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        width: f32,
        current: &str,
        events: Vec<egui::Event>,
    ) -> Option<String> {
        let mut choice = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 1100.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| choice = show(ui, current));
            },
        );
        choice
    }

    #[test]
    fn every_preview_card_selects_its_theme_by_pointer_and_fits_wide_and_small_windows() {
        for width in [1290., 840., 620., 320.] {
            let ctx = egui::Context::default();
            egui_extras::install_image_loaders(&ctx);
            design::apply(&ctx, "moon");
            assert!(frame(&ctx, width, "moon", vec![]).is_none());
            frame(&ctx, width, "moon", vec![]);
            for id in ["moon", "jade", "blue", "rose", "amber", "slate"] {
                let rect = ctx
                    .data(|d| d.get_temp::<Rect>(egui::Id::new(("skin-card", id))))
                    .expect("visible theme card");
                assert!(
                    rect.left() >= 0. && rect.right() <= width,
                    "{id} leaves width {width}: {rect:?}"
                );
                // The picture and footer are both part of the same clickable choice.
                for pos in [
                    rect.center_top() + Vec2::new(0., 18.),
                    rect.center_bottom() - Vec2::new(0., 15.),
                ] {
                    let mut selected = None;
                    for pressed in [true, false] {
                        selected = frame(
                            &ctx,
                            width,
                            "moon",
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
                        .or(selected);
                    }
                    assert_eq!(
                        selected.as_deref(),
                        Some(id),
                        "{id} is not selectable at {pos:?} in width {width}"
                    );
                }
            }
        }
    }
}
