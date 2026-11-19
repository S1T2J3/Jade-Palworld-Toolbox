//! Versioned facts from the offline source bundle. The topic and the requested
//! action must both match, including when the planner emits them separately.
use crate::assistant_engine::{AssistantReply, field_text};
use crate::data::{Data, array, text};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Localized<T> {
    zh: T,
    en: T,
}

#[derive(Deserialize)]
struct Source {
    id: String,
    publisher: String,
    url: String,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    source: String,
    version: String,
    section: String,
    terms: Localized<Vec<String>>,
    text: Localized<String>,
    #[serde(default, rename = "alsoSources")]
    also_sources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Book {
    target_game_version: String,
    sources: Vec<Source>,
    entries: Vec<Entry>,
}

fn book() -> &'static Book {
    static BOOK: OnceLock<Book> = OnceLock::new();
    BOOK.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../pal-mechanics-dev/offline-knowledge/knowledge.json"
        ))
        .expect("bundled gameplay sources")
    })
}

// Entry id, subject aliases, action aliases and the related local handbook page.
const SCOPES: &[(&str, &str, &str, &str)] = &[
    (
        "guild-chest",
        "公会箱|跨据点|跨基地|不同据点|多个基地|所有基地|guild chest|between bases|across bases|all bases|other bases",
        "共享|拿|物资|箱|取物|材料|share|supplies|storage|items|materials|transfer",
        "#work/facilities/Guild_Chest",
    ),
    (
        "base-storage-search",
        "存取机|取出机|箱子|仓库|chest|chests|base storage|retrieval machine",
        "忘|搜|找|在哪|查找|search|where|find|retrieve|locate",
        "#work/facilities/Item_Retrieval_Machine",
    ),
    (
        "expedition-dispatch",
        "远征|expedition|expeditions",
        "派遣|编队|奖励|物资|收取|dispatch|send|supplies|rewards|resources",
        "#work/expeditions",
    ),
    (
        "research-rewards",
        "劳动研究所|工作研究所|据点研究|labor research|labour research|base research|research lab",
        "加成|生产|增益|解锁|加速|研究|productivity|boost|bonus|unlock|rewards",
        "#work/facilities/Pal_Labor_Research_Lab",
    ),
    (
        "skill-cooldowns",
        "冷却|cooldown|cooldowns|cooling down",
        "主动|三招|普攻|普通攻击|全|等待|all|basic|waiting|equipped moves",
        "#details/active",
    ),
    (
        "work-books",
        "适性书|适应性书|技术书|教本|work suitability|work technique|technique book|technique books",
        "提高|永久|读|工作|train|handbook|book|increase|raise|improve",
        "#work/suitability",
    ),
    (
        "flea-market",
        "跳蚤市场|玩家摆摊|其他玩家|flea market|flea markets|player stall|other players|sell to friends",
        "摆摊|交易|卖|出售|trade|sell|sale|trading",
        "#work/facilities/Flea_Market_Items",
    ),
    (
        "storage-privacy",
        "异次元帕鲁|帕鲁次元仓库|帕鲁仓库|dimensional storage|dimensional pal storage|shared pal storage",
        "私有|私人|共享|权限|共享帕鲁|private|privacy|shared|access|guild",
        "#work/facilities/Dimensional_Pal_Storage",
    ),
    (
        "drafting-table",
        "制图台|制图桌|设计图|图纸|schematic|schematics|blueprint|blueprints|drafting table",
        "合成|升阶|稀有度|合并|combine|merge|rarity|upgrade|upgrading",
        "#work/facilities/Drafting_Table",
    ),
    (
        "market-cooling",
        "跳蚤市场|摊位|摆摊|flea market|market stall",
        "腐坏|保鲜|变质|冷却|冷藏|ice pal|spoil|spoilage|decay|fresh",
        "#work/facilities/Flea_Market_Items",
    ),
    (
        "photo-wheel",
        "拍照|截图|摄影|photo|snapshot|screenshot|screenshots|camera",
        "界面|隐藏|镜头|相机|轮盘|模式|hud|camera|photo mode|interface|wheel",
        "#guide/quests",
    ),
    (
        "cosmetic-armour",
        "梳妆台|外观盔甲|护甲外观|幻化|antique dresser|cosmetic armor|cosmetic armour|armor appearance|armour appearance|transmog",
        "换|属性|外观|造型|护甲|stats|appearance|look|cosmetic|transmog",
        "#work/facilities",
    ),
    (
        "cross-platform",
        "跨平台|不同平台|跨端|crossplay|cross-play|cross platform|cross-platform|xbox|playstation",
        "跨|联机|一起|好友|多人|crossplay|cross-play|multiplayer|play together|steam|different platforms",
        "#guide/quests",
    ),
    (
        "fishing-shadows",
        "鱼影|抛竿|鱼饵|钓鱼|fishing|fish shadow|fish shadows|cast a rod",
        "鱼影|位置|开始|鱼饵|抛竿|第一次|bait|cast|shadow|shadows|start",
        "#guide/quests/a-fine-day-for-fishing",
    ),
    (
        "sea-salvage",
        "打捞|海底物资|salvage|salvaging|seafloor",
        "打捞|海|钓竿|水上|矿石|salvage|salvaging|sea|mount|rod|ore",
        "#guide/gathering",
    ),
    (
        "trust-growth",
        "信赖|好感|培养感情|trust|bond|bonding",
        "属性|成长|培养|提升|增加|相处|同伴|stat|stats|increase|improve|relationship|growth|bond",
        "#details/stats",
    ),
    (
        "zoe-recruitment",
        "佐伊|zoe",
        "招募|入队|成为同伴|加入队伍|加入我|recruit|ally|join|companion|mission",
        "#guide/quests/zoe-rayne",
    ),
    (
        "arena-modes",
        "竞技场|arena|战斗票券|battle tickets",
        "观战|单人|票券|规则|排名|solo|spectate|spectator|tickets|rules|rank",
        "#merchants/ArenaShop",
    ),
    (
        "parallel-incubation",
        "孵蛋器|孵化器|孵蛋|孵化|incubator|incubators|incubate|hatch|hatching",
        "大型|同时|多颗|多蛋|一次孵|一批|排队|parallel|multiple|several|at once|at the same time|large",
        "#work/facilities/Large_Incubator",
    ),
    (
        "faction-blueprints",
        "阵营|敌对据点|faction|enemy base|enemy bases",
        "建筑|图纸|设计图|奖励|blueprints|blueprint|building|rewards",
        "#guide/loot",
    ),
    (
        "watchtower-reveal",
        "瞭望塔|了望塔|点亮地图|开图|watchtower|watchtowers|watch tower|map fog",
        "解锁|互动|显示|传送|点亮|探索|开启|地图|activate|reveal|travel|map|explore",
        "#guide/quests/activate-great-eagle-statue",
    ),
    (
        "base-map-travel",
        "据点|基地|帕鲁终端|帕鲁箱|palbox|base|bases",
        "大地图|打开地图|传送|快速旅行|快速移动|不用摸|fast travel|fast-travel|teleport|travel from map|map travel",
        "#work/facilities/Palbox",
    ),
    (
        "repair-supplies",
        "修理包|建筑|设施|据点|repair kit|repair kits|structures|buildings|building",
        "修理|维修|修复|repair|repairs|repairing|repair kits",
        "#work/suitability",
    ),
    (
        "visit-progression",
        "来访|来客|商人|visits|visiting merchant|merchant|merchants",
        "来访|种类|据点等级|品类|到访|访问|visiting|visit|arrive|specialist|base level",
        "#merchants",
    ),
    (
        "awakening-gems",
        "觉醒|辉光宝石|radiant gem|radiant gems|awakening|awaken",
        "材料|宝石|世界树|获取|需要|material|crystal|gems|world tree|where",
        "#items",
    ),
    (
        "guild-roles",
        "公会权限|公会职级|公会成员|公会角色|guild|guildmate",
        "权限|角色|打不开|职级|限制|permissions|role|roles|access|restricted",
        "#work/facilities",
    ),
    (
        "simultaneous-raids",
        "祭坛|团体战|公会突袭|raid boss|raid bosses|summoning altar",
        "同时|一次|并发|两个|上限|simultaneously|simultaneous|second|at once|at a time|limit",
        "#work/facilities/Summoning_Altar",
    ),
    (
        "aquatic-unlock",
        "水上建造|水上建筑|水面建造|aquatic construction|build on water|build over water",
        "套装|工具|解锁|几级|条件|kit|unlock|level|cost|require",
        "#items/WaterBuildKit",
    ),
    (
        "holy-water-sources",
        "圣水|holy water",
        "来源|获取|哪里|刷|钓鱼池|远征|fishing|expedition|expeditions|source|sources|farm|where|get|obtain",
        "#items/WorldTreeHolyWater",
    ),
    (
        "holy-water-weight",
        "圣水|holy water",
        "重量|负重|多重|重多少|拿多少|weight|weigh|encumbrance|carry",
        "#items/WorldTreeHolyWater",
    ),
    (
        "production-refund",
        "取消制作|中止制作|取消生产|返还材料|中止生产|取消订单|cancel|cancelling|canceling|cancelled|canceled|refund",
        "材料|制作|生产|物资|退回|订单|crafting|materials|production|order|supplies",
        "#work/production",
    ),
    (
        "critics-target",
        "鉴定师|评论家|鉴赏家|pal critic|pal critics",
        "樱岛|樱花岛|天空岛|天阳乡|展示|哪只|sakurajima|sunreach|show|request",
        "#guide/quests",
    ),
    (
        "quest-fisherman",
        "超越次元|跨越次元|来自月球|月球背面|beyond dimensions|far side of the moon",
        "任务|渔村|哪里|开始|先决|解锁|how|start|quest|mission|unlock|where|requirements|fisherman's point",
        "#guide/quests/beyond-dimensions",
    ),
    (
        "campfire-burn",
        "篝火|营火|campfire|campfires",
        "烧伤|着火|点燃|踩|火伤|ignite|burn|burns|fire|step",
        "#guide/status",
    ),
    (
        "battle-selection",
        "据点帕鲁|召出|战斗出招|主动招式|summoned pal|base pal|base pals|summoned pals|active skill|active skills",
        "选择|优先|先用|伤害优先|出招|priority|prioritize|chooses|choose|choice|selection",
        "#details/active",
    ),
    (
        "reptyro-cryst-pair",
        "寒霜兽|熔岩兽|reptyro cryst|reptyro",
        "配种|亲本|配出|配吹雪狐|breed|pair|parents|foxcicle",
        "#paldex/VolcanicMonster_Ice",
    ),
    (
        "pengullet-pair",
        "企丸丸|棉悠悠|pengullet|lamball",
        "配种企丸丸|企丸丸亲本|配燎火鹿|breed pengullet|pengullet parents|rooby",
        "#paldex/Penguin",
    ),
    (
        "flambelle-pair",
        "融焰娘|棉悠悠|flambelle|lamball",
        "配种融焰娘|融焰娘亲本|配猫蝠怪|breed flambelle|flambelle parents|tombat",
        "#paldex/LavaGirl",
    ),
    (
        "anubis-level-55",
        "阿努比斯|anubis",
        "沙尘暴|沙尘龙卷|沙暴|升级技能|sand twister|level move",
        "#details/active/move/SandTwister",
    ),
    (
        "lamball-level-22",
        "棉悠悠|绵悠悠|lamball",
        "自爆|同归于尽|implosion|implode|self destruct|self-destruct|level move",
        "#details/active/move/SelfDestruct",
    ),
];

