use crate::navigation::ScrollArea;
use eframe::egui::{self, Align, Rect, Vec2};

pub const TABS: &[(&str, &str)] = &[
    ("皮肤", "settings/skins"),
    ("帕鲁助手", "settings/assistant"),
    ("游戏悬浮窗", "settings/overlay"),
    ("公告与更新", "settings/updates"),
];

fn anchor(page: &str) -> Option<usize> {
    TABS.iter().position(|&(_, id)| id == page)
}

#[derive(Default)]
pub struct SettingsPage {
    active: usize,
    pending: Option<usize>,
}

impl SettingsPage {
    pub fn select(&mut self, page: &str, jump: bool) -> bool {
        let Some(index) = anchor(page) else {
            return false;
        };
        self.active = index;
        self.pending = jump.then_some(index);
        true
    }

    pub fn active(&self) -> &'static str {
        TABS[self.active].1
    }

    pub fn show(&mut self, ui: &mut egui::Ui, mut render: impl FnMut(&mut egui::Ui, &str)) {
        let pending = self.pending.take();
        let output = ScrollArea::vertical()
            .id_salt("settings-document")
            .animated(false)
            .show_viewport(ui, |ui, viewport| {
                let origin = ui.next_widget_position().y;
                let mut starts = [0.; TABS.len()];
                for (index, &(_, id)) in TABS.iter().enumerate() {
                    if index != 0 {
                        ui.add_space(20.);
                        ui.separator();
                        ui.add_space(12.);
                    }
                    let start = ui.next_widget_position();
                    starts[index] = start.y - origin;
                    if pending == Some(index) {
                        ui.scroll_to_rect(Rect::from_min_size(start, Vec2::ZERO), Some(Align::Min));
                    }
                    ui.push_id(id, |ui| render(ui, id));
                }
                // Keep the final heading reachable even when the updates section is short.
                let last_height = ui.next_widget_position().y - origin - starts[TABS.len() - 1];
                let extra = (viewport.height() - last_height).max(0.);
                if extra > 0. {
                    ui.allocate_space(Vec2::new(0., extra));
                }
                starts
            });
        // Use the new scroll offset, including this frame's input and anchor request.
        let reading_line = output.state.offset.y + 12.;
        let active = output
            .inner
            .iter()
            .rposition(|&start| start <= reading_line)
            .unwrap_or(0);
        if active != self.active {
            self.active = active;
            ui.ctx().request_repaint();
        }
    }
}
