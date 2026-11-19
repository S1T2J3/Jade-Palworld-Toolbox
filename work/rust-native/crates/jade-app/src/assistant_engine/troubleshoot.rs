//! Narrow diagnostic requests after the ordinary handbook and calculation routes.
use super::AssistantReply;
use crate::data::{Data, array, text};

fn has(q: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| q.contains(phrase))
}

struct Note {
    en: bool,
    reply: AssistantReply,
}
impl Note {
    fn say(&mut self, en: &str, zh: &str) {
        if !self.reply.text.is_empty() {
            self.reply.text.push('\n');
        }
        self.reply.text.push_str(if self.en { en } else { zh });
    }
    fn link(&mut self, name: &str, route: &str) {
        self.reply.links.push((name.to_owned(), route.to_owned()));
    }
}

fn combat(d: &Data, q: &str, n: &mut Note) -> bool {
    if has(
        q,
        &[
            "fall damage",
            "falling damage",
            "坠落伤害",
            "摔落",
            "game freezes",
            "screen is frozen",
            "frozen screen",
        ],
    ) {
        return false;
    }
    let zero_number = ["0 damage", "0 dmg", "伤害为0", "伤害是0"]
        .iter()
        .any(|word| {
            q.match_indices(word).any(|(start, _)| {
                let invalid_neighbor = |c: char| c.is_ascii_digit() || c == '.';
                let after = &q[start + word.len()..];
                !q[..start].ends_with(invalid_neighbor)
                    && !after.starts_with(|c: char| c.is_ascii_digit())
                    && !after
                        .strip_prefix('.')
                        .is_some_and(|tail| tail.starts_with(|c: char| c.is_ascii_digit()))
            })
        });
    let zero = zero_number
        || has(
            q,
            &[
                "zero damage",
                "no damage",
                "zero dmg",
                "no dmg",
                "health bar never moves",
                "health bar won't move",
                "health bar isn't moving",
                "health bar does not move",
                "health bar doesn’t move",
                "不掉血",
                "不扣血",
                "零伤害",
                "没有伤害",
                "没伤害",
                "打不出伤害",
                "没扣血",
            ],
        );
    let symptoms = has(
        q,
        &[
            "not working",
            "doesn't",
            "doesn’t",
            "won't",
            "won’t",
            "does not",
            "never",
            "can't",
            "is it broken",
            "没挂上",
            "没挂",
            "挂不上",
            "不会中",
            "不触发",
            "没触发",
            "没生效",
            "不生效",
            "没有生效",
            "没打出",
            "不起效",
        ],
    );
    if !zero && !symptoms {
        return false;
    }
    let names = &[
        ("burn", &["burn", "点燃", "灼烧"] as &[&str]),
        ("poison", &["poison", "中毒", "毒伤"]),
        ("freeze", &["freeze", "freezing", "frozen", "冻结", "冰冻"]),
        ("shock", &["shocked", "paraly", "感电", "麻痹"]),
        ("wet", &["soaked", "wet effect", "浸湿", "潮湿"]),
    ];
    let status = names
        .iter()
        .find(|(_, terms)| has(q, terms))
        .and_then(|(id, _)| {
            array(&d.get("combat-guide-data")["status"])
                .iter()
                .find(|row| row["id"] == *id)
        });
    let applying = has(
        q,
        &[
            "attack",
            "target",
            "move",
            "hit",
            "proc",
            "inflict",
            "apply",
            "effect",
            "status",
            "pal won't burn",
            "敌人",
            "目标",
            "异常",
            "攻击",
            "招式",
            "效果",
            "点燃不触发",
            "挂上",
            "触发",
        ],
    );
    if !zero && (status.is_none() || !applying) {
        return false;
    }
    if let Some(row) = status {
        let id = text(row, "id");
        let en = match id {
            "burn" => "Burn deals damage over time. Fire and Water Pals are immune to Burn.",
            "poison" => {
                "Poison drains HP over time. Check the HP changes between hits separately from the impact damage of your next attack."
            }
            "freeze" => {
                "Freeze temporarily stops the target from acting. Look for the target stopping, separately from the damage shown for the hit."
            }
            "shock" => {
                "Shock can temporarily stop the target; Water attacks deal more damage during the effect."
            }
            "wet" => {
                "Wet slows movement. While it is active, Electric attacks deal more damage and Fire attacks deal less."
            }
            _ => return false,
        };
        n.say(en, text(row, "text"));
        n.say("First separate a missed hit from a hit that dealt damage but did not inflict the status. Check that the move can apply this status. Try the same move on another ordinary target, and tell me the move and target if the effect still differs.",
            "先分清是整次攻击未命中，还是命中了、扣血了，却没有附上异常。核对招式能否造成这个异常，用同一招换一个普通目标对照。还不同的话，告诉我具体招式和目标名称。");
        n.link(text(row, "name"), &format!("#guide/status/{id}"));
        return true;
    }
    n.say("Who attacks whom: your character or a Pal, with which weapon or move, and against which target? Check a target's HP bar as well as the floating number. With the same attack, try a normal wild Pal near your level, then try another attack on the original target.",
        "实际是谁在攻击、用的什么武器或招式、目标是谁？同时看目标血条和跳出的数字。先用同一攻击打与自己等级接近的普通野生帕鲁，再换一种攻击打原目标。");
    n.say("If only the original target takes zero, report its name and the two levels. If every target takes zero, compare a player attack with a Pal attack. In a custom world, the host can read the separate player and Pal damage multipliers before changing anything.",
        "若只有原目标不掉血，请提供目标名称和双方等级。若对所有目标都无伤，再对比玩家攻击和帕鲁攻击；自定义世界可请房主先查看玩家伤害与帕鲁伤害的独立倍率。");
    true
}

