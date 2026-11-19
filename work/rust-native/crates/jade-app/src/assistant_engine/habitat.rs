use super::{AssistantReply, field_text};
use crate::assistant_breeding::species_mentions;
use crate::assistant_data::chinese_count;
use crate::data::{Data, array, text};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const CELL: i64 = 100;
const NEAR: i64 = 120;

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}
fn word(q: &str, w: &str) -> bool {
    q.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|s| s == w)
}

fn element(q: &str) -> Option<(&'static str, &'static str)> {
    let species_type = [
        "pal",
        "pals",
        "type",
        "types",
        "monster",
        "monsters",
        "mobs",
        "mob",
        "enemies",
        "creatures",
        "creature",
        "element",
    ]
    .iter()
    .any(|term| word(q, term));
    [
        (
            "electric",
            "雷",
            &[
                "电系",
                "雷系",
                "电属性",
                "雷属性",
                "電屬",
                "electric",
                "lightning",
                "electricity",
            ][..],
        ),
        ("fire", "火", &["火系", "火属性", "fire"][..]),
        ("ice", "冰", &["冰系", "冰属性", "ice"][..]),
        ("water", "水", &["水系", "水属性", "water"][..]),
        ("grass", "草", &["草系", "草属性", "grass"][..]),
        ("dark", "暗", &["暗系", "暗属性", "dark"][..]),
        ("dragon", "龙", &["龙系", "龙属性", "dragon"][..]),
        ("ground", "地", &["地系", "地属性", "地面系", "ground"][..]),
        (
            "neutral",
            "无",
            &["无属性", "无系", "一般系", "neutral"][..],
        ),
    ]
    .into_iter()
    .find(|(_, _, terms)| {
        terms.iter().any(|t| {
            if t.is_ascii() {
                species_type && word(q, t)
            } else {
                q.contains(t)
            }
        })
    })
    .map(|(id, name, _)| (id, name))
}

#[derive(Clone, Debug)]
enum Target {
    Named(Vec<String>),
    Element(&'static str, &'static str),
    Training,
}

