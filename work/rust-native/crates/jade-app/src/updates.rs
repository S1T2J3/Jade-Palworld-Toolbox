//! Separate Rust beta release channel. Downloads are data until archive and
//! per-file hashes have both passed verification. The stable channel is untouched.
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "S1T2J3/Jade-Palworld-Toolbox";
const EXE: &str = "Jade-Palworld-Rust.exe";
const PACKAGE: &str = "rust-package.json";
const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL: u64 = 4 * 1024 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageManifest {
    pub schema: u32,
    pub version: String,
    pub files: Vec<PackageFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Archive {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delta {
    #[serde(rename = "fromManifestSha256")]
    pub from_manifest_sha256: String,
    #[serde(flatten)]
    pub archive: Archive,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub schema: u32,
    pub version: String,
    #[serde(rename = "releaseTag")]
    pub release_tag: String,
    pub package: PackageManifest,
    pub full: Archive,
    #[serde(default)]
    pub deltas: Vec<Delta>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notice {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(rename = "publishedAt", default)]
    pub published_at: String,
    #[serde(default = "yes")]
    pub popup: bool,
}
fn yes() -> bool {
    true
}
#[derive(Clone)]
struct Release {
    manifest: UpdateManifest,
    notes: String,
    url: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    version: String,
    stage: String,
    manifest_sha256: String,
}
enum Event {
    Checked(std::result::Result<Option<Release>, String>, Vec<Notice>),
    Progress(u64, u64),
    Downloaded(Result<Pending>, bool),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdateAction {
    Restart,
}

pub struct UpdateState {
    root: PathBuf,
    exe: PathBuf,
    started: Instant,
    checked: bool,
    busy: bool,
    channel: Option<Receiver<Event>>,
    latest: Option<Release>,
    pub status: String,
    progress: Option<(u64, u64)>,
    notices: Vec<Notice>,
    local_notices_seeded: bool,
    remote_notices_loaded: bool,
    popup: Option<Notice>,
    popup_shown: bool,
    popup_pass: Option<u64>,
    open_notices: HashSet<String>,
    last_checked: Option<Instant>,
    pending: Option<Pending>,
    install_prompt: bool,
}
impl UpdateState {
    pub fn unread_count(&self, state: &Value) -> usize {
        self.notices
            .iter()
            .filter(|n| {
                !state["readAnnouncements"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(&n.id)))
            })
            .count()
    }
    pub fn new(data_dir: PathBuf, exe_path: PathBuf) -> Self {
        let pending = read_pending(&data_dir).ok().flatten();
        let result = fs::read(data_dir.join("updates/result.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        let status = result
            .and_then(|v| v["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "当前为独立 Rust 测试通道。".into());
        Self{root:data_dir,exe:exe_path,started:Instant::now(),checked:false,busy:false,channel:None,latest:None,status,progress:None,
            notices:vec![Notice{id:format!("rust-release-{VERSION}"),title:format!("Rust 原生测试版 {VERSION}"),body:"点击配种、仓库、地图、工作和计算器中的帕鲁名称或头像，统一先打开完整宽度的帕鲁资料页。页首先选择地图与刷新位置、查配种伙伴、反查双亲、目标配种、属性值计算或捕捉概率。明确标注的功能按钮仍直接跳转，帕鲁选择器继续用于选择。配种查询返回时保留原物种和查伙伴/反查双亲模式；子代结果保留独立配种路线入口。QQ 交流群：1126734443。".into(),published_at:String::new(),popup:true}],local_notices_seeded:false,remote_notices_loaded:false,popup:None,popup_shown:false,popup_pass:None,open_notices:HashSet::new(),last_checked:None,pending,install_prompt:false}
    }
    /// Bundled notices arrive on the data worker. Seed them exactly once when
    /// ready, retaining this version's fallback if an older package feed omits it.
    /// A successful remote response always takes precedence over bundled data.
    pub fn seed_local_notices(&mut self, value: &Value) {
        if self.local_notices_seeded || self.remote_notices_loaded {
            return;
        }
        let Some(rows) = value.get("announcements").filter(|rows| rows.is_array()) else {
            return;
        };
        let Ok(mut notices) = serde_json::from_value::<Vec<Notice>>(rows.clone()) else {
            return;
        };
        deduplicate_notices(&mut notices);
        let current_id = format!("rust-release-{VERSION}");
        if !notices.iter().any(|notice| notice.id == current_id) {
            if let Some(current) = self
                .notices
                .iter()
                .find(|notice| notice.id == current_id)
                .cloned()
            {
                notices.insert(0, current);
            }
        }
        self.notices = notices;
        self.local_notices_seeded = true;
    }
    fn check(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.checked = true;
        self.status = "正在检查 Rust 测试版更新…".into();
        let (tx, rx) = mpsc::channel();
        self.channel = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let releases = check_release();
            let notices = read_notices().unwrap_or_default();
            let _ = tx.send(Event::Checked(releases, notices));
            ctx.request_repaint();
        });
    }
    fn download(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let Some(release) = self.latest.clone() else {
            return;
        };
        self.busy = true;
        self.status = "正在下载更新并检查文件…".into();
        self.progress = Some((0, release.manifest.full.size));
        let root = self.root.clone();
        let install = self.exe.parent().unwrap_or(Path::new(".")).to_path_buf();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.channel = Some(rx);
        std::thread::spawn(move || {
            let result = prepare_update(&release.manifest, &root, &install, &tx);
            let (answer, delta) = match result {
                Ok(v) => (Ok(v.0), v.1),
                Err(e) => (Err(e), false),
            };
            let _ = tx.send(Event::Downloaded(answer, delta));
            ctx.request_repaint();
        });
    }
    /// Call each UI frame. The first check starts after the initial frame grace;
    /// network and hashing are performed on a worker thread.
    pub fn tick(&mut self, ctx: &egui::Context, state: &mut Value) -> Option<UpdateAction> {
        if !self.checked
            && self.started.elapsed() > Duration::from_millis(1800)
            && state["autoCheckUpdates"] != false
        {
            self.check(ctx)
        }
        if !self.checked {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        let events: Vec<_> = self
            .channel
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        let mut auto_download = false;
        for event in events {
            match event {
                Event::Checked(result, notices) => {
                    self.busy = false;
                    self.last_checked = Some(Instant::now());
                    if !notices.is_empty() {
                        self.notices = notices;
                        deduplicate_notices(&mut self.notices);
                        self.remote_notices_loaded = true;
                    }
                    match result {
                        Ok(Some(release)) => {
                            self.status =
                                format!("发现 Rust 测试版 {}。", release.manifest.version);
                            self.latest = Some(release);
                            auto_download = state["autoDownload"] == true;
                        }
                        Ok(None) => self.status = "当前已是最新 Rust 测试版。".into(),
                        Err(e) => self.status = format!("暂时无法检查更新：{e}"),
                    }
                }
                Event::Progress(done, total) => self.progress = Some((done, total)),
                Event::Downloaded(result, delta) => {
                    self.busy = false;
                    self.progress = None;
                    match result {
                        Ok(p) => {
                            self.status = format!(
                                "{}更新已下载并通过校验，可立即安装，也可留待下次启动。",
                                if delta { "增量" } else { "完整" }
                            );
                            self.pending = Some(p);
                            self.install_prompt = true
                        }
                        Err(e) => self.status = format!("更新未安装：{e}"),
                    }
                }
            }
        }
        if auto_download
            && self.pending.as_ref().map(|p| &p.version)
                != self.latest.as_ref().map(|r| &r.manifest.version)
        {
            self.download(ctx)
        }
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
        self.queue_announcement(state);
        if !self.install_prompt {
            self.show_notice_popup(ctx);
        }
        let mut action = None;
        if self.install_prompt {
            let mut close = false;
            let palette = crate::design::palette(ctx);
            let modal = egui::Modal::new(egui::Id::new("rust-update-install-prompt"))
                .frame(
                    egui::Frame::new()
                        .fill(palette.surface)
                        .stroke(egui::Stroke::new(1., palette.line))
                        .corner_radius(12)
                        .inner_margin(18),
                )
                .show(ctx, |ui| {
                    ui.set_width(490f32.min(ctx.content_rect().width() - 64.));
                    if crate::design::dialog_header(ui, "download", "安装更新并重启？").clicked()
                    {
                        close = true;
                    }
                    ui.add_space(10.);
                    ui.label(format!(
                        "Rust 测试版 {} 已下载并校验通过。",
                        self.pending
                            .as_ref()
                            .map(|p| p.version.as_str())
                            .unwrap_or("")
                    ));
                    ui.label("现在安装并重启应用吗？");
                    ui.add_space(10.);
                    ui.horizontal_wrapped(|ui| {
                        if crate::design::action(ui, "restart", "是，安装并重启").clicked() {
                            match begin_install(&self.root, &self.exe) {
                                Ok(()) => action = Some(UpdateAction::Restart),
                                Err(e) => self.status = e,
                            }
                            self.install_prompt = false;
                        }
                        if crate::design::action(ui, "close", "否，下次启动时安装").clicked()
                        {
                            self.install_prompt = false;
                            self.status = "更新已保留，将在下次启动时安装。".into();
                        }
                    });
                });
            if close || modal.should_close() {
                self.install_prompt = false;
                self.status = "更新已保留，将在下次启动时安装。".into();
            }
        }
        action
    }
    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut Value) -> Option<UpdateAction> {
        egui::ScrollArea::vertical()
            .id_salt("release-center-scroll")
            .show(ui, |ui| {
                if ui.available_width() >= 850. {
                    let width = (ui.available_width() - 18.) / 2.2;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 18.;
                        ui.allocate_ui_with_layout(
                            egui::vec2(width * 1.2, 0.),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.notices_card(ui, state, width * 1.2);
                            },
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(width, 0.),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.updates_card(ui, state, width);
                            },
                        );
                    });
                } else {
                    let width = ui.available_width();
                    self.notices_card(ui, state, width);
                    ui.add_space(12.);
                    self.updates_card(ui, state, width);
                }
            });
        // A notice opened from this screen must also work in an offline/QA run
        // where the background update poller is intentionally not executed.
        self.show_notice_popup(ui.ctx());
        None
    }
    fn queue_announcement(&mut self, state: &mut Value) {
        if self.popup.is_some()
            || self.popup_shown
            || self.install_prompt
            || state["announcementPopups"] == false
        {
            return;
        }
        if let Some(notice) = self
            .notices
            .iter()
            .find(|n| n.popup && !notice_read(state, &n.id))
            .cloned()
        {
            mark_notice_read(state, &notice.id);
            self.popup = Some(notice);
            self.popup_shown = true;
        }
    }
    /// The original notice dialog has its own heading and a real close icon.
    /// Keep one draw per egui pass when both the app poller and this page run.
    pub fn show_notice_popup(&mut self, ctx: &egui::Context) {
        let Some(notice) = self.popup.clone() else {
            return;
        };
        if self.install_prompt || self.popup_pass == Some(ctx.cumulative_pass_nr()) {
            return;
        }
        self.popup_pass = Some(ctx.cumulative_pass_nr());
        let palette = crate::design::palette(ctx);
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("rust-announcement-popup"))
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(egui::Stroke::new(1., palette.line))
                    .corner_radius(12)
                    .inner_margin(20),
            )
            .show(ctx, |ui| {
                ui.set_width(580f32.min(ctx.content_rect().width() - 72.).max(240.));
                let response = crate::design::dialog_header(ui, "bell", &notice.title);
                update_test_rect(ui, "notice-close", &response);
                if response.clicked() {
                    close = true;
                }
                let date = notice_date(&notice.published_at);
                if !date.is_empty() {
                    ui.add_space(5.);
                    ui.weak(date);
                }
                ui.add_space(12.);
                egui::ScrollArea::vertical()
                    .id_salt(("notice-body", &notice.id))
                    .max_height((ctx.content_rect().height() * 0.62 - 130.).clamp(100., 460.))
                    .show(ui, |ui| {
                        ui.label(&notice.body);
                    });
                ui.add_space(12.);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.small("本公告已自动标记为已阅。");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let response = crate::design::action(ui, "check", "知道了");
                        update_test_rect(ui, "notice-confirm", &response);
                        if response.clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close || modal.should_close() {
            self.popup = None;
        }
    }
    fn notices_card(&mut self, ui: &mut egui::Ui, state: &mut Value, width: f32) {
        crate::design::card(ui).inner_margin(18).show(ui, |ui| {
            ui.set_width((width - 36.).max(180.));
            ui.horizontal(|ui| {
                crate::design::icon(ui, "bell", 21.);
                ui.label(egui::RichText::new("公告").size(20.).strong());
                let unread = self.unread_count(state);
                if unread > 0 {
                    crate::design::badge(ui, format!("{unread} 条未读"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let response = crate::design::small_action(ui, "check", "全部标为已读");
                        update_test_rect(ui, "notice-read-all", &response);
                        if response.clicked() {
                            for notice in &self.notices {
                                mark_notice_read(state, &notice.id);
                            }
                        }
                    });
                }
            });
            ui.add_space(8.);
            let mut popups = state["announcementPopups"] != false;
            let response = crate::design::checkbox(ui, &mut popups, "弹窗提醒重要公告");
            update_test_rect(ui, "notice-popup-setting", &response);
            if response.changed() {
                state["announcementPopups"] = json!(popups);
                if !popups {
                    self.popup = None;
                }
            }
            ui.small("关闭后，重要公告仍可在这里查看。");
            if self.notices.is_empty() {
                ui.add_space(12.);
                ui.weak("暂时没有公告，检查更新可获取最新内容。");
            }
            for notice in self.notices.clone() {
                ui.add_space(8.);
                ui.separator();
                ui.add_space(5.);
                let open = self.open_notices.contains(&notice.id);
                let unread = !notice_read(state, &notice.id);
                ui.horizontal_top(|ui| {
                    let width = (ui.available_width() - if unread { 55. } else { 0. }).max(100.);
                    let response = notice_heading(ui, &notice.title, open, width);
                    update_test_rect(ui, &format!("notice-heading:{}", notice.id), &response);
                    if response.clicked() {
                        if open {
                            self.open_notices.remove(&notice.id);
                        } else {
                            self.open_notices.insert(notice.id.clone());
                            mark_notice_read(state, &notice.id);
                        }
                    }
                    if unread {
                        crate::design::badge(ui, "未读");
                    }
                });
                if self.open_notices.contains(&notice.id) {
                    let date = notice_date(&notice.published_at);
                    if !date.is_empty() {
                        ui.weak(date);
                    }
                    ui.add_space(6.);
                    ui.label(&notice.body);
                    let response = crate::design::action(ui, "bell", "弹窗查看");
                    update_test_rect(ui, &format!("notice-popup:{}", notice.id), &response);
                    if response.clicked() {
                        mark_notice_read(state, &notice.id);
                        self.popup = Some(notice.clone());
                        self.popup_shown = true;
                    }
                }
            }
        });
    }
    fn updates_card(&mut self, ui: &mut egui::Ui, state: &mut Value, width: f32) {
        crate::design::card(ui).inner_margin(18).show(ui, |ui| {
            ui.set_width((width - 36.).max(180.));
            ui.horizontal(|ui| {
                crate::design::icon(ui, "download", 21.);
                ui.label(egui::RichText::new("软件更新").size(20.).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(format!("当前版本 v{VERSION}"));
                });
            });
            ui.add_space(10.);
            ui.label(&self.status);
            if let Some(last) = self.last_checked {
                let age = last.elapsed().as_secs();
                ui.small(if age < 60 {
                    "上次检查：刚刚".to_owned()
                } else {
                    format!("上次检查：{} 分钟前", age / 60)
                });
            }
            if let Some((done, total)) = self.progress {
                ui.add(
                    egui::ProgressBar::new(if total > 0 {
                        done as f32 / total as f32
                    } else {
                        0.
                    })
                    .text(format!(
                        "{:.1} / {:.1} MB",
                        done as f64 / 1e6,
                        total as f64 / 1e6
                    )),
                );
            }
            ui.add_space(7.);
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(!self.busy, |ui| {
                    let response = crate::design::action(ui, "refresh", "检查更新");
                    update_test_rect(ui, "update-check", &response);
                    if response.clicked() {
                        self.check(ui.ctx());
                    }
                });
                if self.latest.is_some() {
                    ui.add_enabled_ui(!self.busy, |ui| {
                        let response = crate::design::action(ui, "download", "下载更新");
                        update_test_rect(ui, "update-download", &response);
                        if response.clicked() {
                            self.download(ui.ctx());
                        }
                    });
                }
                if self.pending.is_some() {
                    let response = crate::design::action(ui, "restart", "立即安装并重启");
                    update_test_rect(ui, "update-install", &response);
                    if response.clicked() {
                        self.install_prompt = true;
                    }
                }
            });
            ui.add_space(10.);
            ui.separator();
            ui.add_space(5.);
            let mut check = state["autoCheckUpdates"] != false;
            let response = crate::design::checkbox(ui, &mut check, "启动后检查测试版更新");
            update_test_rect(ui, "update-auto-check", &response);
            if response.changed() {
                state["autoCheckUpdates"] = json!(check);
            }
            let mut download = state["autoDownload"] == true;
            let response = crate::design::checkbox(ui, &mut download, "自动下载更新");
            update_test_rect(ui, "update-auto-download", &response);
            if response.changed() {
                state["autoDownload"] = json!(download);
            }
            ui.small("打开工具箱后会在后台检查更新，优先下载增量包，下载完成后询问是否立即重启。");
            if let Some(release) = &self.latest {
                ui.add_space(10.);
                egui::CollapsingHeader::new("版本说明")
                    .id_salt("rust-release-notes")
                    .show(ui, |ui| {
                        ui.label(&release.notes);
                    });
            }
            ui.add_space(10.);
            let url = self
                .latest
                .as_ref()
                .map(|r| r.url.as_str())
                .unwrap_or("https://github.com/S1T2J3/Jade-Palworld-Toolbox/releases");
            let response =
                crate::design::action(ui, "external", "查看 GitHub 发布页").on_hover_text(url);
            update_test_rect(ui, "update-release-link", &response);
            if response.clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
            ui.add_space(5.);
            ui.small("Rust 测试版使用独立更新通道。");
        });
    }
}

