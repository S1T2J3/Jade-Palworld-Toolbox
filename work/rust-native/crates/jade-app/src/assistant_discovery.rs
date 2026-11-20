//! Ground open-ended questions and model search terms in the shipping handbook.
use crate::assistant_engine::AssistantReply;
use crate::data::{Data, array, text};
use serde_json::Value;
mod handbook;
mod record_query;
pub(crate) fn filtered_records(d: &Data, q: &str) -> Option<AssistantReply> {
    record_query::filters::lookup(d,q).or_else(|| catalog_text::item_effect_comparison(d, q))
}
pub(crate) fn verified_record(d: &Data, q: &str) -> Option<AssistantReply> {
    record_query::lookup(d, q)
}
pub(crate) fn combat_preparation(d: &Data, q: &str) -> Option<AssistantReply> {
    record_query::combat_passive(d,q)
}
pub(crate) fn verified_procedure(q: &str) -> Option<AssistantReply> {
    environment::spawn_mode(q).or_else(|| survival_guides::procedure(q))
}
pub(crate) use reference_search::article_definition;
mod base_staff;
pub(crate) use base_staff::{carrying, workers};
mod reference_search;
mod bilingual;
mod semantic;
pub(crate) fn evidenced_relation(d:&Data,q:&str)->Option<AssistantReply>{
    semantic::relation_evidence(d,q)
}
pub(crate) use bilingual::{EvidencePassage, offered_passages};
mod catalog_text;
mod patch_history;
pub(crate) fn recorded_fix(q: &str) -> Option<AssistantReply> {
    patch_history::lookup(q)
}
pub(crate) use catalog_text::CatalogText;
mod team;
mod flight;
pub(crate) mod environment;
mod survival_guides;
pub(crate) use survival_guides::bounded_excerpt;
mod updates;
pub(crate) use updates::{handbook_version_matches, staffing_evidence};

pub(crate) fn survival_guide_evidence(question: &str) -> Option<AssistantReply> {
    let subject = question.lines().next().unwrap_or(question);
    updates::lookup(subject).or_else(|| survival_guides::for_question(question))
}

pub(crate) fn article_evidence(question: &str) -> Option<AssistantReply> {
    reference_search::lookup(question, &[])
}

pub(crate) fn team_evidence(d: &Data, question: &str) -> Option<AssistantReply> {
    let q = crate::assistant_engine::field_text(question);
    team::lookup(d, &q).or_else(|| flight::lookup(d, &q))
}

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| q.contains(term))
}

fn table_row<'a>(table: &'a Value, id: &str) -> &'a Value {
    array(table)
        .iter()
        .find(|row| text(row, "id") == id)
        .unwrap_or(&Value::Null)
}

fn summary(d: &Data, row: &Value, q: &str) -> String {
    if let Some(details) = handbook::summary(d, row, q) {
        return details;
    }
    let route = text(row, "href");
    let id = route.rsplit('/').next().unwrap_or("");
    let hint = text(row, "hint");
    let pair = |v: &Value| {
        array(v)
            .iter()
            .filter_map(|a| Some(format!("{} Lv.{}", a[0].as_str()?, a[1].as_u64()?)))
            .collect::<Vec<_>>()
            .join("、")
    };
    let result = match text(row, "kind") {
        "主动技能" => {
            let m = &d.get("pal-details-data")["moves"][id];
            let mut fields = vec![hint.to_owned()];
            if let Some(cooldown) = m["cooldown"].as_u64() {
                fields.push(format!("冷却 {cooldown} 秒"));
            }
            let statuses = array(&m["effects"])
                .iter()
                .filter_map(|s| s[0].as_str())
                .collect::<Vec<_>>();
            if !statuses.is_empty() {
                fields.push(format!("附带{}", statuses.join("、")));
            }
            let fruit = text(m, "fruit");
            if !fruit.is_empty() {
                fields.push(format!("对应果实为{fruit}"));
            }
            fields.join("；")
        }
        "伙伴技能" => text(&d.get("partner-skills-data")[id], "effect").into(),
        "异常状态" => text(table_row(&d.get("combat-guide-data")["status"], id), "text").into(),
        "野外采集" => text(
            table_row(&d.get("resource-guides-data")["gathering"], id),
            "note",
        )
        .into(),
        "任务" => {
            let v = table_row(&d.get("adventure-data")["quests"], id);
            let mut fields = vec![text(v, "summary")];
            for kind in ["objectives", "rewards"] {
                fields.extend(
                    array(&v[kind])
                        .iter()
                        .take(3)
                        .map(|entry| text(entry, "text")),
                );
            }
            fields.retain(|v| !v.is_empty());
            fields.join("；")
        }
        "设施" => {
            let v = table_row(&d.get("facilities-data")["facilities"], id);
            let mut fields = Vec::new();
            if !text(v, "purpose").is_empty() {
                fields.push(text(v, "purpose").to_owned());
            }
            if let Some(level) = v["level"].as_u64() {
                fields.push(format!("科技等级 {level}"));
            }
            let jobs = pair(&v["required"]);
            if !jobs.is_empty() {
                fields.push(format!("所需适性 {jobs}"));
            }
            let materials = array(&v["materials"])
                .iter()
                .filter_map(|m| Some(format!("{} × {}", m[2].as_str()?, m[1].as_u64()?)))
                .collect::<Vec<_>>();
            if !materials.is_empty() {
                fields.push(format!("建造材料为{}", materials.join("、")));
            }
            fields.join("；")
        }
        "帕鲁" => {
            let profile = &d.get("spawn-data")["profiles"][id];
            let mut fields = vec![hint.to_owned()];
            let jobs = pair(&profile["work"]);
            if !jobs.is_empty() {
                fields.push(format!("工作适性 {jobs}"));
            }
            for (key, label) in [("hp", "生命"), ("attack", "攻击"), ("defense", "防御")] {
                if let Some(stat) = profile[key].as_u64() {
                    fields.push(format!("{label}种族值 {stat}"));
                }
            }
            if !text(profile, "partner").is_empty() {
                let name = text(profile, "partner");
                let effect = text(&d.get("partner-skills-data")[id], "effect");
                fields.push(if effect.is_empty() { format!("伙伴技能「{name}」") }
                    else { format!("伙伴技能「{name}」：{effect}") });
            }
            fields.join("；")
        }
        "物品" => {
            let item = table_row(&d.get("item-catalog-data")["items"], id);
            if crate::assistant_data::seller_query(q) {
                return crate::assistant_data::seller_reference(d, item).text;
            }
            if crate::assistant_data::recipe_query(q) {
                return crate::assistant_data::recipe_answer(d, item).text;
            }
            let effect = &d.get("item-effects-data")["items"][id];
            let mut fields = vec![text(effect, "description").to_owned()];
            fields.extend(
                array(&effect["effects"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned),
            );
            if !array(&item["recipes"]).is_empty() {
                fields.push(crate::assistant_data::recipe_answer(d, item).text);
            }
            if let Some(methods) = method_summary(&item["methods"]) {
                fields.push(methods);
            }
            fields.retain(|f| !f.is_empty());
            fields.join("；")
        }
        _ => hint.into(),
    };
    if result.is_empty() {
        hint.into()
    } else {
        result
    }
}

// A search result is a preview. Preserve each channel even when the first
// several records are all combat drops from different species.
fn method_summary(rows: &Value) -> Option<String> {
    let mut channels: Vec<(&str, Vec<&str>)> = Vec::new();
    for row in array(rows) {
        let (Some(kind), Some(place)) = (row[0].as_str(), row[1].as_str()) else { continue };
        if let Some((_, places)) = channels.iter_mut().find(|(name, _)| *name == kind) {
            if !places.contains(&place) { places.push(place); }
        } else {
            channels.push((kind, vec![place]));
        }
    }
    if channels.is_empty() { return None; }
    let preview = channels.into_iter().map(|(kind, places)| match places.as_slice() {
        [one] => format!("{kind} · {one}"),
        [one, two] => format!("{kind} · {one}、{two}"),
        [one, two, ..] => format!("{kind} · {one}、{two}等（{}条）", places.len()),
        [] => unreachable!(),
    }).collect::<Vec<_>>();
    Some(format!("物品表中收录的获取记录：{}", preview.join("；")))
}

pub(crate) fn relevant_names<'a>(rows: &'a [Value], question: &str) -> Vec<&'a Value> {
    let q = question
        .trim_matches(|c: char| c.is_whitespace() || "？！。，!?.".contains(c))
        .to_ascii_lowercase();
    let mut matches = Vec::new();
    for row in rows {
        let name = text(row, "name");
        let kind = text(row, "kind");
        // The surgery index duplicates passive names; the passive detail links to surgery.
        if kind == "手术台" && !has(&q, &["手术", "植入", "改造"]) {
            continue;
        }
        for (alias, raw) in std::iter::once((false, name)).chain(
            array(&row["aliases"])
                .iter()
                .filter_map(Value::as_str)
                .map(|alias| (true, alias)),
        ) {
            let term = raw.to_ascii_lowercase();
            if term.is_empty() || !term.chars().any(char::is_alphabetic) {
                continue;
            }
            if q != term && (term.chars().count() < 2 || term.is_ascii() && term.len() < 4) {
                continue;
            }
            if !q.contains(&term) { continue; }
            let preferred = q.contains(kind)
                || kind == "设施" && has(&q, &["建造", "解锁"])
                || kind == "主动技能" && has(&q, &["冷却", "威力", "cd"])
                || kind == "手术台" && has(&q, &["手术", "植入", "改造"])
                || kind == "据点生产" && has(&q, &["生产", "产出", "供电", "耗电", "产一个"])
                || kind == "任务" && q.contains("任务");
            for (start, _) in q.match_indices(&term) {
                let end = start + term.len();
                if term.is_ascii() && (q[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
                    || q[end..].starts_with(|c: char| c.is_ascii_alphanumeric())) {
                    continue;
                }
                matches.push((start, end, alias, !preferred, row));
            }
        }
    }
    // Prefer the longest nonoverlapping mention. This preserves both operands in a comparison.
    matches.sort_by_key(|(a, b, alias, not_preferred, _)| {
        (std::cmp::Reverse(b - a), *not_preferred, *alias, *a)
    });
    let mut selected: Vec<(usize, usize, &Value)> = Vec::new();
    for (start, end, _, _, row) in matches {
        if selected
            .iter()
            .any(|(a, b, prior)| start < *b && *a < end || prior["href"] == row["href"])
        {
            continue;
        }
        selected.push((start, end, row));
        if selected.len() == 4 {
            break;
        }
    }
    selected.sort_by_key(|(a, _, _)| *a);
    selected.into_iter().map(|(_, _, v)| v).collect()
}

