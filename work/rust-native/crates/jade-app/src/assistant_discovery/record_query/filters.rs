use super::{AssistantReply, Data, Value, array, has, select, text};
use crate::assistant_breeding::species_mentions;

const STATUS: &[(&str, &str, &[&str])] = &[
    ("浸湿", "Wet", &["浸湿", "淋湿", "潮湿", "wet", "soaked"]),
    (
        "点燃",
        "Burn",
        &["点燃", "灼烧", "燃烧", "着火", "burn", "burning", "ignite"],
    ),
    (
        "缠绕",
        "Vines",
        &["缠绕", "藤蔓束缚", "entangle", "vines", "ivy-cling"],
    ),
    (
        "冻结",
        "Freeze",
        &["冻结", "冰冻", "freeze", "freezing", "frozen"],
    ),
    (
        "感电",
        "Shock",
        &["感电", "触电", "麻痹", "electrify", "shock", "paralysis"],
    ),
    ("泥泞", "Muddy", &["泥泞", "muddy", "muddied"]),
    ("眩晕", "Stun", &["眩晕", "击晕", "stun", "stunned"]),
    (
        "中毒",
        "Poison",
        &["中毒", "毒状态", "poison", "poisoned", "poisoning"],
    ),
    (
        "黑暗",
        "Darkness",
        &["黑暗状态", "致盲", "blind", "blindness", "darkness"],
    ),
];

fn word(q: &str, term: &str) -> bool {
    q.match_indices(term).any(|(a, _)| {
        !term.is_ascii()
            || !q[..a].ends_with(|c: char| c.is_ascii_alphanumeric())
                && !q[a + term.len()..].starts_with(|c: char| c.is_ascii_alphanumeric())
    })
}

