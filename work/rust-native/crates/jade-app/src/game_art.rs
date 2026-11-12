//! Verified supplemental game art, embedded for offline and on-demand display.
use eframe::egui;

pub fn enemy_source(id: &str) -> Option<egui::ImageSource<'static>> {
    match id {
        "RAID_YakushimaBoss002" | "RAID_YakushimaBoss002_2" => Some(egui::include_image!(
            "../assets/supplemental/YakushimaBoss002.webp"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn verified_moon_lord_asset_is_unmodified_and_not_used_for_another_boss() {
        let bytes = include_bytes!("../assets/supplemental/YakushimaBoss002.webp");
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "fb1363926afc21a6c1b6926f9b7f5c5de104d0c933f4a4ddf0e30e017f6ee927"
        );
        let image = image::load_from_memory(bytes).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (256, 256));
        assert!(image.pixels().any(|p| p[3] == 0));
        assert!(image.pixels().filter(|p| p[3] > 0).count() > 10000);
        assert!(enemy_source("RAID_YakushimaBoss002").is_some());
        assert!(enemy_source("RAID_YakushimaBoss002_2").is_some());
        assert!(enemy_source("YakushimaBoss002_Mask").is_none());
        assert!(enemy_source("DarkMutant").is_none());
    }
}
