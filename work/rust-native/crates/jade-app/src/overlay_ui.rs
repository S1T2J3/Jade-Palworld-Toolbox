//! Game overlay preferences. Runtime window handling stays in `overlay`.
use crate::{
    design,
    overlay::{Config, Hotkey, Snapshot},
};
use eframe::egui::{self, RichText};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    None,
    Hide,
    ReturnToDesktop,
}

/// Old profiles need no migration or write until a user changes a preference.
pub fn read_preferences(state: &Value) -> Config {
    let value = &state["gameOverlay"];
    Config {
        enabled: value["enabled"].as_bool().unwrap_or(false),
        hotkey: value["hotkey"]
            .as_str()
            .and_then(Hotkey::from_key)
            .unwrap_or_default(),
        auto_hide: value["autoHide"].as_bool().unwrap_or(true),
    }
}

/// Keep world data and any unrelated overlay preferences unchanged.
pub fn write_preferences(state: &mut Value, config: Config) {
    let Some(root) = state.as_object_mut() else {
        return;
    };
    let value = root.entry("gameOverlay").or_insert_with(|| json!({}));
    if !value.is_object() {
        *value = json!({});
    }
    let value = value.as_object_mut().unwrap();
    value.insert("enabled".into(), json!(config.enabled));
    value.insert("hotkey".into(), json!(config.hotkey.key()));
    value.insert("autoHide".into(), json!(config.auto_hide));
}

pub fn show(ui: &mut egui::Ui, config: &mut Config, snapshot: &Snapshot) -> Action {
    let mut action = Action::None;
    let palette = design::palette(ui.ctx());
    design::card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        design::section(ui, "游戏悬浮窗");
        let response = design::checkbox(ui, &mut config.enabled, "启用游戏悬浮窗");
        test_rect(ui, "enabled", &response);
        ui.label("开启工具箱后，切回幻兽帕鲁，按快捷键即可呼出完整工具箱。");
        ui.add_space(7.);
        ui.horizontal_wrapped(|ui| {
            ui.label("呼出 / 收起快捷键");
            let combo = egui::ComboBox::from_id_salt("game-overlay-hotkey")
                .width(190.)
                .selected_text(config.hotkey.label())
                .show_ui(ui, |ui| {
                    for hotkey in [Hotkey::AltJ, Hotkey::CtrlShiftJ, Hotkey::F8, Hotkey::F9] {
                        let response =
                            ui.selectable_value(&mut config.hotkey, hotkey, hotkey.label());
                        test_rect(ui, hotkey.key(), &response);
                    }
                });
            test_rect(ui, "hotkey", &combo.response);
        });
        ui.add_space(7.);
        let response = design::checkbox(ui, &mut config.auto_hide, "切到其他应用时自动收起");
        test_rect(ui, "auto-hide", &response);
        ui.small("切换到其他程序后收起悬浮窗，回到游戏可再次呼出。");
    });
    ui.add_space(12.);
    design::card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        design::section(ui, "当前状态");
        let text = if !config.enabled {
            "已关闭"
        } else if !snapshot.status.trim().is_empty() {
            &snapshot.status
        } else if snapshot.registered {
            "已开启，等待在游戏中呼出"
        } else {
            "正在准备快捷键"
        };
        ui.label(
            RichText::new(text).color(if config.enabled && snapshot.registered {
                palette.accent
            } else {
                palette.ink
            }),
        );
        if snapshot.session {
            ui.add_space(8.);
            ui.horizontal_wrapped(|ui| {
                let response = ui
                    .add_enabled_ui(snapshot.visible, |ui| {
                        design::action(ui, "minimize", "收起悬浮窗")
                    })
                    .inner;
                test_rect(ui, "hide", &response);
                if response.clicked() {
                    action = Action::Hide;
                }
                let response = design::action(ui, "external", "回到普通窗口");
                test_rect(ui, "desktop", &response);
                if response.clicked() {
                    action = Action::ReturnToDesktop;
                }
            });
        }
    });
    ui.add_space(12.);
    design::card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        design::section(ui, "使用方法");
        ui.label("1. 开启悬浮窗后，让工具箱继续运行，再切回幻兽帕鲁。");
        ui.label(format!(
            "2. 按 {} 呼出；再次按下可收起并回到游戏。",
            config.hotkey.label()
        ));
        ui.label("3. 地图、配种、图鉴和仓库继续使用同一份工具箱记录。");
        ui.add_space(6.);
        ui.small("建议游戏使用“无边框”或“窗口”模式。");
        ui.small("悬浮窗中的关闭按钮用于收起；“回到普通窗口”可恢复桌面窗口。");
    });
    action
}

