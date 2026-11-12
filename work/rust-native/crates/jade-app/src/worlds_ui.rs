use crate::{
    data::{Data, array, text},
    store::{Store, unique_name, valid_uuid},
};
use eframe::egui;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
};

type TaskResult = Result<(String, Value, Value), String>;
#[derive(Clone, Default)]
struct ReadContext {
    target: Option<String>,
    prior_owner: Option<String>,
    force_owner: bool,
    started: u64,
}
#[derive(Default, Clone)]
struct GameNames {
    species: HashMap<String, (String, String)>,
    traits: HashMap<String, String>,
    names: HashMap<String, String>,
}
impl GameNames {
    fn from_tables(species: &[Value], traits: &Value) -> Self {
        Self {
            species: species
                .iter()
                .map(|s| {
                    (
                        text(s, "key").to_lowercase(),
                        (text(s, "key").to_owned(), text(s, "zh").to_owned()),
                    )
                })
                .collect(),
            traits: array(&traits["traits"])
                .iter()
                .map(|t| (text(t, "key").to_lowercase(), text(t, "key").to_owned()))
                .collect(),
            names: traits["terminalNames"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.to_lowercase(), v.to_owned())))
                .collect(),
        }
    }
    fn species(&self, key: &str) -> Option<&(String, String)> {
        let source = key.to_lowercase();
        let mut base = source.as_str();
        while let Some((prefix, tail)) = base.split_once('_') {
            if [
                "boss", "raid", "predator", "gym", "tower", "summon", "oilrig",
            ]
            .contains(&prefix)
            {
                base = tail;
            } else {
                break;
            }
        }
        let suffix = [
            "_quest_friend",
            "_quest_enemy",
            "_otomo",
            "_bossrush",
            "_quest",
            "_tower",
        ]
        .iter()
        .find_map(|suffix| base.strip_suffix(suffix))
        .unwrap_or(base);
        self.species
            .get(&source)
            .or_else(|| self.species.get(base))
            .or_else(|| self.species.get(suffix))
    }
    fn record(&self, game: &str, owner: &str, row: &Value) -> Result<Value, String> {
        let uuid = text(row, "instanceId").replace('-', "").to_lowercase();
        let raw = text(row, "speciesKey");
        let traits = row["traits"].as_array().ok_or("终端词条格式无效。")?;
        if !valid_uuid(&uuid)
            || raw.is_empty()
            || raw.len() > 160
            || raw.chars().any(char::is_control)
            || !row["name"]
                .as_str()
                .is_some_and(|n| n.chars().count() <= 256)
            || !matches!(row["sex"].as_str(), Some("M" | "F" | "unknown"))
            || traits.len() > 4
        {
            return Err("终端个体资料不完整，未修改仓库。".into());
        }
        let mut keys = vec![];
        for t in traits {
            let t = t
                .as_str()
                .filter(|t| !t.is_empty() && t.len() <= 160 && !t.chars().any(char::is_control))
                .ok_or("终端词条编号无效。")?;
            if t.eq_ignore_ascii_case("none") {
                continue;
            }
            let key = self
                .traits
                .get(&t.to_lowercase())
                .cloned()
                .unwrap_or_else(|| t.to_owned());
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        let species = self.species(raw);
        let name = text(row, "name");
        let fallback = self
            .names
            .get(&raw.to_lowercase())
            .map(String::as_str)
            .or_else(|| species.map(|p| p.1.as_str()))
            .unwrap_or(raw);
        Ok(
            json!({"id":format!("terminal-{game}-{uuid}"),"speciesKey":species.map(|s|s.0.as_str()).unwrap_or(raw),"name":if name.is_empty(){fallback}else{name},"sex":row["sex"],"traits":keys,"legacy":false,"terminalOwnerId":owner,"terminalSpeciesKey":raw}),
        )
    }
}
#[derive(Default)]
pub struct Worlds {
    pub open: bool,
    pub busy: bool,
    pub message: String,
    games: Vec<Value>,
    new_name: String,
    result: Option<(Value, Value)>,
    owner: String,
    receiver: Option<mpsc::Receiver<TaskResult>>,
    context: ReadContext,
    names: GameNames,
    selected_directory: Option<PathBuf>,
    default_directory: Option<PathBuf>,
    scan_warnings: Vec<String>,
    fallback_pending: bool,
    import_preview: Option<Value>,
    rename: Option<(String, String)>,
    delete: Option<String>,
    unlink: Option<String>,
    export_current: bool,
    was_open: bool,
    creating: bool,
    selected_game: String,
    pub new_window_requested: bool,
}
fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn owner_of(w: &Value) -> Option<String> {
    w["terminalOwner"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn chosen_owner(result: &Value, remembered: Option<&str>, force: bool) -> Option<String> {
    if force {
        return None;
    }
    let owners = array(&result["owners"]);
    if let Some(owner) = remembered.filter(|id| owners.iter().any(|o| text(o, "id") == *id)) {
        return Some(owner.to_owned());
    }
    let local: Vec<_> = owners.iter().filter(|o| o["isLocal"] == true).collect();
    if local.len() == 1 {
        Some(text(local[0], "id").to_owned())
    } else {
        None
    }
}
impl Worlds {
    fn task<F: FnOnce() -> TaskResult + Send + 'static>(&mut self, ctx: &egui::Context, f: F) {
        if self.busy {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.busy = true;
        self.result = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
                .unwrap_or_else(|_| Err("读取时发生错误，请保存游戏后重试。".into()));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    pub fn scan(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        if self.busy {
            return;
        }
        self.selected_directory = path.clone();
        self.message = "正在查找游戏世界…".into();
        self.task(ctx, move || {
            let found = jade_saves::scan_game_saves(path.as_deref())?;
            Ok(("scan".into(), json!(found), Value::Null))
        });
    }
    pub fn read(&mut self, ctx: &egui::Context, world: Value) {
        if self.busy {
            return;
        }
        self.context = ReadContext {
            started: timestamp(),
            ..Default::default()
        };
        self.start_read(ctx, world, false);
    }
    fn start_read(&mut self, ctx: &egui::Context, world: Value, rescan: bool) {
        self.message = "正在读取终端与携带队伍…".into();
        self.task(ctx, move || {
            let mut parsed: jade_saves::GameWorld =
                serde_json::from_value(world).map_err(|e| e.to_string())?;
            if rescan {
                let scan = jade_saves::scan_game_saves(Some(&parsed.path))?;
                parsed = scan
                    .saves
                    .into_iter()
                    .find(|g| g.id == parsed.id)
                    .ok_or("没有找到原关联世界，请确认游戏已保存，或重新选择存档目录。")?;
            }
            let result = jade_saves::read_terminal(&parsed)?;
            Ok(("terminal".into(), json!(result), json!(parsed)))
        });
    }
    fn read_linked(
        &mut self,
        ctx: &egui::Context,
        game: Value,
        store: &Store,
        target: Option<String>,
        force_owner: bool,
    ) {
        if self.busy {
            return;
        }
        let linked = target
            .as_deref()
            .and_then(|id| array(&store.state["worlds"]).iter().find(|w| w["id"] == id))
            .or_else(|| {
                array(&store.state["worlds"])
                    .iter()
                    .find(|w| w["nativeGame"]["id"] == game["id"])
            });
        self.context = ReadContext {
            target: linked.map(|w| text(w, "id").to_owned()),
            prior_owner: linked.and_then(owner_of),
            force_owner,
            started: timestamp(),
        };
        self.start_read(ctx, game, true);
    }
    pub fn refresh(&mut self, ctx: &egui::Context, store: &Store) {
        let game = store.world()["nativeGame"].clone();
        if game.is_object() {
            self.read_linked(
                ctx,
                game,
                store,
                Some(text(store.world(), "id").to_owned()),
                false,
            );
        } else {
            self.open = true;
            self.scan(
                ctx,
                store.state["lastSaveDirectory"].as_str().map(PathBuf::from),
            );
            self.message = "请先关联游戏存档；以后点击刷新即可自动更新终端和携带队伍。".into();
        }
    }
    pub fn tick(&mut self, store: &mut Store) {
        let result = match self.receiver.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(v)) => Some(v),
            Some(Err(mpsc::TryRecvError::Disconnected)) => Some(Err("读取任务已中断。".into())),
            _ => None,
        };
        let Some(result) = result else {
            return;
        };
        self.receiver = None;
        self.busy = false;
        match result {
            Err(e) => self.message = format!("读取未完成，现有仓库已保留：{e}"),
            Ok((kind, value, game)) => {
                if kind == "scan" {
                    self.games = array(&value["saves"]).to_vec();
                    self.scan_warnings = array(&value["warnings"])
                        .iter()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .collect();
                    self.default_directory = value["defaultDirectory"].as_str().map(PathBuf::from);
                    self.fallback_pending = self.open
                        && self.games.is_empty()
                        && value["defaultDirectoryExists"] == true
                        && self.selected_directory.is_none();
                    self.message = if self.games.is_empty() {
                        "未找到可读取的完整世界，可选择游戏存档文件夹；联机客户端需取得主机的完整世界。".into()
                    } else {
                        format!("发现 {} 个游戏世界", self.games.len())
                    };
                    if !self.games.is_empty() {
                        if let Some(path) = self.selected_directory.take() {
                            let mut next = store.state.clone();
                            next["lastSaveDirectory"] = json!(path);
                            if let Err(e) = store.commit(next) {
                                self.message.push_str(&format!("；路径未能记住：{e}"));
                            }
                        }
                    }
                } else {
                    let target = self
                        .context
                        .target
                        .as_deref()
                        .and_then(|id| array(&store.state["worlds"]).iter().find(|w| w["id"] == id))
                        .or_else(|| {
                            array(&store.state["worlds"])
                                .iter()
                                .find(|w| w["nativeGame"]["id"] == game["id"])
                        });
                    let remembered = target.and_then(owner_of);
                    let selected =
                        chosen_owner(&value, remembered.as_deref(), self.context.force_owner);
                    if let Some(owner) = selected {
                        self.finish_import(store, &game, &value, &owner);
                    } else if array(&value["owners"]).is_empty() {
                        self.message = "存档中没有可确认的玩家，现有仓库未修改。".into();
                    } else {
                        self.owner = remembered
                            .filter(|id| {
                                array(&value["owners"]).iter().any(|o| text(o, "id") == id)
                            })
                            .unwrap_or_default();
                        self.result = Some((game, value));
                        self.open = true;
                    }
                }
            }
        }
    }
    fn finish_import(&mut self, store: &mut Store, game: &Value, result: &Value, owner: &str) {
        match import_terminal(store, game, result, owner, &self.context, &self.names) {
            Ok(count) => {
                self.message =
                    format!("已刷新 {count} 只帕鲁，包含终端与携带队伍；不再持有的个体已归档。");
                for warning in array(&result["warnings"]) {
                    if let Some(w) = warning.as_str() {
                        self.message.push_str(&format!("\n{w}"));
                    }
                }
                self.result = None;
            }
            Err(e) => self.message = format!("仓库未修改：{e}"),
        }
    }
    fn pick_directory(&mut self, ctx: &egui::Context, store: &Store) {
        let mut dialog = rfd::FileDialog::new();
        if let Some(path) = store.state["lastSaveDirectory"]
            .as_str()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .or_else(|| self.default_directory.clone().filter(|p| p.is_dir()))
        {
            dialog = dialog.set_directory(path);
        }
        if let Some(path) = dialog.pick_folder() {
            self.scan(ctx, Some(path));
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, store: &mut Store, d: &Data) {
        if self.names.species.is_empty() {
            self.names = GameNames::from_tables(&d.species, d.get("trait-data"));
        }
        if !self.open {
            self.was_open = false;
            return;
        }
        if !self.was_open {
            self.was_open = true;
            if !self.busy && self.result.is_none() && std::env::var_os("JADE_QA_OUTPUT").is_none() {
                self.scan(
                    ctx,
                    store.state["lastSaveDirectory"].as_str().map(PathBuf::from),
                );
            }
        }
        if self.fallback_pending && !self.busy {
            self.fallback_pending = false;
            self.pick_directory(ctx, store);
        }
        let palette = crate::design::palette(ctx);
        let frame = egui::Frame::new()
            .fill(palette.surface)
            .stroke(egui::Stroke::new(1.0_f32, palette.line))
            .corner_radius(14)
            .inner_margin(egui::Margin::symmetric(22, 17));
        let modal=egui::Modal::new(egui::Id::new("world-archives-modal")).frame(frame)
            .backdrop_color(egui::Color32::from_rgba_unmultiplied(30,28,55,104)).show(ctx,|ui|{
            ui.set_width((ctx.content_rect().width()-70.).min(521.).max(280.));
            ui.set_max_height((ctx.content_rect().height()*0.90-34.).max(300.));
            ui.spacing_mut().item_spacing=egui::Vec2::new(8.,8.);
            if crate::design::dialog_header(ui,"layers","世界档案").clicked(){self.open=false;}
            ui.small("每个游戏世界都有独立的仓库、配种计划和探索记录。");ui.separator();
            egui::ScrollArea::vertical().id_salt("world-archive-body").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded).max_height((ctx.content_rect().height()*0.90-108.).max(260.)).show(ui,|ui|{
                crate::design::section(ui,"当前世界");
                let world=store.world().clone();let current=text(&world,"id").to_owned();let mut chosen=current.clone();
                ui.horizontal(|ui|{
                    ui.add_enabled_ui(!self.busy&&self.result.is_none(),|ui|{
                        egui::ComboBox::from_id_salt("archive-current-world").width((ui.available_width()-220.).max(140.)).selected_text(text(&world,"name")).show_ui(ui,|ui|{
                            for w in array(&store.state["worlds"]){ui.selectable_value(&mut chosen,text(w,"id").to_owned(),text(w,"name"));}
                        });
                    });
                    if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("plus")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"新建世界")).clicked(){self.creating=!self.creating;}
                    if crate::design::action(ui,"external","另开窗口").clicked(){self.new_window_requested=true;}
                });
                if chosen!=current {let mut next=store.state.clone();next["activeWorldId"]=json!(chosen);if let Err(e)=store.commit(next){self.message=e;}}
                ui.small(format!("仓库 {} 只 · 已归档 {} 只 · 手术台词条 {} 条 · 配种计划 {} 条",array(&world["inventory"]["records"]).len(),array(&world["inventory"]["archived"]).len(),array(&world["surgery"]).len(),array(&world["plans"]).len()));
                ui.horizontal(|ui|{
                    if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("edit")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"重命名")).clicked(){self.rename=Some((current.clone(),text(&world,"name").into()));}
                    if ui.add_enabled(!self.busy&&array(&store.state["worlds"]).len()>1,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("trash")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"删除世界")).clicked(){self.delete=Some(current.clone());}
                    ui.small(format!("{} 个自定义点位",array(&world["mapNotes"]["custom"]).len()));
                });
                if self.creating{ui.horizontal(|ui|{
                    ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("新世界名称").char_limit(256).desired_width(280.));
                    if ui.add_enabled(!self.busy&&!self.new_name.trim().is_empty(),egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("check")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"创建")).clicked(){match store.create_checked(&self.new_name){Ok(())=>{self.new_name.clear();self.creating=false;},Err(e)=>self.message=e}}
                    if ui.small_button("取消").clicked(){self.creating=false;}
                });}
                if world["nativeGame"].is_object(){
                    ui.small(format!("已关联游戏世界：{}",text(&world["nativeGame"],"name")));
                    ui.horizontal_wrapped(|ui|{
                        if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("refresh")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"刷新现有帕鲁")).clicked(){self.read_linked(ctx,world["nativeGame"].clone(),store,Some(current.clone()),false);}
                        if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("paldex")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"更换导入玩家")).clicked(){self.read_linked(ctx,world["nativeGame"].clone(),store,Some(current.clone()),true);}
                        if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("close")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"解除关联")).clicked(){self.unlink=Some(current.clone());}
                    });
                }
                ui.add_space(8.);ui.separator();crate::design::section(ui,"从游戏存档读取");
                ui.small("自动识别本机存档，沿用游戏中的世界名称，并同步自己的帕鲁终端与背包。");
                ui.horizontal_wrapped(|ui|{
                    if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("refresh")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"重新扫描")).clicked(){self.scan(ctx,store.state["lastSaveDirectory"].as_str().map(PathBuf::from));}
                    if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("search")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"扫描默认目录")).clicked(){self.scan(ctx,None);}
                    if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("book")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"选择存档文件夹")).clicked(){self.pick_directory(ctx,store);}
                    if self.busy{ui.spinner();}
                });
                if !self.message.is_empty(){ui.label(&self.message);}
                if let Some(e)=&store.error{ui.colored_label(egui::Color32::from_rgb(166,48,65),e);}
                if !self.scan_warnings.is_empty(){egui::CollapsingHeader::new("扫描提示").show(ui,|ui|{for warning in &self.scan_warnings{ui.label(warning);}});}
                if let Some(path)=store.state["lastSaveDirectory"].as_str(){ui.small(format!("存档目录：{path}"));}
                if !self.games.is_empty(){
                    if !self.games.iter().any(|g|text(g,"id")==self.selected_game){self.selected_game=text(&self.games[0],"id").into();}
                    let selected=self.games.iter().find(|g|text(g,"id")==self.selected_game).cloned();
                    egui::ComboBox::from_id_salt("archive-game-world").width(ui.available_width()-12.).selected_text(selected.as_ref().map(|g|format!("{} · {}",text(g,"name"),text(g,"platform"))).unwrap_or_default()).show_ui(ui,|ui|{
                        for g in &self.games{ui.selectable_value(&mut self.selected_game,text(g,"id").into(),format!("{} · {}",text(g,"name"),text(g,"platform")));}
                    });
                    if let Some(game)=selected{
                        ui.small(format!("保存时间：{}",text(&game,"modified"))).on_hover_text(text(&game,"path"));
                        ui.horizontal_wrapped(|ui|{
                            if ui.add_enabled(!self.busy,egui::Button::new("读取到对应世界")).clicked(){self.read_linked(ctx,game.clone(),store,None,false);}
                            let linked=array(&store.state["worlds"]).iter().any(|w|w["nativeGame"]["id"]==game["id"]);
                            if ui.add_enabled(!self.busy&&!linked,egui::Button::new("关联当前世界")).clicked(){self.read_linked(ctx,game.clone(),store,Some(current.clone()),true);}
                        });
                    }
                }else if !self.busy&&self.message.is_empty(){ui.small("未找到游戏世界时，可选择 SaveGames 文件夹或具体世界文件夹。");}
                if let Some((game,result))=self.result.clone(){
                    ui.group(|ui|{
                        ui.strong("选择你在此世界中的角色");ui.small("无法识别本人时选一次，以后刷新沿用此选择。");
                        egui::ComboBox::from_id_salt("archive-owner").width(ui.available_width()-12.).selected_text(array(&result["owners"]).iter().find(|o|text(o,"id")==self.owner).map(|o|text(o,"name")).unwrap_or("请选择你的角色")).show_ui(ui,|ui|{
                            for owner in array(&result["owners"]){let count=array(&result["records"]).iter().filter(|r|r["ownerId"]==owner["id"]).count();ui.selectable_value(&mut self.owner,text(owner,"id").to_owned(),format!("{} · {} 只",text(owner,"name"),count)).on_hover_text(text(owner,"id"));}
                        });
                        ui.horizontal(|ui|{if ui.add_enabled(!self.owner.is_empty(),egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("check")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"只导入此玩家")).clicked(){let owner=self.owner.clone();self.finish_import(store,&game,&result,&owner);}if crate::design::action(ui,"close","取消读取").clicked(){self.result=None;}});
                    });
                }
                ui.add_space(8.);ui.separator();crate::design::section(ui,"备份与恢复");
                ui.small("备份包含仓库、归档帕鲁、手术台词条、配种计划及地图标记。");
                ui.horizontal_wrapped(|ui|{
                    egui::ComboBox::from_id_salt("archive-export-scope").width(135.).selected_text(if self.export_current{"仅当前世界"}else{"全部世界"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.export_current,false,"全部世界");ui.selectable_value(&mut self.export_current,true,"仅当前世界");});
                    if crate::design::action(ui,"upload","导出备份").clicked(){if let Some(path)=rfd::FileDialog::new().set_file_name("Jade-worlds.json").add_filter("世界备份",&["json"]).save_file(){let backup=if self.export_current{store.backup_current()}else{store.backup()};self.message=match serde_json::to_vec_pretty(&backup).map_err(|e|e.to_string()).and_then(|bytes|std::fs::write(path,bytes).map_err(|e|e.to_string())){Ok(())=>"已导出世界备份。".into(),Err(e)=>e};}}
                    if ui.add_enabled(!self.busy,egui::Button::image_and_text(egui::Image::new(crate::design::icon_source("download")).fit_to_exact_size(egui::Vec2::splat(14.)).tint(palette.ink),"导入备份")).clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("世界备份",&["json"]).pick_file(){match read_backup(&path){Ok(v)=>{self.import_preview=Some(v);self.message.clear();},Err(e)=>self.message=e}}}
                });
                if let Some(preview)=self.import_preview.clone(){ui.group(|ui|{
                    ui.strong("导入预览");ui.small("现有世界会保留，同名世界会自动编号。");
                    for w in array(&preview["worlds"]){ui.label(format!("{}：{} 只可用 / {} 只归档，{} 条手术台词条，{} 条计划，{} 个自定义点位",text(w,"name"),array(&w["inventory"]["records"]).len(),array(&w["inventory"]["archived"]).len(),array(&w["surgery"]).len(),array(&w["plans"]).len(),array(&w["mapNotes"]["custom"]).len()));}
                    ui.horizontal(|ui|{if crate::design::action(ui,"download","确认导入独立副本").clicked(){match store.import(preview.clone()){Ok(n)=>{self.message=format!("已导入 {n} 个世界，原有世界保持完整。");self.import_preview=None;},Err(e)=>self.message=e}}if crate::design::action(ui,"close","取消导入").clicked(){self.import_preview=None;}});
                });}
                egui::CollapsingHeader::new("从旧版或另一台电脑迁移").show(ui,|ui|{
                    ui.label("在原来的工具箱中打开世界档案，选择全部世界并导出备份；在这里点击导入备份，核对预览后确认导入。");
                    ui.small("正式版和测试版使用独立的工具箱数据目录；此处读取游戏存档不会修改游戏文件。");
                });
            });
        });
        if modal.should_close() {
            self.open = false;
        }
        if !self.open {
            self.was_open = false;
            self.creating = false;
            self.import_preview = None;
        }
        self.confirmations(ctx, store);
    }
    fn confirmations(&mut self, ctx: &egui::Context, store: &mut Store) {
        if let Some((id, mut name)) = self.rename.clone() {
            let mut keep = true;
            let modal = egui::Modal::new(egui::Id::new("rename-world-modal")).show(ctx, |ui| {
                ui.set_width(400f32.min(ctx.content_rect().width() - 72.));
                if crate::design::dialog_header(ui, "edit", "重命名世界").clicked() {
                    keep = false;
                }
                ui.add(egui::TextEdit::singleline(&mut name).char_limit(256));
                ui.horizontal(|ui| {
                    if crate::design::action(ui, "save", "保存名称").clicked() {
                        match store.rename(&id, &name) {
                            Ok(()) => keep = false,
                            Err(e) => self.message = e,
                        }
                    }
                    if crate::design::action(ui, "close", "取消").clicked() {
                        keep = false;
                    }
                });
            });
            if modal.should_close() {
                keep = false;
            }
            self.rename = keep.then_some((id, name));
        }
        for removing in [true, false] {
            let pending = if removing {
                self.delete.clone()
            } else {
                self.unlink.clone()
            };
            if let Some(id) = pending {
                let mut keep = true;
                let modal = egui::Modal::new(egui::Id::new(if removing {"delete-world-modal"} else {"unlink-world-modal"})).show(ctx,|ui|{
                    ui.set_width(420f32.min(ctx.content_rect().width() - 72.));
                    if crate::design::dialog_header(ui, if removing {"trash"} else {"close"}, if removing {"确认删除世界"} else {"确认解除关联"}).clicked() { keep = false; }
                    let name=array(&store.state["worlds"]).iter().find(|w|w["id"]==id).map(|w|text(w,"name")).unwrap_or("");
                    ui.label(if removing {format!("删除「{name}」的工具箱仓库、计划和地图标记？游戏存档不受影响。建议先导出备份。")}
                        else {format!("解除「{name}」的自动刷新关联？现有仓库和记录仍保留。")});
                    ui.horizontal_wrapped(|ui|{
                        if crate::design::action(ui, if removing {"trash"} else {"close"}, if removing {"确认删除"} else {"确认解除"}).clicked(){
                            let result=if removing {store.remove_world(&id)} else {store.unlink(&id)};
                            match result {Ok(())=>keep=false,Err(e)=>self.message=e}
                        }
                        if crate::design::action(ui,"close","取消").clicked(){keep=false;}
                    });
                });
                if modal.should_close() {
                    keep = false;
                }
                if removing {
                    self.delete = keep.then_some(id);
                } else {
                    self.unlink = keep.then_some(id);
                }
            }
        }
    }
}
fn read_backup(path: &std::path::Path) -> Result<Value, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024 {
        return Err("备份超过 256 MB，请检查所选文件。".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let v: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("备份不是有效的 JSON：{e}"))?;
    Store::validate_import(&v)?;
    Ok(v)
}