fn moves(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "哪些",
            "哪几",
            "列出",
            "列举",
            "筛选",
            "有没有",
            "能否列",
            "which",
            "list ",
            "are there",
            "any ",
        ],
    ) || !has(
        q,
        &[
            "招式",
            "攻击技能",
            "主动技",
            "技能果实",
            "moves",
            "attacks",
            "active skills",
        ],
    ) || has(
        q,
        &[
            "被动",
            "passive",
            "怎么获得",
            "在哪",
            "在哪里",
            "掉落",
            "几率",
            "概率",
            "how to get",
            "where",
            " drop",
            "chance",
            "几秒以内",
            "高于",
            "低于",
            "at least",
            "at most",
            "higher than",
            "less than",
            " or ",
            "或者",
            "或是",
        ],
    ) {
        return None;
    }
    let effects = STATUS
        .iter()
        .filter(|(_, _, terms)| terms.iter().any(|x| word(q, x)))
        .collect::<Vec<_>>();
    if effects.is_empty() {
        return None;
    }
    if has(
        q,
        &[
            "不带",
            "不施加",
            "不造成",
            "exclude",
            "except",
            "without the",
            "not inflict",
            "without poison",
            "without burn",
            "without shock",
        ],
    ) {
        return None;
    }
    let en = q.is_ascii();
    let book = d.get("pal-details-data");
    let moves = book["moves"].as_object()?;
    let types = d.get("combat-guide-data")["elements"].as_object()?;
    let selected = types
        .iter()
        .filter(|(key, v)| {
            v.as_str().is_some_and(|zh| {
                q.contains(&format!("{zh}系")) || q.contains(&format!("{zh}属性"))
            }) || word(q, key)
        })
        .map(|(key, _)| key)
        .collect::<Vec<_>>();
    if selected.len() > 1 {
        return None;
    }
    let needs_fruit = if has(
        q,
        &[
            "无果实",
            "没果实",
            "没有果实",
            "没有对应果实",
            "无技能果实",
            "没有技能果实",
            "不能靠果实",
            "no fruit",
            "no skill fruit",
            "without fruit",
            "without a fruit",
            "without a skill fruit",
            "lack a skill fruit",
        ],
    ) {
        Some(false)
    } else if has(q, &["果实", "fruit"]) {
        Some(true)
    } else {
        None
    };
    let inherit = if has(
        q,
        &[
            "不可遗传",
            "不能遗传",
            "无法遗传",
            "不能继承",
            "不可继承",
            "not inherited",
            "cannot be inherited",
            "can't be inherited",
            "non-inheritable",
        ],
    ) {
        Some(false)
    } else if has(
        q,
        &["遗传", "继承", "inheritable", "inherited", "inheritance"],
    ) {
        Some(true)
    } else {
        None
    };
    let named = species_mentions(&d.species, q).0;
    if named.len() > 1
        || !named.is_empty()
            && !has(
                q,
                &[
                    "升级",
                    "自然",
                    "learn by level",
                    "level up",
                    "naturally",
                    "level-up",
                ],
            )
    {
        return None;
    }
    let pal = named.first().map(|(i, _, _)| &d.species[*i]);
    let levels = pal.map(|p| array(&book["profiles"][text(p, "key")]["levels"]));
    let mut rows = moves
        .iter()
        .filter(|(key, m)| {
            selected.first().is_none_or(|k| m["type"] == k.as_str())
                && levels.is_none_or(|r| r.iter().any(|e| e[1] == key.as_str()))
                && effects
                    .iter()
                    .all(|(effect, _, _)| array(&m["effects"]).iter().any(|e| e[0] == *effect))
                && needs_fruit.is_none_or(|needed| {
                    m["fruit"].as_str().is_some_and(|s| !s.is_empty()) == needed
                })
                && inherit.is_none_or(|needed| m["inherit"].as_bool() == Some(needed))
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|(key, m)| {
        (
            m["cooldown"].as_u64().unwrap_or(u64::MAX),
            std::cmp::Reverse(m["power"].as_u64()),
            key.as_str(),
        )
    });
    let mut conditions = effects
        .iter()
        .map(|(zh, eng, _)| if en { *eng } else { *zh })
        .collect::<Vec<_>>();
    if let Some(key) = selected.first() {
        conditions.push(if en { key } else { types.get(*key)?.as_str()? });
    }
    if let Some(b) = needs_fruit {
        conditions.push(match (en, b) {
            (true, true) => "listed skill fruit",
            (true, false) => "no listed skill fruit",
            (false, true) => "有对应技能果实",
            (false, false) => "未列对应技能果实",
        });
    }
    if let Some(b) = inherit {
        conditions.push(match (en, b) {
            (true, true) => "inheritable",
            (true, false) => "not inheritable",
            (false, true) => "可遗传",
            (false, false) => "不可遗传",
        });
    }
    if let Some(p) = pal {
        conditions.push(text(p, if en { "en" } else { "zh" }));
    }
    let count = rows.len();
    let conditions = conditions.join(" · ");
    let mut lines = vec![if en {
        format!("The active-skill table has {count} matches for {conditions}.")
    } else {
        format!("主动招式表中同时满足「{conditions}」的有 {count} 招。")
    }];
    let limit = if has(q, &["全部", "所有", "完整", "all ", "every ", "complete "]) {
        count
    } else {
        12
    };
    let mut links = vec![(
        if en {
            "Active skills"
        } else {
            "主动招式图鉴"
        }
        .into(),
        "#details/active".into(),
    )];
    for (key, m) in rows.iter().take(limit) {
        let zh = text(m, "zh");
        let eng = text(m, "en");
        let mut facts = if en {
            format!(
                "{eng} ({zh}) — power {}, cooldown {} s",
                m["power"], m["cooldown"]
            )
        } else {
            format!(
                "{zh}（{eng}），威力 {}，冷却 {} 秒",
                m["power"], m["cooldown"]
            )
        };
        if let Some(fruit) = m["fruit"].as_str() {
            facts.push_str(&format!(" · {fruit}"));
        }
        lines.push(facts);
        links.push((
            if en { eng } else { zh }.into(),
            format!("#details/active/move/{key}"),
        ));
    }
    if count > limit {
        lines.push(if en {
            format!("Showing {limit} of {count}, shortest cooldown first.")
        } else {
            format!("先列冷却最短的 {limit} 招，共 {count} 招。")
        });
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

fn ranks_after(mut rest: &str) -> Vec<u64> {
    let mut ranks = Vec::new();
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || "#第【[({".contains(c));
        let length = rest.bytes().take_while(u8::is_ascii_digit).count();
        if length == 0 {
            break;
        }
        let Ok(n) = rest[..length].parse() else {
            break;
        };
        if !ranks.contains(&n) {
            ranks.push(n);
        }
        rest = rest[length..]
            .trim_start_matches(|c: char| c.is_whitespace() || "级号】])}".contains(c));
        let Some(next) = ["以及", "和", "及", "与", "、", "/", "&", ",", "and ", "or "]
            .iter()
            .find_map(|s| rest.strip_prefix(s))
        else {
            break;
        };
        rest = next;
    }
    ranks
}

