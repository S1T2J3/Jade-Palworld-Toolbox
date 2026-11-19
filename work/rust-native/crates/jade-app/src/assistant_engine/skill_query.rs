//! Direct answers backed by the passive, surgery and partner-skill records.
use super::{AssistantReply, field_text};
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, array, text};
use serde_json::Value;
mod totals;
mod learning;
mod innate;
pub(super) use innate::record as innate_passive;
pub(super) use learning::profile_question;

#[derive(Default, Clone, Debug)]
pub(super) struct SkillSession {
    combined_traits: Vec<String>,
    pending_trait: bool,
    implant: bool,
    owner: Option<String>,
    partner: bool,
}

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|s| q.contains(s))
}

fn named(q: &str, raw: &str) -> bool {
    name_span(q, raw).is_some()
}

fn name_span(q: &str, raw: &str) -> Option<(usize, usize)> {
    let name = field_text(raw);
    if name.is_empty() {
        return None;
    }
    q.match_indices(&name)
        .find(|&(i, _)| {
            !name.is_ascii()
                || (!q[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
                    && !q[i + name.len()..].starts_with(|c: char| c.is_ascii_alphanumeric()))
        })
        .map(|(i, _)| (i, i + name.len()))
}

pub(super) fn mentioned_traits<'a>(data: &'a Data, q: &str) -> Vec<&'a str> {
    let mut mentions: Vec<_> = data.get("pal-details-data")["passives"]
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(id, row)| {
            [text(row, "zh"), text(row, "en")]
                .into_iter()
                .filter_map(move |name| {
                    name_span(q, name).map(|(start, end)| (start, end, id.as_str()))
                })
        })
        .collect();
    mentions.sort_by_key(|(start, end, _)| (std::cmp::Reverse(end - start), *start));
    let mut selected: Vec<(usize, usize, &str)> = vec![];
    for (start, end, id) in mentions {
        if !selected
            .iter()
            .any(|(a, b, prior)| id == *prior || start < *b && *a < end)
        {
            selected.push((start, end, id));
        }
    }
    selected.sort_by_key(|m| m.0);
    selected.into_iter().map(|(_, _, id)| id).collect()
}

fn reply(lines: Vec<String>, links: Vec<(String, String)>) -> AssistantReply {
    AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    }
}

fn partner_link(id: &str, owner: &str, skill: &str) -> (String, String) {
    (
        format!("{owner} · {skill}"),
        format!("#details/partner/{id}"),
    )
}

fn partner_rows(data: &Data, ids: &[&str], q: &str) -> Option<AssistantReply> {
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for &id in ids.iter().take(5) {
        let row = &data.get("partner-skills-data")[id];
        let effect = text(row, "effect");
        if effect.is_empty() {
            continue;
        }
        let name = text(row, "name");
        let owner = data.pal_name(id);
        lines.push(format!("{owner}的伙伴技能「{name}」：{effect}"));
        if has(q, &["叠加", "叠层", "stack"]) && !has(effect, &["叠加", "叠层", "每只增加"])
        {
            lines.push("这条技能记录未注明多只同伴的效果如何叠加。".into());
        }
        if has(
            q,
            &[
                "等级", "几星", "满星", "每级", "level", "rank", "stars", "凝缩", "浓缩",
            ],
        ) {
            if let Some(curves) = row["curves"].as_object() {
                for (caption, values) in curves {
                    if let Some(values) = values.as_array().filter(|v| v.len() == 5) {
                        lines.push(format!(
                            "{caption} 对应技能等级 1–5 的数值为 {}。",
                            values
                                .iter()
                                .map(Value::to_string)
                                .collect::<Vec<_>>()
                                .join("、")
                        ));
                    }
                }
            }
            for stats in array(&row["stats"]) {
                if let Some(values) = stats[1].as_array().filter(|v| v.len() == 5) {
                    lines.push(format!(
                        "{}在技能等级 1–5 为 {}。",
                        stats[0].as_str().unwrap_or("数值"),
                        values
                            .iter()
                            .map(Value::to_string)
                            .collect::<Vec<_>>()
                            .join("、")
                    ));
                }
            }
        }
        links.push(partner_link(id, &owner, name));
    }
    if ids.len() > 5 {
        lines.push(format!("共有 {} 条符合条件，这里列出前 5 条。", ids.len()));
    }
    (!links.is_empty()).then(|| reply(lines, links))
}

// An effect may have several independent triggers. Keep a trailing stacking
// qualifier with the sentence it describes so it cannot qualify another effect.
fn effect_clauses(mut effect: &str) -> Vec<&str> {
    let mut clauses = vec![];
    while !effect.is_empty() {
        let mut end = effect
            .find('。')
            .map_or(effect.len(), |i| i + '。'.len_utf8());
        for annotation in ["（不可叠加）", "（可叠加）", "(不可叠加)", "(可叠加)"]
        {
            if effect[end..].starts_with(annotation) {
                end += annotation.len();
                break;
            }
        }
        if !effect[..end].trim().is_empty() {
            clauses.push(effect[..end].trim());
        }
        effect = effect[end..].trim_start();
    }
    clauses
}

struct PartnerEffect<'a> {
    id: &'a str,
    skill: &'a Value,
    sentence: &'a str,
}

fn effects_for<'a>(data: &'a Data, predicate: impl Fn(&str) -> bool) -> Vec<PartnerEffect<'a>> {
    let mut hits = Vec::new();
    for (id, skill) in data
        .get("partner-skills-data")
        .as_object()
        .into_iter()
        .flatten()
    {
        for sentence in effect_clauses(text(skill, "effect")) {
            if predicate(sentence) {
                hits.push(PartnerEffect {
                    id,
                    skill,
                    sentence,
                });
            }
        }
    }
    hits
}

