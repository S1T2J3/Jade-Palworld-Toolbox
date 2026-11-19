//! A static species table cannot diagnose a local installation or reproduce a
//! world's shuffled spawn list. Only intercept questions about those conditions.
use super::has;
use crate::assistant_engine::AssistantReply;

#[derive(Default)]
pub(crate) struct SpawnScope {
    previous_world: Option<String>,
    previous_random_query: bool,
}

impl SpawnScope {
    pub fn randomized_habitat(&mut self, q: &str, world: Option<&str>, habitat: bool) -> bool {
        let followup = self.previous_random_query && self.previous_world.as_deref() == world;
        let modified = has(
            q,
            &[
                "randomizer",
                "randomiser",
                "randomized",
                "randomised",
                "random spawns",
                "random pal",
                "随机帕鲁",
                "随机刷新",
                "随机模式",
                "随机化",
                "随机世界",
                "随机种子",
            ],
        );
        let standard = has(
            q,
            &[
                "no randomization",
                "no randomisation",
                "without random",
                "not randomized",
                "not randomised",
                "not using a random",
                "randomizer off",
                "randomizer disabled",
                "randomizer is off",
                "randomization is off",
                "randomizer is disabled",
                "randomizer is turned off",
                "randomisation is off",
                "randomiser is off",
                "disabled random",
                "turned off random",
                "normal world",
                "standard world",
                "default spawn",
                "普通世界",
                "原版分布",
                "默认刷新",
                "正常刷新",
                "关闭随机",
                "关了随机",
                "没开随机",
                "非随机",
                "不开随机",
                "没有开启随机",
                "随机模式已关闭",
                "随机化关了",
                "随机模式关了",
                "没用随机",
            ],
        );
        let query = q.trim().trim_end_matches(['?', '？', '.', '。']).trim_end();
        let asks_time_or_place = followup
            && ([
                "at night",
                "and at night",
                "what about at night",
                "during the day",
                "where else",
                "夜间呢",
                "晚上呢",
                "白天呢",
                "哪里还有",
                "什么位置",
            ]
            .contains(&query)
                || query.starts_with("which places at ")
                || query.starts_with("还有别的地点"));
        self.previous_random_query =
            (habitat || asks_time_or_place) && !standard && (modified || followup);
        self.previous_world = world.map(str::to_owned);
        self.previous_random_query
    }
}

pub(crate) fn is_habitat(reply: &AssistantReply) -> bool {
    reply
        .links
        .iter()
        .any(|(_, route)| route.starts_with("#spawns/"))
        || (reply
            .links
            .iter()
            .any(|(_, route)| route.starts_with("#map/coords/"))
            && reply.text.contains("刷新"))
}

fn response(q: &str, zh: &'static str, en: &'static str) -> &'static str {
    if q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)) {
        zh
    } else {
        en
    }
}

// Official meanings of Region, All and RandomizerSeed:
// https://docs.palworldgame.com/settings-and-operation/configuration/
pub(crate) fn spawn_help(q: &str) -> &'static str {
    response(
        q,
        "这张图鉴标的是默认世界的野生刷新点。随机模式会按世界种子重新分配帕鲁；区域随机按区域打乱，完全随机则跨区域打乱。工具箱目前不能按种子还原这张分布图。寻找目标时，在你自己的世界记录实际遇见它的位置；多人游戏可以向同一世界的玩家询问目击点。",
        "The habitat map lists default wild spawns. Random Pal Mode shuffles them using the world's seed, either by region or across the whole world. The toolbox cannot calculate this seed's locations. Mark sightings in your own world or, on a shared server, ask other players on that same seed where they encountered it.",
    )
}

