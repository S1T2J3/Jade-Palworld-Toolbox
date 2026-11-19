use crate::assistant_client::{AssistantReply, KernelClient, QueryPlan};
use crate::assistant_search::SearchBox;
use crate::data::{Data, text};
use crate::{design, store::Store};
#[cfg(test)]
use eframe::egui::Vec2;
use eframe::egui::{self, Align, RichText};
use serde::Deserialize;
use serde_json::json;
use std::collections::VecDeque;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, TryRecvError};

mod cloud_ui;
mod external;
use cloud_ui::CloudMode;

const STARTERS: &[(&str, &str)] = &[
    ("items", "帕鲁矿碎块怎么获得？"),
    ("target", "输出饰品推荐"),
    ("egg", "我仓库里哪两只能配出阿努比斯？"),
    ("fire", "据点金属锭不够，要换生火帕鲁吗？"),
    ("ranch", "蛋糕总是断产，先查哪里？"),
];

struct Turn {
    question: String,
    answer: AssistantReply,
    direct_result: bool,
    context: Option<String>,
    analysis: Option<ModelComment>,
    cloud_reply: Option<ModelComment>,
    web_reply: Option<(String, String)>,
}

impl Turn {
    fn generated_answer(&self) -> Option<(&str, &str)> {
        [self.cloud_reply.as_ref(), self.analysis.as_ref()]
            .into_iter()
            .flatten()
            .find_map(|reply| Some((reply.model.as_str(), reply.result.as_deref().ok()?)))
    }

    fn history_text(&self) -> String {
        let facts = (!self.answer.links.is_empty()).then_some(self.answer.text.as_str());
        let previous = self
            .web_reply
            .as_ref()
            .map(|(_, reply)| reply.as_str())
            .or_else(|| self.generated_answer().map(|(_, answer)| answer));
        if let Some(reply) = previous {
            // Keep the end of a long reply so a follow-up can answer its last question.
            // Leave room in the model's history window for the supporting facts.
            let chars: Vec<_> = reply.chars().collect();
            let excerpt = if chars.len() <= 180 {
                reply.to_owned()
            } else {
                let start: String = chars[..110].iter().collect();
                let end: String = chars[chars.len() - 69..].iter().collect();
                format!("{start}…{end}")
            };
            return match facts {
                Some(facts) => format!("助手上次回答：{excerpt}\n当时查到的图鉴资料：{facts}"),
                None => format!("助手上次回答：{excerpt}"),
            };
        }
        facts
            .or(self.context.as_deref())
            .unwrap_or(&self.answer.text)
            .to_owned()
    }

    fn merge_evidence(&mut self, extra: AssistantReply) {
        if extra.links.is_empty() {
            return;
        }
        if self.answer.links.is_empty() {
            self.answer.text = extra.text;
        } else {
            for line in extra
                .text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                if !self.answer.text.lines().any(|known| known.trim() == line) {
                    self.answer.text.push('\n');
                    self.answer.text.push_str(line);
                }
            }
        }
        for (label, route) in extra.links {
            if !self.answer.links.iter().any(|(_, known)| known == &route) {
                self.answer.links.push((label, route));
            }
        }
        for suggestion in extra.suggestions {
            if !self.answer.suggestions.contains(&suggestion) {
                self.answer.suggestions.push(suggestion);
            }
        }
    }
}

struct ModelComment {
    model: String,
    result: Result<String, String>,
}

enum ModelOutput {
    Answer(Receiver<Result<String, String>>),
    Plan(Receiver<Result<QueryPlan, String>>),
    Evidence(Receiver<Result<Option<AssistantReply>, String>>, String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Plan,
    Compose,
}

// Setting turn to None discards a late answer without starting another heavy
// request while the old request is still running.
struct PendingAnalysis {
    turn: Option<usize>,
    model: String,
    output: ModelOutput,
}

#[derive(Default)]
struct LocalMode {
    enabled: bool,
    models: Vec<String>,
    selected: String,
    listing: Option<Receiver<Result<Vec<String>, String>>>,
    resume_discovery: bool,
    list_error: Option<String>,
    running: Option<PendingAnalysis>,
    queued: VecDeque<(usize, Step)>,
}

fn finished<T>(receiver: &Receiver<Result<T, String>>) -> Option<Result<T, String>> {
    match receiver.try_recv() {
        Ok(value) => Some(value),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err("回答任务已中断，请重试。".into())),
    }
}

