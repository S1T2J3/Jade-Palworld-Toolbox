use super::super::{AssistantReply, Data, array, text};
use serde_json::Value;
use std::collections::HashSet;

fn contains_name(q: &str, name: &str) -> bool {
    let name = crate::assistant_engine::field_text(name);
    if name.is_empty() {
        return false;
    }
    q.match_indices(&name).any(|(i, _)| {
        !name.is_ascii()
            || (!q[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
                && !q[i + name.len()..].starts_with(|c: char| c.is_ascii_alphanumeric()))
    })
}
fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|s| q.contains(s))
}
fn named(q: &str, row: &Value) -> bool {
    ["zh", "name", "en", "key"]
        .iter()
        .any(|k| contains_name(q, text(row, k)))
        || array(&row["aliases"])
            .iter()
            .filter_map(Value::as_str)
            .any(|s| contains_name(q, s))
}
fn item<'a>(data: &'a Data, id: &str) -> Option<&'a Value> {
    array(&data.get("item-catalog-data")["items"])
        .iter()
        .find(|r| text(r, "id") == id)
}
fn named_item(q: &str, row: &Value) -> bool {
    if named(q, row) {
        return true;
    }
    // Shipped names sometimes include the game name ("High Quality Pal Oil")
    // or an explicit possessive noun ("水栖帕鲁的黏液").
    let english = text(row, "en");
    let short = english
        .split_whitespace()
        .filter(|s| !s.eq_ignore_ascii_case("pal"))
        .collect::<Vec<_>>();
    if short.len() >= 3 && contains_name(q, &short.join(" ")) {
        return true;
    }
    text(row, "name")
        .rsplit_once("帕鲁的")
        .is_some_and(|(_, s)| s.chars().count() >= 2 && contains_name(q, s))
}
fn label<'a>(r: &'a Value, english: bool) -> &'a str {
    let key = if english {
        "en"
    } else if r.get("name").is_some() {
        "name"
    } else {
        "zh"
    };
    text(r, key)
}
#[derive(Clone, Copy, PartialEq)]
enum Intent {
    Drop,
    Learn,
    Habitat,
    Time,
    Unlock,
    Reward,
}
impl Intent {
    fn key(self) -> &'static str {
        match self {
            Self::Drop => "drop:",
            Self::Learn => "learn:",
            Self::Habitat => "habitat:",
            Self::Time => "time:",
            Self::Unlock => "unlock:",
            Self::Reward => "reward:",
        }
    }
    fn from(q: &str) -> Option<Self> {
        // Table rows describe one relation, not the causes, modifiers,
        // inheritance or guarantees of a different outcome.
        if has(
            q,
            &[
                "如果",
                "是否因此",
                "保证",
                "继承",
                "遗传",
                "是否提高",
                "会提高",
                "导致",
                "翻倍",
                "浓缩",
                "随机",
                "必定",
                "唯一",
                "必出",
                "only way",
                "only source",
                "always drops",
                " if ",
                "if i ",
                "prove",
                "guarantee",
                "automatically",
                "inherit",
                "double",
                "faster",
                "increase",
                "modify",
                "randomiz",
                "instead of",
                "compared to",
            ],
        ) {
            return None;
        }
        if has(q, &["远征", "派队", "expedition", "send a squad"]) {
            return if has(q, &["解锁", "前置", "unlock", "tower", "cleared"]) {
                Some(Self::Unlock)
            } else if has(
                q,
                &[
                    "多久",
                    "耗时",
                    "时长",
                    "多少分钟",
                    "分钟",
                    "how long",
                    "duration",
                    "minutes",
                ],
            ) {
                Some(Self::Time)
            } else if has(
                q,
                &[
                    "奖励",
                    "带回",
                    "报酬",
                    "reward",
                    "bring home",
                    "return with",
                ],
            ) {
                Some(Self::Reward)
            } else {
                None
            };
        }
        let learn = has(
            q,
            &[
                "学会",
                "自学",
                "习得",
                "多少级",
                "几级",
                "learn",
                "level",
                "leveling",
            ],
        );
        let drop = has(
            q,
            &[
                "掉落",
                "掉什么",
                "产出",
                "打倒",
                "击败",
                "野外打",
                "drops",
                "drop ",
                "defeat",
                "hunt ",
                "loot",
            ],
        );
        if drop
            && has(
                q,
                &[
                    "牧场",
                    "牧場",
                    "繁殖",
                    "配种",
                    "肢解",
                    "拆解",
                    "钓鱼",
                    "ranch",
                    "breed",
                    "butcher",
                    "disassembl",
                    "dismantl",
                    "fishing",
                ],
            )
        {
            return None;
        }
        let habitat = !drop
            && has(
                q,
                &[
                    "栖息",
                    "遇到",
                    "出没",
                    "出现",
                    "分布",
                    "野生",
                    "野外",
                    "跑图",
                    "spawn",
                    "in the wild",
                    "habitat",
                    "found on",
                    "living on",
                ],
            );
        let requests = [
            (learn, Self::Learn),
            (drop, Self::Drop),
            (habitat, Self::Habitat),
        ];
        let mut matches = requests
            .into_iter()
            .filter(|(yes, _)| *yes)
            .map(|(_, intent)| intent);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }
}
fn expedition_named(q: &str, row: &Value) -> bool {
    if contains_name(q, text(row, "name")) {
        return true;
    }
    let id = text(row, "id")
        .trim_start_matches("Expedition_")
        .trim_end_matches("_Hard")
        .to_ascii_lowercase();
    let region = text(row, "region");
    let matches = contains_name(q, region)
        || contains_name(q, &id)
        || id == "grass" && q.contains("grassland");
    matches
        && if row["hard"].as_bool() == Some(true) {
            has(q, &["困难", "hard"])
        } else {
            has(q, &["普通", "regular", "ordinary", "normal"])
        }
}
fn amount(row: &Value, english: bool) -> Option<String> {
    let (min, max, chance) = (row[1].as_i64()?, row[2].as_i64()?, row[3].as_f64()?);
    Some(if english {
        format!("{min}–{max} per roll ({chance}%)")
    } else {
        format!("每次 {min}–{max}，几率 {chance}%")
    })
}
fn fact(
    d: &Data,
    q: &str,
    key: &str,
    intent: Intent,
    english: bool,
    subjects: &[&Value],
) -> Option<(String, String, String)> {
    let fields: Vec<_> = key.split(':').collect();
    let subject = *fields.get(1)?;
    if matches!(intent, Intent::Time | Intent::Unlock | Intent::Reward) {
        let row = array(&d.get("expedition-data")["stages"])
            .iter()
            .find(|r| text(r, "id") == subject && expedition_named(q, r))?;
        let title = text(row, "name");
        if intent == Intent::Unlock
            && has(
                q,
                &[
                    "打败",
                    "击败",
                    "打赢",
                    "消灭",
                    "defeating",
                    "beating",
                    "after killing",
                ],
            )
            && !has(q, &["哪个", "哪座", "什么", "which", "what"])
            && !contains_name(q, text(row, "tower"))
        {
            return None;
        }
        let result = match intent {
            Intent::Time => {
                let m = row["minutes"].as_i64()?;
                if english {
                    format!("{title} — expedition duration: {m} minutes.")
                } else {
                    format!("{title}的远征时长为 {m} 分钟。")
                }
            }
            Intent::Unlock => format!(
                "{title} · {}{}",
                if english {
                    "Unlock requirement: "
                } else {
                    "解锁条件为"
                },
                text(row, "unlock")
            ),
            Intent::Reward => {
                let id = *fields.get(2)?;
                let item = item(d, id).filter(|r| named_item(q, r))?;
                let reward = array(&row["rewards"])
                    .iter()
                    .find(|v| v[0].as_str() == Some(id))?;
                format!(
                    "{title} · {} {} · {}",
                    if english {
                        "expedition reward"
                    } else {
                        "远征奖励"
                    },
                    label(item, english),
                    amount(reward, english)?
                )
            }
            _ => return None,
        };
        return Some((
            result,
            title.to_owned(),
            format!("#work/expeditions/{subject}"),
        ));
    }
    let pal = subjects.iter().find(|r| text(r, "key") == subject)?;
    let species = label(pal, english);
    let result = match intent {
        Intent::Learn => {
            let id = *fields.get(2)?;
            let move_record = &d.get("pal-details-data")["moves"][id];
            if !named(q, move_record) {
                return None;
            }
            let row = array(&d.get("pal-details-data")["profiles"][subject]["levels"])
                .iter()
                .find(|r| r[1].as_str() == Some(id))?;
            let lv = row[0].as_i64()?;
            let skill = label(move_record, english);
            if english {
                format!("{species} learns {skill} at level {lv}.")
            } else {
                format!("{species}在 {lv} 级自行习得{skill}。")
            }
        }
        Intent::Drop => {
            let band = fields.get(2)?.parse::<i64>().ok()?;
            let id = *fields.get(3)?;
            let item = item(d, id).filter(|r| named_item(q, r))?;
            let name = label(item, english);
            let row = array(&d.get("paldex-data")[subject]["drops"])
                .iter()
                .find(|r| r[0].as_i64() == Some(band))?;
            let drop = array(&row[1]).iter().find(|r| r[0].as_str() == Some(id))?;
            let tier = if band == 0 {
                if english {
                    "base drop table".into()
                } else {
                    "基础掉落表".into()
                }
            } else if english {
                format!("level {band}+ drop table")
            } else {
                format!("Lv.{band} 起的掉落表")
            };
            format!("{species} · {tier} · {name} · {}", amount(drop, english)?)
        }
        Intent::Habitat => {
            let map = *fields.get(2)?;
            let (zh, en, aliases) = match map {
                "worldtree" => ("世界树", "World Tree", &["世界树", "world tree"][..]),
                "palpagos" => (
                    "帕洛斯群岛",
                    "Palpagos Islands",
                    &["帕洛斯", "帕帕戈斯", "主群岛", "palpagos"][..],
                ),
                _ => return None,
            };
            if !has(q, aliases) {
                return None;
            }
            let spawn = d.get("spawn-data");
            let map_id = array(&spawn["mapIds"])
                .iter()
                .position(|v| v.as_str() == Some(map))? as u64;
            if !array(&spawn["bySpecies"][subject])
                .iter()
                .filter_map(|r| r[0].as_u64())
                .any(|idx| spawn["sites"][idx as usize][0].as_u64() == Some(map_id))
            {
                return None;
            }
            if english {
                format!("{species} has recorded overworld spawn sites on the {en} map.")
            } else {
                format!("{zh}地图记录了{species}的地表出没点。")
            }
        }
        _ => return None,
    };
    Some((result, species.to_owned(), format!("#paldex/{subject}")))
}
pub(in super::super) fn lookup(d: &Data, question: &str) -> Option<AssistantReply> {
    let q = crate::assistant_engine::field_text(question);
    let intent = Intent::from(&q)?;
    let subjects: Vec<_> = d.species.iter().filter(|p| named(&q, p)).collect();
    if matches!(intent, Intent::Learn | Intent::Drop | Intent::Habitat) && subjects.len() != 1 {
        return None;
    }
    let scope = subjects
        .first()
        .filter(|_| matches!(intent, Intent::Learn | Intent::Drop | Intent::Habitat))
        .map(|p| format!("{}{id}:", intent.key(), id = text(p, "key")));
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for key in super::index(d)?.ranked(&q, |k| {
        k.starts_with(scope.as_deref().unwrap_or(intent.key()))
    }) {
        if let Some((body, label, href)) = fact(d, &q, key, intent, english, &subjects) {
            if seen.insert(body.clone()) {
                lines.push(body);
            }
            if !links.iter().any(|(_, h)| h == &href) {
                links.push((label, href));
            }
        }
    }
    (!lines.is_empty()).then(|| AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_world_drop_is_not_a_ranch_or_skill_modifier() {
        for q in [
            "Will Lamball drop wool at the ranch?",
            "如果加快孵化会提高掉落率吗？",
            "Can I inherit this move by defeating a wild pal?",
        ] {
            assert!(Intent::from(&crate::assistant_engine::field_text(q)).is_none());
        }
    }
    #[test]
    fn resolves_current_table_cells_and_rejects_missing_relations() {
        let data = crate::assistant_discovery::tests::handbook();
        let found = lookup(&data, "At what level does Lamball learn Air Cannon?").unwrap();
        assert!(found.text.contains("level 7"));
        assert_eq!(found.links[0].1, "#paldex/SheepBall");
        assert!(lookup(&data, "At what level does Lamball learn Dragon Cannon?").is_none());
        assert!(lookup(&data, "Does Lamball drop sulfur?").is_none());
    }
}
