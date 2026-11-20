//! Offline references for broad game mechanics. Dedicated lookups take priority.
use crate::assistant_engine::{AssistantReply, field_text};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub(super) struct Guide {
    pub(super) title: String,
    pub(super) aliases: Vec<String>,
    pub(super) zh: String,
    pub(super) en: String,
    pub(super) source: String,
    #[serde(default)]
    pub(super) also: Vec<String>,
    #[serde(default)]
    pub(super) label: String,
    #[serde(default)]
    pub(super) route: String,
}

pub(super) fn guides() -> &'static [Guide] {
    static PAGES: OnceLock<Vec<Guide>> = OnceLock::new();
    PAGES.get_or_init(|| {
        serde_json::from_str(include_str!("survival_guides.json")).expect("bundled survival guide")
    })
}

fn contains(text: &str, alias: &str) -> bool {
    if !alias.is_ascii() {
        return text.contains(alias);
    }
    text.match_indices(alias).any(|(start, matched)| {
        let end = start + matched.len();
        !text[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
            && !text[end..].starts_with(|c: char| c.is_ascii_alphanumeric())
    })
}

/// After a failed model selection, quote only a curated answer already present
/// in the offered evidence. Match the requested relationship as well as the subject.
pub(crate) fn bounded_excerpt(question: &str, facts: &str) -> Option<String> {
    let q = field_text(question);
    let title = q.lines().next()?.trim();
    let has = |words: &[&str]| words.iter().any(|word| contains(title, word));
    let chinese = title.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let (topic, boundary) = if has(&["food box","feed box","饲料箱","食槽"])
        && has(&["fullness","hungry","hunger","饱食","饱腹","饥饿"])
        && (title.contains('%') || has(&["threshold","阈值","百分比"])) {
        ("据点帕鲁自行进食", if chinese {"这条资料没有给出开始或停止进食的固定饱食百分比，也不能单凭它确定频繁往返的原因。"}
            else {"This source does not give a fixed fullness percentage for starting or stopping a meal, or establish the cause of the repeated trips."})
    } else if has(&["while in party","in the party","in my party","在队伍中","随队"])
        && has(&["saddle","saddles","鞍具","坐骑装备"])
        && !has(&["speed","cooldown","速度","冷却"]) {
        ("伙伴技能与专用装备", if chinese {"告诉我物种或伙伴技能名，就能核对它有没有装备解锁条件。"}
            else {"Which species or Partner Skill is this? Its record determines whether gear is required to unlock the effect."})
    } else if has(&["pal","pals","帕鲁"])
        && has(&["level them up","levels up","get their levels up","train my pals","给帕鲁练级","给队伍练级","提升它的等级"])
        && !has(&["fastest","maximum","cap","最快","上限"]) {
        ("给队伍中的帕鲁练级", "")
    } else {
        return None;
    };
    let guide = guides().iter().find(|g| g.title == topic)?;
    if !facts.contains(&guide.zh) && !facts.contains(&guide.en) { return None; }
    let body = if chinese { &guide.zh } else { &guide.en };
    Some(if boundary.is_empty() {body.clone()} else {format!("{body}\n{boundary}")})
}

#[cfg(test)]
mod bounded_tests {
    use super::*;

    #[test]
    fn a_partial_reply_requires_an_offered_fact_and_the_asked_relationship() {
        let g = guides().iter().find(|g| g.title == "据点帕鲁自行进食").unwrap();
        let q = "My base worker runs to the feed box at 42% fullness, why?";
        let answer = bounded_excerpt(q, &g.en).unwrap();
        assert!(answer.contains(&g.en) && answer.contains("does not give"));
        assert!(!answer.contains("42%"));
        assert!(bounded_excerpt(q, "Unrelated attack food tooltip").is_none());
        assert!(bounded_excerpt("我的食槽里面能放几个食物？", &g.zh).is_none());
    }
}

pub(super) fn lookup(terms: &[String]) -> Option<AssistantReply> {
    let mut matches = Vec::new();
    for guide in guides() {
        let score = guide
            .aliases
            .iter()
            .filter(|alias| terms.iter().any(|term| contains(term, alias)))
            .map(|alias| alias.chars().count())
            .max();
        if let Some(score) = score {
            matches.push((std::cmp::Reverse(score), guide));
        }
    }
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|(score, _)| *score);
    let english = !terms
        .iter()
        .any(|term| term.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)));
    let mut texts = Vec::new();
    let mut links = Vec::new();
    for (_, guide) in matches.into_iter().take(2) {
        let facts = if english { &guide.en } else { &guide.zh };
        let edition = if guide.source.starts_with("https://palworld.gamevault.in/") {
            " · GameVault v1.0.4"
        } else {
            ""
        };
        texts.push(format!(
            "游戏内生存指南 · {}{edition}（{}）\n{facts}",
            guide.title, guide.source
        ));
        if !guide.route.is_empty() && !links.iter().any(|(_, route)| route == &guide.route) {
            links.push((format!("相关图鉴 · {}", guide.label), guide.route.clone()));
        }
        for source in std::iter::once(&guide.source).chain(&guide.also) {
            if !links.iter().any(|(_, route)| route == source) {
                links.push((format!("来源 · {}", guide.title), source.clone()));
            }
        }
    }
    Some(AssistantReply {
        text: texts.join("\n"),
        links,
        suggestions: Vec::new(),
    })
}