impl LocalMode {
    fn discard_analysis(&mut self) {
        self.queued.clear();
        if let Some(task) = &mut self.running {
            task.turn = None;
        }
    }

    fn model_changed(&mut self, last_turn: Option<usize>) {
        self.discard_analysis();
        if let Some(index) = last_turn {
            self.queued.push_back((index, Step::Plan));
        }
    }

    fn discover(&mut self, ctx: &egui::Context, kernel: &KernelClient) {
        self.list_error = None;
        self.listing = Some(kernel.request(ctx, json!({"op":"models"})));
    }

    fn set_enabled(&mut self, ctx: &egui::Context, kernel: &KernelClient, enabled: bool) {
        self.enabled = enabled;
        self.resume_discovery = false;
        self.discard_analysis();
        self.selected.clear();
        self.models.clear();
        self.list_error = None;
        self.listing = None;
        if enabled {
            self.discover(ctx, kernel);
        }
    }

    fn poll(&mut self, turns: &mut [Turn], ctx: &egui::Context, kernel: &KernelClient) {
        if let Some(result) = self.listing.as_ref().and_then(finished) {
            self.listing = None;
            match result {
                Ok(models) => {
                    if !models.contains(&self.selected) {
                        self.selected.clear();
                        self.discard_analysis();
                    }
                    self.models = models;
                }
                Err(message) => {
                    self.models.clear();
                    self.selected.clear();
                    self.discard_analysis();
                    self.list_error = Some(message);
                }
            }
        }
        let Some(task) = self.running.as_mut() else {
            return;
        };
        let result = match &task.output {
            ModelOutput::Answer(receiver) => finished(receiver),
            ModelOutput::Plan(receiver) => match finished(receiver) {
                None => None,
                Some(Err(error)) => Some(Err(error)),
                Some(Ok(QueryPlan::Evidence { answer, complete })) => {
                    if let Some(index) = task.turn
                        && let Some(turn) = turns.get_mut(index)
                    {
                        turn.answer = answer;
                        turn.direct_result = complete;
                        if !complete {
                            self.queued.push_front((index, Step::Compose));
                        }
                    }
                    self.running = None;
                    return;
                }
                Some(Ok(QueryPlan::Outside)) => {
                    Some(Ok("我可以帮你查询和分析幻兽帕鲁里的问题。".into()))
                }
                Some(Ok(QueryPlan::Search {
                    terms,
                    clarification,
                })) if task.turn.is_some() => {
                    let question = task.turn.and_then(|i| turns.get(i)).map(|turn| turn.question.as_str()).unwrap_or("");
                    let receiver = kernel.request(ctx, json!({"op":"retrieve", "terms":terms, "question":question}));
                    task.output = ModelOutput::Evidence(receiver, clarification);
                    return;
                }
                Some(Ok(QueryPlan::Search { clarification, .. })) => Some(Ok(clarification)),
                Some(Ok(QueryPlan::Clarify { question })) => Some(Ok(question)),
            },
            ModelOutput::Evidence(receiver, clarification) => match finished(receiver) {
                None => None,
                Some(Err(error)) => Some(Err(error)),
                Some(Ok(evidence)) => {
                    if let Some(index) = task.turn
                        && let Some(turn) = turns.get_mut(index)
                    {
                        if let Some(reply) = evidence {
                            turn.merge_evidence(reply);
                        }
                        if !turn.answer.links.is_empty() {
                            self.queued.push_front((index, Step::Compose));
                            self.running = None;
                            return;
                        }
                    }
                    Some(Ok(format!("本轮图鉴资料不足，暂时无法确定。{clarification}")))
                }
            },
        };
        if let Some(result) = result
            && let Some(task) = self.running.take()
            && let Some(index) = task.turn
            && let Some(turn) = turns.get_mut(index)
        {
            turn.analysis = Some(ModelComment {
                model: format!("本机 AI · {}", task.model),
                result,
            });
        }
    }

