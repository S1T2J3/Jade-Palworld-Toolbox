use super::{AssistantReply, Data, Value, array, has, link, text};
use crate::assistant_breeding::species_mentions;
use std::collections::BTreeSet;

fn base_products(row: &Value) -> Option<BTreeSet<&str>> {
    let tier = row["base"].as_u64()?.checked_sub(1)? as usize;
    let products = array(row["tiers"].get(tier)?)
        .iter()
        .filter(|r| r["chance"].as_f64().unwrap_or(0.) > 0.)
        .filter_map(|r| r["item"].as_str())
        .collect::<BTreeSet<_>>();
    (!products.is_empty()).then_some(products)
}

pub(in crate::assistant_engine) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let ranch = has(q, &["放牧", "牧场", "牧羊", "ranch", "graze", "grazing"]);
    let equivalent = has(
        q,
        &[
            "替代",
            "替换",
            "代替",
            "还有哪只",
            "还有谁",
            "还有什么帕鲁",
            "有别的帕鲁",
            "其他帕鲁",
            "other pal",
            "another pal",
            "replace",
            "alternative",
            "substitute",
        ],
    );
    let same_action = has(
        q,
        &[
            "做同样的",
            "一样的事",
            "同样的事",
            "功能一样",
            "一样的效果",
            "same thing",
            "the same",
            "do what",
            "what it does",
            "same job",
            "same as",
        ],
    );
    if !equivalent
        || !(ranch || same_action)
        || has(
            q,
            &[
                "捕获率",
                "抓捕",
                "战斗",
                "骑乘",
                "坐骑",
                "配种",
                "配方",
                "recipe",
                "速度",
                "最多",
                "最高",
                "概率",
                "产量",
                "接力",
                "继承",
                "combat",
                "mount",
                "breed",
                "chance",
                "faster",
                "fastest",
                "yield",
                "per hour",
                "quantity",
                "at max",
                "满星",
                "maximum",
                "maxed",
                "four stars",
                "多掉",
            ],
        )
    {
        return None;
    }
    let (owners, _) = species_mentions(&d.species, q);
    let [(index, _, _)] = owners.as_slice() else {
        return None;
    };
    let pal = &d.species[*index];
    let key = text(pal, "key");
    let records = d.get("ranch-data")["pals"].as_object()?;
    let desired = base_products(records.get(key)?)?;
    let mut lines = vec![format!(
        "如果你指在家畜牧场放牧，{}的基础档产物有{}。",
        text(pal, "zh"),
        desired
            .iter()
            .map(|id| d.item_name(id))
            .collect::<Vec<_>>()
            .join("、")
    )];
    let mut complete = Vec::new();
    let mut overlaps = Vec::new();
    for (other, row) in records {
        if other == key {
            continue;
        }
        let products = base_products(row)?;
        let shared = desired
            .intersection(&products)
            .map(|id| d.item_name(id))
            .collect::<Vec<_>>();
        if shared.len() == desired.len() {
            complete.push(d.pal_name(other));
        } else if !shared.is_empty() {
            overlaps.push(format!("{}（{}）", d.pal_name(other), shared.join("、")));
        }
    }
    lines.push(if complete.is_empty() {
        "基础档放牧表里，没有第二种帕鲁能产出上述全部物品。".into()
    } else {
        format!("只想收齐上述基础产物时，{}也能做到。", complete.join("、"))
    });
    if !overlaps.is_empty() {
        lines.push(format!(
            "部分产物也能从这些帕鲁获得：{}。",
            overlaps.join("、")
        ));
    }
    if !ranch {
        lines.push("如果你想比较战斗、采集或其他用途，可以指定要替代的那项能力。".into());
    }
    let mut reply = AssistantReply::new(&lines.join("\n"), &[]);
    link(&mut reply, text(pal, "zh"), format!("#paldex/{key}"));
    for product in desired {
        link(
            &mut reply,
            d.item_name(product),
            format!("#work/ranch/item/{product}"),
        );
    }
    Some(reply)
}
