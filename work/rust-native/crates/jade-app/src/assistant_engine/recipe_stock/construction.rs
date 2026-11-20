use super::{names, number, numeric, quantity, whole_name};
use crate::assistant_engine::quantities::integers;
use crate::assistant_engine::{AssistantReply, facility_effect};
use crate::data::{Data, array, text};

const BUILD: &[&str] = &[
    "建造",
    "建起",
    "建",
    "搭建",
    "搭",
    "盖",
    "造",
    "摆",
    "摆放",
    "放置",
    "安放",
    "添",
    "添置",
    "加装",
    "架设",
    "做",
    "build",
    "construct",
];
const STOP: &[char] = &['，', ',', '。', '；', ';', '？', '?', '!', '！', '\n'];
const UNITS: &[char] = &['座', '台', '个', '间', '栋', '所', '架', '张', '口', '套'];
const NARROW: &[&str] = &[
    "只看",
    "只查",
    "只算",
    "只问",
    "只报",
    "只说",
    "只列",
    "只要",
    "只关心",
    "只需要知道",
    "只想知道",
    "单看",
    "单算",
    "仅看",
    "仅列",
    "单独核对",
];

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

fn asks_material_membership(clause: &str) -> bool {
    has(
        clause,
        &[
            "需不需要",
            "要不要",
            "用不用",
            "用得着",
            "用到",
            "需要用",
            "必须要",
            "还需要",
            "有没有",
            "也要",
            "要用",
            "还要",
            "need",
            "require",
        ],
    ) || (clause.trim_end().ends_with(['吗', '么']) && has(clause, &["需要", "得用", "要", "用"]))
}

fn plain_subject(s: &str) -> &str {
    let s = s.trim_end().trim_end_matches('的');
    ["普通版", "基础版", "普通款", "普通", "常规", "基础款"]
        .iter()
        .find_map(|word| s.strip_suffix(word))
        .unwrap_or(s)
        .trim_end()
}

fn asks_unlock(q: &str) -> bool {
    q.split(STOP).any(|q| {
        !has(
            q,
            &["科技不用", "不用列科技", "别列科技", "不问科技", "不看科技"],
        ) && has(q, &["科技", "技术等级", "解锁", "unlock", "technology"])
            && has(
                q,
                &[
                    "几", "多少", "等级", "门槛", "能学", "能开", "能建", "level",
                ],
            )
    })
}

// 第三张台子 is one bench with a sequence number; 三张台子 is three benches.
fn counted_prefix(tail: &str) -> Option<(u64, &str)> {
    let tail = tail.trim_start();
    if let Some(rest) = tail.strip_prefix('首') {
        return Some((1, rest));
    }
    let ordinal = tail.strip_prefix('第');
    let body = ordinal.unwrap_or(tail).trim_start();
    let n = integers(body)
        .first()
        .copied()
        .filter(|n| n.start == 0 && n.value > 0)?;
    Some((if ordinal.is_some() { 1 } else { n.value }, &body[n.end..]))
}

fn build_count(before: &str) -> Option<u64> {
    let prefix = before.rsplit(STOP).next()?.trim();
    let end = BUILD
        .iter()
        .filter_map(|word| {
            let at = prefix.rfind(word)?;
            if word.is_ascii() && prefix[..at].ends_with(|c: char| c.is_ascii_alphabetic()) {
                return None;
            }
            Some(at + word.len())
        })
        .max()?;
    let tail = plain_subject(&prefix[end..]).trim();
    let tail = ["的就是", "的是", "就是"]
        .iter()
        .find_map(|s| tail.strip_prefix(s))
        .unwrap_or(tail)
        .trim();
    let tail = tail.trim_end_matches(UNITS).trim();
    match tail {
        "" | "a" | "an" | "the" | "one" => Some(1),
        _ if tail.chars().all(numeric) => number(tail).filter(|n| *n > 0),
        _ => counted_prefix(tail)
            .filter(|(_, rest)| rest.trim().is_empty())
            .map(|(n, _)| n),
    }
}

// A detached count such as “两台一共” describes facilities. “两块金属锭” does not.
fn facility_count(q: &str, start: usize, end: usize) -> Option<u64> {
    let before = plain_subject(&q[..start]);
    if before.ends_with(UNITS) {
        let value = before.trim_end_matches(UNITS).trim_end();
        if let Some(n) = integers(value)
            .last()
            .filter(|n| n.end == value.len() && n.value > 0)
        {
            return Some(if value[..n.start].ends_with('第') {
                1
            } else {
                n.value
            });
        }
    }
    for clause in q[end..].split(STOP) {
        for n in integers(clause) {
            if n.value == 0 || clause[..n.start].trim_end().ends_with('第') {
                continue;
            }
            let Some(rest) = clause[n.end..].trim_start().strip_prefix(UNITS) else {
                continue;
            };
            let lead = clause[..n.start].trim_end();
            let counted_goal = lead.is_empty()
                || [
                    "建", "造", "摆", "放", "搭", "准备", "打算", "计划", "一共", "总共", "共",
                    "先", "预算",
                ]
                .iter()
                .any(|s| lead.ends_with(s));
            if counted_goal
                && (rest.trim().is_empty()
                    || [
                        "要",
                        "需要",
                        "得",
                        "共",
                        "一共",
                        "总共",
                        "合计",
                        "用",
                        "造价",
                        "的话",
                        "分别",
                        "每台",
                        "还差",
                        "能",
                        "一起",
                        "各",
                        "够",
                        "的材料",
                    ]
                    .iter()
                    .any(|w| rest.trim_start().starts_with(w)))
            {
                return Some(n.value);
            }
        }
    }
    None
}