fn has_phrase(query: &str, phrase: &str) -> bool {
    if !phrase.is_ascii() {
        return query.contains(phrase);
    }
    query.match_indices(phrase).any(|(start, matched)| {
        let rest = &query[start + matched.len()..];
        !query[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
            && (!rest.starts_with(|c: char| c.is_ascii_alphanumeric())
                || phrase.len()>=4 && rest.strip_prefix('s').is_some_and(|tail|!tail.starts_with(|c:char|c.is_ascii_alphanumeric())))
    })
}

fn longest(query: &str, alternatives: &str) -> Option<usize> {
    alternatives
        .split('|')
        .filter(|word| has_phrase(query, word))
        .map(str::len)
        .max()
}

fn version_number(version: &str) -> Option<[u16; 3]> {
    let mut result = [0; 3];
    for (index, part) in version.split('.').enumerate() {
        *result.get_mut(index)? = part.parse().ok()?;
    }
    Some(result)
}

fn player_version(q: &str) -> Option<[u16; 3]> {
    let mut start = None;
    for (end, ch) in q.char_indices().chain(std::iter::once((q.len(), ' '))) {
        if ch.is_ascii_digit() || ch == '.' {
            start.get_or_insert(end);
            continue;
        }
        if let Some(at) = start.take() {
            let value = q[at..end].trim_matches('.');
            if value.contains('.')
                && (q[end..].trim_start().starts_with('版')
                    || [
                        "version", "patch", " v", "版本", "更新", "补丁", "before", "before v",
                    ]
                    .iter()
                    .any(|marker| q[..at].trim_end().ends_with(marker))
                    || &q[..at] == "v")
                && let Some(version) = version_number(value)
            {
                return Some(version);
            }
        }
    }
    None
}

fn applies_since(version: [u16; 3], question: &str, introduced: &str) -> bool {
    version_number(introduced).is_some_and(|first| {
        version > first
            || version == first
                && !["before ", "版以前", "版之前", "版本以前", "版本之前"]
                    .iter()
                    .any(|phrase| question.contains(phrase))
    })
}

/// The unversioned handbook describes the released 1.0 rules. The curated
/// patch facts below also cover individual changes from early access.
pub(crate) fn handbook_version_matches(question: &str) -> bool {
    player_version(question).is_none_or(|v| {
        Some(v) <= version_number(&book().target_game_version) && applies_since(v, question, "1.0")
    })
}

pub(super) fn lookup(query: &str) -> Option<AssistantReply> {
    lookup_in_context(query, query)
}

pub(super) fn lookup_in_context(query: &str, player_question: &str) -> Option<AssistantReply> {
    let q = field_text(query);
    let context = field_text(player_question);
    if [
        "with a mod",
        "modded",
        "装了模组",
        "装了mod",
        "write a poem",
        "write code",
        "写论文",
        "写诗",
    ]
    .iter()
    .any(|term| has_phrase(&context, term))
    {
        return None;
    }
    let book = book();
    let version = player_version(&context);
    if version > version_number(&book.target_game_version) {
        return None;
    }
    let english = !context
        .chars()
        .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let mut ranked = Vec::new();
    for &(entry_id, subjects, actions, route) in SCOPES {
        let Some(subject) = longest(&q, subjects) else {
            continue;
        };
        let entry = book.entries.iter().find(|entry| entry.id == entry_id)?;
        if version.is_some_and(|v| !applies_since(v, &context, &entry.version)) {
            continue;
        }
        let phrase = entry
            .terms
            .zh
            .iter()
            .chain(&entry.terms.en)
            .filter(|alias| has_phrase(&q, &field_text(alias)))
            .map(|alias| alias.len())
            .max()
            .unwrap_or(0);
        let general = ["怎么用","如何使用","有什么用","怎么运作","怎么工作","how do","how does","what is","what are","how to use"]
            .iter().any(|s|has_phrase(&q,s));
        let Some(action)=longest(&q,actions).or_else(||(general && phrase>0).then_some(1)) else{continue;};
        ranked.push((std::cmp::Reverse(subject + action + phrase), entry, route));
    }
    if ranked.is_empty() {
        return None;
    }
    ranked.sort_by_key(|(score, _, _)| *score);
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for (_, entry, route) in ranked.into_iter().take(2) {
        let source = book.sources.iter().find(|s| s.id == entry.source)?;
        let facts = if english {
            &entry.text.en
        } else {
            &entry.text.zh
        };
        let label = if english {
            &entry.terms.en[0]
        } else {
            &entry.terms.zh[0]
        };
        let mut citation = format!(
            "[{} v{} · {}] {}",
            source.publisher, entry.version, entry.section, source.url
        );
        for extra in &entry.also_sources {
            if let Some(source) = book.sources.iter().find(|s| &s.id == extra) {
                citation.push_str(&format!(" · {} {}", source.publisher, source.url));
            }
        }
        lines.push(format!("{facts}\n{citation}"));
        if !links.iter().any(|(_, known)| known == route) {
            links.push((label.clone(), route.to_owned()));
        }
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: Vec::new(),
    })
}

