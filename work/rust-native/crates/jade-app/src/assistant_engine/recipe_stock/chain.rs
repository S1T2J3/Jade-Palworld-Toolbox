use super::{
    AssistantReply, CAPACITY, Data, UNITS, Value, any, array, names, select_product, text,
    whole_name,
};
use crate::assistant_engine::quantities::{Quantity, integers};
use std::collections::{BTreeMap, BTreeSet};

const ACTIONS: &[&str] = &[
    "制作",
    "合成",
    "加工",
    "磨",
    "烤",
    "炼",
    "烧",
    "织",
    "搓",
    "做",
    "造",
    "煮",
    "烹饪",
    "转化",
    "换算成",
    "craft",
    "produce",
    "make",
    "convert",
    "turn",
    "grind",
    "mill",
    "bake",
    "cook",
    "weave",
    "smelt",
    "forge",
    "refine",
];
const ORDER: &[&str] = &[
    "先",
    "再",
    "然后",
    "接着",
    "随后",
    "之后",
    "最后",
    "then",
    "followed by",
    "afterwards",
];
const OWNED: &[&str] = &[
    "库存",
    "仓库",
    "手里",
    "手头",
    "手上",
    "我有",
    "只有",
    "已有",
    "现有",
    "现成",
    "还剩",
    "还有",
    "i have",
    "i've got",
    "i only have",
    "in stock",
    "on hand",
    "in my inventory",
    "starting with",
];
const CLOSED: &[&str] = &[
    "只有",
    "仅有",
    "其余为0",
    "其他为0",
    "其他都没有",
    "only have",
    "only got",
    "nothing else",
];

#[derive(Clone, Debug)]
struct Step {
    product: String,
    requested: Option<u64>,
    inputs: BTreeSet<String>,
    caps: BTreeMap<String, u64>,
    all: bool,
    uses_previous: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Production {
    steps: Vec<Step>,
    stock: BTreeMap<String, u64>,
    only_listed: bool,
    reserve: BTreeMap<String, u64>,
    inferred: bool,
}

#[derive(Clone, Copy)]
struct Mention<'a> {
    start: usize,
    end: usize,
    row: &'a Value,
}

fn mentions<'a>(s: &str, items: &'a [Value]) -> Vec<Mention<'a>> {
    let mut hits = Vec::new();
    for row in items {
        for alias in names(row) {
            for (start, _) in s
                .match_indices(&alias)
                .filter(|(i, _)| whole_name(s, &alias, *i))
            {
                hits.push(Mention {
                    start,
                    end: start + alias.len(),
                    row,
                });
            }
        }
    }
    hits.sort_by_key(|m| (m.start, std::cmp::Reverse(m.end)));
    let mut longest = Vec::new();
    for hit in hits {
        if !longest.last().is_some_and(|p: &Mention| {
            hit.start < p.end && (hit.start != p.start || hit.end != p.end)
        }) {
            longest.push(hit);
        }
    }
    longest
}

fn boundary(s: &str, word: &str, at: usize) -> bool {
    !word.is_ascii() || whole_name(s, word, at)
}

// Keep a number like 1,200 or 1.5 in one clause, so a fragment never becomes inventory.
fn clauses(q: &str) -> Vec<&str> {
    let mut splits = Vec::new();
    for (i, c) in q.char_indices() {
        let j = i + c.len_utf8();
        if "。！？!?，,；;.".contains(c)
            && !(matches!(c, ',' | '.')
                && q[..i].ends_with(|c: char| c.is_ascii_digit())
                && q[j..].starts_with(|c: char| c.is_ascii_digit()))
        {
            splits.push((i, j));
        }
    }
    for &marker in ORDER {
        for (i, _) in q
            .match_indices(marker)
            .filter(|(i, _)| boundary(q, marker, *i))
        {
            splits.push((i, i + marker.len()));
        }
    }
    splits.sort_unstable();
    let mut start = 0;
    let mut pieces = Vec::new();
    for (i, j) in splits {
        if i >= start {
            if i > start {
                pieces.push(q[start..i].trim());
            }
            start = j;
        }
    }
    pieces.push(q[start..].trim());
    pieces
}