fn excluded_facility(q: &str, a: usize, b: usize) -> bool {
    let before = plain_subject(q[..a].rsplit(STOP).next().unwrap_or(""));
    let clause = q[b..].split(STOP).next().unwrap_or("").trim_start();
    let after = [
        "的那套数据",
        "的那套材料",
        "的那一套",
        "的用料",
        "的材料",
        "的数据",
        "的数值",
        "的数",
        "那一套",
        "那套",
        "这边",
    ]
    .iter()
    .find_map(|s| clause.strip_prefix(s))
    .unwrap_or(clause)
    .trim_start();
    [
        "不是",
        "别按",
        "别算",
        "不要",
        "不用",
        "不算",
        "不看",
        "排除",
        "而非",
        "别拿",
        "别用",
        "不按",
        "不要按",
        "不要拿",
        "忽略",
        "不考虑",
        "不选",
        "别管",
        "不造",
        "不建",
        "别造",
        "别建",
        "不要建",
        "不要造",
        "不打算造",
        "不打算建",
    ]
    .iter()
    .any(|s| before.ends_with(s))
        || [
            "先不算",
            "不用算",
            "不要算",
            "不考虑",
            "先不看",
            "别算",
            "不要计入",
            "不算",
            "不要混",
            "别带",
            "先放一边",
            "先忽略",
        ]
        .iter()
        .any(|s| after.starts_with(s))
}

// A player often names a facility first, then says "在据点摆一台" in a later clause.
// Bind a pronoun/count only when no other named object follows the building verb.
fn referred_count(q: &str) -> Option<u64> {
    let mut found = None;
    for clause in q.split(STOP) {
        for verb in BUILD.iter().filter(|v| !v.is_ascii()) {
            for (i, _) in clause.match_indices(verb) {
                let tail = clause[i + verb.len()..].trim_start();
                let pronoun = ["这么", "这样", "这", "那", "它"]
                    .iter()
                    .find_map(|s| tail.strip_prefix(s));
                let body = pronoun.unwrap_or(tail).trim_start();
                let (count, tail) = counted_prefix(body).unwrap_or((1, body));
                let tail = tail.trim_start();
                let rest = tail
                    .strip_prefix(['座', '台', '个', '架', '张', '口', '套', '间', '栋', '所'])
                    .unwrap_or(tail);
                if rest == tail && pronoun.is_none() {
                    continue;
                }
                let rest = ["台子", "机器", "建筑", "设施", "工作台"]
                    .iter()
                    .find_map(|s| rest.strip_prefix(s))
                    .unwrap_or(rest)
                    .trim_start();
                if !(rest.is_empty()
                    || [
                        "要", "得", "需", "会", "用", "耗", "各", "的", "共", "能", "到", "放",
                        "在", "得花", "吗", "呢", "出来",
                    ]
                    .iter()
                    .any(|s| rest.starts_with(s)))
                {
                    continue;
                }
                if found.is_some_and(|old| old != count) {
                    return None;
                }
                found = Some(count);
            }
        }
    }
    found
}

fn asks_station_recipe(tail: &str, q: &str) -> bool {
    for (i, raw) in tail.split(STOP).enumerate() {
        let clause = raw.trim();
        if clause.is_empty() {
            continue;
        }
        // “多少级才能做？” refers to building the facility. An object after 能做
        // (“能做哪几种药”) asks about what the station can produce.
        if let Some((_, rest)) = clause.split_once("能做") {
            let rest = rest.trim();
            if !rest.is_empty()
                && !matches!(
                    rest,
                    "吗" | "呢" | "出来" | "得出来" | "这个设施" | "这台设施" | "这座设施" | "它"
                )
                || !asks_unlock(q) && rest.is_empty()
            {
                return true;
            }
        }
        let unlock_self = asks_unlock(q)
            && [
                "才能制作",
                "才能制造",
                "才能做出来",
                "能做出来",
                "才能制作出来",
            ]
            .iter()
            .any(|s| clause.trim_end_matches(['呢', '吗']).ends_with(s));
        let own_making = (i == 0 || has(clause, &["它的", "这座的", "这台的", "设施的", "本身的"]))
            && has(clause, &["制作材料", "制造材料", "制作配方", "制作成本"]);
        if !unlock_self
            && !own_making
            && has(
                clause,
                &[
                    "制作", "制造", "生产", "烹饪", "烹调", "加工", "冶炼", "研磨",
                ],
            )
        {
            return true;
        }
        if clause.contains("配方") {
            // The recipe belonging to the named facility is its own construction
            // record; a recipe available at that facility belongs to another item.
            let owned = i == 0 && clause.trim_start_matches('的').starts_with("配方");
            let own_recipe = owned
                || own_making
                || has(
                    clause,
                    &[
                        "建造配方",
                        "建筑配方",
                        "设施配方",
                        "它的配方",
                        "这台的配方",
                        "这座的配方",
                        "本身的配方",
                    ],
                );
            if !own_recipe
                || has(
                    clause,
                    &[
                        "支持哪些",
                        "可以做",
                        "能做什么",
                        "能用哪些",
                        "能生产",
                        "可制作",
                    ],
                )
            {
                return true;
            }
        }
    }
    false
}

