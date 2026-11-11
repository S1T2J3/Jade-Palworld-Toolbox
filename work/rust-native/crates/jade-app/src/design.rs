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
pub const ICON_NAMES: &[&str] = &[
    "arrow",
    "bell",
    "book",
    "branch",
    "check",
    "close",
    "compass",
    "dark",
    "down",
    "download",
    "dragon",
    "edit",
    "egg",
    "electric",
    "enemy",
    "external",
    "fire",
    "first",
    "grass",
    "ground",
    "ice",
    "info",
    "items",
    "last",
    "layers",
    "left",
    "list",
    "maximize",
    "menu",
    "minimize",
    "neutral",
    "paldex",
    "parents",
    "pin",
    "plus",
    "ranch",
    "refresh",
    "relic",
    "reset",
    "restart",
    "restore",
    "right",
    "save",
    "search",
    "settings",
    "surgery",
    "target",
    "trash",
    "upload",
    "warehouse",
    "water",
    "worldmap",
];
pub fn icon_source(name: &str) -> egui::ImageSource<'static> {
    match name {
        "arrow" => egui::include_image!("../assets/arrow.svg"),
        "bell" => egui::include_image!("../assets/bell.svg"),
        "book" => egui::include_image!("../assets/book.svg"),
        "branch" => egui::include_image!("../assets/branch.svg"),
        "check" => egui::include_image!("../assets/check.svg"),
        "close" => egui::include_image!("../assets/close.svg"),
        "compass" => egui::include_image!("../assets/compass.svg"),
        "dark" => egui::include_image!("../assets/dark.svg"),
        "down" => egui::include_image!("../assets/down.svg"),
        "download" => egui::include_image!("../assets/download.svg"),
        "dragon" => egui::include_image!("../assets/dragon.svg"),
        "edit" => egui::include_image!("../assets/edit.svg"),
        "egg" => egui::include_image!("../assets/egg.svg"),
        "electric" => egui::include_image!("../assets/electric.svg"),
        "enemy" => egui::include_image!("../assets/enemy.svg"),
        "external" => egui::include_image!("../assets/external.svg"),
        "fire" => egui::include_image!("../assets/fire.svg"),
        "first" => egui::include_image!("../assets/first.svg"),
        "grass" => egui::include_image!("../assets/grass.svg"),
        "ground" => egui::include_image!("../assets/ground.svg"),
        "ice" => egui::include_image!("../assets/ice.svg"),
        "info" => egui::include_image!("../assets/info.svg"),
        "items" => egui::include_image!("../assets/items.svg"),
        "last" => egui::include_image!("../assets/last.svg"),
        "layers" => egui::include_image!("../assets/layers.svg"),
        "left" => egui::include_image!("../assets/left.svg"),
        "list" => egui::include_image!("../assets/list.svg"),
        "maximize" => egui::include_image!("../assets/maximize.svg"),
        "menu" => egui::include_image!("../assets/menu.svg"),
        "minimize" => egui::include_image!("../assets/minimize.svg"),
        "neutral" => egui::include_image!("../assets/neutral.svg"),
        "paldex" => egui::include_image!("../assets/paldex.svg"),
        "parents" => egui::include_image!("../assets/parents.svg"),
        "pin" => egui::include_image!("../assets/pin.svg"),
        "plus" => egui::include_image!("../assets/plus.svg"),
        "ranch" => egui::include_image!("../assets/ranch.svg"),
        "refresh" => egui::include_image!("../assets/refresh.svg"),
        "relic" => egui::include_image!("../assets/relic.svg"),
        "reset" => egui::include_image!("../assets/reset.svg"),
        "restart" => egui::include_image!("../assets/restart.svg"),
        "restore" => egui::include_image!("../assets/restore.svg"),
        "right" => egui::include_image!("../assets/right.svg"),
        "save" => egui::include_image!("../assets/save.svg"),
        "search" => egui::include_image!("../assets/search.svg"),
        "settings" => egui::include_image!("../assets/settings.svg"),
        "surgery" => egui::include_image!("../assets/surgery.svg"),
        "target" => egui::include_image!("../assets/target.svg"),
        "trash" => egui::include_image!("../assets/trash.svg"),
        "upload" => egui::include_image!("../assets/upload.svg"),
        "warehouse" => egui::include_image!("../assets/warehouse.svg"),
        "water" => egui::include_image!("../assets/water.svg"),
        "worldmap" => egui::include_image!("../assets/worldmap.svg"),
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
/// A real vector icon and a labelled native hit target, shared by page actions.
pub fn action(ui: &mut egui::Ui, name: &str, label: &str) -> egui::Response {
    ui.add(
        egui::Button::image_and_text(
            egui::Image::new(icon_source(name))
                .fit_to_exact_size(Vec2::splat(16.))
                .tint(ui.visuals().text_color()),
            label,
        )
        .min_size(Vec2::new(0., 32.))
        .wrap_mode(egui::TextWrapMode::Extend),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn icon_button(ui: &mut egui::Ui, name: &str, tooltip: &str) -> egui::Response {
    ui.add_sized(
        Vec2::splat(32.),
        egui::Button::image(
            egui::Image::new(icon_source(name))
                .fit_to_exact_size(Vec2::splat(16.))
                .tint(ui.visuals().text_color()),
        ),
    )
    .on_hover_text(tooltip)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn small_action(ui: &mut egui::Ui, name: &str, label: &str) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 20.;
        ui.spacing_mut().button_padding = Vec2::new(4., 2.);
        ui.add(
            egui::Button::image_and_text(
                egui::Image::new(icon_source(name))
                    .fit_to_exact_size(Vec2::splat(12.))
                    .tint(ui.visuals().text_color()),
                RichText::new(label).size(12.),
            )
            .min_size(Vec2::new(0., 20.))
            .wrap_mode(egui::TextWrapMode::Extend),
        )
    })
    .inner
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn small_icon_button(ui: &mut egui::Ui, name: &str, tooltip: &str) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size = Vec2::splat(20.);
        ui.spacing_mut().button_padding = Vec2::splat(2.);
        ui.add_sized(
            Vec2::splat(20.),
            egui::Button::image(
                egui::Image::new(icon_source(name))
                    .fit_to_exact_size(Vec2::splat(12.))
                    .tint(ui.visuals().text_color()),
            ),
        )
    })
    .inner
    .on_hover_text(tooltip)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn checkbox(
    ui: &mut egui::Ui,
    checked: &mut bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        for visual in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.noninteractive,
            &mut widgets.open,
        ] {
            visual.corner_radius = egui::CornerRadius::same(2);
        }
        ui.checkbox(checked, label)
    })
    .inner
}
/// Custom modal headings keep the close target visible even with long titles.
pub fn dialog_header(ui: &mut egui::Ui, name: &str, title: &str) -> egui::Response {
    ui.horizontal(|ui| {
        icon(ui, name, 21.);
        let width = (ui.available_width() - 40.).max(0.);
        ui.add_sized(
            [width, 32.],
            egui::Label::new(RichText::new(title).size(19.).strong())
                .halign(egui::Align::Min)
                .truncate(),
        );
        icon_button(ui, "close", "关闭")
    })
    .inner
}
pub fn tab_icon(page: &str) -> &str {
    match page {
        "lookup" => "branch",
        "warehouse" => "warehouse",
        "offspring" => "egg",
        "planner" => "target",
        "map" => "worldmap",
        "spawns" => "pin",
        "paldex" => "paldex",
        "enemies" => "enemy",
        "merchants" | "neutrals" => "list",
        "items" => "items",
        "details/active" => "electric",
        "details/passive" => "layers",
        "details/partner" => "paldex",
        "details/fruits" => "grass",
        "details/surgery" => "surgery",
        "details/stats" => "list",
        "details/capture" => "target",
        "work/suitability" => "ranch",
        "work/facilities" => "warehouse",
        "work/production" => "items",
        "work/efficiency" => "settings",
        "work/ranch" => "ranch",
        "work/relic" => "relic",
        "work/expeditions" => "compass",
        "guide/elements" => "fire",
        "guide/status" => "info",
        "guide/gathering" => "grass",
        "guide/loot" => "items",
        "guide/quests" => "list",
        "guide/journals" => "book",
        "settings/skins" => "settings",
        "settings/updates" => "bell",
        _ => "info",
    }
}
pub fn tab_with_icon(ui: &mut egui::Ui, label: &str, name: &str, active: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let color = if active { p.accent } else { p.muted };
    let compact = ui.ctx().content_rect().width() < 1120.;
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(if compact { 12. } else { 14. }),
        color,
    );
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(galley.size().x + if compact { 30. } else { 44. }, 44.),
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0., 3.)), 5, p.soft);
    }
    egui::Image::new(icon_source(name)).tint(color).paint_at(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(
                rect.left() + if compact { 10. } else { 15. },
                rect.center().y,
            ),
            Vec2::splat(if compact { 14. } else { 17. }),
        ),
    );
    ui.painter().galley(
        egui::pos2(
            rect.left() + if compact { 22. } else { 29. },
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        color,
    );
    if active {
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), rect.bottom() - 2.),
                egui::pos2(rect.right(), rect.bottom() - 2.),
            ],
            Stroke::new(3., p.accent),
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
/// Caption buttons use vector geometry rather than font-dependent Unicode glyphs.
pub fn window_button(ui: &mut egui::Ui, name: &str, tooltip: &str, ink: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(46., 35.), egui::Sense::click());
    let hovered = response.hovered();
    if hovered {
        let fill = if name == "close" {
            Color32::from_rgb(196, 43, 57)
        } else {
            Color32::from_white_alpha(50)
        };
        ui.painter().rect_filled(rect, 0, fill);
    }
    let color = if name == "close" && hovered {
        Color32::WHITE
    } else {
        ink
    };
    egui::Image::new(icon_source(name)).tint(color).paint_at(
        ui,
        egui::Rect::from_center_size(rect.center(), Vec2::splat(17.)),
    );
    response
        .on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn announcement_button(ui: &mut egui::Ui, unread: usize) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(if unread > 0 { 94. } else { 68. }, 33.),
        egui::Sense::click(),
    );
    ui.painter().rect_filled(
        rect,
        6,
        if response.hovered() {
            p.selected
        } else {
            p.soft
        },
    );
    ui.painter()
        .rect_stroke(rect, 6, Stroke::new(1., p.line), egui::StrokeKind::Inside);
    egui::Image::new(icon_source("bell"))
        .tint(p.accent)
        .paint_at(
            ui,
            egui::Rect::from_center_size(rect.left_center() + Vec2::new(15., 0.), Vec2::splat(16.)),
        );
    ui.painter().text(
        rect.left_center() + Vec2::new(29., 0.),
        egui::Align2::LEFT_CENTER,
        "公告",
        egui::FontId::proportional(13.),
        p.accent,
    );
    if unread > 0 {
        let center = rect.right_center() - Vec2::new(15., 0.);
        ui.painter().circle_filled(center, 9., p.accent);
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            if unread > 99 {
                "99+".into()
            } else {
                unread.to_string()
            },
            egui::FontId::proportional(10.),
            Color32::WHITE,
        );
    }
    response
        .on_hover_text("公告与更新")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
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
    fn every_registered_icon_decodes_as_an_actual_vector_image() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        for name in ICON_NAMES {
            let image = egui::Image::new(icon_source(name));
            let mut ready = false;
            for _ in 0..10 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    match image.load_for_size(ctx, Vec2::splat(24.)) {
                        Ok(egui::load::TexturePoll::Ready { texture }) => {
                            assert!(texture.size.x > 0. && texture.size.y > 0., "{name}");
                            ready = true;
                        }
                        Ok(_) => {}
                        Err(error) => panic!("{name}: {error}"),
                    }
                });
                if ready {
                    break;
                }
            }
            assert!(ready, "{name} never loaded");
        }
    }
    #[test]
    fn caption_and_dialog_close_icons_have_live_distinct_hit_targets() {
        let ctx = egui::Context::default();
        apply(&ctx, "moon");
        let mut rects = Vec::new();
        let mut clicked = Vec::new();
        let mut frame = |events: Vec<egui::Event>| {
            let mut current = Vec::new();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(640., 180.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            for name in ["minimize", "maximize", "close"] {
                                let r = window_button(ui, name, name, palette(ctx).ink);
                                current.push(r.rect);
                                if r.clicked() {
                                    clicked.push(name.to_string());
                                }
                            }
                        });
                        let close =
                            dialog_header(ui, "bell", "较长的公告标题也应保留清晰的关闭入口");
                        current.push(close.rect);
                        if close.clicked() {
                            clicked.push("dialog-close".into());
                        }
                    });
                },
            );
            current
        };
        frame(vec![]);
        rects.extend(frame(vec![]));
        assert!(rects[..3].windows(2).all(|r| r[0].right() <= r[1].left()));
        for rect in &rects {
            let pos = rect.center();
            for pressed in [true, false] {
                frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
        }
        assert_eq!(clicked, ["minimize", "maximize", "close", "dialog-close"]);
    }
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
