//! A single continuing terrain worker, shared by startup and map navigation.
use eframe::egui::{self, load::SizedTexture};
use std::{cell::RefCell, collections::HashMap, path::PathBuf, sync::mpsc, time::Duration};

const REGIONS: [&str; 2] = ["palpagos", "worldtree"];
type Decoded = (String, Result<egui::ColorImage, String>);

pub struct Terrain {
    ctx: egui::Context,
    request: mpsc::Sender<String>,
    results: RefCell<mpsc::Receiver<Decoded>>,
    full: RefCell<HashMap<String, egui::TextureHandle>>,
    previews: RefCell<HashMap<String, egui::TextureHandle>>,
    errors: RefCell<HashMap<String, String>>,
}
impl Terrain {
    pub fn new(root: PathBuf, ctx: egui::Context) -> Self {
        let delay = if std::env::var_os("JADE_QA_OUTPUT").is_some() {
            std::env::var("JADE_QA_TERRAIN_DELAY_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .map(Duration::from_millis)
                .unwrap_or_default()
        } else {
            Duration::ZERO
        };
        Self::start(root, ctx, Duration::from_millis(1200), delay)
    }
    fn start(root: PathBuf, ctx: egui::Context, grace: Duration, delay: Duration) -> Self {
        let (request, requests) = mpsc::channel::<String>();
        // At most one decoded terrain waits for the UI. Only one decoder runs.
        let (sender, results) = mpsc::sync_channel(1);
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            // A fixed first-paint grace, never extended by mouse/key activity.
            // Opening a map wakes this worker early and joins the same load.
            let first = match requests.recv_timeout(grace) {
                Ok(region) => Some(region),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            };
            let mut order = REGIONS.map(str::to_owned).to_vec();
            if first.as_deref() == Some("worldtree") {
                order.swap(0, 1);
            }
            let table = std::fs::read(root.join("map-images.json"))
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string())
                });
            for region in order {
                if matches!(requests.try_recv(), Err(mpsc::TryRecvError::Disconnected)) {
                    return;
                }
                // QA-only delay proves the preview is painted before full decoding.
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                let cap = repaint.input(|i| i.max_texture_side).max(1) as u32;
                let result = table.as_ref().map_err(Clone::clone).and_then(|table| {
                    let uri = table[&region]
                        .as_str()
                        .filter(|s| s.starts_with("assets/"))
                        .ok_or_else(|| format!("缺少 {region} 地图底图"))?;
                    image::open(root.join(uri))
                        .map(|img| crate::data::capped_color_image(img, cap))
                        .map_err(|e| e.to_string())
                });
                // Wake even when the bounded channel is waiting for a UI pass.
                repaint.request_repaint();
                if sender.send((region, result)).is_err() {
                    return;
                }
                repaint.request_repaint();
            }
        });
        Self {
            ctx,
            request,
            results: RefCell::new(results),
            full: RefCell::new(HashMap::new()),
            previews: RefCell::new(HashMap::new()),
            errors: RefCell::new(HashMap::new()),
        }
    }
    pub fn poll(&self) {
        while let Ok((region, result)) = self.results.borrow_mut().try_recv() {
            match result {
                Ok(image) => {
                    let texture = self.ctx.load_texture(
                        format!("jade-terrain-{region}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.full.borrow_mut().insert(region.clone(), texture);
                    self.previews.borrow_mut().remove(&region);
                }
                Err(error) => {
                    self.errors.borrow_mut().insert(region, error);
                }
            }
        }
    }
    pub fn ready(&self, region: &str) -> bool {
        self.full.borrow().contains_key(region)
    }
    pub fn error(&self, region: &str) -> Option<String> {
        self.errors.borrow().get(region).cloned()
    }
    pub fn source(&self, region: &str) -> egui::ImageSource<'static> {
        self.poll();
        if let Some(t) = self.full.borrow().get(region) {
            return egui::ImageSource::Texture(SizedTexture::new(t.id(), t.size_vec2()));
        }
        if !self.previews.borrow().contains_key(region) {
            let bytes = if region == "worldtree" {
                include_bytes!("../assets/map-previews/worldtree.webp").as_slice()
            } else {
                include_bytes!("../assets/map-previews/palpagos.webp").as_slice()
            };
            let img = image::load_from_memory(bytes).expect("validated embedded terrain preview");
            let cap = self.ctx.input(|i| i.max_texture_side).max(1) as u32;
            self.previews.borrow_mut().insert(
                region.to_owned(),
                self.ctx.load_texture(
                    format!("jade-terrain-preview-{region}"),
                    crate::data::capped_color_image(img, cap),
                    egui::TextureOptions::LINEAR,
                ),
            );
            let _ = self.request.send(region.to_owned());
        }
        let t = &self.previews.borrow()[region];
        egui::ImageSource::Texture(SizedTexture::new(t.id(), t.size_vec2()))
    }
    #[cfg(test)]
    pub fn size(&self, region: &str) -> Option<[usize; 2]> {
        self.full.borrow().get(region).map(|t| t.size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "jade-terrain-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(root.join("assets")).unwrap();
        image::RgbaImage::from_pixel(2048, 2048, image::Rgba([30, 150, 90, 255]))
            .save(root.join("assets/map.png"))
            .unwrap();
        std::fs::write(
            root.join("map-images.json"),
            r#"{"palpagos":"assets/map.png","worldtree":"assets/map.png"}"#,
        )
        .unwrap();
        root
    }
    #[test]
    fn preview_then_both_terrains_continue_during_input_and_minimization() {
        let ctx = egui::Context::default();
        let terrain = Terrain::start(
            fixture(),
            ctx.clone(),
            Duration::from_secs(30),
            Duration::from_millis(50),
        );
        let _ = ctx.run(egui::RawInput::default(), |_| {
            assert!(matches!(
                terrain.source("worldtree"),
                egui::ImageSource::Texture(_)
            ));
            assert!(!terrain.ready("worldtree"));
        });
        let start = std::time::Instant::now();
        while !terrain.ready("palpagos") || !terrain.ready("worldtree") {
            let mut input = egui::RawInput {
                max_texture_side: Some(1024),
                events: vec![egui::Event::PointerMoved(egui::pos2(10., 10.))],
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .minimized = Some(true);
            let _ = ctx.run(input, |_| terrain.poll());
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(terrain.size("palpagos"), Some([1024, 1024]));
        assert_eq!(terrain.size("worldtree"), Some([1024, 1024]));
    }
    #[test]
    fn broken_full_asset_keeps_the_embedded_map_visible() {
        let root = fixture();
        std::fs::write(root.join("map-images.json"), "{}").unwrap();
        let ctx = egui::Context::default();
        let terrain = Terrain::start(root, ctx.clone(), Duration::from_millis(20), Duration::ZERO);
        let start = std::time::Instant::now();
        while terrain.error("palpagos").is_none() {
            let _ = ctx.run(egui::RawInput::default(), |_| {
                assert!(matches!(
                    terrain.source("palpagos"),
                    egui::ImageSource::Texture(_)
                ));
            });
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!terrain.ready("palpagos"));
    }
    #[test]
    fn startup_loads_both_without_visiting_map() {
        let ctx = egui::Context::default();
        let terrain = Terrain::start(
            fixture(),
            ctx.clone(),
            Duration::from_millis(20),
            Duration::ZERO,
        );
        let start = std::time::Instant::now();
        while !terrain.ready("palpagos") || !terrain.ready("worldtree") {
            let _ = ctx.run(egui::RawInput::default(), |_| terrain.poll());
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(terrain.previews.borrow().is_empty());
    }
}