fn trait_bonus(effect: &str, key: &str) -> Option<f64> {
    effect.split(['\n', '，', '；']).find_map(|line| {
        let rest = line.trim().strip_prefix(key)?.trim();
        let rest = rest
            .trim_start_matches("提升")
            .trim_start_matches("提高")
            .trim_start_matches("增加")
            .trim();
        let (number, _) = rest.split_once('%')?;
        let value: f64 = number.trim().parse().ok()?;
        (value > 0.).then_some(value)
    })
}

fn trait_choices(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(q, &["词条", "被动", "特性"])
        || !has(
            q,
            &[
                "哪些",
                "什么",
                "推荐",
                "选",
                "最好",
                "最高",
                "有啥",
                "有没有",
                "想要",
                "想加",
                "想提高",
            ],
        )
    {
        return None;
    }
    let conditions: Vec<_> = [
        (
            "工作速度",
            &["工作速度", "工作效率", "干活", "工作快", "制作速度"][..],
        ),
        ("攻击", &["攻击", "伤害", "输出"][..]),
        ("防御", &["防御", "防守"][..]),
        ("移动速度", &["移动速度", "移速", "跑速"][..]),
        ("最大HP", &["生命", "血量", "HP上限"][..]),
    ]
    .into_iter()
    .filter_map(|(key, terms)| has(q, terms).then_some(key))
    .collect();
    let first = *conditions.first()?;
    let passives = d.get("pal-details-data")["passives"].as_object()?;
    let mut candidates: Vec<_> = passives
        .iter()
        .filter_map(|(key, r)| {
            let effect = text(r, "effect");
            if conditions
                .iter()
                .any(|metric| trait_bonus(effect, metric).is_none())
            {
                return None;
            }
            Some((trait_bonus(effect, first)?, key, r))
        })
        .collect();
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    let labels = conditions.join("、");
    let mut lines = vec![format!(
        "按词条说明中的正向{labels}百分比筛选，按{first}加成从高到低列出。"
    )];
    let mut links = Vec::new();
    for (_, key, r) in candidates.iter().take(4) {
        let name = text(r, "zh");
        lines.push(format!("{name}：{}", text(r, "effect").replace('\n', "，")));
        links.push((name.into(), format!("#details/passive/trait/{key}")));
    }
    if links.is_empty() {
        return None;
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

struct EffectQuery {
    category: String,
    phrases: Vec<String>,
    pairs: Vec<String>,
}

// Category names, comparison words and function words cannot identify a mechanic
// in an unrelated item's description. Exact catalogue names are resolved separately.
fn generic_effect_word(word: &str) -> bool {
    matches!(
        word,
        "pal" | "pals" | "palworld" | "the" | "an" | "in" | "of" | "to" | "for"
        | "on" | "with" | "by" | "and" | "or" | "at" | "is" | "are" | "as"
        | "how" | "does" | "do" | "what" | "which" | "after" | "before" | "post"
        | "best" | "strongest" | "better" | "stronger" | "game" | "item" | "items"
        | "update" | "patch" | "version" | "帕鲁" | "游戏" | "版本" | "更新"
        | "最强" | "最强帕鲁" | "比较" | "推荐" | "提升" | "增加" | "上限" | "提高" | "降低"
    )
}

fn matches_effect_line(line: &str, phrase: &str) -> bool {
    if phrase.is_ascii() {
        // An internal key (PAL_Growth_Stone_S) or a partial word ("more" for "ore")
        // gives no evidence for an ordinary-language mechanism query.
        line.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|word| word.eq_ignore_ascii_case(phrase))
    } else {
        line.contains(phrase)
    }
}

impl EffectQuery {
    fn new(term: &str, category: &str) -> Self {
        let mut rest = term.trim().to_ascii_lowercase().replace("挖矿", "采矿");
        let mut category = category.to_owned();
        // Longest names first so "帕鲁装备" keeps its actual catalogue category.
        for (word, filter) in [
            ("伙伴技能", "伙伴技能"),
            ("被动词条", "被动词条"),
            ("主动技能", "主动技能"),
            ("帕鲁装备", "帕鲁装备"),
            ("消耗品", "消耗品"),
            ("饰品", "饰品"),
            ("防具", "防具"),
            ("武器", "武器"),
            ("装备", "装备"),
            ("词条", "被动词条"),
            ("被动", "被动词条"),
            ("道具", "物品"),
            ("物品", "物品"),
        ] {
            if rest.contains(word) {
                if category.is_empty() {
                    category = filter.into();
                }
                rest = rest.replace(word, " ");
            }
        }
        for filler in [
            "幻兽帕鲁",
            "游戏",
            "推荐",
            "有没有",
            "有哪些",
            "有什么",
            "可以",
            "效果",
            "玩家",
            "怎么",
            "背包",
            "什么",
            "如何",
        ] {
            rest = rest.replace(filler, " ");
        }
        // The effects table stores these fields in Chinese even for English-named entries.
        let rest = match rest.trim() {
            "work speed" => "工作速度",
            "movement speed" => "移动速度",
            "carrying capacity" | "carry weight" => "负重上限",
            "cold resistance" => "耐寒",
            "heat resistance" => "耐热",
            other => other,
        };
        let mut phrases = Vec::new();
        let mut pairs = Vec::new();
        for phrase in rest.split(|c: char| !c.is_alphanumeric()) {
            let chars: Vec<_> = phrase.chars().collect();
            if chars.len() < 2 || !chars.iter().any(|c| c.is_alphabetic()) || generic_effect_word(phrase) {
                continue;
            }
            for pair in chars
                .windows(2)
                .filter(|p| !p[0].is_ascii() && !p[1].is_ascii())
            {
                let pair: String = pair.iter().collect();
                if !pair.contains(['的', '哪', '帕', '鲁'])
                    && !generic_effect_word(&pair) && !["可以", "速度"].contains(&pair.as_str())
                    && !pairs.contains(&pair)
                {
                    pairs.push(pair);
                }
            }
            if !phrases.iter().any(|word| word == phrase) {
                phrases.push(phrase.to_owned());
            }
        }
        Self {
            category,
            phrases,
            pairs,
        }
    }

    fn accepts(&self, kind: &str, hint: &str) -> bool {
        match self.category.as_str() {
            "" => true,
            "装备" => {
                kind == "物品" && ["饰品", "防具", "武器", "帕鲁装备", "滑翔伞"].contains(&hint)
            }
            category => category == kind || category == hint,
        }
    }
}

// Score distinct mechanics across the whole record; show the source lines that
// contributed. A catalogue category filters candidates but contributes no score.
fn effect_results(d: &Data, term: &str, category: &str) -> Vec<AssistantReply> {
    // Explicit IDs and underscore-form catalogue aliases are resolved by discover.
    // Do not split an unknown ID into words to search unrelated descriptions.
    if term.contains('_') && term.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return vec![];
    }
    let query = EffectQuery::new(term, category);
    if query.phrases.is_empty() {
        return vec![];
    }
    let english_count = query.phrases.iter().filter(|word| word.is_ascii()).count();
    let mut candidates = Vec::new();
    for row in array(d.get("search-index-data")) {
        let kind = text(row, "kind");
        let hint = text(row, "hint");
        if !query.accepts(kind, hint) {
            continue;
        }
        let route = text(row, "href");
        let Some(id) = route.rsplit('/').next() else {
            continue;
        };
        let fields: Vec<&str> = match kind {
            "物品" => {
                let entry = &d.get("item-effects-data")["items"][id];
                array(&entry["effects"])
                    .iter()
                    .filter_map(Value::as_str)
                    .chain(std::iter::once(text(entry, "description")))
                    .collect()
            }
            "被动词条" | "主动技能" => vec![hint],
            "伙伴技能" => vec![text(&d.get("partner-skills-data")[id], "effect")],
            _ => continue,
        };
        let mut scores = vec![0; fields.len()];
        let mut matched = 0;
        let mut full = 0;
        let mut english_hits = 0;
        let mut coverage = 0u128;
        for (index, (phrase, weight)) in query
            .phrases
            .iter()
            .map(|s| (s, 4))
            .chain(query.pairs.iter().map(|s| (s, 1)))
            .take(128)
            .enumerate()
        {
            let mut found = false;
            for (field, score) in fields.iter().zip(scores.iter_mut()) {
                if !field.eq_ignore_ascii_case(id) && matches_effect_line(field, phrase) {
                    *score += weight;
                    found = true;
                }
            }
            if found {
                coverage |= 1 << index;
                if weight == 4 {
                    full += 1;
                    english_hits += usize::from(phrase.is_ascii());
                } else {
                    matched += 1;
                }
            }
        }
        if english_hits < english_count || full == 0 && (matched == 0 || matched * 3 < query.pairs.len()) {
            continue;
        }
        let mut ranked_fields: Vec<_> = scores
            .into_iter()
            .zip(&fields)
            .enumerate()
            .filter(|(_, (score, _))| *score > 0)
            .collect();
        ranked_fields.sort_by_key(|(_, (score, _))| std::cmp::Reverse(*score));
        let (index, (top_score, &line)) = ranked_fields[0];
        let mechanic = kind == "被动词条" || kind == "物品" && index + 1 < fields.len();
        let numeric = line.chars().any(|c| c.is_ascii_digit());
        let mut facts = vec![line];
        if let Some((_, (_, extra))) = ranked_fields.get(1) {
            if query
                .phrases
                .iter()
                .chain(&query.pairs)
                .any(|p| !matches_effect_line(line, p) && matches_effect_line(extra, p))
            {
                facts.push(extra);
            }
        }
        candidates.push((
            (full, matched, top_score, mechanic, numeric),
            coverage,
            row,
            facts.join("。"),
        ));
    }
    candidates.sort_by_key(|(score, _, _, _)| std::cmp::Reverse(*score));
    let mut results = Vec::new();
    let mut covered = 0;
    while results.len() < 4 && !candidates.is_empty() {
        // If no record covers a combined search, expose complementary evidence
        // before filling the remaining places with alternative items.
        let best = &candidates[0];
        let next = candidates
            .iter()
            .position(|(score, bits, _, _)| {
                (score.0, score.1) == (best.0.0, best.0.1) && (*bits & !covered) != 0
            })
            .unwrap_or(0);
        let (_, bits, row, facts) = candidates.remove(next);
        covered |= bits;
        let label = format!("{}（{}）", text(row, "name"), text(row, "kind"));
        results.push(AssistantReply {
            text: format!("{label}：{facts}"),
            links: vec![(label, text(row, "href").into())],
            suggestions: vec![],
        });
    }
    results
}

