use super::AssistantReply;

struct Fix {
    subject: &'static [&'static str],
    symptom: &'static [&'static str],
    zh: &'static str,
    en: &'static str,
}

// Pocketpair's v1.0.4 release, in our own words. A patch describes a specific
// symptom in an earlier build; it is never proof of what caused a later report.
const SOURCE: &str = "https://store.steampowered.com/news/app/1623730/view/695397189431592457";
const FIXES: &[Fix] = &[
    Fix {
        subject: &[
            "树木",
            "砍树",
            "伐木",
            "natural trees",
            "regrown trees",
            "trees in my base",
            "logging",
        ],
        symptom: &[
            "不砍",
            "不伐",
            "重生",
            "刷新",
            "不干活",
            "won't",
            "stopped",
            "respawn",
            "regrow",
            "not cutting",
            "unreliab",
            "not harvest",
        ],
        zh: "官方修复过据点树木重新长出后，帕鲁不再对它们执行伐木的问题。可以对比新长出的树与伐木场，确认是不是只影响前者。",
        en: "A logging bug affected trees that regrew inside a base. Compare regrown trees with a Logging Site to narrow it down.",
    },
    Fix {
        subject: &["手柄", "控制器", "gamepad", "controller", "ps5"],
        symptom: &[
            "文字聊天",
            "打开聊天",
            "聊天窗口",
            "text chat",
            "open chat",
            "closes chat",
            "chat window",
        ],
        zh: "PS5 可以从轮盘菜单打开聊天。开发者还修复过使用手柄时聊天界面意外关闭的问题。",
        en: "On PS5, open chat from the radial menu. A controller issue that unexpectedly closed chat was fixed.",
    },
    Fix {
        subject: &["瞭望塔", "了望塔", "watchtower"],
        symptom: &[
            "黑",
            "没开",
            "未探索",
            "地图",
            "dark",
            "unexplored",
            "map",
            "reveal",
        ],
        zh: "已经解锁的瞭望塔周围仍显示未探索时，可再次与同一座塔互动，揭开附近地图。",
        en: "Interact again with the same unlocked Watchtower to reveal its surrounding map.",
    },
    Fix {
        subject: &["手下留情", "mercy hit"],
        symptom: &["骑乘", "坐骑", "骑着", "骑上", "mounted", "riding", "mount"],
        zh: "开发者修复过骑乘时，帕鲁攻击没有正确套用「手下留情」效果的问题。",
        en: "The patch corrected Mercy Hit on attacks made by a ridden Pal.",
    },
    Fix {
        subject: &["辉光宝石", "radiant gem"],
        symptom: &[
            "属性不对",
            "属性错误",
            "全是同",
            "同一种",
            "wrong element",
            "same element",
            "biased",
            "mismatch",
        ],
        zh: "世界树区域野生帕鲁掉落的辉光宝石，曾出现属性不符或分布严重偏斜的问题。",
        en: "Wild Pal drops in the World Tree region once produced mismatched or disproportionately frequent gem elements.",
    },
    Fix {
        subject: &[
            "宝箱权限",
            "箱子权限",
            "上锁的箱",
            "chest permission",
            "locked chest",
            "chest access",
        ],
        symptom: &[
            "材料", "消耗", "制作", "取用", "craft", "material", "consume",
        ],
        zh: "据点制作读取和消耗材料时，旧版可能错误处理箱子访问权限。核对该箱对当前玩家的授权，再用同一配方复试。",
        en: "Base crafting previously applied chest permissions incorrectly. Check access and retry the same recipe.",
    },
    Fix {
        subject: &[
            "重上游戏",
            "读档",
            "重新进入",
            "resuming",
            "reloading",
            "reload",
            "restart",
        ],
        symptom: &[
            "古代素材合成器",
            "古代材料合成器",
            "ancient material synthesizer",
        ],
        zh: "旧版有重新进入游戏后古代素材合成器没有正确应用工作速度倍率的问题。",
        en: "The Ancient Material Synthesizer previously lost its work-speed multiplier after resuming play.",
    },
    Fix {
        subject: &[
            "伙伴分配",
            "指派",
            "分配工作",
            "work assignment",
            "worker assignment",
            "pal assignments",
        ],
        symptom: &[
            "设施没",
            "不生效",
            "显示错误",
            "not reflected",
            "facility ignores",
            "doesn't register",
        ],
        zh: "据点设施曾有无法正确反映指派结果的问题。先核对工种要求及设施中显示的帕鲁。",
        en: "Base facilities sometimes failed to register worker assignments. Compare the required suitability and the assigned worker.",
    },
];

pub(super) fn lookup(q: &str) -> Option<AssistantReply> {
    let q = crate::assistant_engine::field_text(q);
    let fix = FIXES.iter().find(|fix| {
        fix.subject.iter().any(|s| q.contains(s)) && fix.symptom.iter().any(|s| q.contains(s))
    })?;
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let message = if en {
        format!(
            "Pocketpair documented this in v1.0.4. {} If it still happens, what version and platform are you playing on?",
            fix.en
        )
    } else {
        format!(
            "Pocketpair 在 v1.0.4 公告中记录了这个情况。{} 如果现在还能复现，你玩的是什么版本、哪个平台？",
            fix.zh
        )
    };
    Some(AssistantReply {
        text: message,
        links: vec![("Pocketpair · v1.0.4".into(), SOURCE.into())],
        suggestions: vec![],
    })
}
