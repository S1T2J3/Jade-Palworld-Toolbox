//! Reverse lookup over gameplay descriptions when the player does not know an item name.
use super::{AssistantReply, Data, array, text};
use std::collections::{BTreeSet, HashMap, HashSet};

struct Record {
    row: usize,
    description: String,
}

#[derive(Default)]
pub(crate) struct CatalogText {
    records: Vec<Record>,
    postings: HashMap<String, Vec<usize>>,
    pub(super) bilingual: std::sync::OnceLock<super::bilingual::Index>,
    pub(super) semantic: std::sync::OnceLock<Option<super::semantic::Index>>,
}

fn tokens(value: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut previous = None;
    for ch in value.chars() {
        if ('\u{3400}'..='\u{9fff}').contains(&ch) {
            if let Some(prior) = previous {
                result.insert(format!("{prior}{ch}"));
            }
            previous = Some(ch);
        } else {
            previous = None;
        }
    }
    for word in value
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() > 2)
    {
        let w = word.to_ascii_lowercase();
        if !matches!(
            w.as_str(),
            "the"
                | "what"
                | "when"
                | "for"
                | "and"
                | "can"
                | "how"
                | "does"
                | "this"
                | "that"
                | "pal"
                | "pals"
                | "which"
                | "game"
                | "have"
                | "with"
                | "about"
                | "any"
        ) {
            result.insert(w);
        }
    }
    result
}

impl CatalogText {
    pub(super) fn build(d: &Data) -> Self {
        let mut index = Self::default();
        // The collection is built from the tables this kernel actually opened.
        let facilities: HashMap<_, _> = array(&d.get("facilities-data")["facilities"])
            .iter()
            .map(|p| (text(p, "id"), text(p, "purpose")))
            .collect();
        let statuses: HashMap<_, _> = array(&d.get("combat-guide-data")["status"])
            .iter()
            .map(|p| (text(p, "id"), text(p, "text")))
            .collect();
        for (row, entry) in array(d.get("search-index-data")).iter().enumerate() {
            let Some(id) = text(entry, "href").rsplit('/').next() else {
                continue;
            };
            let body = match text(entry, "kind") {
                "物品" => text(&d.get("item-effects-data")["items"][id], "description"),
                "设施" => facilities.get(id).copied().unwrap_or(""),
                "伙伴技能" => text(&d.get("partner-skills-data")[id], "effect"),
                "异常状态" => statuses.get(id).copied().unwrap_or(""),
                "被动词条" => text(entry, "hint"),
                _ => continue,
            };
            if body.is_empty() {
                continue;
            }
            let mut words = tokens(body);
            words.extend(tokens(text(entry, "name")));
            for alias in array(&entry["aliases"]).iter().filter_map(|v| v.as_str()) {
                words.extend(tokens(alias));
            }
            let i = index.records.len();
            for token in words {
                index.postings.entry(token).or_default().push(i);
            }
            index.records.push(Record {
                row,
                description: body.into(),
            });
        }
        index
    }

    pub(super) fn descriptions<'a>(&'a self, d: &'a Data) -> impl Iterator<Item = (&'a serde_json::Value, &'a str)> {
        let rows = array(d.get("search-index-data"));
        self.records.iter().map(move |record| (&rows[record.row], record.description.as_str()))
    }
}

/// Compare explicitly named items using their descriptions. A shared Chinese
/// name can cover several rarity tiers, so quote only the common description.
pub(super) fn item_effect_comparison(d: &Data, q: &str) -> Option<AssistantReply> {
    if !super::has(q, &["哪件", "哪一个", "哪个", "哪种", "哪一件", "各有什么效果", "效果有什么区别"])
        || q.contains('\n') || !super::handbook_version_matches(q)
        || super::has(q, &["制作", "材料", "哪里", "获得", "解锁", "比较贵", "几级", "多少", "叠加", "免疫", "百分比", "上限", "更高", "更强", "划算"]) {
        return None;
    }
    if !d.ready("search-index-data") || !d.ready("item-effects-data") { return None; }
    let rows = array(d.get("search-index-data"));
    let subjects = super::relevant_names(rows, q);
    if subjects.len() != 2 || subjects.iter().any(|r| text(r,"kind") != "物品") { return None; }
    let names: Vec<_> = subjects.iter().map(|r| text(r,"name")).collect();
    if names[0] == names[1] { return None; }
    let mut focus = q.to_owned();
    let mut longest_first = names.clone();
    longest_first.sort_by_key(|name| std::cmp::Reverse(name.len()));
    for name in longest_first { focus = focus.replace(name, " "); }
    let all_effects = super::has(&focus, &["各有什么效果", "效果有什么区别", "各有什么作用"]);
    // The comparison must concern an effect actually recorded in one of the
    // two descriptions. A question about where to find an item is different.
    let attributes: &[(&[&str], &[&str])] = &[
        (&["耐热","耐暑","抗热","怕热","高温","酷暑","冷热","寒热","两种温度","两种气温","昼夜温差"], &["耐热","耐暑","抵御炎热"]),
        (&["耐寒","防寒","抗寒","低温","寒冷","冷热","寒热","两种温度","两种气温","昼夜温差"], &["耐寒","防寒","抵御寒冷"]),
        (&["攻击","增伤"], &["攻击"]),
        (&["防御"], &["防御"]),
        (&["负重"], &["负重"]),
        (&["工作速度"], &["工作速度"]),
        (&["移动速度","跑速"], &["移动速度"]),
    ];
    let goals: Vec<_> = attributes.iter().filter(|(markers,_)| super::has(&focus,markers)).collect();
    if goals.is_empty() && !all_effects { return None; }
    let effects = &d.get("item-effects-data")["items"];
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut descriptions = Vec::new();
    for (row, name) in subjects.iter().zip(names) {
        let route = text(row, "href");
        let description = text(&effects[route.strip_prefix("#items/")?], "description");
        if description.is_empty() || rows.iter().filter(|r| text(r,"kind")=="物品" && text(r,"name")==name)
            .any(|r| text(&effects[text(r,"href").trim_start_matches("#items/")], "description") != description) {
            return None;
        }
        lines.push(format!("{name}：{description}"));
        descriptions.push(description);
        links.push((name.to_owned(), route.to_owned()));
    }
    if !all_effects && !goals.iter().all(|(_, terms)| descriptions.iter().any(|d| super::has(d,terms))) {
        return None;
    }
    Some(AssistantReply { text: lines.join("\n"), links, suggestions: Vec::new() })
}