fn described_goal(d: &Data, q: &str) -> Option<AssistantReply> {
    let q = q.trim_end_matches(|c: char| c.is_whitespace() || "？！。?!".contains(c));
    let (query, category) = q.rsplit_once('的')?;
    let kind = match category.trim_end_matches('吗') {
        "饰品" => "饰品",
        "道具" | "物品" => "物品",
        "被动" | "词条" | "被动词条" => "被动词条",
        "伙伴技能" => "伙伴技能",
        _ => return None,
    };
    let term = [
        "有没有",
        "有哪些",
        "有什么",
        "我想要",
        "推荐",
        "我想找",
        "找一下",
    ]
    .iter()
    .find_map(|p| query.strip_prefix(p))
    .unwrap_or(query)
    .trim_start_matches("可以")
    .trim_start_matches('能');
    combine(effect_results(d, term, kind))
}

// "Which pal does X?" describes a mechanic without naming the skill. Match the
// requested object and modifiers together so pickup, hatching and ranching stay distinct.
fn partner_by_effect(d: &Data, q: &str) -> Option<AssistantReply> {
    let q = q.to_lowercase();
    if !has(&q, &["哪只", "哪隻", "哪一只", "哪一隻", "哪个帕鲁", "哪個帕魯", "什么帕鲁", "誰", "谁", "which pal", "what pal"]) {
        return None;
    }
    let mut filters: Vec<&[&str]> = Vec::new();
    for (query, field) in [
        (&["蛋", "egg"][..], &["蛋"][..]),
        (&["负重", "負重", "carry weight"], &["负重"]),
        (&["矿石", "礦石", "ore weight"], &["矿石"]),
    ] {
        if has(&q, query) { filters.push(field); }
    }
    if filters.is_empty() { return None; }
    let objects = filters.len();
    for (query, field) in [
        (&["捡", "撿", "拾取", "领取", "領取", "pick up", "picking up"][..], &["捡", "拾取", "领取"][..]),
        (&["额外", "額外", "多拿", "多捡", "extra", "additional"], &["额外", "再捡", "再获得", "多获得"]),
        (&["头目", "頭目", "alpha"], &["头目"]),
        (&["孵化", "hatch"], &["孵化"]),
        (&["减轻", "減輕", "减少", "減少", "reduce"], &["减轻", "减少", "降低"]),
        (&["队伍", "隊伍", "in the party", "in my party"], &["队伍"]),
    ] {
        if has(&q, query) { filters.push(field); }
    }
    if filters.len() == objects { return None; }
    let skills = d.get("partner-skills-data").as_object()?;
    let matches = skills.iter().filter(|(_, skill)| {
        let effect = text(skill, "effect");
        filters.iter().all(|terms| has(effect, terms))
    }).take(4).map(|(id, skill)| {
        let owner = d.pal_name(id);
        let name = text(skill, "name");
        AssistantReply {
            text: format!("{owner}的伙伴技能「{name}」：{}", text(skill, "effect")),
            links: vec![(format!("{owner} · {name}"), format!("#details/partner/{id}"))],
            suggestions: vec![],
        }
    });
    combine(matches)
}

