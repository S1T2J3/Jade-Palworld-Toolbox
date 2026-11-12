//! Bounded, offline fuzzy matching for passive names, identifiers and effects.
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
}