    fn model_picker(&mut self, ui: &mut egui::Ui, last_turn: Option<usize>, kernel: &KernelClient) {
        ui.horizontal_wrapped(|ui| {
            if self.listing.is_some() {
                ui.spinner();
                ui.weak("正在读取本机模型…");
            } else {
                if !self.models.is_empty() {
                    let prior = self.selected.clone();
                    egui::ComboBox::from_id_salt("assistant-local-model")
                        .selected_text(if self.selected.is_empty() {
                            "选择本机模型"
                        } else {
                            &self.selected
                        })
                        .show_ui(ui, |ui| {
                            for name in &self.models {
                                ui.selectable_value(&mut self.selected, name.clone(), name);
                            }
                        });
                    if self.selected != prior {
                        self.model_changed(last_turn);
                    }
                } else if let Some(error) = &self.list_error {
                    ui.weak(error);
                } else {
                    ui.weak("请先在 Ollama 中安装本地聊天模型，然后刷新。");
                }
                if design::small_action(ui, "refresh", "刷新模型").clicked() {
                    self.discover(ui.ctx(), kernel);
                }
            }
        });
    }

    fn run_queued(&mut self, ctx: &egui::Context, turns: &[Turn], kernel: &KernelClient) {
        if !self.enabled || self.selected.is_empty() || self.running.is_some() {
            return;
        }
        let Some((index, step)) = self.queued.pop_front() else {
            return;
        };
        let Some(turn) = turns.get(index) else { return };
        if turn.direct_result {
            return;
        }
        let question = turn.question.clone();
        let evidence = (step == Step::Compose).then(|| turn.answer.text.clone());
        let mut history: Vec<_> = turns[..index]
            .iter()
            .rev()
            .take(3)
            .map(|turn| (turn.question.clone(), turn.history_text()))
            .collect();
        history.reverse();
        let model = self.selected.clone();
        let output = match evidence {
            Some(facts) => ModelOutput::Answer(kernel.request(ctx, json!({
                "op":"compose", "model":model, "question":question, "facts":facts, "history":history,
            }))),
            None => ModelOutput::Plan(kernel.request(ctx, json!({
                "op":"plan", "model":model, "question":question, "history":history,
            }))),
        };
        self.running = Some(PendingAnalysis {
            turn: Some(index),
            model,
            output,
        });
    }
}

#[derive(Deserialize)]
struct KernelAnswer {
    #[serde(flatten)]
    answer: AssistantReply,
    #[serde(default)]
    direct_result: bool,
    context: Option<String>,
}

pub struct AssistantView {
    world_id: String,
    local: LocalMode,
    cloud: CloudMode,
    web: external::ExternalWeb,
    kernel: KernelClient,
    asking: Option<(usize, Receiver<Result<KernelAnswer, String>>)>,
    turns: Vec<Turn>,
    reset_scroll: bool,
    scroll_to_latest: bool,
}

impl AssistantView {
    pub fn new(kernel: PathBuf, resources: PathBuf) -> Self {
        Self {
            kernel: KernelClient::new(kernel, resources),
            world_id: String::new(),
            local: LocalMode::default(),
            cloud: CloudMode::default(),
            web: Default::default(),
            asking: None,
            turns: Vec::new(),
            reset_scroll: true,
            scroll_to_latest: false,
        }
    }

    fn select_world(&mut self, id: &str) {
        if self.world_id != id {
            self.reset_session();
            self.world_id = id.to_owned();
        }
    }

    pub fn reset_session(&mut self) {
        self.turns.clear();
        self.asking = None;
        self.kernel.reset();
        self.reset_scroll = true;
        self.scroll_to_latest = false;
        self.local.queued.clear();
        self.local.running = None;
        if self.local.listing.take().is_some() && self.local.enabled {
            self.local.resume_discovery = true;
        }
        self.local.list_error = None;
        self.cloud.reset_conversation();
        self.web = Default::default();
    }

    pub(crate) fn prepare(&mut self, world_id: &str, ctx: &egui::Context) {
        self.select_world(world_id);
        self.poll_answer();
        if std::mem::take(&mut self.local.resume_discovery) {
            self.local.discover(ctx, &self.kernel);
        }
        self.local.poll(&mut self.turns, ctx, &self.kernel);
        self.cloud.poll(&mut self.turns);
    }

