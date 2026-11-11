use base64::Engine;
use eframe::egui::{self, Vec2};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    hash::{Hash, Hasher},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Arc},
};
use unicode_normalization::UnicodeNormalization;

static EMPTY: Value = Value::Null;
pub struct Data {
    pub root: PathBuf,
    pub species: Vec<Value>,
    pub pairs: Vec<Value>,
    pub tables: HashMap<String, Arc<Value>>,
    pub routes: HashMap<String, (String, String)>,
    receiver: mpsc::Receiver<(String, Result<Value, String>)>,
    pub errors: Vec<String>,
    pub pending: usize,
    inline_images: RefCell<HashMap<String, (String, Arc<[u8]>)>>,
    preload_started: std::time::Instant,
    preload_last_input: std::time::Instant,
    preload_receiver: Option<mpsc::Receiver<Result<egui::ColorImage, String>>>,
    preload_attempted: bool,
    map_requested: Cell<bool>,
    warmed_map: Option<egui::TextureHandle>,
}
impl Data {
    pub fn new(root: PathBuf, ctx: egui::Context) -> Result<Self, String> {
        // Intercept large terrain decoding on GPUs with smaller texture limits.
        // This also covers a foreground map opened before idle preloading begins.
        ctx.add_image_loader(Arc::new(CappedRasterLoader::default()));
        let mut tables = HashMap::new();
        for id in ["db", "trait-data", "portraits", "trait-ranks"] {
            let bytes = std::fs::read(root.join(format!("{id}.json")))
                .map_err(|e| format!("资料 {id} 无法读取：{e}"))?;
            tables.insert(
                id.to_owned(),
                Arc::new(serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())?),
            );
        }
        let species = tables["db"]["species"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let pairs: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("pairs.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        validate_core_data(&species, &pairs)?;
        let ids = [
            "map-data",
            "map-images",
            "spawn-data",
            "pal-details-data",
            "paldex-index-data",
            "search-index-data",
            "item-catalog-data",
            "item-icons",
            "stat-data",
            "capture-data",
            "paldex-data",
            "partner-skills-data",
            "work-efficiency-data",
            "enemy-data",
            "combat-guide-data",
            "journals-data",
            "expedition-data",
            "adventure-data",
            "resource-guides-data",
            "relic-data",
            "ranch-data",
            "facilities-data",
            "announcements",
        ];
        let (sender, receiver) = mpsc::channel();
        let location = root.clone();
        std::thread::spawn(move || {
            // Initial UI goes first. One serial I/O worker bounds memory on small PCs.
            std::thread::sleep(std::time::Duration::from_millis(450));
            for id in ids {
                let result = std::fs::read(location.join(format!("{id}.json")))
                    .map_err(|e| e.to_string())
                    .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()));
                if sender.send((id.to_owned(), result)).is_err() {
                    break;
                }
                ctx.request_repaint();
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        });
        Ok(Self {
            root,
            species,
            pairs,
            tables,
            routes: HashMap::new(),
            receiver,
            errors: vec![],
            pending: ids.len(),
            inline_images: RefCell::new(HashMap::new()),
            preload_started: std::time::Instant::now(),
            preload_last_input: std::time::Instant::now(),
            preload_receiver: None,
            preload_attempted: false,
            map_requested: Cell::new(false),
            warmed_map: None,
        })
    }
    pub fn poll(&mut self) {
        while let Ok((id, result)) = self.receiver.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            match result {
                Ok(v) => {
                    if id == "search-index-data" {
                        self.routes.clear();
                        for row in array(&v) {
                            let target =
                                (text(row, "href").to_owned(), text(row, "name").to_owned());
                            for key in std::iter::once(text(row, "name"))
                                .chain(array(&row["aliases"]).iter().filter_map(Value::as_str))
                            {
                                if !key.is_empty() {
                                    self.routes
                                        .entry(normalize(key))
                                        .or_insert_with(|| target.clone());
                                }
                            }
                        }
                    }
                    self.tables.insert(id, Arc::new(v));
                }
                Err(e) => self.errors.push(format!("{id}: {e}")),
            }
        }
    }
    pub fn resolve(&self, value: &str) -> Option<&(String, String)> {
        self.routes.get(&normalize(value))
    }
    pub fn background_map_ready(&self) -> bool {
        self.warmed_map.is_some()
    }
    /// Decode the default terrain on a worker only after the first UI and an idle grace.
    /// User navigation always remains free to request its own foreground image.
    pub fn preload(&mut self, ctx: &egui::Context) {
        let (hidden, active) = ctx.input(|i| {
            (
                i.viewport().minimized.unwrap_or(false),
                i.pointer.any_down()
                    || i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::Key { .. }
                                | egui::Event::Text(_)
                                | egui::Event::PointerMoved(_)
                                | egui::Event::PointerButton { .. }
                                | egui::Event::MouseWheel { .. }
                                | egui::Event::Touch { .. }
                        )
                    }),
            )
        });
        if active {
            self.preload_last_input = std::time::Instant::now();
        }
        if hidden || active {
            return;
        }
        if let Some(receiver) = &self.preload_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.preload_receiver = None;
                if !self.map_requested.get() {
                    if let Ok(image) = result {
                        self.warmed_map = Some(ctx.load_texture(
                            "jade-default-terrain",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
            }
            return;
        }
        if self.preload_attempted || self.map_requested.get() || self.warmed_map.is_some() {
            return;
        }
        let quiet = if std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            <= 4
        {
            2400
        } else {
            1200
        };
        if self.preload_started.elapsed().as_millis() < 1200
            || self.preload_last_input.elapsed().as_millis() < quiet
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        let Some(uri) = self.get("map-images")["palpagos"]
            .as_str()
            .filter(|s| s.starts_with("assets/"))
        else {
            return;
        };
        let path = self.root.join(uri);
        let max_side = ctx.input(|i| i.max_texture_side).max(1) as u32;
        let (sender, receiver) = mpsc::channel();
        let repaint = ctx.clone();
        self.preload_receiver = Some(receiver);
        self.preload_attempted = true;
        std::thread::spawn(move || {
            let result = image::open(path)
                .map(|image| capped_color_image(image, max_side))
                .map_err(|e| e.to_string());
            let _ = sender.send(result);
            repaint.request_repaint();
        });
    }
    pub fn get(&self, id: &str) -> &Value {
        self.tables.get(id).map(AsRef::as_ref).unwrap_or(&EMPTY)
    }
    pub fn ready(&self, id: &str) -> bool {
        self.tables.contains_key(id)
    }
    pub fn pal(&self, key: &str) -> Option<&Value> {
        self.species.iter().find(|p| text(p, "key") == key)
    }
    pub fn pal_name(&self, key: &str) -> String {
        self.pal(key)
            .map(|p| text(p, "zh"))
            .filter(|s| !s.is_empty())
            .unwrap_or(key)
            .to_owned()
    }
    pub fn item_name(&self, id: &str) -> String {
        self.get("item-catalog-data")["items"]
            .as_array()
            .and_then(|a| a.iter().find(|i| text(i, "id") == id))
            .map(|v| text(v, "name").to_owned())
            .unwrap_or_else(|| id.to_owned())
    }
    pub fn trait_name(&self, key: &str) -> String {
        self.get("trait-data")["traits"]
            .as_array()
            .and_then(|a| a.iter().find(|i| text(i, "key") == key))
            .map(|v| text(v, "zh").to_owned())
            .unwrap_or_else(|| key.to_owned())
    }
    pub fn image_uri(&self, uri: &str) -> String {
        if uri.starts_with("assets/") {
            format!(
                "file:///{}",
                self.root.join(uri).to_string_lossy().replace('\\', "/")
            )
        } else {
            uri.to_owned()
        }
    }
    pub fn image_source(&self, uri: &str) -> egui::ImageSource<'static> {
        if self.get("map-images")["palpagos"].as_str() == Some(uri) {
            if let Some(texture) = &self.warmed_map {
                return egui::ImageSource::Texture(egui::load::SizedTexture::new(
                    texture.id(),
                    texture.size_vec2(),
                ));
            }
            self.map_requested.set(true);
        }
        if uri.starts_with("data:image/") {
            if let Some((name, bytes)) = self.inline_images.borrow().get(uri) {
                return egui::ImageSource::Bytes {
                    uri: name.clone().into(),
                    bytes: bytes.clone().into(),
                };
            }
            if let Some((header, encoded)) = uri.split_once(',') {
                if header.ends_with(";base64") {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                        let mut hash = std::collections::hash_map::DefaultHasher::new();
                        uri.hash(&mut hash);
                        let extension = header
                            .strip_prefix("data:image/")
                            .unwrap_or("image")
                            .split(';')
                            .next()
                            .unwrap_or("image")
                            .replace("svg+xml", "svg");
                        let name = format!("bytes://inline-{:016x}.{extension}", hash.finish());
                        let bytes: Arc<[u8]> = bytes.into();
                        self.inline_images
                            .borrow_mut()
                            .insert(uri.to_owned(), (name.clone(), bytes.clone()));
                        return egui::ImageSource::Bytes {
                            uri: name.into(),
                            bytes: bytes.into(),
                        };
                    }
                }
            }
        }
        egui::ImageSource::Uri(self.image_uri(uri).into())
    }
    pub fn image(&self, ui: &mut egui::Ui, uri: &str, size: Vec2) {
        if !uri.is_empty() {
            ui.add(egui::Image::new(self.image_source(uri)).fit_to_exact_size(size));
        }
    }
    pub fn portrait(&self, ui: &mut egui::Ui, key: &str, size: f32) {
        if let Some(p) = self.pal(key) {
            let uri = self.get("portraits")[text(p, "en")].as_str().unwrap_or("");
            self.image(ui, uri, Vec2::splat(size));
        }
    }
    pub fn item_icon(&self, ui: &mut egui::Ui, key: &str, size: f32) {
        if let Some(i) = self.get("item-icons")["ids"][key].as_u64() {
            let uri = self.get("item-icons")["images"][i as usize]
                .as_str()
                .unwrap_or("");
            self.image(ui, uri, Vec2::splat(size));
        }
    }
}
fn validate_core_data(species: &[Value], pairs: &[Value]) -> Result<(), String> {
    if species.is_empty() {
        return Err("帕鲁资料为空或格式损坏，请重新解压完整的测试版程序。".into());
    }
    let mut keys = std::collections::HashSet::new();
    for pal in species {
        let key = text(pal, "key");
        if key.trim().is_empty() || !keys.insert(key) {
            return Err("帕鲁资料包含缺失或重复的编号，请重新解压完整的测试版程序。".into());
        }
    }
    if pairs.is_empty() {
        return Err("配种表为空，请重新解压完整的测试版程序。".into());
    }
    for (row, pair) in pairs.iter().enumerate() {
        let Some(values) = pair.as_array() else {
            return Err(format!("配种表第 {} 条记录格式损坏。", row + 1));
        };
        if ![3, 5].contains(&values.len())
            || values
                .iter()
                .take(3)
                .any(|v| v.as_u64().is_none_or(|index| index >= species.len() as u64))
        {
            return Err(format!(
                "配种表第 {} 条记录包含无效的帕鲁编号，请重新解压完整程序。",
                row + 1
            ));
        }
        if values.len() == 5
            && values
                .iter()
                .skip(3)
                .any(|v| !matches!(v.as_str(), Some("M" | "F")))
        {
            return Err(format!("配种表第 {} 条记录的性别条件无效。", row + 1));
        }
    }
    Ok(())
}
pub fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
pub fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn search(v: &Value, q: &str) -> bool {
    let query = normalize(q);
    if query.trim().is_empty() {
        return true;
    }
    let content = normalize(&v.to_string());
    query.split_whitespace().all(|word| content.contains(word))
}
/// The same comparison key for full-width input, aliases, and mixed Latin case.
pub fn normalize(s: &str) -> String {
    s.nfkc().flat_map(char::to_lowercase).collect()
}

