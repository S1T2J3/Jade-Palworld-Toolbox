//! Palworld 1.0 patch notes, "Adjusted Condensation": https://paldb.cc/v1.0.0
//! Same-species material, individual progress: https://palworld.wiki.gg/wiki/Pal_Essence_Condenser
use super::{
    AssistantReply,
    quantities::{Quantity, meter_integers},
};
use crate::data::Data;

const STEPS: [u64; 4] = [4, 8, 12, 24];
const CUMULATIVE: [u64; 5] = [0, 4, 12, 24, 48];
fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|w| q.contains(w))
}
fn begins(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|w| q.starts_with(w))
}
fn ends(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|w| q.ends_with(w))
}
fn reply(s: String, en: bool) -> AssistantReply {
    AssistantReply {
        text: s,
        links: vec![(
            if en {
                "Pal Essence Condenser"
            } else {
                "帕鲁浓缩机"
            }
            .into(),
            "#work/facilities/Pal_Essence_Condenser".into(),
        )],
        suggestions: vec![],
    }
}
const CLAUSE_BREAKS: &[char] = &['，', '。', ',', '.', ';', '；', '?', '？', '!', '！', '\n'];
fn nearby(s: &str) -> &str {
    s.rsplit(CLAUSE_BREAKS)
        .next()
        .unwrap_or(s)
        .trim()
}
fn after(s: &str) -> &str {
    s.split(CLAUSE_BREAKS)
        .next()
        .unwrap_or(s)
        .trim()
}
fn trim_unit(s: &str) -> &str {
    s.trim_start_matches([' ', '-', '–', '‐'])
}
fn star(q: &str, n: Quantity) -> bool {
    begins(trim_unit(&q[n.end..]), &["星", "★", "⭐", "star"])
        || ends(q[..n.start].trim_end(), &["rank", "星级", "星等"])
}

// Quantities can share a clause ("三只本体和五只素材"). A star adjective
// such as "0 星" still belongs to the noun phrase of the preceding headcount.
fn count_context<'a>(q: &'a str, nums: &[Quantity], i: usize) -> &'a str {
    let end = nums[i + 1..].iter().find(|&&n| !star(q, n)).map_or(q.len(), |n| n.start);
    &q[nums[i].end..end]
}

fn material_description(s: &str) -> bool {
    // Stop before a question or the cost of an upgrade. In "三只帕鲁需要
    // 多少素材", the noun 素材 describes the cost rather than the three Pals.
    let end = ["还要", "还需", "还差", "需要", "多少", "几只", "几份", "得用", "要用", "消耗", "喂给", "投给",
        "够不够", "够吗", "够用", "能不能", "能否", "能升", "可以", "是不是", "是否"]
        .iter().filter_map(|w| s.find(w)).min().unwrap_or(s.len());
    let label = s[..end].trim_end();
    if has(label, &["不当素材", "不当材料", "不作素材", "不作材料", "不是素材", "不是材料"]) {
        return false;
    }
    ends(label, &["素材", "材料", "祭品", "祭品用的"])
        || has(label, &["当材料", "作材料", "作为材料", "当素材", "作素材", "作为素材",
            "当祭品", "as fodder", "as donors"])
}

#[derive(Clone, Copy, PartialEq)]
enum Stock {
    Donors,
    All,
    Unclear,
}