// Pocketpair's support notice discusses incompatible mods and missing saves:
// https://news.palworldgame.com/
pub(crate) fn troubleshooting(q: &str) -> Option<&'static str> {
    if has(
        q,
        &[
            "lost my save",
            "lost save data",
            "save is missing",
            "save disappeared",
            "save is gone",
            "world disappeared",
            "character data is gone",
            "corrupted save",
            "save is corrupt",
            "丢档",
            "坏档",
            "存档丢失",
            "存档消失",
            "存档损坏",
            "存档不见",
            "存档没了",
            "世界存档找不到",
        ],
    ) {
        return Some(response(
            q,
            "先停止写入这个世界。单人或好友房间由房主核对所用账号和世界，专服则由管理员检查服务端存档。能访问存档时，先把现有数据和备份另存一份，再选问题发生前的备份时间点。你是在哪个平台、哪种房间丢失了进度？",
            "Stop saving to this world. In solo or co-op, have the host check the account and world being loaded. For a dedicated server, ask its administrator to check the server saves. If accessible, copy the current save and backups separately before choosing a backup from before the loss. Which platform and hosting mode are you using?",
        ));
    }
    if has(
        q,
        &[
            "game keeps crashing",
            "game crashes",
            "game won't launch",
            "game won't start",
            "palworld crashes",
            "palworld keeps crashing",
            "crash on launch",
            "crashes on launch",
            "进游戏就闪退",
            "游戏闪退",
            "游戏崩溃",
            "游戏启动不了",
            "启动游戏失败",
            "进游戏就崩",
        ],
    ) {
        return Some(response(
            q,
            "先确认崩溃发生在标题画面前、载入某个世界时，还是每个世界都发生。若只影响一份存档，先备份它；若刚更新且装过模组，请在备份后用与当前版本匹配的干净环境复测。记录平台、游戏版本和报错原文，才能继续定位。",
            "Check whether it crashes before the title screen, while loading one world, or on every world. Back up an affected save before testing. If you installed mods and recently updated, retest with a clean installation matching the game version. What platform, game version and exact error do you see?",
        ));
    }
    let network_failure = has(
        q,
        &[
            "can't connect",
            "cannot connect",
            "can't join",
            "cannot join",
            "connection timed out",
            "connection timeout",
            "connection refused",
            "连不上",
            "无法连接",
            "连接超时",
            "连接失败",
            "无法加入",
            "进不去",
        ],
    );
    if network_failure
        && has(
            q,
            &[
                "server",
                "multiplayer",
                "co-op",
                "coop",
                "friend",
                "host",
                "服务器",
                "专服",
                "房间",
                "联机",
                "房主",
                "好友的世界",
            ],
        )
    {
        return Some(response(
            q,
            "先让另一位玩家尝试加入同一房间，区分只有你失败还是所有人都失败。好友房间请房主确认世界仍在线，并重发当前邀请；专服请管理员核对服务状态、游戏版本和公布的连接地址。连接时显示的完整错误是什么？",
            "Ask another player to join the same world to see whether the failure is specific to your client. In co-op, have the host confirm the world is online and send the current invitation. For a dedicated server, have the administrator check that it is running and confirm the game version and advertised address. What exact connection error appears?",
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_habitat_requires_a_spawn_lookup_and_the_same_world_for_followups() {
        let mut scope = SpawnScope::default();
        assert!(!scope.randomized_habitat("how do i breed in randomizer mode?", Some("a"), false));
        assert!(scope.randomized_habitat("randomized pal locations", Some("a"), true));
        assert!(scope.randomized_habitat("what about night spawns?", Some("a"), true));
        assert!(scope.randomized_habitat("which places at night?", Some("a"), false));
        assert!(!scope.randomized_habitat("what about night spawns?", Some("b"), true));
        assert!(!scope.randomized_habitat(
            "where does it spawn without randomization?",
            Some("b"),
            true
        ));
        assert!(!scope.randomized_habitat(
            "the randomizer is off; where does it spawn?",
            Some("b"),
            true
        ));
    }

    #[test]
    fn operational_failures_are_distinct_from_gameplay_questions() {
        assert!(
            troubleshooting("游戏闪退，进不到标题界面")
                .unwrap()
                .contains("报错")
        );
        assert!(
            troubleshooting("my save disappeared")
                .unwrap()
                .contains("host")
        );
        assert!(
            troubleshooting("can't join my friend's world")
                .unwrap()
                .contains("invitation")
        );
        assert!(troubleshooting("can't join a guild").is_none());
        assert!(troubleshooting("where do the supply pods crash?").is_none());
        assert!(troubleshooting("如何让存档里的帕鲁提高搬运等级？").is_none());
    }
}