fn unit_bridge(s: &str) -> bool {
    let s = s.trim();
    [
        "现成的",
        "现成",
        "已有的",
        "的",
        "units of",
        "pieces of",
        "bags of",
        "loaves of",
        "loaf of",
        "units",
        "pieces",
    ]
    .iter()
    .any(|post| {
        s.strip_suffix(post)
            .is_some_and(|before| before.trim().chars().all(|c| UNITS.contains(c)))
    }) || s
        .chars()
        .all(|c| UNITS.contains(c) || "×x:=".contains(c) || c.is_whitespace())
}

fn amount(s: &str, m: Mention, nums: &[Quantity]) -> Option<Quantity> {
    nums.iter()
        .copied()
        .rev()
        .find(|n| n.end <= m.start && unit_bridge(&s[n.end..m.start]))
        .or_else(|| {
            nums.iter().copied().find(|n| {
                n.start >= m.end && {
                    let bridge = s[m.end..n.start].trim();
                    unit_bridge(bridge)
                        || [
                            "现在有",
                            "现有",
                            "有",
                            "剩",
                            "改为",
                            "改成",
                            "改到",
                            "为",
                            "is",
                            "to",
                        ]
                        .contains(&bridge)
                }
            })
        })
}

fn action_offset(s: &str) -> Option<usize> {
    ACTIONS
        .iter()
        .flat_map(|term| {
            s.match_indices(term)
                .filter(|(i, _)| boundary(s, term, *i))
                .map(|(i, _)| i)
        })
        .min()
}

fn output_score(s: &str, m: Mention, qty: Option<Quantity>) -> u8 {
    if array(&m.row["recipes"]).is_empty() {
        return 0;
    }
    let prefix = s[..qty
        .filter(|n| n.end <= m.start)
        .map_or(m.start, |n| n.start)]
        .trim_end();
    // A result complement is stronger evidence than an ingredient adjacent to "mill" or "convert".
    if ["成", " into", " to"].iter().any(|p| prefix.ends_with(p)) {
        return 3;
    }
    let mut trimmed = prefix;
    while let Some(rest) = [
        "普通",
        "普通的",
        "基础",
        "基础的",
        " standard",
        " regular",
        " basic",
        " the",
        " a",
        " an",
        " some",
        " more",
        " as many",
        " as much",
    ]
    .iter()
    .find_map(|suffix| trimmed.strip_suffix(suffix))
    {
        trimmed = rest.trim_end();
    }
    if ACTIONS.iter().any(|p| trimmed.ends_with(p)) {
        return 2;
    }
    if any(prefix, &["最多", "多少", "几份", "几个", "how many"]) {
        return 1;
    }
    0
}

fn clarify(msg: impl Into<String>) -> AssistantReply {
    AssistantReply::new(&msg.into(), &[])
}

