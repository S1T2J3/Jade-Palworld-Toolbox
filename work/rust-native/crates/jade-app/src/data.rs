use base64::Engine;
use eframe::egui::{self, Vec2};
use serde_json::Value;
use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, mpsc},
};
#[path = "handbook_data.rs"]
mod handbook;

pub use handbook::{array, normalize, text};
use handbook::validate_core_data;
pub struct Data {
    #[cfg(test)]
    pub(crate) catalog_text: std::sync::OnceLock<crate::assistant_discovery::CatalogText>,
    #[cfg(test)]
    pub(crate) recipe_names: std::sync::OnceLock<crate::assistant_data::recipe_editions::RecipeNames>,
    pub root: PathBuf,
    pub species: Vec<Value>,
    pub pairs: Vec<Value>,
    pub tables: HashMap<String, Arc<Value>>,
    pub routes: HashMap<String, (String, String)>,
    pub trait_search: crate::search::TraitIndex,
    receiver: mpsc::Receiver<(String, Result<Value, String>)>,
    pub errors: Vec<String>,
    pub pending: usize,
    inline_images: RefCell<HashMap<String, (String, Arc<[u8]>)>>,
    terrain: crate::terrain::Terrain,
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
            "item-effects-data",
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
        let terrain = crate::terrain::Terrain::new(root.clone(), ctx.clone());
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
        let trait_search = crate::search::TraitIndex::new(&tables["trait-data"]);
        Ok(Self {
            root,
            species,
            pairs,
            tables,
            routes: HashMap::new(),
            #[cfg(test)]
            catalog_text: Default::default(),
            #[cfg(test)]
            recipe_names: Default::default(),
            trait_search,
            receiver,
            errors: vec![],
            pending: ids.len(),
            inline_images: RefCell::new(HashMap::new()),
            terrain,
        })
    }
    pub fn poll(&mut self) {
        while let Ok((id, result)) = self.receiver.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            match result {
                Ok(v) => {
                    if id == "pal-details-data" {
                        self.trait_search.add_details(&v);
                    }
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
        self.terrain.ready("palpagos")
    }
    pub fn map_ready(&self, region: &str) -> bool {
        self.terrain.ready(region)
    }
    pub fn map_error(&self, region: &str) -> Option<String> {
        self.terrain.error(region)
    }
    pub fn map_source(&self, region: &str) -> egui::ImageSource<'static> {
        self.terrain.source(region)
    }
    /// Always consume results, including while navigating, dragging or minimized.
    pub fn preload(&mut self, _ctx: &egui::Context) {
        self.terrain.poll();
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
        for region in ["palpagos", "worldtree"] {
            if self.get("map-images")[region].as_str() == Some(uri) {
                return self.map_source(region);
            }
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
    /// Species identities always open the same overview; explicit actions keep
    /// their own destinations (map, breeding, skills, and calculators).
    pub fn portrait_link(&self, ui: &mut egui::Ui, key: &str, size: f32) -> bool {
        let Some(p) = self.pal(key) else {
            return false;
        };
        let Some(uri) = self.get("portraits")[text(p, "en")].as_str() else {
            return false;
        };
        ui.add(
            egui::Image::new(self.image_source(uri))
                .fit_to_exact_size(Vec2::splat(size))
                .sense(egui::Sense::click()),
        )
        .on_hover_text(format!("查看{}的资料与功能入口", self.pal_name(key)))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
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
pub fn search(v: &Value, q: &str) -> bool {
    let query = normalize(q);
    if query.trim().is_empty() {
        return true;
    }
    let content = normalize(&v.to_string());
    query.split_whitespace().all(|word| content.contains(word))
}
pub(crate) fn capped_color_image(image: image::DynamicImage, max_side: u32) -> egui::ColorImage {
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
        // This test exercises the foreground raster loader only. Constructing
        // Data would additionally start both background terrain decoders and
        // leave unrelated decoding work competing with the next timed test.
        ctx.add_image_loader(Arc::new(CappedRasterLoader::default()));
        let uri = format!("file:///{}", root.join(maps["worldtree"].as_str().unwrap())
            .to_string_lossy().replace('\\', "/"));
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
    fn requested_map_finishes_while_pointer_keeps_moving() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let ctx = egui::Context::default();
        let mut data = Data::new(root, ctx.clone()).unwrap();
        assert!(!data.background_map_ready());
        let _ = ctx.run(
            egui::RawInput {
                max_texture_side: Some(1024),
                ..Default::default()
            },
            |_| {
                let _ = data.map_source("palpagos");
            },
        );
        let start = std::time::Instant::now();
        while !data.map_ready("palpagos") {
            let raw = egui::RawInput {
                max_texture_side: Some(1024),
                events: vec![egui::Event::PointerMoved(egui::pos2(20., 20.))],
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| data.preload(ctx));
            assert!(
                data.map_error("palpagos").is_none(),
                "{:?}",
                data.map_error("palpagos")
            );
            assert!(start.elapsed().as_secs() < 5);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let second = std::time::Instant::now();
        while !data.map_ready("worldtree") {
            let _ = ctx.run(
                egui::RawInput {
                    max_texture_side: Some(1024),
                    events: vec![egui::Event::PointerMoved(egui::pos2(20., 20.))],
                    ..Default::default()
                },
                |ctx| data.preload(ctx),
            );
            assert!(second.elapsed().as_secs() < 5);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(data.terrain.size("palpagos"), Some([1024, 1024]));
        assert_eq!(data.terrain.size("worldtree"), Some([1024, 1024]));
        assert!(matches!(
            data.map_source("palpagos"),
            egui::ImageSource::Texture(_)
        ));
    }
}