pub(crate) fn for_question(question: &str) -> Option<AssistantReply> {
    let q = field_text(question);
    if ![
        "帕鲁",
        "据点",
        "孵蛋器",
        "科技点",
        "工作适性",
        "巨鹫之像",
        "巨鹫雕像",
        "翠叶鼠",
        "配种牧场",
        "家畜牧场",
        "力量石像",
        "房主",
        "专服",
        "palworld",
        "pal",
        "pals",
        "palbox",
        "pal sphere",
        "lifmunk",
        "pidf",
        "great eagle statue",
        "global palbox",
        "technology points",
        "partner skill",
        "multiplayer world",
        "co-op world",
        "guild",
        "联机世界",
        "世界存档",
        "公会",
        "host",
        "server",
    ]
    .iter()
    .any(|word| contains(&q, word))
        || [
            "写诗",
            "写论文",
            "写代码",
            "股票",
            "驱动",
            "write a poem",
            "write code",
        ]
        .iter()
        .any(|word| contains(&q, word))
    {
        return None;
    }
    let mut terms = vec![];
    if ["world", "save", "世界", "存档"]
        .iter()
        .any(|word| contains(&q, word))
        && ["host", "owner", "ownership", "房主", "主机"]
            .iter()
            .any(|word| contains(&q, word))
        && [
            "change", "move", "switch", "transfer", "pass", "迁移", "更换", "换", "转让", "交给",
        ]
        .iter()
        .any(|word| contains(&q, word))
    {
        terms.push("world save transfer".to_owned());
    }
    terms.push(q);
    lookup(&terms)
}

