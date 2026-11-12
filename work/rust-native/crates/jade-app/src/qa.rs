//! Opt-in native-window probes. Disabled unless JADE_QA_OUTPUT is explicitly set.
use crate::{data::Data, store::Store};
use eframe::egui;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Scene {
    route: String,
    start: Instant,
    frames: usize,
    phase: u8,
    drag_step: usize,
    action_step: usize,
    action_frame: usize,
    actions: Vec<Value>,
    cpu: Vec<f64>,
    intervals: Vec<f64>,
    last: Instant,
    result: Value,
}
pub struct Probe {
    output: Option<PathBuf>,
    scenes: Vec<String>,
    index: usize,
    current: Option<Scene>,
    started: Instant,
    frame_started: Instant,
    frames: usize,
    report: Value,
    initial: String,
    drag: bool,
    pending_input: Vec<egui::Event>,
    actions: Value,
}
impl Default for Probe {
    fn default() -> Self {
        let output = std::env::var_os("JADE_QA_OUTPUT").map(PathBuf::from);
        let scenes = std::env::var("JADE_QA_SCENES")
            .unwrap_or_else(|_| "#map|#spawns/SheepBall|#map/coords/worldtree/0/800".into())
            .split('|')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        Self {
            output,
            scenes,
            index: 0,
            current: None,
            started: Instant::now(),
            frame_started: Instant::now(),
            frames: 0,
            report: json!({"runtime":"Rust native eframe/glow","scenes":[],"notes":["Native window screenshot callbacks verify GPU painting. UI CPU excludes screenshot file encoding.","Navigation ready is measured after at least two UI passes and terrain texture readiness; screenshot completion additionally includes capture overhead."]}),
            initial: std::env::var("JADE_QA_ROUTE").unwrap_or_default(),
            drag: std::env::var("JADE_QA_DRAG").as_deref() == Ok("1"),
            pending_input: vec![],
            actions: std::env::var("JADE_QA_ACTIONS")
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null),
        }
    }
}
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.
}
fn hash(v: &Value) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn stats(values: &[f64]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let mut s = values.to_vec();
    s.sort_by(f64::total_cmp);
    let p = |q: f64| {
        s[((s.len() as f64 * q).ceil() as usize)
            .saturating_sub(1)
            .min(s.len() - 1)]
    };
    json!({"samples":s.len(),"medianMs":p(0.5),"p95Ms":p(0.95),"maxMs":s.last()})
}
impl Probe {
    pub fn begin_frame(&mut self) {
        if self.output.is_some() {
            self.frame_started = Instant::now();
        }
    }
    fn texture_ready(ctx: &egui::Context, data: &Data, route: &str) -> bool {
        if !route.starts_with("#map") && !route.starts_with("#spawns") {
            return true;
        }
        let region = if route.contains("worldtree") {
            "worldtree"
        } else {
            "palpagos"
        };
        if std::env::var("JADE_QA_MAP_EARLY").as_deref() == Ok("1") {
            let _ = data.map_source(region);
            return true;
        }
        let _ = ctx;
        data.map_ready(region)
    }
    pub fn tick(
        &mut self,
        ctx: &egui::Context,
        route: &mut String,
        data: &Data,
        store: &Store,
    ) -> bool {
        let Some(output) = self.output.clone() else {
            return false;
        };
        self.frames += 1;
        let cpu_ms = self.frame_started.elapsed().as_secs_f64() * 1000.;
        ctx.request_repaint_after(Duration::from_millis(16));
        if self.frames == 1 {
            self.report["firstPassAt"] = json!(now());
            self.report["initialMapWarmed"] = json!(data.background_map_ready());
            self.report["initialInventoryHash"] = json!(hash(&store.world()["inventory"]));
            self.report["initialNotesHash"] = json!(hash(&store.world()["mapNotes"]));
            if !self.initial.is_empty() {
                *route = self.initial.clone();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if self.report["firstPaintCaptureAt"].is_null() {
            if let Some(size) = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.size)
                    } else {
                        None
                    }
                })
            }) {
                self.report["firstPaintCaptureAt"] = json!(now());
                self.report["firstPaintSize"] = json!(size);
            }
        }
        if self.frames == 2 {
            self.report["secondPassAt"] = json!(now());
            self.report["inventoryCount"] =
                json!(crate::data::array(&store.world()["inventory"]["records"]).len());
        }
        if data.background_map_ready() && self.report["mapPreloadedAfterMs"].is_null() {
            self.report["mapPreloadedAfterMs"] =
                json!(self.started.elapsed().as_secs_f64() * 1000.);
        }
        if data.map_ready("worldtree") && self.report["worldtreePreloadedAfterMs"].is_null() {
            self.report["worldtreePreloadedAfterMs"] =
                json!(self.started.elapsed().as_secs_f64() * 1000.);
        }
        if let Some(mut current) = self.current.take() {
            current.frames += 1;
            if current.phase == 2 {
                current.cpu.push(cpu_ms);
                current
                    .intervals
                    .push(current.last.elapsed().as_secs_f64() * 1000.);
                current.last = Instant::now();
            }
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if current.phase == 0
                && current.frames >= 3
                && Self::texture_ready(ctx, data, &current.route)
            {
                current.result["fullTerrainReady"] =
                    json!(data.map_ready(if current.route.contains("worldtree") {
                        "worldtree"
                    } else {
                        "palpagos"
                    }));
                current.result["readyMs"] = json!(current.start.elapsed().as_secs_f64() * 1000.);
                current.result["readyPassCpuMs"] = json!(cpu_ms);
                current.phase = 1;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            } else if current.phase == 5 && current.frames >= 3 {
                current.phase = 6;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            } else if (current.phase == 1
                || current.phase == 3
                || current.phase == 6
                || current.phase == 8)
                && screenshot.is_some()
            {
                let image = screenshot.unwrap();
                let bytes = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_array())
                    .collect::<Vec<_>>();
                let filename = format!(
                    "native-scene-{:02}-{}.png",
                    self.index,
                    if current.phase == 1 {
                        "ready".into()
                    } else if current.phase == 8 {
                        format!("action-{:02}", current.action_step + 1)
                    } else if current.phase == 6 {
                        "zoom".into()
                    } else {
                        "drag".into()
                    }
                );
                let file = output.parent().unwrap().join(filename);
                let save = image::save_buffer(
                    &file,
                    &bytes,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                );
                if let Err(e) = save {
                    current.result["screenshotError"] = json!(e.to_string());
                }
                let hash = format!("{:x}", Sha256::digest(&bytes));
                if current.phase == 1 {
                    current.result["screenshotAtMs"] =
                        json!(current.start.elapsed().as_secs_f64() * 1000.);
                    current.result["screenshot"] = json!(file);
                    current.result["imageHash"] = json!(hash);
                    current.result["width"] = json!(image.size[0]);
                    current.result["height"] = json!(image.size[1]);
                    if !current.actions.is_empty() {
                        current.phase = 7;
                        current.action_frame = 0;
                    } else if self.drag
                        && (current.route.starts_with("#map")
                            || current.route.starts_with("#spawns"))
                    {
                        current.phase = 5;
                        current.frames = 0;
                        self.mouse(0, 500., 420.);
                        self.pending_input.push(egui::Event::Zoom(4.));
                    } else {
                        current.phase = 4;
                    }
                } else if current.phase == 8 {
                    let shape =
                        ctx.data(|m| m.get_temp::<Value>(egui::Id::new("jade-window-shape")));
                    current.result["actions"].as_array_mut().unwrap().push(json!({"action":current.actions[current.action_step],"screenshot":file,"imageHash":hash,"routeAfter":route.as_str(),"windowShape":shape}));
                    current.action_step += 1;
                    current.action_frame = 0;
                    current.phase = if current.action_step >= current.actions.len() {
                        4
                    } else {
                        7
                    };
                } else if current.phase == 6 {
                    current.result["dragBaselineScreenshot"] = json!(file);
                    current.result["dragBaselineHash"] = json!(hash);
                    current.result["dragZoom"] = json!("4x");
                    current.phase = 2;
                    current.last = Instant::now();
                    self.mouse(1, 500., 420.);
                } else {
                    current.result["dragScreenshot"] = json!(file);
                    current.result["dragChangedPixels"] =
                        json!(current.result["dragBaselineHash"].as_str() != Some(&hash));
                    current.result["dragUiCpu"] = stats(&current.cpu);
                    current.result["dragFrameIntervals"] = stats(&current.intervals);
                    current.result["dragFrameIntervalsRaw"] = json!(current.intervals);
                    current.phase = 4;
                }
            } else if current.phase == 7 {
                let action = &current.actions[current.action_step];
                let point = action.as_array().or_else(|| action["click"].as_array());
                if let Some(point) = point {
                    if current.action_frame < 2 {
                        self.mouse(
                            if current.action_frame == 0 { 1 } else { 3 },
                            point.first().and_then(Value::as_f64).unwrap_or(0.) as f32,
                            point.get(1).and_then(Value::as_f64).unwrap_or(0.) as f32,
                        );
                    }
                } else if current.action_frame == 0 {
                    if let Some(maximized) = action["maximize"].as_bool() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(maximized));
                    }
                    if let Some(point) = action["hover"].as_array() {
                        self.mouse(
                            0,
                            point.first().and_then(Value::as_f64).unwrap_or(0.) as f32,
                            point.get(1).and_then(Value::as_f64).unwrap_or(0.) as f32,
                        );
                    }
                    if let Some(s) = action["text"].as_str() {
                        self.pending_input.push(egui::Event::Text(s.into()));
                    }
                    if let Some(point) = action["scroll"].as_array() {
                        self.pending_input.push(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::Vec2::new(
                                point.first().and_then(Value::as_f64).unwrap_or(0.) as f32,
                                point.get(1).and_then(Value::as_f64).unwrap_or(0.) as f32,
                            ),
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                    if let Some(name) = action["key"].as_str() {
                        if let Some(key) = match name {
                            "Escape" => Some(egui::Key::Escape),
                            "Enter" => Some(egui::Key::Enter),
                            "Tab" => Some(egui::Key::Tab),
                            "Home" => Some(egui::Key::Home),
                            _ => None,
                        } {
                            for pressed in [true, false] {
                                self.pending_input.push(egui::Event::Key {
                                    key,
                                    physical_key: None,
                                    pressed,
                                    repeat: false,
                                    modifiers: egui::Modifiers::NONE,
                                });
                            }
                        }
                    }
                }
                current.action_frame += 1;
                if current.action_frame
                    >= action["waitFrames"].as_u64().unwrap_or(6).max(4) as usize
                {
                    current.phase = 8;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            } else if current.phase == 2 {
                current.drag_step += 1;
                let t = current.drag_step as f32 / 90.;
                self.mouse(
                    2,
                    500. + (t * std::f32::consts::TAU).sin() * 130. + t * 60.,
                    420. + (t * std::f32::consts::TAU * 2.).sin() * 60.,
                );
                if current.drag_step >= 90 {
                    self.mouse(3, 560., 420.);
                    current.phase = 3;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            }
            if current.start.elapsed()
                > Duration::from_secs(if current.actions.is_empty() { 20 } else { 60 })
            {
                current.result["error"] = json!("Timed out waiting for native scene/capture");
                self.mouse(3, 560., 420.);
                current.phase = 4;
            }
            if current.phase == 4 {
                self.report["scenes"]
                    .as_array_mut()
                    .unwrap()
                    .push(current.result);
                self.index += 1;
            } else {
                self.current = Some(current);
            }
        }
        if self.current.is_none()
            && self.started.elapsed()
                > if std::env::var("JADE_QA_MAP_EARLY").as_deref() == Ok("1") {
                    Duration::from_millis(100)
                } else {
                    Duration::from_secs(5)
                }
            && data.pending == 0
        {
            if let Some(next) = self.scenes.get(self.index) {
                *route = next.clone();
                self.current = Some(Scene {
                    route: next.clone(),
                    start: Instant::now(),
                    frames: 0,
                    phase: 0,
                    drag_step: 0,
                    action_step: 0,
                    action_frame: 0,
                    actions: self.actions[next].as_array().cloned().unwrap_or_default(),
                    cpu: vec![],
                    intervals: vec![],
                    last: Instant::now(),
                    result: json!({"route":next,"actions":[]}),
                });
                ctx.request_repaint();
            } else {
                self.report["finalInventoryHash"] = json!(hash(&store.world()["inventory"]));
                self.report["finalNotesHash"] = json!(hash(&store.world()["mapNotes"]));
                self.report["errors"] = json!(data.errors);
                self.report["mapPoints"] =
                    json!(crate::data::array(&data.get("map-data")["points"]).len());
                self.report["tables"] = json!(data.tables.len());
                self.report["palCount"] = json!(data.species.len());
                self.report["finishedAt"] = json!(now());
                if let Some(shape) =
                    ctx.data(|m| m.get_temp::<Value>(egui::Id::new("jade-window-shape")))
                {
                    self.report["windowShape"] = shape;
                }
                let _ = std::fs::create_dir_all(output.parent().unwrap());
                let _ = std::fs::write(output, serde_json::to_vec_pretty(&self.report).unwrap());
                return true;
            }
        }
        false
    }
}

// Replay through egui's native input boundary without moving the user's real cursor.
impl Probe {
    pub fn raw_input_hook(&mut self, input: &mut egui::RawInput) {
        if self.output.is_some() && !self.pending_input.is_empty() {
            input.focused = true;
            input.events.append(&mut self.pending_input);
        }
    }
    fn mouse(&mut self, kind: u8, x: f32, y: f32) {
        let pos = egui::Pos2::new(x, y);
        self.pending_input.push(egui::Event::PointerMoved(pos));
        if kind == 1 || kind == 3 {
            self.pending_input.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: kind == 1,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
}
