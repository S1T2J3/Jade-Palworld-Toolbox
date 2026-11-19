use super::{has, table_row};
use crate::data::{Data, array, text};
use serde_json::Value;

fn item_name(d: &Data, book: &Value, id: &str) -> String {
    book["itemNames"][id]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| d.item_name(id))
}

/// Return a requested vendor/item intersection only when that exact item appears
/// in one of the vendor's actual stock lists.
pub(super) fn matched_shop_item<'a>(d: &'a Data, vendor: &str, q: &str) -> Option<&'a str> {
    let catalog = d.get("item-catalog-data");
    let item = array(&catalog["items"]).iter()
        .filter(|r| { let name = text(r,"name"); !name.is_empty() && q.contains(name) })
        .max_by_key(|r| text(r,"name").len())?;
    let id = text(item,"id");
    let book = d.get("enemy-data");
    let seller = table_row(&book["people"], vendor);
    array(&seller["shops"]).iter().filter_map(Value::as_str)
        .any(|shop| array(&book["shopLists"][shop]["rows"]).iter().any(|r| r[0] == id))
        .then_some(id)
}

// Preserve separate entries for the same item in different slots. If a player
// names a reward, prefer all matching rows to an arbitrary preview of the pool.
fn examples(entries: Vec<(String, String)>, q: &str) -> String {
    let longest = entries
        .iter()
        .filter(|(name, _)| !name.is_empty() && q.contains(name.as_str()))
        .map(|(name, _)| name.len())
        .max();
    entries
        .into_iter()
        .filter(|(name, _)| longest.is_none_or(|n| name.len() == n && q.contains(name.as_str())))
        .map(|(_, description)| description)
        .take(6)
        .collect::<Vec<_>>()
        .join("、")
}

fn rewards(d: &Data, book: &Value, rows: &Value, q: &str, slotted: bool) -> String {
    let entries = array(rows)
        .iter()
        .filter_map(|r| {
            let name = item_name(d, book, r[0].as_str()?);
            let (min, max, percent) = (r[1].as_u64()?, r[2].as_u64()?, r[3].as_f64()?);
            let quantity = if min == max {
                min.to_string()
            } else {
                format!("{min}–{max}")
            };
            let slot = r[4]
                .as_u64()
                .filter(|_| slotted)
                .map(|n| format!("栏位 {n} "))
                .unwrap_or_default();
            let detail = format!("{slot}{name} × {quantity}（{percent}%）");
            Some((name, detail))
        })
        .collect();
    examples(entries, q)
}

fn coordinates(p: &Value) -> Option<String> {
    let region = match text(p, "map") {
        "worldtree" => "世界树",
        "palpagos" => "帕洛斯群岛",
        other => other,
    };
    Some(format!(
        "{region}（{}，{}）",
        p["x"].as_i64()?,
        p["y"].as_i64()?
    ))
}

fn jobs(values: &Value) -> String {
    array(values)
        .iter()
        .filter_map(|r| Some(format!("{} Lv.{}", r[0].as_str()?, r[1].as_u64()?)))
        .collect::<Vec<_>>()
        .join("、")
}

