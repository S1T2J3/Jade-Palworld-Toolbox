use super::*;

fn measure(job: &str) -> &'static str {
    match job {
        "搬运" => "单趟基础搬运量（件）",
        "牧场" => "牧场等级基础参数",
        _ => "基础工作效率（标准工作量）",
    }
}

fn table_reply(message: String) -> AssistantReply {
    reply(
        message,
        vec![("工作效率计算器".into(), "#work/efficiency".into())],
    )
}

// The general quantity scanner intentionally treats x/y as a fraction and x-y
// as a signed quantity. In an explicitly ranked table those can separate ranks.
fn rank_question(q: &str) -> String {
    let q = rank_text(q);
    let digit = |c: char| c.is_ascii_digit() || "一二两三四五六七八九十".contains(c);
    let mut rank_list = String::new();
    for (i, c) in q.char_indices() {
        if "-–~至/、,".contains(c)
            && q[..i].trim_end().ends_with(|v| digit(v) || v == '级')
            && !(c == ',' && q[..i].trim_end().ends_with('级'))
            && q[i + c.len_utf8()..].trim_start().starts_with(digit)
        {
            rank_list.push('、');
        } else {
            rank_list.push(c);
        }
    }
    rank_list
}

fn table_ranks(q: &str) -> Vec<Quantity> {
    let nums = integers(q);
    let mut marked = ranks(q);
    // A level suffix may be shared by a whole sequence ("一、三和五级").
    for seq in nums.chunk_by(|a, b| {
        matches!(
            q[a.end..b.start].trim().trim_start_matches('级').trim(),
            "、" | "和"
                | "跟"
                | "与"
                | "及"
                | "到"
                | "升"
                | "升到"
                | "升至"
                | "升为"
                | "降到"
                | "变为"
                | "→"
                | "对比"
                | "比"
                | "vs"
                | "to"
                | "and"
        )
    }) {
        let first = seq[0];
        let bridge = &q[first.end..seq.last().unwrap().end];
        if seq.len() > 1
            && (seq.iter().any(|n| marked.contains(n))
                || has(bridge, &["升到", "升至", "升为", "降到", "→"])
                || q[..first.start].trim_end().ends_with('从'))
        {
            for &level in seq {
                let suffix = q[level.end..].trim_start();
                if !suffix.starts_with([
                    '只', '个', '件', '份', '颗', '趟', '次', '张', '瓶', '秒', '分',
                ]) && !marked.contains(&level)
                {
                    marked.push(level);
                }
            }
        }
    }
    marked.retain(|n| {
        let before = q[..n.start].trim_end();
        if n.value == 1
            && (before.ends_with(['每', '哪', '这'])
                || before.ends_with("一级")
                || q[n.end..].starts_with("级一级"))
        {
            return false; // 每一级 and 一级一级 ask for a series, not Lv.1.
        }
        let before = ["达到", "到了", "升到", "为", "是"]
            .iter()
            .find_map(|s| before.strip_suffix(s))
            .unwrap_or(before)
            .trim_end()
            .trim_end_matches("等级")
            .trim_end();
        ![
            "玩家",
            "人物",
            "角色",
            "帕鲁本体",
            "科技",
            "基地",
            "据点",
            "伙伴技能",
            "我现在",
            "我已",
            "player",
            "player level",
            "i'm level",
        ]
        .iter()
        .any(|s| before.ends_with(s))
    });
    marked.sort_by_key(|n| n.start);
    for i in 1..marked.len() {
        let last = marked[i - 1];
        let n = &mut marked[i];
        let before = q[last.end..n.start].trim_end();
        let relative = ["再升", "再提高", "再加", "提升", "升", "下"]
            .iter()
            .any(|s| before.ends_with(s));
        let lower = ["再降", "降低", "降", "上", "前"]
            .iter()
            .any(|s| before.ends_with(s));
        // 升到一级 names a target; 再升一级 is an increment.
        if (relative || lower)
            && (n.value == 1 || before.ends_with("再升") || before.ends_with("再降"))
        {
            n.value = if relative {
                last.value.saturating_add(n.value)
            } else {
                last.value.saturating_sub(n.value)
            };
        }
    }
    marked
}