fn capped_color_image(image: image::DynamicImage, max_side: u32) -> egui::ColorImage {
    let image = if image.width() > max_side || image.height() > max_side {
        image.thumbnail(max_side, max_side)
    } else {
        image
    };
    let rgba = image.into_rgba8();
    egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    )
}
type CappedEntry = Option<Result<Arc<egui::ColorImage>, String>>;
#[derive(Default)]
struct CappedRasterLoader {
    cache: Arc<std::sync::Mutex<HashMap<String, CappedEntry>>>,
}
impl egui::load::ImageLoader for CappedRasterLoader {
    fn id(&self) -> &str {
        "jade-gpu-limited-raster"
    }
    fn load(
        &self,
        ctx: &egui::Context,
        uri: &str,
        _: egui::load::SizeHint,
    ) -> egui::load::ImageLoadResult {
        use egui::load::{BytesPoll, ImagePoll, LoadError};
        let limit = ctx.input(|i| i.max_texture_side).max(1) as u32;
        if limit >= 4096
            || !uri.starts_with("file://")
            || ![".webp", ".png", ".jpeg", ".jpg"]
                .iter()
                .any(|ext| uri.ends_with(ext))
        {
            return Err(LoadError::NotSupported);
        }
        if let Some(entry) = self.cache.lock().unwrap().get(uri).cloned() {
            return match entry {
                None => Ok(ImagePoll::Pending { size: None }),
                Some(Ok(image)) => Ok(ImagePoll::Ready { image }),
                Some(Err(e)) => Err(LoadError::Loading(e)),
            };
        }
        match ctx.try_load_bytes(uri)? {
            BytesPoll::Pending { size } => Ok(ImagePoll::Pending { size }),
            BytesPoll::Ready { bytes, .. } => {
                let key = uri.to_owned();
                self.cache.lock().unwrap().insert(key.clone(), None);
                let cache = self.cache.clone();
                let context = ctx.clone();
                std::thread::spawn(move || {
                    let result = image::load_from_memory(&bytes)
                        .map(|img| Arc::new(capped_color_image(img, limit)))
                        .map_err(|e| e.to_string());
                    {
                        let mut entries = cache.lock().unwrap();
                        if let Some(entry) = entries.get_mut(&key) {
                            *entry = Some(result);
                        }
                    }
                    context.request_repaint();
                });
                Ok(ImagePoll::Pending { size: None })
            }
        }
    }
    fn forget(&self, uri: &str) {
        self.cache.lock().unwrap().remove(uri);
    }
    fn forget_all(&self) {
        self.cache.lock().unwrap().clear();
    }
    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .unwrap()
            .values()
            .filter_map(|v| v.as_ref().and_then(|v| v.as_ref().ok()))
            .map(|v| v.pixels.len() * 4)
            .sum()
    }
}