fn test_rect(ui: &egui::Ui, name: &str, response: &egui::Response) {
    #[cfg(test)]
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new(("overlay-setting", name)), response.rect));
    #[cfg(not(test))]
    let _ = (ui, name, response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect, Vec2};

    #[test]
    fn old_profiles_and_invalid_preferences_use_safe_defaults_without_mutation() {
        for state in [
            Value::Null,
            json!({"worlds":[{"id":"example"}]}),
            json!({"gameOverlay":null}),
            json!({"gameOverlay":"invalid"}),
            json!({"gameOverlay":{"enabled":"yes","hotkey":42,"autoHide":"no"}}),
        ] {
            let before = state.clone();
            assert_eq!(read_preferences(&state), Config::default());
            assert_eq!(state, before);
        }
        assert_eq!(
            read_preferences(&json!({"gameOverlay":{
                "enabled":true,"hotkey":"unknown-key","autoHide":false
            }})),
            Config {
                enabled: true,
                hotkey: Hotkey::AltJ,
                auto_hide: false
            }
        );
    }

    #[test]
    fn saving_overlay_preferences_preserves_worlds_and_unrelated_settings() {
        let mut state = json!({
            "worlds":[{"id":"one","inventory":{"records":[{"id":"pal-one"}]},"mapNotes":{"completed":["point-one"]}}],
            "theme":"moon", "autoDownload":true,
            "gameOverlay":{"enabled":false,"unrelated":"keep"}
        });
        let before = state.clone();
        let expected = Config {
            enabled: true,
            hotkey: Hotkey::F9,
            auto_hide: false,
        };
        write_preferences(&mut state, expected);
        assert_eq!(read_preferences(&state), expected);
        assert_eq!(state["worlds"], before["worlds"]);
        assert_eq!(state["theme"], before["theme"]);
        assert_eq!(state["autoDownload"], before["autoDownload"]);
        assert_eq!(state["gameOverlay"]["unrelated"], "keep");
        assert_eq!(state["gameOverlay"]["hotkey"], "f9");
        state["gameOverlay"] = json!("invalid");
        write_preferences(&mut state, Config::default());
        assert_eq!(read_preferences(&state), Config::default());
        assert_eq!(state["worlds"], before["worlds"]);
    }

    fn frame(
        ctx: &egui::Context,
        config: &mut Config,
        snapshot: &Snapshot,
        events: Vec<egui::Event>,
    ) -> Action {
        let mut action = Action::None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(812., 900.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| action = show(ui, config, snapshot));
            },
        );
        action
    }

    fn target(ctx: &egui::Context, name: &str) -> Rect {
        ctx.data(|data| data.get_temp::<Rect>(egui::Id::new(("overlay-setting", name))))
            .unwrap_or_else(|| panic!("Missing overlay preference target: {name}"))
    }

    fn click(ctx: &egui::Context, config: &mut Config, snapshot: &Snapshot, name: &str) -> Action {
        let pos = target(ctx, name).center();
        let mut action = Action::None;
        for pressed in [true, false] {
            let next = frame(
                ctx,
                config,
                snapshot,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
            if next != Action::None {
                action = next;
            }
        }
        action
    }

    #[test]
    fn overlay_settings_controls_and_hotkey_choices_respond_to_real_pointer_events() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        design::apply(&ctx, "moon");
        let mut config = Config::default();
        let mut snapshot = Snapshot {
            registered: false,
            session: false,
            visible: false,
            status: String::new(),
        };
        frame(&ctx, &mut config, &snapshot, vec![]);
        frame(&ctx, &mut config, &snapshot, vec![]);
        assert_eq!(click(&ctx, &mut config, &snapshot, "enabled"), Action::None);
        assert!(config.enabled);
        click(&ctx, &mut config, &snapshot, "auto-hide");
        assert!(!config.auto_hide);
        click(&ctx, &mut config, &snapshot, "hotkey");
        frame(&ctx, &mut config, &snapshot, vec![]);
        click(&ctx, &mut config, &snapshot, "f9");
        assert_eq!(config.hotkey, Hotkey::F9);
        snapshot = Snapshot {
            registered: true,
            session: true,
            visible: true,
            status: "已在游戏中显示".into(),
        };
        frame(&ctx, &mut config, &snapshot, vec![]);
        frame(&ctx, &mut config, &snapshot, vec![]);
        assert_eq!(click(&ctx, &mut config, &snapshot, "hide"), Action::Hide);
        assert_eq!(
            click(&ctx, &mut config, &snapshot, "desktop"),
            Action::ReturnToDesktop
        );
        for name in ["enabled", "hotkey", "auto-hide", "hide", "desktop"] {
            let rect = target(&ctx, name);
            assert!(
                rect.left() >= 0. && rect.right() <= 812.,
                "{name} leaves the settings column: {rect:?}"
            );
        }
    }
}