fn material_scope<'a>(d: &'a Data, q: &str) -> Option<Vec<&'a serde_json::Value>> {
    let mut items = Vec::new();
    for clause in q.split(STOP) {
        if !has(clause, NARROW) {
            continue;
        }
        if has(
            clause,
            &[
                "全部材料",
                "全部建材",
                "所有材料",
                "所有建材",
                "完整材料",
                "完整建材",
            ],
        ) {
            return None;
        }
        let mut query = clause.to_owned();
        for (_, a, b) in facility_effect::facility_mentions(d, clause)
            .into_iter()
            .rev()
        {
            query.replace_range(a..b, " ");
        }
        for item in facility_effect::items(d, &query) {
            if !items
                .iter()
                .any(|old: &&serde_json::Value| old["id"] == item["id"])
            {
                items.push(item);
            }
        }
    }
    (!items.is_empty()).then_some(items)
}

fn construction_cost(
    d: &Data,
    facility: &serde_json::Value,
    count: u64,
    q: &str,
    en: bool,
) -> Option<AssistantReply> {
    let id = text(facility, "id");
    let name = if en {
        id.replace('_', " ")
    } else {
        text(facility, "name").to_owned()
    };
    let materials = array(&facility["materials"]);
    let scope = material_scope(d, q);
    let mut costs = Vec::new();
    for m in materials {
        if scope
            .as_ref()
            .is_some_and(|items| items.iter().all(|item| item["id"] != m[0]))
        {
            continue;
        }
        let label = if en {
            array(&d.get("item-catalog-data")["items"])
                .iter()
                .find(|r| r["id"] == m[0])
                .and_then(|r| r["en"].as_str())
                .or(m[0].as_str())
        } else {
            m[2].as_str().or(m[0].as_str())
        };
        let Some(label) = label else {
            continue;
        };
        costs.push(match m[1].as_u64() {
            Some(n) => format!("{label} × {}", u128::from(n) * u128::from(count)),
            None => format!(
                "{label} ({})",
                if en {
                    "quantity unlisted"
                } else {
                    "用量未收录"
                }
            ),
        });
    }
    let mut lines = Vec::new();
    if !costs.is_empty() {
        lines.push(if en {
            format!("Building {count} × {name} requires {}.", costs.join(", "))
        } else {
            format!("建造 {count} 座{name}需要{}。", costs.join("、"))
        });
    } else if materials.is_empty()
        && has(
            q,
            &[
                "建材",
                "用料",
                "材料",
                "耗材",
                "原料",
                "需要多少",
                "materials",
                "ingredients",
            ],
        )
    {
        lines.push(if en {
            "The facility table has no material list for this building.".into()
        } else {
            "本地设施表未列出这项设施的建造材料。".into()
        });
    }
    if let Some(level) = facility["level"]
        .as_u64()
        .filter(|_| scope.is_none() || asks_unlock(q) || has(q, &["几级", "多少级"]))
    {
        lines.push(if en {
            format!("The facility technology unlocks at level {level}.")
        } else {
            format!("这项设施科技的解锁等级为 {level} 级。")
        });
        if let Some(player) = player_level(q)
            && player < level
        {
            let diff = level - player;
            lines.push(if en {
                format!("At level {player}, you still need {diff} levels.")
            } else {
                format!("玩家当前 {player} 级，还差 {diff} 级。")
            });
        }
    } else if facility["level"].is_null()
        && has(q, &["科技", "解锁", "几级", "多少级", "level", "unlock"])
    {
        lines.push(if en {
            "The technology level is unlisted.".into()
        } else {
            "本地设施表未收录这项科技的解锁等级。".into()
        });
    }
    if let Some(items) = scope
        && !materials.is_empty()
    {
        for item in items
            .iter()
            .filter(|item| materials.iter().all(|m| m[0] != item["id"]))
        {
            let label = text(item, if en { "en" } else { "name" });
            lines.push(if en {
                format!("{label} is not a listed construction material.")
            } else {
                format!("这项设施的建造材料中不需要{label}。")
            });
        }
    }
    if lines.is_empty() {
        return None;
    }
    extra_fields(d, facility, q, en, &mut lines);
    Some(AssistantReply {
        text: lines.join(" "),
        links: vec![(name, format!("#work/facilities/{id}"))],
        suggestions: vec![],
    })
}

fn extra_fields(d: &Data, f: &serde_json::Value, q: &str, en: bool, lines: &mut Vec<String>) {
    if has(
        q,
        &[
            "多久",
            "几秒",
            "多少秒",
            "几分钟",
            "多长时间",
            "耗时",
            "建造时长",
            "建造时间",
            "how long",
            "construction time",
        ],
    ) {
        lines.push(if en {
            "The facility table does not list a fixed build duration.".into()
        } else {
            "设施表没有固定的建造耗时。".into()
        });
    }
    if has(q, &["科技点", "技术点", "technology points"]) {
        lines.push(if en {
            "The technology point cost is unlisted in the facility table.".into()
        } else {
            "本地设施表未列这项科技所需的科技点数。".into()
        });
    }
    let Some(materials) = f["materials"].as_array() else {
        return;
    };
    if materials.is_empty() || materials.iter().any(|m| !m[0].is_string()) {
        return;
    }
    for clause in q.split(STOP).filter(|c| asks_material_membership(c)) {
        let mut objects = clause.to_owned();
        for (_, a, b) in facility_effect::facility_mentions(d, clause)
            .into_iter()
            .rev()
        {
            objects.replace_range(a..b, " ");
        }
        for item in facility_effect::items(d, &objects) {
            let id = &item["id"];
            let label = text(item, if en { "en" } else { "name" });
            let listed = materials.iter().any(|m| &m[0] == id);
            let fact = match (listed, en) {
                (false, false) => format!("这项设施的建造材料中不需要{label}。"),
                (false, true) => format!("{label} is not a listed construction material."),
                (true, false) => format!("建造这项设施需要{label}。"),
                (true, true) => format!("{label} is a listed construction material."),
            };
            if !lines.contains(&fact) {
                lines.push(fact);
            }
        }
    }
}

