//! Bounded, offline fuzzy matching for names, identifiers and descriptive text.
use crate::data::{Data, array, normalize, text};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct TraitIndex {
    rows: HashMap<String, TraitText>,
}
struct TraitText {
    names: Vec<String>,
    body: String,
    compact_body: String,
}
fn compact(value: &str) -> String {
    normalize(value)
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
impl TraitIndex {
    pub fn new(traits: &Value) -> Self {
        let mut index = Self::default();
        for row in array(&traits["traits"]) {
            index.insert(text(row, "key"), row);
        }
        index
    }
    pub fn add_details(&mut self, details: &Value) {
        if let Some(rows) = details["passives"].as_object() {
            for (key, row) in rows {
                self.insert(key, row);
            }
        }
    }
    fn insert(&mut self, key: &str, row: &Value) {
        if key.is_empty() {
            return;
        }
        let names = [text(row, "zh"), text(row, "en"), key]
            .into_iter()
            .filter(|name| !name.is_empty())
            .map(compact)
            .collect();
        let body = normalize(&format!("{key} {row}"));
        let compact_body = compact(&body);
        self.rows.insert(
            key.to_owned(),
            TraitText {
                names,
                body,
                compact_body,
            },
        );
    }
    pub fn score(&self, key: &str, query: &str) -> Option<u32> {
        let row = self.rows.get(key)?;
        let query = normalize(query.trim());
        if query.is_empty() {
            return Some(0);
        }
        let needle = compact(&query);
        if needle.is_empty() {
            return row.body.contains(&query).then_some(60);
        }
        let letters: Vec<char> = needle.chars().collect();
        let mut score = None;
        for (i, name) in row.names.iter().enumerate() {
            let quality = if name == &needle {
                Some(i as u32)
            } else if name.contains(&needle) {
                Some(12 + i as u32)
            } else if letters.len() >= 2 && letters.len() <= 64 && subsequence(&letters, name) {
                Some(35 + (name.chars().count().saturating_sub(letters.len()).min(20) as u32))
            } else {
                None
            };
            score = minimum(score, quality);
            let name: Vec<char> = name.chars().collect();
            let swapped_pair = letters.len() == 2
                && name.len() == 2
                && letters[0] == name[1]
                && letters[1] == name[0];
            let limit = if letters.len() >= 7 { 2 } else { 1 };
            if (letters.len() >= 3 || swapped_pair)
                && letters.len() <= 64
                && name.len().abs_diff(letters.len()) <= limit
            {
                let distance = edit_distance(&letters, &name);
                if distance <= limit {
                    score = minimum(score, Some(80 + distance as u32 * 10 + i as u32));
                }
            }
        }
        if query.split_whitespace().all(|part| row.body.contains(part)) {
            score = minimum(score, Some(60));
        } else if row.compact_body.contains(&needle) {
            score = minimum(score, Some(65));
        }
        score
    }
}
fn minimum(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn subsequence(needle: &[char], haystack: &str) -> bool {
    let mut index = 0;
    for c in haystack.chars() {
        if index < needle.len() && needle[index] == c {
            index += 1;
        }
    }
    index == needle.len()
}
// Optimal string alignment also accepts a mistyped adjacent pair in English.
fn edit_distance(left: &[char], right: &[char]) -> usize {
    let mut previous_previous = Vec::new();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (i, a) in left.iter().enumerate() {
        let mut current = vec![i + 1; right.len() + 1];
        for (j, b) in right.iter().enumerate() {
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + usize::from(a != b));
            if i > 0 && j > 0 && a == &right[j - 1] && &left[i - 1] == b {
                current[j + 1] = current[j + 1].min(previous_previous[j - 1] + 1);
            }
        }
        previous_previous = previous;
        previous = current;
    }
    previous[right.len()]
}
pub fn trait_score(d: &Data, key: &str, query: &str) -> Option<u32> {
    d.trait_search.score(key, query)
}
pub fn trait_matches(d: &Data, key: &str, query: &str) -> bool {
    trait_score(d, key, query).is_some()
}

/// An exact display name always precedes a scoped alias, then approximate matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    Exact,
    Alias,
    Fuzzy,
}
impl MatchKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Exact => "精确匹配",
            Self::Alias => "别名匹配",
            Self::Fuzzy => "模糊匹配",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchQuality {
    pub score: u32,
    pub match_kind: MatchKind,
    pub explanation: String,
}
#[derive(Clone, Debug)]
struct CachedTerm {
    display: String,
    compact: String,
    chars: Vec<char>,
    sorted: Vec<char>,
}
impl CachedTerm {
    fn new(value: &str) -> Self {
        let display: String = value.chars().take(128).collect();
        let compact = compact(&display);
        let chars: Vec<_> = compact.chars().collect();
        let mut sorted = chars.clone();
        sorted.sort_unstable();
        Self {
            display,
            compact,
            chars,
            sorted,
        }
    }
}