pub(super) fn lookup(d: &Data, question: &str, terms: &[String]) -> Option<AssistantReply> {
    if !d.ready("search-index-data") || !d.ready("item-effects-data") {
        return None;
    }
    let index = d.catalog_text.get_or_init(|| CatalogText::build(d));
    let q = crate::assistant_engine::field_text(question);
    let mut asked = tokens(&q);
    // Short English descriptions often refer to a mechanic rather than a catalog name.
    for (phrase, localized) in [
        ("food slot", "食物栏位"),
        ("feed bag", "饲料袋 自动进食"),
        ("hunger", "饥饿"),
        ("spoil", "腐坏 腐败"),
        ("refriger", "冷藏 保鲜"),
        ("carry weight", "负重上限"),
        ("individual value", "隐藏才能 潜力"),
        ("potential", "潜力"),
        ("cooldown", "冷却时间"),
        ("weak spot", "弱点部位"),
        ("weak point", "弱点部位"),
        ("passive skill", "被动技能"),
        ("ancient technology point", "古代科技点"),
        ("technology point", "科技点数"),
        ("repair kit", "修理工具"),
        ("homeward", "返回据点"),
        ("fast travel", "快速旅行"),
        ("shared storage", "共享物品"),
        ("incubation", "孵化时间"),
        ("hatching", "孵化"),
        ("work speed", "工作速度"),
        ("overweight", "负重上限"),
        ("poison gas", "毒气"),
        ("fire resistance", "耐火 炎热"),
        ("night vision", "夜视"),
        ("revive", "恢复 濒死"),
    ] {
        if q.contains(phrase) {
            asked.extend(tokens(localized));
        }
    }
    let mut words = asked.clone();
    for term in terms.iter().take(5) {
        words.extend(tokens(&crate::assistant_engine::field_text(term)));
    }
    let mut scores = vec![(0.0, 0u16, 0u16); index.records.len()];
    for word in words {
        let Some(postings) = index.postings.get(&word) else {
            continue;
        };
        let weight = ((index.records.len() + 1) as f64 / (postings.len() as f64 + 0.5)).ln();
        if weight < 2.0 {
            continue;
        }
        let original = asked.contains(&word);
        for &i in postings {
            scores[i].0 += weight * if original { 1.0 } else { 0.72 };
            scores[i].1 += 1;
            scores[i].2 += u16::from(original);
        }
    }
    let mut ranked: Vec<_> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, (weight, hits, direct))| {
            *hits >= 3 && *weight >= 14.0 && (*direct > 0 || terms.len() > 1)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.0.total_cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let best = ranked.first()?.1.0;
    let rows = array(d.get("search-index-data"));
    let mut descriptions = HashSet::new();
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for (i, (score, _, _)) in ranked {
        if links.len() >= 4 || score < best * 0.80 {
            break;
        }
        let record = &index.records[i];
        let entry = &rows[record.row];
        let name = text(entry, "name");
        if !descriptions.insert((name, &record.description)) {
            continue;
        }
        let caption = if text(entry, "kind") == "伙伴技能" {
            format!("{} · {name}", text(entry, "hint"))
        } else {
            name.into()
        };
        lines.push(format!(
            "{caption}（{}）：{}",
            text(entry, "kind"),
            record.description
        ));
        links.push((caption, text(entry, "href").into()));
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}