fn base(q: &str, n: &mut Note) -> bool {
    // A worker need not mention a facility by name. Keep connection issues,
    // party combat and personal inventory changes out of the work diagnostics.
    let local = q
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| matches!(word, "base" | "workers" | "worker" | "ranch"))
        || has(
            q,
            &[
                "基地",
                "据点",
                "工人",
                "工作栏",
                "牧场",
                "农场",
                "the farm",
                "my farm",
                "breeding farm",
                "生产线",
                "workstation",
                "work station",
                "派工",
                "assign a pal",
                "assign my pal",
                "assign pals",
                "assign this pal",
                "帕鲁都不",
                "帕鲁不肯",
                "搬运工",
                "hauler",
                "transporter",
            ],
        );
    let server = has(q, &["server", "专服", "服务器", "联机"]);
    let pal = has(q, &["pal ", "pals ", "帕鲁", "伙伴", "worker", "工人"]);
    let offline = has(
        q,
        &[
            "offline",
            "logged off",
            "logged out",
            "log off",
            "log out",
            "no players",
            "nobody online",
            "nobody is online",
            "overnight",
            "unattended",
            "离线",
            "没人在线",
            "无人在线",
            "下线",
            "一晚上",
            "过夜",
        ],
    );
    let illness = has(
        q,
        &[
            " sick",
            " ill ",
            " ill?",
            "illness",
            "disease",
            "depress",
            "starving",
            "starved",
            "malnourish",
            "生病",
            "患病",
            "得病",
            "病了",
            "抑郁",
            "饿坏",
            "饿病",
            "饥饿",
            "营养不良",
        ],
    );
    if illness && (local || server && pal) && offline {
        n.say("Before leaving, inspect each worker's food and SAN, stock a reachable Feed Box, leave a bed for each worker and a clear path to food, beds and the hot spring. Let one work/rest cycle run while you are watching. If only one worker cannot eat or rest, test it on clear ground.",
            "下线前先看各工人的饱食和 SAN，给可到达的饲料箱备足食物，按工人数留床位，并清出通往床、食物和温泉的路径。先在线观察一轮进食和休息；若只有某只吃不到或不休息，把它放到开阔处对照。");
        n.say("Record food stock and the workers' condition before logging out and after returning. If illness appears only while nobody is online, tell me the hosting mode and the affected conditions. A short test with vulnerable workers stored in the Palbox can separate base work from other causes. Use the medicine specified for an existing illness.",
            "记下离线前后剩余食物和异常状态。若仅无人在线时恶化，请说明主机类型和具体病名。可先把易患病的工人短时收回终端做对照；已经生病则按病症用药。");
        n.link("饲料箱 / Feed Box", "#work/facilities/Feed_Box");
        n.link("温泉 / Hot Spring", "#work/facilities/Hot_Spring");
        return true;
    }
    if !local {
        return false;
    }
    let hauling = has(
        q,
        &[
            "haul",
            "transport",
            "carry items",
            "carry the",
            "collect the",
            "pick up",
            "搬运",
            "搬走",
            "不搬",
            "不捡",
            "不收货",
            "不收东西",
            "没人捡",
        ],
    );
    let stalled = has(
        q,
        &[
            "won't",
            "won’t",
            "doesn't",
            "doesn’t",
            "don't",
            "don’t",
            "never",
            "not being",
            "refuse",
            "left on the",
            "pile up",
            "piling up",
            "不搬",
            "没搬",
            "不捡",
            "没人",
            "不肯",
            "一直堆",
            "不收",
        ],
    );
    if hauling && stalled {
        n.say("First check where the output is: on the ground, still in the producing facility, or already in another container. Give a base Pal with Transporting permission to haul, and check that it is free from another fixed assignment. Put a reachable container with a free slot beside the output and allow this item's category in the container settings.",
            "先看成品还在生产设施里、掉在地上，还是已经被送去了别的容器。找一只有搬运适性的据点工人，在监控台允许它搬运并检查固定派工。在产出点旁放一个可到达、有空位的箱子，确认箱子允许该类物品。");
        n.say("Watch one item from the same output. If the nearby test succeeds, check the normal destination and route. For edible produce, also check the Feed Box and its food filters. Which item is left, and where exactly is it sitting?",
            "观察同一来源的一件成品。若近处能搬，回查原目的地和路径；可吃的产物还要查看饲料箱及其食物筛选。留下的具体是什么，停在地上还是设施里？");
        n.link(
            "监控台 / Monitoring Stand",
            "#work/facilities/Monitoring_Stand",
        );
        return true;
    }
    let power = has(
        q,
        &[
            "electric",
            "no power",
            "power supply",
            "power cuts",
            "power goes",
            "power grid",
            "powered",
            "generator",
            "发电",
            "缺电",
            "断电",
            "电器",
            "没电",
            "停电",
        ],
    );
    let failed = has(
        q,
        &[
            "stops",
            "stopped",
            "keeps cutting",
            "not getting",
            "no power",
            "won't",
            "not working",
            "can't",
            "cannot",
            "stalled",
            "not sure",
            "无电",
            "没反应",
            "不开工",
            "停工",
            "全停",
            "缺电",
            "没电",
            "断电",
            "停电",
        ],
    );
    if power && failed {
        n.say("Check the generator and one other powered facility in the same base. Is a worker with Generating Electricity actually working, and does the power gauge change? Temporarily stop other consumers and watch one machine with a valid job.",
            "先看同一据点的发电机和另一台用电设备：有发电适性的工人正在发电吗，电量是否变化？临时停用其他耗电设施，只留一台已有有效工单的机器观察。");
        n.say("If several devices stop together as the gauge empties, check generation versus demand. If the other device keeps working, inspect this station's order, required work type, materials and output space. Which case do you see?",
            "如果电量见底时多台一起停，先查供电能否跟上消耗。如果另一台仍工作，再查这台的工单、工种、原料和成品空间。现场属于哪一种？");
        n.link(
            "发电机 / Power Generator",
            "#work/facilities/Power_Generator",
        );
        return true;
    }
    if has(
        q,
        &[
            "worker slot",
            "assignment",
            "assign",
            "worker list",
            "派工",
            "分配",
            "工位",
            "工作栏",
        ],
    ) && has(
        q,
        &[
            "grey",
            "gray",
            "can't tell",
            "no eligible",
            "no suitable",
            "cannot be assigned",
            "cannot assign",
            "can't assign",
            "can’t assign",
            "unable to assign",
            "not able to assign",
            "won't let",
            "won’t let",
            "can't be assigned",
            "can’t be assigned",
            "fails to assign",
            "派不了",
            "派不上",
            "没法派工",
            "分配不了",
            "指派不了",
            "灰色",
            "是灰的",
            "点不动",
            "没人可选",
            "没有合适",
            "无法分配",
        ],
    ) {
        n.say("Which facility is it, and is the Pal greyed out in the assignment list or does it accept the job and then fail to reach it? First put the Pal in that base's worker slots. Check the station is in the same base and the job is active if it needs an order. Check its required suitability and the worker's permissions or existing fixed assignment at the Monitoring Stand.",
            "是哪座设施？是派工名单里工人已经变灰，还是分配成功后走不到？先确认帕鲁已在这个据点的工作栏，设施在同一基地，需要工单的工位已经下单。核对工种适性及监控台的工种许可和原有固定派工。");
        n.say("If the assignment succeeds, compare the same station in a clear spot on the same floor using a suitable worker. A successful open-floor test points to the route; a failure in both spots calls for rechecking the job or facility.",
            "若已分配成功，在同层开阔处用有适性的工人试同一工位。开阔处能工作，再查通道；两处都不行，则回头查岗位或设施状态。");
        n.link(
            "监控台 / Monitoring Stand",
            "#work/facilities/Monitoring_Stand",
        );
        return true;
    }
    false
}

