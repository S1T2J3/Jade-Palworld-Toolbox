use super::*;

#[derive(Clone, Debug)]
pub(super) struct DropContext {
    key: String,
    count: u64,
    level: Option<u64>,
    base: bool,
    items: Vec<String>,
}

fn exact_percent(v: &Value) -> Option<(u128, u128)> {
    let s = v.as_number()?.to_string();
    let (a, b) = s.split_once('.').unwrap_or((&s, ""));
    let scale = 10u128.checked_pow(b.len().try_into().ok()?)?;
    let value = a
        .parse::<u128>()
        .ok()?
        .checked_mul(scale)?
        .checked_add(if b.is_empty() { 0 } else { b.parse().ok()? })?;
    let denominator = 100u128.checked_mul(scale)?;
    (value <= denominator).then_some((value, denominator))
}
fn decimal(p: u128, d: u128) -> String {
    if p % d == 0 {
        return (p / d).to_string();
    }
    let digits = d.ilog10() as usize;
    let s = format!("{}.{:0digits$}", p / d, p % d);
    s.trim_end_matches('0').to_owned()
}

#[derive(Clone, Copy)]
enum Goal {
    Chance(f64),
    Expected(f64),
}

fn requested_goal(q: &str) -> Option<Goal> {
    for (i, _) in q.char_indices() {
        let tail = &q[i..];
        let Some(word) = [
            "至少达到",
            "能不能达到",
            "能不能到",
            "能不能有",
            "会不会到",
            "会不会有",
            "是否达到",
            "是否有",
            "能达到",
            "能到",
            "能有",
            "够不够",
            "达到",
            "超过",
            "超出",
            "高于",
            "低于",
            "大于",
            "小于",
            "不低于",
            "不超过",
            "至少有",
            "至少",
            "不到",
            "不满",
            "不及",
            "平均有",
            "期望有",
            "higher than ",
            "lower than ",
            "more than ",
            "less than ",
            "at least ",
            "at most ",
            "greater than ",
            "above ",
            "below ",
            "exceed ",
            "over ",
            "reach ",
            "under ",
            ">",
            "<",
            "≥",
            "≤",
        ]
        .into_iter()
        .find(|s| tail.starts_with(s)) else {
            continue;
        };
        let arg = tail[word.len()..].trim_start();
        let end = arg
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(arg.len());
        let (n, suffix) = if end > 0 {
            let Some(n) = arg[..end].parse::<f64>().ok().filter(|v| v.is_finite()) else {
                continue;
            };
            (n, arg[end..].trim_start())
        } else if let Some(n) = integers(arg).into_iter().next().filter(|n| n.start == 0) {
            (n.value as f64, arg[n.end..].trim_start())
        } else {
            continue;
        };
        if starts(suffix, &["%", "％", "percent"]) {
            return Some(Goal::Chance(n));
        }
        if has(
            q,
            &[
                "期望", "预期", "平均", "均值", "expected", "expect ", "average",
            ],
        ) && starts(
            suffix,
            &["个", "份", "枚", "件", "items", "units", "copies", "souls"],
        ) {
            return Some(Goal::Expected(n));
        }
    }
    None
}

fn relation(v: f64, target: f64) -> &'static str {
    if (v - target).abs() <= 8.0 * f64::EPSILON * v.abs().max(target.abs()).max(1.0) {
        "等于"
    } else if v < target {
        "低于"
    } else {
        "高于"
    }
}

