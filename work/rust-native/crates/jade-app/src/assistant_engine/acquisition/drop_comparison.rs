use super::{
    AssistantReply, Data, Value, array, has, item_candidates, link, loot, occurs, pal_drops, text,
};
use crate::assistant_breeding::species_mentions;

fn joint_sources(d: &Data, q: &str) -> Option<[usize; 2]> {
    let (spans, _) = species_mentions(&d.species, q);
    let mut first_mentions = Vec::new();
    for span in spans {
        if !first_mentions.iter().any(|&(id, _, _)| id == span.0) {
            first_mentions.push(span);
        }
    }
    let [(a, _, end), (b, start, stop)] = first_mentions.as_slice() else {
        return None;
    };
    let between = q[*end..*start].trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, ',' | '，' | '、' | '?' | '？' | '.' | '。')
    });
    let after = q[*stop..].trim_start();
    // A helper, attacker, analogy or example in the background is not a second loot source.
    if has(
        between,
        &[
            "类似",
            "就像",
            "比如",
            "例如",
            "帮助",
            "帮我打",
            "对付",
            "像是",
            "such as",
            "similar to",
            "for example",
            "with the help of",
            "just like",
            "instead of using",
        ],
    ) {
        return None;
    }
    let joined = matches!(
        between,
        "和" | "与"
            | "跟"
            | "还是"
            | "或"
            | "或者"
            | "以及"
            | "对比"
            | "相比"
            | "vs"
            | "vs."
            | "or"
            | "and"
            | "versus"
            | "rather than"
            | "instead of"
            | "compared with"
            | "compared to"
            | "/"
            | ""
    );
    let other_also = has(
        after,
        &[
            "同样掉",
            "也掉",
            "同样能掉",
            "也能掉",
            "同样出",
            "也出",
            "不也掉",
            "也有掉",
            "gives the same drop",
            "also drops",
            "has the same drop",
            "drops the same",
            "can also drop",
            "also gives",
        ],
    );
    let comparing = has(
        q,
        &[
            "哪个",
            "哪种",
            "哪只",
            "两者",
            "两种",
            "两个",
            "各自",
            "分别",
            "相比",
            "对比",
            "比较",
            "更适合",
            "更合适",
            "更划算",
            "更划得来",
            "更容易",
            "一样",
            "同样",
            "副产物",
            "还是",
            "或者",
            "vs",
            "compare",
            "between",
            "which",
            "better",
            "same drop",
            "both",
            "easier",
            "side drop",
            "byproduct",
            " or ",
        ],
    );
    (comparing && (joined || other_also)).then_some([*a, *b])
}

fn requested_item<'a>(d: &'a Data, q: &str) -> Option<&'a Value> {
    let catalog = array(&d.get("item-catalog-data")["items"]);
    // In a decision such as "farm X, with a chance of Y", the item after the
    // secondary clause is a bonus even if its localized name happens to be longer.
    let boundary = [
        "副产物",
        "额外能",
        "还会掉",
        "还有概率",
        "顺带",
        "顺便",
        "with the chance",
        "with a chance",
        "also get",
        "bonus drop",
        "byproduct",
        "side drop",
        ", when ",
        "，而",
        "？",
    ]
    .iter()
    .filter_map(|p| q.find(p))
    .min()
    .unwrap_or(q.len());
    for part in [&q[..boundary], q] {
        let matches = item_candidates(catalog, part, has(part, &["设计图", "schematic"]));
        if let [item] = matches.as_slice() {
            return Some(item);
        }
    }
    None
}

fn combat_row<'a>(d: &'a Data, key: &str) -> Option<&'a [Value]> {
    let groups = array(&d.get("paldex-data")[key]["drops"]);
    groups.iter().find(|r| r[0].as_u64() == Some(0))?[1]
        .as_array()
        .map(Vec::as_slice)
}

fn breeding_note(d: &Data, target: usize) -> Option<String> {
    let pal = &d.species[target];
    let name = text(pal, "zh");
    let mut any = false;
    let mut only_pair = true;
    for row in &d.pairs {
        if row[2].as_u64() != Some(target as u64) {
            continue;
        }
        let first = row[0].as_u64()? as usize;
        let second = row[1].as_u64()? as usize;
        any = true;
        if first == target && second == target {
            continue;
        }
        only_pair = false;
        if first != target && second != target {
            let parent = |i, sex: &Value| -> Option<String> {
                let p: &Value = d.species.get(i)?;
                let suffix = match sex.as_str() {
                    Some("M") => "（公）",
                    Some("F") => "（母）",
                    _ => "",
                };
                Some(format!("{}{suffix}", text(p, "zh")))
            };
            return Some(format!(
                "{name}有无需现成{name}的配种组合，例如{} × {}。",
                parent(first, &row[3])?,
                parent(second, &row[4])?
            ));
        }
    }
    any.then(||if only_pair{
        format!("{name}的常规亲本组合仅列{name} × {name}，要靠这条路线持续供给，得先取得可配种的一对。")
    }else{format!("{name}的已录配种组合至少需要一方是{name}。")})
}