fn sufficient(clause: &str) -> bool {
    ![
        "不够",
        "没够",
        "不足",
        "没备",
        "未备",
        "没准备",
        "未准备",
        "还没",
        "not enough",
        "not ready",
    ]
    .iter()
    .any(|m| clause.contains(m))
        && [
            "都够",
            "够了",
            "备齐",
            "备好",
            "足够",
            "enough",
            "all ready",
            "are ready",
        ]
        .iter()
        .any(|m| clause.contains(m))
}

fn player_level(q: &str) -> Option<u64> {
    let nums = integers(q);
    let mut levels = nums.iter().filter(|n| {
        let before = q[..n.start].trim_end();
        let after = q[n.end..].trim_start();
        (after.starts_with('级')
            && [
                "我",
                "我才",
                "我刚到",
                "我刚升到",
                "我升到",
                "我到了",
                "我只有",
                "我的等级是",
                "角色等级是",
                "角色",
                "角色目前",
                "人物",
                "我已经",
                "我现在",
                "我目前",
                "我等级",
                "我才刚到",
                "我才到",
            ]
            .iter()
            .any(|prefix| before.ends_with(prefix)))
            || [
                "i am level",
                "i'm level",
                "i’m level",
                "my level is",
                "i just reached level",
                "i just hit level",
                "player level",
            ]
            .iter()
            .any(|prefix| before.ends_with(prefix))
    });
    let level = levels.next()?.value;
    levels.next().is_none().then_some(level)
}

/// Identify which facility is being asked about before applying the player's level.
pub(in crate::assistant_engine) fn level_requirement(d: &Data, q: &str) -> Option<AssistantReply> {
    let asks = |part: &str| {
        [
            "能学",
            "能不能学",
            "可以学",
            "能解锁",
            "可以解锁",
            "能不能解锁",
            "够解锁",
            "还差几级",
            "还差多少级",
            "差多少级",
            "还要几级",
            "才解锁",
            "几级解锁",
            "几级才能",
            "多少级解锁",
            "解锁了吗",
            "学得了吗",
            "can i unlock",
            "can i learn",
            "is it unlocked",
            "unlocked yet",
            "levels short",
            "levels away",
            "how many more levels",
            "what level",
        ]
        .iter()
        .any(|w| part.contains(w))
    };
    if !asks(q) || !crate::assistant_discovery::handbook_version_matches(q) {
        return None;
    }
    let player = player_level(q)?;
    // The first facility is often something the player already has. Match the one in
    // the actual question, independently of whether either name is a substring.
    let mut asked = Vec::new();
    for clause in q.split(STOP).filter(|part| asks(part)) {
        for (row, start, end) in facility_effect::facilities(d, clause) {
            if clause[..start].trim_end().ends_with('在')
                && clause[end..].trim_start().starts_with(['里', '上'])
            {
                return None;
            }
            if !asked.contains(&row) {
                asked.push(row);
            }
        }
    }
    let facility = match asked.as_slice() {
        [row] => *row,
        [] => {
            // If a different named technology is missing from our catalog, do not
            // answer about a building from the player's background instead.
            let goal = q.split(STOP).find(|part| {
                [
                    "想学",
                    "想建",
                    "想造",
                    "想解锁",
                    "目标是",
                    "打算建",
                    "i want to build",
                    "i want to unlock",
                ]
                .iter()
                .any(|w| part.contains(w))
            })?;
            facility_effect::facility(d, goal)?.0
        }
        _ => return None,
    };
    let level = facility["level"].as_u64()?;
    let name = text(facility, "name");
    let id = text(facility, "id");
    let total = level.saturating_sub(player);
    let body = if q.is_ascii() {
        let name = id.replace('_', " ");
        if total > 0 {
            format!(
                "{name} unlocks at level {level}. At player level {player} you still need {total} more levels."
            )
        } else {
            format!(
                "{name} unlocks at level {level}. At level {player} you meet its level requirement."
            )
        }
    } else if total > 0 {
        format!("{name}的解锁等级是 {level} 级。你现在 {player} 级，还差 {total} 级。")
    } else {
        format!("{name}的解锁等级是 {level} 级。你现在 {player} 级，已达到等级门槛。")
    };
    Some(AssistantReply {
        text: body,
        links: vec![(name.into(), format!("#work/facilities/{id}"))],
        suggestions: vec![],
    })
}