fn effect_reply(data: &Data, hits: &[PartnerEffect], q: &str) -> Option<AssistantReply> {
    if hits.is_empty() {
        return None;
    }
    let mut lines = vec![];
    let mut links = vec![];
    for hit in hits.iter().take(5) {
        let owner = data.pal_name(hit.id);
        let name = text(hit.skill, "name");
        lines.push(format!("{owner}的伙伴技能「{name}」：{}", hit.sentence));
        if has(q, &["叠加", "stack"]) && !has(hit.sentence, &["叠加", "每只增加"]) {
            lines.push("这项效果的说明未注明多只同伴能否叠加。".into());
        }
        if has(q, &["每级", "各级", "几星", "等级", "level", "rank"]) {
            for (key, values) in hit.skill["curves"].as_object().into_iter().flatten() {
                if hit.sentence.contains(key) && values.as_array().is_some_and(|v| v.len() == 5) {
                    lines.push(format!(
                        "{key} 在技能等级 1–5 对应 {}。",
                        array(values)
                            .iter()
                            .map(Value::to_string)
                            .collect::<Vec<_>>()
                            .join("、")
                    ));
                }
            }
        }
        let link = partner_link(hit.id, &owner, name);
        if !links.contains(&link) {
            links.push(link);
        }
    }
    if hits.len() > 5 {
        lines.push(format!(
            "图鉴中有 {} 项匹配效果，这里列出前 5 项。",
            hits.len()
        ));
    }
    Some(reply(lines, links))
}

// Each requested concept must appear in the same effect, including party or base conditions.
const EFFECTS: &[(&[&str], &[&str])] = &[
    (
        &[
            "负重",
            "载重",
            "carry weight",
            "carrying capacity",
            "weight limit",
        ],
        &["负重"],
    ),
    (
        &["矿石重量", "ore weight", "weight of ore"],
        &["矿石的重量"],
    ),
    (&["铬铁矿", "chromite"], &["铬铁矿"]),
    (&["经验值", "experience", " xp"], &["经验值"]),
    (
        &[
            "额外的蛋",
            "额外获得",
            "多捡",
            "多拿一颗",
            "extra egg",
            "additional egg",
        ],
        &["再捡", "额外", "多获得"],
    ),
    (&["孵化", "hatch"], &["孵化"]),
    (
        &["捕获概率", "捕捉率", "捕获率", "capture rate", "catch rate"],
        &["捕获概率", "捕获成功率"],
    ),
    (
        &["掉落", "drop rate", "drop chance", "extra drops"],
        &["掉落"],
    ),
    (
        &["发电", "electricity", "generating electricity"],
        &["发电"],
    ),
    (&["浇水", "watering"], &["浇水"]),
    (&["生火", "kindling"], &["生火"]),
    (&["手工", "handiwork"], &["手工"]),
    (&["采矿", "mining"], &["采矿"]),
    (&["伐木", "lumbering", "logging"], &["伐木"]),
    (
        &[
            "治愈",
            "治疗",
            "healing",
            "restore health",
            "regenerate health",
        ],
        &["恢复", "回复", "治疗"],
    ),
    (
        &["玩家的攻击", "玩家攻击", "player attack"],
        &["玩家的攻击", "玩家使用"],
    ),
    (&["攻击力", "attack power"], &["攻击"]),
    (&["防御力", "defense", "defence"], &["防御"]),
    (
        &[
            "火属性",
            "fire type",
            "fire-type",
            "fire attack",
            "fire pal",
        ],
        &["火属性"],
    ),
    (
        &[
            "水属性",
            "water type",
            "water-type",
            "water attack",
            "water pal",
        ],
        &["水属性"],
    ),
    (
        &[
            "草属性",
            "grass type",
            "grass-type",
            "grass attack",
            "grass pal",
        ],
        &["草属性"],
    ),
    (
        &[
            "雷属性",
            "电属性",
            "electric type",
            "electric-type",
            "electric attack",
            "electric pal",
        ],
        &["雷属性"],
    ),
    (
        &[
            "龙属性",
            "dragon type",
            "dragon-type",
            "dragon attack",
            "dragon pal",
        ],
        &["龙属性"],
    ),
    (
        &[
            "暗属性",
            "dark type",
            "dark-type",
            "dark attack",
            "dark pal",
        ],
        &["暗属性"],
    ),
    (
        &["冰属性", "ice type", "ice-type", "ice attack", "ice pal"],
        &["冰属性"],
    ),
    (
        &[
            "地属性",
            "ground type",
            "ground-type",
            "ground attack",
            "ground pal",
        ],
        &["地属性"],
    ),
    (
        &[
            "无属性",
            "neutral type",
            "neutral-type",
            "neutral attack",
            "neutral pal",
        ],
        &["无属性"],
    ),
    (&["蛋", "egg"], &["蛋"]),
    (
        &[
            "其他据点",
            "别的据点",
            "跨据点",
            "other bases",
            "other base",
            "across bases",
        ],
        &["其他据点"],
    ),
    (&["队伍", "队里", "party", "in the team"], &["队伍"]),
    (&["家畜牧场", "牧场", "ranch"], &["牧场"]),
];

fn matching_partners<'a>(data: &'a Data, q: &str) -> Vec<PartnerEffect<'a>> {
    if has(
        q,
        &[
            "食谱", "蛋糕", "炒蛋", "蛋卷", "omelet", "recipe", "cook", "cake",
        ],
    ) {
        return vec![];
    }
    let targets: Vec<_> = EFFECTS
        .iter()
        .filter(|(phrases, _)| has(q, phrases))
        .collect();
    if targets.is_empty() {
        return vec![];
    }
    // Party/ranch alone describes a very large part of the dex.
    if targets
        .iter()
        .all(|(_, terms)| *terms == ["队伍"] || *terms == ["牧场"])
    {
        return vec![];
    }
    let asks_player = has(q, &["玩家", "player", "trainer", "my own"])
        && !has(q, &["玩家和帕鲁", "玩家与帕鲁", "player and pal"]);
    let attack = has(q, &["攻击", "attack"]);
    let experience = has(q, &["经验", "experience", " xp"]);
    effects_for(data, |sentence| {
        targets.iter().all(|(_, aliases)| has(sentence, aliases))
            && (!asks_player
                || !attack
                || has(
                    sentence,
                    &[
                        "玩家的攻击",
                        "玩家和帕鲁",
                        "玩家的近战",
                        "玩家使用近战",
                        "玩家使用远程",
                    ],
                ))
            && (!asks_player
                || !experience
                || has(sentence, &["玩家获得的经验", "玩家的经验", "玩家经验"]))
    })
}

pub(crate) fn party_stat_sources(data: &Data, q: &str, element: &str, field: &str) -> Option<AssistantReply> {
    let recipient = format!("{element}属性帕鲁的{field}");
    let hits = effects_for(data, |sentence| sentence.contains(&recipient) && sentence.contains("提升"));
    effect_reply(data, &hits, q)
}