fn reward(q: &str, n: &mut Note) -> bool {
    if has(
        q,
        &[
            "frame drop",
            "fps drop",
            "掉帧",
            "掉线",
            "supply drop landing",
            "meteorite/supply drop",
        ],
    ) {
        return false;
    }
    let failed = has(
        q,
        &[
            "didn't get",
            "didn’t get",
            "did not get",
            "not received",
            "can't claim",
            "cannot collect",
            "nothing dropped",
            "never drops",
            "didn't drop",
            "doesn't drop",
            "haven't got",
            "got no",
            "can't find",
            "没收到",
            "没拿到",
            "没领到",
            "没有领到",
            "不能领取",
            "无法领取",
            "领取不了",
            "没掉",
            "不掉",
            "不出",
            "一直没有",
        ],
    );
    let rates = has(
        q,
        &["drop rate", "droprate", "drop chance", "掉率", "掉落概率"],
    );
    let quest = has(q, &["quest", "mission reward", "任务", "任务奖励"]);
    if failed && quest {
        n.say("What is the quest name and the exact objective or prompt currently shown? Distinguish an objective marked complete from a reward already granted. Reopen the quest page and check whether it directs you to a person, an object or another step. Check the inventory for the named item before repeating the fight.",
            "任务名称和当前屏幕上的目标提示是什么？先分清是某一项目标完成，还是奖励已发放。重开任务页，看看是否还要求与人物或物体交互、或继续下一步；核对背包里的奖励名称后再决定要不要重打。");
        return true;
    }
    if (failed || rates)
        && has(
            q,
            &[
                "loot",
                "drop",
                "schematic",
                "blueprint",
                "boss",
                "掉落",
                "图纸",
                "设计图",
                "战利品",
                "宝箱",
            ],
        )
    {
        n.say("Which exact item or schematic tier are you after, and what did you defeat or open? Boss variants, difficulty, reward chests and first-clear rewards can use different tables. Check the entry for that exact source and whether its reward is guaranteed or just a chance. Several misses alone do not show a bug.",
            "你要的是哪件物品或哪一档设计图，实际击败了谁或开了什么箱子？普通个体、不同头目/难度、奖励箱和首通奖励可能分属不同的表。先对准实际来源，确认是必得还是概率奖励；连续几次没出本身不能判定为故障。");
        return true;
    }
    false
}

