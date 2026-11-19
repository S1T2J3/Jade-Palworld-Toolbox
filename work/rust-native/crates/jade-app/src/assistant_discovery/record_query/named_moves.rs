use super::{AssistantReply, Data, Value, array, text};
use crate::assistant_breeding::species_mentions;
use crate::assistant_engine::field_text;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub(super) enum Resolution<'a> {
    One(&'a str, &'a Value),
    Shared(AssistantReply),
}

// The existing versioned PalCalc ActiveSkills records provide traditional Chinese
// and Japanese titles by InternalName. Chinese/English display names, teaching
// items, numeric fields and natural learners come from the loaded handbook.
fn localizations() -> &'static BTreeMap<&'static str, [String; 2]> {
    static TABLE: OnceLock<BTreeMap<&str, [String; 2]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let names: BTreeMap<&str, [&str; 2]> =
            serde_json::from_str(include_str!("move_aliases.json"))
                .expect("bundled PalCalc localized move names");
        names
            .into_iter()
            .map(|(key, labels)| (key, labels.map(field_text)))
            .collect()
    })
}

fn learns(book: &Value, pal: &str, key: &str) -> bool {
    array(&book["profiles"][pal]["levels"])
        .iter()
        .any(|v| v[1].as_str() == Some(key))
}

pub(super) fn resolve<'a>(d: &'a Data, q: &str) -> Option<Resolution<'a>> {
    let book = d.get("pal-details-data");
    let moves = book["moves"].as_object()?;
    let aliases = localizations();
    let mut spans = BTreeMap::<(usize, usize), BTreeSet<&str>>::new();
    for (key, row) in moves {
        let chinese = field_text(text(row, "zh"));
        let english = field_text(text(row, "en"));
        let fruit = text(row, "fruit")
            .split_once('：')
            .map(|(_, name)| field_text(name));
        let extra = aliases.get(key.as_str()).into_iter().flatten();
        for name in [chinese.as_str(), english.as_str()]
            .into_iter()
            .chain(fruit.as_deref())
            .chain(extra.map(String::as_str))
            .filter(|s| !s.is_empty())
        {
            for (start, _) in q.match_indices(name) {
                let end = start + name.len();
                if !name.is_ascii()
                    || (!q[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
                        && !q[end..].starts_with(|c: char| c.is_ascii_alphanumeric()))
                {
                    spans.entry((start, end)).or_default().insert(key);
                }
            }
        }
    }
    // A shorter title can be part of a different one ("Fierce Fang (Electric)").
    // Keep every ID sharing the complete span, not whichever record sorted last.
    let all = spans.keys().copied().collect::<Vec<_>>();
    spans.retain(|&(a, b), _| {
        !all.iter()
            .any(|&(start, end)| (a, b) != (start, end) && a >= start && b <= end)
    });
    let mut groups = spans.values();
    let mut ids = groups.next()?.clone();
    for group in groups {
        ids.retain(|id| group.contains(id));
    }
    if ids.is_empty() {
        return None;
    }
    let mut subject = q.to_owned();
    let mut previous = q.len();
    for &(start, end) in spans.keys().rev() {
        if end > previous {
            return None;
        }
        subject.replace_range(start..end, " ");
        previous = start;
    }
    let (pals, _) = species_mentions(&d.species, &subject);
    if !pals.is_empty() {
        ids.retain(|id| {
            pals.iter()
                .all(|(i, _, _)| learns(book, text(&d.species[*i], "key"), id))
        });
    }
    let &first = ids.first()?;
    if ids.len() == 1 {
        return Some(Resolution::One(first, &moves[first]));
    }
    let mut lines = vec!["图鉴里有多个同名招式。请按使用它的帕鲁选一条：".to_owned()];
    let mut links = Vec::new();
    for id in ids {
        let row = &moves[id];
        let owners = d
            .species
            .iter()
            .filter(|p| learns(book, text(p, "key"), id))
            .map(|p| d.pal_name(text(p, "key")))
            .collect::<Vec<_>>()
            .join("、");
        let label = format!("{}（{}）", text(row, "zh"), owners);
        lines.push(format!(
            "{label}：基础威力 {}，基础冷却 {} 秒。",
            row["power"], row["cooldown"]
        ));
        links.push((label, format!("#details/active/move/{id}")));
    }
    Some(Resolution::Shared(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn localized_moves_share_fields_and_keep_distinct_natural_learners() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for table in ["pal-details-data", "combat-guide-data"] {
            let bytes = std::fs::read(root.join(format!("{table}.json"))).unwrap();
            data.tables.insert(
                table.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        for (q, id) in [
            ("等離子浮游砲的威力和冷卻？", "ThunderFunnel"),
            ("等离子浮游炮的冷却几秒？", "ThunderFunnel"),
            ("Ｐｌａｓｍａ　Ｆｕｎｎｅｌ的威力？", "ThunderFunnel"),
            ("プラズマファンネルのクールタイムは？", "ThunderFunnel"),
            (
                "Fuddler Tunneler cooldown and power?",
                "Unique_CuteMole_DiggingAttack",
            ),
            (
                "How much power does Direhowl's Fierce Fang have?",
                "Unique_Garm_Bite",
            ),
        ] {
            let answer = Conversation::default().ask_world(q, &data, &serde_json::json!({}));
            let row = &data.get("pal-details-data")["moves"][id];
            assert!(
                answer.text.contains(&format!("基础威力 {}", row["power"])),
                "{q}: {}",
                answer.text
            );
            assert!(
                answer
                    .links
                    .iter()
                    .any(|(_, path)| path == &format!("#details/active/move/{id}")),
                "{q}: {:?}",
                answer.links
            );
        }
        let q = field_text("Double Fang (Electric) 的威力？");
        let Some(Resolution::Shared(answer)) = resolve(&data, &q) else {
            panic!("{q} requires a choice")
        };
        assert_eq!(answer.links.len(), 2);
        assert!(
            answer
                .links
                .iter()
                .all(|(_, path)| path.contains("ElecPomeranian") || path.contains("ThunderDog"))
        );
        assert!(resolve(&data, "hydrojetting").is_none());
        assert!(resolve(&data, "hydro jet and plasma funnel").is_none());
    }
}
