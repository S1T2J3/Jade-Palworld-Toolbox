//! Read the requested interaction from the named species or map category.
use super::AssistantReply;
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, array, text};

fn mentions(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| q.contains(term))
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if !crate::assistant_discovery::handbook_version_matches(q) {
        return None;
    }
    let title = q.lines().next()?;
    let start = [
        "我问的是",
        "我想问的是",
        "而是想问",
        "但我想问",
        "my question is",
        "what i want to know is",
    ]
    .iter()
    .filter_map(|marker| title.rfind(marker).map(|i| i + marker.len()))
    .max()
    .unwrap_or(0);
    let focus = &title[start..];
    if mentions(
        focus,
        &["模组", "魔改", "modded", "with a mod", "with mods"],
    ) {
        return None;
    }
    map_terminal(d, focus)
        .or_else(|| supply_power(d, focus))
        .or_else(|| riding_gear(d, focus))
}

fn supply_power(d: &Data, q: &str) -> Option<AssistantReply> {
    if !mentions(
        q,
        &[
            "供电",
            "电力不足",
            "供不上电",
            "电不够用",
            "power my",
            "power the",
            "keep power",
            "keep powered",
        ],
    ) || !mentions(
        q,
        &[
            "怎么",
            "如何",
            "怎样",
            "电力不足",
            "供不上电",
            "不够用",
            "how",
            "can't keep",
            "cannot keep",
        ],
    ) || mentions(
        q,
        &[
            "多少",
            "几台",
            "至少",
            "每秒",
            "高压",
            "high-pressure",
            "how many",
            "per second",
        ],
    ) {
        return None;
    }
    let mut matches = array(&d.get("resource-guides-data")["production"])
        .iter()
        .filter(|r| {
            let name = text(r, "name");
            let english = text(r, "facility").replace('_', " ").to_lowercase();
            r["power"].as_u64().is_some_and(|p| p > 0)
                && ((!name.is_empty() && q.contains(name))
                    || (!english.is_empty() && q.contains(&english)))
        });
    let machine = matches.next()?;
    if matches.any(|m| m["facility"] != machine["facility"]) {
        return None;
    }
    let facilities = array(&d.get("facilities-data")["facilities"]);
    let generator = facilities
        .iter()
        .find(|row| row["id"] == "Large_Power_Generator")?;
    let pylon = facilities
        .iter()
        .find(|row| row["id"] == "Electric_Pylon")?;
    let bonus = pylon["effects"].get(0)?.as_str()?;
    let rate = machine["power"].as_u64()?;
    let body = if q.is_ascii() {
        format!(
            "The {} uses {rate} game power units per second while running. Use a Large Power Generator with a Pal that has Generating Electricity suitability, and place an Electric Pylon to boost its work speed. Watch the base's stored charge with the machine running and verify a worker is on duty at night before adding more load.",
            text(machine, "facility").replace('_', " ")
        )
    } else {
        format!(
            "{}运转时每秒消耗 {rate} 游戏电力单位。可把基础发电机换为{}，安排有发电适性、效率较高的帕鲁在岗。{}的效果是{}。开机后观察据点蓄电量是否持续下降，也要检查夜里有没有工人发电。",
            text(machine, "name"),
            text(generator, "name"),
            text(pylon, "name"),
            bonus
        )
    };
    let links = [
        (text(machine, "name"), text(machine, "facility")),
        (text(generator, "name"), text(generator, "id")),
        (text(pylon, "name"), text(pylon, "id")),
    ]
    .into_iter()
    .map(|(name, id)| (name.into(), format!("#work/facilities/{id}")))
    .collect();
    Some(AssistantReply {
        text: body,
        links,
        suggestions: vec![],
    })
}