fn unit(d: &Data, id: &str, kind: &str, q: &str) -> Option<String> {
    let book = d.get("enemy-data");
    let v = table_row(
        &book[if kind == "敌人" {
            "enemies"
        } else {
            "people"
        }],
        id,
    );
    if v.is_null() {
        return None;
    }
    let mut lines = vec![];
    if let Some(level) = v["level"].as_u64() {
        lines.push(format!("等级 {level}"));
    }
    if let Some(w) = v["weapon"].as_str() {
        lines.push(format!("使用{w}"));
    }
    if let Some(note) = v["note"].as_str() {
        lines.push(note.into());
    }
    let locations = array(&v["points"])
        .iter()
        .filter_map(coordinates)
        .take(4)
        .collect::<Vec<_>>();
    if !locations.is_empty() {
        lines.push(format!("收录位置有{}", locations.join("、")));
    } else if has(q, &["坐标", "哪里", "在哪", "位置"]) {
        lines.push("图鉴未收录具体坐标".into());
    }
    if kind == "商人" {
        for shop in array(&v["shops"]).iter().filter_map(Value::as_str) {
            let catalog = &book["shopLists"][shop];
            let entries = array(&catalog["rows"])
                .iter()
                .filter_map(|r| {
                    let name = item_name(d, book, r[0].as_str()?);
                    let currency = item_name(d, book, r[3].as_str()?);
                    let limit = if r[4] == true { "，限购一次" } else { "" };
                    let cost = format!(
                        "{name} × {}（{} {currency}{limit}）",
                        r[1].as_u64()?,
                        r[2].as_u64()?
                    );
                    Some((name, cost))
                })
                .collect();
            let stock = examples(entries, q);
            if !stock.is_empty() {
                lines.push(format!("{}有{stock}", text(catalog, "name")));
            }
        }
        let stock = &v["palStock"];
        if let (Some(count), Some(min), Some(max)) = (
            stock["count"].as_u64(),
            stock["min"].as_u64(),
            stock["max"].as_u64(),
        ) {
            let pals = array(&stock["pals"])
                .iter()
                .filter_map(Value::as_str)
                .map(|id| {
                    let name = d.pal_name(id);
                    (name.clone(), name)
                })
                .collect();
            lines.push(format!(
                "每批抽取 {count} 只，等级 {min}–{max}，候选包括{}",
                examples(pals, q)
            ));
        }
    }
    if kind == "中立单位" {
        let work = jobs(&v["work"]);
        if !work.is_empty() {
            lines.push(format!("工作适性有{work}"));
        }
        let requests = array(&v["requests"])
            .iter()
            .filter_map(|r| {
                let (subject, requirement) = if let Some(id) = r["pal"].as_str() {
                    let name = d.pal_name(id);
                    (name.clone(), format!("展示{name}"))
                } else {
                    let name = item_name(d, book, r["item"].as_str()?);
                    let qty = r["qty"].as_u64()?;
                    (name.clone(), format!("交付{name} × {qty}"))
                };
                let earned = array(&r["rewards"])
                    .iter()
                    .filter_map(|a| {
                        Some(format!(
                            "{} × {}",
                            item_name(d, book, a[0].as_str()?),
                            a[1].as_u64()?
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("、");
                Some((subject, format!("{requirement}可得{earned}")))
            })
            .collect();
        let requested = examples(requests, q);
        if !requested.is_empty() {
            lines.push(format!("委托包括{requested}"));
        }
    }
    if kind == "敌人" || has(q, &["掉落", "掉什么", "打死", "能掉"]) {
        let base = rewards(d, book, &v["drops"], q, false);
        if !base.is_empty() {
            lines.push(format!("基础掉落包括{base}"));
        }
        for tier in array(&v["dropTiers"]) {
            if let Some(level) = tier["level"].as_u64() {
                let drops = rewards(d, book, &tier["drops"], q, false);
                if !drops.is_empty() {
                    lines.push(format!("Lv.{level} 起的掉落包括{drops}"));
                }
            }
        }
        if v["drops"].is_array()
            && array(&v["drops"]).is_empty()
            && array(&v["dropTiers"]).is_empty()
        {
            lines.push("图鉴未收录直接掉落".into());
        }
    }
    for (field, label) in [("firstReward", "首次胜利奖励"), ("summonItem", "召唤所需")] {
        if let Some(key) = v[field].as_str() {
            lines.push(format!("{label}为{}", item_name(d, book, key)));
        }
    }
    (!lines.is_empty()).then(|| lines.join("。"))
}

fn loot(d: &Data, route: &str, q: &str) -> Option<String> {
    let book = d.get("adventure-data");
    let mut path = route.strip_prefix("#guide/loot/")?.split('/');
    let chapter = table_row(&book["chapters"], path.next()?);
    let pools = array(&chapter["pools"]);
    if pools.is_empty() {
        return None;
    }
    let pool = if let Some(id) = path.next() {
        Some(table_row(&chapter["pools"], id))
    } else {
        pools
            .iter()
            .filter(|p| !text(p, "name").is_empty() && q.contains(text(p, "name")))
            .max_by_key(|p| text(p, "name").len())
    };
    let chapter_name = text(chapter, "name");
    if let Some(p) = pool {
        let slotted = p["slotted"] == true;
        let list = rewards(d, book, &p["rows"], q, slotted);
        if list.is_empty() {
            return None;
        }
        let rule = if slotted {
            "以下概率已计入各栏位触发率，同一物品的不同栏位分别列出。"
        } else {
            "以下是奖励表记录的单项获得率。"
        };
        return Some(format!(
            "{chapter_name} · {}。{rule}可能获得{list}。",
            text(p, "name")
        ));
    }
    let names = pools
        .iter()
        .take(6)
        .map(|p| text(p, "name"))
        .collect::<Vec<_>>()
        .join("、");
    Some(format!(
        "{chapter_name}收录 {} 个奖池，包括{names}。需要查具体奖励或几率时，请指明奖池。",
        pools.len()
    ))
}

pub(super) fn summary(d: &Data, row: &Value, question: &str) -> Option<String> {
    let route = text(row, "href");
    let id = route.rsplit('/').next()?;
    match text(row, "kind") {
        kind @ ("敌人" | "中立单位" | "商人") => unit(d, id, kind, question),
        "探索奖励" => loot(d, route, question),
        "手记" => {
            let v = table_row(&d.get("journals-data")["catalog"], id);
            let summary = v["summary"].as_str()?;
            Some(match coordinates(v) {
                Some(site) => format!("{summary} 手记位置在{site}。"),
                None => summary.to_owned(),
            })
        }
        "据点生产" => {
            let v = table_row(&d.get("resource-guides-data")["production"], id);
            let output = v["itemName"].as_str()?;
            let mut facts = vec![format!("产物为{output}"), text(v, "condition").into()];
            if let Some(level) = v["level"].as_u64() {
                facts.push(format!("解锁科技等级 {level}"));
            }
            let work = jobs(&v["jobs"]);
            if !work.is_empty() {
                facts.push(format!("需要{work}"));
            }
            if let Some(power) = v["power"].as_f64().filter(|n| *n > 0.) {
                facts.push(format!("每秒消耗 {power} 电力"));
            }
            if let (Some(work), Some(speed)) =
                (v["work"].as_f64(), v["auto"].as_f64().filter(|n| *n > 0.))
            {
                facts.push(format!("表内基础生产周期约 {:.1} 秒", work / speed));
            }
            Some(facts.join(" "))
        }
        "远征" => {
            let v = table_row(&d.get("expedition-data")["stages"], id);
            let minutes = v["minutes"].as_u64()?;
            let power = v["power"].as_u64()?;
            let mut result = format!(
                "解锁条件为{}。基础时长 {minutes} 分钟，推荐战斗力 {power}。",
                text(v, "unlock")
            );
            if let (Some(element), Some(count)) =
                (v["element"][0].as_str(), v["element"][1].as_u64())
            {
                result.push_str(&format!("派遣条件为{element}属性 × {count}。"));
            }
            let list = rewards(d, d.get("adventure-data"), &v["rewards"], question, false);
            if !list.is_empty() {
                result.push_str(&format!("奖励举例（数量和概率均按单个栏位）有{list}。"));
            }
            Some(result)
        }
        "手术台" => {
            let v = &d.get("pal-details-data")["passives"][id];
            let op = &v["surgery"];
            let detail = match op["mode"].as_str()? {
                "consume" => format!("每次手术消耗 1 个「{}」", op["name"].as_str()?),
                "coins" => format!("手术台自带，每次花费 {} 金币", op["cost"].as_u64()?),
                "unlock" => format!(
                    "先获得「{}」解锁，之后每次花费 {} 金币",
                    op["name"].as_str()?,
                    op["cost"].as_u64()?
                ),
                _ => return None,
            };
            let mut answer = format!("{}。{detail}。", text(v, "effect"));
            if let Some(source) = op["source"].as_str() {
                answer.push_str(&format!("植入体来源为{source}。"));
            }
            Some(answer)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn reads_record_fields_across_handbook_sections() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "search-index-data",
            "enemy-data",
            "journals-data",
            "expedition-data",
            "adventure-data",
            "resource-guides-data",
            "pal-details-data",
            "item-catalog-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            d.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let index = array(d.get("search-index-data"));
        for (route, question, expected) in [
            (
                "#merchants/SalesPerson",
                "流浪商人卖的蛋要多少钱",
                &["蛋 × 1（200 金币）", "帕洛斯群岛"][..],
            ),
            (
                "#enemies/Hunter_Rifle",
                "盗猎集团 射手会掉什么",
                &["突击步枪", "金币 × 50–200（40%）"],
            ),
            (
                "#neutrals/MobuCitizen",
                "岛民会干哪种活",
                &["手工作业 Lv.1", "伐木 Lv.1"],
            ),
            (
                "#guide/journals/Day-xx",
                "这份手记在哪里",
                &["帕洛斯群岛（135，-471）", "研究者"],
            ),
            (
                "#work/expeditions/Expedition_Grass",
                "远征要多久",
                &["30 分钟", "推荐战斗力 25000", "单个栏位"],
            ),
            (
                "#work/production/OilPump",
                "原油提炼机怎么生产",
                &["建在原油油田上", "每秒消耗 2000 电力", "66.7 秒"],
            ),
            (
                "#guide/loot/SuppliesLoots",
                "草原空投补给能出十字弓设计图1吗",
                &["草原", "栏位 2 十字弓设计图1", "计入各栏位触发率"],
            ),
            (
                "#details/surgery/trait/CraftSpeed_up2",
                "工匠精神手术要多少金币",
                &["悬赏", "50000 金币", "植入体：工匠精神"],
            ),
        ] {
            let row = index.iter().find(|r| r["href"] == route).unwrap();
            let answer = summary(&d, row, question).unwrap();
            for needle in expected {
                assert!(answer.contains(needle), "{route}: {answer}");
            }
        }
    }
}
