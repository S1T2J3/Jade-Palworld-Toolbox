use super::AssistantReply;

pub(super) fn answer(q: &str) -> Option<AssistantReply> {
    let has = |words: &[&str]| words.iter().any(|word| q.contains(word));
    let owned = has(&[
        "帕鲁",
        "帕魯",
        "队友帕",
        "palbox",
        "my pal",
        "our pal",
        "a pal ",
        "a pal?",
        "the pal ",
        "pals ",
        "pals?",
        "pal revive",
        "pal revival",
        "pal revive time",
        "pal recovery time",
    ]);
    let downed = has(&[
        "复活",
        "濒死",
        "倒地",
        "倒下",
        "失去战斗能力",
        "无法战斗",
        "晕倒",
        "昏厥",
        "reviv",
        "resurrect",
        "knocked out",
        "knocked-out",
        "incapacitat",
        "fainted",
        "downed",
        "faints",
    ]);
    if !owned
        || !downed
        || has(&[
            "boss",
            "alpha respawn",
            "野生",
            "头目",
            "首领",
            "复活点",
            "被卖",
            "被分解",
            "卖掉",
            "放生",
            "卖了",
            "永久死亡",
            "永久丢失",
            "hardcore",
            "permadeath",
            "butcher",
            "sold",
            "player respawn",
            "respawn my character",
            "revive another player",
            "revive my friend",
            "revive a player",
            "pal revive me",
            "pal revive you",
            "permanently lost",
            "复活玩家",
            "revival potion recipe",
            "craft a revival potion",
            "where to obtain",
            "drop rate",
            "配方",
            "材料",
            "哪里掉",
            "怎么制作",
        ])
    {
        return None;
    }
    let setting = has(&[
        "设置",
        "设定",
        "倍率",
        "调快",
        "调短",
        "配置",
        "服务器参数",
        "setting",
        "config",
        "timer",
        "multiplier",
        "shorten",
        "reduce the time",
        "change the time",
        "speed up",
    ]);
    let en = q.is_ascii();
    let mut lines = if en {
        vec![
            "For an incapacitated Pal, put it into an ordinary Palbox storage slot. The default revival timer is 10 minutes of running world time. Let its HP recover before the next fight.",
            "For a downed base Pal, leave a Pal bed free and an active worker able to reach it. A worker can carry it to bed to recover. If you have a Revival Potion, use it on the incapacitated Pal.",
        ]
    } else {
        vec![
            "帕鲁失去战斗能力后，可以放入普通帕鲁终端的收纳栏。默认倒计时是世界运行中的 10 分钟，醒来后可继续恢复生命再出战。",
            "倒在据点里的帕鲁还可以由能通行的在岗同伴搬到空闲帕鲁床休养。手上有复活药时，也可以直接给倒下的帕鲁使用。",
        ]
    };
    if setting {
        lines.push(if en {
            "The published vanilla server settings list PalAutoHpRegeneRateInSleep for HP regeneration while stored, and BlockRespawnTime for the player’s respawn. Neither is documented as a control for the Pal’s incapacitation timer. A vanilla slider for that timer is unverified."
        } else {
            "原版专服文档中的 PalAutoHpRegeneRateInSleep 调整终端中帕鲁的生命回复倍率，BlockRespawnTime 调整玩家重生等待。两者的说明都不涉及上述倒地计时；目前未核实原版有直接修改该计时的选项。"
        });
    }
    let mut links = vec![
        ("复活药".into(), "#items/PalRevive".into()),
        ("帕鲁终端".into(), "#work/facilities/Palbox".into()),
        (
            "终端与复活时间".into(),
            "https://palworld.run/conditions/incapacitated".into(),
        ),
        (
            "据点床位与救援".into(),
            "https://palworld.wiki.gg/wiki/Beds".into(),
        ),
    ];
    if setting {
        links.push((
            "官方服务器参数".into(),
            "https://docs.palworldgame.com/settings-and-operation/configuration/".into(),
        ));
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_pals_have_several_recovery_methods_and_distinct_timers() {
        let a = answer("can i shorten my pal's revival timer?").unwrap();
        for known in [
            "10 minutes",
            "Pal bed",
            "Revival Potion",
            "PalAutoHpRegeneRateInSleep",
            "BlockRespawnTime",
        ] {
            assert!(a.text.contains(known), "{known}");
        }
        assert!(
            answer("我的帕鲁倒地了还可以怎么办")
                .unwrap()
                .text
                .contains("空闲帕鲁床")
        );
        for other in [
            "my alpha boss revive timer?",
            "how do i craft a revival potion for my pal?",
            "帕鲁头目多久复活",
            "玩家复活点怎么选",
            "这药的制作材料是什么",
        ] {
            assert!(answer(other).is_none(), "{other}");
        }
    }
}
