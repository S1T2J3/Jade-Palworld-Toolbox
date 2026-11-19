use super::{
    AssistantReply, Data, Source, Value, array, field_text, has, item_sources, link, occurs,
    pal_mentioned, text,
};
use crate::assistant_data::recipe_answer;

pub(crate) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let finding = has(
        q,
        &[
            "获取",
            "获得",
            "怎么得",
            "怎么弄",
            "哪里",
            "在哪",
            "哪刷",
            "怎么刷",
            "怎样刷",
            "怎么找",
            "哪里找",
            "从哪",
            "来源",
            "怎样做",
            "怎么做",
            "配方",
            "材料",
            "制作",
            "how",
            "where",
            "find",
            "farm",
            "get ",
            "obtain",
            "acquire",
            "recipe",
            "craft",
            "make ",
        ],
    );
    if !finding || !crate::assistant_discovery::handbook_version_matches(q) {
        return None;
    }
    let items = array(&d.get("item-catalog-data")["items"]);
    if has(q, &["觉醒", "觉醒石", "awakening", "awaken", "wakening"])
        && has(
            q,
            &[
                "晶石", "辉石", "石头", "结晶", "碎片", "材料", "crystal", "cristal", "gem",
                "stone", "shard", "material",
            ],
        )
    {
        return awakening(d, items, q);
    }
    let first = q.lines().next().unwrap_or(q);
    let books = has(
        first,
        &[
            "技能书",
            "工作适性书",
            "适性书",
            "工作手册",
            "技能手册",
            "工作教材",
            "工作教本",
            "适性教材",
            "skill book",
            "skillbook",
            "work manual",
            "work handbook",
            "work suitability book",
            "work suitability manual",
            "work suitability handbook",
            "work skill book",
        ],
    );
    if books && !has(q, &["技能果实", "skill fruit", "主动技能", "active skill"]) {
        return work_books(items, q);
    }
    None
}

fn awakening(d: &Data, items: &[Value], q: &str) -> Option<AssistantReply> {
    // The title or first question identifies the requested element. Later examples may name a different gem.
    let focus = q.split(['\n', '?', '？', '。']).next().unwrap_or(q);
    let gems: Vec<_> = items
        .iter()
        .filter(|r| text(r, "id").starts_with("PalAwakening_Material_"))
        .collect();
    let detect = |s: &str| -> Vec<&Value> {
        gems.iter()
            .copied()
            .filter(|r| {
                let en = text(r, "en")
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_lowercase();
                let zh = text(r, "name").split('之').next().unwrap_or_default();
                let element = match zh {
                    "雷" => "电",
                    "无" => "一般",
                    "暗" => "黑暗",
                    "地" => "大地",
                    _ => zh,
                };
                occurs(s, &en)
                    || [zh, element].iter().any(|&word| {
                        ["属性", "之", "系", "元素", "属", "辉石", "晶石", "觉醒"]
                            .iter()
                            .any(|suffix| s.contains(&format!("{word}{suffix}")))
                    })
            })
            .collect()
    };
    let mut selected = detect(focus);
    if selected.is_empty() {
        selected = detect(q);
    }
    if selected.is_empty() {
        return None;
    }
    let mut reply = AssistantReply::new("", &[]);
    for gem in selected {
        let element = text(gem, "id").strip_prefix("PalAwakening_Material_")?;
        let id = format!("PalAwakening_{element}");
        let product = items.iter().find(|r| text(r, "id") == id)?;
        let recipe = recipe_answer(d, product);
        if !reply.text.is_empty() {
            reply.text.push('\n');
        }
        reply.text.push_str(&recipe.text);
        for (label, href) in recipe.links {
            link(&mut reply, label, href);
        }
        let sources = item_sources(d, gem, Source::Any, q);
        reply.text.push('\n');
        reply.text.push_str(&sources.text);
        for (label, href) in sources.links {
            link(&mut reply, label, href);
        }
        if let Some(pal) = pal_mentioned(d, q) {
            let key = text(pal, "key");
            let mut drops = vec![];
            for group in array(&d.get("paldex-data")[key]["drops"]) {
                for row in array(&group[1]).iter().filter(|r| {
                    r[0].as_str()
                        .is_some_and(|v| v.starts_with("PalAwakening_Material_"))
                }) {
                    if let Some(detail) = super::loot(d, row) {
                        drops.push(format!("Lv.{} 起 {detail}", group[0]));
                    }
                }
            }
            if !drops.is_empty() {
                reply.text.push_str(&format!(
                    "\n{}的辉石掉落表分等级记录：{}。",
                    text(pal, "zh"),
                    drops.join("、")
                ));
                link(&mut reply, text(pal, "zh"), format!("#paldex/{key}"));
            }
        }
    }
    Some(reply)
}

fn work_books(items: &[Value], q: &str) -> Option<AssistantReply> {
    let all: Vec<_> = items
        .iter()
        .filter(|r| text(r, "id").starts_with("WorkSuitability_AddTicket_"))
        .collect();
    let named: Vec<_> = all
        .iter()
        .copied()
        .filter(|r| {
            let zh = text(r, "name")
                .trim_start_matches("应用")
                .split("学")
                .next()
                .unwrap_or_default();
            let en = field_text(text(r, "en"))
                .trim_start_matches("applied ")
                .split(" handbook")
                .next()
                .unwrap_or_default()
                .to_owned();
            !zh.is_empty() && q.contains(zh) || !en.is_empty() && occurs(q, &en)
        })
        .collect();
    let records = if named.is_empty() { &all } else { &named };
    let example = *records.first()?;
    let mut reply = AssistantReply::new("", &[]);
    let mut methods = vec![];
    for row in array(&example["methods"]) {
        let (Some(kind), Some(place)) = (row[0].as_str(), row[1].as_str()) else {
            continue;
        };
        if records.iter().all(|r| {
            array(&r["methods"])
                .iter()
                .any(|s| s[0] == kind && s[1] == place)
        }) {
            methods.push((kind, place));
        }
    }
    let mut lines = vec![];
    for kind in [
        "空投补给",
        "藏宝图",
        "海上打捞",
        "地下城宝箱",
        "敌方营地",
        "野外钓鱼",
        "属性宝箱",
    ] {
        let examples = methods
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, v)| *v)
            .take(3)
            .collect::<Vec<_>>();
        if !examples.is_empty() {
            lines.push(format!("{kind}（例如{}）", examples.join("、")));
        }
    }
    if lines.is_empty() {
        return None;
    }
    let labels = records
        .iter()
        .map(|r| text(r, "name"))
        .collect::<Vec<_>>()
        .join("、");
    reply.text = format!(
        "如果指提升工作适性的应用学教材，{labels}的共同获取记录有{}。可以选一个有空投补给记录的区域持续收补给，途中兼顾其他已收录的奖励。实际效率取决于补给间隔和路线，图鉴没有可比的每小时产量。",
        lines.join("；")
    );
    for r in records {
        link(
            &mut reply,
            text(r, "name"),
            format!("#items/{}", text(r, "id")),
        );
    }
    if named.is_empty() {
        reply
            .text
            .push_str("\n若指提高经验的修炼之书，或获得科技点的技术书，需要查另一类物品。");
        for id in ["ExpBoost_01", "TechnologyBook_G1"] {
            if let Some(r) = items.iter().find(|r| text(r, "id") == id) {
                link(&mut reply, text(r, "name"), format!("#items/{id}"));
            }
        }
    }
    Some(reply)
}
