use super::{AssistantView, KernelClient, ModelComment, Turn, finished};
use eframe::egui;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;

const SOURCE: &str = "DeepSeek R1 0528 蒸馏版 8B · 硅基流动";

struct Request {
    turn: Option<usize>,
    receiver: Receiver<Result<String, String>>,
}

#[derive(Default)]
pub(super) struct CloudMode {
    pub(super) enabled: bool,
    key: String,
    queued: VecDeque<usize>,
    running: Option<Request>,
}

impl CloudMode {
    pub(super) fn reset_conversation(&mut self) {
        self.queued.clear();
        self.running = None;
    }

    pub(super) fn cancel_turn(&mut self, index: usize) {
        self.queued.retain(|queued| *queued != index);
    }

    pub(super) fn discard(&mut self) {
        self.queued.clear();
        if let Some(request) = &mut self.running {
            request.turn = None;
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.discard();
        self.enabled = enabled;
        if !enabled {
            self.key.clear();
        }
    }

    pub(super) fn enqueue(&mut self, index: usize) {
        if self.enabled
            && !self.key.trim().is_empty()
            && !self.queued.contains(&index)
            && !self
                .running
                .as_ref()
                .is_some_and(|req| req.turn == Some(index))
        {
            self.queued.push_back(index);
        }
    }

    pub(super) fn poll(&mut self, turns: &mut [Turn]) {
        if let Some(result) = self
            .running
            .as_ref()
            .and_then(|request| finished(&request.receiver))
            && let Some(request) = self.running.take()
            && let Some(turn) = request.turn.and_then(|index| turns.get_mut(index))
        {
            turn.cloud_reply = Some(ModelComment {
                model: SOURCE.into(),
                result,
            });
        }
    }

    fn credentials(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("硅基流动密钥");
            let entry = egui::TextEdit::singleline(&mut self.key)
                .id_salt("pal-siliconflow-session-key")
                .password(true)
                .char_limit(512)
                .desired_width(188.)
                .hint_text("仅本次启用期间使用");
            if ui.add(entry).changed() {
                self.discard();
            }
            ui.hyperlink_to("获取密钥 ↗", "https://cloud.siliconflow.cn/account/ak");
        });
        ui.weak("需要硅基流动中国站实名账户。发送本题时会附上最近两轮对话和相关工具箱资料，可能包含仓库信息。");
        if self.key.trim().is_empty() {
            ui.weak("当前仅显示离线回答。填好密钥后可点「用云端回答」。");
        }
    }

    pub(super) fn turn_controls(&mut self, ui: &mut egui::Ui, index: usize, turn: &Turn) {
        if self
            .running
            .as_ref()
            .is_some_and(|request| request.turn == Some(index))
        {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("正在等待云端 DeepSeek 回答…");
            });
        } else if self.queued.contains(&index) {
            ui.weak("正在等待前一条云端请求结束…");
        } else if turn
            .cloud_reply
            .as_ref()
            .is_none_or(|reply| reply.result.is_err())
        {
            let label = if turn.cloud_reply.is_some() {
                "重试云端回答"
            } else {
                "用云端回答"
            };
            if ui
                .add_enabled(!self.key.trim().is_empty(), egui::Button::new(label))
                .clicked()
            {
                self.enqueue(index);
            }
        }
    }
}

