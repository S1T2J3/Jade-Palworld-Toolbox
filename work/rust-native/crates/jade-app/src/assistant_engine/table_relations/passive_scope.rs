//! Read the activation column per passive and the recipient column per effect.
use super::*;
use std::sync::OnceLock;

fn records() -> &'static Value {
    static BOOK: OnceLock<Value> = OnceLock::new();
    BOOK.get_or_init(|| {
        serde_json::from_str(include_str!("passive-effects.json"))
            .expect("validated passive effects")
    })
}

fn words(en: bool, en_text: &'static str, zh_text: &'static str) -> &'static str {
    if en { en_text } else { zh_text }
}

fn recipient(code: &str, en: bool) -> Option<&'static str> {
    Some(match code {
        "ToSelf" => words(en, "Pal carrying the trait", "持有词条的帕鲁"),
        "ToTrainer" => words(en, "player", "玩家"),
        "ToSelfAndTrainer" => words(en, "both the holder and the player", "持有者和玩家"),
        "ToBuildObject" => words(en, "relevant base facility", "对应的据点设施"),
        _ => return None,
    })
}

fn activation(code: &str, en: bool) -> Option<&'static str> {
    Some(match code {
        "always" => words(
            en,
            "No deployment or riding condition is set for this trait.",
            "该词条没有出战或骑乘触发条件。",
        ),
        "otomo" => words(
            en,
            "The holder must be in the party; this flag does not require deployment.",
            "持有者须在队伍中，该标记不要求放出来。",
        ),
        "activeOtomo" => words(
            en,
            "The holder must be the active party Pal. For the stamina effects the Pal must also be rideable.",
            "持有者须为当前出战的队伍帕鲁，这类耐力加成还要求帕鲁可骑乘。",
        ),
        "baseCamp" => words(
            en,
            "The holder must be assigned to the base.",
            "持有者须在据点内。",
        ),
        "riding" => words(
            en,
            "The effect activates while riding the holder.",
            "骑乘持有者时生效。",
        ),
        _ => return None,
    })
}

