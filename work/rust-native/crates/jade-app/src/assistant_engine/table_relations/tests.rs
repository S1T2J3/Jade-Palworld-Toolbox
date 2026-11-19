use super::*;
use std::{path::PathBuf, sync::Arc};

fn handbook() -> Data {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut d = Data::new(path.clone(), eframe::egui::Context::default()).unwrap();
    for name in [
        "trait-data",
        "pal-details-data",
        "partner-skills-data",
        "stat-data",
        "spawn-data",
    ] {
        let raw = std::fs::read(path.join(format!("{name}.json"))).unwrap();
        d.tables
            .insert(name.into(), Arc::new(serde_json::from_slice(&raw).unwrap()));
    }
    d
}

#[test]
fn captures_do_not_change_which_traits_can_be_implanted_or_inherited() {
    let d = handbook();
    let ask = |q| {
        answer(&d, &crate::assistant_engine::field_text(q))
            .unwrap()
            .text
    };
    let chinese = ask("八云犬野捕能复制哪些词条？");
    assert!(chinese.contains("85 项") && chinese.contains("工匠精神") && !chinese.contains("传说"));
    let implant = ask("Which passive traits are outside the random pool but implantable?");
    assert!(
        implant.contains("12 passives")
            && implant.contains("Babysitter")
            && !implant.contains("Siren of the Void")
    );
    let offspring = ask("Do offspring inherit Lamball's partner skill when breeding?");
    assert!(offspring.contains("tied to the offspring's species"));
    let single = ask("Can Yakumo copy Legend on capture?");
    assert!(single.contains("excluded: Legend") && single.contains("cannot copy"));
}

#[test]
fn active_moves_need_a_real_level_relation_and_no_fruit() {
    let d = handbook();
    let result = answer(&d, "列出无果实但可遗传的主动招式").unwrap();
    let details = d.get("pal-details-data");
    for (_, url) in &result.links {
        let id = url.strip_prefix("#details/active/move/").unwrap();
        let row = &details["moves"][id];
        assert_eq!(row["inherit"], true);
        assert!(row["fruit"].is_null());
        assert!(d.species.iter().any(|p| {
            array(&details["profiles"][text(p, "key")]["levels"])
                .iter()
                .any(|r| r[1] == id)
        }));
    }
    assert_eq!(result.links.len(), 11);
}

#[test]
fn exact_growth_includes_counterexamples_and_distinct_parent_species() {
    let d = handbook();
    let ask = |q| {
        answer(&d, &crate::assistant_engine::field_text(q))
            .unwrap()
            .text
    };
    assert!(ask("Compare base stats of Fuack and Fuack Ignis").contains("Fuack Ignis"));
    let short = ask("阿努和绵悠悠的三围对比");
    assert!(short.contains("阿努比斯") && short.contains("棉悠悠"));
    let form = ask("普通和头目的奥沧鲸基础属性差多少？");
    assert!(form.contains("180/120/200") && form.contains("200/120/180"));
    let special = ask("What are the base stats of a Lucky Lamball?");
    assert!(
        special.contains("70/70/70")
            && special.contains("84/70/70")
            && special.contains("Work Speed +20%")
    );
}

#[test]
fn unsupported_conditions_do_not_turn_into_unfiltered_lists_or_stats() {
    let d = handbook();
    for q in [
        "Which passive traits can only be obtained through surgery?",
        "Which passive traits roll randomly or have implants?",
        "Which passive traits can Yakumo copy except Legend?",
        "List fire-type attacks that are inheritable without fruit",
        "Which passive traits improve attack but never roll randomly?",
        "How do I breed a Yakumo with Legend?",
        "Can an alpha that I breed pass a higher base attack to the offspring?",
        "Which gives more base stats, Souls or IVs?",
        "Which pal has the best base stats against Jormuntide?",
        "Do lucky pals have higher stamina or speed?",
    ] {
        assert!(
            answer(&d, &crate::assistant_engine::field_text(q)).is_none(),
            "{q}"
        );
    }
}

#[test]
fn effect_target_belongs_to_its_effect_not_to_the_entire_passive() {
    let d = handbook();
    let ask = |q| {
        answer(&d, &crate::assistant_engine::field_text(q))
            .unwrap()
            .text
    };
    let split = ask("Does Idiosyncratic affect the player or just the Pal?");
    assert!(split.contains("both the holder and the player: natural HP regeneration rate +50%"));
    assert!(split.contains("Pal carrying the trait: Defense +25%"));
    assert!(!split.contains("player: Defense"));
    let drop = ask("奉献精神会影响钓鱼概率吗？");
    assert!(drop.contains("自身的道具掉落量 +50%") && drop.contains("钓点独立奖励"));
    let building = ask("育婴师在据点内对谁生效？");
    assert!(building.contains("对应的据点设施") && building.contains("配种牧场产蛋速度 +30%"));
    let storage = ask("Does Vanguard affect the player from the Palbox?");
    assert!(
        storage.contains("player: Attack +10%") && storage.contains("does not establish an effect")
    );
    let guard = |q| passive_scope::answer(&d, &crate::assistant_engine::field_text(q)).is_none();
    assert!(guard(
        "Which traits affect fishing when breeding for Immortality?"
    ));
    assert!(guard("How do I get the Vanguard passive?"));
    assert!(guard("Does my Lucky Pal affect my fishing RNG?"));
    assert!(guard("Does Service-Minded stack with copies in my party?"));
}

#[test]
fn field_spawners_are_distinct_from_other_encounters_and_owned_pals() {
    let d = handbook();
    let ask = |q| wild_spawns::answer(&d, &crate::assistant_engine::field_text(q));
    let normal = ask("Will a wild Fuack come back after I catch it?").unwrap();
    assert!(normal.text.contains("new individuals") && normal.links[0].1 == "#spawns/BluePlatypus");
    let boss = ask("地图上的野外头目已击败打勾，还会再刷新吗？").unwrap();
    assert!(boss.text.contains("记录了挑战进度"));
    for q in [
        "Will my dead Pal respawn in the Palbox?",
        "Do chests respawn when wild Pals are caught?",
        "Can I respawn where the wild Pals killed me?",
        "Do wild Pals in the dungeon respawn before the boss?",
        "Will ore respawn where wild Pals spawn?",
        "Do summoned raid bosses respawn after being defeated?",
        "Where can I find wild Fuack?",
        "How do I make wild Pals respawn faster in the server settings?",
    ] {
        assert!(ask(q).is_none(), "{q}");
    }
}
