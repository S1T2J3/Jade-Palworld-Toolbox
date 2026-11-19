//! Only the game's structured records are needed for a headless conversation.
#[path = "handbook_data.rs"]
mod handbook;

pub use handbook::{array, normalize, text};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path, sync::Arc};

const TABLES: &[&str] = &[
    "adventure-data",
    "capture-data",
    "combat-guide-data",
    "enemy-data",
    "expedition-data",
    "facilities-data",
    "item-catalog-data",
    "item-effects-data",
    "journals-data",
    "map-data",
    "pal-details-data",
    "paldex-data",
    "paldex-index-data",
    "partner-skills-data",
    "ranch-data",
    "relic-data",
    "resource-guides-data",
    "search-index-data",
    "spawn-data",
    "stat-data",
    "trait-data",
    "work-efficiency-data",
];

pub struct Data {
    pub root: std::path::PathBuf,
    pub(crate) catalog_text: std::sync::OnceLock<crate::assistant_discovery::CatalogText>,
    pub(crate) recipe_names: std::sync::OnceLock<crate::assistant_data::recipe_editions::RecipeNames>,
    pub species: Vec<Value>,
    pub pairs: Vec<Value>,
    pub tables: HashMap<String, Arc<Value>>,
    pub pending: usize,
    #[cfg(test)]
    pub errors: Vec<String>,
}

fn read<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> Result<T, String> {
    let bytes = fs::read(dir.join(format!("{name}.json")))
        .map_err(|_| format!("无法读取工具箱资料 {name}.json。"))?;
    serde_json::from_slice(&bytes).map_err(|_| format!("工具箱资料 {name}.json 格式有误。"))
}

impl Data {
    pub fn load(dir: &Path) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct SpeciesBook {
            species: Vec<Value>,
        }
        let SpeciesBook { species } = read(dir, "db")?;
        let pairs: Vec<Value> = read(dir, "pairs")?;
        handbook::validate_core_data(&species, &pairs)?;
        let mut tables = HashMap::with_capacity(TABLES.len());
        for &name in TABLES {
            tables.insert(name.to_owned(), Arc::new(read(dir, name)?));
        }
        Ok(Self {
            root: dir.to_owned(),
            catalog_text: Default::default(),
            recipe_names: Default::default(),
            species,
            pairs,
            tables,
            pending: 0,
            #[cfg(test)]
            errors: Vec::new(),
        })
    }

    #[cfg(test)]
    pub fn new(dir: std::path::PathBuf, _: eframe::egui::Context) -> Result<Self, String> {
        Self::load(&dir)
    }
    #[cfg(test)]
    pub fn poll(&mut self) {}
}