fn effect(code: &str, value: i64, en: bool) -> Option<String> {
    let element = |s| {
        Some(match s {
            "Normal" => words(en, "Neutral", "无"),
            "Fire" => words(en, "Fire", "火"),
            "Water" => words(en, "Water", "水"),
            "Electricity" => words(en, "Electric", "雷"),
            "Leaf" => words(en, "Grass", "草"),
            "Ice" => words(en, "Ice", "冰"),
            "Earth" => words(en, "Ground", "地"),
            "Dark" => words(en, "Dark", "暗"),
            "Dragon" => words(en, "Dragon", "龙"),
            _ => return None,
        })
    };
    if let Some(s) = code.strip_prefix("ElementBoost_") {
        return Some(format!(
            "{}{} {value:+}%",
            element(s)?,
            words(en, " attack damage", "属性攻击伤害")
        ));
    }
    if let Some(s) = code.strip_prefix("ElementResist_") {
        return Some(format!(
            "{}{} {:+}%",
            element(s)?,
            words(en, " damage received", "属性受到的伤害"),
            -value
        ));
    }
    let flag = match code {
        "LeanBackInvalid_ForPassiveSkill" => Some(words(en, "immune to flinch", "免疫硬直")),
        "KnockbackInvalid_ForPassiveSkill" => Some(words(en, "immune to knockback", "免疫击退")),
        "NonKilling" => Some(words(
            en,
            "its hit leaves the target with at least 1 HP",
            "攻击使目标至少保留 1 点生命",
        )),
        "Nocturnal" => Some(words(
            en,
            "can stay awake and work at night",
            "夜间可保持清醒并工作",
        )),
        "NightOwl" => Some(words(en, "tends to nap during the day", "白天容易打盹")),
        "WorldTreeDecayImmunity" => Some(words(
            en,
            "approaching World Tree resources does not make them vanish",
            "靠近世界树资源时可防止资源消失",
        )),
        _ => None,
    };
    if let Some(flag) = flag {
        return Some(flag.into());
    }
    let caption = match code {
        "CraftSpeed" => words(en, "work speed", "工作速度"),
        "Defense" => words(en, "Defense", "防御"),
        "ShotAttack" => words(en, "Attack", "攻击"),
        "MoveSpeed" => words(en, "movement speed", "移动速度"),
        "Mining" => words(en, "mining efficiency", "采矿效率"),
        "Logging" => words(en, "logging efficiency", "伐木效率"),
        "FullStomatch_Decrease" => words(en, "hunger depletion rate", "饱食度消耗速度"),
        "Sanity_Decrease" => words(en, "SAN depletion rate", "SAN 消耗速度"),
        "BreedSpeed" => words(
            en,
            "egg production speed when this Pal breeds in a Breeding Farm",
            "该帕鲁在配种牧场配种时的产蛋速度",
        ),
        "ActiveSkillCoolTime_Decrease" => {
            words(en, "active skill cooldown duration", "主动技能冷却时长")
        }
        "PalSP_Increase" => words(en, "rideable Pal maximum stamina", "可骑乘帕鲁的最大耐力"),
        "ShopSellPrice_Money_Increase" => {
            words(en, "shop selling price modifier", "商店卖出价修正")
        }
        "ShopBuyPrice_Money_Increase" => words(en, "shop buying price modifier", "商店买入价修正"),
        "LifeSteal" => words(
            en,
            "HP recovered as a share of damage dealt by this Pal",
            "该帕鲁以自身造成伤害恢复生命的比例",
        ),
        "SwimSpeed" => words(en, "swimming speed", "水中移动速度"),
        "MaxHP" => words(en, "maximum HP", "最大生命"),
        "PalEggHatchingSpeed" => words(en, "egg incubation speed", "帕鲁蛋孵化速度"),
        "BreedSpeed_InBaseCamp" => {
            words(en, "Breeding Farm egg production speed", "配种牧场产蛋速度")
        }
        "AutoHPRegeneRate" => words(en, "natural HP regeneration rate", "生命值自然恢复速度"),
        "ResistAdditionalEffect_Burn" => words(en, "resistance to burn status", "对灼烧状态的抗性"),
        "ResistAdditionalEffect_Poison" => {
            words(en, "resistance to poison status", "对中毒状态的抗性")
        }
        "SelfDeathAddItemDrop" => words(
            en,
            "quantity of items dropped by this Pal itself",
            "这只帕鲁自身的道具掉落量",
        ),
        "WorkSuitabilityAddRank_MonsterFarm" => {
            words(en, "Ranching suitability level", "牧场工作适性等级")
        }
        "ExplosionResist" => words(en, "resistance to explosion damage", "爆炸伤害抗性"),
        "RideJumpCount_Increase" => words(en, "mounted jump count", "骑乘跳跃次数"),
        "PlayerSP_DecreaseRate" => words(en, "stamina consumption rate", "耐力消耗速度"),
        "ReloadSpeedUp" => words(en, "reload speed", "装填速度"),
        _ => return None,
    };
    let suffix = if matches!(
        code,
        "RideJumpCount_Increase" | "WorkSuitabilityAddRank_MonsterFarm"
    ) {
        ""
    } else {
        "%"
    };
    let shown = if code == "ActiveSkillCoolTime_Decrease" {
        -value
    } else {
        value
    };
    Some(format!("{caption} {shown:+}{suffix}"))
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let scope = has(
        q,
        &[
            "给谁",
            "对谁",
            "影响谁",
            "作用在谁",
            "谁获得",
            "谁生效",
            "对象",
            "作用于",
            "给我",
            "给玩家",
            "给帕鲁",
            "对玩家",
            "全队",
            "其他帕鲁",
            "同伴",
            "玩家也",
            "什么时候",
            "何时",
            "触发",
            "放出",
            "出战",
            "在队",
            "背包",
            "终端",
            "在箱",
            "仓库",
            "骑着",
            "骑乘时",
            "生效",
            "有效",
            "影响",
            "钓鱼",
            "打捞",
            "能叠",
            "什么时候",
            "apply to",
            "applies to",
            "affect",
            "who gets",
            "who receive",
            "recipient",
            "does it give",
            "give me",
            "buff me",
            "buff my",
            "buff the",
            "give the player",
            "on the player",
            "my own",
            "the trainer",
            "my pal or",
            "the pal or",
            "in my party",
            "in the party",
            "in your party",
            "deployed",
            "summoned",
            "palbox",
            "in storage",
            "while riding",
            "while mounted",
            "while fishing",
            "work with",
            "work for",
            "work on",
            "need to ride",
            "when does",
            "activate",
            "trigger",
        ],
    );
    if !scope
        || has(
            q,
            &[
                "inherit",
                "breed down",
                "which passives",
                "which traits",
                "what passives",
                "what traits",
                "recommend",
                "compare dps",
                "how to get",
                "where can",
                "how many eggs",
                "how do i get",
                "roll randomly",
                "遗传",
                "随机刷",
                "怎样获得",
                "怎么获得",
                "从哪",
                "有哪些词条",
                "哪些被动",
                "推荐",
                "词条搭配",
                "转移",
                "冷却几秒",
                "冷却多久",
                "空间坐标",
            ],
        )
    {
        return None;
    }
    let ids = super::super::skill_query::mentioned_traits(d, q);
    if ids.is_empty() || ids.len() > 4 {
        return None;
    }
    let explicit = has(q, &["passive", "trait", "perk", "词条", "被动"]);
    if !explicit
        && (has(
            q,
            &[
                "lucky pal",
                "lucky pals",
                "传奇帕鲁",
                "幸运帕鲁",
                "闪光帕鲁",
            ],
        ) || ids.iter().any(|id| {
            matches!(
                *id,
                "Legend" | "Rare" | "PAL_oraora" | "PAL_ALLAttack_down1" | "CraftSpeed_up1"
            )
        }))
    {
        return None;
    }
    if has(
        q,
        &[
            "stack",
            "叠加",
            "叠层",
            "几层",
            "能叠",
            "合计",
            "合起来",
            "总共",
            "总计",
            "百分点",
            "相差",
            "差多少",
            "哪只更",
            "谁快",
            "谁跑得快",
            "谁更快",
            "总加成",
            "一只带",
            "乙带",
            "第二只带",
            "total ",
            "combined ",
            "altogether",
            "percentage points",
            "which has higher",
            "the other has",
            "faster than",
            "difference in",
        ],
    ) {
        return None;
    }
    let en = english(q);
    let records = records().get("records")?;
    let book = &d.get("pal-details-data")["passives"];
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut has_self_drop = false;
    let mut always = false;
    for id in ids {
        let row = records.get(id)?;
        let flag = row["activation"].as_str()?;
        let name = book[id][if en { "en" } else { "zh" }].as_str()?;
        lines.push(format!("{name} — {}", activation(flag, en)?));
        for entry in row["effects"].as_array()? {
            let code = entry[0].as_str()?;
            let label = effect(code, entry[1].as_i64()?, en)?;
            let target = recipient(entry[2].as_str()?, en)?;
            lines.push(if en {
                format!("• {target}: {label}.")
            } else {
                format!("• {target}：{label}。")
            });
            has_self_drop |= code == "SelfDeathAddItemDrop";
        }
        always |= flag == "always";
        links.push((name.into(), format!("#details/passive/trait/{id}")));
    }
    if always
        && has(
            q,
            &[
                "palbox",
                "pal box",
                "storage",
                "warehouse",
                "box ",
                "仓库",
                "帕鲁箱",
                "终端",
                "在箱",
            ],
        )
    {
        lines.push(
            words(
                en,
                "The activation flag does not establish an effect from a Pal stored in the Palbox.",
                "这条触发记录无法证明放在终端仓库的帕鲁仍然提供加成。",
            )
            .into(),
        );
    }
    if has_self_drop && has(q, &["fishing", "fish ", "salvage", "钓", "打捞"]) {
        lines.push(words(en,"This modifier belongs to the Pal that drops the items. Carrying it as a helper does not give a general bonus to fishing odds or other loot pools. If a caught Pal itself has the trait, distinguish its own drops from the fishing spot's rewards; this table does not establish how every fishing reward rolls.","加成取决于掉东西的那只帕鲁。放在队伍里作助手无法给钓鱼概率或其他战利品池提供通用加成。若钓到的目标自己带有词条，需要区分它自身的掉落和钓点独立奖励；表中没有逐个钓鱼奖励池的结算记录。").into());
    } else if has_self_drop
        && has(
            q,
            &[
                "chest",
                "ranch",
                "drop chance",
                "drop rate",
                "宝箱",
                "牧场",
                "掉率",
                "掉落率",
            ],
        )
    {
        lines.push(words(en,"The quantity modifier applies to drops from the holder itself. This record does not raise chest odds, Ranch production, or drops from other targets.","该词条增加持有者自身的道具掉落量。这条记录不增加宝箱概率、牧场产量或其他目标的掉落。").into());
    }
    Some(answer_lines(lines, links))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_exported_columns_render_in_both_languages() {
        let rows = records()["records"].as_object().unwrap();
        assert_eq!(rows.len(), 115);
        let mut count = 0;
        for row in rows.values() {
            for en in [false, true] {
                assert!(activation(row["activation"].as_str().unwrap(), en).is_some());
            }
            for e in row["effects"].as_array().unwrap() {
                count += 1;
                for en in [false, true] {
                    assert!(recipient(e[2].as_str().unwrap(), en).is_some());
                    assert!(
                        effect(e[0].as_str().unwrap(), e[1].as_i64().unwrap(), en).is_some(),
                        "{e}"
                    );
                }
            }
        }
        assert_eq!(count, 167);
    }
}