fn rarity(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "什么品质",
            "什么品级",
            "品质是",
            "稀有度",
            "珍稀度",
            "什么颜色",
            "rarity",
            "rarities",
            "which quality",
        ],
    ) || has(
        q,
        &[
            "概率",
            "掉率",
            "掉落",
            "哪里",
            "怎么获得",
            "需要材料",
            "drop rate",
            "where",
            "drop from",
            "crafting material",
            "提高",
            "升级",
            "upgrade",
            "increase",
        ],
    ) {
        return None;
    }
    let items = array(&d.get("item-catalog-data")["items"]);
    let en = q.is_ascii();
    let mut rows = Vec::new();
    let mut families = Vec::new();
    let details = d.get("pal-details-data");
    if has(q, &["被动", "词条", "passive"]) {
        let traits = details["passives"].as_object()?;
        let row = select(traits.values(), q, &["zh", "en"])?;
        let (id, _) = traits.iter().find(|(_, v)| std::ptr::eq(*v, row))?;
        let name = text(row, if en { "en" } else { "zh" });
        let rank = row["rank"].as_i64()?;
        let line = if en {
            format!(
                "{name} is passive rank {rank}. Its documented effect is {}.",
                text(row, "effect")
            )
        } else {
            format!(
                "「{name}」为 {rank} 级被动词条。效果为{}。",
                text(row, "effect")
            )
        };
        return Some(super::card(
            line,
            name,
            format!("#details/passive/trait/{id}"),
        ));
    }
    if has(q, &["主动技", "招式", "attack", "move"]) && !has(q, &["设计图", "schematic"]) {
        if let Some(found) = super::named_moves::resolve(d,q) {
            let (id,row) = match found {
                super::named_moves::Resolution::One(id,row) => (id,row),
                super::named_moves::Resolution::Shared(answer) => return Some(answer),
            };
            let name = text(row, if en { "en" } else { "zh" });
            let mut result = super::card(
                if en {
                    format!("The move table assigns no rarity tier to {name}.")
                } else {
                    format!("主动招式表没有为「{name}」标注稀有度。")
                },
                name,
                format!("#details/active/move/{id}"),
            );
            if let Some(fruit) = row["fruit"].as_str()
                && let Some(item) = items.iter().find(|i| i["name"] == fruit)
            {
                let label = text(item, if en { "en" } else { "name" });
                let grade = item_rarity(item, en)?;
                result.text.push_str(&if en {
                    format!(" The teaching item {label} is {grade}.")
                } else {
                    format!("对应道具「{label}」是{grade}品质。")
                });
                result
                    .links
                    .push((label.into(), format!("#items/{}", text(item, "id"))));
            }
            return Some(result);
        }
    }
    for row in items {
        for field in ["name", "en"] {
            let label = text(row, field).to_lowercase();
            if !label.contains("设计图") && !label.contains("schematic") {
                continue;
            }
            let stem = label
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .trim_end();
            if stem == label || stem.is_empty() {
                continue;
            }
            let Some(rank) = label[stem.len()..].trim().parse::<u64>().ok() else {
                continue;
            };
            if let Some((a, _)) = q.match_indices(stem).find(|(a, _)| {
                !stem.is_ascii() || !q[..*a].ends_with(|c: char| c.is_ascii_alphanumeric())
            }) {
                let ranks = ranks_after(&q[a + stem.len()..]);
                if !families.iter().any(|(s, _)| *s == stem) {
                    families.push((stem.to_string(), ranks.clone()));
                }
                if (ranks.is_empty() || ranks.contains(&rank))
                    && !rows.iter().any(|r: &&Value| r["id"] == row["id"])
                {
                    rows.push(row);
                }
            }
        }
    }
    if families.len() > 1 {
        // A longer family name can contain a shorter, different schematic family.
        return None;
    }
    if families.is_empty() {
        let chosen = select(items.iter(), q, &["name", "en"])?;
        rows.extend(items.iter().filter(|r| r["name"] == chosen["name"]));
    }
    let mut lines = Vec::new();
    let mut links = Vec::new();
    rows.sort_by_key(|r| r["rarity"].as_u64());
    for row in &rows {
        let word = item_rarity(row, en)?;
        let name = text(row, if en { "en" } else { "name" });
        lines.push(format!("{name} — {word}"));
        links.push((name.into(), format!("#items/{}", text(row, "id"))));
    }
    if let Some((family, requested)) = families.first() {
        for n in requested {
            if !rows.iter().any(|r| {
                ["en", "name"].iter().any(|key| {
                    text(r, key)
                        .trim_end_matches(|c: char| c.is_ascii_digit())
                        .trim_end()
                        .eq_ignore_ascii_case(family)
                        && text(r, key).ends_with(&n.to_string())
                })
            }) {
                lines.push(if en {
                    format!("No {family} {n} is listed in this handbook.")
                } else {
                    format!("本版物品表未收录「{family}{n}」。")
                });
            }
        }
    }
    if links.is_empty() && !lines.is_empty() {
        links.push((
            if en { "Item catalog" } else { "物品目录" }.into(),
            "#items".into(),
        ));
    }
    (!lines.is_empty()).then(|| AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

fn item_rarity(row: &Value, en: bool) -> Option<&'static str> {
    let n = row["rarity"].as_u64()? as usize;
    Some(if en {
        ["Common", "Uncommon", "Rare", "Epic", "Legendary"]
            .get(n)
            .copied()
            .unwrap_or("Special")
    } else {
        ["普通", "优秀", "稀有", "史诗", "传说"]
            .get(n)
            .copied()
            .unwrap_or("特殊")
    })
}