fn parse_step(
    s: &str,
    hits: &[Mention],
    nums: &[Quantity],
) -> Result<Option<Step>, AssistantReply> {
    let Some(action) = action_offset(s) else {
        return Ok(None);
    };
    let outputs: Vec<_> = hits
        .iter()
        .copied()
        .map(|m| {
            let qty = amount(s, m, nums);
            (output_score(s, m, qty), m, qty)
        })
        .filter(|(score, m, _)| *score > 0 && (m.start >= action || *score == 1))
        .collect();
    let Some(top) = outputs.iter().map(|(score, _, _)| *score).max() else {
        return Ok(None);
    };
    let (_, out, qty) = outputs.iter().find(|(score, _, _)| *score == top).unwrap();
    let id = text(out.row, "id");
    if outputs
        .iter()
        .any(|(score, m, _)| *score == top && text(m.row, "id") != id)
    {
        return Err(clarify(format!(
            "「{}」这一步有多个可能的成品或同名品质。请指定要做的物品及品质。",
            s
        )));
    }
    let mut inputs = BTreeSet::new();
    let mut caps = BTreeMap::new();
    for &m in hits
        .iter()
        .filter(|m| m.start < out.start && text(m.row, "id") != id)
    {
        let input = text(m.row, "id").to_owned();
        if let Some(n) = amount(s, m, nums) {
            caps.insert(input.clone(), n.value);
        }
        inputs.insert(input);
    }
    Ok(Some(Step {
        product: id.into(),
        requested: qty.map(|n| n.value).or_else(|| {
            [" a", " an", " a regular", " a basic", " a standard"]
                .iter()
                .any(|a| s[..out.start].trim_end().ends_with(a))
                .then_some(1)
        }),
        inputs,
        caps,
        uses_previous: any(
            s,
            &[
                "上一步的产物",
                "上一步产物",
                "刚做出的",
                "拿它做",
                "拿它们做",
                "用它做",
                "用它们做",
                "use that to",
                "use those to",
                "use it to",
                "use them to",
                "with the output",
            ],
        ),
        all: any(
            s,
            &[
                "全部",
                "全都",
                "所有",
                "尽量",
                "用光",
                "用完",
                "全用",
                "耗尽",
                "all ",
                "as much as",
            ],
        ),
    }))
}

fn parse_stock(s: &str, hits: &[Mention], nums: &[Quantity], stock: &mut BTreeMap<String, u64>) {
    for &m in hits {
        if let Some(n) = amount(s, m, nums) {
            stock.insert(text(m.row, "id").into(), n.value);
        }
    }
}