fn lookup(job: &str, table: &Value, levels: &[u64]) -> String {
    let limit = table.as_array().map_or(0, Vec::len);
    let values = levels
        .iter()
        .map(|&n| match level_value(table, n) {
            Some(value) => format!("Lv.{n} 为 {value}"),
            None => format!("Lv.{n} 未收录（本地表覆盖 Lv.1–{limit}）"),
        })
        .collect::<Vec<_>>()
        .join("，");
    format!("{job}的{}：{values}。", measure(job))
}

fn change(from: u64, to: u64, old: u64, new: u64) -> String {
    let verb = if new >= old { "提升" } else { "下降" };
    let difference = if new >= old { "增加" } else { "减少" };
    let percent = compact(100. * new.abs_diff(old) as f64 / old as f64);
    let ratio = compact(new as f64 / old as f64);
    format!(
        "从 Lv.{from} 到 Lv.{to}，基础值{difference} {}，相对初值{verb} {percent}%（{ratio} 倍）。",
        new.abs_diff(old)
    )
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let table_named = has(
        q,
        &[
            "适性",
            "适应性",
            "效率表",
            "基础值",
            "基础数值",
            "表值",
            "表里的",
            "表中",
            "基础参数",
            "标准工作量",
            "基础效率",
            "基础工作",
            "工作量",
            "基础产出",
            "基础能力",
            "base efficiency",
            "work suitability",
            "suitability level",
        ],
    );
    let metric = has(
        q,
        &[
            "效率",
            "百分",
            "提升多少",
            "提高多少",
            "相差",
            "差多少",
            "多多少",
            "多了多少",
            "增加多少",
            "减少多少",
            "增幅",
            "倍率",
            "倍",
            "快多少",
            "搬几件",
            "一趟",
            "搬多少",
            "每趟",
            "单趟",
            "每次搬",
            "搬运量",
            "搬运容量",
            "产出参数",
            "对应的值",
            "基准值",
            "工作速度",
            "efficiency",
            "per trip",
            "carry per",
            "percent",
            "increase",
            "ratio",
            "times",
        ],
    );
    if !table_named && !metric {
        return None;
    }
    if has(
        q,
        &[
            "哪里抓",
            "在哪抓",
            "哪里捉",
            "哪只帕鲁",
            "推荐帕鲁",
            "怎么升级",
            "如何升级",
            "怎么提升",
            "如何提升",
            "需要几只",
            "顿饭",
            "科技几级",
            "几级解锁",
            "how many pals",
        ],
    ) {
        return None;
    }
    let base_only = has(
        q,
        &[
            "不算词条",
            "不计词条",
            "忽略词条",
            "不考虑词条",
            "不看词条",
            "不带词条",
            "无词条",
            "没有词条",
            "不带任何词条",
            "不算加成",
            "不计加成",
            "忽略加成",
            "只看基础",
            "只比较基础",
            "只看适性",
            "只看适应性",
            "without passives",
            "ignoring passives",
        ],
    );
    if !base_only && has(q, &["叠加", "词条", "passive", "stack"]) {
        return None;
    }
    let actual_output = has(
        q,
        &[
            "每小时",
            "每分钟",
            "要多久",
            "几秒",
            "多少秒",
            "几分钟",
            "耗时",
            "完成时间",
            "实际产量",
            "per hour",
        ],
    );
    if actual_output && !table_named {
        return None;
    }
    let book = d.get("work-efficiency-data")["levels"].as_object()?;
    let q = rank_question(q);
    let mut jobs: Vec<_> = crate::assistant_engine::JOBS
        .iter()
        .filter_map(|&(job, _, aliases)| {
            let table = book.get(job)?;
            let pos = std::iter::once(job)
                .chain(aliases.iter().copied())
                .filter_map(|s| q.find(s))
                .min()?;
            Some((pos, job, table))
        })
        .collect();
    jobs.sort_by_key(|(i, ..)| *i);
    if jobs.is_empty() {
        return None;
    }
    let mut levels = table_ranks(&q);
    if levels.is_empty() {
        return None;
    }
    // "A 是 B 的几倍" asks for A/B. Earlier clauses often repeat both levels
    // in another order. Repetition must not turn a comparison into an upgrade.
    if let [(_, job, table)] = jobs.as_slice()
        && has(&q, &["几倍", "多少倍", "how many times", "times as"])
    {
        for clause in q.rsplit(['。', '?', '？', '；', ';', '，', ',']) {
            let operands = table_ranks(clause);
            let [left, right] = operands.as_slice() else {
                continue;
            };
            if !has(clause, &["几倍", "多少倍", "times"]) {
                continue;
            }
            let bridge = &clause[left.end..right.start];
            if !has(
                bridge,
                &[
                    "是",
                    "相当于",
                    "times",
                    "compared to",
                    "compared with",
                    "relative to",
                ],
            ) {
                continue;
            }
            let (Some(l), Some(r)) = (
                level_value(table, left.value),
                level_value(table, right.value),
            ) else {
                continue;
            };
            if r == 0 {
                return None;
            }
            let mut message = format!(
                "{} {l} ÷ {r} = {} 倍。",
                lookup(job, table, &[left.value, right.value]),
                compact(l as f64 / r as f64)
            );
            if has(
                &q,
                &[
                    "百分",
                    "多少%",
                    "相差",
                    "增加多少",
                    "多了多少",
                    "增幅",
                    "提高",
                    "提升",
                ],
            ) {
                message.push_str(&format!(
                    "相对于 Lv.{}，Lv.{} {} {}，幅度为 {}%。",
                    right.value,
                    left.value,
                    if l >= r { "增加" } else { "减少" },
                    l.abs_diff(r),
                    compact(100. * l.abs_diff(r) as f64 / r as f64)
                ));
            }
            return Some(table_reply(message));
        }
    }
    if jobs.len() == 1 && levels.iter().any(|n| n.value != levels[0].value) {
        let mut unique = Vec::new();
        for level in levels {
            if !unique.iter().any(|q: &Quantity| q.value == level.value) {
                unique.push(level);
            }
        }
        levels = unique;
    }
    // Two jobs can be given one CURRENT level each ("生火3级和浇水4级").
    // They are then two separate observations rather than a shared upgrade path.
    if let ([(p1, k1, t1), (p2, k2, t2)], [n1, n2]) = (jobs.as_slice(), levels.as_slice())
        && ((*p1 < n1.start && n1.end <= *p2 && *p2 < n2.start)
            || (n1.end <= *p1 && *p1 < n2.start && n2.end <= *p2))
        && !has(
            &q[n1.end..n2.start],
            &["升到", "升级到", "升至", " → ", " to "],
        )
    {
        let mut msg = format!(
            "{}\n{}",
            lookup(k1, t1, &[n1.value]),
            lookup(k2, t2, &[n2.value])
        );
        let (Some(a), Some(b)) = (level_value(t1, n1.value), level_value(t2, n2.value)) else {
            return Some(table_reply(msg));
        };
        if has(
            &q,
            &[
                "增幅",
                "提升多少",
                "升幅",
                "提升比例",
                "increase",
                "improvement",
            ],
        ) {
            msg.push_str("要比较各自的提升比例，还要知道各自从几级开始。");
            return Some(table_reply(msg));
        }
        if [*k1, *k2].iter().any(|j| matches!(*j, "搬运" | "牧场")) {
            msg.push_str("这些工种的计量含义不同，直接比较表值无法断定哪项工作完成更快。");
        } else {
            let cmp = match a.cmp(&b) {
                std::cmp::Ordering::Greater => *k1,
                std::cmp::Ordering::Less => *k2,
                std::cmp::Ordering::Equal => "",
            };
            msg.push_str(&if cmp.is_empty() {
                "二者相同。".into()
            } else {
                format!("{cmp}较高，基础数值相差 {}。", a.abs_diff(b))
            });
        }
        return Some(table_reply(msg));
    }
    let mut lines = Vec::new();
    let mut factors = Vec::new();
    let last_job_pos = jobs.last()?.0;
    let shared = jobs.len() == 1
        || levels.len() <= 2
        || levels.iter().all(|n| n.end <= jobs[0].0)
        || levels.iter().all(|n| n.start >= last_job_pos);
    for (i, &(pos, job, table)) in jobs.iter().enumerate() {
        let end = jobs.get(i + 1).map_or(q.len(), |j| j.0);
        let mut part: Vec<_> = levels
            .iter()
            .filter(|n| shared || (n.start >= pos && n.end <= end))
            .map(|n| n.value)
            .collect();
        if part.is_empty() {
            lines.push(format!("要查{job}的表值，还需要该工种的适性等级。"));
            continue;
        }
        let first = part[0];
        let last = *part.last()?;
        let by_rank = part.len() == 2
            && first != last
            && has(
                &q,
                &[
                    "逐级",
                    "每级",
                    "每一级",
                    "每一档",
                    "每档",
                    "各级",
                    "各等级",
                    "各档",
                    "一级一级",
                ],
            );
        if by_rank {
            let limit = table.as_array()?.len() as u64;
            let (low, high) = (first.min(last), first.max(last));
            part = (low.max(1)..=high.min(limit)).collect();
            if low == 0 {
                part.insert(0, 0);
            }
            if high > limit {
                part.push(high);
            }
            if first > last {
                part.reverse();
            }
            if high > limit + 1 {
                lines.push(format!(
                    "{job} Lv.{}–{high} 的数值未收录。",
                    low.max(limit + 1)
                ));
            }
        }
        lines.push(lookup(job, table, &part));
        if part.len() == 1 {
            if has(
                &q,
                &["增幅", "提高多少", "提升多少", "相差", "几倍", "变化比例"],
            ) && level_value(table, first).is_some()
            {
                lines.push(format!("计算{job}的变化还需要另一个适性等级。"));
            }
            continue;
        }
        if part.len() > 2
            && has(
                &q,
                &[
                    "逐级",
                    "每一级",
                    "每一步",
                    "每档",
                    "每一档",
                    "每级",
                    "相邻",
                    "两两",
                ],
            )
            && has(
                &q,
                &["差", "提升", "提高", "增加", "增幅", "百分", "变化", "下降"],
            )
        {
            for pair in part.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if let (Some(old), Some(new)) = (level_value(table, a), level_value(table, b))
                    && old > 0
                {
                    lines.push(change(a, b, old, new));
                }
            }
        }
        let reverse = levels.len() == 2
            && has(
                &q[levels[0].end..levels[1].start],
                &["比", "相比于", "compared to"],
            )
            && !has(&q[levels[0].end..levels[1].start], &["对比", "比较"]);
        let (from, to) = if reverse {
            (last, first)
        } else {
            (first, last)
        };
        let (Some(old), Some(new)) = (level_value(table, from), level_value(table, to)) else {
            continue;
        };
        if old == 0 {
            continue;
        }
        lines.push(change(from, to, old, new));
        factors.push((job, new, old));
    }
    if factors.len() > 1 && factors.len() == jobs.len() {
        let best = factors
            .iter()
            .max_by(|a, b| (a.1 as u128 * b.2 as u128).cmp(&(b.1 as u128 * a.2 as u128)))?;
        let winners: Vec<_> = factors
            .iter()
            .filter(|v| v.1 as u128 * best.2 as u128 == best.1 as u128 * v.2 as u128)
            .map(|v| v.0)
            .collect();
        lines.push(if winners.len() == factors.len() {
            "这些工种相对初值的变化比例相同。".into()
        } else {
            format!("按相对初值的变化比例比较，{}最大。", winners.join("、"))
        });
    }
    if actual_output {
        lines.push(
            "这些是无加成的等级表值；实际耗时或单位时间产量还取决于具体任务、设施及加成。".into(),
        );
    }
    Some(table_reply(lines.join("\n")))
}

