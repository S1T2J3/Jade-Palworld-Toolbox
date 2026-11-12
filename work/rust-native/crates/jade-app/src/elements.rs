//! In-game element emblems, copied byte-for-byte from the existing asset archive.
use eframe::egui::{self, Vec2};

pub const KEYS: &[&str] = &[
    "neutral", "fire", "water", "grass", "electric", "ice", "ground", "dark", "dragon",
];

/// These textures already contain the game's colours and diamond background.
/// Callers must not multiply them by the UI accent or a single element colour.
pub fn source(key: &str) -> Option<egui::ImageSource<'static>> {
    Some(match key {
        "neutral" => egui::include_image!("../assets/game-elements/Normal.png"),
        "fire" => egui::include_image!("../assets/game-elements/Fire.png"),
        "water" => egui::include_image!("../assets/game-elements/Water.png"),
        "grass" => egui::include_image!("../assets/game-elements/Leaf.png"),
        "electric" => egui::include_image!("../assets/game-elements/Electricity.png"),
        "ice" => egui::include_image!("../assets/game-elements/Ice.png"),
        "ground" => egui::include_image!("../assets/game-elements/Earth.png"),
        "dark" => egui::include_image!("../assets/game-elements/Dark.png"),
        "dragon" => egui::include_image!("../assets/game-elements/Dragon.png"),
        _ => return None,
    })
}

pub fn image(ui: &mut egui::Ui, key: &str, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    if let Some(source) = source(key) {
        egui::Image::new(source).paint_at(ui, rect);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn all_nine_game_elements_decode_as_square_coloured_images() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let hashes = [
            "113cf8d8a8423a975120817083f394beec947fc525ce523cf8ca5a813e567fd5",
            "8e0b4177b9cef219f78c6811ff2283f27442b1685ff0912d2d24634ddc8be638",
            "361a6182daeaffb59c35569920b7401b6e5d842b34f1d5f67d8809e832a2250c",
            "d6ec3f06d848710d2a63cd9f209e90551a0b5ecb0e297a6767e064adcba9ca78",
            "50ec6ff342d880d3ff3ce66869b7f2ee5deef15ae20132afad1c24a951363d05",
            "9fba55243f7740db5e6445c6e45689e5259fb4e715c6f2c04a9620b55a3e0138",
            "6bff1663520097c13059b211b76d9b639e5d388104eed609b63875e2e66dbff0",
            "c016b3804b94f17e9f0200df545c52f8c41c238a0d37520e2e72be25deafd547",
            "75e973738d901e30eaf19af296ac4d2d0e1bff46a104f57fe7aee171833444e5",
        ];
        for (key, hash) in KEYS.iter().zip(hashes) {
            let source = source(key).unwrap();
            let egui::ImageSource::Bytes { bytes, .. } = source.clone() else {
                panic!("element must be bundled bytes");
            };
            let decoded = image::load_from_memory(&bytes).unwrap().into_rgba8();
            assert_eq!(decoded.width(), decoded.height(), "{key}");
            assert!(decoded.width() >= 32, "{key}");
            assert!(
                decoded.pixels().any(|p| p[3] == 0),
                "transparent diamond corners: {key}"
            );
            assert!(
                decoded.pixels().any(|p| p[3] > 200),
                "visible emblem: {key}"
            );
            assert!(
                decoded
                    .pixels()
                    .map(|p| p.0)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    > 16,
                "shaded game texture: {key}"
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                hash,
                "unchanged archive image: {key}"
            );
            let mut bounds = None;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| bounds = Some(image(ui, key, 18.).rect));
            });
            assert_eq!(bounds.unwrap().size(), Vec2::splat(18.));
        }
        assert!(source("invalid").is_none());
    }
}