fn map_terminal(d: &Data, q: &str) -> Option<AssistantReply> {
    if !mentions(
        q,
        &[
            "防空炮",
            "防空装置",
            "防空塔",
            "防空区",
            "天坠之地炮台",
            "高射炮",
            "anti-air",
            "anti air",
            "air defense cannon",
        ],
    ) || !mentions(
        q,
        &[
            "能关",
            "关闭",
            "关掉",
            "解除",
            "停用",
            "怎么关",
            "怎么停",
            "如何关",
            "禁用",
            "disable",
            "deactivate",
            "turn off",
            "switch off",
        ],
    ) {
        return None;
    }
    let point = array(&d.get("map-data")["points"])
        .iter()
        .find(|p| p["category"] == "anti_air" && !text(p, "detail").is_empty())?;
    let zh = q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let detail = text(point, "detail");
    let answer = if mentions(
        q,
        &[
            "我造的",
            "我建的",
            "自建",
            "我自己据点",
            "我的据点",
            "my own base",
            "my base turret",
        ],
    ) {
        if zh {
            "地图资料收录的是天坠之地的固定防空炮。你说的是自己的据点，请提供建造菜单里的设施全名，以便核对它的开关。".into()
        } else {
            "The map records the fixed Feybreak anti-air sites. What is the exact build-menu name of the structure at your own base?".into()
        }
    } else if zh {
        format!("天坠之地的固定防空炮可以逐处解除。{detail}")
    } else {
        // The English rendering describes the interaction recorded in this category.
        if !detail.contains("操作防空炮终端") {
            return None;
        }
        "For the fixed Feybreak anti-air sites, enter the camp on the ground and interact with its terminal. This deactivates that site's anti-air zone.".into()
    };
    Some(AssistantReply {
        text: answer,
        links: vec![(
            text(point, "name").into(),
            format!("#map/category/anti_air/{}", text(point, "map")),
        )],
        suggestions: vec![],
    })
}