pub(super) fn answer(
    q: &str,
    d: &Data,
    prior: Option<&Production>,
) -> Option<(Option<Production>, AssistantReply)> {
    let ordered = any(q, ORDER);
    let items = array(&d.get("item-catalog-data")["items"]);
    if any(
        q,
        &[
            "购买", "买", "卖", "换购", "商店", "金币", "buy", "sell", "purchase", "costs",
        ],
    ) {
        return None;
    }
    if !ordered && action_offset(q).is_none() {
        let mut plan = prior?.clone();
        if any(
            q,
            &[
                "拿到",
                "用掉",
                "多了",
                "少了",
                "增加",
                "减少",
                "额外",
                "additional",
                "used up",
                "gained",
            ],
        ) {
            return None;
        }
        let hits = mentions(q, items);
        let mut updated = BTreeMap::new();
        parse_stock(q, &hits, &integers(q), &mut updated);
        let relevant = plan
            .steps
            .iter()
            .filter_map(|s| items.iter().find(|r| text(r, "id") == s.product))
            .flat_map(|p| array(&p["recipes"]))
            .flat_map(|r| array(&r["materials"]))
            .filter_map(|m| m[0].as_str())
            .collect::<BTreeSet<_>>();
        if updated.is_empty() || updated.keys().all(|id| !relevant.contains(id.as_str())) {
            return None;
        }
        plan.stock.extend(updated);
        let reply = plan.calculate(d, items)?;
        return Some((Some(plan), reply));
    }
    if action_offset(q).is_none() || integers(q).is_empty() || (!ordered && !any(q, CAPACITY)) {
        return None;
    }
    if any(
        q,
        &[
            "不加工",
            "不磨",
            "不炼",
            "不合成",
            "不许加工",
            "仅用现成",
            "只用现成",
            "without crafting",
            "without processing",
            "without milling",
            "already crafted only",
        ],
    ) {
        return None;
    }
    let mut plan = Production {
        steps: vec![],
        stock: BTreeMap::new(),
        only_listed: any(q, CLOSED),
        reserve: BTreeMap::new(),
        inferred: false,
    };
    let mut unclear_reserve = false;
    let mut final_goal = None;
    for piece in clauses(q) {
        let hits = mentions(piece, items);
        let nums = integers(piece);
        if let Some(at) = [
            "预留",
            "保留",
            "留下",
            "留出",
            "留着",
            "留给",
            "留作",
            "留",
            "reserve",
            "set aside",
            "keep",
        ]
        .iter()
        .filter_map(|word| piece.find(word))
        .min()
        .filter(|i| nums.iter().any(|n| n.start > *i))
        {
            let reserved = hits
                .iter()
                .copied()
                .filter(|m| m.start >= at)
                .collect::<Vec<_>>();
            let before = hits
                .iter()
                .copied()
                .filter(|m| m.end <= at)
                .collect::<Vec<_>>();
            parse_stock(
                &piece[..at],
                &before,
                &nums
                    .iter()
                    .copied()
                    .filter(|n| n.end <= at)
                    .collect::<Vec<_>>(),
                &mut plan.stock,
            );
            parse_stock(
                piece,
                &reserved,
                &nums
                    .iter()
                    .copied()
                    .filter(|n| n.start > at)
                    .collect::<Vec<_>>(),
                &mut plan.reserve,
            );
            if plan.reserve.is_empty() || action_offset(&piece[at..]).is_some() {
                unclear_reserve = true;
            }
            continue;
        }
        if let Some(at) = action_offset(piece) {
            match parse_step(piece, &hits, &nums) {
                Ok(Some(step)) => {
                    if any(
                        piece,
                        &[
                            "能做几个",
                            "能做多少",
                            "能做几份",
                            "最多能做",
                            "最多可以做",
                            "how many",
                            "how much can",
                        ],
                    ) {
                        // A question may name its final product before the instructions or repeat it afterwards.
                        final_goal = Some(step);
                        if let Some((i, marker)) = [" with ", " using "]
                            .iter()
                            .find_map(|p| piece.find(p).map(|i| (i, p)))
                        {
                            let after = i + marker.len();
                            let supplies: Vec<_> =
                                hits.iter().copied().filter(|m| m.start >= after).collect();
                            parse_stock(piece, &supplies, &nums, &mut plan.stock);
                        }
                    } else {
                        plan.steps.push(step);
                    }
                    continue;
                }
                Err(e) if !plan.steps.is_empty() => return Some((None, e)),
                _ => {}
            }
            if at == 0 {
                continue;
            }
        }
        if any(piece, OWNED)
            || (plan.steps.is_empty()
                && !any(
                    piece,
                    &[
                        "如果", "想", "打算", "留", "除去", "需要", "目标", "want", "need",
                        "reserve",
                    ],
                ))
        {
            parse_stock(piece, &hits, &nums, &mut plan.stock);
        }
    }
    if let Some(goal) = final_goal
        && plan.steps.last().is_none_or(|s| s.product != goal.product)
    {
        plan.steps.push(goal);
    }
    if plan.stock.is_empty() {
        return None;
    }
    if plan.steps.len() < 2 {
        if !any(q, CAPACITY) {
            return None;
        }
        // Only infer an unspoken step for an unambiguous, lossless choice of production route:
        // one intermediate, one recipe, raw stock supplied, and no raw material shared with the final craft.
        let product = select_product(items, q)?;
        let [recipe] = array(&product["recipes"]) else {
            return None;
        };
        let direct = array(&recipe["materials"])
            .iter()
            .filter_map(|m| m[0].as_str())
            .collect::<BTreeSet<_>>();
        let mut possible = vec![];
        for mat in &direct {
            let row = items.iter().find(|r| text(r, "id") == *mat)?;
            let [r] = array(&row["recipes"]) else {
                continue;
            };
            let raw = array(&r["materials"]);
            if !raw.is_empty()
                && raw.iter().all(|m| {
                    m[0].as_str()
                        .is_some_and(|id| plan.stock.contains_key(id) && !direct.contains(id))
                })
            {
                possible.push(*mat);
            }
        }
        let [intermediate] = possible.as_slice() else {
            return None;
        };
        if !plan.steps.is_empty() && plan.steps.iter().any(|s| s.product != text(product, "id")) {
            return None;
        }
        let step = |id: &str| Step {
            product: id.to_owned(),
            requested: None,
            inputs: BTreeSet::new(),
            caps: BTreeMap::new(),
            all: false,
            uses_previous: false,
        };
        plan.steps = vec![step(intermediate), step(text(product, "id"))];
        plan.inferred = true;
    }
    if unclear_reserve
        || any(
            q,
            &[
                "一半",
                "半数",
                "一部分",
                "半份",
                "分之一",
                "half of",
                "some of",
                "fraction of",
            ],
        )
    {
        return Some((
            None,
            clarify("这条制作顺序有材料用量限制，请给出每步可用的物品和整数数量。"),
        ));
    }
    let reply = plan.calculate(d, items)?;
    Some((Some(plan), reply))
}

