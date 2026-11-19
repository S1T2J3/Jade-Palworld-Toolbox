//! Choose current accessories from the same effect rows and recipes shown in the item dex.
use crate::assistant_engine::AssistantReply;
use crate::data::{Data, array, text};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Recipient {
    #[default]
    Any,
    Player,
    Pal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Goal {
    #[default]
    Attack,
    Defense,
    Health,
    Carry,
    WorkSpeed,
}

impl Goal {
    fn stat(self) -> &'static str {
        match self {
            Self::Attack => "攻击力",
            Self::Defense => "防御力",
            Self::Health => "生命值上限",
            Self::Carry => "负重上限",
            Self::WorkSpeed => "工作速度",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EquipmentQuery {
    pub recipient: Recipient,
    pub goal: Goal,
    pub element: Option<String>,
    pub craft_only: bool,
    pub level: Option<u32>,
}

struct Recipe<'a> {
    entry: &'a Value,
    station: Option<(&'a str, u32)>,
    min_level: Option<u32>,
}

fn earliest_recipe<'a>(item: &'a Value, stations: &HashMap<&str, u32>) -> Option<Recipe<'a>> {
    array(&item["recipes"])
        .iter()
        .filter(|r| !array(&r["materials"]).is_empty())
        .map(|entry| {
            let possible_stations = array(&entry["stations"]);
            let station = possible_stations
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|name| Some((name, *stations.get(name)?)))
                .min_by_key(|(_, level)| *level);
            let tech = entry["tech"].as_u64().and_then(|n| n.try_into().ok());
            let min_level = if possible_stations.is_empty() {
                tech
            } else {
                station.map(|(_, station_level)| station_level.max(tech.unwrap_or(0)))
            };
            Recipe {
                entry,
                station,
                min_level,
            }
        })
        .min_by_key(|r| r.min_level.unwrap_or(u32::MAX))
}

/// The bundled data distinguishes “自身受到的…伤害” from “队伍帕鲁…属性伤害”.
/// Match complete positive stat names. A defensive item must never become an attack suggestion.
fn bonus(lines: &[&str], stat: &str) -> u32 {
    lines
        .iter()
        .flat_map(|line| line.split('；'))
        .filter_map(|part| {
            let amount = part.trim().strip_prefix(stat)?.trim().strip_prefix('+')?;
            amount.trim_end_matches('%').trim().parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
}

struct Accessory<'a> {
    item: &'a Value,
    lines: Vec<&'a str>,
    recipe: Option<Recipe<'a>>,
    player: u32,
    pal: u32,
    element: u32,
    has_element_bonus: bool,
}

fn select<'a>(data: &'a Data, query: &EquipmentQuery) -> Vec<Accessory<'a>> {
    let stations = array(&data.get("facilities-data")["facilities"])
        .iter()
        .filter_map(|r| Some((text(r, "name"), r["level"].as_u64()?.try_into().ok()?)))
        .collect();
    let player_stat = format!("自身{}", query.goal.stat());
    let pal_stat = format!("队伍帕鲁{}", query.goal.stat());
    let element_stat = query
        .element
        .as_ref()
        .filter(|_| query.goal == Goal::Attack)
        .map(|element| format!("队伍帕鲁{element}属性伤害"));
    let effects = &data.get("item-effects-data")["items"];
    array(&data.get("item-catalog-data")["items"])
        .iter()
        .filter(|item| text(item, "category") == "饰品" && item["other"] != true)
        .filter_map(|item| {
            let lines: Vec<_> = array(&effects[text(item, "id")]["effects"])
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|v| v.split_once(" · ").map(|(_, effect)| effect))
                .collect();
            let player = bonus(&lines, &player_stat);
            let pal = bonus(&lines, &pal_stat);
            let element = element_stat
                .as_deref()
                .map_or(0, |stat| bonus(&lines, stat));
            if player == 0 && pal == 0 && element == 0 {
                return None;
            }
            let recipe = earliest_recipe(item, &stations);
            if query.craft_only && recipe.is_none() {
                return None;
            }
            if let Some(level) = query.level {
                let minimum = recipe.as_ref()?.min_level?;
                if minimum > level {
                    return None;
                }
            }
            let has_element_bonus = lines
                .iter()
                .any(|v| v.starts_with("队伍帕鲁") && v.contains("属性伤害 +"));
            Some(Accessory {
                item,
                lines,
                recipe,
                player,
                pal,
                element,
                has_element_bonus,
            })
        })
        .collect()
}

