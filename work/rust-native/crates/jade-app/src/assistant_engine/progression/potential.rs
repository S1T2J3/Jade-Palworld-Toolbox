//! Individual growth and inheritable potential. Offline evidence: versioned game stat
//! calculator and item descriptions, and the bundled "Passing Down Potentials" article.
use super::{AssistantReply, Data, array, has, text};

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let iv = q
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| matches!(w, "iv" | "ivs"));
    if !(iv
        || has(
            q,
            &[
                "potential",
                "individual value",
                "individual stat",
                "个体值",
                "潜力值",
                "个体潜力",
                "个体潜能",
                "三项潜力",
                "三维潜能",
                "潜力",
                "潜能",
            ],
        ))
        || !has(
            q,
            &[
                "raise",
                "improve",
                "increase",
                "boost",
                "max out",
                "maximize",
                "maximise",
                "change",
                "upgrade",
                "without",
                "inherit",
                "train",
                "improvement",
                "提升",
                "提高",
                "加满",
                "拉满",
                "怎么加",
                "怎么升",
                "如何升",
                "补满",
                "刷满",
                "遗传",
                "不吃",
                "不用果",
                "能练",
                "能改",
            ],
        )
        || has(
            q,
            &[
                "chance",
                "probability",
                "odds",
                "how many",
                "how much",
                "where",
                "cost",
                "drop",
                "how to craft",
                "formula",
                "passive",
                "skill fruit",
                "modded",
                "player's",
                "my character",
                "概率",
                "几率",
                "几颗",
                "多少",
                "哪里",
                "掉落",
                "制作",
                "配方",
                "词条",
                "技能果",
                "玩家属性",
                "主角",
                "公式",
            ],
        )
    {
        return None;
    }
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let items = array(&d.get("item-catalog-data")["items"]);
    let mut links = Vec::new();
    let mut choices = Vec::new();
    for (id, zh, eng) in [
        ("Fruit_hp_01", "生命", "HP"),
        ("Fruit_attack_01", "攻击", "Attack"),
        ("Fruit__defense_01", "防御", "Defense"),
    ] {
        let item = items.iter().find(|i| i["id"] == id)?;
        let name = text(item, if en { "en" } else { "name" });
        choices.push(format!("{name} → {}", if en { eng } else { zh }));
        links.push((name.into(), format!("#items/{id}")));
    }
    links.push((
        if en {
            "Ability Glasses"
        } else {
            "伯乐眼镜"
        }
        .into(),
        "#items/Accessory_TalentChecker".into(),
    ));
    links.push((
        if en {
            "Breeding and potential"
        } else {
            "亲本潜力的遗传"
        }
        .into(),
        "https://www.thepalwiki.com/wiki/breeding".into(),
    ));
    let fruit = choices.join(" / ");
    let msg = if en {
        format!(
            "Equip Ability Glasses to check the current Pal's three individual potentials, each out of 100. To raise the same Pal's IV, give it the fruit for that stat: {fruit}. Leveling, condensation and Pal souls improve the displayed stats through separate bonuses and leave the three IVs unchanged.\nTo aim for high IVs without fruit, catch and inspect other individuals, or breed parents with the high HP, Attack or Defense IVs you want. Inspect the offspring and keep the best as parents for the next generation. This produces a new individual, and each egg can inherit different values."
        )
    } else {
        format!(
            "先戴伯乐眼镜看这只帕鲁的生命、攻击、防御三项潜力，各项上限 100。想补这只帕鲁的潜力，按属性喂果实：{fruit}。升级、浓缩和帕鲁之魂都能通过各自的机制提升面板，个体值仍保持原值。\n如果想省下果实，可以挑选野生个体，或让亲本分别提供所需的高潜力。孵化后用眼镜挑出继承较好的一只继续配种。这样得到的是新的个体，每个蛋继承的数值可能不同。"
        )
    };
    Some(AssistantReply {
        text: msg,
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_one_individual_from_breeding_and_panel_training() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        data.tables.insert(
            "item-catalog-data".into(),
            std::sync::Arc::new(
                serde_json::from_slice(
                    &std::fs::read(root.join("item-catalog-data.json")).unwrap(),
                )
                .unwrap(),
            ),
        );
        for (q, facts) in [
            (
                "怎么提升这只帕鲁的防御潜能？",
                ["坚壁之果", "伯乐眼镜", "新的个体"],
            ),
            (
                "can i improve a pal's ivs without any fruit?",
                ["Stout Fruit", "condensation", "breed parents"],
            ),
        ] {
            let mut c = crate::assistant_engine::Conversation::default();
            let a = c.ask_world(q, &data, &serde_json::json!({"id":"player"}));
            assert!(
                c.direct_result() && facts.iter().all(|s| a.text.contains(s)),
                "{q}: {}",
                a.text
            );
        }
        for q in [
            "what is the chance to inherit a defense iv?",
            "where can i find a fruit to improve my iv?",
            "how can i improve my survival build?",
        ] {
            assert!(answer(&data, q).is_none(), "{q}");
        }
    }
}