pub(super) fn answer(_data: &Data, q: &str) -> Option<AssistantReply> {
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let feeding_to_rank = has(q, &["星", "★", "star"])
        && has(q, &["喂到", "喂成", "喂满", "升到", "升至", "升满", "raise to", "rank up to"])
        && has(q, &["同种", "本体", "素材", "只", "same species", "keeper", "donor", "copies"]);
    if !feeding_to_rank && !has(
        q,
        &[
            "浓缩",
            "凝缩",
            "condens",
            "升星",
            "升满星",
            "满星",
            "四星",
            "4星",
            "4-star",
            "four-star",
            "4 star",
            "four star",
        ],
    ) {
        return None;
    }
    let context = has(
        q,
        &[
            "几只",
            "多少只",
            "多少个",
            "几份",
            "只",
            "同种",
            "素材",
            "祭品",
            "几星",
            "星级",
            "几档",
            "多少",
            "还差",
            "够吗",
            "进度",
            "喂",
            "献祭",
            "how many",
            "how much",
            "copies",
            "fodder",
            "duplicates",
            "sacrific",
            "donor",
            "keeper",
            "short",
            "enough",
            "rank up",
            "max out",
            "how far",
            "how close",
            "more do i need",
            "progress",
        ],
    );
    if !context
        || has(
            q,
            &[
                "工作适应性",
                "工作速度",
                "被动",
                "个体值加",
                "伙伴技能",
                "强度",
                "技能果实",
                "技能果",
                "哪一只",
                "优先",
                "哪个更",
                "which is better",
                "partner skill",
                "work suitability",
                "skill fruit",
                "which should i",
                "which pal should",
                "stat boost",
                "work speed",
                "build the condenser",
                "建造浓缩机",
                "浓缩机的建材",
                "浓缩机的材料",
                "解锁",
                "科技点",
            ],
        )
    {
        return None;
    }
    let table = if en {
        "In Palworld 1.0 each step costs 4 / 8 / 12 / 24 uncondensed same-species donors (0★→1★→2★→3★→4★)."
    } else {
        "1.0 版从 0 星依次升到 1、2、3、4 星，每档消耗 4、8、12、24 只未浓缩的同种素材。"
    };
    let clarify =
        |s: &str, e: &str| Some(reply(format!("{table}\n{}", if en { e } else { s }), en));
    let ranked_material = q
        .split(CLAUSE_BREAKS)
        .any(|part| {
            let unranked = has(
                part,
                &[
                    "没浓缩",
                    "没被浓缩",
                    "没有浓缩",
                    "未浓缩",
                    "没升星",
                    "未升星",
                    "零星",
                    "0星",
                    "uncondensed",
                    "unranked",
                    "not condensed",
                ],
            );
            !unranked
                && has(
                    part,
                    &[
                        "浓缩过的素材",
                        "带星素材",
                        "浓缩素材本身",
                        "有星的素材",
                        "部分浓缩的素材",
                        "已经浓缩的素材",
                        "ranked donor",
                        "starred donor",
                        "condensed donor",
                        "condensed fodder",
                        "condensed pal as",
                        "condensed pals as",
                    ],
                )
                || !unranked
                    && has(
                        part,
                        &["浓缩过", "已浓缩", "已经浓缩", "升过星", "有星级", "带星"],
                    )
                    && has(
                        part,
                        &[
                            "当材料",
                            "作材料",
                            "作为材料",
                            "当素材",
                            "作素材",
                            "作为素材",
                            "祭品",
                            "喂给",
                        ],
                    )
        });
    if ranked_material {
        return clarify(
            "用已升星或已有浓缩进度的帕鲁当素材时，折算取决于素材自身的星级和已积累进度。选中要保留的本体，再放入同种素材核对预览；不能只按素材头数算。",
            "Ranked or partly condensed donors carry progress. Select the same-species donor and check the projected progress on the keeper; its headcount alone is insufficient.",
        );
    }
    let nums = meter_integers(q);
    let mut used = vec![false; nums.len()];
    let mut rank_numbers = Vec::new();
    let mut meter = None;
    for (i, n) in nums.iter().copied().enumerate() {
        if i + 1 < nums.len() && matches!(q[n.end..nums[i + 1].start].trim(), "/" | "⁄" | "／") {
            if meter.is_some() {
                return clarify(
                    "请分别给出每只本体的星级和当前进度。",
                    "Please give the current rank and progress for each keeper separately.",
                );
            }
            meter = Some((n.value, nums[i + 1].value));
            used[i] = true;
            used[i + 1] = true;
        } else if !used[i] && star(q, n) {
            if ends(&q[..n.start], &["下", "再升"]) && &q[n.start..n.end] == "一" {
                used[i] = true; // “下一星” refers to the next step, not to rank one.
                continue;
            }
            let rank_suffix = trim_unit(&q[n.end..]);
            let clause = after(count_context(q, &nums, i));
            let rank_in_stock = ends(nearby(&q[..n.start]), &["只", "头", "个"])
                && !has(clause, &["本体", "主力", "要保留"])
                && has(clause, &["同种", "同类", "当材料", "当素材", "可消耗", "库存"]);
            let ranked_donor = rank_in_stock || ["stars", "star", "星级", "星", "★"]
                .iter()
                .find_map(|s| rank_suffix.strip_prefix(s))
                .is_some_and(|s| {
                    begins(
                        s.trim_start_matches([' ', '的']),
                        &["素材", "材料", "祭品", "捐赠者", "donor", "fodder", "sacrifice"],
                    )
                });
            if ranked_donor && n.value > 0 {
                return clarify(
                    "带星素材还要计入其已积累的浓缩进度。请核对这些素材放入后本体的预览进度，再计算普通素材缺口。",
                    "Ranked donors carry their stored condensation progress. Check the projected progress after adding them before counting the remaining plain donors.",
                );
            }
            if ranked_donor {
                // An unranked donor is worth one point. Its rank is not the keeper's.
                used[i] = true;
                continue;
            }
            if n.value > 4 {
                return clarify(
                    "游戏内浓缩最高 4 星，请确认目标星级。",
                    "The maximum is four stars. Check the requested rank.",
                );
            }
            rank_numbers.push((i, n));
            used[i] = true;
        }
    }
    let max = has(
        q,
        &[
            "满星",
            "升满",
            "浓缩满",
            "满浓缩",
            "满级",
            "满阶",
            "拉满",
            "max",
            "fully condens",
            "full condens",
        ],
    );
    let mut from = None;
    let mut goal = None;
    if rank_numbers.len() > 2
        || (max
            && rank_numbers.len() > 1
            && rank_numbers.iter().all(|(_, n)| n.value < 4)
            && has(
                q,
                &[
                    "分别",
                    "一只是",
                    "另一只",
                    "another",
                    "one at",
                    "each at",
                    "one is",
                    "另一隻",
                ],
            ))
    {
        return clarify(
            "几只本体的星级不同。请按本体分别列出当前星级、该档进度和目标星级。",
            "These keepers have different starting ranks. Specify each keeper’s current rank, partial progress and target.",
        );
    }
    for &(i, n) in &rank_numbers {
        let before = nearby(&q[..n.start]);
        let after = trim_unit(&q[n.end..]);
        // “刚升到二星” is a completed step. Its destination is the keeper's
        // starting rank for the next calculation, even if a new target follows.
        let completed = ends(before, &[
            "刚到", "刚到达", "刚升到", "刚升至", "刚升成", "已经到", "已升到",
            "已升至", "已经升到", "已经升至", "已经升成", "刚刚升到", "刚刚升至",
            "just reached", "already reached", "just reached rank", "already reached rank",
            "just raised to", "already raised to", "just upgraded to", "already upgraded to",
        ]);
        let is_target = ends(
            before,
            &[
                "到",
                "至",
                "升成",
                "想要",
                "目标",
                "变成",
                "for",
                "into",
                "to",
                "target",
                "reach",
                "reach rank",
            ],
        ) || has(
            after
                .split(['，', ',', '。', '?', '？'])
                .next()
                .unwrap_or(after),
            &["目标"],
        ) && rank_numbers.len() == 1;
        if completed {
            from = Some(n.value);
        } else if is_target {
            goal = Some(n.value);
        } else if ends(
            before,
            &[
                "from",
                "from rank",
                "currently at",
                "starting at",
                "already at",
                "从",
                "当前",
                "现为",
                "已经",
                "已有",
            ],
        ) {
            from = Some(n.value);
        } else if rank_numbers.len() > 1 && i == rank_numbers.last()?.0 {
            goal = Some(n.value);
        } else if max
            || rank_numbers.len() > 1
            || has(
                before,
                &[
                    "current",
                    "already",
                    "starting",
                    "have a",
                    "have an",
                    "from",
                    "keeper is",
                    "现在",
                    "已有",
                    "已经",
                    "目前",
                    "本体",
                    "从",
                    "我有",
                    "手里",
                    "手上",
                ],
            )
        {
            from = Some(n.value);
        } else {
            goal = Some(n.value);
        }
    }
    if max {
        goal = Some(4);
    }
    let mut progress = 0;
    if let Some((value, total)) = meter {
        let Some(r) = STEPS.iter().position(|v| *v == total) else {
            return clarify(
                "当前档进度的分母应为 4、8、12 或 24；请核对星级和进度。",
                "The meter denominator must be 4, 8, 12 or 24. Check the rank and meter.",
            );
        };
        if from.is_some_and(|v| v != r as u64) || value > total {
            return clarify(
                "星级与该档进度有冲突，请核对本体星级和进度条。",
                "The rank and progress meter do not agree. Check the keeper’s rank and meter.",
            );
        }
        from = Some(r as u64);
        progress = value;
    }
    let capacity = has(
        q,
        &[
            "最多",
            "多少只满",
            "几只满",
            "几只可以",
            "能满几只",
            "能做几只",
            "how many can",
            "how many full",
            "how many max",
            "how many keepers",
            "how many pals can",
            "how many could",
            "maximum number",
        ],
    );
    let mut recipients = None;
    let mut stock = None;
    let mut total_fed = None;
    let excludes_keeper = ["不含本体", "不包括本体", "不算本体", "本体另算", "本体另计",
        "excluding the keeper", "not including the keeper"];
    for (i, n) in nums.iter().copied().enumerate().filter(|(i, _)| !used[*i]) {
        let before = nearby(&q[..n.start]);
        let context = count_context(q, &nums, i);
        let after = after(context);
        let value = n.value;
        if &q[n.start..n.end] == "一" && begins(after, &["共", "起", "样", "定"]) {
            continue;
        }
        // A demonstrative can repeat previously counted stock in the question.
        if ends(before, &["这", "那", "these", "those"])
            && stock.is_some_and(|(_, amount)| amount == value) {
            continue;
        }
        if ends(
            before,
            &[
                "进度",
                "进度是",
                "进度为",
                "已喂",
                "喂了",
                "喂过",
                "投入了",
                "已经投入",
                "已经喂了",
                "已投入",
                "已放入",
                "already fed",
                "already added",
                "already used",
                "have fed",
                "fed",
                "already put in",
            ],
        ) || begins(after, &["already fed", "already invested", "already added"])
        {
            if total_fed.replace(value).is_some() {
                return clarify(
                    "请注明每只已投入多少。",
                    "Please specify the amount already invested in each keeper.",
                );
            }
            continue;
        }
        let only = after.trim_start_matches(['只', '个', '头', ' ']);
        let keeper_noun = (ends(before, &["其中", "里头", "里面"]) && has(only, &["留", "主力", "本体"]))
            || begins(only, &["本体", "目标", "主宠", "主力", "要保留", "keeper", "recipient"]);
        let donor = begins(
            only,
            &[
                "普通素材",
                "素材",
                "祭品",
                "材料",
                "可消耗",
                "用作素材",
                "未浓缩的素材",
                "fodder",
                "spares",
                "spare",
                "donors",
                "donor",
                "duplicates",
                "extra",
                "uncondensed copies",
            ],
        ) || material_description(only) || !keeper_noun && ends(
            before,
            &[
                "额外",
                "可用素材",
                "另有",
                "另外有",
                "素材有",
                "现有素材",
                "spare",
                "extra",
                "donors:",
                "fodder:",
            ],
        );
        let shared_rank = ["都想", "都要", "都从", "各自", "一起升", "分别", "每只都",
            "both", "each starting", "each at"];
        let keeper = keeper_noun || ends(
            before,
            &[
                "要做",
                "想做",
                "想养",
                "培养",
                "要养",
                "要升",
                "养",
                "把",
                "给",
                "做",
                "保留",
                "我要",
                "want",
                "for",
                "make",
                "raise",
                "keep",
                "max out",
                "maxing",
                "fully condense",
                "to condense",
                "to max",
                "condense",
            ],
        ) || (!donor
            && !capacity
            && (has(only, &shared_rank) || context.split(CLAUSE_BREAKS).skip(1)
                .find(|s| !s.trim().is_empty()).is_some_and(|s| begins(s.trim(), &shared_rank))));
        let possession = has(
            before,
            &[
                "have",
                "got",
                "own",
                "with",
                "available",
                "total",
                "stock",
                "库存",
                "箱里",
                "箱中",
                "仓库",
                "背包",
                "共有",
                "我有",
                "手头",
                "手里",
                "手上",
                "总共",
                "一共",
                "一共有",
                "目前有",
                "现有",
                "只有",
                "还有",
                "有",
            ],
        ) || begins(only, &["copies", "pals", "同种", "同类"])
            || context.split(CLAUSE_BREAKS).take(2).any(|part| has(part, &excludes_keeper));
        if !donor && (keeper
            || (!capacity
                && begins(
                    only,
                    &[
                        "分别",
                        "都要",
                        "都从",
                        "各自",
                        "一起升",
                        "都想",
                        "满星",
                        "零星",
                        "未升星",
                    ],
                )
                && !possession))
        {
            recipients = Some(value);
        } else if donor || possession {
            let kind = if donor || has(q, &excludes_keeper) {
                Stock::Donors
            } else if has(
                q,
                &[
                    "包括本体",
                    "含本体",
                    "算本体",
                    "含目标",
                    "总共",
                    "一共",
                    "总数",
                    "including",
                    "in total",
                    "total of",
                    "all i have",
                ],
            ) || capacity
            {
                Stock::All
            } else {
                Stock::Unclear
            };
            if stock.replace((kind, value)).is_some() {
                return clarify(
                    "请把可用同种素材的数量和要保留的本体数量分开列出。",
                    "Please separate the donor stock from the number of keepers.",
                );
            }
        } else if !capacity && (after.starts_with('只') || begins(only, &["pals", "copies"])) {
            recipients.get_or_insert(value);
        }
    }
    let recipients = recipients.unwrap_or(1);
    let from = from.unwrap_or(0);
    let next = has(
        q,
        &[
            "下一星",
            "下一档",
            "下一阶",
            "再升一星",
            "next star",
            "next rank",
        ],
    );
    let to = goal.unwrap_or_else(|| if next && !max { (from + 1).min(4) } else { 4 });
    if let Some(fed) = total_fed {
        if meter.is_some() || from >= 4 || (from > 0 && fed > STEPS[from as usize]) || fed > 48 {
            return clarify(
                "请注明是当前这档已投入的数量，还是从零星累计的总量，以免重复计数。",
                "Specify current-step progress or the lifetime total already fed, so the same copies are not counted twice.",
            );
        }
        progress = fed;
    }
    if meter.is_none()
        && total_fed.is_none()
        && has(
            q,
            &[
                "部分进度",
                "喂过几只",
                "有些进度",
                "已经喂了些",
                "partly condensed",
                "partially condensed",
                "already fed some",
                "partway",
            ],
        )
    {
        return clarify(
            "现有进度会减少所需素材。请给本体的星级，以及这一档显示的已投入量和所需量。",
            "Existing progress reduces the required copies. Give the keeper’s current stars and the progress meter shown for the next star.",
        );
    }
    if recipients == 0 || to < from {
        return clarify(
            "请确认保留几只本体，以及当前星级和想升到的星级。",
            "Please give the keeper count, current rank and desired higher rank.",
        );
    }
    let current = CUMULATIVE[from as usize] + progress;
    let each = CUMULATIVE[to as usize].saturating_sub(current) as u128;
    let count = u128::from(recipients);
    let need = each * count;
    let mut details = vec![table.to_owned()];
    if from > 0 && goal.is_none() && !next && from < 4 {
        let left = STEPS[from as usize].saturating_sub(progress);
        details.push(if en {format!("For just the next star each keeper needs {left} more plain donors; the full-rank budget follows.")}else{format!("若只升下一星，每只还需 {left} 只普通素材；以下为升满星的用量。")});
    }
    if let Some((_, denom)) = meter {
        details.push(if en {
            format!("Each keeper starts at {from}★, with {progress}/{denom} towards the next star.")
        } else {
            format!("每只本体按 {from} 星、当前档 {progress}/{denom} 计。")
        });
    }
    details.push(if en {format!("From the stated starting progress to {to}★: {each} more plain donors per keeper. For {count} keeper(s): {need} donors, or {} Pals including the keeper(s).",need+count)}else{format!("从上述起点升至 {to} 星，每只本体还需 {each} 只普通同种素材；保留 {count} 只本体，共需 {need} 只素材，连本体共 {} 只。",need+count)});
    if let Some((kind, amount)) = stock {
        let amount = u128::from(amount);
        for model in [Stock::Donors, Stock::All] {
            if kind != Stock::Unclear && kind != model {
                continue;
            }
            let all = model == Stock::All;
            let required = need + if all { count } else { 0 };
            let cost = each + u128::from(all);
            let label = match (all, en) {
                (true, true) => "including the keeper(s)",
                (false, true) => "spare donors",
                (true, false) => "含本体的总数",
                (false, false) => "另有的素材数",
            };
            let ask_rank = has(
                q,
                &[
                    "能升到几星",
                    "能到几星",
                    "能升几星",
                    "可以升几星",
                    "能浓缩到几星",
                    "达到几星",
                    "能到哪一星",
                    "what rank",
                    "which rank",
                    "how many stars",
                    "how far",
                ],
            );
            if ask_rank && (!all || amount >= count) {
                let donors = amount - if all { count } else { 0 };
                let points = u128::from(current) + donors / count;
                let reached = CUMULATIVE
                    .iter()
                    .rposition(|&v| u128::from(v) <= points)
                    .unwrap_or(0);
                let next = if reached < 4 {
                    format!(
                        " ({}/{})",
                        points - u128::from(CUMULATIVE[reached]),
                        STEPS[reached]
                    )
                } else {
                    String::new()
                };
                details.push(if en {format!("With {amount} {label}, dividing the donors evenly among {count} keeper(s) gets each to {reached}★{next}.")}else{format!("若 {amount} 只是{label}，平均分给 {count} 只本体，每只可升到 {reached} 星{next}。")});
            } else if capacity && cost > 0 {
                details.push(if en {
                    format!(
                        "With {amount} {label}, the maximum is {} keeper(s) at {to}★, leaving {}.",
                        amount / cost,
                        amount % cost
                    )
                } else {
                    format!(
                        "若 {amount} 只是{label}，最多能升好 {} 只 {to} 星本体，余 {} 只。",
                        amount / cost,
                        amount % cost
                    )
                });
            } else {
                let missing = required.saturating_sub(amount);
                details.push(if en {format!("If {amount} is the count {label}: short by {missing}; {} left after reserving the requirement if sufficient.",amount.saturating_sub(required))}else if missing>0 {format!("若 {amount} 只是{label}，还差 {missing} 只。")}else{format!("若 {amount} 只是{label}，数量足够，余 {} 只。",amount-required)});
            }
        }
    }
    Some(reply(details.join("\n"), en))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::{Conversation, field_text};
    fn data() -> Data {
        Data::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
            eframe::egui::Context::default(),
        )
        .unwrap()
    }
    #[test]
    fn distinguishes_stock_from_keepers_in_condensation_questions() {
        let data = data();
        for (question, keepers, donors, outcome) in [
            ("1.0版，我只给一只刚升到三星的帕鲁升最后一星，本体进度是空的。箱里22只是另外抓的同种零星素材，能直接升满吗？", 1, 24, "还差 2 只"),
            ("本体三星；箱里十七只是另外抓来的同种零星材料！现在升到四星够吗？", 1, 24, "还差 7 只"),
            ("箱中有二十六只是我另抓的同种素材。把一只三星主力浓缩到满星。", 1, 24, "数量足够，余 2 只"),
            ("我保留3只本体和65只同种零星素材，三只本体都要从三星升到满星。还差多少？", 3, 72, "还差 7 只"),
            ("两只本体都是三星；把49只同种素材喂给它们，都要升到四星。", 2, 48, "数量足够，余 1 只"),
            ("四只帕鲁都想从三星升到满星，需要多少素材？", 4, 96, "连本体共 100 只"),
            ("我有三只二星帕鲁，各自想升到三星。另有二十只同种素材，差几只？", 3, 36, "还差 16 只"),
            ("两只三星本体和四十只普通同种素材，准备一起升满，要再抓多少？", 2, 48, "还差 8 只"),
            ("箱里总共二十五只同种帕鲁，包括一只三星本体。其余是零星素材，能升满吗？", 1, 24, "数量足够，余 0 只"),
            ("我的本体已经三星。手里共23只同种零星帕鲁，不含本体，升满还差几只？", 1, 24, "还差 1 只"),
            ("保留两只帕鲁不当素材，都从三星升到四星，另外有43只同种零星素材，够不够？", 2, 48, "还差 5 只"),
        ] {
            let mut conversation = Conversation::default();
            let result = conversation.ask_world(question, &data, &serde_json::json!({}));
            let budget = format!("保留 {keepers} 只本体，共需 {donors} 只素材");
            assert!(conversation.direct_result() && result.text.contains(&budget)
                && result.text.contains(outcome), "{question}: {}", result.text);
        }
    }
    #[test]
    fn plain_donors_keep_keepers_separate_and_account_for_partial_progress() {
        let data = data();
        for (q, expect) in [
            ("箱子里共有二十四只零星的同种帕鲁，其中一只留着当主力。从零星喂到三星，这二十四只够不够？", "还差 1 只"),
            ("主力已经是二星，只升到三星，要再准备几只零星的同种帕鲁当素材？", "还需 12 只"),
            ("我保留的那只刚升到2星，这一档进度是零。箱里另有11只同种零星素材，能一次升到3星吗？", "还差 1 只"),
            ("本体已经升至一星，当前档进度为三，另有4只同种素材。想升到二星还差几只？", "还差 1 只"),
            ("一只帕鲁从零星浓缩到满星，要多少同种？", "连本体共 49 只"),
            (
                "两只本体都是二星，各自浓缩进度 3/12，都要升到满星。我有 60 只素材，还差多少只？",
                "还差 6 只",
            ),
            (
                "I want to fully condense three Pals from zero stars. How many donors?",
                "144 donors, or 147 Pals",
            ),
            (
                "我的本體現在三星，濃縮進度 9/24，升到四星還差幾隻？",
                "还需 15 只",
            ),
            (
                "I have 101 Pals in total. How many fully condensed keepers can I make?",
                "maximum is 2 keeper(s) at 4★, leaving 3",
            ),
            (
                "How many copies to condense one Pal from 1 star to 3 stars?",
                "20 more plain donors",
            ),
            (
                "If I have just 20 copies, can I max out my Pal with the condenser?",
                "short by 28",
            ),
            (
                "本体三星，已经喂了七只，继续浓缩到满星还差多少？",
                "还需 17 只",
            ),
            ("浓缩一只二星帕鲁的进度显示 3/8，离满星差多少只？", "有冲突"),
            (
                "1.0 版，我的捣蛋猫刚好二星，下一星进度是零。仓库有 27 只没浓缩的捣蛋猫当材料。升到四星一共要喂多少只，还得再抓几只？",
                "还差 9 只",
            ),
            (
                "本体二星，另有24只没有浓缩的同种作为材料，升到四星还差多少只？",
                "还差 12 只",
            ),
        ] {
            let mut c = Conversation::default();
            let result = c.ask_world(q, &data, &serde_json::json!({}));
            assert!(
                c.direct_result() && result.text.contains(expect),
                "{q}: {}",
                result.text
            );
        }
        let rank = answer(
            &data,
            &field_text("本体从零星开始，我有 15 只素材，能浓缩到几星？"),
        )
        .unwrap();
        assert!(rank.text.contains("2 星 (3/12)"), "{}", rank.text);
        let ranked = answer(
            &data,
            &field_text("Can I use a condensed Pal as fodder for my keeper?"),
        )
        .unwrap();
        assert!(ranked.text.contains("projected progress"));
        assert!(answer(&data, &field_text("满星后伙伴技能的加成是多少？")).is_none());
        assert!(answer(&data, &field_text("建造浓缩机需要多少材料？")).is_none());
    }
}
