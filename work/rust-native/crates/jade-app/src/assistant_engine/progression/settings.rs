//! Pocketpair's Palworld Server Guide, v1.0.4, "Configuration parameters".
//! Preserve the spelling in PalWorldSettings.ini, including Decreace/Regene.
use super::{AssistantReply, has};

const GUIDE: &str = "https://docs.palworldgame.com/settings-and-operation/configuration/";
const SETTINGS: &[(&str, &[&str], &str)] = &[
    (
        "ServerPlayerMaxNum",
        &[
            "同时在线人数",
            "服务器人数上限",
            "玩家在线上限",
            "players on the server",
            "server player cap",
        ],
        "限制同时加入这台服务器的玩家人数。",
    ),
    (
        "GuildPlayerMaxNum",
        &[
            "公会成员上限",
            "每个公会人数",
            "players per guild",
            "guild member limit",
        ],
        "限制单个公会可容纳的玩家人数。",
    ),
    (
        "GuildRejoinCooldownMinutes",
        &[
            "再加入公会",
            "重新加入公会",
            "退会冷却",
            "guild rejoin",
            "rejoining a guild",
        ],
        "重新加入公会的冷却，单位是分钟。",
    ),
    (
        "ItemWeightRate",
        &[
            "物品重量倍率",
            "背包物品减重",
            "item weight",
            "weight multiplier",
        ],
        "调节物品的重量倍率，修改的是物品重量。",
    ),
    (
        "EquipmentDurabilityDamageRate",
        &[
            "耐久消耗",
            "装备耐久损耗",
            "durability loss",
            "equipment wear",
        ],
        "调节装备耐久损耗倍率。",
    ),
    (
        "PlayerStaminaDecreaceRate",
        &["玩家耐力消耗", "角色耐力消耗", "player stamina depletion"],
        "调节玩家消耗耐力的速率。",
    ),
    (
        "PalStaminaDecreaceRate",
        &[
            "帕鲁耐力消耗",
            "坐骑耐力消耗",
            "pal stamina depletion",
            "mount stamina drain",
        ],
        "调节帕鲁消耗耐力的速率。参数名中的 Decreace 是配置文件采用的拼法。",
    ),
    (
        "PalSpawnNumRate",
        &[
            "野生帕鲁数量",
            "野怪数量倍率",
            "野外刷新数量",
            "pal spawn rate",
            "pal spawn multiplier",
        ],
        "调节帕鲁生成倍率，提高数值还会增加处理负载。",
    ),
    (
        "FishingDifficultyRate",
        &["钓鱼难度", "垂钓难度", "fishing difficulty"],
        "调节钓鱼难度。官方记录的范围为 0.1 到 1.0。",
    ),
    (
        "bAutoResetGuildNoOnlinePlayers",
        &[
            "清理退坑公会",
            "清理无人公会",
            "inactive guild cleanup",
            "delete inactive guilds",
        ],
        "为 True 时，整个公会无人登录达到时限后会清除其建筑与据点帕鲁。实施前核对公会、保留完整备份。",
    ),
    (
        "AutoResetGuildTimeNoOnlinePlayers",
        &[
            "公会离线时限",
            "退坑清理时间",
            "inactive guild timeout",
            "guild inactivity period",
        ],
        "指定整个公会无人登录多久后执行自动清理。必须先启用 bAutoResetGuildNoOnlinePlayers。",
    ),
    (
        "BaseCampMaxNumInGuild",
        &[
            "每个公会的据点",
            "公会据点上限",
            "每个公会几个基地",
            "bases per guild",
        ],
        "每个公会可建的据点上限，官方列出的默认值为 4，最高 10。",
    ),
    (
        "BaseCampWorkerMaxNum",
        &[
            "据点人数",
            "据点帕鲁上限",
            "据点工人",
            "基地工人",
            "pals per base",
            "workers per base",
        ],
        "控制单个据点可派驻的帕鲁数量，上限 50。",
    ),
    (
        "BaseCampMaxNum",
        &["全服据点", "服务器总基地", "bases across the server"],
        "控制全服的据点总数。",
    ),
    (
        "MaxBuildingLimitNum",
        &["建造数量", "建筑数量", "building count", "building cap"],
        "控制每位玩家的建筑数上限，0 表示无限制。",
    ),
    (
        "bEnableFastTravelOnlyBaseCamp",
        &[
            "只允许据点传送",
            "仅限据点传送",
            "only between bases",
            "fast travel between bases only",
        ],
        "控制是否只允许在据点之间传送。",
    ),
    (
        "bEnableFastTravel",
        &["禁用传送", "关闭快速旅行", "disable fast travel"],
        "控制快速旅行是否启用。",
    ),
    (
        "ExpRate",
        &[
            "经验倍率",
            "经验值倍率",
            "经验获取倍率",
            "exp multiplier",
            "xp multiplier",
            "experience multiplier",
        ],
        "经验获取倍率。",
    ),
    (
        "PalCaptureRate",
        &[
            "捕获倍率",
            "捕捉倍率",
            "捕获率倍率",
            "catch rate",
            "capture rate",
        ],
        "捕捉倍率；目标状态和所用帕鲁球仍会影响单次成功率。",
    ),
    (
        "PalEggDefaultHatchingTime",
        &["孵蛋时间", "孵化时间", "hatching time", "incubation time"],
        "设置巨大蛋的基准孵化时长，单位为小时。",
    ),
    (
        "MonsterFarmActionSpeedRate",
        &["牧场产出", "放牧速度", "grazing speed", "ranch production"],
        "调节牧场放牧产出的速度。",
    ),
    (
        "CollectionDropRate",
        &[
            "采集产量",
            "采集物数量",
            "gathered items",
            "gatherable items",
        ],
        "调节采集所得的物品数量。",
    ),
    (
        "CollectionObjectRespawnSpeedRate",
        &[
            "矿点刷新",
            "矿石刷新",
            "资源刷新",
            "resource respawn",
            "node respawn",
        ],
        "调节采集物刷新间隔。",
    ),
    (
        "EnemyDropItemRate",
        &[
            "敌人掉落数量",
            "怪物掉落数量",
            "掉落物数量倍率",
            "enemy drops",
            "enemy drop quantity",
        ],
        "调节敌人掉落物数量；说明中并未把它列作稀有物品的掉率。",
    ),
    (
        "BuildObjectDeteriorationDamageRate",
        &[
            "建筑劣化",
            "建筑腐朽",
            "建筑老化",
            "building decay",
            "structure deterioration",
        ],
        "调节建筑劣化的伤害倍率。",
    ),
    (
        "BuildObjectDamageRate",
        &[
            "建筑受伤",
            "建筑承伤",
            "building damage",
            "damage to buildings",
        ],
        "调节建筑物承受的伤害。",
    ),
    (
        "ItemCorruptionMultiplier",
        &["腐坏速度", "食物腐烂", "food spoilage", "spoil rate"],
        "调节物品腐坏速度。",
    ),
    (
        "PalAutoHpRegeneRateInSleep",
        &[
            "终端回血",
            "终端恢复",
            "palbox healing",
            "healing in the palbox",
        ],
        "调节睡眠中（帕鲁终端内）帕鲁的生命恢复倍率。",
    ),
    (
        "PalDamageRateAttack",
        &[
            "帕鲁造成伤害",
            "帕鲁造成的伤害",
            "damage dealt by pals",
            "pal damage dealt",
        ],
        "所有帕鲁造成伤害的倍率。",
    ),
    (
        "PalDamageRateDefense",
        &[
            "帕鲁受到伤害",
            "帕鲁受到的伤害",
            "damage taken by pals",
            "damage to pals",
        ],
        "所有帕鲁所受伤害的倍率。",
    ),
    (
        "PlayerDamageRateAttack",
        &[
            "玩家造成伤害",
            "玩家造成的伤害",
            "player damage dealt",
            "damage dealt by players",
        ],
        "玩家造成伤害的倍率。",
    ),
    (
        "PlayerDamageRateDefense",
        &[
            "玩家受到伤害",
            "玩家受到的伤害",
            "damage taken by players",
            "player damage taken",
        ],
        "玩家受到伤害的倍率。",
    ),
    (
        "bPalLost",
        &[
            "永久失去帕鲁",
            "帕鲁永久死亡",
            "pal permadeath",
            "permadeath for pals",
            "permanently lose pals",
        ],
        "控制帕鲁死亡后是否永久失去。修改前先备份世界。",
    ),
    (
        "DeathPenalty",
        &[
            "死亡惩罚",
            "死亡掉落",
            "death penalty",
            "drop items on death",
        ],
        "None 不掉落，Item 掉背包物品、保留身上装备，ItemAndEquipment 连装备也掉落，All 还会掉落队伍帕鲁。",
    ),
    (
        "bEnableInvaderEnemy",
        &[
            "关闭据点袭击",
            "关闭入侵",
            "基地入侵",
            "disable invasions",
            "disable base raids",
        ],
        "控制据点入侵事件是否开启。",
    ),
    (
        "SupplyDropSpan",
        &[
            "空投间隔",
            "陨石间隔",
            "supply drop interval",
            "meteorite interval",
        ],
        "空投或陨石事件的间隔，单位为分钟。",
    ),
    (
        "DayTimeSpeedRate",
        &["白天时长", "白天速度", "day length", "daytime speed"],
        "调节游戏内白天时间推进的速度。",
    ),
    (
        "NightTimeSpeedRate",
        &["夜晚时长", "夜间速度", "night length", "nighttime speed"],
        "调节游戏内夜晚时间推进的速度。",
    ),
    (
        "CrossplayPlatforms",
        &[
            "跨平台列表",
            "跨平台联机",
            "crossplay platforms",
            "crossplay setting",
        ],
        "决定允许加入的客户端平台，设置内容应包含来访玩家的平台。",
    ),
    (
        "bIsUseBackupSaveData",
        &["开启备份", "启用备份", "enable backups", "world backups"],
        "开启世界备份，由服务端在存档目录下生成 backup。",
    ),
    (
        "bIsPvP",
        &["enable pvp", "开启pvp", "打开pvp", "关闭pvp", "disable pvp"],
        "控制服务端是否开启 PvP。",
    ),
];