fn deduplicate_notices(notices: &mut Vec<Notice>) {
    let mut seen = HashSet::new();
    notices.retain(|notice| !notice.id.trim().is_empty() && seen.insert(notice.id.clone()));
}
fn notice_read(state: &Value, id: &str) -> bool {
    state["readAnnouncements"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|v| v.as_str() == Some(id)))
}
fn notice_heading(ui: &mut egui::Ui, title: &str, open: bool, width: f32) -> egui::Response {
    let palette = crate::design::palette(ui.ctx());
    let title = ui.painter().layout(
        title.to_owned(),
        egui::FontId::proportional(15.),
        palette.ink,
        (width - 27.).max(60.),
    );
    let height = (title.size().y + 10.).max(34.);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 5, palette.soft);
    }
    egui::Image::new(crate::design::icon_source(if open {
        "down"
    } else {
        "right"
    }))
    .tint(palette.accent)
    .paint_at(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 9., rect.top() + 17.),
            egui::Vec2::splat(14.),
        ),
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 25., rect.top() + 5.),
        title,
        palette.ink,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn mark_notice_read(state: &mut Value, id: &str) {
    if !state["readAnnouncements"].is_array() {
        state["readAnnouncements"] = json!([]);
    }
    if !notice_read(state, id) {
        state["readAnnouncements"]
            .as_array_mut()
            .unwrap()
            .push(json!(id));
    }
}
fn notice_date(value: &str) -> String {
    let Some(day) = value.get(..10) else {
        return String::new();
    };
    let b = day.as_bytes();
    if b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        format!("{}年{}月{}日", &day[..4], &day[5..7], &day[8..10])
    } else {
        String::new()
    }
}
#[cfg(test)]
fn update_test_rect(ui: &egui::Ui, key: &str, response: &egui::Response) {
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new(format!("update-control:{key}")),
            response.rect,
        )
    });
}
#[cfg(not(test))]
fn update_test_rect(_: &egui::Ui, _: &str, _: &egui::Response) {}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn sha_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn beta_version(s: &str) -> Option<(u64, u64, u64, u64)> {
    let (main, beta) = s.split_once("-beta.")?;
    let p: Vec<_> = main.split('.').collect();
    if p.len() != 3 {
        return None;
    }
    Some((
        p[0].parse().ok()?,
        p[1].parse().ok()?,
        p[2].parse().ok()?,
        beta.parse().ok()?,
    ))
}
fn safe_relative(value: &str) -> Result<PathBuf> {
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value.ends_with('/')
        || value.len() > 240
    {
        return Err("更新包包含无效路径。".into());
    }
    let path = Path::new(value);
    if !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err("更新包路径越界。".into());
    }
    for part in value.split('/') {
        if part.is_empty()
            || part.ends_with('.')
            || part.ends_with(' ')
            || part.chars().any(|c| c < ' ' || "<>\"|?*".contains(c))
        {
            return Err("更新包包含不支持的文件名。".into());
        }
        let stem = part.split('.').next().unwrap_or("").to_uppercase();
        if [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
        {
            return Err("更新包包含保留文件名。".into());
        }
    }
    Ok(path.to_path_buf())
}
fn validate_package(package: &PackageManifest) -> Result<()> {
    if package.schema != 1
        || beta_version(&package.version).is_none()
        || package.files.is_empty()
        || package.files.len() > 100_000
    {
        return Err("更新清单格式不受支持。".into());
    }
    let mut total = 0u64;
    let mut names = HashSet::new();
    let mut exe = false;
    for file in &package.files {
        safe_relative(&file.path)?;
        if file.path == PACKAGE
            || !names.insert(file.path.to_lowercase())
            || !sha_valid(&file.sha256)
        {
            return Err("更新清单含重复文件或无效散列。".into());
        }
        total = total.checked_add(file.size).ok_or("更新体积超出限制")?;
        if file.size > MAX_DOWNLOAD || total > MAX_TOTAL {
            return Err("更新体积超出限制。".into());
        }
        if file.path == EXE {
            exe = true
        }
    }
    if !exe {
        return Err("更新缺少 Rust 主程序。".into());
    }
    Ok(())
}
fn validate_archive(archive: &Archive, tag: &str) -> Result<()> {
    let prefix = format!("https://github.com/{REPO}/releases/download/{tag}/");
    if archive.size == 0
        || archive.size > MAX_DOWNLOAD
        || !sha_valid(&archive.sha256)
        || archive.name.contains('/')
        || !archive.name.ends_with(".zip")
        || archive.url != format!("{prefix}{}", archive.name)
    {
        return Err("更新下载地址或校验信息无效。".into());
    }
    Ok(())
}
fn validate_update(m: &UpdateManifest) -> Result<()> {
    validate_package(&m.package)?;
    if m.schema != 1
        || m.version != m.package.version
        || m.release_tag != format!("rust-v{}", m.version)
    {
        return Err("更新版本信息不一致。".into());
    }
    validate_archive(&m.full, &m.release_tag)?;
    for delta in &m.deltas {
        if !sha_valid(&delta.from_manifest_sha256) {
            return Err("增量基线散列无效。".into());
        }
        validate_archive(&delta.archive, &m.release_tag)?
    }
    Ok(())
}
fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout)))
        .build()
        .into()
}
fn get_json(url: &str) -> Result<Value> {
    let mut response = agent(25)
        .get(url)
        .header("User-Agent", "Jade-Palworld-Rust-Beta")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(12 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 12 * 1024 * 1024 {
        return Err("远程清单超出大小限制。".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn check_release() -> Result<Option<Release>> {
    let feed = get_json(&format!(
        "https://api.github.com/repos/{REPO}/releases?per_page=100"
    ))?;
    let current = beta_version(VERSION).ok_or("当前测试版版本号无效")?;
    let Some(release) = feed
        .as_array()
        .ok_or("发布列表格式无效")?
        .iter()
        .filter(|r| r["prerelease"] == true && r["draft"] != true)
        .filter_map(|r| {
            r["tag_name"]
                .as_str()
                .and_then(|tag| tag.strip_prefix("rust-v"))
                .and_then(beta_version)
                .map(|v| (v, r))
        })
        .filter(|(v, _)| *v > current)
        .max_by_key(|(v, _)| *v)
        .map(|(_, r)| r)
    else {
        return Ok(None);
    };
    let tag = release["tag_name"].as_str().ok_or("版本标签缺失")?;
    let expected = format!("https://github.com/{REPO}/releases/download/{tag}/rust-update.json");
    let asset = release["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"] == "rust-update.json"))
        .ok_or("新版尚未提供原生更新清单")?;
    if asset["browser_download_url"] != expected {
        return Err("更新清单地址不匹配。".into());
    }
    let manifest: UpdateManifest =
        serde_json::from_value(get_json(&expected)?).map_err(|e| e.to_string())?;
    validate_update(&manifest)?;
    if manifest.release_tag != tag {
        return Err("发布标签与更新清单不一致。".into());
    }
    Ok(Some(Release {
        manifest,
        notes: release["body"].as_str().unwrap_or("").into(),
        url: format!("https://github.com/{REPO}/releases/tag/{tag}"),
    }))
}
fn read_notices() -> Result<Vec<Notice>> {
    let v = get_json(&format!(
        "https://raw.githubusercontent.com/{REPO}/main/rust-announcements.json"
    ))?;
    serde_json::from_value(v["announcements"].clone()).map_err(|e| e.to_string())
}
fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| e.to_string())
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("目标目录无效")?;
    create_dir(parent)?;
    let tmp = parent.join(format!(".jade-write-{}", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    let result = replace_file(&tmp, path);
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
fn replace_file(from: &Path, to: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        unsafe extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        let a: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let b: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 0x1 | 0x8) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::rename(from, to).map_err(|e| e.to_string())
    }
}
fn download_archive(archive: &Archive, path: &Path, tx: &Sender<Event>) -> Result<()> {
    let mut response = agent(900)
        .get(&archive.url)
        .header("User-Agent", "Jade-Palworld-Rust-Beta")
        .call()
        .map_err(|e| e.to_string())?;
    let mut out = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut input = response.body_mut().as_reader();
    let mut total = 0;
    let mut buf = [0u8; 65536];
    let mut last = Instant::now();
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > archive.size {
            return Err("下载文件大于声明大小。".into());
        }
        hash.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        if last.elapsed() > Duration::from_millis(100) {
            let _ = tx.send(Event::Progress(total, archive.size));
            last = Instant::now();
        }
    }
    out.sync_all().map_err(|e| e.to_string())?;
    if total != archive.size || format!("{:x}", hash.finalize()) != archive.sha256 {
        return Err("更新下载不完整或 SHA-256 校验失败。".into());
    }
    Ok(())
}
fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}
fn checked_source(base: &Path, relative: &str) -> Result<PathBuf> {
    let relative = safe_relative(relative)?;
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let path = base.join(relative);
    let actual = path.canonicalize().map_err(|e| e.to_string())?;
    if !actual.starts_with(&base) || !regular_file(&actual) {
        return Err("文件路径越出程序目录。".into());
    }
    Ok(actual)
}
fn checked_destination(base: &Path, relative: &str) -> Result<PathBuf> {
    let relative = safe_relative(relative)?;
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let target = base.join(relative);
    let mut ancestor = target.parent().ok_or("目标路径无效")?;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or("目标路径越界")?;
    }
    let resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
    if !resolved.starts_with(&base) {
        return Err("目标文件夹越出程序目录。".into());
    }
    Ok(target)
}
fn verify_files(package: &PackageManifest, root: &Path) -> Result<()> {
    validate_package(package)?;
    for file in &package.files {
        let path = checked_source(root, &file.path)?;
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() != file.size
            || hash_file(&path)? != file.sha256
        {
            return Err(format!("文件校验失败：{}", file.path));
        }
    }
    Ok(())
}
fn extract_package(zip_path: &Path, destination: &Path, package: &PackageManifest) -> Result<()> {
    validate_package(package)?;
    create_dir(destination)?;
    let input = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(input).map_err(|e| e.to_string())?;
    if archive.len() > package.files.len() + 1 {
        return Err("更新包文件数超过清单。".into());
    }
    let allowed: HashMap<_, _> = package.files.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut seen = HashSet::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        safe_relative(&name)?;
        if !seen.insert(name.to_lowercase())
            || entry.is_dir()
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("更新压缩包含重复路径或不支持的条目。".into());
        }
        if name == PACKAGE {
            if entry.size() > 12 * 1024 * 1024 {
                return Err("包清单过大。".into());
            }
            let mut b = Vec::new();
            entry.read_to_end(&mut b).map_err(|e| e.to_string())?;
            let actual: PackageManifest = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
            if serde_json::to_value(actual).unwrap() != serde_json::to_value(package).unwrap() {
                return Err("包内清单与发布清单不一致。".into());
            }
            continue;
        }
        let expected = allowed
            .get(name.as_str())
            .ok_or_else(|| format!("更新包含清单外文件：{name}"))?;
        if entry.size() != expected.size {
            return Err(format!("更新文件大小不匹配：{name}"));
        }
        let target = checked_destination(destination, &name)?;
        create_dir(target.parent().unwrap())?;
        let mut bytes = Vec::new();
        entry
            .take(expected.size + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 != expected.size || digest(&bytes) != expected.sha256 {
            return Err(format!("更新文件散列不匹配：{name}"));
        }
        atomic_write(&target, &bytes)?;
    }
    Ok(())
}
fn prepare_update(
    m: &UpdateManifest,
    root: &Path,
    install: &Path,
    tx: &Sender<Event>,
) -> Result<(Pending, bool)> {
    validate_update(m)?;
    let update_root = root.join("updates");
    create_dir(&update_root)?;
    let stage_name = format!("stage-{}", uuid::Uuid::new_v4());
    let stage = update_root.join(&stage_name);
    create_dir(&stage)?;
    let base_sha = hash_file(&install.join(PACKAGE)).ok();
    let delta = base_sha.and_then(|sha| m.deltas.iter().find(|d| d.from_manifest_sha256 == sha));
    let mut used_delta = false;
    if let Some(delta) = delta {
        let attempt = (|| {
            for file in &m.package.files {
                if let Ok(source) = checked_source(install, &file.path) {
                    if fs::metadata(&source).map_err(|e| e.to_string())?.len() == file.size
                        && hash_file(&source)? == file.sha256
                    {
                        let target = stage.join(&file.path);
                        create_dir(target.parent().unwrap())?;
                        fs::copy(source, target).map_err(|e| e.to_string())?;
                    }
                }
            }
            let zip = update_root.join(format!("delta-{}.zip", uuid::Uuid::new_v4()));
            let result = download_archive(&delta.archive, &zip, tx)
                .and_then(|_| extract_package(&zip, &stage, &m.package))
                .and_then(|_| verify_files(&m.package, &stage));
            let _ = fs::remove_file(zip);
            result
        })();
        used_delta = attempt.is_ok();
    }
    if !used_delta {
        let zip = update_root.join(format!("full-{}.zip", uuid::Uuid::new_v4()));
        let result = download_archive(&m.full, &zip, tx)
            .and_then(|_| extract_package(&zip, &stage, &m.package))
            .and_then(|_| verify_files(&m.package, &stage));
        let _ = fs::remove_file(zip);
        result?;
    }
    let bytes = serde_json::to_vec_pretty(&m.package).map_err(|e| e.to_string())?;
    atomic_write(&stage.join(PACKAGE), &bytes)?;
    let pending = Pending {
        version: m.version.clone(),
        stage: stage_name,
        manifest_sha256: digest(&bytes),
    };
    atomic_write(
        &update_root.join("pending.json"),
        &serde_json::to_vec(&pending).unwrap(),
    )?;
    Ok((pending, used_delta))
}
fn read_pending(root: &Path) -> Result<Option<Pending>> {
    let file = root.join("updates/pending.json");
    if !file.exists() {
        return Ok(None);
    }
    let pending: Pending = serde_json::from_slice(&fs::read(file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if !pending.stage.starts_with("stage-")
        || uuid::Uuid::parse_str(&pending.stage[6..]).is_err()
        || !sha_valid(&pending.manifest_sha256)
        || beta_version(&pending.version).is_none()
    {
        return Err("待安装更新记录无效。".into());
    }
    Ok(Some(pending))
}
fn pending_package(root: &Path) -> Result<(Pending, PathBuf, PackageManifest)> {
    let pending = read_pending(root)?.ok_or("没有待安装的更新。")?;
    let stage = root.join("updates").join(&pending.stage);
    let bytes = fs::read(stage.join(PACKAGE)).map_err(|e| e.to_string())?;
    if digest(&bytes) != pending.manifest_sha256 {
        return Err("待安装清单校验失败。".into());
    }
    let package: PackageManifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if package.version != pending.version {
        return Err("待安装版本不一致。".into());
    }
    verify_files(&package, &stage)?;
    Ok((pending, stage, package))
}
fn command(exe: &Path) -> Command {
    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
#[cfg(windows)]
fn other_app_window(exe: &Path) -> bool {
    use std::ffi::c_void;
    struct Search {
        exe: String,
        pid: u32,
        found: bool,
    }
    unsafe extern "system" {
        fn EnumWindows(
            callback: unsafe extern "system" fn(*mut c_void, isize) -> i32,
            param: isize,
        ) -> i32;
        fn GetWindowThreadProcessId(window: *mut c_void, pid: *mut u32) -> u32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            process: *mut c_void,
            flags: u32,
            path: *mut u16,
            length: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    unsafe extern "system" fn visit(window: *mut c_void, param: isize) -> i32 {
        let state = unsafe { &mut *(param as *mut Search) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if pid == 0 || pid == state.pid {
            return 1;
        }
        let process = unsafe { OpenProcess(0x1000, 0, pid) };
        if process.is_null() {
            return 1;
        }
        let mut name = vec![0u16; 32768];
        let mut length = name.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut length) };
        unsafe { CloseHandle(process) };
        if ok != 0 {
            let text = String::from_utf16_lossy(&name[..length as usize]).to_lowercase();
            if text.trim_start_matches("\\\\?\\") == state.exe {
                state.found = true;
                return 0;
            }
        }
        1
    }
    let path = exe
        .canonicalize()
        .unwrap_or_else(|_| exe.into())
        .to_string_lossy()
        .to_lowercase();
    let mut state = Search {
        exe: path.trim_start_matches("\\\\?\\").into(),
        pid: std::process::id(),
        found: false,
    };
    unsafe { EnumWindows(visit, &mut state as *mut Search as isize) };
    state.found
}
#[cfg(not(windows))]
fn other_app_window(_exe: &Path) -> bool {
    false
}
#[cfg(windows)]
struct ApplyLock(*mut std::ffi::c_void);
#[cfg(windows)]
impl Drop for ApplyLock {
    fn drop(&mut self) {
        unsafe extern "system" {
            fn ReleaseMutex(handle: *mut std::ffi::c_void) -> i32;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
fn apply_lock(install: &Path) -> Result<ApplyLock> {
    unsafe extern "system" {
        fn CreateMutexW(
            attributes: *const std::ffi::c_void,
            owner: i32,
            name: *const u16,
        ) -> *mut std::ffi::c_void;
        fn GetLastError() -> u32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    let path = install
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_lowercase();
    let name: Vec<u16> = format!("Local\\JadeRustApply_{}", digest(path.as_bytes()))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
    let error = unsafe { GetLastError() };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if error == 183 {
        unsafe { CloseHandle(handle) };
        return Err("另一个 Rust 更新安装仍在进行，请等待完成。".into());
    }
    Ok(ApplyLock(handle))
}
#[cfg(not(windows))]
fn apply_lock(_install: &Path) -> Result<()> {
    Ok(())
}
fn begin_install(root: &Path, exe: &Path) -> Result<()> {
    if other_app_window(exe) {
        return Err("请先关闭其它 Rust 测试版窗口，再安装更新。正式版窗口不受影响。".into());
    }
    pending_package(root)?;
    let install = exe
        .parent()
        .ok_or("程序安装目录无效")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if exe.file_name().and_then(|v| v.to_str()) != Some(EXE) || !install.join(PACKAGE).exists() {
        return Err("请从完整 Rust 测试版程序包运行更新。".into());
    }
    let helper = root
        .join("updates")
        .join(format!("helper-{}.exe", uuid::Uuid::new_v4()));
    fs::copy(exe, &helper).map_err(|e| e.to_string())?;
    if hash_file(&helper)? != hash_file(exe)? {
        return Err("安装助手校验失败。".into());
    }
    command(&helper)
        .arg("--apply-rust-update")
        .arg(root)
        .arg(&install)
        .arg(std::process::id().to_string())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Call before opening the UI. true means the helper was launched and this
/// process must exit normally; the helper waits for this process to finish.
pub fn apply_pending_on_start(root: &Path, exe: &Path) -> Result<bool> {
    if std::env::var_os("JADE_SKIP_PENDING_ONCE").is_some() {
        return Ok(false);
    }
    if read_pending(root)?.is_none() {
        return Ok(false);
    }
    begin_install(root, exe)?;
    Ok(true)
}
/// Call once AFTER the new version has displayed a usable first frame.
pub fn mark_ready() {
    if let Some(path) = std::env::var_os("JADE_UPDATE_ACK") {
        let path = PathBuf::from(path);
        if path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|s| s.starts_with("ready-") && s.ends_with(".json"))
        {
            let _ = atomic_write(
                &path,
                &serde_json::to_vec(&json!({"version":VERSION,"ready":true})).unwrap(),
            );
        }
    }
}

fn wait_parent(pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
            fn WaitForSingleObject(handle: *mut std::ffi::c_void, ms: u32) -> u32;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        let h = unsafe { OpenProcess(0x00100000, 0, pid) };
        if h.is_null() {
            return Ok(());
        }
        let result = unsafe { WaitForSingleObject(h, 30000) };
        unsafe { CloseHandle(h) };
        if result != 0 {
            return Err("原窗口未能及时退出，更新已保留。".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct BackupEntry {
    path: String,
    existed: bool,
}
fn install_transaction(
    stage: &Path,
    package: &PackageManifest,
    install: &Path,
    backup: &Path,
) -> Result<Vec<BackupEntry>> {
    verify_files(package, stage)?;
    create_dir(backup)?;
    let mut changed: Vec<BackupEntry> = Vec::new();
    let names: Vec<_> = package
        .files
        .iter()
        .map(|f| f.path.clone())
        .chain(std::iter::once(PACKAGE.into()))
        .collect();
    let mut unchanged = HashSet::new();
    // Inspect every path before changing the first file. Reusing unchanged
    // images keeps a small patch cheap to install, including on slower disks.
    for name in &names {
        checked_destination(install, name)?;
        if let Some(expected) = package.files.iter().find(|f| f.path == *name) {
            if let Ok(current) = checked_source(install, name) {
                if fs::metadata(&current).map_err(|e| e.to_string())?.len() == expected.size
                    && hash_file(&current)? == expected.sha256
                {
                    unchanged.insert(name.clone());
                }
            }
        }
    }
    for name in names {
        if unchanged.contains(&name) {
            continue;
        }
        let relative = safe_relative(&name)?;
        let target = checked_destination(install, &name)?;
        let saved = backup.join(&relative);
        let existed = target.exists();
        let result = (|| {
            if existed {
                let source = checked_source(install, &name)?;
                create_dir(saved.parent().unwrap())?;
                fs::copy(source, &saved).map_err(|e| e.to_string())?;
            }
            let record = BackupEntry {
                path: name.clone(),
                existed,
            };
            changed.push(record);
            atomic_write(
                &backup.join("transaction.json"),
                &serde_json::to_vec(&changed).unwrap(),
            )?;
            create_dir(target.parent().unwrap())?;
            let bytes = fs::read(stage.join(&relative)).map_err(|e| e.to_string())?;
            atomic_write(&target, &bytes)
        })();
        if let Err(error) = result {
            let restored = rollback(install, backup, &changed);
            return Err(format!(
                "安装失败：{error}{}",
                if restored.is_ok() {
                    "，已还原旧版本。".to_string()
                } else {
                    format!(
                        "；自动还原未完成：{}。备份位于 {}",
                        restored.unwrap_err(),
                        backup.display()
                    )
                }
            ));
        }
    }
    Ok(changed)
}
fn rollback(install: &Path, backup: &Path, changed: &[BackupEntry]) -> Result<()> {
    let mut errors = Vec::new();
    for entry in changed.iter().rev() {
        let path = safe_relative(&entry.path)?;
        let target = install.join(&path);
        let result = if entry.existed {
            fs::read(backup.join(&path))
                .map_err(|e| e.to_string())
                .and_then(|b| atomic_write(&target, &b))
        } else if target.exists() {
            fs::remove_file(&target).map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        if let Err(e) = result {
            errors.push(e)
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}
fn run_install(root: &Path, install: &Path, pid: u32) -> Result<()> {
    if !root.is_absolute() || !install.is_absolute() || !install.join(PACKAGE).exists() {
        return Err("安装位置无效。".into());
    }
    let _lock = apply_lock(install)?;
    let (pending, stage, package) = pending_package(root)?;
    wait_parent(pid)?;
    if other_app_window(&install.join(EXE)) {
        return Err("请先关闭其它 Rust 测试版窗口，再安装更新。更新文件已保留。".into());
    }
    let backup = root
        .join("updates")
        .join(format!("rollback-{}", uuid::Uuid::new_v4()));
    let changed = install_transaction(&stage, &package, install, &backup)?;
    let ack = root
        .join("updates")
        .join(format!("ready-{}.json", uuid::Uuid::new_v4()));
    let launch = command(&install.join(EXE))
        .arg("--data-dir")
        .arg(root)
        .env("JADE_SKIP_PENDING_ONCE", "1")
        .env("JADE_UPDATE_ACK", &ack)
        .spawn();
    let mut ready = false;
    let mut launch_error = String::new();
    match launch {
        Ok(mut child) => {
            let deadline = Instant::now() + Duration::from_secs(45);
            while Instant::now() < deadline {
                if let Ok(bytes) = fs::read(&ack) {
                    if serde_json::from_slice::<Value>(&bytes)
                        .is_ok_and(|v| v["ready"] == true && v["version"] == pending.version)
                    {
                        ready = true;
                        break;
                    }
                }
                if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !ready {
                let _ = child.kill();
                let _ = child.wait();
                launch_error = "新版未能显示可用界面".into();
            }
        }
        Err(e) => launch_error = e.to_string(),
    }
    let _ = fs::remove_file(&ack);
    if ready {
        fs::remove_file(root.join("updates/pending.json")).map_err(|e| e.to_string())?;
        atomic_write(
            &root.join("updates/result.json"),
            &serde_json::to_vec(
                &json!({"message":format!("已安装 Rust 测试版 {}。",pending.version)}),
            )
            .unwrap(),
        )?;
        return Ok(());
    }
    rollback(install, &backup, &changed)?;
    let rejected = root
        .join("updates")
        .join(format!("rejected-{}.json", uuid::Uuid::new_v4()));
    let _ = fs::rename(root.join("updates/pending.json"), rejected);
    let message = format!(
        "{launch_error}，已自动还原旧版本。更新备份保存在 {}。",
        backup.display()
    );
    atomic_write(
        &root.join("updates/result.json"),
        &serde_json::to_vec(&json!({"message":message})).unwrap(),
    )?;
    command(&install.join(EXE))
        .arg("--data-dir")
        .arg(root)
        .env("JADE_SKIP_PENDING_ONCE", "1")
        .env_remove("JADE_UPDATE_ACK")
        .spawn()
        .map_err(|e| e.to_string())?;
    Err(message)
}
/// Pass std::env::args().skip(1).collect(); Some is an updater-only process.
pub fn handle_cli(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("--apply-rust-update") {
        return None;
    }
    if args.len() != 4 {
        return Some(2);
    }
    let Ok(pid) = args[3].parse() else {
        return Some(2);
    };
    let root = PathBuf::from(&args[1]);
    let install = PathBuf::from(&args[2]);
    match run_install(&root, &install, pid) {
        Ok(()) => Some(0),
        Err(e) => {
            let _ = atomic_write(
                &root.join("updates/result.json"),
                &serde_json::to_vec(&json!({"message":e})).unwrap(),
            );
            Some(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("jade-rust-update-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn file(path: &str, bytes: &[u8]) -> PackageFile {
        PackageFile {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: digest(bytes),
        }
    }
    fn package(files: Vec<PackageFile>) -> PackageManifest {
        PackageManifest {
            schema: 1,
            version: "2.0.0-beta.2".into(),
            files,
        }
    }
    #[test]
    fn local_notice_seed_waits_for_data_and_preserves_current_version() {
        let root = dir();
        let mut updates = UpdateState::new(root.clone(), root.join(EXE));
        let current = format!("rust-release-{VERSION}");
        updates.seed_local_notices(&Value::Null);
        assert!(!updates.local_notices_seeded);
        let old = json!({"id":"previous","title":"旧版本公告","body":"本地历史内容","popup":true});
        updates.seed_local_notices(&json!({"announcements":[old.clone(),old]}));
        assert!(updates.local_notices_seeded);
        assert_eq!(updates.notices.len(), 2);
        assert_eq!(updates.notices[0].id, current);
        assert_eq!(updates.notices[1].id, "previous");
        updates.seed_local_notices(
            &json!({"announcements":[{"id":"other","title":"另一个公告","body":"不会重复加载"}]}),
        );
        assert_eq!(updates.notices.len(), 2);
        assert_eq!(updates.notices[1].id, "previous");

        let mut existing = UpdateState::new(root.clone(), root.join(EXE));
        existing.seed_local_notices(&json!({"announcements":[
            {"id":current,"title":"随包本版公告","body":"完整的随包正文","publishedAt":"2026-11-11T12:00:00Z"},
            {"id":format!("rust-release-{VERSION}"),"title":"重复本版","body":"不应覆盖第一条"}
        ]}));
        assert_eq!(existing.notices.len(), 1);
        assert_eq!(existing.notices[0].title, "随包本版公告");
        assert_eq!(existing.notices[0].body, "完整的随包正文");
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn remote_notice_response_is_not_overwritten_by_late_local_data() {
        let root = dir();
        let (mut updates, mut state) = notice_ui_state(&root);
        state["announcementPopups"] = json!(false);
        let remote = Notice {
            id: "remote-newest".into(),
            title: "网络新公告".into(),
            body: "远端最新内容".into(),
            published_at: "2026-11-11T12:00:00Z".into(),
            popup: true,
        };
        let (sender, receiver) = mpsc::channel();
        updates.channel = Some(receiver);
        sender
            .send(Event::Checked(Ok(None), vec![remote.clone(), remote]))
            .unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            updates.tick(ctx, &mut state);
        });
        assert!(updates.remote_notices_loaded);
        assert_eq!(updates.notices.len(), 1);
        assert_eq!(updates.notices[0].id, "remote-newest");
        updates.seed_local_notices(&json!({"announcements":[{"id":"bundled-old","title":"旧本地公告","body":"不得覆盖网络最新公告"}]}));
        assert_eq!(updates.notices.len(), 1);
        assert_eq!(updates.notices[0].id, "remote-newest");
        assert_eq!(updates.notices[0].body, "远端最新内容");
        let _ = fs::remove_dir_all(root);
    }
    fn notice_ui_state(root: &Path) -> (UpdateState, Value) {
        let mut updates = UpdateState::new(root.to_path_buf(), root.join(EXE));
        updates.checked = true;
        updates.notices = vec![
            Notice {
                id: "new".into(),
                title: "本次更新公告".into(),
                body: "第一段公告内容。\n\n第二段保留换行。".into(),
                published_at: "2026-11-11T12:00:00Z".into(),
                popup: true,
            },
            Notice {
                id: "old".into(),
                title: "上一次更新公告".into(),
                body: "此前版本说明。".into(),
                published_at: "2026-11-10T12:00:00Z".into(),
                popup: true,
            },
        ];
        (
            updates,
            json!({"autoCheckUpdates":false,"autoDownload":false,"readAnnouncements":[]}),
        )
    }
    fn notice_ui_frame(
        ctx: &egui::Context,
        updates: &mut UpdateState,
        state: &mut Value,
        events: Vec<egui::Event>,
        size: egui::Vec2,
    ) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    updates.show(ui, state);
                });
            },
        );
    }
    fn notice_ui_click(
        ctx: &egui::Context,
        updates: &mut UpdateState,
        state: &mut Value,
        key: &str,
        size: egui::Vec2,
    ) {
        for _ in 0..2 {
            notice_ui_frame(ctx, updates, state, vec![], size);
        }
        let rect = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(format!("update-control:{key}"))))
            .unwrap_or_else(|| panic!("missing notice control {key}"));
        assert!(rect.is_positive(), "invisible control {key}");
        let pos = rect.center();
        for pressed in [true, false] {
            notice_ui_frame(
                ctx,
                updates,
                state,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                size,
            );
        }
    }
    #[test]
    fn notice_center_native_controls_open_read_reopen_and_close_popup() {
        let root = dir();
        let (mut updates, mut state) = notice_ui_state(&root);
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        crate::design::apply(&ctx, "moon");
        let size = egui::vec2(1200., 800.);
        notice_ui_click(&ctx, &mut updates, &mut state, "notice-heading:new", size);
        assert!(updates.open_notices.contains("new"));
        assert!(notice_read(&state, "new"));
        assert_eq!(updates.unread_count(&state), 1);
        notice_ui_click(&ctx, &mut updates, &mut state, "notice-popup:new", size);
        assert_eq!(updates.popup.as_ref().map(|n| n.id.as_str()), Some("new"));
        notice_ui_click(&ctx, &mut updates, &mut state, "notice-close", size);
        assert!(updates.popup.is_none());
        notice_ui_click(&ctx, &mut updates, &mut state, "notice-popup:new", size);
        assert!(updates.popup.is_some());
        notice_ui_click(&ctx, &mut updates, &mut state, "notice-confirm", size);
        assert!(updates.popup.is_none());
        notice_ui_click(&ctx, &mut updates, &mut state, "notice-read-all", size);
        assert_eq!(updates.unread_count(&state), 0);
        assert_eq!(state["readAnnouncements"].as_array().unwrap().len(), 2);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn announcement_popup_preference_and_automatic_read_receipt_are_preserved() {
        let root = dir();
        let (mut updates, mut state) = notice_ui_state(&root);
        state["announcementPopups"] = json!(false);
        updates.queue_announcement(&mut state);
        assert!(updates.popup.is_none());
        assert_eq!(updates.unread_count(&state), 2);
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        crate::design::apply(&ctx, "moon");
        notice_ui_click(
            &ctx,
            &mut updates,
            &mut state,
            "notice-popup-setting",
            egui::vec2(760., 800.),
        );
        assert_eq!(state["announcementPopups"], true);
        updates.queue_announcement(&mut state);
        assert_eq!(updates.popup.as_ref().map(|n| n.id.as_str()), Some("new"));
        assert!(notice_read(&state, "new"));
        updates.popup = None;
        updates.queue_announcement(&mut state);
        assert!(
            updates.popup.is_none(),
            "one automatic notice per launch; other notices stay in the list"
        );
        assert_eq!(updates.unread_count(&state), 1);
        assert_eq!(notice_date("2026-11-11T12:00:00Z"), "2026年11月11日");
        assert!(notice_date("").is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn update_settings_and_busy_buttons_respond_to_native_input() {
        let root = dir();
        let (mut updates, mut state) = notice_ui_state(&root);
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        crate::design::apply(&ctx, "moon");
        let size = egui::vec2(1200., 800.);
        notice_ui_click(&ctx, &mut updates, &mut state, "update-auto-check", size);
        assert_eq!(state["autoCheckUpdates"], true);
        notice_ui_click(&ctx, &mut updates, &mut state, "update-auto-download", size);
        assert_eq!(state["autoDownload"], true);
        updates.busy = true;
        notice_ui_click(&ctx, &mut updates, &mut state, "update-check", size);
        assert!(
            updates.channel.is_none(),
            "busy controls must not start another network request"
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn paths_and_channel_are_strict() {
        for bad in [
            "../x", "/x", "C:/x", "a\\b", "a/../b", "a:", "NUL.txt", "a./b", "a//b",
        ] {
            assert!(safe_relative(bad).is_err(), "{bad}")
        }
        assert!(safe_relative("data/assets/moon.webp").is_ok());
        assert!(beta_version("2.0.0-beta.2") > beta_version("2.0.0-beta.1"));
        assert!(beta_version("1.0.18").is_none());
    }
    #[test]
    fn copies_verified_data_and_rolls_back() {
        let root = dir();
        let old = root.join("old");
        let stage = root.join("stage");
        let backup = root.join("backup");
        create_dir(&old).unwrap();
        create_dir(&stage).unwrap();
        fs::write(old.join(EXE), b"old").unwrap();
        fs::write(old.join(PACKAGE), b"old manifest").unwrap();
        fs::write(stage.join(EXE), b"new").unwrap();
        fs::write(stage.join("data.json"), b"data").unwrap();
        let p = package(vec![file(EXE, b"new"), file("data.json", b"data")]);
        fs::write(stage.join(PACKAGE), serde_json::to_vec_pretty(&p).unwrap()).unwrap();
        let changes = install_transaction(&stage, &p, &old, &backup).unwrap();
        assert_eq!(fs::read(old.join(EXE)).unwrap(), b"new");
        rollback(&old, &backup, &changes).unwrap();
        assert_eq!(fs::read(old.join(EXE)).unwrap(), b"old");
        assert_eq!(fs::read(old.join(PACKAGE)).unwrap(), b"old manifest");
        assert!(!old.join("data.json").exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn corrupted_stage_does_not_touch_install() {
        let root = dir();
        let install = root.join("install");
        let stage = root.join("stage");
        create_dir(&install).unwrap();
        create_dir(&stage).unwrap();
        fs::write(install.join(EXE), b"old").unwrap();
        fs::write(stage.join(EXE), b"bad").unwrap();
        let p = package(vec![file(EXE, b"good")]);
        assert!(install_transaction(&stage, &p, &install, &root.join("backup")).is_err());
        assert_eq!(fs::read(install.join(EXE)).unwrap(), b"old");
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn pending_requires_exact_stage_identity() {
        let root = dir();
        create_dir(&root.join("updates")).unwrap();
        fs::write(
            root.join("updates/pending.json"),
            serde_json::to_vec(&Pending {
                version: "2.0.0-beta.2".into(),
                stage: "../../outside".into(),
                manifest_sha256: "0".repeat(64),
            })
            .unwrap(),
        )
        .unwrap();
        assert!(read_pending(&root).is_err());
        let _ = fs::remove_dir_all(root);
    }
    fn zip_file(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    #[test]
    fn delta_reuses_verified_existing_files() {
        let root = dir();
        let stage = root.join("stage");
        create_dir(&stage).unwrap();
        fs::write(stage.join("data.json"), b"same-data").unwrap();
        let p = package(vec![file(EXE, b"new-exe"), file("data.json", b"same-data")]);
        let archive = root.join("delta.zip");
        zip_file(&archive, &[(EXE, b"new-exe")]);
        extract_package(&archive, &stage, &p).unwrap();
        verify_files(&p, &stage).unwrap();
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn archive_traversal_and_bad_hash_are_rejected() {
        let root = dir();
        let stage = root.join("stage");
        let p = package(vec![file(EXE, b"new")]);
        let traversal = root.join("traversal.zip");
        zip_file(&traversal, &[("../escaped.txt", b"bad")]);
        assert!(extract_package(&traversal, &stage, &p).is_err());
        assert!(!root.join("escaped.txt").exists());
        let damaged = root.join("damaged.zip");
        zip_file(&damaged, &[(EXE, b"bad")]);
        assert!(extract_package(&damaged, &stage, &p).is_err());
        assert!(!stage.join(EXE).exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn unchanged_files_are_not_replaced() {
        let root = dir();
        let install = root.join("install");
        let stage = root.join("stage");
        create_dir(&install).unwrap();
        create_dir(&stage).unwrap();
        for directory in [&install, &stage] {
            fs::write(directory.join(EXE), b"same-exe").unwrap();
        }
        fs::write(install.join(PACKAGE), b"old-manifest").unwrap();
        let p = package(vec![file(EXE, b"same-exe")]);
        fs::write(stage.join(PACKAGE), serde_json::to_vec(&p).unwrap()).unwrap();
        let changed = install_transaction(&stage, &p, &install, &root.join("backup")).unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].path, PACKAGE);
        let _ = fs::remove_dir_all(root);
    }
    #[cfg(windows)]
    #[test]
    fn install_mutex_is_scoped_to_one_installation() {
        let first = dir();
        let second = dir();
        let guard = apply_lock(&first).unwrap();
        assert!(apply_lock(&first).is_err());
        assert!(apply_lock(&second).is_ok());
        drop(guard);
        assert!(apply_lock(&first).is_ok());
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }
}