fn named_stats(d: &Data, q: &str) -> Option<AssistantReply> {
    let all = has(
        q,
        &[
            "基础面板",
            "基础三围",
            "基础数值",
            "种族值",
            "base stats",
            "base statistics",
        ],
    );
    let fields = [
        (
            "hp",
            "生命",
            "HP",
            &[
                "基础生命",
                "基础体力",
                "基础hp",
                "生命种族值",
                "hp种族值",
                "base hp",
                "base health",
            ][..],
        ),
        (
            "attack",
            "攻击",
            "Attack",
            &["基础攻击", "攻击种族值", "base attack"][..],
        ),
        (
            "defense",
            "防御",
            "Defense",
            &["基础防御", "防御种族值", "base defense", "base defence"][..],
        ),
    ];
    let asked = fields
        .iter()
        .filter(|(_, _, _, terms)| has(q, terms))
        .collect::<Vec<_>>();
    if (!all && asked.is_empty())
        || has(
            q,
            &[
                "最高",
                "最低",
                "更高",
                "最强",
                "排名",
                "比较",
                "对比",
                "谁高",
                "伤害",
                "词条",
                "伙伴技能",
                "哪个",
                "why",
                "compare",
                "higher",
                "highest",
                "best",
                "passive",
                "partner skill",
                "damage",
                "after ",
                "each level",
            ],
        )
    {
        return None;
    }
    let en = q.is_ascii();
    // A full English form before 的 or in parentheses names one record. A
    // partial match on the base species cannot stand in for an unknown form.
    for row in &d.species {
        let base = text(row, "en").to_ascii_lowercase();
        let Some((a, _)) = q.match_indices(&base).find(|(a, _)| {
            !base.is_empty() && !q[..*a].ends_with(|c: char| c.is_ascii_alphabetic())
        }) else {
            continue;
        };
        let tail = &q[a + base.len()..];
        let trimmed =
            tail.trim_start_matches(|c: char| c.is_whitespace() || "([{（【-_".contains(c));
        if trimmed.len() == tail.len() {
            continue;
        }
        let n = trimmed.bytes().take_while(u8::is_ascii_alphabetic).count();
        if n == 0 {
            continue;
        }
        let suffix = &trimmed[..n];
        let after =
            trimmed[n..].trim_start_matches(|c: char| c.is_whitespace() || ")]}）】".contains(c));
        let closed = trimmed[n..].starts_with([')', ']', '}', '）', '】']);
        let official_form = [
            "ignis", "noct", "lux", "cryst", "botan", "aqua", "terra", "primo",
        ]
        .contains(&suffix);
        if !(closed || official_form || after.starts_with('的'))
            || ["pal", "male", "female", "alpha", "lucky"].contains(&suffix)
        {
            continue;
        }
        let full = format!("{base} {suffix}");
        if d.species
            .iter()
            .any(|p| text(p, "en").eq_ignore_ascii_case(&full))
        {
            continue;
        }
        let mut alternatives = vec![row];
        alternatives.extend(d.species.iter().filter(|p| {
            text(p, "en")
                .to_ascii_lowercase()
                .starts_with(&format!("{base} "))
        }));
        let display = alternatives
            .iter()
            .map(|p| text(p, "en"))
            .collect::<Vec<_>>()
            .join(" / ");
        let used = format!("{} {suffix}", text(row, "en"));
        return Some(AssistantReply {
            text: if en {
                format!(
                    "The Paldeck has no form named {used}. Which of these did you mean: {display}?"
                )
            } else {
                format!("图鉴没有名为「{used}」的形态。你指的是 {display} 中的哪一只？")
            },
            links: alternatives
                .iter()
                .map(|p| {
                    (
                        text(p, "en").to_owned(),
                        format!("#paldex/{}", text(p, "key")),
                    )
                })
                .collect(),
            suggestions: vec![],
        });
    }
    let mut names = species_mentions(&d.species, q)
        .0
        .into_iter()
        .filter(|(_, a, _)| !crate::assistant_breeding::excluded_record_subject(&q[..*a]))
        .collect::<Vec<_>>();
    if let Some(pos) = ["只查", "只看", "只问", "only look at", "only asking about"]
        .iter()
        .filter_map(|term| q.rfind(term))
        .max()
        && names.iter().any(|(_, a, _)| *a >= pos)
    {
        names.retain(|(_, a, _)| *a >= pos);
    }
    let [first, ..] = names.as_slice() else {
        return None;
    };
    let idx = &first.0;
    if names.iter().any(|(i, _, _)| i != idx) {
        return None;
    }
    let pal = &d.species[*idx];
    let key = text(pal, "key");
    let profile = &d.get("paldex-index-data")["profiles"][key];
    let mut values = Vec::new();
    for (id, zh, eng, terms) in fields {
        if (all && asked.is_empty()) || has(q, terms) {
            values.push(format!(
                "{} {}",
                if en { eng } else { zh },
                profile[id].as_u64()?
            ));
        }
    }
    let label = format!("{} ({})", text(pal, "zh"), text(pal, "en"));
    Some(super::card(
        format!("{label} — {}", values.join(" / ")),
        &label,
        format!("#paldex/{key}"),
    ))
}