/// Prepare once for an entire map/catalogue filter. Oversized input is rejected,
/// rather than silently truncating it into an unrelated, apparently exact result.
#[derive(Clone, Debug)]
pub struct PreparedQuery {
    term: CachedTerm,
    parts: Vec<String>,
    valid: bool,
    empty: bool,
}
impl PreparedQuery {
    pub fn new(query: &str) -> Self {
        if query.chars().take(129).count() > 128 {
            return Self {
                term: CachedTerm::new(""),
                parts: vec![],
                valid: false,
                empty: false,
            };
        }
        let query = normalize(query.trim());
        let term = CachedTerm::new(&query);
        let parts: Vec<_> = query
            .split(|c: char| {
                c.is_whitespace() || matches!(c, ',' | '，' | '、' | ';' | '；' | '/' | '|')
            })
            .map(compact)
            .filter(|s| !s.is_empty())
            .take(9)
            .collect();
        let empty = query.is_empty();
        let valid = empty || (!term.chars.is_empty() && term.chars.len() <= 64 && parts.len() <= 8);
        Self {
            term,
            parts,
            valid,
            empty,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.valid
    }
    pub fn is_empty(&self) -> bool {
        self.empty
    }
}

/// Cached text usable independently of the global search window. Only source
/// names/aliases are spell-corrected; hints are searched literally or by tokens.
#[derive(Clone, Debug)]
pub struct CachedText {
    name: CachedTerm,
    aliases: Vec<CachedTerm>,
    body: String,
}
impl CachedText {
    pub fn new(name: &str, hint: &str, aliases: &[String]) -> Self {
        let name = CachedTerm::new(name);
        let mut cached_aliases = Vec::<CachedTerm>::new();
        for alias in aliases.iter().take(48) {
            let alias = CachedTerm::new(alias);
            if !alias.compact.is_empty()
                && alias.compact != name.compact
                && !cached_aliases.iter().any(|a| a.compact == alias.compact)
            {
                cached_aliases.push(alias);
            }
        }
        let body = compact(&format!(
            "{} {} {}",
            name.compact,
            cached_aliases
                .iter()
                .map(|a| a.compact.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            hint.chars().take(1024).collect::<String>()
        ));
        Self {
            name,
            aliases: cached_aliases,
            body,
        }
    }
    pub fn from_row(row: &Value) -> Self {
        let mut aliases: Vec<String> = match &row["aliases"] {
            Value::String(value) => vec![value.clone()],
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => vec![],
        };
        let href = text(row, "href");
        if !href.is_empty() {
            aliases.push(href.to_owned());
        }
        let name = text(row, "name");
        let canonical_name = compact(name);
        let category = text(row, "category");
        let scoped = |key: &str| {
            category == key
                || aliases.iter().any(|a| a == key)
                || (href.starts_with("#map/") && href.contains(key))
        };
        let anti_air = scoped("anti_air")
            || canonical_name == "防空炮"
            || (canonical_name.contains("天坠")
                && (canonical_name.contains("防空炮") || canonical_name.ends_with("炮台")));
        let extra: &[&str] = if anti_air {
            &[
                "防空炮",
                "防空炮台",
                "天坠之地炮台",
                "天坠之地防空炮",
                "天坠之民据点防空炮",
                "高射炮",
                "Anti-Air",
                "Anti-Aircraft",
                "Anti-Aircraft Cannon",
                "Anti Air Gun",
                "Air Defense Cannon",
                "anti_air",
            ]
        } else if scoped("fast_travel") {
            &["传送点", "快速旅行", "巨鹫之像", "Fast Travel"]
        } else if scoped("journal")
            || text(row, "kind") == "手记"
            || href.starts_with("#guide/journals")
        {
            &["手记", "探索手记", "日记", "Journal", "Diary"]
        } else if scoped("merchant")
            || text(row, "kind") == "商人"
            || href.starts_with("#merchants")
        {
            &["商人", "商贩", "Merchant", "Trader"]
        } else if text(row, "kind") == "手术台" || href.starts_with("#details/surgery") {
            &["手术台", "帕鲁手术台", "手术", "Pal Surgery"]
        } else {
            &[]
        };
        aliases.extend(extra.iter().map(|s| (*s).to_owned()));
        Self::new(
            name,
            &format!("{} {}", text(row, "hint"), text(row, "kind")),
            &aliases,
        )
    }
    pub fn score(&self, query: &str) -> Option<MatchQuality> {
        self.match_query(&PreparedQuery::new(query))
    }
    pub fn match_query(&self, query: &PreparedQuery) -> Option<MatchQuality> {
        if !query.valid {
            return None;
        }
        if query.empty {
            return Some(quality(0, MatchKind::Exact, "全部条目"));
        }
        if self.name.compact == query.term.compact {
            return Some(quality(0, MatchKind::Exact, "名称完全匹配"));
        }
        let mut best = match_term(&self.name, query, false);
        // One-character searches are literal names only. They never trigger
        // synonyms, edit distance, hints or arbitrary subsequences.
        if query.term.chars.len() == 1 {
            return best;
        }
        for alias in &self.aliases {
            prefer(&mut best, match_term(alias, query, true));
        }
        if query.parts.len() >= 2 && query.parts.iter().all(|p| self.body.contains(p)) {
            prefer(
                &mut best,
                Some(quality(105, MatchKind::Fuzzy, "多个关键词匹配资料")),
            );
        } else if self.body.contains(&query.term.compact) {
            prefer(
                &mut best,
                Some(quality(110, MatchKind::Fuzzy, "资料文字包含关键词")),
            );
        }
        best
    }
}
fn quality(score: u32, match_kind: MatchKind, explanation: impl Into<String>) -> MatchQuality {
    MatchQuality {
        score,
        match_kind,
        explanation: explanation.into(),
    }
}
fn prefer(best: &mut Option<MatchQuality>, candidate: Option<MatchQuality>) {
    if let Some(candidate) = candidate {
        if best.as_ref().is_none_or(|old| candidate.score < old.score) {
            *best = Some(candidate);
        }
    }
}
fn match_term(term: &CachedTerm, query: &PreparedQuery, alias: bool) -> Option<MatchQuality> {
    let needle = &query.term;
    if term.compact.is_empty() {
        return None;
    }
    if term.compact == needle.compact {
        return Some(quality(
            if alias { 10 } else { 0 },
            if alias {
                MatchKind::Alias
            } else {
                MatchKind::Exact
            },
            if alias {
                format!("别名匹配：{}", term.display)
            } else {
                "名称完全匹配".into()
            },
        ));
    }
    if term.compact.contains(&needle.compact) {
        return Some(quality(
            if alias { 38 } else { 20 }
                + term.chars.len().saturating_sub(needle.chars.len()).min(12) as u32,
            if alias {
                MatchKind::Alias
            } else {
                MatchKind::Fuzzy
            },
            if alias {
                format!("别名包含：{}", term.display)
            } else {
                "名称包含关键词".into()
            },
        ));
    }
    if needle.chars.len() < 2 {
        return None;
    }
    let mut best = None;
    if term.chars.len() <= (needle.chars.len() * 4).max(16)
        && subsequence(&needle.chars, &term.compact)
    {
        best = Some(quality(
            55 + u32::from(alias) * 5
                + term.chars.len().saturating_sub(needle.chars.len()).min(20) as u32,
            MatchKind::Fuzzy,
            if alias {
                format!("别名省略匹配：{}", term.display)
            } else {
                "名称省略匹配".into()
            },
        ));
    }
    if query.parts.len() >= 2 && query.parts.iter().all(|p| term.compact.contains(p)) {
        prefer(
            &mut best,
            Some(quality(
                50 + u32::from(alias) * 5,
                MatchKind::Fuzzy,
                "关键词分段匹配（字序可不同）",
            )),
        );
    }
    if reordered_blocks(&needle.chars, &term.compact) {
        prefer(
            &mut best,
            Some(quality(
                74 + u32::from(alias) * 5,
                MatchKind::Fuzzy,
                "名称词组顺序不同",
            )),
        );
    }
    let limit = if needle.chars.len() >= 9 { 2 } else { 1 };
    if (3..=32).contains(&needle.chars.len())
        && term.chars.len().abs_diff(needle.chars.len()) <= limit
        && shared_chars(&needle.sorted, &term.sorted)
            >= term
                .chars
                .len()
                .max(needle.chars.len())
                .saturating_sub(limit)
    {
        let distance = edit_distance(&needle.chars, &term.chars);
        if distance <= limit {
            prefer(
                &mut best,
                Some(quality(
                    85 + distance as u32 * 5 + u32::from(alias) * 3,
                    MatchKind::Fuzzy,
                    if alias {
                        format!("别名近似：{}（可能有错字）", term.display)
                    } else {
                        "名称近似（可能有错字）".into()
                    },
                )),
            );
        }
    }
    best
}
fn shared_chars(left: &[char], right: &[char]) -> usize {
    let (mut i, mut j, mut shared) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                i += 1;
                j += 1;
            }
        }
    }
    shared
}
fn reordered_blocks(needle: &[char], name: &str) -> bool {
    if !(4..=16).contains(&needle.len())
        || !needle.iter().all(|c| ('\u{3400}'..='\u{9fff}').contains(c))
    {
        return false;
    }
    // At most thirteen two-block splits, with at least two characters per block.
    // Do not turn Chinese text into a bag of single, arbitrarily ordered letters.
    (2..=needle.len() - 2).any(|split| {
        let first: String = needle[..split].iter().collect();
        let second: String = needle[split..].iter().collect();
        name.find(&first)
            .zip(name.find(&second))
            .is_some_and(|(a, b)| b + second.len() <= a && a - b - second.len() <= 12)
    })
}

