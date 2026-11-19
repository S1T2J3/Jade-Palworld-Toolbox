//! Teaching a move is a different operation from increasing a pal's rank.
//! The local item effect carries the move ID. The learnset's level is for natural
//! progression, and its inheritance flag describes breeding.
//!
//! References checked against the shipped 1.0.5 tables:
//! https://palworld.wiki.gg/wiki/Skill_Fruits (party, species and element)
//! https://www.palworld-db.com/skill-fruits (fruit learning and level)
//! https://palworld.wiki.gg/wiki/Active_Skills (learned and equipped moves)
use super::AssistantReply;
use crate::assistant_discovery::relevant_names;
use crate::data::{Data, array, text};
use serde_json::Value;

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|word| q.contains(word))
}

fn teaching_question(q: &str) -> bool {
    let title = q
        .split(['?', '？', '\n'])
        .find(|s| !s.trim().is_empty())
        .unwrap_or(q);
    let subject = has(
        q,
        &[
            "技能果",
            "技能水果",
            "招式果实",
            "skill fruit",
            "skillfruit",
            "move fruit",
        ],
    ) || has(q, &["果实", "fruit"])
        && has(
            q,
            &[
                "主动技能",
                "学会",
                "招式",
                "teach a move",
                "learn a move",
                "learn the move",
                "teach the move",
            ],
        );
    if !subject {
        return false;
    }
    let using = |s: &str| {
        has(
            s,
            &[
                "怎么用",
                "如何用",
                "如何使用",
                "使用条件",
                "食用",
                "吃",
                "喂",
                "可以用",
                "能用",
                "用不了",
                "用了",
                "学",
                "领悟",
                "习得",
                "教",
                "招式",
                "满星",
                "满阶",
                "等级限制",
                "谁能用",
                "干什么",
                "有什么用",
                "作用",
                "能不能用",
                "升段苹果",
                "learn",
                "teach",
                "feed",
                "fed",
                " eat",
                "eaten",
                "can use",
                "can i use",
                "how to use",
                "how do i use",
                "used for",
                "use them for",
                "use it for",
                "what do i do with",
                "how does",
                "how do skill",
                "already know",
                "max rank",
                "max level",
                "any type",
                "any pal",
                "different element",
                "work on",
                "work with",
                "use it on",
                "usable",
                "does it give",
                "give a skill fruit",
                "give skill fruits",
                "give the fruit",
                "moveslot",
                "skill slot",
            ],
        )
    };
    if has(
        title,
        &[
            "果树园",
            "果实树",
            "种植",
            "种下",
            "刷新",
            "当种子",
            "作种子",
            "坐标",
            "在哪里",
            "哪里刷",
            "哪里获取",
            "哪里获得",
            "哪里找",
            "在哪买",
            "哪里买",
            "掉率",
            "多少钱",
            "哪里掉",
            "orchard",
            "skill fruit tree",
            "skillfruit tree",
            "where can i find",
            "where to find",
            "where do i find",
            "how to farm",
            "where can i get",
            "where to get",
            "how to grow",
            "how to plant",
            "can i plant",
            "to plant",
            "for planting",
            "as a seed",
            "grow skill fruit",
            "drop rate",
            "where to buy",
            "respawn",
            "where do i get",
        ],
    ) && !has(
        title,
        &[
            "能不能学",
            "能学",
            "不能学",
            "可以学",
            "能否学",
            "能不能吃",
            "可以喂",
            "can it learn",
            "can my pal learn",
            "can it eat",
            "can my pal eat",
            "can i feed",
        ],
    ) {
        return false;
    }
    let what_it_does = title
        .trim()
        .strip_prefix("what do ")
        .or_else(|| title.trim().strip_prefix("what does "))
        .is_some_and(|rest| {
            rest.ends_with(" do") || has(rest, &[" do for ", " do in ", " do when "])
        });
    using(q)
        || what_it_does
        || has(
            title,
            &[
                "技能果实是什么",
                "什么是技能果实",
                "what are skill fruits",
                "what is a skill fruit",
            ],
        )
}

fn move_ids<'a>(data: &'a Data, q: &str) -> Vec<&'a str> {
    let effects = &data.get("item-effects-data")["items"];
    let mut moves = Vec::new();
    for row in relevant_names(array(data.get("search-index-data")), q) {
        let href = text(row, "href");
        let key = if let Some(key) = href.strip_prefix("#details/active/move/") {
            key
        } else if let Some(item) = href.strip_prefix("#items/") {
            text(&effects[item], "skill")
        } else {
            continue;
        };
        if !key.is_empty() && !moves.contains(&key) {
            moves.push(key);
        }
    }
    moves
}