// For a named pal, look up the requested benefit without enforcing the proposed recipient.
fn benefit_fields(q: &str) -> Vec<&'static [&'static str]> {
    [
        (
            &["攻击", "attack"][..],
            &[
                "攻击力",
                "攻击会转变",
                "攻击会被赋予",
                "攻击速度",
                "武器时的伤害",
            ][..],
        ),
        (&["防御", "defense", "defence"], &["防御"]),
        (&["经验", "experience", " xp"], &["经验值"]),
        (
            &[
                "负重",
                "载重",
                "carry weight",
                "carry capacity",
                "carrying capacity",
            ],
            &["负重"],
        ),
        (&["回血", "治疗", "heal", "regenerat"], &["恢复", "回复"]),
    ]
    .into_iter()
    .filter(|(words, _)| has(q, words))
    .map(|(_, fields)| fields)
    .collect()
}

fn named_benefits(
    data: &Data,
    q: &str,
    owners: &[&str],
    fields: &[&[&str]],
) -> Option<AssistantReply> {
    if owners.is_empty() || fields.is_empty() {
        return None;
    }
    let mut answer = reply(vec![], vec![]);
    for &id in owners {
        let skill = &data.get("partner-skills-data")[id];
        let effect = text(skill, "effect");
        if effect.is_empty() {
            continue;
        }
        let hits = effect_clauses(effect)
            .into_iter()
            .filter(|s| fields.iter().any(|f| has(s, f)))
            .map(|sentence| PartnerEffect {
                id,
                skill,
                sentence,
            })
            .collect::<Vec<_>>();
        let result = if let Some(value) = effect_reply(data, &hits, q) {
            value
        } else {
            let owner = data.pal_name(id);
            let name = text(skill, "name");
            reply(
                vec![format!("{owner}的伙伴技能「{name}」未列出这项加成。")],
                vec![partner_link(id, &owner, name)],
            )
        };
        if !answer.text.is_empty() {
            answer.text.push('\n');
        }
        answer.text.push_str(&result.text);
        answer.links.extend(result.links);
    }
    (!answer.links.is_empty()).then_some(answer)
}

#[derive(Clone, Copy, PartialEq)]
enum EggEffect {
    Extra,
    Alpha,
    Hatching,
    Breeding,
}

impl EggEffect {
    fn recorded(self, sentence: &str) -> bool {
        match self {
            Self::Extra => {
                sentence.contains("蛋") && has(sentence, &["再捡", "额外获得", "多获得"])
            }
            Self::Alpha => {
                sentence.contains("蛋")
                    && has(sentence, &["捡", "拾取"])
                    && sentence.contains("头目")
            }
            Self::Hatching => sentence.contains("蛋") && sentence.contains("孵化速度"),
            Self::Breeding => {
                sentence.contains("牧场") && has(sentence, &["产蛋速度", "产出蛋的速度"])
            }
        }
    }
}

/// Egg count and egg speed are different quantities. Resolve the player's
/// requested outcome first and then show the trigger from the corresponding row.
fn egg_question(q: &str) -> Vec<EggEffect> {
    if !has(q, &["蛋", "egg"])
        || has(
            q,
            &[
                "蛋糕", "烹饪", "炒蛋", "食谱", "蛋卷", "配方", "omelet", "cake", "recipe", "cook",
            ],
        )
    {
        return vec![];
    }
    let extra = has(
        q,
        &[
            "额外",
            "多一",
            "多拿",
            "多捡",
            "多产",
            "多出",
            "多孵",
            "双份",
            "翻倍",
            "第二颗",
            "extra",
            "second egg",
            "additional",
            "duplicate",
            "another egg",
            "double egg",
        ],
    );
    let alpha = has(q, &["头目", "alpha"]);
    let companion = has(
        q,
        &[
            "伙伴",
            "夥伴",
            "队伍",
            "队里",
            "据点",
            "基地",
            "哪只",
            "哪一只",
            "哪个帕鲁",
            "什么帕鲁",
            "谁",
            "技能",
            "partner",
            "party",
            "base",
            "which pal",
            "what pal",
            "which companion",
        ],
    );
    if !companion {
        return vec![];
    }
    let mut requested = vec![];
    if extra {
        requested.push(EggEffect::Extra);
    }
    if alpha {
        requested.push(EggEffect::Alpha);
    }
    let speed = has(
        q,
        &[
            "速度",
            "加快",
            "速率",
            "更快",
            "加速",
            "更短",
            "时间减少",
            "speed",
            "faster",
            "quicker",
            "shorten",
            "accelerat",
        ],
    );
    if speed && has(q, &["孵", "hatch", "incubat"]) {
        requested.push(EggEffect::Hatching);
    }
    if speed
        && has(
            q,
            &[
                "配种",
                "繁殖",
                "产蛋",
                "下蛋",
                "生蛋",
                "breed",
                "laying",
                "lay eggs",
                "egg production",
            ],
        )
    {
        requested.push(EggEffect::Breeding);
    }
    requested
}

fn egg_answer(
    data: &Data,
    q: &str,
    wanted: &[EggEffect],
    owners: &[&str],
) -> Option<AssistantReply> {
    let all = effects_for(data, |s| wanted.iter().any(|e| e.recorded(s)));
    let mut hits: Vec<_> = all
        .into_iter()
        .filter(|m| owners.is_empty() || owners.contains(&m.id))
        .collect();
    let mut preface = Vec::new();
    let mut links = Vec::new();
    if !owners.is_empty() {
        let table = data.get("partner-skills-data");
        for id in owners
            .iter()
            .copied()
            .filter(|&id| !hits.iter().any(|m| m.id == id))
            .take(4)
        {
            if text(&table[id], "effect").is_empty() {
                return None;
            }
            let owner = data.pal_name(id);
            let skill = text(&table[id], "name");
            preface.push(format!(
                "{owner}的伙伴技能「{skill}」未列出所说的这项蛋效果。"
            ));
            links.push(partner_link(id, &owner, skill));
        }
        if hits.is_empty() {
            hits = effects_for(data, |s| wanted.iter().any(|e| e.recorded(s)));
        }
    }
    let mut answer = effect_reply(data, &hits, q)?;
    if hits.iter().any(|m| EggEffect::Extra.recorded(m.sentence))
        && has(
            q,
            &[
                "孵", "配种", "产蛋", "下蛋", "hatch", "breed", "incubat", "lay eggs",
            ],
        )
    {
        preface.push("图鉴把多得到一颗蛋的触发时机记为捡起帕鲁蛋。孵化完成或在配种牧场产蛋时额外获得一颗，当前记录没有列出这种触发条件。".into());
    }
    if hits.iter().any(|m| EggEffect::Alpha.recorded(m.sentence))
        && has(q, &["孵", "hatch", "incubat"])
    {
        preface.push("头目蛋的判定记录在捡起帕鲁蛋时。".into());
    }
    preface.push(answer.text);
    answer.text = preface.join("\n");
    for link in links {
        if !answer.links.contains(&link) {
            answer.links.push(link);
        }
    }
    Some(answer)
}