fn import_terminal(
    store: &mut Store,
    game: &Value,
    result: &Value,
    owner: &str,
    context: &ReadContext,
    names: &GameNames,
) -> Result<usize, String> {
    if !valid_uuid(owner)
        || !array(&result["owners"])
            .iter()
            .any(|o| text(o, "id") == owner)
    {
        return Err("无法确认玩家。".into());
    }
    let owner_ids: HashSet<_> = array(&result["owners"])
        .iter()
        .map(|o| text(o, "id"))
        .collect();
    if owner_ids.len() != array(&result["owners"]).len()
        || owner_ids.iter().any(|id| !valid_uuid(id))
        || array(&result["records"])
            .iter()
            .any(|r| !owner_ids.contains(text(r, "ownerId")))
    {
        return Err("终端快照的玩家归属不完整。".into());
    }
    let game_id = text(game, "id");
    if game_id.is_empty()
        || game_id.len() > 256
        || !game_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:+-".contains(&b))
    {
        return Err("游戏世界编号无效。".into());
    }
    let selected: Vec<_> = result["records"]
        .as_array()
        .ok_or("读取结果缺少个体列表。")?
        .iter()
        .filter(|r| text(r, "ownerId") == owner)
        .collect();
    let mut incoming = HashMap::new();
    let mut order = vec![];
    for r in selected {
        let record = names.record(game_id, owner, r)?;
        let id = text(&record, "id").to_owned();
        if let Some(previous) = incoming.insert(id.clone(), record.clone()) {
            if previous != record {
                return Err("同一终端个体出现冲突记录，请等游戏保存完成后重试。".into());
            }
        } else {
            order.push(id);
        }
    }
    let mut next = store.state.clone();
    let worlds = next["worlds"].as_array_mut().unwrap();
    let index = if let Some(id) = &context.target {
        worlds
            .iter()
            .position(|w| w["id"] == *id)
            .ok_or("本次要更新的工具箱世界已删除。")?
    } else if let Some(i) = worlds
        .iter()
        .position(|w| w["nativeGame"]["id"] == game["id"])
    {
        i
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let name = unique_name(text(game, "name"), worlds);
        worlds.push(Store::new_world(&id, &name));
        worlds.len() - 1
    };
    if worlds
        .iter()
        .enumerate()
        .any(|(i, w)| i != index && w["nativeGame"]["id"] == game["id"])
    {
        return Err("这个游戏世界已关联到另一个工具箱世界。".into());
    }
    let w = &mut worlds[index];
    if w["terminalSnapshot"]
        .as_u64()
        .is_some_and(|time| time > context.started)
    {
        return Err("已有更新的读取结果，本次旧结果已丢弃。".into());
    }
    if context.target.is_some() && owner_of(w) != context.prior_owner {
        return Err("导入玩家已被其他操作修改，请重新读取。".into());
    }
    let prefix = format!("terminal-{game_id}-");
    let removed: HashSet<String> = array(&w["inventory"]["removed"])
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let old = array(&w["inventory"]["records"]).to_vec();
    let mut archive = array(&w["inventory"]["archived"]).to_vec();
    let manually_archived: HashSet<String> = archive
        .iter()
        .filter(|r| r["terminalArchived"] != true)
        .map(|r| text(r, "id").to_owned())
        .collect();
    let mut active = vec![];
    for record in old {
        let id = text(&record, "id");
        let tracked = id.starts_with(&prefix)
            || (text(&record, "terminalGame") == game_id && id.starts_with("terminal-"));
        if tracked {
            if !incoming.contains_key(id) {
                let mut lost = record.clone();
                lost["terminalArchived"] = json!(true);
                archive.retain(|a| a["id"] != lost["id"]);
                archive.push(lost);
            }
        } else {
            active.push(record);
        }
    }
    let mut count = 0;
    for id in order {
        if removed.contains(&id) || manually_archived.contains(&id) {
            continue;
        }
        let record = incoming.remove(&id).unwrap();
        archive.retain(|r| r["id"] != id);
        active.retain(|r| r["id"] != id);
        active.push(record);
        count += 1;
    }
    w["inventory"]["records"] = json!(active);
    w["inventory"]["archived"] = json!(archive);
    w["nativeGame"] = game.clone();
    w["terminalOwner"] = json!(owner);
    w["terminalSnapshot"] = json!(context.started);
    if w["customName"] != true {
        w["name"] = game["name"].clone();
    }
    let world_id = w["id"].clone();
    next["activeWorldId"] = world_id;
    store.commit(next)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{Temp, record};
    #[test]
    fn dismissing_world_confirmations_never_mutates_the_archive() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let before = store.state.clone();
        let id = text(store.world(), "id").to_owned();
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut worlds = Worlds::default();
        let render = |worlds: &mut Worlds, store: &mut Store, escape: bool| {
            let events = vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: escape,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }];
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960., 620.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| worlds.confirmations(ctx, store),
            );
        };
        for kind in 0..3 {
            match kind {
                0 => worlds.rename = Some((id.clone(), "未保存的新名称".into())),
                1 => worlds.delete = Some(id.clone()),
                _ => worlds.unlink = Some(id.clone()),
            }
            for _ in 0..2 {
                render(&mut worlds, &mut store, false);
            }
            render(&mut worlds, &mut store, true);
            assert!(worlds.rename.is_none() && worlds.delete.is_none() && worlds.unlink.is_none());
            assert_eq!(
                store.state, before,
                "dismiss must not rename, delete or unlink a world"
            );
            // Release Escape before opening the next modal.
            render(&mut worlds, &mut store, false);
        }
    }
    const OWNER: &str = "00000000000000000000000000000001";
    const OTHER: &str = "74e02bf7000000000000000000000000";
    fn game() -> Value {
        json!({"id":"pal-test","name":"测试世界","path":"X:/synthetic","platform":"Steam","modified":"2026"})
    }
    fn row(uuid: &str, owner: &str) -> Value {
        json!({"instanceId":uuid,"speciesKey":"BOSS_SheepBall","name":"自己的昵称","sex":"F","traits":["legend","None"],"ownerId":owner})
    }
    fn sample() -> Value {
        json!({"owners":[{"id":OWNER,"name":"本人"},{"id":OTHER,"name":"另一位"}],"records":[row("11111111111111111111111111111111",OWNER),row("22222222222222222222222222222222",OWNER),row("33333333333333333333333333333333",OTHER)],"warnings":[]})
    }
    fn names() -> GameNames {
        GameNames::from_tables(
            &[json!({"key":"SheepBall","zh":"棉悠悠"})],
            &json!({"traits":[{"key":"Legend"}]}),
        )
    }
    fn context(store: &Store, stamp: u64) -> ReadContext {
        ReadContext {
            target: Some(text(store.world(), "id").into()),
            prior_owner: owner_of(store.world()),
            started: stamp,
            force_owner: false,
        }
    }
    #[test]
    fn owner_memory_prefers_explicit_selection_and_never_guesses_ambiguous() {
        let mut result = sample();
        assert!(chosen_owner(&result, None, false).is_none());
        assert_eq!(
            chosen_owner(&result, Some(OWNER), false).as_deref(),
            Some(OWNER)
        );
        result["owners"][1]["isLocal"] = json!(true);
        assert_eq!(chosen_owner(&result, None, false).as_deref(), Some(OTHER));
        assert_eq!(
            chosen_owner(&result, Some(OWNER), false).as_deref(),
            Some(OWNER)
        );
        assert!(chosen_owner(&result, Some(OWNER), true).is_none());
        result["owners"] = json!([{"id":OWNER}]);
        assert!(chosen_owner(&result, None, false).is_none());
    }
    #[test]
    fn snapshots_deduplicate_archive_restore_and_preserve_manual_records() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let ctx = ReadContext {
            started: 1,
            ..Default::default()
        };
        let result = sample();
        assert_eq!(
            import_terminal(&mut store, &game(), &result, OWNER, &ctx, &names()).unwrap(),
            2
        );
        let original = store.world()["inventory"]["records"][0].clone();
        assert_eq!(original["name"], "自己的昵称");
        assert_eq!(original["speciesKey"], "SheepBall");
        assert_eq!(original["traits"], json!(["Legend"]));
        assert_eq!(original["sex"], "F");
        store.world_mut()["inventory"]["records"]
            .as_array_mut()
            .unwrap()
            .push(record("manual"));
        let ctx = context(&store, 2);
        import_terminal(&mut store, &game(), &result, OWNER, &ctx, &names()).unwrap();
        assert_eq!(array(&store.world()["inventory"]["records"]).len(), 3);
        let mut missing = result.clone();
        missing["records"].as_array_mut().unwrap().remove(0);
        let ctx = context(&store, 3);
        import_terminal(&mut store, &game(), &missing, OWNER, &ctx, &names()).unwrap();
        assert_eq!(array(&store.world()["inventory"]["records"]).len(), 2);
        assert_eq!(
            store.world()["inventory"]["archived"][0]["terminalArchived"],
            true
        );
        let ctx = context(&store, 4);
        import_terminal(&mut store, &game(), &result, OWNER, &ctx, &names()).unwrap();
        assert!(array(&store.world()["inventory"]["archived"]).is_empty());
        assert!(array(&store.world()["inventory"]["records"]).contains(&original));
        let ctx = context(&store, 5);
        import_terminal(&mut store, &game(), &result, OTHER, &ctx, &names()).unwrap();
        assert_eq!(array(&store.world()["inventory"]["records"]).len(), 2);
        assert_eq!(array(&store.world()["inventory"]["archived"]).len(), 2);
        assert!(
            array(&store.world()["inventory"]["records"])
                .iter()
                .any(|r| r["id"] == "manual")
        );
    }
    #[test]
    fn manual_removal_wins_over_refresh_unknown_ids_preserved_and_failure_is_atomic() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let mut result = sample();
        result["records"][0]["speciesKey"] = json!("FuturePal");
        result["records"][0]["traits"] = json!(["FutureTrait"]);
        import_terminal(
            &mut store,
            &game(),
            &result,
            OWNER,
            &ReadContext {
                started: 1,
                ..Default::default()
            },
            &names(),
        )
        .unwrap();
        let r = store.world_mut()["inventory"]["records"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        assert_eq!(r["traits"], json!(["FutureTrait"]));
        store.world_mut()["inventory"]["removed"] = json!([r["id"]]);
        store.world_mut()["inventory"]["archived"] = json!([r]);
        let ctx = context(&store, 2);
        import_terminal(&mut store, &game(), &result, OWNER, &ctx, &names()).unwrap();
        assert_eq!(array(&store.world()["inventory"]["records"]).len(), 1);
        assert_eq!(array(&store.world()["inventory"]["archived"]).len(), 1);
        let before = store.state.clone();
        let mut malformed = result.clone();
        let mut duplicate = malformed["records"][1].clone();
        duplicate["name"] = json!("冲突名字");
        malformed["records"].as_array_mut().unwrap().push(duplicate);
        let ctx = context(&store, 3);
        assert!(import_terminal(&mut store, &game(), &malformed, OWNER, &ctx, &names()).is_err());
        assert_eq!(store.state, before);
        let mut bad = result;
        bad["records"][1]["ownerId"] = json!("unknown");
        assert!(import_terminal(&mut store, &game(), &bad, OWNER, &ctx, &names()).is_err());
        assert_eq!(store.state, before);
    }
    #[test]
    fn read_failure_never_creates_world_and_old_result_never_overwrites_new() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.join("blocked"));
        std::fs::write(&store.root, b"blocked").unwrap();
        let before = store.state.clone();
        assert!(
            import_terminal(
                &mut store,
                &game(),
                &sample(),
                OWNER,
                &ReadContext {
                    started: 2,
                    ..Default::default()
                },
                &names()
            )
            .is_err()
        );
        assert_eq!(store.state, before);
        let mut store = Store::open(temp.0.join("valid"));
        import_terminal(
            &mut store,
            &game(),
            &sample(),
            OWNER,
            &ReadContext {
                started: 4,
                ..Default::default()
            },
            &names(),
        )
        .unwrap();
        let before = store.state.clone();
        let ctx = context(&store, 3);
        assert!(import_terminal(&mut store, &game(), &sample(), OWNER, &ctx, &names()).is_err());
        assert_eq!(store.state, before);
    }
    #[test]
    #[ignore = "uses downloaded public save fixtures; run explicitly in QA"]
    fn public_terminal_fixture_to_warehouse_state_roundtrip() {
        let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../terminal-import/parser/research");
        let parsed = jade_saves::read_terminal_files(
            &[corpus.join("modern-Level.sav")],
            &[
                jade_saves::PlayerFile {
                    id: OWNER.into(),
                    path: corpus.join(format!("modern-Players-{}.sav", OWNER.to_uppercase())),
                },
                jade_saves::PlayerFile {
                    id: OTHER.into(),
                    path: corpus.join(format!("modern-Players-{}.sav", OTHER.to_uppercase())),
                },
            ],
        )
        .unwrap();
        let expected = parsed
            .records
            .iter()
            .filter(|r| r.owner_id == OWNER)
            .count();
        assert!(expected > 0);
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let db: Value =
            serde_json::from_slice(&std::fs::read(data.join("db.json")).unwrap()).unwrap();
        let traits: Value =
            serde_json::from_slice(&std::fs::read(data.join("trait-data.json")).unwrap()).unwrap();
        let mapper = GameNames::from_tables(array(&db["species"]), &traits);
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let result = json!(parsed);
        let ctx = ReadContext {
            started: 1,
            ..Default::default()
        };
        assert_eq!(
            import_terminal(&mut store, &game(), &result, OWNER, &ctx, &mapper).unwrap(),
            expected
        );
        let ctx = context(&store, 2);
        assert_eq!(
            import_terminal(&mut store, &game(), &result, OWNER, &ctx, &mapper).unwrap(),
            expected
        );
        assert_eq!(
            array(&store.world()["inventory"]["records"]).len(),
            expected
        );
        assert!(
            array(&store.world()["inventory"]["records"])
                .iter()
                .all(|r| r["terminalOwnerId"] == OWNER)
        );
        let restored = Store::open(temp.0.clone());
        assert_eq!(restored.state, store.state);
        let backup = store.backup();
        Store::validate_import(&backup).unwrap();
    }
}