fn teaching_item<'a>(data: &'a Data, key: &str) -> Option<&'a Value> {
    let effects = &data.get("item-effects-data")["items"];
    array(&data.get("item-catalog-data")["items"])
        .iter()
        .find(|item| {
            text(item, "category") == "技能果实" && text(&effects[text(item, "id")], "skill") == key
        })
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if !teaching_question(q) {
        return None;
    }
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let moves = &data.get("pal-details-data")["moves"];
    let ids = move_ids(data, q);
    let wants_nonactive = has(
        q,
        &[
            "词条",
            "被动",
            "伙伴技能",
            "夥伴技能",
            "搭档技能",
            "passive",
            "trait",
            "partner skill",
            "partner ability",
        ],
    );
    if wants_nonactive {
        lines.push(
            "技能果实教的是对应的主动招式。被动词条和物种的伙伴技能都不属于它的教学范围。"
                .to_owned(),
        );
    }
    for key in &ids {
        let name = text(&moves[*key], "zh");
        if name.is_empty() {
            continue;
        }
        if let Some(fruit) = teaching_item(data, key) {
            let label = text(fruit, "name");
            let item = text(fruit, "id");
            lines.push(format!("「{label}」可以教会帕鲁「{name}」。"));
            links.push((label.into(), format!("#items/{item}")));
        } else if !text(&moves[*key], "fruit").is_empty() {
            lines.push(format!(
                "招式图鉴为「{name}」列出了「{}」。",
                text(&moves[*key], "fruit")
            ));
        } else {
            lines.push(format!(
                "当前招式图鉴没有给「{name}」列出对应技能果实，不能据此确认有果实可教这招。"
            ));
        }
        links.push((name.into(), format!("#details/active/move/{key}")));
    }
    if ids.is_empty() && !wants_nonactive {
        lines.push(
            "先看主动招式有没有对应的技能果实，再查看这只帕鲁的已学技能。只有部分招式制成果实，已经学过的招式可以直接在技能栏选用。".into(),
        );
        if q.contains('「')
            || q.contains('“')
            || has(
                q,
                &[
                    "这个招式",
                    "这招有果实吗",
                    "this particular move",
                    "this specific move",
                ],
            )
        {
            lines.push(
                "当前问题没有匹配到可核对的具体招式，请提供主动技能或技能果实在游戏中的全名。"
                    .into(),
            );
        }
    }
    lines.push("有对应果实时，任何属性的帕鲁都能学。把目标帕鲁放进队伍，从背包使用果实并选中它。它的属性无需与招式相同，也不用等到自然习得等级或满足某个浓缩星级。".into());
    if has(
        q,
        &[
            "已经",
            "已学",
            "重复",
            "再吃",
            "满了",
            "装满",
            "技能栏",
            "技能槽",
            "几招",
            "多少招",
            "几个技能",
            "already",
            "duplicate",
            "again",
            "slots",
            "how many",
            "equipped",
        ],
    ) {
        lines.push("可在已学主动技能列表里核对这招。战斗时可同时装备三招，学会的新招可以在队伍或帕鲁终端中换上。".into());
    }
    if has(
        q,
        &[
            "升段苹果",
            "升阶苹果",
            "ripe starfruit",
            "rank-up apple",
            "rank up apple",
            "ranking apple",
        ],
    ) {
        let apple_id = "Rankup_Arbitrary";
        let apple = array(&data.get("item-catalog-data")["items"])
            .iter()
            .find(|i| text(i, "id") == apple_id);
        let description = text(
            &data.get("item-effects-data")["items"][apple_id],
            "description",
        );
        if let Some(item) = apple.filter(|_| !description.is_empty()) {
            let label = text(item, "name");
            lines.push(format!("「{label}」的用途另见物品说明：{description}"));
            links.push((label.into(), format!("#items/{apple_id}")));
        }
    }
    if has(
        q,
        &[
            "用不了",
            "吃不了",
            "不能喂",
            "不能吃",
            "点不了",
            "没学",
            "没反应",
            "灰色",
            "报错",
            "cannot use",
            "can't use",
            "can't feed",
            "cannot feed",
            "won't eat",
            "won’t eat",
            "not working",
            "greyed",
            "grayed",
        ],
    ) {
        lines.push("如果界面仍无法使用，请提供果实全名、目标帕鲁、其已学招式和界面提示，才能进一步判断这次失败的原因。".into());
    }
    if has(
        q,
        &[
            "依据",
            "出处",
            "来源证明",
            "reference",
            "citation",
            "evidence",
        ],
    ) {
        lines.push("机制说明见 Palworld Wiki（https://palworld.wiki.gg/wiki/Skill_Fruits）与 Palworld Database（https://www.palworld-db.com/skill-fruits）。具体果实对应的招式可在下方图鉴核对。".into());
    }
    links.push((
        "查看技能果实与对应主动招式".into(),
        "#details/fruits".into(),
    ));
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{Conversation, field_text};
    use std::{path::PathBuf, sync::Arc};

    fn handbook() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in [
            "item-catalog-data",
            "item-effects-data",
            "pal-details-data",
            "search-index-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            data.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        data
    }

    #[test]
    fn teaching_conditions_do_not_inherit_rankup_or_breeding_rules() {
        let data = handbook();
        for (question, expected) in [
            ("What is a normal Skill Fruit used for?", true),
            ("What do Skill Fruits actually do?", true),
            ("What does a Skill Fruit do for a Pal?", true),
            ("What is a Skill Fruit Orchard used for?", false),
            ("What do Skill Fruit Trees do?", false),
        ] {
            assert_eq!(
                teaching_question(&field_text(question)),
                expected,
                "{question}"
            );
        }
        let mut chat = Conversation::default();
        let result = chat.ask_world(
            "技能果实可以给哪些帕鲁吃，判断能不能学的条件是什么？",
            &data,
            &serde_json::json!({}),
        );
        assert!(
            chat.direct_result()
                && result.text.contains("任何属性")
                && result.text.contains("自然习得等级")
                && result.text.contains("浓缩星级"),
            "{}",
            result.text
        );
        assert!(
            !result.text.contains("苹果")
                && !result.links.iter().any(|(_, r)| r.contains("passive"))
        );
        let both = answer(&data, &field_text("技能果實和升段蘋果吃了分別有什麼作用？")).unwrap();
        assert!(
            both.links
                .iter()
                .any(|(_, r)| r == "#items/Rankup_Arbitrary")
                && both.text.contains("阶级上升1阶")
        );
    }

    #[test]
    fn resolves_available_and_missing_fruit_by_real_move_id() {
        let data = handbook();
        let water = answer(
            &data,
            &field_text("Can a fire Pal learn Aqua Gun with a skill fruit before level 10?"),
        )
        .unwrap();
        assert!(
            water
                .links
                .iter()
                .any(|(_, r)| r == "#items/SkillCard_WaterGun"),
            "{}",
            water.text
        );
        let exclusive = answer(
            &data,
            &field_text("Can my Lamball learn Chicken Rush with a skill fruit?"),
        )
        .unwrap();
        assert!(
            exclusive.text.contains("皮皮冲鸡")
                && exclusive.text.contains("没有给")
                && exclusive
                    .links
                    .iter()
                    .all(|(_, r)| !r.starts_with("#items"))
        );
        let slots=answer(&data,&field_text("I already have three active skill slots equipped. Can my Pal learn another skill fruit?")).unwrap();
        assert!(slots.text.contains("同时装备三招"));
        assert!(
            answer(
                &data,
                &field_text("Where do I find the Aqua Gun skill fruit tree?")
            )
            .is_none()
        );
        assert!(answer(&data, &field_text("工作适性书只能给几级适性的帕鲁用？")).is_none());
        let seed = answer(
            &data,
            &field_text("Can a level 1 Pal learn Seed Mine from a Skill Fruit?"),
        )
        .unwrap();
        assert!(
            seed.links
                .iter()
                .any(|(_, p)| p.starts_with("#items/SkillCard_"))
        );
    }

    #[test]
    fn indexed_teaching_items_agree_with_the_move_pages() {
        let data = handbook();
        let fruit_items = array(&data.get("item-catalog-data")["items"])
            .iter()
            .filter(|item| text(item, "category") == "技能果实");
        let effects = &data.get("item-effects-data")["items"];
        let moves = &data.get("pal-details-data")["moves"];
        let mut count = 0;
        for item in fruit_items {
            let key = text(&effects[text(item, "id")], "skill");
            assert!(!key.is_empty());
            assert_eq!(text(&moves[key], "fruit"), text(item, "name"));
            assert_eq!(
                teaching_item(&data, key).and_then(|r| r["id"].as_str()),
                item["id"].as_str()
            );
            count += 1;
        }
        assert!(count > 0);
    }
}