fn training_levels(q: &str) -> Option<Levels> {
    if !has(q, &["练级", "练等", "刷经验", "level up", "leveling", "levelling", "gain xp"])
        || !has(q, &["抓", "捕捉", "捕获", "catch", "capture"]) {
        return None;
    }
    for n in super::quantities::integers(q) {
        let prefix = q[..n.start].trim_end();
        let suffix = q[n.end..].trim_start();
        let player = ["玩家", "玩家现在", "我", "我才", "我现在", "我目前", "我已经", "角色", "人物"]
            .iter().any(|p| prefix.ends_with(p)) && suffix.starts_with('级')
            || ["i'm level", "i am level", "i’m level", "player level", "my level is"]
                .iter().any(|p| prefix.ends_with(p));
        if player {
            let level = i64::try_from(n.value).ok()?;
            if level == 0 { return None; }
            // Remove the player level before interpreting a second level as the encounter goal.
            let rest = suffix.strip_prefix('级').unwrap_or(suffix);
            let other = format!("{prefix}；{rest}");
            return Some(Levels::parse(&other).unwrap_or(Levels::Range((level - 5).max(1), level)));
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
enum Levels {
    Early,
    At(i64),
    Range(i64, i64),
}
impl Levels {
    fn parse(q: &str) -> Option<Self> {
        let mut nums = Vec::new();
        let mut digits = None;
        for (i, c) in q.char_indices().chain(std::iter::once((q.len(), ' '))) {
            if c.is_ascii_digit() || "零一二两三四五六七八九十百".contains(c) {
                digits.get_or_insert(i);
            } else if let Some(start) = digits.take()
                && let Some(n) = q[start..i]
                    .parse::<i64>()
                    .ok()
                    .or_else(|| chinese_count(&q[start..i]).and_then(|v| i64::try_from(v).ok()))
            {
                nums.push((start, i, n));
            }
        }
        for pair in nums.windows(2) {
            let [(a, end, min), (start, b, max)] = pair else {
                continue;
            };
            let middle = q[*end..*start].trim().trim_start_matches('级').trim();
            if has(q, &["级", "level", "lv"])
                && ["-", "~", "–", "到", "至", "to", "and"].contains(&middle)
                && min <= max
                && (q[*b..].trim_start().starts_with('级') || has(&q[..*a], &["level", "lv"]))
            {
                return Some(Self::Range(*min, *max));
            }
        }
        for (start, end, n) in nums {
            let before = q[..start].trim_end();
            let after = q[end..].trim_start();
            if !after.starts_with('级')
                && !["level", "levels", "lv.", "lv"]
                    .iter()
                    .any(|x| before.ends_with(x))
            {
                continue;
            }
            let qualifier = before
                .strip_suffix("level")
                .or_else(|| before.strip_suffix("levels"))
                .unwrap_or(before)
                .trim_end();
            if has(
                before,
                &["不超过", "最高", "最多", "我才", "我现在", "我目前"],
            ) || has(
                after,
                &[
                    "级及以下",
                    "级以下",
                    "级以内",
                    "or lower",
                    "or below",
                    "and below",
                ],
            ) || qualifier.ends_with("up to")
                || qualifier.ends_with("at most")
            {
                return Some(Self::Range(0, n));
            }
            if has(before, &["低于", "不到"])
                || qualifier.ends_with("under")
                || qualifier.ends_with("below")
            {
                return Some(Self::Range(0, n - 1));
            }
            if has(before, &["至少", "不低于"])
                || has(after, &["级以上", "级及以上", "or higher"])
                || qualifier.ends_with("at least")
            {
                return Some(Self::Range(n, i64::MAX));
            }
            if before.ends_with("over level") || has(before, &["高于", "超过"]) {
                return Some(Self::Range(n.saturating_add(1), i64::MAX));
            }
            return Some(Self::At(n));
        }
        (has(
            q,
            &[
                "早期",
                "前期",
                "初期",
                "开局",
                "新手",
                "刚开",
                "early-game",
                "beginner",
                "starter",
                "starting out",
            ],
        ) || word(q, "early"))
        .then_some(Self::Early)
    }
    fn accepts(self, pair: Option<(i64, i64)>) -> bool {
        let Some((a, b)) = pair else { return false };
        match self {
            Self::Early => b <= 15,
            Self::At(n) => a <= n && n <= b,
            Self::Range(min, max) => a >= min && b <= max,
        }
    }
    fn label(self) -> String {
        match self {
            Self::Early => "前期按记录最高等级不超过 Lv.15 筛选".into(),
            Self::At(n) => format!("记录等级包含 Lv.{n}"),
            Self::Range(0, b) => format!("记录最高不超过 Lv.{b}"),
            Self::Range(a, i64::MAX) => format!("记录最低为 Lv.{a} 或更高"),
            Self::Range(a, b) => format!("记录等级在 Lv.{a}–{b} 以内"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct HabitatSession {
    target: Option<Target>,
    level: Option<Levels>,
    map: Option<String>,
    anchor: Option<String>,
    region_sample: bool,
    time: Option<&'static str>,
    boss: Option<bool>,
}

fn named_map(q: &str) -> Option<&'static str> {
    if has(q, &["世界树", "world tree", "worldtree"]) {
        Some("worldtree")
    } else if has(
        q,
        &["帕洛斯", "帕罗斯", "帕拉帕戈斯", "palpagos", "palapagos"],
    ) {
        Some("palpagos")
    } else {
        None
    }
}

pub(super) fn landmark<'a>(d: &'a Data, q: &str) -> Option<&'a Value> {
    let points = array(&d.get("map-data")["points"]);
    let direct = points
        .iter()
        .filter(|p| matches!(text(p, "category"), "fast_travel" | "syndicate_tower"))
        .filter(|p| {
            let name = text(p, "name");
            name.chars().count() >= 2 && q.contains(&field_text(name))
        })
        .max_by_key(|p| text(p, "name").len());
    if direct.is_some() {
        return direct;
    }
    let region = if q.contains("feybreak") {
        "天坠之地"
    } else if q.contains("sakurajima") {
        "樱岛"
    } else if has(q, &["mount obsidian", "volcano"]) {
        "火山"
    } else if word(q, "desert") {
        "沙漠"
    } else if has(q, &["snowy mountain", "astral mountains"]) {
        "雪山"
    } else {
        q
    };
    let stage = array(&d.get("expedition-data")["stages"])
        .iter()
        .find(|s| {
            let name = text(s, "region");
            !name.is_empty()
                && name != "世界树"
                && (region.contains(name) || (name == "天坠之地" && region.contains("天坠岛")))
        })?;
    points.iter().find(|p| p["id"] == stage["point"])
}

#[derive(Clone, Copy)]
struct Encounter<'a> {
    key: &'a str,
    name: &'a str,
    x: i64,
    y: i64,
    level: Option<(i64, i64)>,
    time: u64,
    boss: bool,
}
struct Cell<'a> {
    sites: BTreeSet<(i64, i64)>,
    names: BTreeSet<&'a str>,
    example: Encounter<'a>,
}
fn example_order(e: Encounter<'_>) -> (bool, i64, i64, u64, i64, i64, &str) {
    let (lo, hi) = e.level.unwrap_or((i64::MAX, i64::MAX));
    (e.boss, hi, lo, e.time, e.x, e.y, e.key)
}
fn valid_level(row: &Value) -> Option<(i64, i64)> {
    let (lo, hi) = (row[1].as_i64()?, row[2].as_i64()?);
    (lo > 0 && hi >= lo).then_some((lo, hi))
}

fn subject_text(question: &str) -> &str {
    // A Pal mentioned as part of the player's party need not be the thing they are looking for.
    for (i, ch) in question
        .char_indices()
        .filter(|(_, c)| matches!(c, '，' | ',' | '。' | '?' | '？' | '.' | ';' | '；'))
    {
        let (first, rest) = (&question[..i], question[i + ch.len_utf8()..].trim());
        let background = |s: &str| {
            [
                "i use ",
                "i have ",
                "i own ",
                "my party ",
                "my team ",
                "我带着",
                "我已经有",
                "我手上有",
                "我用的是",
                "我的队伍",
            ]
            .iter()
            .any(|p| s.starts_with(p))
        };
        if background(rest) {
            return first;
        }
        if background(first) && !rest.is_empty() {
            return rest;
        }
    }
    question
}

fn item_mentioned(d: &Data, q: &str) -> bool {
    array(&d.get("item-catalog-data")["items"])
        .iter()
        .any(|item| {
            [text(item, "name"), text(item, "en")]
                .into_iter()
                .filter(|n| n.chars().count() >= 2)
                .any(|name| {
                    let name = field_text(name);
                    q.match_indices(&name).any(|(start, _)| {
                        !name.is_ascii()
                            || (!q[..start]
                                .bytes()
                                .next_back()
                                .is_some_and(|c| c.is_ascii_alphanumeric())
                                && !q[start + name.len()..]
                                    .bytes()
                                    .next()
                                    .is_some_and(|c| c.is_ascii_alphanumeric()))
                    })
                })
        })
}

fn repeats_location(q: &str) -> bool {
    let mut s = q
        .trim()
        .trim_end_matches(['?', '？', '。', '.', '!'])
        .trim();
    if s.is_ascii() {
        let words: Vec<_> = s.split_whitespace().collect();
        return words.iter().any(|w| {
            matches!(
                *w,
                "coordinates" | "coords" | "location" | "where" | "locations"
            )
        }) && words.iter().all(|w| {
            matches!(
                *w,
                "its"
                    | "their"
                    | "it"
                    | "them"
                    | "they"
                    | "the"
                    | "that"
                    | "same"
                    | "those"
                    | "one"
                    | "ones"
                    | "coordinates"
                    | "coords"
                    | "location"
                    | "locations"
                    | "where"
                    | "what"
                    | "are"
                    | "is"
                    | "again"
                    | "exact"
                    | "exactly"
                    | "please"
                    | "and"
                    | "show"
                    | "me"
                    | "can"
                    | "you"
                    | "give"
            )
        });
    }
    loop {
        if let Some(rest) = [
            "那么",
            "那只的",
            "那",
            "请问",
            "请",
            "再给我",
            "给我",
            "看看",
            "再看",
            "它们的",
            "它们",
            "它的",
            "它",
            "这只的",
            "上一个的",
            "具体",
            "的",
        ]
        .iter()
        .find_map(|p| s.strip_prefix(p))
        {
            s = rest.trim();
        } else {
            break;
        }
    }
    s = s.trim_end_matches(['呢', '吗', '吧']).trim();
    matches!(
        s,
        "坐标"
            | "地图坐标"
            | "位置"
            | "具体位置"
            | "具体坐标"
            | "点位"
            | "出没点"
            | "出没位置"
            | "栖息地"
            | "刷新点"
            | "在哪"
            | "在哪里"
            | "在哪儿"
            | "在哪个位置"
    )
}

impl HabitatSession {
    pub(super) fn respond(&mut self, d: &Data, question: &str) -> Option<AssistantReply> {
        let q = field_text(question);
        if let Some(reply) = reported_sighting(d, &q) {
            *self = Self::default();
            return Some(reply);
        }
        let where_to = has(
            &q,
            &[
                "哪里",
                "哪儿",
                "在哪",
                "哪裏",
                "分布",
                "出没",
                "出现在",
                "刷新点",
                "能找到哪些",
                "可以找到哪些",
                "能遇到哪些",
                "能遇到什么",
                "有哪些野生",
                "what wild pals",
                "which wild pals",
                "which wild",
                "刷怪点",
                "去哪抓",
                "去哪找",
                "哪有",
                "哪边",
                "坐标",
                "座标",
                "地点",
                "哪能",
                "哪可以",
                "哪张图",
                "哪最多",
                "在什么地方",
                "哪个地方",
                "哪一带",
                "哪片",
                "where",
                "spawn location",
                "spawn point",
                "habitat",
                "coordinates",
                "location of",
                "spots for",
            ],
        );
        let repeated = self.target.is_some() && repeats_location(&q);
        let follow = self.target.is_some()
            && q.chars().count() < 95
            && (repeated
                || q.starts_with(['那', '只', '换', '改', '限'])
                || q.trim_end_matches(['?', '？', '。']).ends_with('呢')
                || has(
                    &q,
                    &[
                        "what about",
                        "how about",
                        "only ",
                        "instead",
                        "up to level",
                        "under level",
                    ],
                ));
        if !where_to && !follow {
            return None;
        }
        if has(
            &q,
            &[
                "配种",
                "繁殖",
                "掉落",
                "掉率",
                "配方",
                "蓝图",
                "设计图",
                "图纸",
                "防具",
                "器官",
                "武器",
                "鞍具",
                "盔甲",
                "头盔",
                "建筑",
                "设施",
                "工作台",
                "传送装置",
                "防御装置",
                "饲料",
                "钥匙",
                "祭坛",
                "商人",
                "购买",
                "词条",
                "技能果实",
                "蛋糕",
                "克制",
                "工作适性",
                "工作能力",
                "伤害",
                "捕获率",
                "捕捉率",
                "装备",
                "材料",
                "尸体",
                "地基",
                "breed",
                "drops",
                "schematic",
                "merchant",
                "skill fruit",
                "cake",
                "counter",
                "capture rate",
                "catch rate",
                "weapon",
                "armor",
                "armour",
                "design plan",
                "blueprint",
                "schematic",
                "saddle",
                "organ",
                "structures",
                "building",
                "facility",
                "workbench",
                "ingredient",
                "dropped by",
            ],
        ) {
            return None;
        }
        let time = if has(
            &q,
            &[
                "仅夜",
                "只在晚上",
                "只在夜间",
                "night only",
                "nocturnal only",
            ],
        ) {
            Some("仅夜间出现")
        } else if has(&q, &["夜间", "晚上", "夜里", "夜晚", "night"]) {
            Some("夜间可遇")
        } else if has(
            &q,
            &[
                "白天",
                "白昼",
                "昼间",
                "daytime",
                "during the day",
                "by day",
            ],
        ) {
            Some("白天可遇")
        } else {
            None
        };
        let boss = if has(
            &q,
            &[
                "非头目",
                "不看头目",
                "不要头目",
                "普通野怪",
                "普通刷新",
                "non-alpha",
                "non-boss",
                "no bosses",
            ],
        ) {
            Some(false)
        } else if has(&q, &["头目", "首领", "alpha", "boss"]) {
            Some(true)
        } else {
            None
        };
        let practice = training_levels(&q);
        let level = practice.or_else(|| Levels::parse(&q));
        let map = named_map(&q);
        let poi = landmark(d, &q);
        // A boss/landmark is the subject; party members in later sentences do not describe its habitat.
        if has(
            &q,
            &[
                "tower boss",
                "塔的首领",
                "塔的头目",
                "塔首领",
                "塔主",
                "塔的boss",
                "塔boss",
            ],
        ) {
            return None;
        }
        let subject = subject_text(&q);
        let types = element(subject);
        let (mentions, _) = species_mentions(&d.species, subject);
        if (!mentions.is_empty() || types.is_some())
            && !has(
                subject,
                &[
                    "野生",
                    "野外",
                    "出没",
                    "刷新点",
                    "刷怪点",
                    "遇到",
                    "哪里抓",
                    "去哪抓",
                    "where to catch",
                    "to catch",
                    "spawn",
                    "in the wild",
                    "wild pal",
                    "encounter",
                ],
            )
            && item_mentioned(d, subject)
        {
            return None;
        }
        let mut names = Vec::new();
        for (i, _, _) in mentions {
            let key = text(&d.species[i], "key").to_owned();
            if !names.contains(&key) {
                names.push(key);
            }
        }
        let target = if names.is_empty() {
            types.map(|(id, label)| Target::Element(id, label))
                .or_else(|| practice.map(|_| Target::Training))
        } else {
            Some(Target::Named(names))
        };
        let reset_level = has(&q, &["不限等级", "所有等级", "any level", "all levels"]);
        let reset_map = has(
            &q,
            &["不限地图", "不限地区", "所有地图", "all maps", "any map"],
        );
        if target.is_none()
            && (!follow
                || (!repeated
                    && time.is_none()
                    && level.is_none()
                    && map.is_none()
                    && poi.is_none()
                    && boss.is_none()
                    && !reset_level
                    && !reset_map))
        {
            return None;
        }
        if !follow {
            *self = Self::default();
        }
        if let Some(target) = target {
            self.target = Some(target);
        }
        self.target.as_ref()?;
        if practice.is_some() && self.map.is_none() {
            self.map = Some("palpagos".into());
        }
        if let Some(map) = map {
            self.map = Some(map.into());
            self.anchor = None;
            self.region_sample = false;
        }
        if let Some(point) = poi {
            self.map = Some(text(point, "map").into());
            self.anchor = Some(text(point, "id").into());
            self.region_sample = !q.contains(&field_text(text(point, "name")));
        }
        if reset_map {
            self.anchor = None;
            self.map = None;
            self.region_sample = false;
        }
        if let Some(l) = level {
            self.level = Some(l);
        }
        if reset_level {
            self.level = None;
        }
        if time.is_some() {
            self.time = time;
        }
        if boss.is_some() {
            self.boss = boss;
        }
        if has(&q, &["副本内部", "地牢", "地下城", "dungeon"]) {
            return Some(AssistantReply::new(
                "出没表只登记地表野生刷新中心和固定头目；目前无法用这张表定位副本内部的刷新点。",
                &[],
            ));
        }
        if !d.ready("spawn-data") {
            return Some(AssistantReply::new("野生刷新资料还没有载入。", &[]));
        }
        self.lookup(d)
    }

    fn lookup(&self, d: &Data) -> Option<AssistantReply> {
        let target = self.target.as_ref()?;
        let wild = d.get("spawn-data");
        let map_data = d.get("map-data");
        let points = array(&map_data["points"]);
        let anchor = self
            .anchor
            .as_ref()
            .and_then(|id| points.iter().find(|p| text(p, "id") == id));
        let map_name = |id: &str| {
            array(&map_data["maps"])
                .iter()
                .find(|p| p["id"] == id)
                .and_then(|p| p["name"].as_str())
                .unwrap_or(if id == "worldtree" {
                    "世界树"
                } else {
                    "帕洛斯群岛"
                })
        };
        let mut filters = Vec::new();
        let label = match target {
            Target::Training => "适合边练级边捕捉的野生帕鲁".to_owned(),
            Target::Element(_, name) => format!("{name}属性帕鲁"),
            Target::Named(keys) => d
                .species
                .iter()
                .filter(|s| keys.iter().any(|k| k == text(s, "key")))
                .map(|s| text(s, "zh"))
                .collect::<Vec<_>>()
                .join("、"),
        };
        if let Some(l) = self.level {
            filters.push(l.label());
        }
        if let Some(map) = self.map.as_deref() {
            filters.push(map_name(map).into());
        }
        if let Some(a) = anchor {
            filters.push(format!(
                "以{}为中心，半径 {NEAR} 游戏坐标单位",
                text(a, "name")
            ));
        }
        if let Some(t) = self.time {
            filters.push(t.into());
        }
        let boss = self.boss.or(match target {
            Target::Element(..) | Target::Training => Some(false),
            _ => None,
        });
        if let Some(b) = boss {
            filters.push(if b { "固定头目" } else { "普通野外" }.into());
        }
        let suffix = if filters.is_empty() {
            String::new()
        } else {
            format!("（{}）", filters.join("，"))
        };
        let mut cells = BTreeMap::new();
        for species in &d.species {
            let key = text(species, "key");
            let selected = match target {
                Target::Training => true,
                Target::Element(id, _) => array(&species["types"]).iter().any(|v| v == *id),
                Target::Named(keys) => keys.iter().any(|k| k == key),
            };
            if !selected {
                continue;
            }
            for row in array(&wild["bySpecies"][key]) {
                let Some(id) = row[0].as_u64() else { continue };
                let Some(site) = wild["sites"].get(id as usize) else {
                    continue;
                };
                let Some(map) = site[0]
                    .as_u64()
                    .and_then(|i| wild["mapIds"][i as usize].as_str())
                else {
                    continue;
                };
                let (Some(x), Some(y), Some(time), Some(kind)) = (
                    site[3].as_i64(),
                    site[4].as_i64(),
                    row[3].as_u64(),
                    row[4].as_u64(),
                ) else {
                    continue;
                };
                let level = valid_level(row);
                if self.map.as_deref().is_some_and(|m| map != m)
                    || self.level.is_some_and(|l| !l.accepts(level))
                    || boss.is_some_and(|b| u64::from(b) != kind)
                    || !match self.time {
                        Some("仅夜间出现") => time == 1,
                        Some("夜间可遇") => time <= 1,
                        Some("白天可遇") => time == 0,
                        _ => true,
                    }
                {
                    continue;
                }
                if let Some(a) = anchor
                    && let (Some(px), Some(py)) = (a["x"].as_i64(), a["y"].as_i64())
                    && (px - x).pow(2) + (py - y).pow(2) > NEAR * NEAR
                {
                    continue;
                }
                let entry = Encounter {
                    key,
                    name: text(species, "zh"),
                    x,
                    y,
                    level,
                    time,
                    boss: kind == 1,
                };
                let cell = cells
                    .entry((map, x.div_euclid(CELL), y.div_euclid(CELL)))
                    .or_insert_with(|| Cell {
                        sites: BTreeSet::new(),
                        names: BTreeSet::new(),
                        example: entry,
                    });
                cell.sites.insert((x, y));
                cell.names.insert(entry.name);
                if example_order(entry) < example_order(cell.example) {
                    cell.example = entry;
                }
            }
        }
        let mut reply = AssistantReply::new("", &[]);
        let mut lines = Vec::new();
        if matches!(target, Target::Training) {
            lines.push("按下方等级条件挑选地表刷新点。未指定目标等级时，默认找比玩家低 0–5 级的普通帕鲁。可从最近传送点过去，沿途抓尚未收集的种类；图鉴没有每小时经验的实测值。".into());
        }
        if self.region_sample {
            lines.push("出没表只按整张地图分层，没有岛界或地形分区。以下按该区域已登记的高塔周边筛选，无法据此列全整座岛或整个区域。".into());
        }
        if cells.is_empty() {
            lines.push(format!("地表刷新表里没有查到符合条件的{label}{suffix}。"));
            if let Target::Named(keys) = target {
                for key in keys {
                    if array(&d.get("capture-data")["raidEggOnly"])
                        .iter()
                        .any(|v| v == key.as_str())
                    {
                        lines.push(format!(
                            "捕捉资料把{}列为团体战奖励蛋（突袭蛋）物种。普通地图上没有固定的野生捕捉坐标。可在据点的召唤祭坛使用对应团体战石板挑战首领，击败后获取奖励蛋。",
                            d.species
                                .iter()
                                .find(|s| s["key"] == key.as_str())
                                .map(|s| text(s, "zh"))
                                .unwrap_or(key)
                        ));
                        reply
                            .links
                            .push(("查看捕捉资料".into(), format!("#details/capture/{key}")));
                    }
                }
            }
        } else {
            lines.push(format!("{label}的出没地点{suffix}。每张地图按 {CELL}×{CELL} 地图坐标网格的不同刷新中心数排序，同一坐标只计一次。点数不代表同时出现的只数或刷新概率。"));
            let mut groups: BTreeMap<&str, Vec<_>> = BTreeMap::new();
            for ((map, cx, cy), cell) in cells {
                groups.entry(map).or_default().push((cx, cy, cell));
            }
            let mut remaining = 3;
            let maps = groups.len();
            for (i, (map, mut ranked)) in groups.into_iter().enumerate() {
                ranked.sort_by_key(|(cx, cy, c)| {
                    (
                        std::cmp::Reverse(c.sites.len()),
                        example_order(c.example),
                        *cx,
                        *cy,
                    )
                });
                let quota = remaining - (maps - i - 1).min(remaining);
                for (_, _, cell) in ranked.into_iter().take(quota) {
                    remaining -= 1;
                    let p = cell.example;
                    let nearest = points
                        .iter()
                        .filter(|l| l["category"] == "fast_travel" && l["map"] == map)
                        .filter_map(|l| {
                            Some((
                                ((l["x"].as_i64()? - p.x).pow(2) + (l["y"].as_i64()? - p.y).pow(2)),
                                text(l, "name"),
                                text(l, "id"),
                            ))
                        })
                        .min();
                    let near = nearest
                        .map(|(_, name, _)| format!("，距此点最近的传送点为「{name}」"))
                        .unwrap_or_default();
                    let lvl = match p.level {
                        Some((a, b)) if a == b => format!("Lv.{a}"),
                        Some((a, b)) => format!("Lv.{a}–{b}"),
                        None => "等级待核实".into(),
                    };
                    let time = match p.time {
                        0 => "昼夜均可",
                        1 => "仅夜间",
                        _ => "时段未标注",
                    };
                    let examples: Vec<_> = cell.names.iter().copied().take(4).collect();
                    let more = if cell.names.len() > examples.len() {
                        format!("等 {} 种", cell.names.len())
                    } else {
                        String::new()
                    };
                    lines.push(format!("{}（{}, {}）{}。这个网格有 {} 个符合条件的中心，收录{}{more}。所给坐标记录为{} {lvl}，{}，{time}。",map_name(map),p.x,p.y,near,cell.sites.len(),examples.join("、"),p.name,if p.boss{"固定头目"}else{"普通野外"}));
                    reply.links.push((
                        format!("定位{}（{}, {}）", p.name, p.x, p.y),
                        format!("#map/coords/{map}/{}/{}", p.x, p.y),
                    ));
                }
            }
        }
        if let Target::Named(keys) = target {
            for key in keys {
                reply.links.push((
                    format!(
                        "查看{}出没图",
                        d.species
                            .iter()
                            .find(|s| s["key"] == key.as_str())
                            .map(|s| text(s, "zh"))
                            .unwrap_or(key)
                    ),
                    format!("#spawns/{key}"),
                ));
            }
        }
        reply.text = lines.join("\n");
        Some(reply)
    }
}

/// The point a player reports is not a spawn record. Compare only like observations.
fn reported_sighting(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(q, &["spawn","sighting","目击","遇到","刷出","出没","遇见"]) ||
        has(q,&["randomizer","随机化","随机模式"]) { return None; }
    let (x,y) = q.match_indices('(').find_map(|(start,_)| {
        let (pair,_) = q[start+1..].split_once(')')?;
        let (a,b) = pair.split_once(',')?;
        Some((a.trim().parse::<i64>().ok()?,b.trim().parse::<i64>().ok()?))
    })?;
    let title=q.lines().next()?;
    let (pals,_) = crate::assistant_breeding::species_mentions(&d.species,title);
    let [(index,_,_)]=pals.as_slice() else {return None};
    let pal=&d.species[*index];
    let key=text(pal,"key");
    let spawns=d.get("spawn-data");
    let rows=array(&spawns["bySpecies"][key]);
    // When a species also has regular roaming spawns, a fixed boss point alone cannot answer this question.
    if rows.is_empty() || rows.iter().any(|r|r[4]!=1) {return None;}
    let mut points=Vec::new();
    let mut links=Vec::new();
    let mut exact=false;
    for row in rows.iter().take(10) {
        let site=&spawns["sites"][row[0].as_u64()? as usize];
        let map=spawns["mapIds"][site[0].as_u64()? as usize].as_str()?;
        let map_name=array(&d.get("map-data")["maps"]).iter()
            .find(|r|r["id"]==map).map(|r|text(r,"name")).unwrap_or(map);
        let (sx,sy)=(site[3].as_i64()?,site[4].as_i64()?);
        let label=format!("{map_name} ({sx}, {sy}) Lv.{}",row[1].as_i64()?);
        points.push(label.clone());
        links.push((label,format!("#map/coords/{map}/{sx}/{sy}")));
        exact |= x==sx && y==sy;
    }
    let note=if exact {"坐标数字吻合；还要核对是否在同一张地图。"}else{
        "该坐标与上面登记的固定点不一致。单凭这次目击不能认定为常规刷新点，也不能断言是漏洞；可核对截图、所在地图、世界随机化或服务器设置。"
    };
    Some(AssistantReply{text:format!("{}（{}）在当前地表记录中的固定头目点为{}。你提供的位置是 ({x}, {y})。{note}",
        text(pal,"zh"),text(pal,"en"),points.join("、")),links,suggestions:vec![]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    fn data() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for table in [
            "spawn-data",
            "map-data",
            "capture-data",
            "expedition-data",
            "item-catalog-data",
        ] {
            d.tables.insert(
                table.into(),
                Arc::new(
                    serde_json::from_slice(
                        &std::fs::read(root.join(format!("{table}.json"))).unwrap(),
                    )
                    .unwrap(),
                ),
            );
        }
        d
    }

    #[test]
    fn training_places_use_player_level_and_actual_spawn_coordinates() {
        let d = data();
        for q in ["玩家45级时，去哪些地点练级和抓帕鲁比较合适？",
                  "I'm level 45. Where can I catch Pals and level up?"] {
            let r = HabitatSession::default().respond(&d, q).unwrap();
            assert!(r.text.contains("Lv.40–45") && r.text.contains("普通野外"));
            assert_eq!(r.links.iter().filter(|(_, link)| link.starts_with("#map/coords/palpagos/")).count(), 3);
        }
        assert!(training_levels("想升到45级，去哪些地点练级和抓帕鲁？").is_none());
        assert!(matches!(training_levels("我30级，想抓45级的帕鲁来练级，去哪抓？"), Some(Levels::At(45))));
        assert!(matches!(training_levels(&field_text("I'm level 30. Where can I catch level 45 Pals and level up?")), Some(Levels::At(45))));
    }

    #[test]
    fn early_electric_locations_match_source_rows_and_coordinate_counts() {
        let d = data();
        for q in [
            "前期哪里能遇到比较多电系怪？",
            "前期哪裡能遇到比較多電系怪？",
            "Where can I find lots of electric-type Pals early in the game?",
        ] {
            let r = HabitatSession::default().respond(&d, q).unwrap();
            assert!(
                r.text.contains("Lv.15") && r.text.contains("伏特喵") && r.text.contains("电棘鼠"),
                "{}",
                r.text
            );
            for n in [23, 20, 17] {
                assert!(r.text.contains(&format!("有 {n} 个符合条件的中心")));
            }
            let coordinates: Vec<_> = r
                .links
                .iter()
                .filter_map(|(_, u)| u.strip_prefix("#map/coords/palpagos/"))
                .collect();
            assert_eq!(coordinates.len(), 3);
            let s = d.get("spawn-data");
            for coords in coordinates {
                let (x, y) = coords.split_once('/').unwrap();
                assert!(
                    d.species
                        .iter()
                        .filter(|p| array(&p["types"]).iter().any(|t| t == "electric"))
                        .any(|p| {
                            array(&s["bySpecies"][text(p, "key")]).iter().any(|row| {
                                let site = &s["sites"][row[0].as_u64().unwrap() as usize];
                                site[0] == 0
                                    && site[3].as_i64() == x.parse().ok()
                                    && site[4].as_i64() == y.parse().ok()
                                    && row[4] == 0
                                    && valid_level(row).is_some_and(|(_, b)| b <= 15)
                            })
                        })
                );
            }
        }
    }

    #[test]
    fn filters_species_level_world_and_time_and_exits_unrelated_topics() {
        let d = data();
        let mut c = HabitatSession::default();
        let frost = c.respond(&d, "Where can I find Frostallion?").unwrap();
        assert!(
            frost.text.contains("固定头目")
                && frost.text.contains("Lv.60")
                && frost
                    .links
                    .iter()
                    .any(|(_, url)| url == "#map/coords/palpagos/-357/509")
        );
        assert!(
            c.respond(&d, "只看世界树呢？")
                .unwrap()
                .text
                .contains("没有查到")
        );
        let raid = c.respond(&d, "贝菈诺娃出现在哪里？").unwrap();
        assert!(raid.text.contains("团体战奖励蛋"));
        let r = c.respond(&d, "前期哪里比较多雷系帕鲁？").unwrap();
        assert!(r.text.contains("23 个符合条件的中心"));
        let r = c.respond(&d, "只看低于十级的呢？").unwrap();
        assert!(
            r.text.contains("Lv.9")
                && r.text.contains("13 个符合条件的中心")
                && !r.text.contains("伏特喵"),
            "{}",
            r.text
        );
        let r = c.respond(&d, "只看世界树呢？").unwrap();
        assert!(r.text.contains("没有查到") && r.text.contains("世界树"));
        let r = c.respond(&d, "不限等级，只看夜间可遇呢？").unwrap();
        assert!(
            r.links
                .iter()
                .filter(|(_, u)| u.starts_with("#map/coords/"))
                .all(|(_, u)| u.starts_with("#map/coords/worldtree/"))
        );
        assert!(
            r.text.contains("夜间可遇") && r.text.contains("世界树") && !r.text.contains("Lv.9")
        );
        for q in [
            "想找火属性防具的设计图，去哪里拿？",
            "Where is the Feybreak Tower boss? I use an ice-type Pal.",
            "Where can I find water?",
            "I have a Lamball. Where can I find Wool?",
            "Where can I find wool from a Lamball?",
            "棉悠悠的羊毛哪里有？",
            "买喷火器官要多少钱？",
        ] {
            assert!(c.respond(&d, q).is_none(), "{q}");
        }
        let r = c.respond(&d, "天坠之地能找到哪些野生冰属性帕鲁？").unwrap();
        assert!(
            r.text.contains("没有岛界") && r.text.contains("天坠之塔") && r.text.contains("120")
        );
    }

    #[test]
    fn unverified_level_fields_are_never_used_to_filter_a_site() {
        assert!(valid_level(&serde_json::json!([1, 35, 34, 0, 0])).is_none());
        assert!(!Levels::Early.accepts(None));
        assert!(
            Levels::parse("levels 10 to 15")
                .unwrap()
                .accepts(Some((11, 15)))
        );
        assert!(
            !Levels::parse("levels 10 to 15")
                .unwrap()
                .accepts(Some((9, 12)))
        );
        assert!(Levels::parse("十至十五级").unwrap().accepts(Some((11, 15))));
        assert!(
            !Levels::parse("under level 10")
                .unwrap()
                .accepts(Some((8, 10)))
        );
    }

    #[test]
    fn coordinates_follow_the_last_target_until_a_new_subject_or_reset() {
        let d = data();
        let mut c = HabitatSession::default();
        let r = c.respond(&d, "空涡龙具体出没在哪，给我个点位").unwrap();
        for q in [
            "那它的坐标呢",
            "Where exactly is it?",
            "What are its coordinates?",
        ] {
            assert_eq!(c.respond(&d, q).unwrap().links, r.links);
        }
        assert!(c.respond(&d, "那硫磺的坐标呢？").is_none());
        let raid = c.respond(&d, "贝菈露洁的栖息地在哪").unwrap();
        assert!(raid.text.contains("突袭蛋") && !raid.text.contains("空涡龙"));
        assert!(
            HabitatSession::default()
                .respond(&d, "那它的坐标呢")
                .is_none()
        );
    }
}