fn result(
    d: &Data,
    row: &Value,
    n: u64,
    uniform: bool,
    odds: bool,
    goal: Option<Goal>,
) -> Option<String> {
    let id = row[0].as_str()?;
    let (lo, hi) = (u128::from(row[1].as_u64()?), u128::from(row[2].as_u64()?));
    let (p, den) = exact_percent(&row[3])?;
    if lo > hi {
        return None;
    }
    let single = if lo == hi {
        lo.to_string()
    } else {
        format!("{lo}–{hi}")
    };
    let n = u128::from(n);
    let hits_min = if p == den { n * lo } else { 0 };
    let max = if p == 0 { 0 } else { n * hi };
    let expect_lo = (n * lo).checked_mul(p)?;
    let expect_hi = (n * hi).checked_mul(p)?;
    let ev = if expect_lo == expect_hi {
        format!("期望 {} 个", decimal(expect_lo, den))
    } else if uniform {
        let doubled = expect_lo.checked_add(expect_hi)?.checked_mul(5)?;
        format!(
            "按你指定的等概率数量假设，期望 {} 个",
            decimal(doubled, den.checked_mul(10)?)
        )
    } else {
        format!(
            "期望在 {}–{} 个之间；表中没有各数量的权重，无法确定单一期望值",
            decimal(expect_lo, den),
            decimal(expect_hi, den)
        )
    };
    let mut line = format!(
        "{}：每次 {}% 掉 {single} 个。{n} 次{ev}，保底 {hits_min} 个，最多 {max} 个。",
        d.item_name(id),
        row[3]
    );
    if odds && lo > 0 {
        let prob = p as f64 / den as f64;
        let at_least = if n == 0 || p == 0 {
            0.0
        } else if p == den {
            1.0
        } else {
            -((n as f64) * (-prob).ln_1p()).exp_m1()
        };
        line.push_str(&format!(
            "若各次独立，至少掉一次的概率为 1 − (1 − {prob})^{n}，约 {:.4}%。",
            at_least * 100.0
        ));
        if let Some(Goal::Chance(target)) = goal {
            let cmp = if target == 100.0 && (n == 0 || p < den) {
                "低于"
            } else {
                relation(at_least * 100.0, target)
            };
            line.push_str(&format!("该概率{cmp} {target}%。"));
        }
    }
    if let Some(Goal::Expected(target)) = goal {
        let (a, b) = (expect_lo as f64 / den as f64, expect_hi as f64 / den as f64);
        let low = relation(a, target);
        let high = relation(b, target);
        if uniform || low == high {
            let order = if uniform {
                relation((a + b) / 2., target)
            } else {
                low
            };
            line.push_str(&format!("期望数量{order} {target} 个。"));
        } else {
            line.push_str(&format!(
                "这个范围跨过或接触 {target} 个，需知道各数量的权重才能比较期望。"
            ));
        }
    }
    Some(line)
}