struct Goal {
    label: &'static str,
    queries: &'static [&'static str],
    fields: &'static [&'static str],
}
const GOALS: &[Goal] = &[
    Goal {
        label: "工作速度",
        queries: &[
            "工作",
            "工作速度",
            "工作效率",
            "据点生产",
            "干活",
            "打工",
            "work speed",
            "worker",
            "for work",
            "base work",
            "crafting speed",
            "working pals",
        ],
        fields: &["工作速度"],
    },
    Goal {
        label: "攻击",
        queries: &[
            "攻击", "伤害", "输出", "战斗", "打架", "combat", "damage", "attack", "dps", "fighting",
        ],
        fields: &["攻击力", "攻击"],
    },
    Goal {
        label: "防御",
        queries: &[
            "防御",
            "生存",
            "坦克",
            "防守",
            "defense",
            "defence",
            "defensive",
            "tank",
        ],
        fields: &["防御力", "防御"],
    },
    Goal {
        label: "移动速度",
        queries: &[
            "移动速度",
            "移速",
            "跑速",
            "跑得快",
            "骑乘移动",
            "movement speed",
            "move speed",
            "running speed",
            "fast mount",
        ],
        fields: &["移动速度"],
    },
    Goal {
        label: "耐力",
        queries: &["耐力", "体力", "stamina"],
        fields: &["最大耐力"],
    },
    Goal {
        label: "技能冷却",
        queries: &["冷却", "cooldown"],
        fields: &["主动技能的冷却时间缩短"],
    },
    Goal {
        label: "水上速度",
        queries: &["游泳", "泳速", "swim"],
        fields: &["水上移动速度"],
    },
];