pub(in crate::assistant_discovery) fn lookup(d: &Data, q: &str) -> Option<AssistantReply> {
    if !super::super::handbook_version_matches(q) {
        return None;
    }
    moves(d, q)
        .or_else(|| rarity(d, q))
        .or_else(|| named_stats(d, q))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};
    #[test]
    fn related_records_respect_each_named_field_form_and_qualifier() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "item-catalog-data",
            "pal-details-data",
            "paldex-index-data",
            "combat-guide-data",
            "partner-skills-data",
        ] {
            data.tables.insert(
                id.into(),
                Arc::new(
                    serde_json::from_slice(
                        &std::fs::read(root.join(format!("{id}.json"))).unwrap(),
                    )
                    .unwrap(),
                ),
            );
        }
        let mut chat = Conversation::default();
        for (q, keywords, required, forbidden) in [
            (
                "{Fuack} 跟冲浪鸭是同一只；只查 Fuack-Ignis 的基础防御，普通形态不算。",
                &["防御 60"][..],
                &["#paldex/BluePlatypus_Fire"][..],
                &["#paldex/BluePlatypus"][..],
            ),
            (
                "Frostallion (Noct) 的基础防御是多少？",
                &["防御 135"][..],
                &["#paldex/IceHorse_Dark"][..],
                &["#paldex/IceHorse"][..],
            ),
            (
                "Frostallion Verde 的基础攻击呢？",
                &["图鉴没有", "Frostallion Noct"][..],
                &[][..],
                &[][..],
            ),
            (
                "看 {Fuack} 的伙伴技能；不是 Fuack Ignis。",
                &["乘风破浪"][..],
                &["#details/partner/BluePlatypus"][..],
                &["#details/partner/BluePlatypus_Fire"][..],
            ),
            (
                "Fuack (Ignis) 的伙伴技能是哪一招？",
                &["热浪滚滚"][..],
                &["#details/partner/BluePlatypus_Fire"][..],
                &["#details/partner/BluePlatypus"][..],
            ),
            (
                "列出冰属性、能让目标冻结并有技能果实的主动招式。",
                &["10 招", "冰刃"][..],
                &["#details/active/move/IceBlade"][..],
                &["#details/active/move/Unique_ScorpionMan_Uppercut"][..],
            ),
            (
                "Which water attacks apply wet and are inheritable with no skill fruit?",
                &["0 matches", "inheritable"][..],
                &["#details/active"][..],
                &[][..],
            ),
            (
                "卓越弓设计图1、3和4各是什么稀有度？",
                &["设计图1 — 优秀", "设计图3 — 史诗", "设计图4 — 传说"][..],
                &["#items/Blueprint_SFBow_5"][..],
                &["#items/Blueprint_SFBow_3"][..],
            ),
            (
                "What rarity is Advanced Bow Schematic 7?",
                &["No advanced bow schematic 7"][..],
                &["#items"][..],
                &["#items/Blueprint_SFBow_5"][..],
            ),
            (
                "What rarity is the Artisan passive?",
                &["passive rank 3"][..],
                &["#details/passive/trait/CraftSpeed_up2"][..],
                &[][..],
            ),
            (
                "冰刃这个主动招式的稀有度与技能果实是一样的吗？",
                &["没有为「冰刃」标注稀有度", "冰之技能果实"][..],
                &["#details/active/move/IceBlade"][..],
                &[][..],
            ),
        ] {
            let result = chat.ask_world(q, &data, &Value::Null);
            assert!(
                chat.direct_result()
                    && keywords.iter().all(|s| result.text.contains(s))
                    && required
                        .iter()
                        .all(|s| result.links.iter().any(|(_, v)| v == s))
                    && forbidden
                        .iter()
                        .all(|s| result.links.iter().all(|(_, v)| v != s)),
                "{q}: {result:?}"
            );
        }
    }
}