/// Join the actual station requirement to the suitability table. A pal named
/// by the player remains in the comparison even when it cannot operate the station.
pub(crate) fn staffing_evidence(data: &Data, q: &str) -> Option<AssistantReply> {
    if longest(q, "派谁|派哪只|派哪种|谁能操作|谁能负责|什么帕鲁|哪只帕鲁|哪个帕鲁|哪些帕鲁|哪种帕鲁|安排哪只|安排谁|适合派|适合在|能操作|能胜任|can operate|which pal|which pals|which worker|who should|who can|assign|assigned to|can work at|can run").is_none()
        || longest(q, "不工作|做不了|为什么|排除|不要|前期|只限|在我仓库|随机|掉落|搬走|建筑材料|where|why|early game|early-game|without|except|only my|in my palbox|not working|won't|cannot|drops|build cost").is_some()
    {
        return None;
    }
    let (station, _, _) = crate::assistant_engine::named_facility(data, q)?;
    // A power source and a station operator can be two different Pals.
    // Multi-role stations need their own explanation.
    let [requirement] = array(&station["required"]) else {
        return None;
    };
    let job = requirement[0].as_str()?;
    let minimum = requirement[1].as_u64()?;
    let jobs = [
        ("手工作业", "Handiwork"),
        ("生火", "Kindling"),
        ("制药", "Medicine Production"),
        ("采矿", "Mining"),
        ("伐木", "Lumbering"),
        ("浇水", "Watering"),
        ("播种", "Planting"),
        ("发电", "Generating Electricity"),
        ("冷却", "Cooling"),
        ("采集", "Gathering"),
        ("搬运", "Transporting"),
        ("牧场", "Farming"),
    ];
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let job_label = if english {
        jobs.iter().find(|(name, _)| *name == job)?.1
    } else {
        job
    };
    let station_id = text(station, "id");
    let station_name = if english {
        station_id.replace('_', " ")
    } else {
        text(station, "name").to_owned()
    };
    let names = crate::assistant_breeding::species_mentions(&data.species, q).0;
    let profiles = data.get("paldex-index-data")["profiles"].as_object()?;
    let mut workers = Vec::new();
    for (index, species) in data.species.iter().enumerate() {
        if text(species, "reason").contains("无法捕获")
            || !names.is_empty() && !names.iter().any(|p| p.0 == index)
        {
            continue;
        }
        let key = text(species, "key");
        let Some(work) = profiles.get(key).and_then(|p| p["work"].as_array()) else {
            continue;
        };
        let level = work
            .iter()
            .find(|pair| pair[0] == job)
            .and_then(|pair| pair[1].as_u64())
            .unwrap_or(0);
        if names.is_empty() && level < minimum {
            continue;
        }
        workers.push((
            std::cmp::Reverse(level),
            text(species, if english { "en" } else { "zh" }),
            key,
        ));
    }
    if workers.is_empty() {
        return None;
    }
    workers.sort_unstable();
    let mut links = vec![(
        station_name.clone(),
        format!("#work/facilities/{station_id}"),
    )];
    let mut lines = vec![if english {
        format!(
            "{station_name} requires {job_label} Lv.{minimum} or higher. Assign a base Pal that meets this requirement. Natural suitability from the Paldeck:"
        )
    } else {
        format!(
            "{station_name}需要{job_label} Lv.{minimum} 或以上。把符合条件的帕鲁分派到据点，再安排到这台设施。图鉴的基础工作适性如下。"
        )
    }];
    for &(std::cmp::Reverse(level), name, key) in
        workers
            .iter()
            .take(if names.is_empty() { 3 } else { workers.len() })
    {
        let status = match (english, level >= minimum) {
            (true, true) => "eligible",
            (true, false) => "below requirement",
            (false, true) => "可以派工",
            (false, false) => "未达到要求",
        };
        lines.push(format!("{name} · {job_label} Lv.{level} ({status})"));
        links.push((name.to_owned(), format!("#work/suitability/{key}")));
    }
    if longest(
        q,
        "词条|被动|工作速度|passive|passives|trait|traits|work speed",
    )
    .is_some()
        && matches!(
            job,
            "生火" | "制药" | "手工作业" | "采矿" | "伐木" | "浇水" | "播种"
        )
    {
        let trait_row = &data.get("pal-details-data")["passives"]["CraftSpeed_up2"];
        if let Some(effect) = trait_row["effect"].as_str() {
            let name = text(trait_row, if english { "en" } else { "zh" });
            lines.push(if english { format!("After checking suitability, consider {name} for work speed. Recorded effect: {effect}. It does not grant the missing suitability.") }
                else { format!("先满足岗位适性，再考虑「{name}」的{effect}；它不能让没有该适性的帕鲁获得这个工种。") });
            links.push((
                name.to_owned(),
                "#details/passive/trait/CraftSpeed_up2".into(),
            ));
        }
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_bundle_is_scoped_and_versions_follow_the_original_question() {
        let book = book();
        assert_eq!(book.entries.len(), 40);
        assert!(
            book.entries.iter().all(
                |entry| SCOPES.iter().filter(|r| r.0 == entry.id).count() == 1
                    && book.sources.iter().any(|source| source.id == entry.source)
            )
        );
        for entry in &book.entries {
            for (terms, facts) in [
                (&entry.terms.zh, &entry.text.zh),
                (&entry.terms.en, &entry.text.en),
            ] {
                let query = terms.join(" ");
                let response =
                    lookup(&query).unwrap_or_else(|| panic!("missing entry {}: {query}", entry.id));
                assert!(response.text.contains(facts), "{}: {query}", entry.id);
            }
        }
        let guild = lookup("How do we share storage across bases?").unwrap();
        assert!(
            guild.text.contains("guild chests")
                && guild.text.contains("Pocketpair v0.4.11")
                && guild.text.contains("https://")
        );
        let farm = lookup("基地范围内，能从大地图直接传送到另一座雕像吗？").unwrap();
        assert!(farm.text.contains("无须先走回帕鲁终端") && farm.text.contains("v1.0"));
        assert!(lookup("Which ice Pal is best in combat?").is_none());
        assert!(lookup("How many ancient technology points exist?").is_none());
        assert!(lookup("What other foods can my Pal eat besides fish bait?").is_none());
        assert!(
            lookup_in_context(
                "aquatic construction kit unlock",
                "what was the unlock in v1.0.2?"
            )
            .is_none()
        );
        assert!(
            lookup_in_context("build on water unlock", "水上建造在 1.0.5 版如何解锁？")
                .unwrap()
                .text
                .contains("23 级")
        );
        assert!(
            lookup("What changed in version 2.0 for guild chests and shared materials?").is_none()
        );
    }
}