#[derive(Clone, Debug)]
pub struct GlobalRow {
    pub name: String,
    pub hint: String,
    pub kind: String,
    pub href: String,
    pub image: String,
}
#[derive(Clone, Debug)]
pub struct SearchHit {
    pub row_index: usize,
    pub score: u32,
    pub match_kind: MatchKind,
    pub explanation: String,
}
#[derive(Clone, Debug, Default)]
pub struct GlobalIndex {
    rows: Vec<GlobalRow>,
    texts: Vec<CachedText>,
}
impl GlobalIndex {
    pub fn new(rows: &Value) -> Self {
        let values = array(rows);
        Self {
            rows: values
                .iter()
                .map(|row| GlobalRow {
                    name: text(row, "name").into(),
                    hint: text(row, "hint").into(),
                    kind: text(row, "kind").into(),
                    href: text(row, "href").into(),
                    image: text(row, "image").into(),
                })
                .collect(),
            texts: values.iter().map(CachedText::from_row).collect(),
        }
    }
    pub fn rows(&self) -> &[GlobalRow] {
        &self.rows
    }
    pub fn search(&self, query: &str, kind: &str, limit: usize) -> Vec<SearchHit> {
        let query = PreparedQuery::new(query);
        if !query.valid || limit == 0 {
            return vec![];
        }
        let all_kinds = kind.is_empty() || kind == "全部";
        let mut hits: Vec<_> = self
            .texts
            .iter()
            .enumerate()
            .filter(|(i, _)| all_kinds || self.rows[*i].kind == kind)
            .filter_map(|(row_index, row)| {
                row.match_query(&query).map(|found| SearchHit {
                    row_index,
                    score: found.score,
                    match_kind: found.match_kind,
                    explanation: found.explanation,
                })
            })
            .collect();
        hits.sort_by_key(|hit| (hit.score, hit.row_index));
        hits.truncate(limit);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn index() -> TraitIndex {
        let mut index = TraitIndex::new(&json!({"traits":[
            {"key":"CraftSpeed_up2","zh":"工匠精神","en":"Artisan"},
            {"key":"CraftSpeed_up3","zh":"卓绝技艺","en":"Remarkable Craftsmanship"},
            {"key":"Legend","zh":"传说","en":"Legend"}]}));
        index.add_details(&json!({"passives":{"CraftSpeed_up2":{"zh":"工匠精神","en":"Artisan","effect":"工作速度 +50%"}}}));
        index
    }
    #[test]
    fn partial_abbreviated_typo_and_effect_queries_find_the_real_passive() {
        let index = index();
        for query in [
            "工匠",
            "工精神",
            "工匠精伸",
            "工匠精",
            "ＡＲＴＩＳＡＮ",
            "artisna",
            "CraftSpeed_up2",
            "工作速度 50",
            "工作速度50%",
        ] {
            assert!(index.score("CraftSpeed_up2", query).is_some(), "{query}");
        }
        assert!(index.score("CraftSpeed_up3", "卓技").is_some());
        assert_eq!(
            index.score("Legend", "传奇"),
            None,
            "a one-character typo in a short query is too broad"
        );
        assert_eq!(index.score("Legend", "工作速度 50"), None);
        assert_eq!(index.score("Legend", "不相干的查询"), None);
    }
    #[test]
    fn exact_names_outrank_fuzzy_results_and_long_input_is_bounded() {
        let index = index();
        assert!(
            index.score("CraftSpeed_up2", "工匠精神") < index.score("CraftSpeed_up2", "工精神")
        );
        assert!(
            index.score("CraftSpeed_up2", "工精神") < index.score("CraftSpeed_up2", "工匠精伸")
        );
        assert_eq!(index.score("Legend", ""), Some(0));
        assert_eq!(index.score("Legend", &"错".repeat(10000)), None);
        assert_eq!(index.score("missing", ""), None);
    }
    #[test]
    fn global_search_preserves_ambiguous_locations_and_exact_name_priority() {
        let index = GlobalIndex::new(&json!([
            {"name":"天坠之地炮台","kind":"地图点位","href":"#map/point/anti_air-a","category":"anti_air","hint":"(-608,-1146)"},
            {"name":"天坠之民据点 · 防空炮","kind":"地图点位","href":"#map/point/anti_air-b","category":"anti_air","hint":"(-993,-1453)","image":"icons/anti_air.png"},
            {"name":"防空炮","kind":"设施","href":"#work/facilities/test"},
            {"name":"高射炮","kind":"物品","href":"#items/cannon"},
            {"name":"普通炮台","kind":"设施","href":"#work/facilities/cannon"}
        ]));
        let hits = index.search("防空炮", "", usize::MAX);
        assert_eq!(hits.len(), 3);
        assert_eq!(
            hits[0].row_index, 2,
            "exact display name must precede synonyms"
        );
        assert_eq!(hits[0].match_kind, MatchKind::Exact);
        assert_eq!(hits[1].match_kind, MatchKind::Alias);
        assert!(hits[1].explanation.contains("防空炮"));
        assert_eq!(index.rows()[1].image, "icons/anti_air.png");
        let hits = index.search("Anti-Air", "地图点位", usize::MAX);
        assert_eq!(hits.iter().map(|h| h.row_index).collect::<Vec<_>>(), [0, 1]);
        assert_ne!(
            index.rows()[hits[0].row_index].href,
            index.rows()[hits[1].row_index].href
        );
        let hits = index.search("高射炮", "", 1);
        assert_eq!(hits[0].row_index, 3);
        assert_eq!(hits[0].match_kind, MatchKind::Exact);
        assert!(
            index.search("Anti-Air", "物品", usize::MAX).is_empty(),
            "ordinary artillery must not acquire anti-air aliases"
        );
        assert!(index.search("防空炮", "", 0).is_empty());
    }
    #[test]
    fn generic_names_support_omission_typos_and_bounded_word_order() {
        let cache = CachedText::new("生命果实", "提升帕鲁生命值", &["Life Fruit".into()]);
        assert_eq!(
            cache.score("生命果实").unwrap().match_kind,
            MatchKind::Exact
        );
        for query in [
            "生命",
            "生果",
            "生命果是",
            "果实生命",
            "果实 生命",
            "ＬＩＦＥ ＦＲＵＩＴ",
            "life fruti",
            "提升 生命",
        ] {
            assert!(cache.score(query).is_some(), "{query}");
        }
        assert!(
            cache.score("果生实命").is_none(),
            "arbitrary character permutations must not match"
        );
        assert!(cache.score("生生生生").is_none());
        let short = CachedText::new("传说", "", &[]);
        assert!(
            short.score("传奇").is_none(),
            "two-character typo would guess too broadly"
        );
        assert!(cache.score(&"错".repeat(10000)).is_none());
        assert!(cache.score(&"a".repeat(65)).is_none());
        assert!(!PreparedQuery::new(&"a ".repeat(9)).is_valid());
        assert!(!PreparedQuery::new("!!!").is_valid());
        assert!(PreparedQuery::new("  ").is_empty());
        assert_eq!(cache.score("生").unwrap().match_kind, MatchKind::Fuzzy);
        assert!(
            cache.score("提").is_none(),
            "one character cannot expand from a hint"
        );
        assert!(
            cache.score("f").is_none(),
            "one character cannot expand from an alias"
        );
    }
    #[test]
    fn common_aliases_are_scoped_to_their_real_catalogues() {
        let rows = json!([
            {"name":"余烬熔岩台","category":"fast_travel","href":"#map/point/fast-travel-1"},
            {"name":"佐伊 1","kind":"手记","href":"#guide/journals/zoe-1"},
            {"name":"渔夫","kind":"商人","href":"#merchants/fisher"},
            {"name":"耗材植入体","kind":"手术台","href":"#details/surgery/implant"},
            {"name":"佐伊","kind":"敌人","href":"#enemies/zoe"},
            {"name":"普通火炮","kind":"物品","href":"#items/cannon","hint":""}
        ]);
        let caches: Vec<_> = array(&rows).iter().map(CachedText::from_row).collect();
        for (query, target) in [
            ("巨鹫之像", 0),
            ("快速旅行", 0),
            ("日记", 1),
            ("商贩", 2),
            ("手术", 3),
        ] {
            let query = PreparedQuery::new(query);
            let hits: Vec<_> = caches
                .iter()
                .enumerate()
                .filter_map(|(i, c)| c.match_query(&query).map(|m| (i, m)))
                .collect();
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].0, target);
            assert_eq!(hits[0].1.match_kind, MatchKind::Alias);
        }
        assert!(caches[0].score("鹫").is_none());
        assert!(caches[1].score("记").is_none());
        assert!(caches[4].score("日记").is_none());
        assert!(caches[5].score("高射炮").is_none());
    }
    #[test]
    fn shipped_anti_air_points_and_pals_are_found_without_renaming_them() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut rows: Value =
            serde_json::from_slice(&std::fs::read(root.join("search-index-data.json")).unwrap())
                .unwrap();
        let map: Value =
            serde_json::from_slice(&std::fs::read(root.join("map-data.json")).unwrap()).unwrap();
        let points: Vec<_> = array(&map["points"])
            .iter()
            .filter(|p| text(p, "category") == "anti_air")
            .collect();
        assert!(points.len() >= 9);
        for point in &points {
            rows.as_array_mut().unwrap().push(json!({
                "name":point["name"],"kind":"地图点位","category":point["category"],
                "href":format!("#map/point/{}",text(point,"id")),
                "hint":format!("{} ({}, {})",text(point,"nearest"),point["x"],point["y"])
            }));
        }
        let index = GlobalIndex::new(&rows);
        for query in [
            "防空炮",
            "天坠之地炮台",
            "防空炮台",
            "高射炮",
            "Anti-Air",
            "防空袍",
            "炮台 天坠",
        ] {
            let hits = index.search(query, "地图点位", usize::MAX);
            assert_eq!(
                hits.len(),
                points.len(),
                "{query}: every distinct location remains a candidate"
            );
            for hit in hits {
                assert_eq!(index.rows()[hit.row_index].name, "天坠之民据点 · 防空炮");
                assert!(!index.rows()[hit.row_index].hint.is_empty());
            }
        }
        assert_eq!(
            index.rows()[index.search("棉悠悠", "", 1)[0].row_index].href,
            "#paldex/SheepBall"
        );
        assert_eq!(
            index.rows()[index.search("绵悠悠", "帕鲁", 1)[0].row_index].href,
            "#paldex/SheepBall"
        );
        assert!(
            index
                .search(&"未知".repeat(10000), "", usize::MAX)
                .is_empty()
        );
    }
}