// Player actions with an unambiguous destination in the shipped handbook.
// Detailed numeric, version-specific and named-object questions stay with the table lookups.
pub(super) fn procedure(q: &str) -> Option<AssistantReply> {
    let any=|words:&[&str]|words.iter().any(|s|q.contains(s));
    if !super::handbook_version_matches(q) || any(&["modded","with a mod","装了mod","我的模组"]) { return None; }
    let chinese=q.chars().any(|c|('\u{3400}'..='\u{9fff}').contains(&c));
    let topics: &[&str];
    let mut extra=String::new();
    if any(&["request features","request a feature","suggest a feature","feature request","submit feedback",
        "file a bug report","report a bug","功能建议","给开发者提建议","反馈游戏","报告游戏问题","反馈bug","提交建议"]) &&
       !any(&["mod作者","mod author","工具箱","toolbox","unreal editor"]) {
        topics=&["向游戏开发者报告问题或提建议"];
    } else if any(&["孵蛋器","孵化器","incubator"]) && any(&["进度已满","进度满了","进度已完成","进度完成","时间到了","孵好了","孵化完成",
            "egg is ready","finished","timer is over","timer ran out","at 100%","incubation complete"]) &&
        any(&["取","领","拿","留在","拿不出来","怎么","how","get it out","stuck","still there"]) &&
        !any(&["ancient hatchery","古代孵化","自动","automatic","modded"]) {
        topics=&["从普通孵蛋器领取帕鲁"];
    } else if any(&["帕鲁","my pal","my pals","level pals","level up pals","pal xp","party pal"]) &&
        (any(&["练级","练等","快升级","level up","level pals","fast xp","more xp"]) ||
            any(&["拉高","速升","提升","快速升","练到"]) && any(&["等级","经验"])) &&
        !any(&["工种","适性","技能等级","伙伴技能","浓缩","技能果实","几级学","升星","最快的据点","skill level","suitability","condens","stars","skill fruit","learn a move",
            "刚孵","已孵","已经孵","刚出生","孵好的","刚配出","孵出来的","bred pal","hatched","newborn","after breeding","freshly bred","newly bred","already bred",
            "用完","没有修炼之书","without manuals","out of training manuals"]) {
        topics=&["给队伍中的帕鲁练级"];
    } else if any(&["新手","刚开始玩","刚入坑","初心者","first time","just started","new player","finally jumping in","beginner","starting out"]) &&
        any(&["怎么起步","干什么","做什么","建议","入门","攻略","tips","advice","first steps","where to start","what should i do"]) {
        topics=&["开局采集与第一处据点","捕捉时机","夜间生存与保暖"];
    } else if any(&["让帕鲁攻击","命令帕鲁攻击","集火目标","指挥帕鲁","command pals","pals attack","pal attack my target","focus on the same enemy"]) &&
        !any(&["伤害","damage","攻击速度","攻击力","attack speed","attack stat","boost","stack","power","cooldown","冷却","基地","据点"]) {
        topics=&["队伍指令与喂食轮盘"];
        extra=if chinese {"召出要参战的帕鲁。选「集中攻击相同敌人」，然后自己先命中目标。希望它主动找敌人时改用积极攻击。"}else{"Summon your combat Pal. For a particular target, choose Focus on Same Enemy and hit that target yourself. Use Attack Aggressively for automatic targeting."}.into();
    } else if any(&["据点","基地","base"]) && any(&["工人上限","能放几只","派驻数量","人数上限","提高上限","pal limit","worker limit","more pals per base","increase the number of pals"]) {
        topics=&["据点等级与工作栏"];
        extra=if chinese {"若已碰到世界设置上限，由房主调高每个据点的工作帕鲁上限。专服对应 BaseCampWorkerMaxNum。"} else {"If world settings cap the slots, the host can raise the worker limit. On dedicated servers the parameter is BaseCampWorkerMaxNum."}.into();
    } else if any(&["角色","存档","character","save","world"]) && any(&["单人","房主","single player","single-player","new host","another host"]) && any(&["转成","转移","迁移","transfer","move","convert"]) {
        topics=&["更换世界主机与保留存档"];
        if any(&["专服","服务器","dedicated server"]) && any(&["单人","single player","single-player"]) {
            extra=if chinese {"在 PC 上先新建并关闭一个测试单人世界，定位本地存档目录，再在副本上迁移。完整世界含 Level.sav 和 Players 等文件。专服角色与本地房主的标识可能不同，若出现新人物，需按存档版本重映射原角色，不能只改文件名。"}else{"On PC, create and close a test solo world to identify its save directory. Work on a copy of the entire server world, including Level.sav and Players. A new character can indicate a host-ID mismatch requiring a save-format-compatible remap."}.into();
        }
    } else if any(&["帕鲁","pals","pal "]) && any(&["心情低落","抑郁","降低san","sad","depress"]) &&
        any(&["怎么","如何","为什么","how","why","make","way to"]) {
        topics=&["据点 SAN","治疗生病的帕鲁"];
        extra=if chinese {"劳动强度、工作时长和进食影响 SAN。持续低 SAN 会提高患病风险，无法据此保证某一种病一定出现。"}else{"Long shifts and hunger lower SAN and can lead to illness. They do not guarantee a particular negative condition."}.into();
    } else if any(&["卡在地形","卡进地下","stuck under the map","stuck in terrain","stuck inside a rock"]) {
        topics=&["卡住时返回据点"];
    } else if any(&["房主下线","房主退出","host is offline","host logs off","host is not online","without the host"]) &&
        any(&["继续","还能","联机","server","world","play","join"]) {
        topics=&["房主下线与专用服务器"];
    } else {return None;}
    let mut lines=Vec::new();
    let mut links=Vec::new();
    for title in topics {
        let guide=guides().iter().find(|g|g.title==*title)?;
        lines.push(if chinese {guide.zh.clone()}else{guide.en.clone()});
        links.push((guide.title.clone(),guide.source.clone()));
        for source in &guide.also {
            if !links.iter().any(|(_, old)| old==source) {links.push((guide.title.clone(),source.clone()));}
        }
        if !guide.route.is_empty(){links.push((guide.label.clone(),guide.route.clone()));}
    }
    if !extra.is_empty(){lines.push(extra);}
    if topics==["更换世界主机与保留存档"] {links.push(("PC 存档格式与房主迁移说明".into(),"https://github.com/CraftHell/palworld-oodle-hostfix".into()));}
    Some(AssistantReply{text:lines.join("\n"),links,suggestions:Vec::new()})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilingual_guides_keep_species_products_and_world_limits_distinct() {
        for (query, fact) in [
            ("What's the Global Palbox for?", "cannot coexist"),
            ("我收集的翠叶鼠雕像要怎么用？", "捕获力"),
            (
                "Can I use a sphere on a Predator Pal?",
                "prevents sphere capture",
            ),
            ("配种牧场要放什么？", "一雄一雌"),
            ("Where do my Pals get ranch drops?", "Lamball produces wool"),
            ("据点饲料箱给谁用？", "自行前往进食"),
            (
                "Where do I change work types for base Pals at the Monitoring Stand?",
                "allow or prohibit",
            ),
            ("How does level sync affect my Pal?", "Tamer’s level"),
        ] {
            let r = for_question(query).unwrap();
            assert!(
                r.text.contains(fact) && r.text.contains("GameVault v1.0.4"),
                "{query}: {}",
                r.text
            );
            assert!(
                r.links
                    .iter()
                    .any(|(_, target)| target.starts_with("https://palworld.gamevault.in/"))
            );
        }
        assert!(for_question("Can you write code for a command wheel?").is_none());
    }

    #[test]
    fn distinguishes_world_host_character_and_guild_transfers() {
        for (query, fact) in [
            (
                "Could my co-op world change to a new host?",
                "separate complete copy",
            ),
            (
                "Can I migrate my character onto a different server?",
                "player’s level",
            ),
            ("怎样把公会转让给朋友，也让他接任会长？", "会长转让对话框"),
        ] {
            let result = for_question(query).unwrap();
            assert!(result.text.contains(fact), "{query}: {}", result.text);
            assert!(
                result
                    .links
                    .iter()
                    .filter(|(_, route)| route.starts_with("https://"))
                    .count()
                    >= 2
            );
        }
    }
}