fn named_row<'a>(rows: &'a [Value], id: &str) -> Option<&'a Value> {
    rows.iter().find(|row| row[0].as_str() == Some(id))
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let butcher = has(
        q,
        &["解体", "分解帕鲁", "屠宰", "宰杀", "butcher", "disassembl"],
    );
    let combat = has(
        q,
        &["战利品", "掉落", "掉率", "掉什么", "掉", "drop", "loot"],
    );
    if !(butcher || combat)
        || has(
            q,
            &[
                "掉帧",
                "掉线",
                "frame drop",
                "掉落倍率",
                "掉落加成",
                "drop multiplier",
                "牧场产",
                "放牧",
                "at the ranch",
                "in the ranch",
                "partner skill",
            ],
        )
    {
        return None;
    }
    if !d.ready("item-catalog-data") || !d.ready("paldex-data") {
        return None;
    }
    let indices = joint_sources(d, q)?;
    let item = requested_item(d, q)?;
    let target = text(item, "id");
    let material = text(item, "name");
    // Species names inside an item name (e.g. Chikipi Poultry) are not candidates.
    let masked = [text(item, "name"), text(item, "en")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .fold(q.to_owned(), |s, alias| {
            s.replace(&super::field_text(alias), " ")
        });
    if masked != q && joint_sources(d, &masked) != Some(indices) {
        return None;
    }
    let special = has(
        q,
        &[
            "头目",
            "掠食",
            "狂暴",
            "领主",
            "捕食者",
            "alpha",
            "predator",
            "boss",
        ],
    );
    let mut lines = Vec::new();
    let mut answer = AssistantReply::new("", &[]);
    if special {
        lines.push(format!(
            "比较指定敌人形态的战斗掉落，目标为「{material}」。"
        ));
        for index in indices {
            let pal = &d.species[index];
            let section = pal_drops(d, pal, None, q);
            if !section.text.contains(material) {
                lines.push(format!(
                    "{}在该敌人形态的已录掉落中未列「{material}」。",
                    text(pal, "zh")
                ));
            }
            lines.push(section.text);
            for (label, url) in section.links {
                link(&mut answer, label, url);
            }
        }
    } else {
        lines.push(format!("按两种帕鲁的基础战斗掉落表比较「{material}」："));
        let mut primary = Vec::new();
        for &index in &indices {
            let pal = &d.species[index];
            let key = text(pal, "key");
            let name = text(pal, "zh");
            let base = combat_row(d, key)?;
            let entry = named_row(base, target);
            primary.push(entry);
            let amount = entry
                .and_then(|r| loot(d, r))
                .unwrap_or_else(|| format!("基础表未列「{material}」"));
            let mut byproducts: Vec<_> = base
                .iter()
                .filter(|r| r[0].as_str() != Some(target))
                .collect();
            byproducts.sort_by_key(|r| {
                let id = r[0].as_str().unwrap_or("");
                !occurs(q, &d.item_name(id)) && !occurs(q, &id.to_lowercase())
            });
            let extra = byproducts
                .iter()
                .filter_map(|r| loot(d, r))
                .collect::<Vec<_>>();
            let side = if extra.is_empty() {
                "无其他基础掉落记录".to_owned()
            } else {
                format!("另有{}", extra.join("、"))
            };
            lines.push(format!("{name}：{amount}。{side}。"));
            for group in array(&d.get("paldex-data")[key]["drops"]) {
                let Some(level) = group[0].as_u64().filter(|&v| v > 0) else {
                    continue;
                };
                let variant = named_row(array(&group[1]), target);
                if variant == entry {
                    continue;
                }
                let details = variant
                    .and_then(|r| loot(d, r))
                    .unwrap_or_else(|| format!("该档未列「{material}」"));
                lines.push(format!("{name} Lv.{level} 起的战斗档：{details}。"));
            }
            link(&mut answer, format!("{name}图鉴"), format!("#paldex/{key}"));
        }
        if primary.iter().all(Option::is_some) && primary[0] == primary[1] {
            lines.push(format!(
                "两者基础表中「{material}」的数量和概率相同。要兼收副产物，可以按上面各自的条目选。"
            ));
        }
    }
    if butcher {
        lines.push("以上数量和概率来自战斗掉落表。没有同等级、同设置的解体逐项记录可核对，不能将这些战斗数值直接作为解体收益。".into());
    }
    let getting = has(
        q,
        &[
            "更容易获取",
            "更好抓",
            "捕获难度",
            "野外捕捉",
            "easier to get",
            "easier to catch",
            "easier to obtain",
        ],
    );
    if getting {
        let spawn = d.get("spawn-data");
        for &index in &indices {
            let pal = &d.species[index];
            let key = text(pal, "key");
            let name = text(pal, "zh");
            if let [entry] = array(&spawn["bySpecies"][key])
                && entry[4].as_u64() == Some(1)
                && let (Some(low), Some(high)) = (entry[1].as_u64(), entry[2].as_u64())
                && low == high
            {
                lines.push(format!("{name}在当前出没表中收录的固定头目为 Lv.{low}。"));
                link(
                    &mut answer,
                    format!("{name}出没点"),
                    format!("#spawns/{key}"),
                );
            }
        }
    }
    if butcher || getting || has(q, &["配种", "繁殖", "育种", "供给", "breed", "supply"]) {
        lines.extend(indices.iter().filter_map(|&i| breeding_note(d, i)));
        lines.push("实际哪种更快还取决于已有亲本、捕捉或产蛋耗时和处理速度，当前资料没有可比较的每小时产量。".into());
    } else if super::asks_efficiency(q) {
        lines.push(
            "每小时能取得多少还取决于遭遇和击败每只所需的时间，单次掉落表无法核定效率。".into(),
        );
    }
    link(&mut answer, material, format!("#items/{target}"));
    answer.text = lines.join("\n");
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{Conversation, field_text};
    use std::{path::PathBuf, sync::Arc};
    fn handbook() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for n in [
            "paldex-data",
            "item-catalog-data",
            "enemy-data",
            "spawn-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{n}.json"))).unwrap();
            d.tables
                .insert(n.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        d
    }
    #[test]
    fn two_species_keep_their_own_loot_and_breeding_rules() {
        let d = handbook();
        for q in [
            "为啥刷超高热核心都推荐解体磐甲龙？空涡龙同样掉核心、还有概率拿钻石，又更容易获取，它不更合适吗？",
            "Why is Aegidron the go to to butcher for thermal cores, when Jetragon gives the same drops with the chance of a diamond. And is easier to get?",
        ] {
            let mut chat = Conversation::default();
            let r = chat.ask_world(q, &d, &serde_json::json!({}));
            assert!(
                chat.direct_result()
                    && r.text.contains("究极帕鲁之魂 × 1（100%）")
                    && r.text.contains("钻石 × 1（30%）"),
                "{}",
                r.text
            );
            assert_eq!(
                r.text.matches("超高热核心 × 1–2（100%）").count(),
                2,
                "{}",
                r.text
            );
            assert!(
                r.text.contains("空涡龙 × 空涡龙")
                    && r.text.contains("无需现成磐甲龙")
                    && r.text.contains("解体逐项记录")
                    && r.text.contains("每小时产量"),
                "{}",
                r.text
            );
        }
    }

    #[test]
    fn supports_other_materials_without_comparing_examples_or_helpers() {
        let d = handbook();
        for q in [
            "为羊毛解体棉悠悠还是米露菲，哪个更合适？",
            "Compare Lamball and Cremis drops for wool, including byproducts.",
        ] {
            let r = answer(&d, &field_text(q)).unwrap();
            assert!(
                r.text.contains("棉悠悠：羊毛 × 1–3（100%）")
                    && r.text.contains("米露菲：羊毛 × 1–2（100%）")
                    && r.text.contains("棉悠悠的羊肉"),
                "{}",
                r.text
            );
        }
        assert!(
            answer(
                &d,
                &field_text("Use Lamball to defeat Cremis for wool. Which drops can I get?")
            )
            .is_none()
        );
        assert!(
            answer(
                &d,
                &field_text("我说的掉落机制类似棉悠悠，不是在比较它。我想知道米露菲掉多少羊毛。")
            )
            .is_none()
        );
        assert!(
            answer(
                &d,
                &field_text("Is Lamball similar to Cremis when it drops wool?")
            )
            .is_none()
        );
        assert!(answer(&d, &field_text("棉悠悠和米露菲放在牧场哪个产羊毛快？")).is_none());
    }
}