#[cfg(test)]
mod preload_tests {
    use super::*;
    #[test]
    fn corrupt_core_data_is_rejected_before_ui() {
        let pals = vec![serde_json::json!({"key":"SheepBall"})];
        assert!(validate_core_data(&pals, &[serde_json::json!([0, 0, 0, "F", "M"])]).is_ok());
        for (species, pairs) in [
            (vec![], vec![serde_json::json!([0, 0, 0])]),
            (
                vec![pals[0].clone(), pals[0].clone()],
                vec![serde_json::json!([0, 0, 0])],
            ),
            (pals.clone(), vec![]),
            (pals.clone(), vec![serde_json::json!([0, 0, 1])]),
            (pals.clone(), vec![serde_json::json!([0, 0])]),
            (pals.clone(), vec![serde_json::json!([0, 0, 0, "X", "M"])]),
        ] {
            assert!(validate_core_data(&species, &pairs).is_err());
        }
    }
    #[test]
    fn normalized_search_matches_all_words_in_any_order() {
        let value = serde_json::json!({"name":"棉悠悠", "en":"Lamball", "dex":"001"});
        assert!(search(&value, "ＬＡＭＢＡＬＬ　００１"));
        assert!(search(&value, "001 棉悠悠"));
        assert!(!search(&value, "棉悠悠 电"));
        assert_eq!(normalize("Ｆｏｏ e\u{301}"), normalize("foo é"));
    }
    #[test]
    fn foreground_terrain_respects_gpu_texture_limit() {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = [base.join("../../data"), base.join("../../rust-native/data")]
            .into_iter()
            .find(|p| p.join("map-images.json").exists())
            .unwrap();
        let maps: Value =
            serde_json::from_slice(&std::fs::read(root.join("map-images.json")).unwrap()).unwrap();
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let data = Data::new(root, ctx.clone()).unwrap();
        let uri = data.image_uri(maps["worldtree"].as_str().unwrap());
        let start = std::time::Instant::now();
        let mut size = None;
        while size.is_none() && start.elapsed().as_secs() < 10 {
            let raw = egui::RawInput {
                max_texture_side: Some(1024),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                if let Ok(egui::load::TexturePoll::Ready { texture }) = ctx.try_load_texture(
                    &uri,
                    egui::TextureOptions::LINEAR,
                    egui::load::SizeHint::default(),
                ) {
                    size = Some(texture.size);
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(size, Some(Vec2::splat(1024.)));
    }
    #[test]
    fn initial_hidden_and_input_grace_then_worker_decode() {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = [base.join("../../data"), base.join("../../rust-native/data")]
            .into_iter()
            .find(|p| p.join("map-images.json").exists())
            .unwrap();
        let ctx = egui::Context::default();
        let mut data = Data::new(root.clone(), ctx.clone()).unwrap();
        data.tables.insert(
            "map-images".into(),
            Arc::new(
                serde_json::from_slice(&std::fs::read(root.join("map-images.json")).unwrap())
                    .unwrap(),
            ),
        );
        let _ = ctx.run(egui::RawInput::default(), |ctx| data.preload(ctx));
        assert!(!data.preload_attempted);
        assert!(!data.background_map_ready());
        data.preload_started -= std::time::Duration::from_secs(3);
        data.preload_last_input -= std::time::Duration::from_secs(3);
        let mut hidden = egui::RawInput::default();
        hidden
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .minimized = Some(true);
        let _ = ctx.run(hidden, |ctx| data.preload(ctx));
        assert!(!data.preload_attempted);
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .minimized = Some(false);
        input
            .events
            .push(egui::Event::PointerMoved(egui::Pos2::new(20., 20.)));
        let _ = ctx.run(input, |ctx| data.preload(ctx));
        assert!(!data.preload_attempted);
        data.preload_last_input -= std::time::Duration::from_secs(3);
        let _ = ctx.run(egui::RawInput::default(), |ctx| data.preload(ctx));
        assert!(data.preload_attempted);
        let start = std::time::Instant::now();
        while !data.background_map_ready() && start.elapsed().as_secs() < 5 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let _ = ctx.run(egui::RawInput::default(), |ctx| data.preload(ctx));
        }
        assert!(data.background_map_ready());
        let uri = data.get("map-images")["palpagos"].as_str().unwrap();
        assert!(matches!(
            data.image_source(uri),
            egui::ImageSource::Texture(_)
        ));
        let cap = ctx.input(|i| i.max_texture_side).min(4096);
        assert_eq!(data.warmed_map.as_ref().unwrap().size(), [cap, cap]);
    }
}