fn positive(row: &Value, fields: &[&str], player: bool) -> Option<f64> {
    text(row, "effect").lines().find_map(|s| {
        let s = if player {
            s.trim().strip_prefix("玩家")?.trim_start_matches('的')
        } else {
            s.trim()
        };
        let rest = fields
            .iter()
            .find_map(|f| s.trim().strip_prefix(f))?
            .trim_start();
        let amount = rest
            .trim_start_matches("提升")
            .trim_start_matches("提高")
            .trim_start_matches("增加")
            .trim();
        let value = amount.split_once('%')?.0.trim().parse::<f64>().ok()?;
        (value > 0.).then_some(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    pub(super) fn handbook() -> Data {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(path.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["partner-skills-data", "trait-data", "pal-details-data"] {
            let bytes = std::fs::read(path.join(format!("{name}.json"))).unwrap();
            data.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        data
    }

    #[test]
    fn reverse_lookup_requires_the_requested_effect_and_context() {
        let data = handbook();
        let mut session = SkillSession::default();
        let answer = session
            .respond(
                &field_text("Which Pal's partner skill boosts watering at other bases?"),
                &data,
            )
            .unwrap();
        assert!(
            answer
                .links
                .iter()
                .any(|(_, p)| p == "#details/partner/ClioneTwins")
        );
        assert!(answer.links.iter().all(|(_, p)| {
            data.get("partner-skills-data")[p.rsplit('/').next().unwrap()]["effect"]
                .as_str()
                .unwrap()
                .contains("其他据点")
        }));
        let answer = session
            .respond(&field_text("哪隻擁有「茸茸盾牌」？"), &data)
            .unwrap();
        assert_eq!(answer.links[0].1, "#details/partner/SheepBall");
        assert!(
            session
                .respond(&field_text("Which Pal drops Pal Metal Ingots?"), &data)
                .is_none()
        );
    }

    #[test]
    fn clarify_owner_then_use_the_record_for_the_named_form() {
        let data = handbook();
        let mut session = SkillSession::default();
        assert!(
            session
                .respond("想查伙伴技能", &data)
                .unwrap()
                .text
                .contains("哪只")
        );
        let answer = session.respond(&field_text("Fuack Ignis"), &data).unwrap();
        assert_eq!(answer.links.len(), 1);
        assert_eq!(answer.links[0].1, "#details/partner/BluePlatypus_Fire");
        assert!(
            session
                .respond("does it stack", &data)
                .unwrap()
                .text
                .contains("未注明")
        );
        assert!(session.respond("多少个金属锭", &data).is_none());
    }

    #[test]
    fn surgery_filters_and_keeps_the_actual_effects() {
        let data = handbook();
        let mut session = SkillSession::default();
        let first = session
            .respond(
                &field_text("What are the best passives to implant on Anubis?"),
                &data,
            )
            .unwrap();
        assert!(first.text.contains("阿努比斯") && first.text.contains("用途"));
        let by_goal = session.respond("工作速度", &data).unwrap();
        assert!(by_goal.text.contains("正向加成") && by_goal.links.len() >= 2);
        for (_, link) in &by_goal.links {
            let id = link.rsplit('/').next().unwrap();
            assert!(data.get("pal-details-data")["passives"][id]["surgery"].is_object());
        }
        let lucky = session
            .respond(&field_text("Can I implant Lucky?"), &data)
            .unwrap();
        assert!(
            lucky.text.contains("不可植入") && lucky.links[0].1 == "#details/passive/trait/Rare"
        );
        let distinct = session
            .respond(&field_text("棉悠悠的夥伴技能能在手術台植入嗎？"), &data)
            .unwrap();
        assert!(distinct.text.contains("茸茸盾牌") && distinct.text.contains("工匠精神"));
        let player = session
            .respond("which passives increase player attack?", &data)
            .unwrap();
        assert!(
            player
                .links
                .iter()
                .any(|(_, key)| key == "#details/passive/trait/TrainerATK_UP_1")
        );
        assert!(
            player
                .links
                .iter()
                .all(|(_, key)| !key.ends_with("PAL_ALLAttack_up3"))
        );
        let night = session
            .respond("哪些被动可以让帕鲁夜间工作", &data)
            .unwrap();
        assert!(night.text.contains("不眠") && !night.text.contains("工匠精神"));
    }

    #[test]
    fn desktop_conversation_routes_the_followup_and_keeps_the_complete_variant() {
        let mut chat = crate::assistant_engine::Conversation::default();
        let data = handbook();
        let world = serde_json::json!({"id":"skill-test"});
        chat.ask_world(
            "What are the best passives to implant on Anubis?",
            &data,
            &world,
        );
        let answer = chat.ask_world("工作速度", &data, &world);
        assert!(
            chat.direct_result()
                && answer.text.contains("手术台")
                && answer.text.contains("工作速度"),
            "{}",
            answer.text
        );
        let answer = chat.ask_world("Fuack Ignis 的夥伴技能是什麼？", &data, &world);
        assert!(
            chat.direct_result()
                && answer
                    .links
                    .iter()
                    .any(|(_, href)| href == "#details/partner/BluePlatypus_Fire"),
            "{}",
            answer.text
        );
        let answer = chat.ask_world("那它能叠加吗？", &data, &world);
        assert!(
            chat.direct_result()
                && answer.text.contains("热浪鸭")
                && answer.text.contains("未注明"),
            "{}",
            answer.text
        );
    }

    #[test]
    fn compare_named_passives_in_player_order_and_preserve_penalties() {
        let data = handbook();
        let mut chat = crate::assistant_engine::Conversation::default();
        let answer = chat.ask_world(
            "破壞神和雙刃聖劍分別加什麼、各扣什麼？",
            &data,
            &serde_json::json!({}),
        );
        assert!(chat.direct_result(), "{}", answer.text);
        let lines: Vec<_> = answer.text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].contains("破坏神")
                && lines[0].contains("攻击+40%")
                && lines[0].contains("防御+20%")
                && lines[0].contains("最大HP-50%"),
            "{}",
            lines[0]
        );
        assert!(
            lines[1].contains("双刃圣剑")
                && lines[1].contains("攻击+50%")
                && lines[1].contains("防御-30%"),
            "{}",
            lines[1]
        );
        assert_eq!(
            answer.links[0].1,
            "#details/passive/trait/WorldTree_ATK_DEF"
        );
        let q = field_text("Compare Artisan versus Musclehead. What are the trade-offs?");
        let answer = SkillSession::default().respond(&q, &data).unwrap();
        assert_eq!(answer.links[0].1, "#details/passive/trait/CraftSpeed_up2");
        assert_eq!(answer.links[1].1, "#details/passive/trait/Noukin");
        assert!(
            SkillSession::default()
                .respond("分别去哪里获取神秘宝箱和海鲜浓汤？", &data)
                .is_none()
        );
    }

    #[test]
    fn distinguishes_extra_egg_pickup_from_hatching_breeding_and_alpha_eggs() {
        let data = handbook();
        let mut chat = crate::assistant_engine::Conversation::default();
        let world = serde_json::json!({"id":"egg"});
        let extra = chat.ask_world("听说能孵出额外一颗蛋，是在队伍还是据点？", &data, &world);
        assert!(
            chat.direct_result() && extra.links.len() == 1,
            "{}",
            extra.text
        );
        assert_eq!(extra.links[0].1, "#details/partner/NaughtyCat");
        assert!(
            extra.text.contains("捡起")
                && extra.text.contains("孵化完成")
                && extra.text.contains("若它在队伍中"),
            "{}",
            extra.text
        );
        let alpha = chat.ask_world(
            "Which Pal's partner skill turns a picked-up egg into an alpha egg?",
            &data,
            &world,
        );
        assert_eq!(alpha.links.len(), 2);
        assert!(alpha.links.iter().all(|(_, p)| p.contains("SakuraSaurus")));
        let speed = chat.ask_world(
            "Which Pal speeds up egg hatching at my base?",
            &data,
            &world,
        );
        assert_eq!(speed.links.len(), 1);
        assert_eq!(speed.links[0].1, "#details/partner/ThunderFluffyBird");
        let farming = chat.ask_world("哪只伙伴在据点里能让配种牧场产蛋更快？", &data, &world);
        assert_eq!(farming.links.len(), 1);
        assert_eq!(farming.links[0].1, "#details/partner/Plesiosaur");
        let wrong_owner = chat.ask_world(
            "Cattiva in my party can hatch an extra egg, right?",
            &data,
            &world,
        );
        assert!(
            wrong_owner.text.contains("捣蛋猫")
                && wrong_owner.text.contains("未列出")
                && wrong_owner.text.contains("笑魇猫")
        );
        assert!(
            SkillSession::default()
                .respond("which pal likes extra eggs in an omelet recipe?", &data)
                .is_none()
        );
    }

    #[test]
    fn reverse_effects_do_not_mix_actors_or_triggers_across_sentences() {
        let data = handbook();
        let q =
            field_text("Which Pal's partner skill gives electric Pals attack power at a ranch?");
        assert!(matching_partners(&data, &q).is_empty());
        let good =
            field_text("Which Pal's partner skill gives electric Pals attack power in the party?");
        let answer = SkillSession::default().respond(&good, &data).unwrap();
        assert!(
            answer
                .links
                .iter()
                .any(|(_, key)| key == "#details/partner/ElecCat"),
            "{}",
            answer.text
        );
        assert!(!answer.text.contains("家畜牧场") && answer.text.contains("雷属性帕鲁的攻击力"));
        assert!(matching_partners(&data, "哪只伙伴在队伍中提升玩家经验值").is_empty());
        let checked = SkillSession::default()
            .respond(&field_text("伏特喵在牧场是给玩家还是帕鲁提高攻击？"), &data)
            .unwrap();
        assert!(
            checked.text.contains("队伍")
                && checked.text.contains("雷属性帕鲁的攻击力")
                && !checked.text.contains("掉落发电器官"),
            "{}",
            checked.text
        );
    }
}

fn trait_entry(data: &Data, id: &str, surgery: bool) -> Option<(String, (String, String))> {
    let row = &data.get("pal-details-data")["passives"][id];
    let name = text(row, "zh");
    if name.is_empty() {
        return None;
    }
    let mut line = format!(
        "「{name}」({})：{}",
        text(row, "en"),
        text(row, "effect").replace('\n', "，")
    );
    if surgery {
        let s = &row["surgery"];
        let detail = match text(s, "mode") {
            "consume" => format!("每次消耗「{}」", text(s, "name")),
            "unlock" => format!(
                "解锁物为「{}」，手术费用为 {} 金币",
                text(s, "name"),
                s["cost"]
            ),
            "coins" => format!("手术费用为 {} 金币", s["cost"]),
            _ => {
                let listed = array(&data.get("trait-data")["traits"])
                    .iter()
                    .find(|t| text(t, "key") == id);
                if listed.and_then(|r| r["surgery"].as_bool()) == Some(false) {
                    "图鉴标记手术台不可植入".into()
                } else {
                    "图鉴没有列出手术条件".into()
                }
            }
        };
        line.push_str(&format!("（{detail}）"));
    }
    Some((line, (name.into(), format!("#details/passive/trait/{id}"))))
}

fn traits_by_goal(
    data: &Data,
    goal: &[&Goal],
    surgery: bool,
    excluded: &[&str],
    q: &str,
) -> Option<AssistantReply> {
    let all = data.get("pal-details-data")["passives"].as_object()?;
    let player = has(q, &["玩家", "player", "trainer"]);
    let no_penalty = has(
        q,
        &[
            "无负面",
            "无副作用",
            "不降低",
            "不掉san",
            "no downside",
            "no penalty",
            "without downside",
        ],
    );
    let mut candidates: Vec<_> = all
        .iter()
        .filter_map(|(id, row)| {
            if excluded.contains(&id.as_str()) || (surgery && !row["surgery"].is_object()) {
                return None;
            }
            if no_penalty && has(text(row, "effect"), &["-", "下降速度加快", "消耗量增加"])
            {
                return None;
            }
            if !goal
                .iter()
                .all(|g| positive(row, g.fields, player).is_some())
            {
                return None;
            }
            Some((positive(row, goal.first()?.fields, player)?, id.as_str()))
        })
        .collect();
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
    if candidates.is_empty() {
        return None;
    }
    let label = goal.iter().map(|g| g.label).collect::<Vec<_>>().join("和");
    let scope = if surgery {
        "可在手术台植入的词条"
    } else {
        "被动词条"
    };
    let target = if player { "玩家的" } else { "" };
    let mut lines = vec![format!(
        "按{target}{label}正向加成筛选{scope}，优先列出{}加成较高的候选。",
        goal[0].label
    )];
    let mut links = vec![];
    for (_, id) in candidates.iter().take(3) {
        let (line, link) = trait_entry(data, id, surgery)?;
        lines.push(line);
        links.push(link);
    }
    Some(reply(lines, links))
}

impl SkillSession {
    pub(super) fn respond(&mut self, q: &str, data: &Data) -> Option<AssistantReply> {
        let previous = std::mem::take(&mut self.combined_traits);
        if let Some((answer, ids)) = totals::answer(data, q, &previous) {
            *self = Self { combined_traits: ids, ..Self::default() };
            return Some(answer);
        }
        let implant = has(
            q,
            &[
                "植入",
                "移植",
                "手术台",
                "手术",
                "implant",
                "surgery",
                "surgical",
            ],
        );
        let trait_word = implant || has(q, &["词条", "被动", "特性", "passive", "trait"]);
        let partner_word = has(
            q,
            &[
                "伙伴技",
                "夥伴技",
                "搭档技",
                "伙伴能力",
                "夥伴能力",
                "partner skill",
                "partner ability",
            ],
        );
        let who = has(
            q,
            &[
                "哪只",
                "哪一只",
                "哪种帕鲁",
                "哪个帕鲁",
                "哪些帕鲁",
                "哪个伙伴",
                "哪个夥伴",
                "什么帕鲁",
                "哪一个",
                "谁",
                "which pal",
                "which pals",
                "what pal",
                "who has",
                "who can",
                "whose",
            ],
        );
        let recommend = has(
            q,
            &[
                "推荐",
                "最好",
                "最佳",
                "用什么",
                "选什么",
                "选哪些",
                "怎么选",
                "适合",
                "值得",
                "哪些",
                "best",
                "suggest",
                "recommend",
                "which passive",
                "what passive",
                "what are the best",
            ],
        );
        let goal: Vec<_> = GOALS.iter().filter(|g| has(q, g.queries)).collect();
        let simple = q
            .trim_matches(|c: char| c.is_whitespace() || "。.!?！？".contains(c))
            .trim();
        let goal_reply = simple
            .trim_start_matches("主要")
            .trim_start_matches("我想要")
            .trim_start_matches("优先")
            .trim_start_matches("我要")
            .trim_start_matches("选")
            .trim_start_matches("for ");
        let follow = self.pending_trait
            && !goal.is_empty()
            && (GOALS.iter().any(|g| g.queries.contains(&goal_reply))
                || ["工作", "据点生产", "战斗输出", "骑乘移动"].contains(&goal_reply));
        let skill_refs = has(
            q,
            &[
                "它的技能",
                "它能",
                "它会",
                "它的伙伴",
                "does it",
                "its skill",
                "its ability",
                "能叠加",
                "能叠层",
                "does it stack",
                "几级",
                "满级",
            ],
        );
        let active_partner = self.partner && skill_refs;
        let owner_lookup = who
            && (partner_word
                || has(
                    q,
                    &[
                        "能力",
                        "伙伴",
                        "加成",
                        "增加",
                        "提高",
                        "提升",
                        "额外",
                        "领取蛋",
                        "捡蛋",
                        "跨据点",
                        "其他据点",
                        "负重",
                        "extra egg",
                        "carrying capacity",
                        "other bases",
                        "chromite",
                        "carry weight",
                        "boost",
                        "increas",
                    ],
                ));
        let inquiry = has(
            q,
            &[
                "作用",
                "有什么用",
                "效果",
                "能做什么",
                "ability",
                "how does",
                "what does",
            ],
        );
        let stat_question = has(
            q,
            &[
                "各加",
                "各扣",
                "加什么",
                "扣什么",
                "加多少",
                "扣多少",
                "减多少",
                "加减",
                "副作用",
                "负面",
                "加成",
                "增益",
                "代价",
                "属性变化",
                "分别",
                "what stats",
                "which stats",
                "stat bonus",
                "stat change",
                "penalty",
                "penalties",
                "trade-off",
                "tradeoff",
                "downsides",
                "stat difference",
            ],
        );
        let contrast = has(
            q,
            &[
                "比较",
                "对比",
                "相比",
                "区别",
                "差别",
                "差异",
                "compare",
                " versus ",
                " vs ",
                "difference between",
            ],
        );
        let requested_eggs = egg_question(q);
        let scope_check = has(
            q,
            &[
                "队伍",
                "队里",
                "入队",
                "据点",
                "基地",
                "牧场",
                "给玩家",
                "给帕鲁",
                "玩家还是",
                "还是玩家",
                "party",
                "base",
                "ranch",
                "player or",
                "or player",
                "beneficiary",
            ],
        ) && has(
            q,
            &[
                "还是",
                "能不能",
                "会不会",
                "是否",
                "加成",
                "提高",
                "提升",
                "增加",
                "增强",
                " or ",
                "buff",
                "boost",
                "increas",
            ],
        );
        let awaiting_owner = self.partner && self.owner.is_none();
        if !(trait_word
            || partner_word
            || who
            || inquiry
            || stat_question
            || contrast
            || scope_check
            || !requested_eggs.is_empty()
            || follow
            || active_partner
            || awaiting_owner)
        {
            return None;
        }
        if has(
            q,
            &[
                "继承概率",
                "遗传概率",
                "遗传率",
                "怎么配",
                "如何配",
                "parent combination",
                "breeding chance",
                "chance to inherit",
            ],
        ) {
            return None;
        }
        let (spans, exact_owner) = species_mentions(&data.species, q);
        let owners: Vec<_> = spans
            .iter()
            .filter(|(_,start,_)| !crate::assistant_breeding::excluded_record_subject(&q[..*start]))
            .map(|(i, _, _)| text(&data.species[*i], "key"))
            .collect();
        let named_traits = mentioned_traits(data, q);
        if !trait_word && !requested_eggs.is_empty() {
            if let Some(answer) = egg_answer(data, q, &requested_eggs, &owners) {
                self.owner = if answer.links.len() == 1 {
                    answer.links[0]
                        .1
                        .strip_prefix("#details/partner/")
                        .map(str::to_owned)
                } else {
                    None
                };
                self.partner = true;
                self.pending_trait = false;
                return Some(answer);
            }
        }
        if implant && partner_word && !recommend {
            let mut lines =
                vec!["伙伴技能随帕鲁种类确定。手术台可植入的是登记在手术台中的被动词条。".into()];
            let mut links = Vec::new();
            if let Some(r) = partner_rows(data, &owners, q) {
                lines.push(r.text);
                links = r.links;
            }
            if !named_traits.is_empty() {
                for id in &named_traits {
                    let (line, link) = trait_entry(data, id, true)?;
                    lines.push(line);
                    links.push(link);
                }
            } else {
                if let Some((example, link)) = trait_entry(data, "CraftSpeed_up2", true) {
                    lines.push(format!("例如{example}"));
                    links.push(link);
                }
                lines.push(
                    "告诉我想植入的词条名，或希望提升的属性，就能核对可用的植入方式。".into(),
                );
            }
            self.pending_trait = true;
            self.implant = true;
            self.partner = false;
            return Some(reply(lines, links));
        }
        let disregarded = has(
            q,
            &[
                "不能用",
                "排除",
                "除了",
                "不用",
                "除外",
                "无法",
                "can't",
                "cannot",
                "without",
                "except",
                "besides",
                "other than",
                "can't get",
                "dont have",
            ],
        );
        let recommendation = trait_word && recommend;
        let fixed_query = has(
            q,
            &[
                "固定",
                "天生",
                "自带",
                "guaranteed",
                "fixed",
                "innate",
                "natural passive",
            ],
        );
        if who && fixed_query && !named_traits.is_empty() {
            let profiles = data.get("pal-details-data")["profiles"].as_object()?;
            let mut lines = Vec::new();
            let mut links = Vec::new();
            for id in &named_traits {
                let pals: Vec<_> = profiles
                    .iter()
                    .filter(|(_, r)| array(&r["fixed"]).iter().any(|k| k.as_str() == Some(*id)))
                    .map(|(k, _)| k)
                    .collect();
                let (desc, link) = trait_entry(data, id, false)?;
                links.push(link);
                if pals.is_empty() {
                    lines.push(format!("{desc}。已载入的物种固定词条表未列出持有者。"));
                } else {
                    let owners = pals.iter().map(|k| data.pal_name(k)).collect::<Vec<_>>();
                    lines.push(format!(
                        "{desc}。图鉴列有该固定词条的帕鲁为{}。",
                        owners.join("、")
                    ));
                    links.extend(
                        pals.iter()
                            .zip(owners)
                            .take(5)
                            .map(|(id, owner)| (owner, format!("#paldex/{id}"))),
                    );
                }
            }
            return Some(reply(lines, links));
        }
        let asks_source = has(
            q,
            &[
                "哪儿拿",
                "哪里找",
                "哪里掉",
                "怎么获得",
                "怎么获取",
                "在哪里买",
                "去哪里刷",
                "where to find",
                "how to get",
                "where can i get",
                "where can i find",
                "how do i get",
            ],
        );
        if (!named_traits.is_empty()
            && (trait_word || inquiry || stat_question || contrast && named_traits.len() > 1))
            && !(recommendation && disregarded)
            && !asks_source
        {
            let mut lines = vec![];
            let mut links = vec![];
            for id in &named_traits {
                let (mut line, link) = trait_entry(data, id, implant)?;
                if has(
                    q,
                    &[
                        "谁的",
                        "给谁",
                        "给玩家",
                        "玩家还是",
                        "还是玩家",
                        "受益",
                        "benefit",
                        "player or",
                        "or the player",
                    ],
                ) {
                    let effect = text(&data.get("pal-details-data")["passives"][*id], "effect");
                    if has(effect, &["帕鲁和玩家", "玩家和帕鲁"]) {
                        line.push_str("描述中同时列有帕鲁和玩家。");
                    } else if effect.lines().any(|s| s.starts_with("玩家")) {
                        line.push_str("描述把这项加成列在玩家身上。");
                    } else if effect.lines().any(|s| {
                        ["攻击", "防御", "工作速度", "最大HP", "最大耐力"]
                            .iter()
                            .any(|p| s.starts_with(p))
                    }) {
                        line.push_str("这里的属性属于携带词条的帕鲁。");
                    }
                }
                lines.push(line);
                links.push(link);
            }
            self.pending_trait = false;
            self.partner = false;
            return Some(reply(lines, links));
        }
        if trait_word {
            let specific: Vec<&[&str]> = [
                (
                    &[
                        "夜间工作",
                        "晚上工作",
                        "夜晚工作",
                        "通宵",
                        "work at night",
                        "work through the night",
                    ][..],
                    &["夜晚也不会睡觉"][..],
                ),
                (
                    &[
                        "牧场适性",
                        "牧场的工作适",
                        "牧场工作适",
                        "ranching suitability",
                        "ranching level",
                    ],
                    &["牧场的工作适应性"],
                ),
                (
                    &["孵化速度", "孵蛋速度", "hatching speed", "hatch speed"],
                    &["孵化速度"],
                ),
                (
                    &[
                        "产蛋速度",
                        "繁殖速度",
                        "breeding speed",
                        "egg production speed",
                    ],
                    &["产蛋速度", "产出蛋的速度"],
                ),
            ]
            .into_iter()
            .filter(|(words, _)| has(q, words))
            .map(|(_, fields)| fields)
            .collect();
            if !specific.is_empty() {
                let mut lines = Vec::new();
                let mut links = Vec::new();
                for (key, row) in data.get("pal-details-data")["passives"].as_object()? {
                    if !(specific
                        .iter()
                        .all(|wanted| has(text(row, "effect"), wanted))
                        && (!implant || row["surgery"].is_object()))
                    {
                        continue;
                    }
                    let (line, link) = trait_entry(data, key, implant)?;
                    lines.push(line);
                    links.push(link);
                }
                if lines.is_empty() {
                    lines.push(
                        "已载入的词条效果与手术记录里，未找到同时满足这些条件的条目。".into(),
                    );
                }
                return Some(reply(lines, links));
            }
        }
        if recommendation || follow || implant || trait_word && !goal.is_empty() {
            let surgery = implant || follow && self.implant;
            self.pending_trait = true;
            self.implant = surgery;
            self.partner = false;
            if !goal.is_empty() {
                let excluded = if disregarded {
                    named_traits.as_slice()
                } else {
                    &[]
                };
                if let Some(r) = traits_by_goal(data, &goal, surgery, excluded, q) {
                    return Some(r);
                }
                return Some(reply(
                    vec![
                        "现有词条表没有能同时核实这些条件的候选。你最希望优先提升哪项属性？".into(),
                    ],
                    vec![],
                ));
            }
            let subject = owners
                .first()
                .map(|id| data.pal_name(id))
                .unwrap_or_else(|| "这只帕鲁".into());
            let mut lines = vec![if surgery {
                format!("为{subject}挑选植入词条时，可以先按用途看手术台中对应的词条。")
            } else {
                format!("{subject}的词条选择取决于用途。")
            }];
            let mut links = vec![];
            for (purpose, id) in [
                ("工作速度", "CraftSpeed_up2"),
                ("战斗攻击", "PAL_ALLAttack_up2"),
                ("坐骑移速", "MoveSpeed_up_2"),
            ] {
                if let Some((desc, link)) = trait_entry(data, id, surgery) {
                    lines.push(format!("{purpose}可以看{desc}"));
                    links.push(link);
                }
            }
            lines.push("你准备让它负责据点生产、战斗输出，还是骑乘移动？".into());
            return Some(reply(lines, links));
        }
        if trait_word && !owners.is_empty() && fixed_query {
            let mut lines = vec![];
            let mut links = vec![];
            for id in owners.iter().take(4) {
                let profile = &data.get("pal-details-data")["profiles"][*id];
                let fixed = profile["fixed"].as_array()?;
                let owner = data.pal_name(id);
                if fixed.is_empty() {
                    lines.push(format!("{owner}在图鉴中未列固定被动词条。"));
                } else {
                    lines.push(format!("{owner}的固定被动词条："));
                    for key in fixed.iter().filter_map(Value::as_str) {
                        if let Some((line, link)) = trait_entry(data, key, false) {
                            lines.push(line);
                            links.push(link);
                        }
                    }
                }
                links.push((owner, format!("#paldex/{id}")));
            }
            return Some(reply(lines, links));
        }
        if trait_word {
            return None;
        }
        let skills = data.get("partner-skills-data").as_object()?;
        let benefits = benefit_fields(q);
        if (scope_check || partner_word) && !benefits.is_empty() && !owners.is_empty() {
            if let Some(answer) = named_benefits(data, q, &owners, &benefits) {
                self.owner = (owners.len() == 1).then(|| owners[0].to_owned());
                self.partner = true;
                self.pending_trait = false;
                return Some(answer);
            }
        }
        let exact: Vec<_> = skills
            .iter()
            .filter(|(_, row)| {
                let skill = text(row, "name");
                named(q, skill)
                    && (skill.chars().count() > 2
                        || partner_word
                        || q.contains(&format!("「{skill}」"))
                        || q.contains(&format!("叫{skill}")))
            })
            .map(|(id, _)| id.as_str())
            .collect();
        if !exact.is_empty() && (partner_word || who) {
            self.owner = (exact.len() == 1).then(|| exact[0].to_owned());
            self.partner = true;
            return partner_rows(data, &exact, q);
        }
        if who && (partner_word || owner_lookup) {
            let matches = matching_partners(data, q);
            if !matches.is_empty() {
                self.owner = (matches.len() == 1).then(|| matches[0].id.to_owned());
                self.partner = true;
                self.pending_trait = false;
                return effect_reply(data, &matches, q);
            }
            if partner_word && scope_check && owners.is_empty() && !benefits.is_empty() {
                return Some(reply(vec!["图鉴没有找到在同一项伙伴技能效果中同时符合所述受益对象、属性与触发条件的记录。".into()],vec![]));
            }
        }
        if partner_word
            || active_partner
            || awaiting_owner && exact_owner
            || has(q, &["ability", "技能效果"])
        {
            let mut selected = owners;
            if selected.is_empty() && active_partner {
                if let Some(id) = self.owner.as_deref() {
                    selected.push(id);
                }
            }
            if let Some(result) = partner_rows(data, &selected, q) {
                let id = (selected.len() == 1).then(|| selected[0].to_owned());
                self.owner = id;
                self.partner = true;
                self.pending_trait = false;
                return Some(result);
            }
            if partner_word {
                self.owner = None;
                self.partner = true;
                self.pending_trait = false;
                return Some(reply(vec!["你想查哪只帕鲁的伙伴技能，或想找能提供什么具体效果的伙伴？给出帕鲁、技能名或效果就能核对。".into()],vec![]));
            }
        }
        None
    }
}
