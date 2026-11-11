//! Shared native surfaces, using the original handbook's palette and line icons.
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub soft: Color32,
    pub accent: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub selected: Color32,
    pub rail: Color32,
    pub gold: Color32,
}
fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
pub fn colors(name: &str) -> Palette {
    let (bg, surface, soft, accent, ink, muted, line, selected, rail) = match name {
        "jade" => (
            0xf2f5ef, 0xfffefa, 0xf0f4eb, 0x27634d, 0x233d32, 0x58705f, 0xcbd8c8, 0xdeecdf,
            0x22483c,
        ),
        "blue" => (
            0xf0f5f9, 0xfdfeff, 0xeaf1f7, 0x286389, 0x243844, 0x5b7080, 0xc7d7e3, 0xdbeaf7,
            0x244660,
        ),
        "rose" => (
            0xf9f2f5, 0xfffcfd, 0xf7edf1, 0x94445e, 0x452e38, 0x806370, 0xe1c8d2, 0xf5e0e8,
            0x613746,
        ),
        "amber" => (
            0xf8f4ed, 0xfffdfa, 0xf7f0e4, 0x80591d, 0x42392b, 0x7c6e53, 0xded1b8, 0xf4e6c9,
            0x57412b,
        ),
        "slate" => (
            0xf1f3f5, 0xfdfefe, 0xecf0f4, 0x465467, 0x2c3540, 0x647181, 0xcbd4df, 0xe1e7ed,
            0x343c48,
        ),
        _ => (
            0xb4c0d6, 0xd1d9e8, 0xc9d1e2, 0x524182, 0x29283f, 0x42485f, 0x9ca6bd, 0xb9b9dc,
            0x9dd5ec,
        ),
    };
    Palette {
        bg: rgb(bg),
        surface: rgb(surface),
        soft: rgb(soft),
        accent: rgb(accent),
        ink: rgb(ink),
        muted: rgb(muted),
        line: rgb(line),
        selected: rgb(selected),
        rail: rgb(rail),
        gold: rgb(0xefdc8e),
    }
}
pub fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(egui::Id::new("jade-palette")))
        .unwrap_or(colors("moon"))
}
pub fn apply(ctx: &egui::Context, name: &str) {
    let p = colors(name);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("jade-palette"), p));
    let mut v = egui::Visuals::light();
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.soft;
    v.hyperlink_color = p.accent;
    v.override_text_color = Some(p.ink);
    v.weak_text_color = Some(p.muted);
    v.selection.bg_fill = p.selected;
    v.selection.stroke = Stroke::new(1., p.accent);
    v.window_stroke = Stroke::new(1., p.line);
    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
        widget.bg_stroke = Stroke::new(1., p.line);
        widget.fg_stroke = Stroke::new(1., p.ink);
        widget.bg_fill = p.soft;
        widget.weak_bg_fill = p.soft;
        widget.expansion = 0.;
    }
    v.widgets.hovered.bg_fill = p.selected;
    v.widgets.hovered.weak_bg_fill = p.selected;
    v.widgets.hovered.bg_stroke = Stroke::new(1., p.accent);
    v.widgets.active.bg_fill = p.selected;
    v.widgets.active.weak_bg_fill = p.selected;
    v.widgets.active.bg_stroke = Stroke::new(1., p.accent);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8., 7.);
        s.spacing.button_padding = Vec2::new(10., 6.);
        s.spacing.interact_size = Vec2::new(34., 30.);
        s.spacing.combo_height = 380.;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(28.));
    });
}
pub fn card(ui: &egui::Ui) -> egui::Frame {
    let p = palette(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1., p.line))
        .corner_radius(10)
        .inner_margin(12)
}
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(5.);
    ui.label(
        RichText::new(title)
            .size(17.)
            .strong()
            .color(palette(ui.ctx()).ink),
    );
    ui.add_space(2.);
}
pub fn badge(ui: &mut egui::Ui, label: impl Into<String>) -> egui::Response {
    let p = palette(ui.ctx());
    egui::Frame::new()
        .fill(p.soft)
        .stroke(Stroke::new(0.6, p.line))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(5, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(11.).color(p.accent))
        })
        .inner
}
pub fn stat(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    let p = palette(ui.ctx());
    egui::Frame::new()
        .fill(p.soft)
        .corner_radius(6)
        .inner_margin(9)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.small(label);
                ui.label(RichText::new(value).size(22.).strong().color(p.accent));
            });
        });
}
pub fn gradient(painter: &egui::Painter, rect: egui::Rect, colors: &[Color32], horizontal: bool) {
    if colors.len() < 2 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for i in 0..colors.len() - 1 {
        let t = i as f32 / (colors.len() - 1) as f32;
        let u = (i + 1) as f32 / (colors.len() - 1) as f32;
        let r = if horizontal {
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + rect.width() * t, rect.top()),
                egui::pos2(rect.left() + rect.width() * u, rect.bottom()),
            )
        } else {
            egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + rect.height() * t),
                egui::pos2(rect.right(), rect.top() + rect.height() * u),
            )
        };
        let n = mesh.vertices.len() as u32;
        for (pos, color) in [
            (r.left_top(), colors[i]),
            (
                r.right_top(),
                if horizontal { colors[i + 1] } else { colors[i] },
            ),
            (r.right_bottom(), colors[i + 1]),
            (
                r.left_bottom(),
                if horizontal { colors[i] } else { colors[i + 1] },
            ),
        ] {
            mesh.colored_vertex(pos, color);
        }
        mesh.add_triangle(n, n + 1, n + 2);
        mesh.add_triangle(n, n + 2, n + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
}
pub fn icon_source(name: &str) -> egui::ImageSource<'static> {
    match name {
        "egg" => egui::include_image!("../assets/egg.svg"),
        "worldmap" => egui::include_image!("../assets/worldmap.svg"),
        "paldex" => egui::include_image!("../assets/paldex.svg"),
        "enemy" => egui::include_image!("../assets/enemy.svg"),
        "book" => egui::include_image!("../assets/book.svg"),
        "ranch" => egui::include_image!("../assets/ranch.svg"),
        "items" => egui::include_image!("../assets/items.svg"),
        "compass" => egui::include_image!("../assets/compass.svg"),
        "settings" => egui::include_image!("../assets/settings.svg"),
        "search" => egui::include_image!("../assets/search.svg"),
        "warehouse" => egui::include_image!("../assets/warehouse.svg"),
        "branch" => egui::include_image!("../assets/branch.svg"),
        "target" => egui::include_image!("../assets/target.svg"),
        "bell" => egui::include_image!("../assets/bell.svg"),
        "pin" => egui::include_image!("../assets/pin.svg"),
        "parents" => egui::include_image!("../assets/parents.svg"),
        "list" => egui::include_image!("../assets/list.svg"),
        _ => egui::include_image!("../assets/info.svg"),
    }
}
pub fn icon(ui: &mut egui::Ui, name: &str, size: f32) -> egui::Response {
    ui.add(
        egui::Image::new(icon_source(name))
            .fit_to_exact_size(Vec2::splat(size))
            .tint(ui.visuals().text_color()),
    )
}
pub fn nav_button(
    ui: &mut egui::Ui,
    name: &str,
    icon: &str,
    selected: bool,
    height: f32,
    moon: bool,
) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::click(),
    );
    let fg = if moon {
        if selected {
            Color32::from_rgb(247, 243, 255)
        } else {
            p.ink
        }
    } else {
        if selected {
            p.accent
        } else {
            Color32::from_rgb(237, 241, 232)
        }
    };
    if selected || response.hovered() {
        let bg = if selected {
            if moon { rgb(0x595583) } else { p.selected }
        } else {
            Color32::from_white_alpha(28)
        };
        ui.painter().rect_filled(rect, 8, bg);
        if selected {
            ui.painter().rect_stroke(
                rect,
                8,
                Stroke::new(1., if moon { rgb(0x6f6a92) } else { p.selected }),
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    rect.min + Vec2::new(1., 7.),
                    Vec2::new(2., height - 14.),
                ),
                2,
                p.gold,
            );
        }
    }
    if height >= 46. {
        let ir = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 17.),
            Vec2::splat(22.),
        );
        egui::Image::new(icon_source(icon))
            .fit_to_exact_size(ir.size())
            .tint(fg)
            .paint_at(ui, ir);
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 13.),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional(13.),
            fg,
        );
    } else {
        let ir = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 15., rect.center().y),
            Vec2::splat(17.),
        );
        egui::Image::new(icon_source(icon))
            .fit_to_exact_size(ir.size())
            .tint(fg)
            .paint_at(ui, ir);
        ui.painter().text(
            egui::pos2(rect.left() + 28., rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(12.),
            fg,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn tab(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let response = ui.add(
        egui::Button::new(RichText::new(label).strong().color(if active {
            p.accent
        } else {
            p.muted
        }))
        .frame(false)
        .min_size(Vec2::new(0., 44.)),
    );
    if active {
        ui.painter().line_segment(
            [response.rect.left_bottom(), response.rect.right_bottom()],
            Stroke::new(3., p.accent),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rail_hit_targets_remain_separate_after_painting_icons() {
        let ctx = egui::Context::default();
        apply(&ctx, "moon");
        for height in [36., 54.] {
            let mut rectangles = Vec::new();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(108., 900.),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for label in [
                        "配种手册",
                        "世界地图",
                        "帕鲁图鉴",
                        "单位图鉴",
                        "帕鲁详情",
                        "帕鲁工作",
                        "物品图鉴",
                        "冒险指南",
                        "设置",
                    ] {
                        rectangles.push(nav_button(ui, label, "egg", false, height, true).rect);
                    }
                });
            });
            assert!(
                rectangles.windows(2).all(|r| r[1].top() >= r[0].bottom()),
                "Navigation entries overlap at height {height}"
            );
            assert!(
                rectangles
                    .iter()
                    .all(|r| (r.height() - height).abs() < 0.01)
            );
        }
    }
}
