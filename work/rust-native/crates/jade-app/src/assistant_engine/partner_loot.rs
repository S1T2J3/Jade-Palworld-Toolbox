use super::{AssistantReply, field_text, quantities::integers};
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, array, text};

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|w| q.contains(w))
}

fn after_hunt_verb(prefix: &str) -> bool {
    let mut s = prefix.trim_end();
    while let Some(rest) = [
        "普通的",
        "野生的",
        "普通",
        "野生",
        "一只",
        "a wild",
        "a regular",
        "an alpha",
        "a",
        "the",
    ]
    .iter()
    .find_map(|m| s.strip_suffix(m))
    {
        s = rest.trim_end();
    }
    [
        "打",
        "击倒",
        "击败",
        "击杀",
        "讨伐",
        "猎杀",
        "杀死",
        "hunt",
        "hunting",
        "defeating",
        "killing",
        "defeat",
        "kill",
    ]
    .iter()
    .any(|v| s.ends_with(v))
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "掉落", "爆率", "掉率", "掉宝", "爆油", "掉油", "刷油", "drop", "loot",
        ],
    ) || !has(
        q,
        &[
            "队",
            "带上",
            "带着",
            "我带",
            "升星",
            "浓缩",
            "升级",
            "伙伴",
            "技能",
            "加成",
            "配合",
            "猎龙者",
            "condens",
            "in my party",
            "in the party",
            "with ",
            "partner",
            "leveling up",
            "upgrading",
            "hunter",
        ],
    ) {
        return None;
    }
    let en = q.is_ascii();
    let (mentions, _) = species_mentions(&d.species, q);
    let raid_reward = !en && has(q, &["团战", "突袭", "团本", "结算奖励", "首通奖励"]);
    let reward_limit = "技能条目确认的是符合条件的敌人掉落。单凭它无法确定团战结算、首通奖励或奖励蛋也有加成。";
    if raid_reward && mentions.is_empty() && has(q, &["伙伴技能", "掉落加成", "掉落技能", "掉宝技能"]) {
        for pal in &d.species {
            let key = text(pal, "key");
            let row = &d.get("partner-skills-data")[key];
            let effect = text(row, "effect").split('。').next().unwrap_or("");
            if effect.starts_with("若它在队伍中，击倒") && effect.contains("属性帕鲁时获得的掉落道具增加") {
                let name = text(pal, "zh");
                let skill = text(row, "name");
                return Some(AssistantReply {
                    text: format!("要看所带帕鲁和敌人的属性。例如{name}的「{skill}」：{effect}。{reward_limit}"),
                    links: vec![(skill.to_owned(), format!("#details/partner/{key}"))],
                    suggestions:vec![],
                });
            }
        }
    }
    let mut hunters = Vec::new();
    for &(i, start, _) in &mentions {
        let pal = &d.species[i];
        let key = text(pal, "key");
        let row = &d.get("partner-skills-data")[key];
        let effect = text(row, "effect");
        let Some((_, clause)) = effect.split_once("击倒") else {
            continue;
        };
        let Some((element, amount)) = clause.split_once("属性帕鲁时获得的掉落道具增加")
        else {
            continue;
        };
        let marker = amount.split_once(')')?.0.to_owned() + ")";
        // A Pal being hunted is a target even if its own partner skill also affects drops.
        if after_hunt_verb(&q[..start]) {
            continue;
        }
        if !hunters
            .iter()
            .any(|(idx, ..): &(usize, &str, &str, &serde_json::Value)| *idx == i)
        {
            hunters.push((i, element, key, &row["curves"][&marker]));
        }
    }
    let [(hunter, element, key, curve)] = hunters.as_slice() else {
        return None;
    };
    let values: Vec<_> = array(curve)
        .iter()
        .map(|v| v.as_u64())
        .collect::<Option<_>>()?;
    if values.len() != 5 {
        return None;
    }
    let pal = &d.species[*hunter];
    let name = text(pal, if en { "en" } else { "zh" });
    let skill = text(&d.get("partner-skills-data")[*key], "name");
    let elements = d.get("combat-guide-data")["elements"].as_object()?;
    let kind = elements
        .iter()
        .find(|(_, name)| name.as_str() == Some(*element))?
        .0;
    let bonuses = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("Lv.{} +{v}%", i + 1))
        .collect::<Vec<_>>()
        .join(" / ");
    let mut msg = if en {
        format!(
            "Keep {name} in your party and defeat {kind}-type Pals. Its partner skill increases the drops obtained from those targets. Bonuses by skill level: {bonuses}. The skill does not add new items to a target's loot table. The data labels this as a loot bonus, without specifying the new probability of any single rare item."
        )
    } else {
        format!(
            "把{name}放在队伍中，击倒{element}属性帕鲁时，「{skill}」会增加从目标获得的掉落。各伙伴技能等级为 {bonuses}。目标原本就要能掉你要的物品。图鉴给出的是掉落加成，无法直接换算为某件稀有物品的单次概率。"
        )
    };
    let mut links = vec![(skill.to_owned(), format!("#details/partner/{key}"))];
    if raid_reward {
        msg.push_str(reward_limit);
    }
    let mut seen = Vec::new();
    for &(i, _, _) in &mentions {
        if i == *hunter || seen.contains(&i) {
            continue;
        }
        seen.push(i);
        let target = &d.species[i];
        let target_key = text(target, "key");
        let valid = array(&target["types"])
            .iter()
            .any(|v| v.as_str() == Some(kind));
        let target_name = text(target, if en { "en" } else { "zh" });
        msg.push_str(&match (en, valid) {
            (true, true) => format!(" {target_name} meets the element requirement."),
            (true, false) => format!(" {target_name} does not meet the element requirement."),
            (false, true) => format!(" {target_name}符合这个属性条件。"),
            (false, false) => format!(" {target_name}不符合这个属性条件。"),
        });
        links.push((target_name.to_owned(), format!("#paldex/{target_key}")));
    }
    let requested = array(&d.get("item-catalog-data")["items"])
        .iter()
        .filter(|it| {
            [text(it, "name"), text(it, "en")]
                .iter()
                .any(|s| s.chars().count() >= 2 && q.contains(&field_text(s)))
                || (text(it, "id") == "PalOil" && has(q, &["pal oil", "帕鲁油"]))
        })
        .max_by_key(|it| text(it, if en { "en" } else { "name" }).len());
    if let Some(it) = requested {
        let item = text(it, "id");
        let mut examples = Vec::new();
        for (i, target) in d.species.iter().enumerate() {
            if !seen.is_empty() && !seen.contains(&i) {
                continue;
            }
            if !array(&target["types"])
                .iter()
                .any(|v| v.as_str() == Some(kind))
            {
                continue;
            }
            let target_key = text(target, "key");
            let drops = array(&d.get("paldex-data")[target_key]["drops"]);
            if !drops.iter().any(|tier| {
                tier[0] == 0 && array(&tier[1]).iter().any(|r| r[0].as_str() == Some(item))
            }) {
                continue;
            }
            let target_name = text(target, if en { "en" } else { "zh" });
            examples.push(target_name);
            let url = format!("#paldex/{target_key}");
            if !links.iter().any(|(_, href)| *href == url) {
                links.push((target_name.to_owned(), url));
            }
            if examples.len() == 3 {
                break;
            }
        }
        if !examples.is_empty() {
            let item_name = text(it, if en { "en" } else { "name" });
            msg.push_str(&if en {
                format!(
                    " Eligible targets whose base drop table includes {item_name}: {}.",
                    examples.join(", ")
                )
            } else {
                format!(
                    " 符合条件且基础掉落含{item_name}的目标有{}。",
                    examples.join("、")
                )
            });
        }
    }
    if has(q, &["stack", "叠加", "叠不叠"])
        && text(&d.get("partner-skills-data")[*key], "effect").contains("不可叠加")
    {
        msg.push_str(if en {
            " The skill is marked non-stacking."
        } else {
            " 该技能标注为不可叠加。"
        });
    }
    if has(q, &["chance", "probab", "概率", "几率"]) && !integers(q).is_empty() && seen.is_empty()
    {
        msg.push_str(if en {
            " For an item-specific calculation, identify the defeated species and its level."
        } else {
            " 要核对某件物品的基础掉率，还需要知道实际被击倒的物种和等级。"
        });
    }
    Some(AssistantReply {
        text: msg,
        links,
        suggestions: vec![],
    })
}
