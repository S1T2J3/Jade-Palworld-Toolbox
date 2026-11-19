use crate::assistant_engine::{AssistantReply, field_text};
use crate::data::{Data, array, normalize, text};
use serde_json::Value;
use std::collections::HashSet;

mod scope;
mod candidates;

const JOBS: &[&str] = &[
    "生火",
    "浇水",
    "播种",
    "发电",
    "手工作业",
    "采集",
    "伐木",
    "采矿",
    "制药",
    "冷却",
    "搬运",
    "牧场",
];
const ELEMENTS: &[(&str, &str)] = &[
    ("火", "fire"),
    ("水", "water"),
    ("草", "grass"),
    ("雷", "electric"),
    ("冰", "ice"),
    ("地", "ground"),
    ("暗", "dark"),
    ("龙", "dragon"),
    ("无", "neutral"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Goal {
    #[default]
    Combat,
    Attack,
    Defense,
    Health,
    Stats(BaseStats),
    Work,
    Job(&'static str),
    MountSpeed,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PalComparison {
    active: bool,
    goal: Goal,
    names: Vec<String>,
    excluded: Vec<String>,
    shown: Vec<String>,
    element: Option<&'static str>,
    opponent: Option<&'static str>,
    owned: bool,
    scope: scope::Scope,
}

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

fn word(q: &str, choices: &[&str]) -> bool {
    q.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|part| choices.contains(&part))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BaseStats {
    health: bool,
    attack: bool,
    defense: bool,
}

impl BaseStats {
    fn requested(q: &str) -> Option<Goal> {
        let mut stats = Self::default();
        for clause in q.split([',', '，', '。', ';', '；']) {
            let positive = [
                "不考虑",
                "不比较",
                "不比",
                "不看",
                "忽略",
                "ignore ",
                "ignoring ",
                "excluding ",
                "without ",
                "but not ",
            ]
            .iter()
            .filter_map(|marker| clause.find(marker))
            .min()
            .map_or(clause, |end| &clause[..end]);
            let all = has(
                positive,
                &[
                    "攻防血",
                    "血攻防",
                    "血防攻",
                    "攻血防",
                    "防攻血",
                    "防血攻",
                    "攻防和血",
                    "三围",
                ],
            );
            stats.health |=
                all || has(
                    positive,
                    &[
                        "生命",
                        "血量",
                        "血上限",
                        "攻血",
                        "血攻",
                        "血防",
                        "防血",
                        "hit points",
                    ],
                ) || word(positive, &["health", "hp"]);
            stats.attack |= all
                || has(positive, &["攻击", "攻防", "攻血", "血攻", "面板攻"])
                || word(positive, &["attack", "atk"]);
            stats.defense |= all
                || has(positive, &["防御", "攻防", "防攻", "血防", "防血"])
                || word(positive, &["defense", "defence", "def"]);
        }
        let mut requested = stats.fields();
        let (single, _) = requested.next()?;
        Some(if requested.next().is_none() {
            single
        } else {
            Goal::Stats(stats)
        })
    }

    fn fields(self) -> impl Iterator<Item = (Goal, &'static str)> {
        [
            (self.health, Goal::Health, "基础生命"),
            (self.attack, Goal::Attack, "基础攻击"),
            (self.defense, Goal::Defense, "基础防御"),
        ]
        .into_iter()
        .filter_map(|(included, goal, label)| included.then_some((goal, label)))
    }

    fn compare(
        self,
        rows: &[Candidate<'_>],
        show_values: bool,
        reply: &mut AssistantReply,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        for (goal, label) in self.fields() {
            let mut values = Vec::new();
            let mut winners = Vec::new();
            let mut best = None;
            let mut missing = 0;
            for &row in rows {
                if show_values {
                    add_link(reply, row);
                }
                if let Some(value) = row.value(goal) {
                    if show_values {
                        values.push(format!("{} {value}", row.name()));
                    }
                    if best.is_none_or(|old| value > old) {
                        best = Some(value);
                        winners.clear();
                    }
                    if best == Some(value) {
                        winners.push(row);
                    }
                } else {
                    missing += 1;
                    if show_values {
                        values.push(format!("{}未收录", row.name()));
                    }
                }
            }
            let detail = if show_values {
                format!("{}。", values.join("、"))
            } else {
                String::new()
            };
            if missing > 0 {
                lines.push(format!(
                    "{label}：{detail}有 {missing} 种缺少数值，无法确定全部候选中的最高值。"
                ));
            } else if let Some(best) = best {
                let upper = if show_values { winners.len() } else { 4 };
                let mut names = Vec::new();
                for row in winners.iter().take(upper) {
                    add_link(reply, *row);
                    names.push(row.name());
                }
                let extra = if winners.len() > upper {
                    format!("等 {} 种", winners.len())
                } else {
                    String::new()
                };
                let result = if winners.len() > 1 {
                    "并列最高"
                } else {
                    "最高"
                };
                lines.push(format!(
                    "{label}：{detail}{}{extra}{result}（{best}）。",
                    names.join("、")
                ));
            }
        }
        lines
    }
}

fn requested_goal(q: &str) -> Option<Goal> {
    if let Some(job) = JOBS.iter().find(|job| q.contains(**job)) {
        return Some(Goal::Job(job));
    }
    if has(q, &["挖矿", "矿工"]) {
        return Some(Goal::Job("采矿"));
    }
    for (label, terms) in [
        ("生火", &["kindling", "smelting"][..]),
        ("浇水", &["watering"][..]),
        ("播种", &["planting"][..]),
        ("发电", &["generating electricity"][..]),
        ("手工作业", &["handiwork"][..]),
        ("采集", &["gathering"][..]),
        ("伐木", &["lumbering", "logging"][..]),
        ("采矿", &["mining", "to mine", "for ores", "for ore"][..]),
        ("制药", &["medicine production"][..]),
        ("冷却", &["cooling", "refrigeration"][..]),
        ("搬运", &["transporting", "hauling"][..]),
        ("牧场", &["ranching"][..]),
    ] {
        if has(q, terms) {
            return Some(Goal::Job(label));
        }
    }
    if has(q, &["工作", "打工", "干活", "据点用", "at base", "my base"])
        || q.trim_end_matches(['?', '？', '.']).ends_with("for base")
        || word(q, &["work", "working", "workers", "worker"])
    {
        return Some(Goal::Work);
    }
    if has(
        q,
        &[
            "跑得快",
            "跑得最快",
            "跑得更快",
            "飞得快",
            "飞得最快",
            "速度",
            "跑速",
            "移速",
            "飞行",
            "坐骑",
        ],
    ) || word(
        q,
        &[
            "fastest", "mount", "mounts", "riding", "flying", "flyer", "swimming",
        ],
    ) {
        return Some(Goal::MountSpeed);
    }
    if let Some(goal) = BaseStats::requested(q) {
        return Some(goal);
    }
    if has(
        q,
        &[
            "防御",
            "抗揍",
            "扛",
            "抗打",
            "耐打",
            "耐久",
            "最肉",
            "坦度",
            "站得住",
        ],
    ) || word(q, &["tank", "tanky", "defense", "defence", "toughest"])
    {
        return Some(Goal::Defense);
    }
    if has(q, &["血量", "生命", "血厚", "血最多"]) || word(q, &["hp", "health"]) {
        return Some(Goal::Health);
    }
    if has(q, &["攻击", "输出", "伤害", "物攻", "技攻", "面板攻"])
        || word(q, &["attack", "damage", "dps"])
    {
        return Some(Goal::Attack);
    }
    if has(q, &["战斗", "打架", "对战", "实战", "站场", "打boss"])
        || word(q, &["combat", "fighting", "battle", "battles"])
    {
        return Some(Goal::Combat);
    }
    None
}

#[derive(Clone, Copy)]
struct Candidate<'a> {
    species: &'a Value,
    profile: &'a Value,
}

impl Candidate<'_> {
    fn value(&self, goal: Goal) -> Option<u64> {
        match goal {
            Goal::Attack => self.profile["attack"].as_u64(),
            Goal::Health => self.profile["hp"].as_u64(),
            Goal::Defense => self.profile["defense"].as_u64(),
            Goal::Job(job) => array(&self.profile["work"])
                .iter()
                .find(|v| v[0] == job)?
                .get(1)?
                .as_u64(),
            _ => None,
        }
    }
    fn name(&self) -> &str {
        text(self.species, "zh")
    }
    fn key(&self) -> &str {
        text(self.species, "key")
    }
}

fn add_link(reply: &mut AssistantReply, row: Candidate<'_>) {
    let route = format!("#paldex/{}", row.key());
    if !reply.links.iter().any(|(_, href)| href == &route) {
        reply.links.push((row.name().into(), route));
    }
}

fn ranking(
    rows: &[Candidate<'_>],
    goal: Goal,
    label: &str,
    reply: &mut AssistantReply,
) -> Option<String> {
    let mut ranked: Vec<_> = rows
        .iter()
        .filter_map(|p| Some((p.value(goal)?, *p)))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name().cmp(b.1.name())));
    let (best, _) = ranked.first()?;
    let tied: Vec<_> = ranked.iter().take_while(|p| p.0 == *best).collect();
    let mut names = Vec::new();
    for (_, p) in tied.iter().take(4) {
        add_link(reply, *p);
        names.push(p.name());
    }
    let extra = if tied.len() > 4 {
        format!("等 {} 种", tied.len())
    } else {
        String::new()
    };
    let notation = if matches!(goal, Goal::Job(_)) {
        "Lv."
    } else {
        ""
    };
    Some(format!(
        "{label}优先选{}{}（{}{notation}{best}）。",
        names.join("、"),
        extra,
        if tied.len() > 1 { "并列 " } else { "" }
    ))
}

impl PalComparison {
    pub(crate) fn remember_work(&mut self, answer: &AssistantReply) {
        let names: Vec<_> = answer
            .links
            .iter()
            .filter_map(|(_, href)| {
                let key = href.strip_prefix("#work/suitability/")?;
                (!key.is_empty() && !key.contains('/')).then(|| key.to_owned())
            })
            .collect();
        *self = Self::default();
        if !names.is_empty() {
            self.active = true;
            self.goal = Goal::Work;
            self.shown = names.clone();
            self.names = names;
        }
    }

    pub(crate) fn direct_result(&self) -> bool {
        self.active
    }

    /// Some lower-level field lookups would interpret a blocked Pal as the
    /// subject. Only move comparisons with an identified constraint ahead of them.
    pub(crate) fn respond_constrained(
        &mut self,
        data: &Data,
        world: &Value,
        q: &str,
    ) -> Option<AssistantReply> {
        let follow = self.active
            && !self.excluded.is_empty()
            && q.chars().count() <= 80
            && (q.starts_with(['那', '只', '换', '看', '按'])
                || has(q, &["what about", "how about", "only ", "instead"]));
        if !follow && (!candidates::has_exclusion(q) || candidates::participants(data, q).1.is_empty()) {
            return None;
        }
        self.respond(data, world, q)
    }

    pub(crate) fn respond(
        &mut self,
        d: &Data,
        world: &Value,
        question: &str,
    ) -> Option<AssistantReply> {
        let q = field_text(question.trim());
        if candidates::other_task(&q) { return None; }
        if has(
            &q,
            &[
                "装备", "饰品", "首饰", "项链", "徽章", "护符", "吊坠", "戒指", "配种", "词条",
                "配方", "合成", "武器", "配队", "掉率", "概率",
            ],
        ) || word(
            &q,
            &[
                "breed",
                "breeding",
                "weapon",
                "weapons",
                "armor",
                "accessory",
                "accessories",
                "passive",
                "passives",
                "schematic",
                "farm",
                "farming",
                "recipe",
                "catch",
                "capture",
                "drops",
                "ammo",
                "skill",
                "skills",
                "team",
            ],
        ) || has(
            &q,
            &[
                "best way",
                "best method",
                "work speed",
                "pal sphere",
                "pal oil",
                "base location",
                "帕鲁球",
                "帕鲁油",
                "帕鲁矿",
                "帕鲁金属",
                "帕鲁床",
                "帕鲁终端",
            ],
        ) {
            return None;
        }
        let goal = requested_goal(&q);
        let rank = has(
            &q,
            &[
                "最强",
                "厉害",
                "最快",
                "最硬",
                "最肉",
                "最好",
                "哪只强",
                "谁强",
                "谁更",
                "哪个强",
                "哪个更",
                "哪只更",
                "哪种更",
                "谁最",
                "排行",
                "推荐",
                "对比",
                "比较",
                "相比",
                "哪个好",
                "哪只好",
                "最适合",
                "最值得",
                "更值得",
                "谁好",
                "最能打",
                "排名第一",
                "排第一",
                "谁第一",
                "谁的",
                "哪个高",
                "谁攻击高",
                "哪个攻击高",
                "哪只攻击高",
                "比一比",
                "最高",
                "值得养",
                "值得练",
                "值得培养",
                "选哪只",
                "选哪个",
                "该选谁",
                "选谁",
                "会选谁",
                "会选什么",
                "用哪只",
                "用哪个",
                "选什么帕鲁",
                "练什么帕鲁",
            ],
        ) || word(
            &q,
            &[
                "best",
                "strongest",
                "highest",
                "toughest",
                "fastest",
                "recommend",
                "recommended",
                "recommendation",
                "better",
                "stronger",
                "compare",
            ],
        );
        let element = ELEMENTS
            .iter()
            .find(|(zh, en)| {
                q.contains(&format!("{zh}系"))
                    || q.contains(&format!("{zh}属性"))
                    || [" type", "-type", " pal", " element"]
                        .iter()
                        .any(|suffix| q.contains(&format!("{en}{suffix}")))
            })
            .map(|p| p.1);
        let owned = has(
            &q,
            &[
                "仓库",
                "我有的",
                "我已有",
                "我手里",
                "我现有",
                "我养的",
                "my pals",
                "i own",
                "my palbox",
                "my inventory",
            ],
        );
        let all_species = has(&q, &["所有物种", "全图鉴", "all species", "whole paldeck"])
            || q == "不限"
            || q == "全部";
        let all_regions = has(
            &q,
            &[
                "不限地区",
                "不限区域",
                "不限定地区",
                "不限定区域",
                "不分地区",
                "不限地图",
                "all regions",
                "regardless of region",
                "any region",
            ],
        );
        let global = all_species || all_regions;
        let scope = scope::Scope::parse(d, &q);
        let referring = q.chars().count() <= 80
            && (q.starts_with(['那', '只', '换', '看', '按'])
                || q.ends_with('呢')
                || q.ends_with("呢？")
                || q.ends_with("呢?")
                || has(
                    &q,
                    &[
                        "what about",
                        "how about",
                        "only ",
                        "for work",
                        "for combat",
                        "for riding",
                    ],
                )
                || q.trim_end_matches(['?', '？']).chars().count() <= 6);
        let (mut names, banned) = candidates::participants(d, &q);
        let ban_only = !banned.is_empty() && names.is_empty() && !rank;
        let narrow = self.active
            && ((referring && (goal.is_some() || element.is_some() || owned || scope.is_some()))
                || global || ban_only);
        let explicit_stock = !names.is_empty() && has(&q, &["我有", "我已经有", "我已有", "我现有", "我手里有", "i have ", "i own ", "i already have "]);
        let referred = if has(
            &q,
            &["前两只", "前两个", "这两只", "这两个", "这两种", "这俩"],
        ) {
            self.shown.iter().take(2).cloned().collect()
        } else if has(&q, &["第一只", "第一个", "前者"]) {
            self.shown.first().cloned().into_iter().collect()
        } else if has(&q, &["第二只", "第二个", "后者"]) {
            self.shown.get(1).cloned().into_iter().collect()
        } else {
            Vec::new()
        };
        let referred = if names.is_empty() && !referred.is_empty() {
            names = referred;
            true
        } else {
            false
        };
        // A named enemy is not a candidate to recommend against itself.
        if d.species
            .iter()
            .filter(|s| names.iter().any(|k| k == text(s, "key")))
            .any(|s| {
                ["打", "对付", "挑战", "克制", "对抗"]
                    .iter()
                    .any(|verb| q.contains(&format!("{verb}{}", text(s, "zh"))))
                    || ["against ", "counter ", "beat ", "defeat "]
                        .iter()
                        .any(|verb| q.contains(&format!("{verb}{}", normalize(text(s, "en")))))
            })
        {
            return None;
        }
        let named_comparison = names.len() >= 2
            && (rank
                || goal.is_some() && has(&q, &["分别", "各自", "多少", "对照", "分别是", "各是", " and ", " vs ", " vs. "])
                || has(&q, &["谁", "哪一只", "哪个", "哪只", "vs", "二选一"])
                || self.active && has(&q, &["和", "还是", "或", "、", " and ", " or "]));
        let direct = rank
            && (q.contains("帕鲁")
                || word(&q, &["pal", "pals", "mount", "mounts", "flyer"])
                || scope.is_some() && has(&q, &["哪只", "哪一只", "哪种"])
                || names.len() >= 2
                || element.is_some()
                || !banned.is_empty()
                || goal.is_some()
                    && (q.contains('谁') || q.contains("坐骑") || q.contains("哪只")));
        // A follow-up may omit both the subject and "帕鲁". Keep it narrow
        // so a new question about an item or production doesn't inherit a ranking.
        let new_topic = has(
            &q,
            &[
                "怎么做",
                "怎么获得",
                "怎么抓",
                "哪里抓",
                "为什么",
                "怎么解决",
                "怎么提升",
                "不够",
                "不出",
                "断产",
                "怎么办",
                "打不动",
                "能叠加",
                "几级解锁",
                "吃什么",
            ],
        );
        let next_named = self.active
            && names.len() == 1
            && (q.ends_with('呢') || q.ends_with("呢？") || q.ends_with("呢?") || rank);
        if new_topic || !(direct || named_comparison || narrow || next_named || referred) {
            return None;
        }
        let follow_candidates = self.active && named_comparison && (!rank || referring);
        if !self.active
            || (!referred && !follow_candidates && (named_comparison || direct && !narrow))
        {
            *self = Self::default();
        }
        if !names.is_empty() {
            self.names = names;
        }
        let applies_exclusion = !banned.is_empty();
        for key in banned {
            if !self.excluded.contains(&key) { self.excluded.push(key); }
        }
        if all_species {
            self.names.clear();
            self.scope = scope::Scope::default();
            self.element = None;
            self.opponent = None;
            self.owned = false;
            if !applies_exclusion { self.excluded.clear(); }
        }
        if all_regions {
            self.scope.clear_region();
        }
        if let Some(mut scope) = scope {
            if applies_exclusion { scope.consume_named_exclusions(&q); }
            self.scope.refine(scope);
        }
        if owned {
            // Explicitly listed available Pals can be compared even when no save is imported.
            self.owned = !explicit_stock;
        }
        if let Some(element) = element {
            let zh = ELEMENTS
                .iter()
                .find(|(_, value)| *value == element)
                .unwrap()
                .0;
            let counter = ["打", "对付", "克制", "对上", "挑战", "对抗"]
                .iter()
                .any(|verb| {
                    q.contains(&format!("{verb}{zh}系")) || q.contains(&format!("{verb}{zh}属性"))
                })
                || ["against ", "counter ", "beat ", "against a ", "against an "]
                    .iter()
                    .any(|verb| q.contains(&format!("{verb}{element}")));
            if counter {
                self.opponent = Some(element);
                self.element = None;
            } else {
                self.element = Some(element);
                self.opponent = None;
            }
        }
        if let Some(goal) = goal {
            self.goal = goal;
        }
        self.active = true;
        let mut reply = AssistantReply {
            text: String::new(),
            links: Vec::new(),
            suggestions: vec![],
        };
        if !d.ready("paldex-index-data") {
            reply.text = "物种属性资料还没加载完成，稍后可以再比一次。".into();
            return Some(reply);
        }
        if self.opponent.is_some() && !d.ready("combat-guide-data") {
            reply.text = "属性克制表尚未载入，稍后可以再问要打哪种属性的对手。".into();
            return Some(reply);
        }
        let stock: HashSet<_> = array(&world["inventory"]["records"])
            .iter()
            .map(|v| text(v, "speciesKey"))
            .collect();
        let mut rows = Vec::new();
        let mut excluded = Vec::new();
        for species in &d.species {
            let key = text(species, "key");
            if self.excluded.iter().any(|excluded| excluded == key) { continue; }
            if !self.names.is_empty() && !self.names.iter().any(|n| n == key) {
                continue;
            }
            if self.owned && !stock.contains(key) {
                continue;
            }
            if self
                .element
                .is_some_and(|e| !array(&species["types"]).iter().any(|t| t == e))
            {
                continue;
            }
            if let Some(opponent) = self.opponent
                && !array(&species["types"])
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|e| {
                        array(&d.get("combat-guide-data")["strong"][e])
                            .iter()
                            .any(|v| v == opponent)
                    })
            {
                continue;
            }
            if text(species, "reason").contains("无法捕获") {
                if !self.names.is_empty() {
                    excluded.push(text(species, "reason"));
                }
                continue;
            }
            let profile = &d.get("paldex-index-data")["profiles"][key];
            if profile.is_object() || matches!(self.goal, Goal::Stats(_)) {
                rows.push(Candidate { species, profile });
            }
        }
        if !self.names.is_empty() && matches!(self.goal, Goal::Stats(_)) {
            rows.sort_by_key(|p| self.names.iter().position(|key| key == p.key()));
        }
        if let Some(scoped) = self
            .scope
            .answer(d, &rows, self.goal, &self.names, self.owned, &q)
        {
            self.shown = scoped
                .links
                .iter()
                .filter_map(|(_, href)| href.strip_prefix("#paldex/").map(str::to_owned))
                .collect();
            return Some(scoped);
        }
        if rows.is_empty() {
            self.shown.clear();
            reply.text = if self.owned {
                "当前世界仓库没有符合这些条件的已登记帕鲁。可以放宽筛选，或去仓库登记你想比较的个体。".into()
            } else {
                "图鉴里没有符合当前条件的可培养帕鲁。".into()
            };
            if !excluded.is_empty() {
                reply.text.push_str(&excluded.join(" "));
            }
            if self.owned {
                reply.links.push(("查看仓库".into(), "#warehouse".into()));
            }
            reply.suggestions = vec!["比较全图鉴".into()];
            return Some(reply);
        }
        let mut lines = Vec::new();
        if !self.excluded.is_empty() {
            let banned: Vec<_> = d.species.iter().filter(|s| self.excluded.iter().any(|k| k == text(s,"key")))
                .map(|s| text(s,"zh")).collect();
            lines.push(format!("按你的限制，已从候选中排除{}。", banned.join("、")));
            if goal.is_none() && self.goal == Goal::Combat {
                if let Some(job) = match self.element {
                    Some("electric") => Some("发电"), Some("fire") => Some("生火"),
                    Some("water") => Some("浇水"), Some("ice") => Some("冷却"), _ => None,
                } && let Some(work) = ranking(&rows, Goal::Job(job), &format!("如果想找据点{job}替代，按基础适性"), &mut reply) {
                    lines.push(work);
                }
                lines.push("战斗用途另看下列攻击和防御。图鉴能比较各项基础数值，具体选哪只还取决于你能获得的物种和要打的对手。".into());
            }
        }
        if let Some(opponent) = self.opponent {
            let zh = &d.get("combat-guide-data")["elements"][opponent];
            lines.push(format!(
                "对付{}属性对手，先筛出拥有克制属性的帕鲁，实战要选对应属性的攻击技能。",
                zh.as_str().unwrap_or(opponent)
            ));
            reply
                .links
                .push(("查看属性克制".into(), "#guide/elements".into()));
        }
        if self.owned || self.element.is_some() {
            let element = ELEMENTS
                .iter()
                .find(|(_, key)| Some(*key) == self.element)
                .map(|(zh, _)| format!("{zh}属性"))
                .unwrap_or_default();
            lines.push(format!(
                "在{} {} 种{element}帕鲁中比较。",
                if self.owned {
                    "当前仓库的"
                } else {
                    "图鉴收录的"
                },
                rows.len()
            ));
        }
        let metric = match self.goal {
            Goal::Combat => vec![
                (Goal::Attack, "想要高基础攻击，"),
                (Goal::Defense, "想要高基础防御，"),
            ],
            Goal::Work => vec![
                (Goal::Job("生火"), "冶炼"),
                (Goal::Job("采矿"), "采矿"),
                (Goal::Job("搬运"), "搬运"),
            ],
            Goal::Job(job) => vec![(self.goal, job)],
            Goal::Attack => vec![(Goal::Attack, "看基础攻击")],
            Goal::Defense if has(&q, &["抗揍", "坦度", "耐打", "坦克", "tanky", "toughest"]) =>
            {
                vec![(Goal::Defense, "看基础防御"), (Goal::Health, "看基础生命")]
            }
            Goal::Defense => vec![(Goal::Defense, "看基础防御")],
            Goal::Health => vec![(Goal::Health, "看基础生命")],
            Goal::MountSpeed | Goal::Stats(_) => vec![],
        };
        if let Goal::Stats(stats) = self.goal {
            let values = !self.names.is_empty() || self.owned && rows.len() <= 10;
            lines.extend(stats.compare(&rows, values, &mut reply));
        } else if self.goal == Goal::MountSpeed {
            lines.push(
                "图鉴收录了骑乘方式，当前资料没有统一的移动速度数值，无法据此排出最快坐骑。".into(),
            );
            if self.names.is_empty() {
                scope::mount_examples(d, &rows, &q, &mut reply, &mut lines);
            } else {
                for row in &rows {
                    let skill = &d.get("partner-skills-data")[row.key()];
                    if let Some(effect) = skill["effect"].as_str() {
                        lines.push(format!("{}的伙伴技能：{effect}", row.name()));
                        add_link(&mut reply, *row);
                    }
                }
            }
            reply.suggestions = vec!["那基础攻击呢？".into()];
        } else {
            for (criterion, label) in &metric {
                if let Some(line) = ranking(&rows, *criterion, label, &mut reply) {
                    lines.push(line);
                } else if let Goal::Job(job) = criterion {
                    lines.push(format!("所选物种中没有查到能{job}的候选。"));
                }
            }
            if !self.names.is_empty() {
                for &p in rows.iter().take(6) {
                    let fields: Vec<_> = if let Goal::Job(job) = self.goal {
                        vec![p.value(self.goal).map_or_else(
                            || format!("未收录{job}适性"),
                            |v| format!("{job} Lv.{v}"),
                        )]
                    } else if self.goal == Goal::Work {
                        array(&p.profile["work"])
                            .iter()
                            .filter_map(|v| {
                                Some(format!("{} Lv.{}", v[0].as_str()?, v[1].as_u64()?))
                            })
                            .collect()
                    } else {
                        [
                            ("攻击", Goal::Attack),
                            ("生命", Goal::Health),
                            ("防御", Goal::Defense),
                        ]
                        .into_iter()
                        .filter_map(|(name, goal)| p.value(goal).map(|v| format!("{name} {v}")))
                        .collect()
                    };
                    lines.push(format!(
                        "{}：{}。",
                        p.name(),
                        if fields.is_empty() {
                            "图鉴未收录对应属性".into()
                        } else {
                            fields.join("、")
                        }
                    ));
                    if self.names.len() <= 2 && matches!(self.goal, Goal::Combat | Goal::Attack) {
                        let skill = &d.get("partner-skills-data")[p.key()];
                        if let (Some(name), Some(effect)) =
                            (skill["name"].as_str(), skill["effect"].as_str())
                        {
                            lines.push(format!("伙伴技能「{name}」{effect}"));
                        }
                    }
                    add_link(&mut reply, p);
                }
            }
            if matches!(
                self.goal,
                Goal::Attack | Goal::Combat | Goal::Health | Goal::Defense
            ) {
                let eggs = array(&d.get("capture-data")["raidEggOnly"]);
                let raid_names: Vec<_> = reply
                    .links
                    .iter()
                    .filter_map(|(name, href)| {
                        let key = href.strip_prefix("#paldex/")?;
                        eggs.iter().any(|v| v == key).then_some(name.as_str())
                    })
                    .collect();
                if !raid_names.is_empty() && !self.owned {
                    lines.push(format!(
                        "想培养{}，可以打对应团体战，孵化胜利奖励的蛋。",
                        raid_names.join("、")
                    ));
                }
                if has(&q, &["物攻", "技攻", "物理攻击", "技能攻击"]) {
                    lines.push(
                        "当前图鉴只收录一个基础攻击值，尚无单独的物攻和技攻数值可以比较。".into(),
                    );
                }
                lines.push(
                    "实战还要看对手属性和所用技能。同种帕鲁的等级、词条与强化也会拉开差距。".into(),
                );
                if self.goal == Goal::Combat && goal.is_none() {
                    lines.push("若用途是据点工作，可以指定采矿、搬运等工种；若是骑乘，可以按飞行、水上或陆地来选。".into());
                }
                reply.suggestions = vec![
                    "只看暗系".into(),
                    "只看我仓库里的".into(),
                    "据点采矿谁最强？".into(),
                ];
            } else {
                lines.push(
                    "这里按基础工作适性比较。同等级时，再看夜间是否工作和个体的工作词条。".into(),
                );
                reply.suggestions = vec!["只看我仓库里的".into(), "如果是搬运呢？".into()];
            }
        }
        if !excluded.is_empty() {
            lines.push(excluded.join(" "));
        }
        reply.text = lines.join("\n");
        self.shown = reply
            .links
            .iter()
            .filter_map(|(_, href)| href.strip_prefix("#paldex/").map(str::to_owned))
            .collect();
        Some(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{path::PathBuf, sync::Arc};

    fn fixture() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in [
            "paldex-index-data",
            "work-efficiency-data",
            "combat-guide-data",
            "partner-skills-data",
            "capture-data",
            "spawn-data",
            "enemy-data",
            "expedition-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            data.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        data
    }

    #[test]
    fn excluded_pals_never_become_the_subjects_of_a_work_lookup() {
        let data = fixture();
        let mut chat = crate::assistant_engine::Conversation::default();
        let world = json!({"id":"work"});
        let filtered = chat.ask_world("排除覆海龙和壶小象，哪只帕鲁浇水最好？", &data, &world);
        assert!(chat.direct_result() && filtered.text.contains("浇水优先选霄龙"), "{}", filtered.text);
        assert_eq!(filtered.links.len(), 1);
        assert_eq!(filtered.links[0].1, "#paldex/BlueSkyDragon");

        let remaining = chat.ask_world("Compare Fuack and Pengullet for watering, excluding Pengullet.", &data, &world);
        assert_eq!(remaining.links.len(), 1, "{}", remaining.text);
        assert_eq!(remaining.links[0].1, "#paldex/BluePlatypus");
        let next = chat.ask_world("那只看采集呢？", &data, &world);
        assert!(next.text.contains("企丸丸") && next.links.iter().all(|(_, url)| !url.ends_with("/Penguin")), "{}", next.text);

        let ordinary = chat.ask_world("Compare Fuack and Pengullet for watering.", &data, &world);
        assert!(ordinary.text.contains("standard work amount") && ordinary.links.len() == 2, "{}", ordinary.text);
        let none = chat.ask_world("Compare Fuack and Pengullet for watering, excluding Fuack and Pengullet.", &data, &world);
        assert!(none.links.is_empty() && none.text.contains("没有符合"), "{}", none.text);
        let list = chat.ask_world("Which Pal is best at watering except Jormuntide, Fuack, and Pengullet?", &data, &world);
        assert!(list.text.contains("霄龙") && list.links.len() == 1, "{}", list.text);
        assert!(["覆海龙", "冲浪鸭", "企丸丸"].iter().all(|pal| list.text.lines().next().unwrap().contains(pal)));
    }

    #[test]
    fn keeps_locked_species_out_of_rankings_and_does_not_rank_unrelated_objects() {
        let d = fixture();
        let w = json!({"id":"filters"});
        let mut c = crate::assistant_engine::Conversation::default();
        let r = c.ask_world("Now that Orserk is end game locked, which electric Pal is the second best option?", &d, &w);
        assert!(c.direct_result() && r.text.contains("曼波皇") && r.text.contains("雷云鹫") && r.text.contains("发电") &&
            r.links.iter().all(|(_,url)| url != "#paldex/ThunderDragonMan"), "{}",r.text);
        let follow = c.ask_world("那只看基础攻击呢？", &d, &w);
        assert!(!follow.links.is_empty() && follow.links.iter().all(|(_,url)| url != "#paldex/ThunderDragonMan"), "{}",follow.text);
        let mut c=PalComparison::default();
        let pair=c.respond(&d,&w,"我手里有雷角马和伏特喵。波鲁杰克斯还没解锁。比较哪个基础攻击高？").unwrap();
        assert!(pair.text.contains("雷角马") && pair.text.contains("105") && !pair.text.contains("没有符合"),"{}",pair.text);
        let more=PalComparison::default().respond(&d,&w,"排除唤冬兽，唤夜兽和空涡龙相比谁更抗打？").unwrap();
        assert!(more.links.iter().any(|(_,u)|u.ends_with("/IceHorse_Dark")) && more.links.iter().all(|(_,u)|!u.ends_with("/IceHorse")),"{}",more.text);
        for q in ["帕鲁的第一个据点怎么搬家？","两只帕鲁不停掉帧，哪个设置最好？","What is the best controller setting for Pals?"] {
            assert!(PalComparison::default().respond(&d,&w,q).is_none(),"{q}");
        }
    }

    #[test]
    fn strength_ranks_raiseable_species_and_applies_inventory_and_element_filters() {
        let d = fixture();
        let mut compare = PalComparison::default();
        let world = json!({"inventory":{"records":[{"speciesKey":"IceHorse_Dark"},{"speciesKey":"IceHorse"}]}});
        let first = compare.respond(&d, &world, "最强帕鲁是谁").unwrap();
        assert!(first.text.contains("贝菈露洁") && first.text.contains("贝菈诺娃"));
        assert!(first.text.contains("奥沧鲸") && !first.text.contains("枯星龙"));
        assert!(first.text.contains("孵化胜利奖励的蛋"));
        let pair = compare
            .respond(&d, &world, "前两只哪个更适合打架？")
            .unwrap();
        assert!(pair.text.contains("伙伴技能") && pair.links.len() == 2);
        let _ = compare.respond(&d, &world, "最强帕鲁是谁").unwrap();
        let owned = compare.respond(&d, &world, "只比较我仓库里的").unwrap();
        assert!(owned.text.contains("唤冬兽") && owned.text.contains("唤夜兽"));
        let dark = compare.respond(&d, &world, "只看暗系").unwrap();
        assert!(dark.text.contains("唤夜兽") && !dark.text.contains("唤冬兽"));
        assert!(compare.respond(&d, &world, "给暗系帕鲁的饰品").is_none());
        assert!(compare.respond(&d, &world, "干活速度怎么提升？").is_none());
    }

    #[test]
    fn compares_named_pals_changes_metric_and_distinguishes_opponent_from_own_type() {
        let d = fixture();
        let world = Value::Null;
        let mut compare = PalComparison::default();
        let pair = compare
            .respond(&d, &world, "唤夜兽和空涡龙哪个更强")
            .unwrap();
        assert!(pair.text.contains("并列 140"));
        assert!(pair.text.contains("唤夜兽：攻击 140、生命 140、防御 135"));
        assert_eq!(pair.links.len(), 2);
        let work = compare.respond(&d, &world, "只看采集").unwrap();
        assert!(work.text.contains("采集优先选空涡龙（Lv.8）"));
        let speed = compare.respond(&d, &world, "那谁飞得快呢").unwrap();
        assert!(speed.text.contains("没有统一的移动速度数值") && speed.text.contains("空涡龙"));
        let counter = PalComparison::default()
            .respond(&d, &world, "打暗系最强帕鲁是谁")
            .unwrap();
        assert!(
            counter.text.contains("对付暗属性")
                && counter
                    .links
                    .iter()
                    .any(|(_, href)| href == "#paldex/JetDragon")
        );
        assert!(!counter.text.contains("贝菈露洁"));
    }

    #[test]
    fn patch_timing_is_distinct_from_island_membership_and_remembers_the_scope() {
        let d = fixture();
        let world = Value::Null;
        let mut c = PalComparison::default();
        let after = c
            .respond(&d, &world, "Strongest pal after Feybreak?")
            .unwrap();
        assert!(
            after.text.contains("更新之后")
                && after.text.contains("未按岛内出现")
                && after.text.contains("贝菈露洁"),
            "{}",
            after.text
        );
        assert!(
            after.text.contains("1.0.5") && after.text.contains("Bastigor") && c.direct_result()
        );
        let zh = PalComparison::default()
            .respond(&d, &world, "天坠后最强帕鲁是谁？")
            .unwrap();
        assert!(zh.text.contains("更新之后"));
        let local = c.respond(&d, &world, "天墜之地哪隻最強？").unwrap();
        assert!(
            local.text.contains("范围限定在天坠之地")
                && local.text.contains("攻击 130")
                && !local.text.contains("贝菈露洁"),
            "{}",
            local.text
        );
        assert!(
            local
                .links
                .iter()
                .any(|(_, url)| url == "#enemies/GYM_SnowTigerBeastman")
        );
        let job = c.respond(&d, &world, "What about for base work?").unwrap();
        assert!(job.text.contains("冷却 Lv.8") && job.text.contains("天坠之地"));
        let mount = c.respond(&d, &world, "What about riding?").unwrap();
        assert!(mount.text.contains("冰属性的大锤") && mount.text.contains("速度表"));
        let island = PalComparison::default()
            .respond(&d, &world, "Best pal on Sakurajima island?")
            .unwrap();
        assert!(island.text.contains("月花众") && !island.text.contains("霜牙王"));
        let past = PalComparison::default()
            .respond(
                &d,
                &world,
                "Which pal was strongest in Feybreak patch 0.4.11?",
            )
            .unwrap();
        assert!(past.text.contains("0.4.11") && !past.text.contains("贝菈露洁"));
    }

    #[test]
    fn open_rank_uses_the_requested_candidates_and_metric() {
        let d = fixture();
        let world = json!({"inventory":{"records":[{"speciesKey":"IceHorse"},{"speciesKey":"IceHorse_Dark"}]}});
        let mut c = PalComparison::default();
        let owned = c
            .respond(&d, &world, "Strongest of my pals after Feybreak?")
            .unwrap();
        assert!(
            owned.text.contains("你的仓库")
                && owned.text.contains("唤夜兽")
                && !owned.text.contains("霜牙王"),
            "{}",
            owned.text
        );
        let dual = c.respond(&d, &world, "Jetragon or Frostallion?").unwrap();
        assert!(dual.text.contains("点名的候选") && !dual.text.contains("霜牙王"));
        let simple = PalComparison::default()
            .respond(&d, &world, "Which pal has the highest base attack?")
            .unwrap();
        assert!(simple.text.contains("基础攻击") && !simple.text.contains("冶炼"));
        let fly = PalComparison::default()
            .respond(&d, &world, "Recommend a flying mount")
            .unwrap();
        assert!(fly.text.contains("飞行骑乘例子") && !fly.text.contains("陆地骑乘例子"));
        let restricted = PalComparison::default()
            .respond(&d, &world, "Best pal on Feybreak excluding raid pals?")
            .unwrap();
        assert!(restricted.text.contains("排除") && !restricted.text.contains("贝菈"));
        assert!(
            c.respond(&d, &world, "Best way to farm high quality pal oil?")
                .is_none()
        );
        assert!(
            c.respond(&d, &world, "Best pal against Jetragon?")
                .is_none()
        );
    }

    #[test]
    fn followups_keep_named_candidates_and_can_release_only_the_region_filter() {
        let d = fixture();
        let mut c = PalComparison::default();
        assert!(
            c.respond(&d, &Value::Null, "企丸王和捣蛋猫的工作能力相比如何？")
                .is_some()
        );
        let mining = c.respond(&d, &Value::Null, "要是只比较采矿呢？").unwrap();
        assert!(
            mining.text.contains("企丸王：采矿 Lv.3")
                && mining.text.contains("捣蛋猫：采矿 Lv.1")
                && !mining.text.contains("手工作业")
        );
        let mut c = PalComparison::default();
        let mining = c
            .respond(
                &d,
                &Value::Null,
                "What is the best pal to mine with in Feybreak?",
            )
            .unwrap();
        assert!(mining.text.contains("采矿适性是 Lv.5"));
        c.respond(&d, &Value::Null, "天坠之地之后哪只帕鲁最强？")
            .unwrap();
        let defense = c
            .respond(&d, &Value::Null, "不限定地区，只看基础防御。")
            .unwrap();
        assert!(
            defense.text.contains("奥沧鲸")
                && !defense.text.contains("高塔")
                && !defense.text.contains("看基础生命")
        );
    }

    #[test]
    fn compares_each_requested_stat_for_two_or_three_candidates() {
        let data = fixture();
        for q in [
            "只比较基础生命、攻击、防御，唤冬兽和异构格里芬在每项里谁更高，双方的数值分别是多少？",
            "Compare Frostallion and Shadowbeak using only their base HP, attack and defense. Which Pal is higher in each stat, and what are the values?",
        ] {
            let mut c = PalComparison::default();
            let r = c.respond(&data, &Value::Null, q).unwrap();
            assert_eq!(r.links.len(), 2);
            assert_eq!(
                r.text.lines().collect::<Vec<_>>(),
                [
                    "基础生命：唤冬兽 140、异构格里芬 120。唤冬兽最高（140）。",
                    "基础攻击：唤冬兽 140、异构格里芬 120。唤冬兽最高（140）。",
                    "基础防御：唤冬兽 120、异构格里芬 140。异构格里芬最高（140）。"
                ]
            );
        }
        let mut c = PalComparison::default();
        let r=c.respond(&data,&Value::Null,"Compare Frostallion, Jetragon and Shadowbeak on Feybreak for base attack and defense.").unwrap();
        assert!(
            r.text.contains("未按岛内出现")
                && r.text.contains("唤冬兽、空涡龙并列最高（140）")
                && r.text.contains("异构格里芬最高（140）")
        );
        assert!(!r.text.contains("基础生命"));
        assert_eq!(
            requested_goal("只比防御，不考虑生命和攻击"),
            Some(Goal::Defense)
        );
        let world = json!({"inventory":{"records":[{"speciesKey":"IceHorse_Dark"},{"speciesKey":"IceHorse"}]}});
        let r = PalComparison::default()
            .respond(
                &data,
                &world,
                "Which of my pals has the highest HP and attack?",
            )
            .unwrap();
        assert!(r.text.contains("并列最高（140）") && !r.text.contains("贝菈露洁"));
    }

    #[test]
    fn missing_values_do_not_create_a_winner() {
        let mut data = fixture();
        let table = Arc::make_mut(data.tables.get_mut("paldex-index-data").unwrap());
        table["profiles"]["IceHorse"]["hp"] = Value::Null;
        let r = PalComparison::default()
            .respond(
                &data,
                &Value::Null,
                "唤冬兽和异构格里芬的生命和防御相比呢？",
            )
            .unwrap();
        let hp = r.text.lines().find(|l| l.starts_with("基础生命")).unwrap();
        assert!(
            hp.contains("唤冬兽未收录") && hp.contains("异构格里芬 120") && hp.contains("无法确定")
        );
        assert!(r.text.contains("异构格里芬最高（140）"));
    }
}
