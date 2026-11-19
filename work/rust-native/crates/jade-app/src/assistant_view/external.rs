use super::AssistantView;
use eframe::egui;
use serde_json::json;

#[derive(Clone, Copy, Default, PartialEq)]
enum Website {
    #[default]
    DeepSeek,
    ChatGpt,
}

impl Website {
    fn name(self) -> &'static str {
        match self {
            Self::DeepSeek => "DeepSeek",
            Self::ChatGpt => "ChatGPT",
        }
    }

    fn url(self) -> &'static str {
        match self {
            Self::DeepSeek => "https://chat.deepseek.com/",
            Self::ChatGpt => "https://chatgpt.com/",
        }
    }
}

#[derive(Default)]
pub(super) struct ExternalWeb {
    turn: Option<usize>,
    site: Website,
    prompt: String,
    reply: String,
    status: String,
}

fn excerpt(value: &str, max: usize) -> String {
    let mut chars = value.trim().chars();
    let mut result: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

impl AssistantView {
    pub fn has_dialog(&self) -> bool {
        self.web.turn.is_some()
    }

    pub fn dismiss_dialog(&mut self) -> bool {
        if !self.has_dialog() {
            return false;
        }
        self.web = ExternalWeb {
            site: self.web.site,
            ..Default::default()
        };
        true
    }

    pub(super) fn external_prompt(&self, index: usize) -> String {
        let Some(turn) = self.turns.get(index) else {
            return String::new();
        };
        let history: Vec<_> = self.turns[index.saturating_sub(2)..index]
            .iter()
            .map(|prior| {
                json!({
                    "玩家": excerpt(&prior.question, 512),
                    "当时的回答": excerpt(&prior.history_text(), 650),
                })
            })
            .collect();
        let request = json!({
            "最近对话": history,
            "本轮问题": excerpt(&turn.question, 512),
            "工具箱本轮查询结果": excerpt(&turn.answer.text, 2400),
        });
        format!(
            "请作为《幻兽帕鲁》攻略助手，用中文回答本轮问题，结合最近对话理解追问。优先使用工具箱查到的资料，区分已知事实和建议。资料不足时说明缺什么，涉及数值请勿猜测。只处理与本游戏有关的问题。\n\n{request:#}"
        )
    }

    pub(super) fn open_web_dialog(&mut self, index: usize) {
        let Some(turn) = self.turns.get(index) else {
            return;
        };
        let (site, reply) =
            turn.web_reply
                .as_ref()
                .map_or((self.web.site, String::new()), |(source, reply)| {
                    (
                        if source == Website::ChatGpt.name() {
                            Website::ChatGpt
                        } else {
                            Website::DeepSeek
                        },
                        reply.clone(),
                    )
                });
        self.web = ExternalWeb {
            turn: Some(index),
            site,
            prompt: self.external_prompt(index),
            reply,
            status: String::new(),
        };
    }

    fn import_web_reply(&mut self) {
        let Some(index) = self.web.turn else { return };
        let reply = self.web.reply.trim();
        if !reply.is_empty()
            && let Some(turn) = self.turns.get_mut(index)
        {
            turn.web_reply = Some((self.web.site.name().into(), reply.to_owned()));
            self.web = ExternalWeb {
                site: self.web.site,
                ..Default::default()
            };
        }
    }

    pub(super) fn web_dialog(&mut self, ctx: &egui::Context) {
        if self.web.turn.is_none() {
            return;
        }
        let mut open = true;
        let mut import = false;
        egui::Window::new("用免费网页版问")
            .id(egui::Id::new("assistant_web_exchange"))
            .open(&mut open)
            .collapsible(false)
            .default_width(480.)
            .max_height((ctx.content_rect().height() - 64.).max(150.))
            .vscroll(true)
            .show(ctx, |ui| {
                let web = &mut self.web;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut web.site, Website::DeepSeek, "DeepSeek");
                    ui.selectable_value(&mut web.site, Website::ChatGpt, "ChatGPT");
                });
                ui.label("复制后在官网粘贴并发送。你可以先修改要带过去的对话和资料。");
                ui.collapsing("查看或编辑要复制的内容", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut web.prompt)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(
                        !web.prompt.trim().is_empty(),
                        egui::Button::new(format!("复制提问并打开 {}", web.site.name())),
                    )
                    .clicked()
                {
                    ctx.copy_text(web.prompt.clone());
                    web.status = match webbrowser::open(web.site.url()) {
                        Ok(_) => "已复制，请在网页中粘贴提问。".into(),
                        Err(_) => format!("已复制。请在浏览器打开 {}", web.site.url()),
                    };
                }
                if !web.status.is_empty() {
                    ui.weak(&web.status);
                }
                ui.separator();
                ui.label("复制网页里的回答，在这里粘贴");
                ui.add(
                    egui::TextEdit::multiline(&mut web.reply)
                        .id(egui::Id::new("assistant_web_reply"))
                        .char_limit(64_000)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
                import = ui
                    .add_enabled(
                        !web.reply.trim().is_empty(),
                        egui::Button::new("将回答显示在这条问题下"),
                    )
                    .clicked();
            });
        if import {
            self.import_web_reply();
        } else if !open {
            self.dismiss_dialog();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_client::AssistantReply;
    use crate::assistant_view::Turn;

    fn example(question: &str, answer: &str) -> Turn {
        Turn {
            direct_result: false,
            question: question.into(),
            answer: AssistantReply {
                text: answer.into(),
                links: vec![],
                suggestions: vec![],
            },
            context: None,
            analysis: None,
            cloud_reply: None,
            web_reply: None,
        }
    }

    #[test]
    fn handoff_and_import_keep_the_question_source_and_recent_context() {
        let mut view = AssistantView::default();
        view.turns.extend([
            example("以前的无关问题", "旧答案"),
            example("问材料", "做蛋糕"),
            example("牛奶呢", "资料里的牛奶"),
            example("要做两份呢", "需要的数量"),
            example("后来的问题", "未来的答案"),
        ]);
        view.open_web_dialog(3);
        assert!(view.web.prompt.contains("要做两份呢") && view.web.prompt.contains("资料里的牛奶"));
        assert!(
            !view.web.prompt.contains("以前的无关问题") && !view.web.prompt.contains("后来的问题")
        );
        view.web.site = Website::ChatGpt;
        view.web.reply = "   ".into();
        view.import_web_reply();
        assert!(view.turns[3].web_reply.is_none());
        view.web.reply = " 外部答案 ".into();
        view.import_web_reply();
        assert_eq!(
            view.turns[3].web_reply,
            Some(("ChatGPT".into(), "外部答案".into()))
        );
        assert!(view.turns[4].web_reply.is_none());
        assert!(view.external_prompt(4).contains("外部答案"));
        view.open_web_dialog(3);
        assert!(view.has_dialog());
        view.web.reply = "还没导入的草稿".into();
        assert!(view.dismiss_dialog());
        assert!(!view.has_dialog() && !view.dismiss_dialog());
        assert!(view.web.reply.is_empty() && view.web.prompt.is_empty());
        assert_eq!(view.turns.len(), 5);
        assert_eq!(view.turns[3].web_reply, Some(("ChatGPT".into(), "外部答案".into())));
        view.open_web_dialog(3);
        assert_eq!(view.web.reply, "外部答案");
        view.reset_session();
        assert!(view.web.turn.is_none());
        view.turns.push(example("另一次访问", "本次本地回答"));
        view.import_web_reply();
        assert!(view.turns[0].web_reply.is_none());
        assert!(view.web.prompt.is_empty() && view.web.reply.is_empty());
    }

    #[test]
    fn pasted_website_answer_is_imported_by_the_dialog_button() {
        let ctx = egui::Context::default();
        let mut view = AssistantView::default();
        view.turns.push(example("打龙属性用什么？", "参考图鉴"));
        view.open_web_dialog(0);
        let frame = |view: &mut AssistantView, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| view.web_dialog(ctx),
            )
        };
        frame(&mut view, vec![]);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("assistant_web_reply")));
        frame(
            &mut view,
            vec![egui::Event::Paste("网页中的攻略回答".into())],
        );
        assert_eq!(view.web.reply, "网页中的攻略回答");
        let output = frame(&mut view, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "将回答显示在这条问题下"
                {
                    Some(text.pos + text.galley.size() * 0.5)
                } else {
                    None
                }
            })
            .expect("import button is visible");
        let click = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&mut view, vec![egui::Event::PointerMoved(pos), click(true)]);
        frame(&mut view, vec![click(false)]);
        assert!(view.web.turn.is_none());
        assert_eq!(
            view.turns[0].web_reply,
            Some(("DeepSeek".into(), "网页中的攻略回答".into()))
        );
    }
}
