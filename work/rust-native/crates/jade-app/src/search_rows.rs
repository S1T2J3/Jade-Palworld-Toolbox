//! Search-only map rows. Game records, displayed names and coordinates stay unchanged.
use crate::data::{Data, array, text};
use serde_json::{Value, json};
use std::collections::HashMap;

pub fn all(d: &Data) -> Value {
    let mut rows = array(d.get("search-index-data")).to_vec();
    append_map(
        &mut rows,
        d.get("map-data"),
        d.get("map-images"),
        d.get("db"),
        d.get("portraits"),
    );
    Value::Array(rows)
}

fn append_map(rows: &mut Vec<Value>, map: &Value, images: &Value, db: &Value, portraits: &Value) {
    let categories: HashMap<_, _> = array(&map["categories"])
        .iter()
        .map(|c| (text(c, "id"), c))
        .collect();
    let pal_art: HashMap<_, _> = array(&db["species"])
        .iter()
        .filter_map(|p| {
            portraits[text(p, "en")]
                .as_str()
                .map(|uri| (text(p, "key"), uri))
        })
        .collect();
    let mut counts = HashMap::<(&str, &str), usize>::new();
    for p in array(&map["points"]) {
        let id = text(p, "id");
        let cat = text(p, "category");
        let region = text(p, "map");
        if id.is_empty() || !["palpagos", "worldtree"].contains(&region) {
            continue;
        }
        *counts.entry((cat, region)).or_default() += 1;
        let cat_name = categories.get(cat).map(|c| text(c, "name")).unwrap_or(cat);
        let art = if cat == "alpha_pals" {
            pal_art.get(text(p, "palKey")).copied().unwrap_or("")
        } else {
            ""
        };
        let art = if art.is_empty() {
            category_image(images, cat, text(p, "iconKey"))
        } else {
            art
        };
        rows.push(json!({"name":text(p,"name"),"kind":"地图点位","href":format!("#map/point/{id}"),
            "category":cat,"image":art,
            "hint":format!("坐标 {}，{} · {} · {cat_name} · {}",coordinate(&p["x"]),coordinate(&p["y"]),text(p,"nearest"),region_name(region)),
            "aliases":[id,cat,cat_name,text(p,"nearest"),text(p,"en"),text(p,"detail")] }));
    }
    for c in array(&map["categories"]) {
        let id = text(c, "id");
        for region in ["palpagos", "worldtree"] {
            let count = counts.get(&(id, region)).copied().unwrap_or(0);
            if count == 0 {
                continue;
            }
            rows.push(
                json!({"name":text(c,"name"),"kind":"地图分类","category":id,
                "href":format!("#map/category/{id}/{region}"),
                "image":category_image(images,id,""),
                "hint":format!("{} · {count} 处点位 · 在地图中查看全部",region_name(region)),
                "aliases":[id,text(c,"name"),region_name(region)]}),
            );
        }
    }
}
fn category_image<'a>(images: &'a Value, category: &str, explicit: &str) -> &'a str {
    if let Some(uri) = images["minerals"][category].as_str() {
        return uri;
    }
    let key = if explicit.is_empty() {
        images["mapIcons"]["defaults"][category]
            .as_str()
            .unwrap_or("")
    } else {
        explicit
    };
    images["mapIcons"]["art"][key].as_str().unwrap_or("")
}
fn region_name(id: &str) -> &str {
    if id == "worldtree" {
        "世界树"
    } else {
        "帕洛斯群岛"
    }
}
fn coordinate(v: &Value) -> String {
    v.as_f64().map(|x| format!("{x:.0}")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_map_points_keep_distinct_coordinates_and_open_the_existing_routes() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let map: Value =
            serde_json::from_slice(&std::fs::read(root.join("map-data.json")).unwrap()).unwrap();
        let mut rows = vec![];
        append_map(&mut rows, &map, &Value::Null, &Value::Null, &Value::Null);
        let air: Vec<_> = rows
            .iter()
            .filter(|r| r["category"] == "anti_air" && r["kind"] == "地图点位")
            .collect();
        assert_eq!(air.len(), 11);
        let unique: std::collections::HashSet<_> = air.iter().map(|r| text(r, "href")).collect();
        assert_eq!(unique.len(), 11);
        for r in air {
            let id = text(r, "href").strip_prefix("#map/point/").unwrap();
            let source = array(&map["points"])
                .iter()
                .find(|p| text(p, "id") == id)
                .unwrap();
            assert_eq!(r["name"], source["name"]);
            assert!(text(r, "hint").contains(&coordinate(&source["x"])));
            assert!(text(r, "hint").contains(&coordinate(&source["y"])));
        }
        assert!(
            rows.iter()
                .any(|r| r["href"] == "#map/category/anti_air/palpagos")
        );
    }
}