impl AssistantView {
    pub(super) fn mode_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .radio(
                    !self.cloud.enabled && !self.local.enabled,
                    "增强问答 · 离线可用",
                )
                .clicked()
            {
                self.cloud.set_enabled(false);
                self.local.set_enabled(ui.ctx(), &self.kernel, false);
            }
            if ui.radio(self.cloud.enabled, "免费云端 DeepSeek").clicked() && !self.cloud.enabled
            {
                self.local.set_enabled(ui.ctx(), &self.kernel, false);
                self.cloud.set_enabled(true);
            }
            if ui.radio(self.local.enabled, "本机 Ollama").clicked() && !self.local.enabled {
                self.cloud.set_enabled(false);
                self.local.set_enabled(ui.ctx(), &self.kernel, true);
            }
        });
        if self.cloud.enabled {
            self.cloud.credentials(ui);
        } else if self.local.enabled {
            self.local
                .model_picker(ui, self.turns.len().checked_sub(1), &self.kernel);
        }
    }

    pub(super) fn run_cloud(&mut self, ctx: &egui::Context) {
        self.run_cloud_using(ctx, |kernel, ctx, key, prompt| {
            kernel.request(ctx, json!({"op":"cloud", "key":key, "prompt":prompt}))
        });
    }

    fn run_cloud_using(
        &mut self,
        ctx: &egui::Context,
        complete: impl FnOnce(
            &KernelClient,
            &egui::Context,
            String,
            String,
        ) -> Receiver<Result<String, String>>,
    ) {
        if !self.cloud.enabled || self.cloud.running.is_some() || self.cloud.key.trim().is_empty() {
            return;
        }
        let Some(index) = self.cloud.queued.pop_front() else {
            return;
        };
        let key = self.cloud.key.trim().to_owned();
        let prompt = self.external_prompt(index);
        self.cloud.running = Some(Request {
            turn: Some(index),
            receiver: complete(&self.kernel, ctx, key, prompt),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_client::AssistantReply;
    use std::time::{Duration, Instant};

    fn turn(question: &str) -> Turn {
        Turn {
            direct_result: false,
            question: question.into(),
            answer: AssistantReply {
                text: "工具箱资料。".into(),
                links: vec![("图鉴".into(), "#pals".into())],
                suggestions: vec![],
            },
            context: None,
            analysis: None,
            cloud_reply: None,
            web_reply: None,
        }
    }

    fn wait_for_answer(view: &mut AssistantView) {
        let start = Instant::now();
        while view.cloud.running.is_some() && start.elapsed() < Duration::from_secs(5) {
            view.cloud.poll(&mut view.turns);
            std::thread::yield_now();
        }
        assert!(view.cloud.running.is_none());
    }

    #[test]
    fn cloud_is_opt_in_and_retries_a_failed_turn_without_duplicating_it() {
        let ctx = egui::Context::default();
        let mut view = AssistantView::default();
        view.turns.push(turn("我该怎么配种？"));
        view.cloud.enqueue(0);
        view.cloud.set_enabled(true);
        view.cloud.enqueue(0);
        assert!(view.cloud.queued.is_empty());
        view.cloud.key = "fake-test-key".into();
        assert!(view.cloud.queued.is_empty());
        view.cloud.enqueue(0);
        view.cloud.enqueue(0);
        view.run_cloud_using(&ctx, |_, _, _, _| {
            super::super::tests::ready(Err("服务限流".into()))
        });
        view.cloud.enqueue(0);
        assert!(view.cloud.queued.is_empty());
        wait_for_answer(&mut view);
        assert!(view.turns[0].generated_answer().is_none());
        assert!(view.turns[0].history_text().contains("工具箱资料"));
        view.cloud.enqueue(0);
        view.run_cloud_using(&ctx, |_, _, key, prompt| {
            assert_eq!(key, "fake-test-key");
            assert!(prompt.contains("我该怎么配种？") && !prompt.contains("fake-test-key"));
            super::super::tests::ready(Ok("先把两只亲本放进牧场。".into()))
        });
        view.turns.push(turn("那缺蛋糕呢？"));
        view.cloud.enqueue(1);
        wait_for_answer(&mut view);
        assert_eq!(
            view.turns[0].generated_answer(),
            Some((SOURCE, "先把两只亲本放进牧场。"))
        );
        view.run_cloud_using(&ctx, |_, _, _, prompt| {
            assert!(prompt.contains("先把两只亲本放进牧场") && prompt.contains("那缺蛋糕呢"));
            super::super::tests::ready(Ok("先查配方。".into()))
        });
        wait_for_answer(&mut view);
        assert_eq!(view.turns.len(), 2);
        assert!(view.turns[1].history_text().contains("先查配方"));
        view.cloud.set_enabled(false);
        assert!(view.cloud.key.is_empty());
        assert!(view.turns[0].history_text().contains("先把两只亲本"));
    }

    #[test]
    fn switching_world_discards_both_pending_and_queued_cloud_turns() {
        let mut view = AssistantView::default();
        view.select_world("old");
        view.cloud.set_enabled(true);
        view.cloud.key = "fake-test-key".into();
        view.turns.push(turn("旧世界的问题"));
        view.turns.push(turn("旧世界的追问"));
        let (send, receiver) = std::sync::mpsc::channel();
        view.cloud.running = Some(Request {
            turn: Some(0),
            receiver,
        });
        view.cloud.enqueue(1);
        view.select_world("new");
        view.turns.push(turn("新世界的问题"));
        assert!(send.send(Ok("旧世界的结果".into())).is_err());
        wait_for_answer(&mut view);
        assert!(view.cloud.queued.is_empty());
        assert!(view.turns[0].cloud_reply.is_none());
        assert!(view.cloud.enabled);
        assert_eq!(view.cloud.key, "fake-test-key");
    }
}
