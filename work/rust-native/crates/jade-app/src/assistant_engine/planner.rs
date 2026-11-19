use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Planner {
    weights: BTreeMap<String, BTreeMap<String, u8>>,
    category: BTreeMap<String, Vec<String>>,
}

/// Retrieve one supported mechanic constrained to the equipment/skill category the
/// player actually requested. Abstain on negation or ambiguity to protect grounding.
pub(super) fn terms(question: &str) -> Vec<String> {
    static MODEL: OnceLock<Planner> = OnceLock::new();
    let q = question.to_lowercase();
    if q.chars().count() > 512 || [
        "不要", "不考虑", "除了", "不想要", "不需要", "用不着", "不用加", "不想加", "不在乎",
        "现实", "汽车", "手机", "电脑", "股票", "上班",
    ].iter().any(|word| q.contains(word)) {
        return vec![];
    }
    let model = MODEL.get_or_init(|| {
        serde_json::from_str(include_str!("planner.json")).expect("validated mechanic planner")
    });
    let Some(category) = ["饰品", "被动词条", "伙伴技能", "装备", "消耗品"].into_iter().find(|key| {
        model.category[*key].iter().any(|phrase| q.contains(phrase))
    }) else {
        return vec![];
    };
    let mut matches = model.weights.iter().filter(|(_, entries)| {
        let mut fragments: Vec<&String> = Vec::new();
        for (phrase, weight) in *entries {
            if !q.contains(phrase) { continue; }
            if *weight >= 10 || fragments.iter().any(|other| {
                !other.contains(phrase) && !phrase.contains(other.as_str())
            }) {
                return true;
            }
            fragments.push(phrase);
        }
        false
    });
    match (matches.next(), matches.next()) {
        (Some((term, _)), None) => vec![term.clone(), category.to_owned()],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstains_without_a_category_or_with_conflicting_goals() {
        assert!(terms("给汽车装什么装备能提高载重").is_empty());
        assert!(terms("我最近老是背不动了").is_empty());
        assert!(terms("饰品我不要耐热").is_empty());
        assert!(terms("想同时要负重和攻击力，应该选什么首饰").is_empty());
    }

    #[test]
    fn predicts_mechanics_instead_of_an_item_from_a_teacher_question() {
        for (question, expected) in [
            ("掉落物太沉，我想换个首饰多背点", ["负重上限", "饰品"]),
            ("基地里的木匠想靠被动把干活速度提起来", ["工作速度", "被动词条"]),
            ("打这个守关的想要更抗揍，有啥装备合适", ["防御力", "装备"]),
        ] {
            assert_eq!(terms(question), expected);
        }
    }
}