fn priority(item: &Accessory, recipient: Recipient, q: &EquipmentQuery) -> Option<(u32, u32, u32)> {
    if recipient == Recipient::Player {
        return (item.player > 0).then_some((item.player, item.pal, 0));
    }
    if q.goal == Goal::Attack && q.element.is_some() {
        // Among matching elemental bonuses prefer the stronger bonus, then the bigger general attack
        // buff. These describe different attributes and must never be added into a DPS percentage.
        return (item.element > 0 || item.pal > 0 && !item.has_element_bonus).then_some((
            item.element,
            item.pal,
            item.player,
        ));
    }
    // Without a skill element use unrestricted buffs. Avoid choosing an arbitrary elemental baton.
    (item.pal > 0 && (!item.has_element_bonus || q.goal != Goal::Attack)).then_some((
        item.pal,
        item.player,
        u32::from(!item.has_element_bonus),
    ))
}

fn append_group(
    reply: &mut AssistantReply,
    data: &Data,
    candidates: &[Accessory],
    q: &EquipmentQuery,
    recipient: Recipient,
) {
    let mut ranked: Vec<_> = candidates
        .iter()
        .filter_map(|r| Some((priority(r, recipient, q)?, r)))
        .collect();
    ranked.sort_by(|(ap, a), (bp, b)| {
        bp.cmp(ap)
            .then_with(|| {
                let level = |v: &Accessory<'_>| {
                    v.recipe
                        .as_ref()
                        .and_then(|r| r.min_level)
                        .unwrap_or(u32::MAX)
                };
                level(a).cmp(&level(b))
            })
            .then_with(|| text(a.item, "name").cmp(text(b.item, "name")))
    });
    if ranked.is_empty() {
        return;
    }
    let subject = if recipient == Recipient::Player {
        "玩家"
    } else {
        "队伍帕鲁"
    };
    let heading = match (&q.element, q.goal, recipient) {
        (Some(element), Goal::Attack, Recipient::Pal) => format!(
            "主打{element}属性技能时，优先选提高{element}属性伤害、同时增加帕鲁攻击力的饰品。"
        ),
        (_, Goal::Attack, Recipient::Pal) => {
            "帕鲁打伤害时，以下推荐按通用攻击加成排序，换属性技能也能用。".into()
        }
        (_, Goal::Attack, Recipient::Player) => "自己开火或近战时，优先提高玩家自身攻击力。".into(),
        _ => format!("按{subject}{}加成从高到低推荐。", q.goal.stat()),
    };
    if !reply.text.is_empty() {
        reply.text.push_str("\n\n");
    }
    reply.text.push_str(&heading);
    for (_, candidate) in ranked.iter().take(2) {
        let name = text(candidate.item, "name");
        let href = format!("#items/{}", text(candidate.item, "id"));
        reply
            .text
            .push_str(&format!("\n{name} · {}", candidate.lines.join("，")));
        if !reply.links.iter().any(|(_, url)| url == &href) {
            reply.links.push((name.to_owned(), href));
        }
        if (q.craft_only || q.level.is_some())
            && let Some(recipe) = &candidate.recipe
        {
            let mut requirements = Vec::new();
            if let Some(id) = recipe.entry["plan"].as_str().filter(|s| !s.is_empty()) {
                requirements.push(data.item_name(id));
            }
            if let Some(tech) = recipe.entry["tech"].as_u64() {
                requirements.push(format!("配方科技 Lv.{tech}"));
            }
            if let Some((station, level)) = recipe.station {
                requirements.push(format!("{station}（设施 Lv.{level}）"));
            }
            if !requirements.is_empty() {
                reply
                    .text
                    .push_str(&format!("\n　制作需要{}", requirements.join("、")));
            }
        }
    }
}