    pub fn can_ask(&self) -> bool {
        self.asking.is_none()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _data: &Data,
        store: &Store,
        submitted: Option<String>,
        entry: &mut SearchBox,
    ) -> Option<String> {
        let world = store.world();
        let palette = design::palette(ui.ctx());
        let mut route = None;
        let mut question = submitted;
        let mut web_index = None;

        ui.horizontal(|ui| {
            design::icon(ui, "warehouse", 16.);
            ui.label(
                RichText::new(format!("当前世界 · {}", text(world, "name")))
                    .color(palette.muted)
                    .size(13.),
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if !self.turns.is_empty() && design::small_action(ui, "reset", "清空对话").clicked()
                {
                    self.reset_session();
                    entry.clear_input();
                    question = None;
                }
                ui.weak(concat!("助手版本 ", env!("CARGO_PKG_VERSION")));
            });
        });
        self.mode_controls(ui);
        ui.add_space(4.);

        let height = ui.available_height();
        let mut history = crate::navigation::ScrollArea::vertical()
            .id_salt(("pal-assistant-history", &self.world_id))
            .auto_shrink([false, false]);
        if std::mem::take(&mut self.reset_scroll) {
            history = history.vertical_scroll_offset(0.);
        }
        history.show(ui, |ui| {
            if self.turns.is_empty() {
                let spacing = ((height - 190.) * 0.34).max(0.);
                ui.add_space(spacing);
                design::section(ui, "试着问");
                for &(icon, prompt) in STARTERS {
                    if design::action(ui, icon, prompt).clicked() {
                        question = Some(prompt.to_owned());
                    }
                }
            }
            for (i, turn) in self.turns.iter().enumerate() {
                ui.push_id(i, |ui| {
                    ui.add_space(4.);
                    ui.label(
                        RichText::new(format!("你　{}", turn.question))
                            .size(15.)
                            .color(palette.accent)
                            .strong(),
                    );
                    design::card(ui).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if self.asking.as_ref().is_some_and(|(index, _)| *index == i) {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak("内核正在查询本地资料…");
                            });
                            return;
                        }
                        match turn.generated_answer() {
                            Some((model, answer)) => {
                                ui.weak(model);
                                ui.label(answer);
                                if !turn.answer.links.is_empty() {
                                    ui.collapsing("查看工具箱依据", |ui| {
                                        ui.label(&turn.answer.text);
                                    });
                                }
                            }
                            None => {
                                ui.label(&turn.answer.text);
                            }
                        }
                        for comment in [turn.analysis.as_ref(), turn.cloud_reply.as_ref()]
                            .into_iter()
                            .flatten()
                        {
                            if let Err(reason) = &comment.result {
                                ui.weak(format!("{} · {reason}", comment.model));
                            }
                        }
                        if let Some((source, answer)) = &turn.web_reply {
                            ui.separator();
                            ui.weak(format!("{source} · 手动导入"));
                            ui.label(answer);
                        }
                        if !turn.answer.links.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for (label, target) in &turn.answer.links {
                                    if article_url(target) {
                                        ui.hyperlink_to(label, target);
                                    } else if target.starts_with('#') && ui.link(label).clicked() {
                                        route = Some(target.clone());
                                    }
                                }
                            });
                        }
                        if self.local.enabled && !self.local.selected.is_empty() && !turn.direct_result {
                            if self
                                .local
                                .running
                                .as_ref()
                                .is_some_and(|task| task.turn == Some(i))
                            {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.weak(format!("{} 正在分析…", self.local.selected));
                                });
                            } else if self.local.queued.iter().any(|(turn, _)| *turn == i) {
                                ui.weak("上一个本机请求结束后分析这条问题。");
                            } else if i + 1 == self.turns.len()
                                && turn.analysis.as_ref().is_none_or(|c| {
                                    c.model != format!("本机 AI · {}", self.local.selected)
                                        || c.result.is_err()
                                })
                                && ui.button("用本机模型分析").clicked()
                            {
                                self.local.queued.push_back((i, Step::Plan));
                            }
                        }
                        if self.cloud.enabled {
                            self.cloud.turn_controls(ui, i, turn);
                        }
                        if ui.small_button("用免费网页版问").clicked() {
                            web_index = Some(i);
                        }
                        if i + 1 == self.turns.len()
                            && turn.answer.links.is_empty()
                            && turn.answer.suggestions.is_empty()
                            && entry.is_empty()
                            && ui.button("补充这条问题").clicked()
                        {
                            entry.prefill(format!(
                                "{}，",
                                turn.question.trim_end_matches(['？', '?', '。'])
                            ));
                        }
                    });
                    if i + 1 == self.turns.len() {
                        ui.horizontal_wrapped(|ui| {
                            for next in &turn.answer.suggestions {
                                if ui.button(next).clicked() {
                                    question = Some(next.clone());
                                }
                            }
                        });
                    }
                });
            }
            if std::mem::take(&mut self.scroll_to_latest) {
                ui.scroll_to_cursor(Some(Align::BOTTOM));
            }
        });
        if let Some(question) = question
            && self.can_ask()
            && !question.trim().is_empty()
        {
            entry.clear_input();
            let question = question.trim().to_owned();
            self.asking = Some((
                self.turns.len(),
                self.kernel.request(
                    ui.ctx(),
                    json!({
                        "op":"ask", "question":question, "world":world
                    }),
                ),
            ));
            let turn = Turn {
                question,
                direct_result: false,
                answer: AssistantReply {
                    text: String::new(),
                    links: vec![],
                    suggestions: vec![],
                },
                context: None,
                analysis: None,
                cloud_reply: None,
                web_reply: None,
            };
            let use_model = self.local.enabled && !self.local.selected.is_empty();
            self.turns.push(turn);
            if use_model {
                self.local
                    .queued
                    .push_back((self.turns.len() - 1, Step::Plan));
            } else if self.cloud.enabled {
                self.cloud.enqueue(self.turns.len() - 1);
            }
            self.scroll_to_latest = true;
            ui.ctx().request_repaint();
        }
        if self.asking.is_none() {
            self.local.run_queued(ui.ctx(), &self.turns, &self.kernel);
            self.run_cloud(ui.ctx());
        }
        if let Some(index) = web_index {
            self.open_web_dialog(index);
        }
        self.web_dialog(ui.ctx());
        route
    }

    fn poll_answer(&mut self) {
        if let Some(result) = self.asking.as_ref().and_then(|(_, rx)| finished(rx))
            && let Some((index, _)) = self.asking.take()
            && let Some(turn) = self.turns.get_mut(index)
        {
            match result {
                Ok(result) => {
                    turn.answer = result.answer;
                    turn.direct_result = result.direct_result;
                    if turn.direct_result {
                        self.local.queued.retain(|(i, _)| *i != index);
                    }
                    turn.context = result.context;
                }
                Err(error) => {
                    turn.answer.text = format!("增强内核暂时无法回答。{error}");
                    self.local.queued.retain(|(i, _)| *i != index);
                    self.cloud.cancel_turn(index);
                }
            }
            self.scroll_to_latest = true;
        }
    }
}