impl Production {
    fn calculate(&self, d: &Data, items: &[Value]) -> Option<AssistantReply> {
        let mut stock: BTreeMap<&str, u128> = self
            .stock
            .iter()
            .map(|(id, n)| (id.as_str(), u128::from(*n)))
            .collect();
        let mut used = BTreeSet::new();
        let mut lines = Vec::new();
        if self.inferred {
            lines.push("按已报原料可先加工成中间材料计算。".to_owned());
        }
        if !self.reserve.is_empty() {
            lines.push(format!(
                "预留{}，以下制作不动用这部分。",
                self.reserve
                    .iter()
                    .map(|(id, n)| format!("{} × {n}", d.item_name(id)))
                    .collect::<Vec<_>>()
                    .join("、")
            ));
        }
        let mut links = Vec::new();
        let mut previous = None;
        for (index, step) in self.steps.iter().enumerate() {
            let product = items.iter().find(|r| text(r, "id") == step.product)?;
            let name = text(product, "name");
            let id = text(product, "id");
            let mut candidates: Vec<_> = array(&product["recipes"])
                .iter()
                .filter(|r| {
                    let materials = array(&r["materials"]);
                    step.inputs.iter().all(|input| {
                        materials
                            .iter()
                            .any(|m| m[0].as_str() == Some(input.as_str()))
                    })
                })
                .collect();
            // Independent steps still draw from the same ledger. A material reference
            // or a choice of multiple recipes can make the last output relevant.
            if let Some(prev) = previous
                && (step.uses_previous || candidates.len() > 1)
            {
                let using_previous: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|r| {
                        array(&r["materials"])
                            .iter()
                            .any(|m| m[0].as_str() == Some(prev))
                    })
                    .collect();
                if step.uses_previous || !using_previous.is_empty() {
                    candidates = using_previous;
                }
            }
            if candidates.len() != 1 {
                let msg = if candidates.is_empty() {
                    format!(
                        "配方表中没有找到能按这条顺序制作{name}的配方。请确认要用的上一步产物及各步材料。"
                    )
                } else {
                    format!("{name}有多种配方，请指明这一段要用哪种材料。")
                };
                lines.push(msg);
                return Some(AssistantReply {
                    text: lines.join("\n"),
                    links,
                    suggestions: vec![],
                });
            }
            let r = candidates[0];
            let batch = u128::from(r["qty"].as_u64().filter(|n| *n > 0)?);
            let materials: Option<Vec<_>> = array(&r["materials"])
                .iter()
                .map(|m| {
                    Some((
                        m[0].as_str()?,
                        u128::from(m[1].as_u64().filter(|n| *n > 0)?),
                    ))
                })
                .collect();
            let materials = materials?;
            let later_needs_same_raw = self.steps[index + 1..]
                .iter()
                .filter_map(|s| items.iter().find(|p| text(p, "id") == s.product))
                .flat_map(|p| array(&p["recipes"]))
                .flat_map(|r| array(&r["materials"]))
                .any(|later| {
                    materials
                        .iter()
                        .any(|(id, _)| later[0].as_str() == Some(*id))
                });
            if later_needs_same_raw && !step.all && step.requested.is_none() && step.caps.is_empty()
            {
                lines.push(format!("制作{name}与后续步骤可能消耗同一种材料。请指定这一步要做多少个，或明确先将可用材料全部投入。"));
                return Some(AssistantReply {
                    text: lines.join("\n"),
                    links,
                    suggestions: vec![],
                });
            }
            let mut bounds = Vec::new();
            let mut unknown = Vec::new();
            for &(mat, rate) in &materials {
                let owned = stock
                    .get(mat)
                    .copied()
                    .or_else(|| self.only_listed.then_some(0));
                let cap = step.caps.get(mat).map(|n| u128::from(*n));
                if let Some(owned) = owned {
                    let available =
                        owned.saturating_sub(u128::from(*self.reserve.get(mat).unwrap_or(&0)));
                    bounds.push(available.min(cap.unwrap_or(available)) / rate);
                } else {
                    unknown.push(d.item_name(mat));
                }
            }
            if !unknown.is_empty() {
                lines.push(format!(
                    "计算{name}还需要{}的初始库存（没有可填 0）。",
                    unknown.join("、")
                ));
                return Some(AssistantReply {
                    text: lines.join("\n"),
                    links,
                    suggestions: vec![],
                });
            }
            let limit = bounds.into_iter().min()?;
            if index + 1 < self.steps.len()
                && !step.all
                && let Some(goal) = step.requested
                && u128::from(goal).div_ceil(batch) > limit
            {
                let batches = u128::from(goal).div_ceil(batch);
                let missing = materials
                    .iter()
                    .filter_map(|(mat, rate)| {
                        let owned = stock.get(mat).copied().unwrap_or(0);
                        let available = owned
                            .saturating_sub(u128::from(*self.reserve.get(*mat).unwrap_or(&0)))
                            .min(step.caps.get(*mat).map_or(owned, |n| u128::from(*n)));
                        let short = (batches * rate).saturating_sub(available);
                        (short > 0).then(|| format!("{} × {short}", d.item_name(mat)))
                    })
                    .collect::<Vec<_>>();
                lines.push(format!("按指定顺序，先做 {goal} 个{name}还缺{}。这一步最多能做 {} 个；要满足此前提，需补足材料后才能继续分配余料。",missing.join("、"),limit*batch));
                links.push((format!("查看{name}配方"), format!("#items/{id}")));
                return Some(AssistantReply {
                    text: lines.join("\n"),
                    links,
                    suggestions: vec![],
                });
            }
            let batches = step
                .requested
                .map(|n| limit.min(u128::from(n).div_ceil(batch)))
                .unwrap_or(limit);
            let output = batches.checked_mul(batch)?;
            let mut costs = Vec::new();
            for (mat, rate) in materials {
                let take = batches.checked_mul(rate)?;
                *stock.entry(mat).or_default() -= take;
                costs.push(format!("{} × {take}", d.item_name(mat)));
                used.insert(mat);
            }
            let old = stock.get(id).copied().unwrap_or_default();
            let total = old.checked_add(output)?;
            stock.insert(id, total);
            let mut line = format!("消耗{}，做出 {output} 个{name}。", costs.join("、"));
            if old > 0 {
                line.push_str(&format!("加上已有 {old} 个，现有 {total} 个{name}。"));
            }
            if step.requested.is_some_and(|n| u128::from(n) > output) {
                line.push_str("材料不足，未达到这一步的目标。");
            }
            links.push((format!("查看{name}配方"), format!("#items/{id}")));
            lines.push(line);
            previous = Some(id);
        }
        let leftovers = used
            .into_iter()
            .map(|id| format!("{} × {}", d.item_name(id), stock[id]))
            .collect::<Vec<_>>();
        lines.push(format!("剩余材料：{}。", leftovers.join("、")));
        Some(AssistantReply {
            text: lines.join("\n"),
            links,
            suggestions: vec![],
        })
    }
}