pub fn recommend(data: &Data, query: &EquipmentQuery) -> AssistantReply {
    let mut reply = AssistantReply {
        text: String::new(),
        links: Vec::new(),
        suggestions: Vec::new(),
    };
    if !data.ready("item-catalog-data")
        || !data.ready("item-effects-data")
        || query.level.is_some() && !data.ready("facilities-data")
    {
        reply.text = "饰品资料尚未就绪。资料载入后可再查一次。".into();
        return reply;
    }
    let candidates = select(data, query);
    if query.recipient != Recipient::Pal {
        append_group(&mut reply, data, &candidates, query, Recipient::Player);
    }
    if query.recipient != Recipient::Player {
        append_group(&mut reply, data, &candidates, query, Recipient::Pal);
    }
    if reply.links.is_empty() {
        let subject = if query.recipient == Recipient::Pal {
            "队伍帕鲁"
        } else {
            "对应角色"
        };
        let condition = match query.level {
            Some(level) => format!("符合 {level} 级配方与设施条件、"),
            None if query.craft_only => "已收录制作配方、".into(),
            None => String::new(),
        };
        reply.text = format!(
            "当前资料里没有找到{condition}明确增加{subject}{}的饰品。",
            query.goal.stat()
        );
    } else if query.goal == Goal::Attack
        && query.recipient != Recipient::Player
        && query.element.is_none()
    {
        reply
            .text
            .push_str("\n\n如果帕鲁主要用同一属性的技能，可以再按技能属性挑专属增伤饰品。");
        reply.suggestions.push("主打暗属性技能".into());
    }
    if query.craft_only || query.level.is_some() {
        reply.text.push_str(
            "\n制作筛选依据图鉴配方。实际能否制作，还取决于已解锁的设施、持有的设计图和材料。",
        );
    } else if !reply.links.is_empty() {
        reply.suggestions.push("只看能制作的".into());
    }
    if query.recipient == Recipient::Any && query.goal == Goal::Attack {
        reply.suggestions.push("主要靠自己开枪".into());
    }
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn real_equipment_effects_beneficiaries_and_unlocks_stay_separate() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["item-catalog-data", "item-effects-data", "facilities-data"] {
            let tab =
                serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json"))).unwrap())
                    .unwrap();
            data.tables.insert(name.into(), Arc::new(tab));
        }
        let mut query = EquipmentQuery {
            recipient: Recipient::Player,
            ..Default::default()
        };
        let reply = recommend(&data, &query);
        assert_eq!(reply.links[0].1, "#items/Accessory_PPAT_1");
        assert!(reply.text.contains("自身攻击力 +20%"));
        assert!(
            reply
                .links
                .iter()
                .any(|(_, r)| r == "#items/Accessory_AT_1")
        );
        assert!(
            !reply
                .links
                .iter()
                .any(|(_, r)| r == "#items/Accessory_AT_3")
        );
        query.level = Some(18);
        query.craft_only = true;
        let reply = recommend(&data, &query);
        assert_eq!(reply.links[0].1, "#items/Accessory_AT_1");
        assert!(reply.text.contains("攻击吊坠设计图") && !reply.text.contains("汪宗师"));
        query.level = Some(7);
        assert!(recommend(&data, &query).links.is_empty());
        query = EquipmentQuery {
            recipient: Recipient::Pal,
            element: Some("暗".into()),
            ..Default::default()
        };
        let reply = recommend(&data, &query);
        assert_eq!(reply.links[0].1, "#items/Otomo_ATDark_ElementBoost_1");
        assert!(
            reply.text.contains("队伍帕鲁攻击力 +16%")
                && reply.text.contains("队伍帕鲁暗属性伤害 +20%")
        );
        assert!(!reply.text.contains("+36%") && !reply.text.contains("炎帝"));
        query = EquipmentQuery {
            recipient: Recipient::Player,
            goal: Goal::Carry,
            ..Default::default()
        };
        let reply = recommend(&data, &query);
        assert!(reply.text.contains("负重上限 +300"));
        query.recipient = Recipient::Pal;
        assert!(recommend(&data, &query).links.is_empty());

        // The winner follows the loaded table even when a future update changes an effect.
        let table = Arc::make_mut(data.tables.get_mut("item-effects-data").unwrap());
        table["items"]["Accessory_AT_1"]["effects"] =
            serde_json::json!(["提升攻击 · 自身攻击力 +24%"]);
        let query = EquipmentQuery {
            recipient: Recipient::Player,
            ..Default::default()
        };
        assert_eq!(recommend(&data, &query).links[0].1, "#items/Accessory_AT_1");
    }
}