fn article_url(target: &str) -> bool {
    if matches!(target, "https://pocketpair.jp/en/game-contact-en/"
        | "https://steamcommunity.com/app/1623730/discussions/0/592886359937520760/"
        | "https://palworld.run/items/training-manual-xl") {
        return true;
    }
    [
        "https://docs.palworldgame.com/",
        "https://palworld.gamevault.in/guide/",
        "https://www.thepalwiki.com/wiki/",
    ].iter().any(|prefix| target.starts_with(prefix))
}

#[cfg(test)]
impl Default for AssistantView {
    fn default() -> Self {
        Self::new(PathBuf::new(), PathBuf::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn ready<T>(value: Result<T, String>) -> Receiver<Result<T, String>> {
        let (tx, rx) = mpsc::channel();
        assert!(tx.send(value).is_ok());
        rx
    }

    fn pending(output: ModelOutput) -> PendingAnalysis {
        PendingAnalysis {
            turn: Some(0),
            model: "local-test".into(),
            output,
        }
    }

    fn unknown_turn(question: &str) -> Turn {
        Turn {
            question: question.into(),
            direct_result: false,
            answer: AssistantReply {
                text: "暂时没有找到对应的资料。".into(),
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
    fn retrieved_evidence_is_merged_before_synthesis_and_late_answers_are_discarded() {
        let ctx = egui::Context::default();
        let mut view = AssistantView::default();
        view.select_world("old");
        let mut turn = unknown_turn("打首领要提升输出，怎么看这只帕鲁的潜力？");
        turn.answer.text = "先比较攻击加成。".into();
        turn.answer
            .links
            .push(("饰品图鉴".into(), "#items/Accessory_AT_1".into()));
        turn.answer.suggestions.push("获取难度呢？".into());
        view.turns.push(turn);
        view.local.model_changed(Some(0));
        assert_eq!(view.local.queued.pop_front(), Some((0, Step::Plan)));
        let evidence = || AssistantReply {
            text: "伯乐眼镜可以查看个体潜力。".into(),
            links: vec![("伯乐眼镜".into(), "#items/Accessory_TalentChecker".into())],
            suggestions: vec![],
        };
        view.local.running = Some(pending(ModelOutput::Evidence(
            ready(Ok(Some(evidence()))),
            "你想看哪项能力？".into(),
        )));
        view.local.poll(&mut view.turns, &ctx, &view.kernel);
        assert_eq!(view.local.queued.pop_front(), Some((0, Step::Compose)));
        view.turns[0].merge_evidence(evidence());
        assert_eq!(view.turns[0].answer.links.len(), 2);
        assert_eq!(view.turns[0].answer.text.matches("伯乐眼镜").count(), 1);
        assert_eq!(view.turns[0].answer.suggestions, ["获取难度呢？"]);
        view.local.running = Some(pending(ModelOutput::Answer(ready(Ok(
            "上个世界的分析".into()
        )))));
        view.select_world("new");
        view.turns.push(unknown_turn("问个新问题"));
        view.local.poll(&mut view.turns, &ctx, &view.kernel);
        assert!(view.turns[0].analysis.is_none());
        assert!(view.local.queued.is_empty() && view.local.running.is_none());
    }

    #[test]
    fn clarification_is_shown_even_if_the_initial_lookup_found_links() {
        let mut view = AssistantView::default();
        let mut turn = unknown_turn("该选哪一种？");
        turn.answer.links.push(("饰品".into(), "#items".into()));
        view.turns.push(turn);
        let question = "你想提高角色负重，还是提高搬运效率？";
        let plan = QueryPlan::Clarify { question: question.into() };
        view.local.running = Some(pending(ModelOutput::Plan(ready(Ok(plan)))));
        view.local.poll(&mut view.turns, &egui::Context::default(), &view.kernel);
        assert!(view.local.queued.is_empty());
        assert_eq!(view.turns[0].generated_answer().unwrap().1, question);
    }

    #[test]
    fn model_selected_field_replaces_unrelated_evidence_before_synthesis() {
        let mut view = AssistantView::default();
        let mut turn = unknown_turn("它在据点做哪些工作？");
        turn.answer.text = "一条战斗资料".into();
        turn.answer.links.push(("战斗".into(), "#combat".into()));
        view.turns.push(turn);
        let evidence = AssistantReply {
            text: "该物种有采矿适性。".into(),
            links: vec![("工作适性".into(), "#paldex/example".into())],
            suggestions: vec![],
        };
        view.local.running = Some(pending(ModelOutput::Plan(ready(Ok(QueryPlan::Evidence { answer: evidence.clone(), complete: false })))));
        view.local.poll(&mut view.turns, &egui::Context::default(), &view.kernel);
        assert_eq!(view.turns[0].answer.text, "该物种有采矿适性。");
        assert_eq!(view.turns[0].answer.links.len(), 1);
        assert_eq!(view.local.queued.pop_front(), Some((0, Step::Compose)));
        view.local.running = Some(pending(ModelOutput::Plan(ready(Ok(QueryPlan::Evidence { answer: evidence, complete: true })))));
        view.local.poll(&mut view.turns, &egui::Context::default(), &view.kernel);
        assert!(view.turns[0].direct_result && view.local.queued.is_empty());
    }

    #[test]
    fn verified_tool_answer_survives_model_selection_and_queued_analysis() {
        let mut view = AssistantView::default();
        view.local.enabled = true;
        view.local.selected = "local-test".into();
        view.turns.push(unknown_turn("现有 5 个，还缺多少？"));
        view.local.model_changed(Some(0));
        view.asking = Some((0, ready(Ok(KernelAnswer {
            direct_result: true,
            context: Some("现有 5 个，目标 8 个".into()),
            answer: AssistantReply {
                text: "还缺 3 个。".into(),
                links: vec![],
                suggestions: vec![],
            },
        }))));
        view.poll_answer();
        assert!(view.local.queued.is_empty());
        view.local.model_changed(Some(0));
        view.local.run_queued(&egui::Context::default(), &view.turns, &view.kernel);
        assert!(view.local.running.is_none());
        assert_eq!(view.turns[0].answer.text, "还缺 3 个。");
    }

    #[test]
    fn failed_kernel_query_is_displayed_without_sending_an_incomplete_cloud_prompt() {
        let mut view = AssistantView::default();
        view.turns.push(unknown_turn("蛋糕"));
        view.local.queued.push_back((0, Step::Plan));
        view.asking = Some((0, ready(Err("内核已关闭。".into()))));
        view.poll_answer();
        assert!(view.asking.is_none() && view.local.queued.is_empty());
        assert!(view.turns[0].answer.text.contains("内核已关闭"));
    }

    #[test]
    fn followups_remember_the_actual_clarifying_question_and_skip_model_errors() {
        let mut turn = unknown_turn("那个怎么选？");
        turn.context = Some("只能确认这是个模糊的游戏问题。".into());
        turn.analysis = Some(ModelComment {
            model: "local-test".into(),
            result: Ok(format!(
                "{}你说的是饰品，还是帕鲁的词条？",
                "解释取舍。".repeat(80)
            )),
        });
        let history = turn.history_text();
        assert!(
            history.contains("解释取舍。") && history.ends_with("你说的是饰品，还是帕鲁的词条？")
        );
        assert!(history.chars().count() < 260);
        turn.analysis.as_mut().unwrap().result = Err("连接中断".into());
        assert_eq!(turn.history_text(), turn.context.as_deref().unwrap());
    }

    #[test]
    fn submitted_search_questions_and_source_links_use_the_same_session() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut data = Data::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
            ctx.clone(),
        )
        .unwrap();
        let start = std::time::Instant::now();
        while data.pending > 0 && start.elapsed().as_secs() < 20 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            data.poll();
        }
        assert_eq!(data.pending, 0);
        assert!(data.errors.is_empty());
        let temp = crate::store::tests::Temp::new();
        let store = Store::open(temp.0.clone());
        let mut view = AssistantView::default();
        let entry = std::cell::RefCell::new(crate::assistant_search::SearchBox::default());
        let frame =
            |view: &mut AssistantView, events: Vec<egui::Event>, mut submitted: Option<String>| {
                let mut route = None;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(820., 620.),
                        )),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            view.prepare(text(store.world(), "id"), ctx);
                            let mut entry = entry.borrow_mut();
                            let question = match entry.ui(ui, &data, view.can_ask()) {
                                Some(crate::assistant_search::SearchAction::Open(result)) => {
                                    route = Some(result);
                                    return;
                                }
                                Some(crate::assistant_search::SearchAction::Ask(question)) => {
                                    Some(question)
                                }
                                None => submitted.take(),
                            };
                            route = view.ui(ui, &data, &store, question, &mut entry);
                        });
                    },
                );
                (route, output)
            };
        let click = |view: &mut AssistantView, label: &str| {
            frame(view, vec![], None);
            let output = frame(view, vec![], None).1;
            let pos = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::epaint::Shape::Text(t) = &shape.shape
                        && t.galley.job.text == label
                    {
                        Some(t.pos + t.galley.size() * 0.5)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("control not rendered: {label}"));
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(
                view,
                vec![egui::Event::PointerMoved(pos), pointer(true)],
                None,
            );
            frame(view, vec![pointer(false)], None)
        };

        entry.borrow_mut().prefill("帕鲁矿碎块怎么获得？".into());
        click(&mut view, "问助手");
        assert_eq!(view.turns.len(), 1);
        assert!(entry.borrow().is_empty());
        let (answer_tx, answer_rx) = mpsc::channel();
        view.asking = Some((0, answer_rx));
        assert!(!view.can_ask());
        frame(&mut view, vec![], Some("重复发送".into()));
        assert_eq!(view.turns.len(), 1);
        assert!(
            answer_tx
                .send(Ok(KernelAnswer {
                    direct_result: false,
                    answer: AssistantReply {
                        text: "可以在图鉴里查看采集资料。".into(),
                        links: vec![("采集资料".into(), "#guide/gathering".into())],
                        suggestions: vec![],
                    },
                    context: None,
                }))
                .is_ok()
        );
        view.poll_answer();
        assert!(view.can_ask());
        let (label, expected) = view.turns[0].answer.links.first().unwrap().clone();
        assert_eq!(click(&mut view, &label).0, Some(expected));
        let source = "https://docs.palworldgame.com/getting-started/about-server/";
        view.turns[0].answer.links.push(("专用服务器原文".into(), source.into()));
        let (route, output) = click(&mut view, "专用服务器原文");
        assert!(route.is_none());
        assert!(output.platform_output.commands.iter().any(|cmd| matches!(cmd, egui::OutputCommand::OpenUrl(target) if target.url == source)));
        assert!(!article_url("https://docs.palworldgame.com.example.org/"));

        entry.borrow_mut().prefill("概率是多少？".into());
        click(&mut view, "问助手");
        assert_eq!(view.turns.len(), 2);
        assert_eq!(view.turns[1].question, "概率是多少？");
        click(&mut view, "清空对话");
        assert!(view.turns.is_empty());
        view.turns.push(unknown_turn("我该换哪个？"));
        let output = frame(&mut view, vec![], None).1;
        let visible = |label: &str| {
            output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::epaint::Shape::Text(t) if t.galley.job.text == label)
        })
        };
        assert!(visible("增强问答 · 离线可用") && visible("补充这条问题"));
        assert!(visible(concat!("助手版本 ", env!("CARGO_PKG_VERSION"))));
        click(&mut view, "补充这条问题");
        assert!(!entry.borrow().is_empty());
        entry.borrow_mut().clear_input();
        assert!(!view.local.enabled);

        click(&mut view, "免费云端 DeepSeek");
        assert!(view.cloud.enabled && !view.local.enabled);
        assert!(view.turns[0].cloud_reply.is_none());
        view.turns[0].cloud_reply = Some(ModelComment {
            model: "云端来源".into(),
            result: Ok("这是一条云端回答。".into()),
        });
        view.turns[0].web_reply = Some(("ChatGPT".into(), "网页回答保存在同一轮里。".into()));
        click(&mut view, "增强问答 · 离线可用");
        assert!(!view.cloud.enabled);
        let output = frame(&mut view, vec![], None).1;
        for label in [
            "云端来源",
            "这是一条云端回答。",
            "ChatGPT · 手动导入",
            "网页回答保存在同一轮里。",
        ] {
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::epaint::Shape::Text(t) if t.galley.job.text == label)
            }), "{label}");
        }
    }

    #[test]
    fn changing_world_drops_pending_answers_and_preserves_the_selected_model() {
        let mut view = AssistantView::default();
        view.select_world("first");
        view.turns.push(unknown_turn("谁适合生火？"));
        view.local.enabled = true;
        view.local.selected = "local-test".into();
        view.local.models.push("local-test".into());
        let (list_tx, list_rx) = mpsc::channel();
        view.local.listing = Some(list_rx);
        let (answer_tx, answer_rx) = mpsc::channel();
        view.asking = Some((0, answer_rx));
        let (model_tx, model_rx) = mpsc::channel();
        view.local.running = Some(pending(ModelOutput::Answer(model_rx)));
        view.local.queued.push_back((0, Step::Plan));
        view.select_world("first");
        assert_eq!(view.turns.len(), 1);
        assert!(!view.can_ask());
        view.select_world("second");
        assert!(view.turns.is_empty());
        assert!(view.can_ask() && view.local.running.is_none() && view.local.queued.is_empty());
        assert!(answer_tx.send(Err("之前世界的查询".into())).is_err());
        assert!(model_tx.send(Ok("之前世界的分析".into())).is_err());
        assert!(list_tx.send(Ok(vec!["过期模型".into()])).is_err());
        assert!(view.local.resume_discovery && view.local.listing.is_none());
        assert!(view.local.enabled);
        assert_eq!(view.local.selected, "local-test");
    }
}