pub(super) fn answer(q: &str) -> Option<AssistantReply> {
    if has(
        q,
        &[
            "专服",
            "私人服务器",
            "私服",
            "dedicated server",
            "private server",
        ],
    ) && has(
        q,
        &[
            "退坑",
            "长期离线",
            "不再上线",
            "无人公会",
            "inactive",
            "abandoned guild",
        ],
    ) && has(
        q,
        &[
            "清理", "移除", "删除", "拆除", "remove", "clear", "delete", "clean up",
        ],
    ) && has(q, &["基地", "据点", "公会", "base", "guild"])
    {
        return Some(AssistantReply{
            text: "先核对据点归属和公会成员，再给整个世界存档做备份。专服在实际使用的 PalWorldSettings.ini 中可启用 bAutoResetGuildNoOnlinePlayers，再用 AutoResetGuildTimeNoOnlinePlayers 设置离线时限。只有整个公会的成员都未登录达到时限，才会触发自动清除其建筑及据点帕鲁。若只是某位成员退坑而公会仍活跃，请由该公会处理具体据点。保存设置并重启专服后生效。".into(),
            links:vec![("Pocketpair 专服参数说明".into(),GUIDE.into())],suggestions:vec![],
        });
    }
    let server_option = has(q, &["server", "服务器", "服务端", "世界"])
        && has(q, &["setting", "config", "slider", "parameter", ".ini", "option", "设置", "配置", "选项", "参数"]);
    let configuration = server_option || has(
        q,
        &[
            "服务器设置",
            "专服",
            "世界设置",
            "配置文件",
            "调倍率",
            "哪个参数",
            "什么参数",
            "哪一项设置",
            "倍率怎么",
            "倍率如何",
            "倍率怎样",
            "server setting",
            "configuration",
            "config ",
            "which setting",
            "world setting",
            "ini file",
            "enable pvp",
        ],
    );
    let matches: Vec<_> = SETTINGS
        .iter()
        .filter(|(key, descriptions, _)| {
            // Match a whole config key, so …Attack does not accidentally match a longer key.
            let name = key.to_ascii_lowercase();
            let named = q
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|part| part == name);
            named || configuration && has(q, descriptions)
        })
        .collect();
    if matches.is_empty() {
        return None;
    }
    let mut lines = vec!["按 Pocketpair 的专服设置表，对应参数如下。".to_owned()];
    for (key, _, detail) in matches {
        lines.push(format!("{key}：{detail}"));
    }
    lines.push("专服要改实际使用的 PalWorldSettings.ini，保存后重启服务。房主从单机菜单改难度时使用世界设置界面。".into());
    if has(q, &["broken", "ignored", "not working", "reset", "doesn't", "doesn’t", "won't", "won’t", "失效", "没生效", "不生效", "重置", "不起作用"]) {
        lines.push("先停服并保留当前配置副本，在该实例的 PalWorldSettings.ini 的 OptionSettings 中核对键名及允许的值。启动后重新读取配置，检查管理面板或启动参数是否覆盖，然后用一次可还原的小范围测试验证。".into());
    }
    lines.push(format!("官方参数说明：{GUIDE}"));
    Some(AssistantReply {
        text: lines.join("\n"),
        links: vec![],
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_keys_and_configuration_intents_only() {
        let abandoned = answer("怎么在私人服务器清理长期离线玩家的据点？").unwrap();
        assert!(
            abandoned.text.contains("AutoResetGuildTimeNoOnlinePlayers")
                && abandoned.text.contains("整个公会")
                && abandoned.text.contains("备份")
        );
        assert!(answer("我在私人服务器想清理背包").is_none());
        assert!(
            answer("which setting changes equipment wear?")
                .unwrap()
                .text
                .contains("EquipmentDurabilityDamageRate")
        );
        assert!(answer("我的捕获倍率看起来很低").is_none());
        let a = answer("which setting changes the resource respawn interval?").unwrap();
        assert!(a.text.contains("CollectionObjectRespawnSpeedRate"));
        assert!(
            answer("expRate".to_lowercase().as_str())
                .unwrap()
                .text
                .contains("ExpRate")
        );
        assert!(answer("myexprateoverride").is_none());
        let ignored = answer("my server's equipment wear option gets reset every restart").unwrap();
        assert!(ignored.text.contains("EquipmentDurabilityDamageRate") && ignored.text.contains("OptionSettings"));
        assert!(answer("can i carry more weight on the server").is_none());
    }
}