fn setting(q: &str, n: &mut Note) -> bool {
    let option = has(
        q,
        &[
            "setting",
            "option",
            "slider",
            "multiplier",
            "设置",
            "选项",
            "倍率",
        ],
    );
    let issue = has(
        q,
        &[
            "grey",
            "gray",
            "locked",
            "not applied",
            "doesn't apply",
            "doesn't save",
            "didn't change",
            "not taking effect",
            "disabled for me",
            "灰色",
            "是灰的",
            "点不了",
            "改不了",
            "没有生效",
            "没生效",
            "看不到",
            "不能修改",
        ],
    );
    let session = has(
        q,
        &[
            "server",
            "world",
            "solo",
            "coop",
            "co-op",
            "host",
            "multiplayer",
            "服务器",
            "专服",
            "多人",
            "朋友",
            "单人",
            "房主",
            "世界",
        ],
    );
    if !(option && issue && session) {
        return false;
    }
    n.say("Which setting is it, and are you the world host, an invited player, or using a dedicated server? Compare the same option on the host in that same world. For a dedicated server ask its administrator to check the effective configuration and whether the changed instance was restarted. Record the old value before testing one change.",
        "具体是哪项设置？你是这个世界的房主、受邀加入的玩家，还是在专服？先让房主在同一个世界对照这一选项。专服则请管理员查看生效的实例配置、修改后是否重启了对应实例。记录原值，每次只改一项做对照。");
    n.link(
        "Pocketpair dedicated server settings",
        "https://docs.palworldgame.com/settings-and-operation/configuration/",
    );
    true
}