/// Cost of constructing the named facility, independent of the recipes it can produce.
pub(in crate::assistant_engine) fn building_budget(d: &Data, q: &str) -> Option<AssistantReply> {
    let build_verb = has(q, BUILD);
    let membership = q.split(STOP).any(asks_material_membership);
    let narrow = has(q, NARROW);
    let building_fields = asks_unlock(q)
        || has(
            q,
            &[
                "建材",
                "建造",
                "造价",
                "造一",
                "建一",
                "造两",
                "建两",
                "材料",
                "用料",
                "用量",
                "建造时长",
                "耗材",
                "原料",
                "科技几级",
                "科技多少级",
                "科技等级",
                "解锁等级",
                "几级解锁",
                "多少级解锁",
                "科技点",
                "建造成本",
                "建造配方",
                "建造费用",
            ],
        );
    let amounts = has(
        q,
        &[
            "要多少",
            "需多少",
            "多少个",
            "够不够",
            "还缺",
            "还差",
            "多少份",
            "要留多少",
            "要准备多少",
            "用多少",
            "几级",
            "哪些料",
            "要不要",
            "需不需要",
            "用不用",
            "得用",
            "要几",
            "几块",
            "几根",
            "备多少",
            "合计多少",
            "留多少",
            "各多少",
        ],
    );
    if (!build_verb && !building_fields && !amounts && !membership && !narrow)
        || !crate::assistant_discovery::handbook_version_matches(q)
        || has(q, &["模组", "modded", "mods", "mod版"])
    {
        return None;
    }
    let has_stock = [
        "有",
        "还剩",
        "手头",
        "手里",
        "库存",
        "背包",
        "现有",
        "手上",
        "还缺",
        "还差",
        "有了",
        "凑够",
        "备齐",
        "我的是",
        "i have",
        "i've got",
        "in stock",
        "i own",
    ]
    .iter()
    .any(|marker| q.contains(marker));
    let cost_request = building_fields
        || narrow
        || membership
        || amounts
        || has(
            q,
            &[
                "需要什么",
                "木料",
                "石料",
                "消耗",
                "几份",
                "几个",
                "几枚",
                "各要",
                "各需",
                "各用",
                "分别要",
                "分别需",
                "多少块",
                "多少根",
                "一共多少",
                "各是多少",
                "总共多少",
                "差多少",
                "materials",
                "ingredients",
                "what level",
            ],
        );
    if !has_stock && !cost_request {
        return None;
    }
    let mut matches = facility_effect::facility_mentions(d, q);
    matches.retain(|&(_, a, b)| !excluded_facility(q, a, b));
    // A background facility may precede the one actually being priced.
    if matches.len() > 1 && !matches.iter().all(|m| m.0["id"] == matches[0].0["id"]) {
        let asked: Vec<_> = matches
            .iter()
            .copied()
            .filter(|(_, a, b)| {
                let before = q[..*a].rsplit(STOP).next().unwrap_or("");
                let after = q[*b..].split(STOP).next().unwrap_or("");
                build_count(before).is_some()
                    || has(
                        after,
                        &[
                            "科技几级",
                            "科技多少级",
                            "几级解锁",
                            "多少级解锁",
                            "要多少",
                            "造价",
                            "的建材",
                            "的材料",
                            "的用料",
                        ],
                    )
            })
            .collect();
        if !asked.is_empty() && asked.iter().all(|m| m.0["id"] == asked[0].0["id"]) {
            matches = asked;
        }
    }
    let first = matches.first().copied()?;
    let one = matches.iter().all(|m| m.0["id"] == first.0["id"]);
    let explicit: Vec<_> = matches
        .iter()
        .filter_map(|&(f, a, b)| build_count(&q[..a]).map(|n| (f, a, b, n)))
        .collect();
    let (facility, start, end, count) = match explicit.as_slice() {
        [target] => {
            let (f, a, b, n) = *target;
            (f, a, b, facility_count(q, a, b).unwrap_or(n))
        }
        [] if (cost_request || has_stock) && one => {
            let (f, a, b) = first;
            let count = facility_count(q, a, b)
                .or_else(|| referred_count(q))
                .or_else(|| {
                    let subject_question = q[b..].split(STOP).next()?.trim();
                    [
                        "的建造材料",
                        "的建造配方",
                        "建造时",
                        "建起来",
                        "本身的建造成本",
                    ]
                    .iter()
                    .any(|w| subject_question.starts_with(w))
                    .then_some(1)
                    .or_else(|| {
                        (["几级", "多少级", "等级", "科技"]
                            .iter()
                            .any(|w| subject_question.contains(w))
                            && ["才能造", "才能建", "才能摆", "能建造", "可以建", "才能搭"]
                                .iter()
                                .any(|w| subject_question.ends_with(w)))
                        .then_some(1)
                    })
                })
                .or_else(|| {
                    // A named subject and a request about its construction record do not need a build verb.
                    // A bench mentioned as a place to craft a different product has a separate recipe.
                    if player_level(q).is_some()
                        && !has(
                            q,
                            &[
                                "建材",
                                "用料",
                                "材料",
                                "需要多少",
                                "还缺",
                                "还差多少个",
                                "要留多少",
                            ],
                        )
                    {
                        return None;
                    }
                    let before = q[..a].trim_end();
                    let tail = q[b..].trim_start();
                    let owned_fields = building_fields
                        || narrow
                        || membership
                        || (amounts
                            && (facility_count(q, a, b).is_some()
                                || has(
                                    tail,
                                    &[
                                        "要多少",
                                        "需多少",
                                        "几级",
                                        "几枚",
                                        "几块",
                                        "几份",
                                        "几根",
                                        "备多少",
                                        "留多少",
                                        "一共多少",
                                        "要几",
                                        "多少个",
                                        "够不够",
                                        "用多少",
                                        "用不用",
                                        "要不要",
                                        "需不需要",
                                    ],
                                )));
                    (owned_fields
                        && (!before.ends_with(['用', '在'])
                            || has(q, &["建造它", "建它", "造它", "建造这", "造这"])))
                    .then_some(1)
                })?;
            (f, a, b, count)
        }
        _ => return None,
    };
    if !build_verb
        && !building_fields
        && !has(q, &["解锁", "科技", "几级", "多少级"])
        && !array(&facility["materials"])
            .iter()
            .filter_map(|m| m[2].as_str())
            .any(|label| q.contains(label))
        && !has(q, &["建材", "造价", "木头", "石料", "铁锭"])
        && !(membership
            && facility_effect::items(d, &format!("{} {}", &q[..start], &q[end..]))
                .iter()
                .any(|item| {
                    array(&d.get("facilities-data")["facilities"])
                        .iter()
                        .any(|f| array(&f["materials"]).iter().any(|m| m[0] == item["id"]))
                }))
    {
        return None;
    }
    if has(
        q,
        &[
            "适性",
            "适应性",
            "帕鲁要几级",
            "伙伴技能",
            "值班",
            "值守",
            "what jobs",
            "work suitability",
            "which workers",
            "required jobs",
        ],
    ) && !has(
        q,
        &[
            "建材",
            "用料",
            "材料",
            "造价",
            "建造成本",
            "还缺",
            "造一座要",
            "materials",
            "ingredients",
            "construction cost",
        ],
    ) {
        return None;
    }
    // Ignore repeated references to the bench itself ("这张修理台") while
    // retaining a real repair action such as "在修理台修理装备" in the same text.
    let tail = q[end..].replace(&q[start..end], " ");
    if [
        "维修",
        "修理",
        "修理费",
        "用它做",
        "用它制作",
        "用它加工",
        "用它生产",
        "能磨",
        "能炼",
        "能产",
        "产几",
        "能修",
        "修复费用",
        "耐久",
        "物品维修",
        "在里面做",
        "在上面做",
        "在这里做",
        "里面制作",
        "里面加工",
        "在它上面",
        "它能做",
        "它能制作",
        "生产一个",
        "生产一份",
        "能制作",
        "来做",
        "来生产",
        "制造哪",
        "拿来做",
        "上做",
        "里做",
        "上制作",
        "里制作",
        "能加工",
    ]
    .iter()
    .any(|s| tail.contains(s))
        || asks_station_recipe(&tail, q)
        || (has(
            &tail,
            &[
                "效率",
                "速度",
                "供电范围",
                "发电量",
                "持续",
                "掉落",
                "产出",
                "属性",
                "加成",
            ],
        ) && !building_fields
            && !has(q, &["建造", "搭建", "造一", "建一"]))
    {
        return None;
    }
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let name = if en {
        text(facility, "id").replace('_', " ")
    } else {
        text(facility, "name").to_owned()
    };
    if !has_stock {
        return construction_cost(d, facility, count, q, en);
    }
    let catalog = array(&d.get("item-catalog-data")["items"]);
    let inventory = format!("{}，{}", &q[..start], &q[end..]);
    let rest_ready = inventory.split(STOP).any(|c| {
        sufficient(c)
            && [
                "其他材料",
                "其它材料",
                "其余材料",
                "剩下的材料",
                "其余都",
                "其他都",
                "everything else",
                "all other materials",
            ]
            .iter()
            .any(|m| c.contains(m))
    });
    let mut missing = Vec::new();
    let mut unknown = Vec::new();
    let mut covered = Vec::new();
    let mut verified = 0;
    for material in array(&facility["materials"]) {
        let (Some(id), Some(per)) = (material[0].as_str(), material[1].as_u64()) else {
            return construction_cost(d, facility, count, q, en);
        };
        let needed = u128::from(per) * u128::from(count);
        let Some(item) = catalog.iter().find(|r| text(r, "id") == id) else {
            return construction_cost(d, facility, count, q, en);
        };
        let label = text(item, if en { "en" } else { "name" });
        let aliases = names(item);
        let mut stock = None;
        let mut ready = false;
        for clause in inventory.split(STOP) {
            for alias in &aliases {
                for (at, _) in clause
                    .match_indices(alias)
                    .filter(|(at, _)| whole_name(clause, alias, *at))
                {
                    // E.g. 精炼金属锭 and 金属锭 are different inventories.
                    if catalog.iter().any(|row| {
                        let other = text(row, "name");
                        other.len() > alias.len()
                            && other.contains(alias)
                            && clause
                                .match_indices(other)
                                .any(|(pos, _)| pos <= at && at + alias.len() <= pos + other.len())
                    }) {
                        continue;
                    }
                    if let Some(value) = quantity(clause, at, at + alias.len()) {
                        if stock.is_some_and(|old| old != value) {
                            return None;
                        }
                        stock = Some(value);
                    }
                    ready |= sufficient(clause);
                }
            }
        }
        if let Some(value) = stock {
            verified += 1;
            if u128::from(value) < needed {
                let deficit = needed - u128::from(value);
                missing.push(if en {
                    format!("{label}: need {needed}, have {value}, short by {deficit}")
                } else {
                    format!("{label}需要 {needed}，现有 {value}，还差 {deficit}")
                });
            } else {
                covered.push(if en {
                    format!("{label}: need {needed}, have {value}")
                } else {
                    format!("{label}需要 {needed}，现有 {value}，已够用")
                });
            }
        } else if !ready && !rest_ready {
            unknown.push(format!("{label} × {needed}"));
        } else {
            covered.push(if en {
                format!("{label}: need {needed}, reported ready")
            } else {
                format!("{label}需要 {needed}，按你所述已备齐")
            });
        }
    }
    if verified == 0 {
        return if cost_request {
            construction_cost(d, facility, count, q, en)
        } else {
            None
        };
    }
    let mut lines = Vec::new();
    if !missing.is_empty() {
        lines.push(if en {
            format!(
                "You cannot build {count} × {name} with this stock. {}.",
                missing.join("; ")
            )
        } else {
            format!("目前材料不够建 {count} 个{name}。{}。", missing.join("。"))
        });
    } else if unknown.is_empty() {
        lines.push(if en {
            format!("You have enough materials to build {count} × {name}.")
        } else {
            format!("材料足够建 {count} 个{name}。")
        });
    } else {
        lines.push(if en {
            format!("The quantities supplied meet those requirements for {count} × {name}.")
        } else {
            format!("已报数的材料足够建 {count} 个{name}。")
        });
    }
    if !unknown.is_empty() {
        lines.push(if en {
            format!("Also check your stock of {}.", unknown.join(", "))
        } else {
            format!("还要核对库存中是否有{}。", unknown.join("、"))
        });
    }
    if !covered.is_empty() {
        lines.push(covered.join(if en { "; " } else { "。" }));
    }
    if let Some(level) = facility["level"].as_u64() {
        let current = player_level(q);
        lines.push(match (current, en) {
            (Some(n), false) if n < level => format!(
                "该设施科技需要 {level} 级，你目前 {n} 级，还差 {} 级。",
                level - n
            ),
            (Some(n), true) if n < level => format!(
                "The technology requires level {level}. At level {n} you still need {} levels.",
                level - n
            ),
            (Some(n), false) => format!("你目前 {n} 级，已达到这项科技的 {level} 级门槛。"),
            (Some(n), true) => {
                format!("At level {n} you meet the technology's level {level} requirement.")
            }
            (None, true) => format!("This construction technology unlocks at level {level}."),
            (None, false) => format!("该设施科技的解锁等级为 {level} 级。"),
        });
    } else if has(q, &["科技", "几级", "多少级", "解锁", "level"]) {
        lines.push(if en {
            "The technology level is unlisted.".into()
        } else {
            "本地设施表未收录这项科技的解锁等级。".into()
        });
    }
    extra_fields(d, facility, q, en, &mut lines);
    Some(AssistantReply {
        text: lines.join(" "),
        links: vec![(name, format!("#work/facilities/{}", text(facility, "id")))],
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{Conversation, field_text};

    #[test]
    fn compares_player_level_to_the_requested_building_not_an_already_known_one() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        let raw = std::fs::read(dir.join("facilities-data.json")).unwrap();
        d.tables.insert(
            "facilities-data".into(),
            std::sync::Arc::new(serde_json::from_slice(&raw).unwrap()),
        );
        let ask = |q| {
            Conversation::default()
                .ask_world(q, &d, &serde_json::json!({}))
                .text
        };
        let cold =
            ask("我50級，普通飼料箱早就解鎖了。現在能學低溫保鮮飼料箱了嗎？如果還不能，差幾級？");
        assert!(
            cold.contains("低温保鲜饲料箱") && cold.contains("还差 1 级"),
            "{cold}"
        );
        let furnace = ask("我只有33级，原始炉子已经会了，可以学改良熔炉吗？");
        assert!(
            furnace.contains("改善后的炉子") && furnace.contains("还差 1 级"),
            "{furnace}"
        );
        let met = ask("我36级，能学电能帕鲁蛋孵化器吗？");
        assert!(met.contains("36 级") && met.contains("已达到"), "{met}");
        assert!(
            level_requirement(&d, &field_text("我的帕鲁刚到50级，我能学大型发电机吗？")).is_none()
        );
        assert!(
            level_requirement(&d, &field_text("我20级，在优质作业台上能学习什么配方？")).is_none()
        );
        assert!(
            level_requirement(
                &d,
                &field_text("我20级，原始炉子早就会了。能学我装的模组里的传送仓库吗？")
            )
            .is_none()
        );
    }

    #[test]
    fn building_stock_is_scoped_to_the_facility_and_preserves_unknowns() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in ["facilities-data", "item-catalog-data"] {
            let json = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            data.tables.insert(
                id.into(),
                std::sync::Arc::new(serde_json::from_slice(&json).unwrap()),
            );
        }
        let ask = |q: &str| {
            Conversation::default()
                .ask_world(q, &data, &serde_json::json!({}))
                .text
        };
        let reply = ask(
            "我刚到12级，木材和帕鲁矿碎块都备齐了，手里有4枚钉子。现在能搭一个中世纪制药台吗？",
        );
        assert!(
            reply.contains("钉子需要 5，现有 4，还差 1") && reply.contains("12 级"),
            "{reply}"
        );
        let several = ask("我有99个木材，想建两个伐木场，够吗？");
        assert!(
            several.contains("木材需要 100，现有 99，还差 1") && several.contains("石头 × 40"),
            "{several}"
        );
        let covered =
            ask("I have 50 wood, all other materials are ready. Can I build a logging site?");
        assert!(covered.contains("enough materials"), "{covered}");
        let locked = ask("我才5级，手里有50木材，其余材料也备齐了。能搭一个伐木场吗？");
        assert!(locked.contains("还差 2 级"), "{locked}");
        for q in [
            "我想弄一台磨粉机。在据点摆一台要哪些材料，科技多少级？",
            "磨粉机我还没有。搭一座要多少木板和金属锭？几级解锁？",
            "打算造一台磨粉机。材料和解锁等级是多少？",
        ] {
            let reply = ask(q);
            assert!(
                reply.contains("木板 × 10")
                    && reply.contains("金属锭 × 10")
                    && reply.contains("15 级"),
                "{q}: {reply}"
            );
        }
        let bench = ask("优质作业台我想摆两张台子。木材和钉子各要多少？");
        assert!(
            bench.contains("木材 × 40") && bench.contains("钉子 × 20") && bench.contains("11 级"),
            "{bench}"
        );
        for (q, count, cost) in [
            (
                "修理台要科技几级才能造？摆第一张台子，木材和石头各要多少？",
                1,
                10,
            ),
            (
                "我打算摆第三张修理台。这张修理台的木材和石头各要多少？",
                1,
                10,
            ),
            ("修理台要多少级才能造？石头和木材各要几份？", 1, 10),
            (
                "想问下修理台。要是在营地摆那三张台子，木材和石头各要多少？",
                3,
                30,
            ),
        ] {
            let mut chat = Conversation::default();
            let got = chat.ask_world(q, &data, &serde_json::json!({}));
            assert!(
                chat.direct_result()
                    && got.links[0].1 == "#work/facilities/Repair_Bench"
                    && got.text.contains(&format!("建造 {count} 座修理台"))
                    && got.text.contains(&format!("木材 × {cost}"))
                    && got.text.contains(&format!("石头 × {cost}"))
                    && got.text.contains("2 级"),
                "{q}: {}",
                got.text
            );
        }
        assert!(
            building_budget(
                &data,
                &field_text("我建了中世纪制药台，手里有4枚钉子，可以用它做哪种药？")
            )
            .is_none()
        );
        for q in [
            "准备摆一台优质作业台，拿它修理装备要多少金属锭？",
            "想造一台磨粉机，用它生产一份面粉要多少小麦？",
            "准备建修理台。用这张修理台修理装备要多少金币？",
            "大型发电机已经建好了。工人只有手工作业，面板没有发电适性，能去值班供电吗？设施至少要求哪种适性、几级？",
            "我想在中世纪制药台上制作药品，能做哪些配方？",
            "在磨粉机上，生产一份面粉要多少小麦？",
        ] {
            assert!(building_budget(&data, &field_text(q)).is_none(), "{q}");
        }
        for (q, id, fields) in [
            (
                "想在据点里造普通发电机。它科技多少级解锁？一台要留多少金属锭和发电器官？",
                "Power_Generator",
                &["金属锭 × 50", "发电器官 × 20", "26 级"][..],
            ),
            (
                "大型发电机先不算，我要造两台普通发电机。发电器官需要多少？",
                "Power_Generator",
                &["发电器官 × 40", "金属锭 × 100"][..],
            ),
            (
                "普通发电机先不看。两台大型发电机的造价和科技等级是多少？",
                "Large_Power_Generator",
                &["帕鲁金属锭 × 300", "发电器官 × 300", "50 级"][..],
            ),
            (
                "人力发电机。两台一共用多少木板？还要金属锭吗？",
                "Human-Powered_Generator",
                &["木板 × 10", "发电器官 × 10", "不需要金属锭"][..],
            ),
            (
                "低温保鲜饲料箱的建材是什么？要科技几级，建造时长呢？",
                "Cold_Food_Box",
                &["帕鲁金属锭 × 20", "51 级", "没有固定的建造耗时"][..],
            ),
            (
                "庄严烛台科技多少级解锁，得留多少金属锭？",
                "Majestic_Candlestick",
                &["金属锭 × 5", "喷火器官 × 2", "未收录这项科技的解锁等级"][..],
            ),
            (
                "我有95个石头和11个喷火器官，打算造改善后的炉子。材料还差多少？",
                "Improved_Furnace",
                &[
                    "石头需要 100，现有 95，还差 5",
                    "喷火器官需要 15，现有 11，还差 4",
                    "水泥 × 30",
                ][..],
            ),
            (
                "我库存里有130个石头，想建两座采石场。还缺多少木材？",
                "Stone_Pit",
                &["石头需要 100，现有 130", "木材 × 40"][..],
            ),
            (
                "普通发电机需要塑钢吗？",
                "Power_Generator",
                &["不需要塑钢"][..],
            ),
            (
                "营地要添两张修理台，科技多少级才能做？木材与石头各几份？",
                "Repair_Bench",
                &["2 级", "木材 × 20", "石头 × 20"][..],
            ),
            (
                "我要造的是优质温泉，普通温泉的数别算进去。科技几级能开，要多少水泥？",
                "High_Quality_Hot_Spring",
                &["31 级", "水泥 × 10"][..],
            ),
            (
                "一台磨粉机要到多少科技才能解锁？木板的用量呢？固定花几秒能盖好？",
                "Mill",
                &["15 级", "木板 × 10", "没有固定的建造耗时"][..],
            ),
            (
                "磨粉机配方我只记得有木板。建一台要多少块，还用不用电路板？",
                "Mill",
                &["木板 × 10", "不需要电路板"][..],
            ),
        ] {
            let mut chat = Conversation::default();
            let ans = chat.ask_world(q, &data, &serde_json::json!({}));
            assert!(
                chat.direct_result()
                    && ans
                        .links
                        .iter()
                        .any(|l| l.1 == format!("#work/facilities/{id}"))
                    && fields.iter().all(|s| ans.text.contains(s)),
                "{q}: {}",
                ans.text
            );
        }
        let narrow = ask("我想添一座优质温泉。单看水泥和石头，各要多少？");
        assert!(
            narrow.contains("水泥 × 10")
                && narrow.contains("石头 × 50")
                && !narrow.contains("木材")
                && !narrow.contains("级"),
            "{narrow}"
        );
    }
}