fn level_value(table: &Value, level: u64) -> Option<u64> {
    table
        .get(usize::try_from(level.checked_sub(1)?).ok()?)?
        .as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn work_table_resolves_stated_levels_units_and_partial_data() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        d.tables.insert(
            "work-efficiency-data".into(),
            Arc::new(
                serde_json::from_slice(
                    &std::fs::read(root.join("work-efficiency-data.json")).unwrap(),
                )
                .unwrap(),
            ),
        );
        let cases: &[(&str, &[&str])] = &[
            (
                "同样是干手工作业，适应性1级升到2级，效率表的基础值从多少变到多少？按初值算提高百分之几？",
                &["Lv.1 为 50", "Lv.2 为 80", "增加 30", "60%", "1.6 倍"],
            ),
            (
                "我现在玩家等级42，查发电三级工作适应性的基础参数。",
                &["Lv.3 为 400"],
            ),
            (
                "工作表中，冷却一级、三级和五级分别是多少？",
                &["Lv.1 为 50", "Lv.3 为 140", "Lv.5 为 400"],
            ),
            (
                "播种适应性从二升到四，每级的基础值呢？",
                &["Lv.2 为 70", "Lv.3 为 100", "Lv.4 为 140", "100%"],
            ),
            (
                "浇水四级比二级的基础工作量提高多少百分比？",
                &["增加 70", "100%"],
            ),
            (
                "搬运工作适应性 6/7 级时每趟能搬多少？",
                &["单趟基础搬运量", "Lv.6 为 70", "Lv.7 为 120"],
            ),
            (
                "牧场适性九级到十一级，各等级的基础参数是多少？能用它算每小时实际产量吗？",
                &[
                    "牧场等级基础参数",
                    "Lv.9 为 28",
                    "Lv.10 为 30",
                    "Lv.11 未收录",
                    "实际耗时或单位时间产量",
                ],
            ),
            (
                "三级采矿和四级搬运的基础表值各是多少？",
                &["采矿", "Lv.3 为 100", "Lv.4 为 20", "计量含义不同"],
            ),
            (
                "不算词条加成，只查发电九级到十八级，每一级的基础值是多少？",
                &["Lv.9 为 3000", "Lv.10 为 4000", "Lv.11–18 的数值未收录"],
            ),
            (
                "搬运适性四级再升一级，单趟能多多少？",
                &["Lv.4 为 20", "Lv.5 为 40", "增加 20", "100%"],
            ),
        ];
        for &(q, facts) in cases {
            let mut chat = Conversation::default();
            let a = chat.ask_world(q, &d, &serde_json::json!({}));
            assert!(
                chat.direct_result()
                    && a.links.iter().any(|l| l.1 == "#work/efficiency")
                    && facts.iter().all(|s| a.text.contains(s)),
                "{q}: {}",
                a.text
            );
        }
        for q in [
            "采矿3级帕鲁哪里抓？",
            "我科技等级达到5级，牧场的基础效率是多少？",
            "玩家等级5，角色等级6，手工效率受这些影响吗？",
            "制药三级的帕鲁推荐哪只帕鲁？",
        ] {
            assert!(answer(&d, &field_text(q)).is_none(), "{q}");
        }
    }
}