pub(super) fn respond(
    d: &Data,
    q: &str,
    prior: Option<&DropContext>,
) -> Option<(AssistantReply, DropContext)> {
    let follow = prior.is_some() && followup(q);
    let action = has(
        q,
        &[
            "打",
            "击败",
            "击倒",
            "击杀",
            "杀",
            "讨伐",
            "刷",
            "掉落",
            "掉率",
            "掉宝",
            "掉几个",
            "掉多少",
            "kill",
            "defeat",
            "hunt",
            "farming",
            "drop",
            "loot",
            "encounter",
        ],
    );
    let math = has(
        q,
        &[
            "平均",
            "期望",
            "预期",
            "保底",
            "必出",
            "保证",
            "至少",
            "最多",
            "上限",
            "下限",
            "expected",
            "expect",
            "average",
            "guarantee",
            "at least",
            "at most",
            "minimum",
            "maximum",
            "on average",
            "how many",
            "多少",
            "机会",
            "可能性",
            "几率",
            "概率",
            "chance",
            "odds",
            "probability",
        ],
    );
    if !follow && !(action && math) {
        return None;
    }
    if has(
        q,
        &[
            "采购",
            "预算",
            "商人",
            "购买",
            "买",
            "商店",
            "buy ",
            "budget",
            "merchant",
            "掉落倍率",
            "倍率",
            "drop multiplier",
            "drop rate bonus",
            "掉落加成",
            "增加掉落",
            "配种",
            "繁殖",
            "孵化",
            "据点工作",
            "育种",
            "breeding",
            "经验",
            "输出",
            "伤害",
            "命中率",
            "耗时",
            "捕获率",
            "捕捉率",
            "experience",
            "damage",
            "capture rate",
            "catch rate",
        ],
    ) {
        return None;
    }
    let (mentions, _) = species_mentions(&d.species, q);
    let mut keys = Vec::new();
    for (i, _, _) in &mentions {
        let k = text(&d.species[*i], "key");
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    let same = prior.filter(|p| keys.is_empty() || keys == [p.key.as_str()]);
    let key = match keys.as_slice() {
        [k] => *k,
        [] if follow => same?.key.as_str(),
        _ => return None,
    };
    let pal = d.species.iter().find(|s| s["key"] == key)?;
    let base = has(
        q,
        &[
            "基础掉落",
            "基础档",
            "基础表",
            "base drop",
            "base tier",
            "base loot",
            "base table",
        ],
    ) || follow && same.is_some_and(|p| p.base);
    let tier_data = array(&d.get("paldex-data")[key]["drops"]);
    let mut level = None;
    let mut count = None;
    let nums = integers(q);
    let goal = requested_goal(q);
    for n in &nums {
        let before = clause_before(&q[..n.start]);
        let after = q[n.end..].trim_start();
        if starts(after, &["%", "％", "percent"]) {
            continue;
        }
        if starts(after, &["级"]) || ends(before, &["level", "lv.", "lv", "lvl"]) {
            level = Some(n.value);
            continue;
        }
        if ends(before, &["至少", "至少掉", "至少出", "最少掉", "at least"]) {
            continue;
        }
        if starts(after, &["只", "头", "场", "次"])
            || ends(
                before,
                &[
                    "打",
                    "打了",
                    "打满",
                    "击倒",
                    "击败",
                    "击杀",
                    "杀",
                    "刷",
                    "hunt",
                    "hunt down",
                    "kill",
                    "killing",
                    "killed",
                    "defeat",
                    "defeating",
                    "farm",
                    "hunting",
                    "after",
                    "across",
                    "over",
                ],
            )
            || mentions
                .iter()
                .any(|(_, s, _)| *s >= n.end && q[n.end..*s].trim().is_empty())
            || starts(after, &["kills", "defeats", "encounters", "times"])
        {
            if count.is_some_and(|v| v != n.value) {
                return None;
            }
            count = Some(n.value);
        }
    }
    if count.is_none() && follow && nums.len() == 1 && level.is_none() && goal.is_none() {
        count = Some(nums[0].value);
    }
    let count = count.or(if follow { prior.map(|p| p.count) } else { None })?;
    let level = level.or(if follow {
        same.and_then(|p| p.level)
    } else {
        None
    });
    let mut ids = Vec::new();
    let catalog = array(&d.get("item-catalog-data")["items"]);
    let possible: Vec<_> = tier_data
        .iter()
        .flat_map(|g| array(&g[1]))
        .filter_map(|r| r[0].as_str())
        .collect();
    for item in catalog {
        let id = text(item, "id");
        if !possible.contains(&id) {
            continue;
        }
        let zh = field_text(text(item, "name"));
        let en = field_text(text(item, "en"));
        if occurs(q, &zh) || occurs(q, &en) || (!en.is_empty() && occurs(q, &format!("{en}s"))) {
            ids.push(id.to_owned());
        }
    }
    if follow
        && !action
        && keys.is_empty()
        && ids.is_empty()
        && nums.is_empty()
        && !has(
            q,
            &[
                "保底",
                "均值",
                "期望",
                "概率",
                "机会",
                "可能性",
                "几率",
                "chance",
                "odds",
                "probability",
                "上限",
                "下限",
                "最高",
                "保证",
                "average",
                "expect",
                "guarantee",
                "at least",
                "at most",
            ],
        )
    {
        return None;
    }
    if ids.is_empty() {
        let mut matched = item_candidates(catalog, q, false);
        matched.retain(|i| {
            !text(i, "id").is_empty()
                && !mentions.iter().any(|(_, start, end)| {
                    let span = &q[*start..*end];
                    occurs(span, &field_text(text(i, "name")))
                        || occurs(span, &field_text(text(i, "en")))
                })
        });
        if let [item] = matched.as_slice() {
            ids.push(text(item, "id").into());
        } else if follow
            && !has(q, &["全部", "所有", "全表", "all drops", "all items"])
            && let Some(prev) = same
        {
            ids = prev.items.clone();
        }
    }
    let context = DropContext {
        key: key.to_owned(),
        count,
        level,
        base,
        items: ids.clone(),
    };
    let mut r = AssistantReply::new("", &[]);
    link(
        &mut r,
        format!("{}掉落图鉴", text(pal, "zh")),
        format!("#paldex/{key}"),
    );
    if has(
        q,
        &[
            "头目",
            "首领",
            "狂暴",
            "突袭",
            "解体",
            "屠宰",
            "首杀",
            "boss",
            "alpha",
            "raid",
            "butcher",
            "disassembl",
            "first kill",
        ],
    ) {
        r.text = format!(
            "{}的普通掉落表和头目、首杀、解体等奖励口径不同。当前这项运算仅支持普通击倒，不能据普通表计算你指定的奖励。",
            text(pal, "zh")
        );
        return Some((r, context));
    }
    let mut groups: Vec<_> = tier_data
        .iter()
        .filter_map(|g| Some((g[0].as_u64()?, array(&g[1]))))
        .collect();
    groups.sort_by_key(|(lvl, _)| *lvl);
    if let Some(lvl) = level {
        let eligible = groups.iter().rposition(|(threshold, _)| *threshold <= lvl);
        groups = eligible.map(|i| vec![groups[i]]).unwrap_or_default();
    } else if base {
        groups.retain(|(l, _)| *l == 0);
    }
    let mut lines = vec![format!(
        "按{}的普通击倒掉落表和默认倍率，计算 {count} 次。{}",
        text(pal, "zh"),
        if groups.len() > 1 {
            "未指定怪物等级，以下各档分别计算，同一次击倒只用适用档。"
        } else {
            ""
        }
    )];
    let uniform = has(
        q,
        &[
            "按均匀",
            "假设均匀",
            "假设等概率",
            "assuming uniform",
            "assume uniform",
            "equally likely quantities",
        ],
    );
    let requested_min = nums.iter().find(|n| {
        ends(
            q[..n.start].trim_end(),
            &[
                "至少",
                "至少掉",
                "至少出",
                "至少有",
                "至少掉落",
                "至少获得",
                "至少拿到",
                "至少得到",
                "at least",
            ],
        ) && !starts(q[n.end..].trim_start(), &["%", "％", "percent"])
    });
    let chance_request = matches!(goal, Some(Goal::Chance(_)))
        || has(
            q,
            &[
                "机会",
                "可能性",
                "概率",
                "几率",
                "chance",
                "odds",
                "probability",
                "at least once",
            ],
        );
    let odds = requested_min.is_none_or(|n| n.value == 1)
        && (requested_min.is_some_and(|n| n.value == 1) || chance_request);
    if groups.is_empty() {
        lines.push("没有符合条件的普通掉落档，无法核算。".into());
    }
    if chance_request && !odds {
        lines.push("你问的是多次命中或累计多件物品的概率。下列数据给出每次掉落和期望值，不能把它们当作达到该门槛的概率。".into());
    }
    for (threshold, rows) in groups {
        lines.push(if threshold == 0 {
            "基础档".into()
        } else {
            format!("Lv.{threshold} 起的档位")
        });
        let mut present = Vec::new();
        for row in rows {
            let id = row[0].as_str().unwrap_or("");
            if !ids.is_empty() && !ids.iter().any(|v| v == id) {
                continue;
            }
            present.push(id);
            if let Some(line) = result(d, row, count, uniform, odds, goal) {
                lines.push(line);
            } else {
                lines.push(format!(
                    "{}的数量范围或概率资料不足，无法计算。",
                    d.item_name(id)
                ));
            }
        }
        for id in &ids {
            if !present.contains(&id.as_str()) {
                lines.push(format!(
                    "这一档未列{}，无法据此计算其掉落期望。",
                    d.item_name(id)
                ));
            }
        }
    }
    r.text = lines.join("\n");
    Some((r, context))
}
