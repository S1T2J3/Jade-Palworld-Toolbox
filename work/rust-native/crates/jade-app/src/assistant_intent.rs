use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const CLASS_COUNT: usize = 7;

#[derive(Deserialize)]
struct Model {
    labels: [String; CLASS_COUNT],
    threshold: f64,
    margin: f64,
    // idf × 100, followed by the component of each unit-length centroid × 1000.
    features: HashMap<String, [u16; CLASS_COUNT + 1]>,
}

/// A suggestion to the dialogue engine. Ambiguity, unrelated questions and
/// unknown wording fall through to regular search or a clarification.
pub fn classify(text: &str) -> Option<&'static str> {
    static MODEL: OnceLock<Option<Model>> = OnceLock::new();
    let model = MODEL
        .get_or_init(|| serde_json::from_str(include_str!("assistant_intent_model.json")).ok())
        .as_ref()?;
    let chars: Vec<char> = text
        .chars()
        .map(|ch| ch.to_ascii_lowercase())
        .filter(|ch| matches!(ch, 'a'..='z' | '0'..='9' | '\u{3400}'..='\u{9fff}'))
        .take(513)
        .collect();
    if chars.len() < 2 || chars.len() > 512 {
        return None;
    }

    let mut seen = HashSet::new();
    let mut scores = [0.0f64; CLASS_COUNT];
    let mut squared_norm = 0.0;
    for size in 1..=3 {
        for window in chars.windows(size) {
            let term: String = window.iter().collect();
            let Some(values) = model.features.get(&term) else {
                continue;
            };
            if !seen.insert(term) {
                continue;
            }
            let weight = f64::from(values[0]);
            squared_norm += weight * weight;
            for (sum, component) in scores.iter_mut().zip(&values[1..]) {
                *sum += weight * f64::from(*component);
            }
        }
    }
    if squared_norm == 0.0 {
        return None;
    }
    let norm = squared_norm.sqrt() * 1000.0;
    let mut ranked = scores.iter().copied().enumerate().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (index, first) = ranked[0];
    let second = ranked[1].1;
    let label = model.labels[index].as_str();
    (label != "other"
        && first >= model.threshold * norm
        && first - second >= model.margin * norm)
        .then_some(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_model_finds_a_logistics_request() {
        assert_eq!(classify("矿石挖出来了没运走"), Some("ore_transport"));
    }

    #[test]
    fn the_embedded_model_can_abstain() {
        assert_eq!(classify(""), None);
        assert_eq!(classify("!😄!"), None);
        assert_eq!(classify("请帮我写一封邮件"), None);
    }
}
