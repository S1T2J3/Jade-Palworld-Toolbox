//! Compare normal ranch output and combat drops for the same species.
use super::{AssistantReply, Data, array, has, link, text};
use crate::assistant_breeding::species_mentions;
use crate::assistant_engine::facility_effect;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::assistant_engine) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let title = q.lines().next()?;
    if !has(title, &["牧场", "放牧", "ranch", "grazing"])
        || !has(
            title,
            &[
                "产什么",
                "产出",
                "生产什么",
                "产哪些",
                "能产",
                "会产",
                "只给",
                "能给",
                "出什么",
                "能得到",
                "能获得",
                "收获",
                "捡到",
                "拿到",
                "放牧得",
                "掉什么",
                "掉落",
                "战利品",
                "拾取",
                "collect",
                "produce",
                "drop",
                "loot",
                "get from",
            ],
        )
        || has(
            title,
            &[
                "配种牧场",
                "繁殖牧场",
                "头目",
                "领主",
                "掠食",
                "捕食者",
                "每小时",
                "几率",
                "多少",
                "概率",
                "适性几级",
                "替代",
                "种田",
                "最快",
                "多久",
                "工作速度",
                "搬运",
                "不捡",
                "不拾",
                "堆着",
                "升星",
                "更多",
                "变多",
                "breeding farm",
                "alpha",
                "boss",
                "predator",
                "per hour",
                "chance",
                "probability",
                "how many",
                "how much",
                "replace",
                "alternative",
                "work speed",
            ],
        )
    {
        return None;
    }
    let includes_wild = has(
        title,
        &[
            "野外",
            "野生",
            "击败",
            "击倒",
            "打倒",
            "打死",
            "打败",
            "狩猎",
            "战利品",
            "打怪",
            "捕获",
            "战斗掉落",
            "wild",
            "combat",
            "defeat",
            "caught",
            "catching",
            "hunting",
        ],
    );
    let (hits, _) = species_mentions(&d.species, title);
    // In "my Lamball ... Mozzarina meat", the meat item contains a second species name.
    let owners: BTreeSet<_> = hits
        .iter()
        .filter(|(_, _, end)| {
            ![
                "肉",
                "的牛肉",
                "的羊肉",
                "的鸡肉",
                "的鱼肉",
                " meat",
                " poultry",
                " mutton",
            ]
            .iter()
            .any(|suffix| title[*end..].starts_with(suffix))
        })
        .map(|m| m.0)
        .collect();
    let owner = if owners.len() == 1 {
        *owners.first()?
    } else {
        return None;
    };
    let pal = &d.species[owner];
    let key = text(pal, "key");
    let ranch_table = d.get("ranch-data")["pals"].as_object()?;
    let mut ranch_ranks = BTreeMap::new();
    let mut base = 0;
    if let Some(row) = ranch_table.get(key) {
        base = row["base"].as_u64()?;
        let max = row["condensed"].as_u64()?;
        for (i, tier) in row["tiers"].as_array()?.iter().enumerate() {
            let rank = i as u64 + 1;
            if !(base..=max).contains(&rank) {
                continue;
            }
            for product in array(tier)
                .iter()
                .filter(|r| r["chance"].as_f64().is_some_and(|v| v > 0.0))
            {
                ranch_ranks.entry(product["item"].as_str()?).or_insert(rank);
            }
        }
    }
    let mut wild = BTreeMap::new();
    if includes_wild {
        for group in d.get("paldex-data")[key]["drops"].as_array()? {
            let min_level = group[0].as_u64()?;
            for drop in array(&group[1])
                .iter()
                .filter(|r| r[3].as_f64().is_some_and(|v| v > 0.0))
            {
                wild.entry(drop[0].as_str()?)
                    .and_modify(|n: &mut u64| *n = (*n).min(min_level))
                    .or_insert(min_level);
            }
        }
    }
    let catalog = d.get("item-catalog-data")["items"].as_array()?;
    let mut wanted: BTreeSet<_> = facility_effect::items(d, title)
        .into_iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    // Speech often omits the possessive from cuts, e.g. 波霸牛的牛肉 / 波霸牛肉.
    if let Some(meat) = catalog
        .iter()
        .find(|i| text(i, "id").strip_prefix("Meat_") == Some(key))
    {
        let cut = text(meat, "name").split_once('的').map(|(_, s)| s);
        if cut.is_some_and(|s| title.contains(s)) {
            wanted.insert(text(meat, "id"));
        }
    }
    if has(title, &["鸡蛋", "禽蛋"]) {
        wanted.insert("Egg");
    }
    if wanted.is_empty() {
        wanted.extend(ranch_ranks.keys().chain(wild.keys()).copied());
    }
    if wanted.is_empty() {
        return None;
    }
    let english = title.is_ascii();
    let pal_name = text(pal, if english { "en" } else { "zh" });
    let mut reply = AssistantReply::new("", &[]);
    let mut lines = Vec::new();
    for id in wanted {
        let item = catalog.iter().find(|i| i["id"] == id)?;
        let name = text(item, if english { "en" } else { "name" });
        let farm = match ranch_ranks.get(id) {
            Some(&rank) if rank > base => {
                if english {
                    format!("ranch output from grazing rank {rank}")
                } else {
                    format!("牧场适性 {rank} 级起有产出")
                }
            }
            Some(_) => {
                if english {
                    "produced at the ranch".into()
                } else {
                    "放牧可产出".into()
                }
            }
            None => {
                if english {
                    "no ranch output recorded".into()
                } else {
                    "牧场产物表中没有".into()
                }
            }
        };
        let mut line = format!("{name} — {farm}");
        if includes_wild {
            let result = match wild.get(id) {
                Some(&0) => {
                    if english {
                        "listed in normal wild drops".into()
                    } else {
                        "普通野外掉落表中有".into()
                    }
                }
                Some(level) => {
                    if english {
                        format!("wild drop from level {level}")
                    } else {
                        format!("野外 {level} 级起列有掉落")
                    }
                }
                None => {
                    if english {
                        "no normal wild drop recorded".into()
                    } else {
                        "普通野外掉落表中没有".into()
                    }
                }
            };
            line.push_str(&format!(" / {result}"));
        }
        lines.push(line);
        link(&mut reply, name, format!("#items/{id}"));
    }
    link(&mut reply, pal_name, format!("#paldex/{key}"));
    reply.text = format!("{pal_name}\n{}", lines.join("\n"));
    Some(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{Conversation, field_text};
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn farmed_items_and_combat_loot_are_bound_to_one_pal() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        for table in ["item-catalog-data", "ranch-data", "paldex-data"] {
            let raw = std::fs::read(dir.join(format!("{table}.json"))).unwrap();
            d.tables.insert(
                table.into(),
                Arc::new(serde_json::from_slice(&raw).unwrap()),
            );
        }
        let ask = |q| {
            Conversation::default()
                .ask_world(q, &d, &serde_json::json!({}))
                .text
        };
        let cow = ask("波霸牛在牧场能捡到牛奶和波霸牛肉吗？哪种只能靠野外掉落？");
        assert!(
            cow.contains("牛奶 — 放牧可产出 / 普通野外掉落表中有"),
            "{cow}"
        );
        assert!(
            cow.contains("波霸牛的牛肉 — 牧场产物表中没有 / 普通野外掉落表中有"),
            "{cow}"
        );
        let chicken = ask("皮皮鸡的鸡蛋和鸡肉都是放牧产出的吗？野外还能掉哪些？");
        assert!(
            chicken.contains("蛋 — 放牧可产出")
                && chicken.contains("皮皮鸡的鸡肉 — 牧场产物表中没有"),
            "{chicken}"
        );
        let lamb = ask("棉悠悠在牧场能捡到波霸牛的牛肉吗，还是野外有？");
        assert!(
            lamb.contains("棉悠悠")
                && lamb.contains("波霸牛的牛肉 — 牧场产物表中没有 / 普通野外掉落表中没有"),
            "{lamb}"
        );
        assert!(answer(&d, &field_text("棉悠悠和波霸牛的牧场适性几级？")).is_none());
        assert!(answer(&d, &field_text("波霸牛在配种牧场会产什么蛋？")).is_none());
    }
}