fn note(q: &str) -> Option<Note> {
    if has(
        q,
        &[
            "in minecraft",
            "in terraria",
            "my computer",
            "windows task",
            "real life",
            "in real life",
            "现实中",
            "现实生活",
        ],
    ) {
        return None;
    }
    Some(Note {
        en: !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)),
        reply: AssistantReply {
            text: String::new(),
            links: vec![],
            suggestions: vec![],
        },
    })
}
pub(super) fn combat_report(d: &Data, q: &str) -> Option<AssistantReply> {
    let mut n = note(q)?;
    combat(d, q, &mut n).then_some(n.reply)
}
pub(super) fn answer(q: &str) -> Option<AssistantReply> {
    let mut n = note(q)?;
    (setting(q, &mut n) || reward(q, &mut n)).then_some(n.reply)
}

pub(super) fn work_report(q: &str) -> Option<AssistantReply> {
    let mut n = note(q)?;
    base(q, &mut n).then_some(n.reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{Conversation, field_text};
    use serde_json::Value;
    use std::{path::PathBuf, sync::Arc};
    #[test]
    fn diagnostic_branches_require_symptoms_and_preserve_regular_queries() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["combat-guide-data", "facilities-data", "item-catalog-data"] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            d.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        for (q, needle) in [
            (
                "招式命中了，但是怎么都没挂上点燃。",
                "火、水属性帕鲁免疫点燃",
            ),
            ("Why do all my attacks deal zero damage?", "normal wild Pal"),
            ("得到了任务完成提示，可是没领到任务奖励。", "目标提示"),
            (
                "The worker slot at my base is grey. How do I check it?",
                "assignment list",
            ),
            ("我单人能改，跟朋友玩世界设置就点不了。", "房主"),
            (
                "I'm unable to assign my Pal, the list keeps closing.",
                "base's worker slots",
            ),
            ("農場收成一直堆在地上，帕魯不肯搬運，怎麼檢查？", "箱子允许"),
            (
                "I found my workers starving after I logged out overnight.",
                "Feed Box",
            ),
            ("下线回来据点工人就病了，饲料箱还有东西。", "离线前后"),
        ] {
            let mut conv = Conversation::default();
            let a = conv.ask_world(q, &d, &Value::Null);
            assert!(
                conv.direct_result() && a.text.contains(needle),
                "{q}: {}",
                a.text
            );
        }
        for q in [
            "which element does more damage?",
            "普通采矿场建造材料是什么",
            "how do i equip a poison attack?",
            "my computer drops wi-fi when idle",
            "gold drop price in real life",
            "which work skill does a mill need?",
            "my attack did 120 damage, which skill is next?",
            "what if fall damage is zero damage?",
            "不能领物品时我的屏幕是冻结的",
            "what is the base power of a move that cannot poison?",
            "I'm farming a wild Pal while my character won't pick up loot",
            "can a healthy Pal in my party transport me to another island?",
            "I can't assign a key to my controller",
            "my dedicated server has no players online, how do I set up a password?",
        ] {
            let s = field_text(q);
            assert!(
                answer(&s).is_none()
                    && combat_report(&d, &s).is_none()
                    && work_report(&s).is_none(),
                "{q}"
            );
        }
    }
}
