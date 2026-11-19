//! Site rows describe where an encounter can be generated, not individual save entities.
//! Source: Awy64/palworld-atlas-data, MIT, build 25247047, listed in spawn-data.json.
//! Alpha and Sealed Realm distinction: https://palworld.wiki.gg/wiki/Alpha_Pals
use super::*;

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let returns = has(
        q,
        &[
            "respawn",
            "re-spawn",
            "reappear",
            "come back",
            "spawn again",
            "spawn more",
            "repopulate",
            "extinct",
            "gone for good",
            "gone forever",
            "one-time spawn",
            "another spawn",
            "刷新",
            "重生",
            "再刷",
            "重新出现",
            "再出现",
            "还会出现",
            "还会有",
            "绝种",
            "灭绝",
            "抓光",
            "抓完",
            "打光",
        ],
    );
    if !returns {
        return None;
    }
    let (mentions, _) = species_mentions(&d.species, q);
    let wild = has(
        q,
        &[
            "wild pal",
            "wild animal",
            "wild creature",
            "regular pal",
            "normal pal",
            "wild encounter",
            "overworld pal",
            "野生帕鲁",
            "普通野怪",
            "野外普通",
            "野外帕鲁",
        ],
    ) || !mentions.is_empty()
        && (q.contains("野生")
            || q.split(|c: char| !c.is_ascii_alphabetic())
                .any(|w| w == "wild"));
    let encounter = has(
        q,
        &[
            "caught",
            "catch every",
            "capture every",
            "captured",
            "after capturing",
            "defeated",
            "killed",
            "knocked out",
            "抓完",
            "捕完",
            "抓光",
            "打光",
            "打死",
            "击倒",
            "打败",
            "捕获",
        ],
    );
    let marker = has(
        q,
        &[
            "checkmark",
            "check mark",
            "ticked",
            "marked defeated",
            "marked as defeated",
            "says defeated",
            "图标",
            "标记",
            "打勾",
            "打钩",
            "已击败",
            "已讨伐",
            "已击倒",
        ],
    );
    let boss =
        has(
            q,
            &[
                "field boss",
                "overworld boss",
                "overworld alpha",
                "field alpha",
                "map boss",
                "野外头目",
                "野外首领",
                "地图头目",
                "地图首领",
                "固定头目",
            ],
        ) || marker && has(q, &["map", "地图"]) && has(q, &["boss", "alpha", "头目", "首领"]);
    if !wild && !encounter && !(boss && marker) {
        return None;
    }
    // These each have their own encounter, regeneration or save-state rules.
    if has(
        q,
        &[
            "chest",
            "treasure",
            "dungeon",
            "sealed realm",
            "cave boss",
            "tower",
            "summon",
            "raid",
            "predator",
            "bounty",
            "resource node",
            "resource respawn",
            "fruit tree",
            "loot",
            "drop rate",
            "egg",
            "lucky",
            "shiny",
            "respawn point",
            "spawn point for me",
            "when i respawn",
            "my character",
            "player respawn",
            "respawn location",
            "my palbox",
            "in the palbox",
            "in storage",
            "my party",
            "my own pal",
            "my dead",
            "my pals",
            "revive",
            "settings",
            "randomizer",
            "server config",
            "建造阻止",
            "基地抑制",
            "设置",
            "随机模式",
            "宝箱",
            "箱子",
            "物资",
            "掉落",
            "地下城",
            "地牢",
            "封印",
            "高塔",
            "唤起",
            "召唤",
            "团体战",
            "赏金",
            "凶祸",
            "矿石",
            "矿点",
            "资源",
            "技能果",
            "闪光",
            "头目蛋",
            "孵蛋",
            "传送点",
            "复活点",
            "玩家复活",
            "我死",
            "我自己",
            "队伍",
            "仓库",
            "帕鲁终端",
            "我养的",
            "我自己的",
            "濒死复活",
        ],
    ) || has(
        q,
        &[
            "i respawn",
            "can i respawn",
            "killed me",
            "kill me",
            "do i respawn",
            "玩家重生",
            "我会重生",
            "palbox",
            "pal box",
            "where exactly",
            "具体坐标",
            "hardcore",
            "硬核",
        ],
    ) || q
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| w == "ore")
    {
        return None;
    }
    let en = english(q);
    if mentions.len() > 1 {
        return None;
    }
    // Generic repeated spawns must explicitly name wild Pals, not just an action.
    if mentions.is_empty() && !wild && !(boss && marker) {
        return None;
    }
    let data = d.get("spawn-data");
    if data["bySpecies"].as_object()?.is_empty() {
        return None;
    }
    let mut links = Vec::new();
    let mut line = String::new();
    let named = if let [(i, _, _)] = mentions.as_slice() {
        let pal = &d.species[*i];
        let key = text(pal, "key");
        let records = data["bySpecies"][key].as_array()?;
        let ordinary = records.iter().any(|r| r[4] == 0);
        let alpha = records.iter().any(|r| r[4] == 1);
        if !(if boss { alpha } else { ordinary || alpha }) {
            return None;
        }
        let name = text(pal, if en { "en" } else { "zh" });
        links.push((name.into(), format!("#spawns/{key}")));
        if en {
            line.push_str(&format!("{name}: "));
        } else {
            line.push_str(&format!("{name}："));
        }
        Some((ordinary, alpha))
    } else {
        None
    };
    let field = boss || named == Some((false, true));
    // Existing mechanics covers ordinary field/realm boss timer questions.
    if field && boss && !marker && !wild {
        return None;
    }
    if field {
        line.push_str(if en {"A fixed overworld Alpha can appear again at its marked site after a defeat or capture. A defeated checkmark records that encounter's completion. It does not remove the spawn site. Revisit after the in-game day changes."}
            else{"固定点野外头目在被击败或捕获后仍可重新出现。「已击败」勾选记录了挑战进度，该刷新点依然存在。可在游戏内换日后回到标记处查看。"});
    } else {
        if !wild && (!encounter || named.is_none()) {
            return None;
        }
        line.push_str(if en {"Yes. Capturing or defeating ordinary wild Pals does not clear the species from its habitat. The area can generate new individuals from its encounter pool. Leave the area and check the habitat again, including the appropriate time of day. The spawn table gives no universal wait in minutes and a particular individual is not guaranteed to return."}
            else{"会。抓走或击倒一群普通野生帕鲁后，栖息地仍能从当地遭遇池生成新个体。可以先离开这片区域，再在相应昼夜时段回来看。刷新表没有给普通个体统一的等待分钟数，也不保证再出现的就是之前那一只。"});
    }
    if links.is_empty() {
        links.push((
            if en {
                "Wild spawn map"
            } else {
                "野生分布图"
            }
            .into(),
            "#spawns".into(),
        ));
    }
    Some(answer_lines(vec![line], links))
}