fn riding_gear(d: &Data, q: &str) -> Option<AssistantReply> {
    if !mentions(
        q,
        &[
            "鞍具",
            "马鞍",
            "的鞍",
            "坐骑装备",
            "能骑",
            "可骑",
            "可以骑",
            "不能骑",
            "没法骑",
            "怎么骑",
            "saddle",
            "can i ride",
            "can't ride",
            "can you ride",
        ],
    ) {
        return None;
    }
    // Crafting budgets, speed comparisons and incidental riding references have their own evidence.
    if mentions(
        q,
        &[
            "材料",
            "配方",
            "库存",
            "还缺",
            "速度",
            "飞多快",
            "掉落",
            "配种",
            "怎么买",
            "哪里买",
            "卖价",
            "耐久",
            "多少钱",
            "刷到",
            "坐标",
            "material",
            "ingredient",
            "recipe",
            "stock",
            "speed",
            "how fast",
            "buy",
            "drop rate",
            "breed",
            "price",
            "coordinate",
        ],
    ) {
        return None;
    }
    let (matches, _) = species_mentions(&d.species, q);
    let index = matches.first()?.0;
    if !matches.iter().all(|m| m.0 == index) {
        return None;
    }
    let pal = &d.species[index];
    let key = text(pal, "key");
    let skill = &d.get("partner-skills-data")[key];
    let effect = skill["effect"].as_str()?;
    let skill_name = skill["name"].as_str()?;
    let catalog = array(&d.get("item-catalog-data")["items"]);
    if effect.is_empty() || !catalog.iter().any(|i| i["category"] == "帕鲁装备") {
        return None;
    }
    let gear = array(&skill["gear"]);
    // A missing link in a partner row is insufficient to rule out its species equipment.
    let species_gear_id = format!("SkillUnlock_{key}");
    if catalog.iter().any(|i| i["id"] == species_gear_id)
        && !gear
            .iter()
            .any(|g| g.get(0).and_then(|v| v.as_str()) == Some(species_gear_id.as_str()))
    {
        return None;
    }
    let ride = mentions(effect, &["可骑在它", "可坐在它背后"]);
    let zh = q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let name = text(pal, if zh { "zh" } else { "en" });
    let mut links = vec![(skill_name.into(), format!("#details/partner/{key}"))];
    let mut body = if zh {
        if ride {
            format!("{name}可以骑乘。")
        } else {
            format!(
                "{name}的伙伴技能「{skill_name}」没有骑乘效果，图鉴没有该物种的鞍具科技。{effect}"
            )
        }
    } else if ride {
        format!("{name} can be ridden.")
    } else {
        format!(
            "{name} has no riding effect on its recorded Partner Skill and no species saddle technology."
        )
    };
    for g in gear {
        let id = g.get(0)?.as_str()?;
        let item = catalog.iter().find(|item| item["id"] == id)?;
        let gear_name = text(item, if zh { "name" } else { "en" });
        let tech = skill["tech"].as_u64()?;
        body.push_str(&if zh {
            format!(" 它的专用装备是「{gear_name}」，在科技等级 {tech} 解锁。")
        } else {
            format!(" Its recorded Pal Gear is {gear_name}, unlocked at technology level {tech}.")
        });
        links.push((gear_name.into(), format!("#items/{id}")));
    }
    if ride && gear.is_empty() {
        body.push_str(if zh {" 伙伴技能与装备表均未列出专用鞍具，现有条目无法确认额外的解锁条件。"}
            else {" The records do not list a species saddle or establish an additional unlock requirement."});
    }
    Some(AssistantReply {
        text: body,
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    fn game_data() -> Data {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        for table in [
            "map-data",
            "item-catalog-data",
            "partner-skills-data",
            "resource-guides-data",
            "facilities-data",
        ] {
            let bytes = std::fs::read(dir.join(format!("{table}.json"))).unwrap();
            d.tables.insert(
                table.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        d
    }

    #[test]
    fn matches_the_species_and_separates_riding_from_gear_names() {
        let d = game_data();
        let ask = |q| answer(&d, &super::super::field_text(q));
        let absent = ask("抓到波魯傑克斯，但卻沒有馬鞍可以學，請問是需要甚麼前置任務嗎?").unwrap();
        assert!(
            absent.text.contains("没有该物种的鞍具科技") && absent.text.contains("凶猛的迅雷龙")
        );
        assert_eq!(absent.links[0].1, "#details/partner/ThunderDragonMan");
        let regular = ask("云海鹿的鞍具几级解锁？").unwrap();
        assert!(regular.text.contains("26") && !regular.text.contains("雷隐鹿"));
        let special = ask("空涡龙可以骑吗？为什么找不到鞍具？").unwrap();
        assert!(
            special.text.contains("可以骑乘")
                && special
                    .links
                    .iter()
                    .any(|l| l.1 == "#items/SkillUnlock_JetDragon")
        );
        let free = ask("奥沧鲸的鞍具在哪？").unwrap();
        assert!(free.text.contains("可以骑乘") && free.text.contains("未列出"));
        assert!(ask("云海鹿的鞍具要多少材料").is_none());
        assert!(ask("用什么配种能得到云海鹿？我有鞍具。").is_none());
        assert!(ask("云海鹿和疾风隼的鞍具哪个早？").is_none());
    }

    #[test]
    fn ties_the_requested_action_to_the_map_category_or_private_container() {
        let d = game_data();
        let ask = |q| answer(&d, &super::super::field_text(q));
        let reply = ask("天墜之地的防空裝置要怎麼關？").unwrap();
        assert!(
            reply.text.contains("终端") && reply.links[0].1 == "#map/category/anti_air/palpagos"
        );
        assert!(
            ask("我建的防空炮怎么关？")
                .unwrap()
                .text
                .contains("设施全名")
        );
        assert!(ask("天坠之地的防空炮？我问的是空气弹技能能不能关掉").is_none());
        assert!(ask("空气弹技能能关掉吗？").is_none());
        assert!(ask("防空炮掉落什么？").is_none());
        let private = super::super::mechanics::answer("朋友放了私人锁的储物箱，谁能拆？").unwrap();
        assert!(private.text.contains("实际放置箱子的人") && !private.text.contains("钥匙"));
        let access =
            super::super::mechanics::answer("野外宝箱的钥匙能开别人放的私锁储物箱吗？").unwrap();
        assert!(access.text.contains("没有记载可用哪把宝箱钥匙"));
        assert!(super::super::mechanics::answer("我捡到钥匙，野外宝箱怎么开？").is_none());
        let power = ask("怎么给原油提炼机供电？\n是不是干脆去配一群波鲁杰克斯？").unwrap();
        assert!(
            power.text.contains("大型发电机")
                && power.text.contains("输电塔")
                && power.text.contains("2000")
        );
        assert!(ask("原油提炼机要多少电？我有3台。").is_none());
        let traits = super::super::mechanics::answer("繁育子代能去掉固定词条吗？").unwrap();
        assert!(traits.text.contains("未继承") && !traits.text.contains("亲本组合"));
    }
}