/// No named game entry and no supported goal means insufficient evidence; let the
/// conversation ask the player for the missing context in that case.
pub(crate) fn discover(d: &Data, q: &str) -> Option<AssistantReply> {
    if q.trim().is_empty() || q.chars().take(513).count() > 512 {
        return None;
    }
    if has(
        q,
        &[
            "写诗",
            "写首诗",
            "写一首",
            "写论文",
            "写代码",
            "编个故事",
            "写邮件",
            "翻译成",
            "股票",
            "汇率",
            "银行卡",
            "医院",
            "显卡驱动",
        ],
    ) {
        return None;
    }
    if let Some(result) = partner_by_effect(d, q) {
        return Some(result);
    }
    let rows = relevant_names(array(d.get("search-index-data")), q);
    let query: String = q
        .trim_matches(|c: char| c.is_whitespace() || "？！。!?.".contains(c))
        .to_ascii_lowercase();
    let explicit = rows.iter().any(|r| {
        text(r, "name").eq_ignore_ascii_case(&query)
            || array(&r["aliases"])
                .iter()
                .filter_map(Value::as_str)
                .any(|a| a.eq_ignore_ascii_case(&query))
    });
    let asks_game = has(
        q,
        &[
            "帕鲁",
            "词条",
            "被动",
            "主动",
            "技能",
            "效果",
            "威力",
            "冷却",
            "几秒",
            "等级",
            "几级",
            "解锁",
            "设施",
            "建造",
            "合成",
            "制作",
            "任务",
            "状态",
            "在哪",
            "哪里",
            "位置",
            "点位",
            "怎么",
            "如何",
            "多少",
            "几率",
            "概率",
            "有什么",
            "有哪些",
            "有没有",
            "推荐",
            "对比",
            "区别",
            "比较",
            "更好",
            "哪个",
            "哪种",
            "选",
            "强不强",
            "好用",
            "厉害",
            "是什么",
            "介绍",
            "查询",
            "弱点",
            "克制",
            "相克",
            "多久",
            "多长时间",
            "耗电",
            "供电",
            "能出",
            "卖不卖",
            "卖的",
            "多少钱",
            "干哪种",
            "手术",
            "植入",
            "产出",
            "说说",
            "讲讲",
        ],
    );
    if !explicit && !asks_game && !crate::assistant_data::seller_query(q) {
        return None;
    }
    if !rows.iter().any(|r| text(r, "kind") == "被动词条") {
        if let Some(choices) = trait_choices(d, q) {
            return Some(choices);
        }
    }
    if rows.is_empty() {
        return described_goal(d, q);
    }
    if let [row] = rows.as_slice()
        && text(row, "kind") == "物品"
        && crate::assistant_data::seller_query(q)
        && let Some(id) = text(row, "href").strip_prefix("#items/")
    {
        let item = table_row(&d.get("item-catalog-data")["items"], id);
        if !item.is_null() { return Some(crate::assistant_data::seller_reference(d, item).into()); }
    }
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut stock_link = None;
    if let [row] = rows.as_slice()
        && text(row, "kind") == "商人"
        && crate::assistant_data::seller_query(q)
        && let Some(vendor) = text(row, "href").strip_prefix("#merchants/")
        && let Some(id) = handbook::matched_shop_item(d, vendor, q)
    {
        stock_link = Some(("查看物品售价".into(), format!("#items/{id}/shops")));
    }
    for row in rows {
        let name = text(row, "name");
        let kind = text(row, "kind");
        let details = summary(d, row, q);
        if details.trim().is_empty() {
            continue;
        }
        let label = format!("{name}（{kind}）");
        lines.push(format!("{label}：{details}"));
        links.push((label, text(row, "href").into()));
    }
    if links.is_empty() {
        return None;
    }
    links.extend(stock_link);
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

/// A model supplies search terms, never game facts. The evidence and links both
/// come back from the local data pack, and overlapping results are deduplicated.
pub(crate) fn retrieve(d: &Data, terms: &[String]) -> Option<AssistantReply> {
    retrieve_local(d, terms, "")
}

fn retrieve_local(d: &Data, terms: &[String], question: &str) -> Option<AssistantReply> {
    let normalized: Vec<_> = terms
        .iter()
        .take(6)
        .map(|term| term.trim())
        .filter(|t| t.chars().count() <= 80)
        .map(crate::assistant_engine::field_text)
        .collect();
    let terms: Vec<&str> = normalized.iter().map(String::as_str).collect();
    let category_requested = terms.iter().any(|term| !EffectQuery::new(term, "").category.is_empty());
    // Allow a planner to emit a stand-alone category as well as "负重 饰品".
    let mut categories: Vec<_> = terms
        .iter()
        .map(|t| EffectQuery::new(t, ""))
        .filter(|q| q.phrases.is_empty() && !q.category.is_empty())
        .map(|q| q.category)
        .collect();
    categories.sort();
    categories.dedup();
    let category = if categories.len() == 1 {
        categories[0].as_str()
    } else {
        ""
    };
    let mut matches = Vec::new();
    // A planner may split a subject and a relation into separate search terms.
    let is_matchup = |term: &str| {
        matches!(
            term,
            "弱点"
                | "克制"
                | "相克"
                | "弱什么"
                | "属性弱点"
                | "属性克制"
                | "属性相克"
                | "属性克制表"
        )
    };
    let asks_matchup = terms.iter().any(|t| is_matchup(t));
    for term in terms {
        if term.chars().count() != 1 && EffectQuery::new(term, "").phrases.is_empty() {
            continue;
        }
        let entry = discover(d, term);
        let matchup = crate::assistant_elements::reference(d, term);
        if let Some(facts) = matchup {
            matches.push((0, facts));
        } else if asks_matchup && let Some(ref answer) = entry {
            for (_, route) in &answer.links {
                if let Some(id) = route.strip_prefix("#paldex/")
                    && let Some(facts) = crate::assistant_elements::reference(
                        d,
                        &format!("{}的弱点", d.pal_name(id)),
                    )
                {
                    matches.push((0, facts));
                }
            }
        } else if entry.is_none() && !is_matchup(term) {
            matches.extend(
                effect_results(d, term, category)
                    .into_iter()
                    .enumerate()
                    .map(|(i, answer)| (i + 1, answer)),
            );
        }
        if let Some(answer) = entry {
            matches.push((1, answer));
        }
    }
    // Show the strongest result for each search before showing its alternatives.
    matches.sort_by_key(|(rank, _)| *rank);
    let guide = if category_requested && !matches.is_empty() {
        None
    } else {
        survival_guides::lookup(&normalized)
    };
    let plan = normalized.join(" ");
    let subject = question.lines().next().unwrap_or("");
    let update = if subject.is_empty() {
        updates::lookup(&plan)
    } else {
        updates::lookup_in_context(subject, question)
            .or_else(|| updates::lookup_in_context(&plan, question))
    };
    combine(matches.into_iter().map(|(_, answer)| answer).chain(update).chain(guide))
}

pub(crate) fn combine(results: impl IntoIterator<Item = AssistantReply>) -> Option<AssistantReply> {
    let mut texts = Vec::new();
    let mut links = Vec::new();
    for answer in results {
        if answer.links.is_empty() {
            continue;
        }
        if !texts.contains(&answer.text) {
            texts.push(answer.text);
        }
        for link in answer.links {
            if !links.iter().any(|(_, route)| route == &link.1) {
                links.push(link);
            }
        }
    }
    if links.is_empty() {
        return None;
    }
    Some(AssistantReply {
        text: texts.join("\n"),
        links,
        suggestions: vec![],
    })
}

/// Search the actual question as well as the model's shorter entity names.
/// An article is context for synthesis; the response is never marked direct.
pub(crate) fn retrieve_for_question(d: &Data, terms: &[String], question: &str) -> Option<AssistantReply> {
    let local = retrieve_local(d, terms, question);
    if question.trim().is_empty() { return local; }
    let articles = reference_search::lookup(question,terms);
    let descriptions = catalog_text::lookup(d, question, terms);
    combine(semantic::relation_evidence(d, question).into_iter().chain(survival_guide_evidence(question)).chain(team_evidence(d, question)).chain(descriptions).chain(local).chain(articles))
}

pub(crate) fn retrieve_planned(d: &Data, terms: &[String], question: &str, translation: Option<&str>) -> Option<AssistantReply> {
    let original = retrieve_for_question(d, terms, question);
    let Some(query) = translation else { return original; };
    let bilingual = bilingual::lookup(d, question, terms, query);
    combine(bilingual.into_iter().chain(original))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    pub(super) fn handbook() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "search-index-data",
            "pal-details-data",
            "partner-skills-data",
            "resource-guides-data",
            "facilities-data",
            "item-catalog-data",
            "item-effects-data",
            "combat-guide-data",
            "adventure-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            d.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        d
    }

    #[test]
    fn plan_translation_retrieves_source_text_and_empty_translation_preserves_older_results() {
        let data = handbook();
        let question = "地上拾到的野生帕鲁蛋，孵出来有没有随机词条？";
        let planned = bilingual::lookup(&data, question, &[], "Can Pals hatched from wild Pal Eggs spawn with random Passive Skills?").unwrap();
        assert!(planned.text.contains("Wild generation"));
        assert!(planned.links.iter().any(|(_, link)| link == "https://www.thepalwiki.com/wiki/passive-skills"));
        assert!(!planned.text.contains("Can Pals hatched"));
        let original = retrieve_for_question(&data, &[], question).map(|r| r.text);
        assert_eq!(retrieve_planned(&data, &[], question, Some("")).map(|r| r.text), original);
    }

    #[test]
    fn indirect_gameplay_descriptions_keep_the_source_and_ignore_unrelated_text() {
        let d = handbook();
        let found = catalog_text::lookup(&d, "什么东西能显示帕鲁的隐藏才能和潜力？", &[]).unwrap();
        assert!(found.links.iter().any(|(_, route)| route == "#items/Accessory_TalentChecker"));
        assert!(found.text.contains("100分"));
        let food = catalog_text::lookup(&d, "Why do I have separate food slots?", &[]).unwrap();
        assert!(food.links.iter().any(|(_, route)| route == "#items/AutoMealPouch_Tier5"));
        assert!(catalog_text::lookup(&d, "What is my email password?", &[]).is_none());
    }

    #[test]
    fn named_item_effects_remain_attached_to_the_requested_subject() {
        let d = handbook();
        let q = "白天热、夜里又冷，我只留一个饰品位。耐热内衣和耐热防寒内衣，哪件本身同时管两种温度？";
        let answer = filtered_records(&d, q).unwrap();
        assert!(answer.text.contains("耐热内衣：拥有耐热效果") && answer.text.contains("耐热防寒内衣：同时拥有耐热、耐寒效果"), "{}", answer.text);
        let nested = filtered_records(&d, "耐热防寒内衣和防寒内衣，哪件同时管两种温度？").unwrap();
        assert_eq!(nested.links.len(), 2);
        assert!(catalog_text::item_effect_comparison(&d, "耐热内衣和耐热防寒内衣买哪个好？哪个便宜？").is_none());
        assert!(catalog_text::item_effect_comparison(&d, "耐热内衣和耐热防寒内衣哪件会加工作速度？").is_none());
        let guide = survival_guides::guides().first().unwrap();
        let facts = format!("{}\n耐热内衣（物品）：拥有耐热效果的饰品。\n耐热防寒内衣（物品）：同时拥有耐热、耐寒效果的饰品。",guide.zh);
        let options = offered_passages(&d, q, &facts);
        assert!(options.iter().any(|p| p.body == "耐热防寒内衣（物品）：同时拥有耐热、耐寒效果的饰品。"));
        let crossed = format!("{}\n耐热防寒内衣（物品） · #items/Accessory_HeatColdResist_1\n另一件饰品：同时拥有耐热、耐寒效果的饰品。",guide.zh);
        assert!(!offered_passages(&d, q, &crossed).iter().any(|p| p.title == "耐热防寒内衣（物品）"));
    }

    #[test]
    fn matchup_uses_opponents_typing_and_updates_match_specific_symptoms() {
        let data = handbook();
        let reply = crate::assistant_elements::against_species(&data, "how do i beat shaolong?").unwrap();
        assert!(reply.text.contains("冰属性") && reply.text.contains("雷属性") && !reply.text.contains("队伍中"), "{}", reply.text);
        assert!(recorded_fix("基地里刷新出来的树木不砍了").unwrap().text.contains("v1.0.4"));
        assert!(recorded_fix("瞭望塔的建造材料").is_none());
    }

    #[test]
    fn a_named_record_keeps_numeric_facts_together() {
        let mut d = handbook();
        let data=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        for table in ["expedition-data","paldex-index-data"] {
            let b=std::fs::read(data.join(format!("{table}.json"))).unwrap();
            d.tables.insert(table.into(),Arc::new(serde_json::from_slice(&b).unwrap()));
        }
        let move_card=verified_record(&d,"主动技能水流射击的威力和冷却是多少？").unwrap();
        assert!(move_card.text.contains("40") && move_card.text.contains("2 秒"));
        let stage=verified_record(&d,"远征去森林的秘境要多少战力，多久回来？").unwrap();
        assert!(stage.text.contains("推荐战斗力") && stage.text.contains("基础时长"));
        let jobs=verified_record(&d,"阿努比斯在基地会做哪些工作？").unwrap();
        assert!(jobs.text.contains("手工作业 Lv.6") && jobs.text.contains("搬运 Lv.4"));
        assert!(verified_record(&d,"水流射击和空气炮的冷却哪个长").is_none());
        assert!(verified_procedure("how do i make my pals attack the target?").unwrap().text.contains("Focus on Same Enemy"));
        assert!(verified_procedure("how do i boost my pals attack stat?").is_none());
    }

    #[test]
    fn item_preview_retains_each_channel_and_marks_examples() {
        let records = serde_json::json!([
            ["帕鲁掉落", "甲"], ["帕鲁掉落", "乙"], ["帕鲁掉落", "丙"],
            ["帕鲁掉落", "丁"], ["帕鲁掉落", "甲"],
            ["牧场生产", "戊"], ["商人", "己"]
        ]);
        let preview = method_summary(&records).unwrap();
        assert!(preview.contains("帕鲁掉落 · 甲、乙等（4条）"));
        assert!(preview.contains("牧场生产 · 戊") && preview.contains("商人 · 己"));
    }

    #[test]
    fn ordinary_purchase_questions_use_the_item_offers() {
        let d = handbook();
        for q in ["哪个商人卖优质帕鲁油？", "Which merchant sells High Quality Pal Oil?"] {
            let answer = crate::assistant_engine::Conversation::default().ask_world(q, &d, &Value::Null);
            assert!(answer.text.contains("边远渔村") && answer.text.contains("300 金币 / 1 件"), "{}", answer.text);
        }
    }

    #[test]
    fn identifies_skill_owner_from_effect_in_an_ordinary_conversation() {
        let d = handbook();
        for (question, owner) in [
            ("請問 在隊伍中 領取蛋會有機率拿到額外的蛋是哪一隻帕魯啊？", "NaughtyCat"),
            ("哪只帕鲁在队伍中捡蛋会变为头目蛋？", "SakuraSaurus"),
        ] {
            let answer = crate::assistant_engine::Conversation::default()
                .ask_world(question, &d, &Value::Null);
            assert!(answer.links.iter().any(|(_, r)| r == &format!("#details/partner/{owner}")), "{}", answer.text);
            assert!(answer.text.contains(&d.pal_name(owner)));
        }
    }

    #[test]
    fn station_assignment_uses_the_machine_and_the_named_workers() {
        let d = handbook();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = d;
        d.tables.insert("paldex-index-data".into(), Arc::new(serde_json::from_slice(
            &std::fs::read(root.join("paldex-index-data.json")).unwrap()).unwrap()));
        let q = "Which Pal should I assign to my Ore Mining Site: Cattiva or Lamball? Does a work speed passive help?";
        let mut dialog = crate::assistant_engine::Conversation::default();
        let reply = dialog.ask_world(q, &d, &Value::Null);
        assert!(reply.text.contains("Cattiva · Mining Lv.1 (eligible)") && reply.text.contains("Lamball · Mining Lv.0 (below requirement)"), "{}",reply.text);
        assert!(reply.links.iter().any(|(_, route)| route.ends_with("/CraftSpeed_up2")) && dialog.direct_result());
        assert!(staffing_evidence(&d, "How many iron ingots does a Primitive Furnace recipe need?").is_none());
        let travel = dialog.ask_world("我在基地里面，传送一定要点帕鲁终端吗？", &d, &Value::Null);
        assert!(travel.text.contains("无须走回帕鲁终端") && travel.text.contains("大地图"), "{}",travel.text);
    }

    #[test]
    fn backpack_and_base_assignments_use_separate_game_tables() {
        let mut data = handbook();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        for key in ["paldex-index-data", "work-efficiency-data", "ranch-data"] {
            data.tables.insert(key.into(), Arc::new(serde_json::from_slice(
                &std::fs::read(root.join(format!("{key}.json"))).unwrap()).unwrap()));
        }
        let mut dialog = crate::assistant_engine::Conversation::default();
        let player = dialog.ask_world("Which party Pal lets me carry more on the trip home?", &data, &Value::Null);
        assert!(player.text.contains("Lunaris · +300 → +600") && player.text.contains("Cattiva · +100 → +200") && dialog.direct_result(), "{}", player.text);
        let weapons = dialog.ask_world("Which Pal helps me carry more weapons in my backpack?", &data, &Value::Null);
        assert!(weapons.text.contains("Finsider Ignis") && weapons.text.contains("100%"), "{}", weapons.text);
        let workers = dialog.ask_world("Who should staff my mining base and ranch?", &data, &Value::Null);
        assert!(workers.text.contains("Mining") && workers.text.contains("Transporting Lv.7")
            && workers.text.contains("Chikipi → Egg") && dialog.direct_result(), "{}", workers.text);
        let haul = dialog.ask_world("推荐哪些帕鲁在据点搬货？", &data, &Value::Null);
        assert!(haul.text.contains("每趟最多搬 120 件") && !haul.text.contains("负重上限"), "{}", haul.text);
        for (q, facts, links) in [
            ("基地要供电、冷却和搬运，怎么分工？", &["发电 Lv.", "冷却 Lv.", "搬运 Lv.", "每趟"][..], true),
            ("Can one Pal cover both watering and planting?", &["Watering Lv.", "Planting Lv.", "one task at a time"][..], true),
            ("What is strong against Dark, and what does Ice beat?", &["Against a Dark defender, use Dragon", "Ice attacks are strong against Dragon"][..], false),
            ("地系怕什么？雷属性克制谁？", &["打地属性对手，优先用草属性", "雷属性攻击克制水属性"][..], false),
            ("火属性克制哪些，又被谁克制？", &["火属性攻击克制", "打火属性对手，优先用水属性"][..], false),
            ("水属性打火有优势吗，反过来火对水呢？", &["水属性攻击克制火属性对手", "火属性攻击对水属性没有克制优势"][..], false),
            ("火属性能打哪些？哪些属性容易打火属性？", &["火属性攻击克制草属性、冰属性", "打火属性对手，优先用水属性"][..], false),
        ] {
            let reply = dialog.ask_world(q, &data, &Value::Null);
            assert!(dialog.direct_result() && facts.iter().all(|s| reply.text.contains(s)), "{q}: {}", reply.text);
            if !links { assert_eq!(reply.links.iter().filter(|(_,u)|u=="#guide/elements").count(), 1); }
        }
    }

    #[test]
    fn survival_guide_evidence_uses_local_links_and_does_not_override_catalog_results() {
        let data = handbook();
        let index = array(data.get("search-index-data"));
        for (query, expected) in [
            ("incubator temperature", "temperature"),
            ("低理智", "温泉"),
            ("death penalty", "World settings"),
            ("party slots", "five"),
            ("food spoilage", "spoil timer"),
            ("竞技场", "两只副手"),
            ("负重", "属性点"),
            ("egg is cold", "incubator"),
        ] {
            let result = retrieve(&data, &[query.to_owned()]).unwrap();
            assert!(result.text.contains("游戏内生存指南") && result.text.contains(expected), "{query}: {}", result.text);
        }
        let combined = retrieve(&data, &["烈焰风暴".into(), "party slots".into()]).unwrap();
        assert!(combined.text.find("威力 200").unwrap() < combined.text.find("游戏内生存指南").unwrap());
        let source: Vec<Value> = serde_json::from_str(include_str!("assistant_discovery/survival_guides.json")).unwrap();
        for guide in source {
            let source = guide["source"].as_str().unwrap();
            let secondary = array(&guide["also"]).iter().map(|v| v.as_str().unwrap());
            for url in std::iter::once(source).chain(secondary) {
                assert!(url.starts_with("https://palworld.gamevault.in/guide/") || url == "https://palworld.gamevault.in/building/"
                    || url.starts_with("https://docs.palworldgame.com/") || url == "https://news.palworldgame.com/"
                    || url.starts_with("https://steamcommunity.com/games/1623730/announcements/detail/")
                    || matches!(url, "https://pocketpair.jp/en/game-contact-en/"
                        | "https://steamcommunity.com/app/1623730/discussions/0/592886359937520760/"
                        | "https://palworld.run/items/training-manual-xl"), "{url}");
            }
            if let Some(route) = guide["route"].as_str() {
                assert!(index.iter().any(|entry| entry["href"] == route), "{route}");
            }
        }
        let host = survival_guide_evidence("朋友想在房主离线后继续玩").unwrap();
        assert!(host.links.iter().any(|(_, url)| url == "https://docs.palworldgame.com/getting-started/about-server/"));
        assert!(survival_guide_evidence("How does my computer repair a broken boot sector?").is_none());
        assert!(retrieve(&data, &["sanskrit".into()]).is_none());
        let mut chat = crate::assistant_engine::Conversation::default();
        let answer = chat.ask_world("What happens to the inventory on respawn in Palworld?", &data, &Value::Null);
        assert!(!chat.direct_result() && answer.text.contains("death penalty"), "{}", answer.text);
    }

    #[test]
    fn named_comparisons_read_both_skill_records_and_exclude_substring_items() {
        let d = handbook();
        let comparison = discover(&d, "卓绝技艺和工匠精神对比起来怎么样？").unwrap();
        assert!(comparison.text.contains("+75%") && comparison.text.contains("+50%"));
        assert_eq!(comparison.links.len(), 2);
        assert!(
            comparison
                .links
                .iter()
                .all(|l| l.1.starts_with("#details/passive/trait/"))
        );
        let active = discover(&d, "烈焰风暴的威力是多少，冷却几秒？").unwrap();
        assert!(active.text.contains("威力 200") && active.text.contains("冷却 12 秒"));
        assert!(discover(&d, "帮我写一首关于工匠精神的诗").is_none());
    }

    #[test]
    fn compound_goals_filter_effects_and_model_search_returns_sourced_facts() {
        let d = handbook();
        let result = discover(&d, "有什么词条同时加攻击和防御？").unwrap();
        assert!(!result.links.is_empty());
        for (_, route) in result.links {
            let key = route.rsplit('/').next().unwrap();
            let effect = text(&d.get("pal-details-data")["passives"][key], "effect");
            assert!(trait_bonus(effect, "攻击").is_some() && trait_bonus(effect, "防御").is_some());
        }
        let result = retrieve(
            &d,
            &[
                "烈焰风暴".into(),
                "Flare Storm".into(),
                "启动巨鹫之像".into(),
                "股票".into(),
            ],
        )
        .unwrap();
        assert_eq!(result.links.iter().filter(|(_, route)| route.starts_with('#')).count(), 2);
        assert!(result.text.contains("威力 200") && result.text.contains("初始台地"));
        let effects = retrieve(&d, &["负重上限".into()]).unwrap();
        assert!(effects.text.contains("负重上限") && !effects.links.is_empty());
        let items = discover(&d, "有没有能提升负重上限的饰品？").unwrap();
        assert!(
            items.text.contains("提升负重上限")
                && items.links.iter().all(|l| l.1.starts_with("#items/"))
        );
    }

    #[test]
    fn planned_element_search_returns_direction_and_shares_the_chart_with_named_pals() {
        let d = handbook();
        // A player asks how to fight a dragon without knowing any specific counter.
        let elements = retrieve(&d, &["龙属性".into(), "火属性".into()]).unwrap();
        assert!(
            elements.text.contains("冰") && elements.text.contains("暗"),
            "{}",
            elements.text
        );
        assert!(
            elements.text.contains("水") && elements.text.contains("草"),
            "{}",
            elements.text
        );
        assert_eq!(
            elements
                .links
                .iter()
                .filter(|(_, href)| href == "#guide/elements")
                .count(),
            1
        );
        for plan in [vec!["空涡龙", "弱点"], vec!["空涡龙的弱点"]] {
            let terms = plan.into_iter().map(str::to_owned).collect::<Vec<_>>();
            let answer = retrieve(&d, &terms).unwrap();
            assert!(
                answer.text.contains("空涡龙") && answer.text.contains("冰"),
                "{}",
                answer.text
            );
            assert!(
                answer
                    .links
                    .iter()
                    .any(|(_, href)| href == "#guide/elements")
            );
            assert!(
                answer
                    .links
                    .iter()
                    .any(|(_, href)| href == "#paldex/JetDragon")
            );
        }
        for term in ["冥王指挥棒", "龙属性伤害的饰品", "空涡龙"] {
            assert!(
                retrieve(&d, &[term.into()]).is_none_or(|answer| answer
                    .links
                    .iter()
                    .all(|(_, href)| href != "#guide/elements")),
                "{term}"
            );
        }
    }

    #[test]
    fn model_keywords_rank_actual_mechanics_and_honor_equipment_categories() {
        let d = handbook();
        for terms in [vec!["负重 饰品"], vec!["负重饰品"], vec!["饰品", "负重"]] {
            let terms: Vec<_> = terms.into_iter().map(str::to_owned).collect();
            let answer = retrieve(&d, &terms).unwrap();
            assert!(answer.text.lines().all(|line| line.contains("负重")));
            for (_, href) in answer.links {
                assert!(
                    array(d.get("search-index-data"))
                        .iter()
                        .any(|row| text(row, "href") == href && text(row, "hint") == "饰品")
                );
            }
        }
        let both = retrieve(&d, &["耐寒负重饰品".into()]).unwrap();
        let first = both.text.lines().next().unwrap();
        assert!(first.contains("负重") && first.contains("耐寒"), "{first}");
        let compound = retrieve(&d, &["采矿负重装备".into()]).unwrap();
        assert!(
            compound.text.contains("采矿") && compound.text.contains("负重"),
            "{}",
            compound.text
        );
        let plan = retrieve(&d, &["挖矿".into(), "背包".into(), "负重".into()]).unwrap();
        let first_choices = plan.text.lines().take(2).collect::<String>();
        assert!(
            first_choices.contains("采矿") && first_choices.contains("负重"),
            "{first_choices}"
        );
        assert!(plan.text.contains("+300") && !plan.text.contains("扩展背包"));
        assert!(retrieve(&d, &["装备".into(), "游戏".into(), "背包".into()]).is_none());
    }

    #[test]
    fn descriptive_queries_need_specific_evidence_and_exact_names_still_resolve() {
        let d = handbook();
        for plan in [
            vec!["Feybreak", "strongest pal", "post-Feybreak"],
            vec!["mysterious unobtainium stone"],
            vec!["PAL_Stone"],
            vec!["最强帕鲁", "更新", "版本"],
            vec!["pal", "Palworld", "best item"],
        ] {
            let terms: Vec<_> = plan.into_iter().map(str::to_owned).collect();
            assert!(retrieve(&d, &terms).is_none(), "{terms:?}");
        }
        assert!(!matches_effect_line("PAL_Growth_Stone_S", "stone"));
        assert!(!matches_effect_line("increase your score", "ore"));
        assert!(matches_effect_line("A MYSTERIOUS stone.", "mysterious"));
        for (term, expected) in [
            ("High Quality Pal Oil", "#items/PalOil"),
            ("PAL_Growth_Stone_S", "#items/PAL_Growth_Stone_S"),
            ("帕魯球", "#items/PalSphere"),
        ] {
            let answer = retrieve(&d, &[term.into()]).unwrap();
            assert!(answer.links.iter().any(|(_, link)| link == expected), "{term}: {}", answer.text);
        }
        for (term, expected) in [("work speed", "工作速度"), ("carrying capacity", "负重")] {
            let answer = retrieve(&d, &[term.into()]).unwrap();
            assert!(answer.text.contains(expected), "{term}: {}", answer.text);
        }
    }

    #[test]
    fn cake_crafting_omits_unrelated_sources_and_grounds_station_level() {
        let d = handbook();
        let result = discover(&d, "蛋糕怎么合成").unwrap();
        assert!(result.text.contains("面粉 × 5") && result.text.contains("蜂蜜 × 2"));
        assert!(result.text.contains("料理锅在科技 17 级"));
        assert!(!result.text.contains("任务奖励") && !result.text.contains("帕鲁掉落"));
        let mut session = crate::assistant_data::ReferenceSession::default();
        let answer = session.respond(&d, &Value::Null, "蛋糕怎么合成").unwrap();
        assert!(!answer.text.contains("任务奖励") && answer.text.contains("牛奶 × 7"));
        assert!(
            answer
                .links
                .iter()
                .any(|x| x.1 == "#work/facilities/Cooking_Pot")
        );
    }
}
