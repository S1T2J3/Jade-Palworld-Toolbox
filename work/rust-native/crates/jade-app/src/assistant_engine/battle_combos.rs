//! Match an explicit status producer to the condition in a named partner skill.
use super::AssistantReply;
use crate::assistant_breeding::species_mentions;
use crate::assistant_discovery::relevant_names;
use crate::data::{Data, array, text};

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|word| q.contains(word))
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "搭配",
            "配合",
            "联动",
            "组合",
            "触发",
            "好用吗",
            "一起用",
            "效果如何",
            "worth using",
            "good now",
            "any good",
            "go well",
            "good with",
            "pair",
            "synerg",
            "combo",
            "work with",
            "trigger",
        ],
    ) {
        return None;
    }
    if has(
        q,
        &[
            "哪里学",
            "几级学",
            "怎么做",
            "哪里抓",
            "设计图",
            "需要多少",
            "how to craft",
            "where to get",
            "what level",
            "which is faster",
            "schematic",
            "breeding",
            "immune",
            "resistant",
            "how often",
            "proc chance",
            "trigger chance",
            "what chance",
            "guaranteed",
            "every hit",
            "免疫",
            "抗性",
            "概率",
            "必定",
            "几率",
            "每次命中",
        ],
    ) {
        return None;
    }
    let (mentions, _) = species_mentions(&data.species, q);
    let [(number, _, _)] = mentions.as_slice() else {
        return None;
    };
    let pal = &data.species[*number];
    let pal_id = text(pal, "key");
    let partner = &data.get("partner-skills-data")[pal_id];
    let effect = partner["effect"].as_str()?;
    let status = array(&data.get("combat-guide-data")["status"])
        .iter()
        .find(|status| {
            let name = text(status, "name");
            !name.is_empty() && effect.contains(&format!("{name}状态的敌人"))
        })?;
    let state = text(status, "name");
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let locale = if en { "en" } else { "zh" };
    let mut sources = Vec::new();
    for row in relevant_names(array(data.get("search-index-data")), q) {
        let route = text(row, "href");
        let id = route.rsplit('/').next()?;
        let (title, marker, is_move) = match text(row, "kind") {
            "主动技能" => {
                let movement = &data.get("pal-details-data")["moves"][id];
                if !array(&movement["effects"])
                    .iter()
                    .any(|s| s[0].as_str() == Some(state))
                {
                    continue;
                }
                (text(movement, locale), format!("附带{state}"), true)
            }
            "物品" => {
                let Some(item) = array(&data.get("item-catalog-data")["items"])
                    .iter()
                    .find(|r| r["id"] == id && r["category"] == "武器")
                else {
                    continue;
                };
                let description = text(&data.get("item-effects-data")["items"][id], "description");
                let explicit = description.contains(&format!("使攻击目标陷入{state}状态"))
                    || match state {
                        "点燃" => description.contains("让帕鲁着火"),
                        "感电" => description.contains("敲下去就会触电"),
                        "中毒" => {
                            description.contains("让帕鲁中毒") || description.contains("使帕鲁中毒")
                        }
                        _ => false,
                    };
                if !explicit {
                    continue;
                }
                (
                    text(item, if en { "en" } else { "name" }),
                    description.into(),
                    false,
                )
            }
            _ => continue,
        };
        if !title.is_empty() && !sources.iter().any(|(name, _, _, _)| *name == title) {
            sources.push((title, marker, route, is_move));
        }
    }
    let [(title, source_text, route, is_move)] = sources.as_slice() else {
        return None;
    };
    let name = text(pal, locale);
    let skill = text(partner, "name");
    let status_id = text(status, "id");
    let summary = if en {
        let trigger = if *is_move {
            "Use the move to set up the status, then attack while it is active."
        } else {
            "The weapon can apply the status. Keep hitting the target while the status is active."
        };
        let bonus = if effect.contains("一击就会让敌人陷入冻结状态") {
            "a player hit freezes the affected target".to_owned()
        } else {
            let curves = partner["curves"].as_object()?;
            if curves.len() != 1 {
                return None;
            }
            let values = curves.values().next()?.as_array()?;
            let range = format!(
                "{}–{}%",
                values.first()?.as_u64()?,
                values.last()?.as_u64()?
            );
            if effect.contains("玩家和帕鲁对") && effect.contains("造成的伤害将提升")
            {
                format!("both player and Pal attacks gain {range} damage against that target")
            } else if effect.contains("该敌人的攻击力降低") {
                format!("attacking that target reduces its attack by {range}")
            } else if effect.contains("会使敌人周边燃烧") {
                format!(
                    "player hits create a burning area that deals damage over time based on {range} of player attack"
                )
            } else if effect.contains("会使敌人爆炸") || effect.contains("令敌人放电") {
                format!(
                    "player hits on the affected enemy trigger an area attack for {range} of player attack"
                )
            } else {
                return None;
            }
        };
        let stacking = if effect.contains("不可叠加") {
            " Multiple copies of this bonus do not stack."
        } else {
            ""
        };
        format!(
            "{title} and {name} can work together against a target susceptible to {status_id}. {trigger} With {name} in the party, {bonus}.{stacking}"
        )
    } else {
        let action = if *is_move {
            "帕鲁招式可用来先给敌人上状态，再按伙伴技能指定的攻击者和时机出手。"
        } else {
            "用它给能吃这个异常的目标上状态，在状态持续期间按伙伴技能的条件继续攻击。"
        };
        format!(
            "能配合。{title}的图鉴记录为「{source_text}」。{action}\n{name}的伙伴技能「{skill}」：{effect}"
        )
    };
    Some(AssistantReply {
        text: summary,
        links: vec![
            (name.into(), format!("#details/partner/{pal_id}")),
            ((*title).into(), (*route).into()),
            (state.into(), format!("#guide/status/{status_id}")),
        ],
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};
    #[test]
    fn exact_weapon_or_move_supplies_the_partners_required_status() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        for name in [
            "search-index-data",
            "partner-skills-data",
            "combat-guide-data",
            "item-effects-data",
            "pal-details-data",
            "item-catalog-data",
        ] {
            d.tables.insert(
                name.into(),
                Arc::new(
                    serde_json::from_slice(
                        &std::fs::read(dir.join(format!("{name}.json"))).unwrap(),
                    )
                    .unwrap(),
                ),
            );
        }
        for (q, state, expected) in [
            (
                "is the fire bow any good with majex?",
                "burn",
                "burning area",
            ),
            (
                "will aqua gun trigger jormuntide's partner skill?",
                "wet",
                "both player and Pal",
            ),
            ("火焰喷射器和昭炎狐能搭配吗？", "burn", "点燃状态"),
        ] {
            let r = answer(&d, q).unwrap_or_else(|| panic!("{q}"));
            assert!(r.text.contains(expected), "{}", r.text);
            assert!(
                r.links
                    .iter()
                    .any(|(_, link)| link == &format!("#guide/status/{state}"))
            );
        }
        assert!(
            answer(
                &d,
                "can the fire bow trigger jormuntide's wet-target bonus?"
            )
            .is_none()
        );
        assert!(answer(&d, "how to craft the fire bow to pair with majex?").is_none());
        assert!(
            answer(
                &d,
                "is the fire bow guaranteed to trigger majex's skill every hit?"
            )
            .is_none()
        );
    }
}
