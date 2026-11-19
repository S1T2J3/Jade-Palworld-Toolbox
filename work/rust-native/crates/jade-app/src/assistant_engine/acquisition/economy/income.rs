use super::{
    AssistantReply, Data, array, has, integers, link, purchase_words, species_mentions, text,
};
use std::borrow::Cow;

fn clauses(q: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut start = 0;
    for (i, c) in q.char_indices() {
        let j = i + c.len_utf8();
        if "。；;，,？！?!.\n".contains(c)
            && !(matches!(c, '.' | ',')
                && q[..i].ends_with(|c: char| c.is_ascii_digit())
                && q[j..].starts_with(|c: char| c.is_ascii_digit()))
        {
            if i > start {
                out.push(q[start..i].trim());
            }
            start = j;
        }
    }
    if start < q.len() {
        out.push(q[start..].trim());
    }
    out
}

fn historic(s: &str) -> bool {
    has(
        s,
        &[
            "赚过",
            "赚了",
            "挣了",
            "挣过",
            "曾赚",
            "卖了",
            "卖过",
            "卖掉了",
            "卖出过",
            "花过",
            "花了",
            "曾经有",
            "原预算",
            "以前的预算",
            "上次的预算",
            "退费中",
            "退款中",
            "还没到账",
            "尚未到账",
            "未到账",
            "earned ",
            "i sold ",
            "just sold",
            "sold for",
            "sold my",
            "i spent ",
            "previous budget",
            "old budget",
            "pending refund",
            "refund is pending",
            "refund pending",
            "waiting for a refund",
            "not yet refunded",
            "will be refunded",
        ],
    ) || (has(
        s,
        &[
            "以前",
            "上次",
            "昨天",
            "之前",
            "前天",
            "last time",
            "yesterday",
            "earlier",
            "last week",
        ],
    ) && has(
        s,
        &[
            "购买", "买", "花", "卖", "收入", "预算", "买过", "bought", "made ", "spent", "sold",
            "sold ", "budget",
        ],
    ))
}

/// Historical proceeds and an uncredited refund are only context. A player can
/// explicitly allocate a known receipt as the current budget.
pub(super) fn purchase_input(q: &str) -> Cow<'_, str> {
    if !historic(q) {
        return Cow::Borrowed(q);
    }
    let mut active = vec![];
    let mut earned = vec![];
    for piece in clauses(q) {
        if !historic(piece) {
            active.push(piece);
            continue;
        }
        let cut = [
            "现在",
            "目前",
            "这次",
            "这回",
            "眼下",
            "今天打算",
            "我想买",
            "想用这笔",
            "想拿这笔",
            "拿这笔",
            "用这笔",
            "预算只",
            " but ",
            " and now ",
            " now i ",
            " and i want ",
            " and want to ",
            " and use the proceeds",
            " and use those",
            " and plan to ",
            " and my budget ",
            " can i buy ",
            " how many ",
            " how much ",
        ]
        .iter()
        .filter_map(|p| piece.find(p).filter(|&i| i > 0 && historic(&piece[..i])))
        .min()
        .unwrap_or(piece.len());
        let (past, current) = piece.split_at(cut);
        if !current.is_empty() {
            active.push(current);
        }
        if has(past, &["赚", "挣", "卖", "earned", "sold"])
            && !has(past, &["退款", "refund", "未到账", "没到账"])
        {
            for n in integers(past) {
                let unit = past[n.end..].trim_start();
                if ["金币", "金幣", "gold", "coin"]
                    .iter()
                    .any(|p| unit.starts_with(p))
                {
                    earned.push(n.value);
                }
            }
        }
    }
    let current = active.join("，");
    if let [budget] = earned.as_slice()
        && has(
            &current,
            &[
                "这笔钱",
                "这笔收入",
                "把收入全部",
                "刚赚的钱",
                "用刚卖的钱",
                "use the proceeds",
                "using the proceeds",
                "use those proceeds",
                "spend the proceeds",
                "spend all of it",
                "using all of it",
                "use that gold",
            ],
        )
    {
        return Cow::Owned(format!("预算 {budget} 金币，{current}"));
    }
    Cow::Owned(current)
}

fn income_goal(s: &str) -> bool {
    has(
        s,
        &[
            "刷钱",
            "刷金币",
            "怎么赚钱",
            "如何赚钱",
            "赚钱方法",
            "赚钱方式",
            "赚金币",
            "挣金币",
            "来钱",
            "钱怎么赚",
            "怎么挣钱",
            "赚钱路线",
            "赚钱路子",
            "赚钱快",
            "赚钱的办法",
            "money farm",
            "farm money",
            "farm gold",
            "farm coins",
            "gold farm",
            "coin farm",
            "make money",
            "earn money",
            "make more gold",
            "earn gold",
            "earn coins",
            "get more gold",
        ],
    )
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if !income_goal(q) {
        return None;
    }
    let parts = clauses(q);
    let goal = parts.iter().rposition(|s| income_goal(s))?;
    // A money-making anecdote can also introduce a purchase question.
    if parts[goal + 1..].iter().any(|s| {
        !historic(s)
            && purchase_words(s)
            && has(
                s,
                &[
                    "想买",
                    "预算",
                    "买几",
                    "多少钱",
                    "能买",
                    "够买",
                    "怎么买",
                    "花多少",
                    "want to buy",
                    "can i buy",
                    "how many",
                    "afford",
                    "budget",
                    "how much",
                    "cost",
                ],
            )
    }) {
        return None;
    }
    if has(
        q,
        &["how much for selling", "卖出价", "回收价", "卖给商人多少钱"],
    ) {
        return None;
    }
    let game = d.get("item-catalog-data");
    let money = array(&game["items"]).iter().find(|r| r["id"] == "Money")?;
    let ranch = d.get("ranch-data")["pals"].as_object()?;
    let (already, _) = species_mentions(&d.species, q);
    let mut options = vec![];
    for (key, record) in ranch {
        let Some(drop) = array(&record["tiers"][0])
            .iter()
            .find(|r| r["item"] == "Money")
        else {
            continue;
        };
        let Some(row) = d.species.iter().find(|r| r["key"] == key.as_str()) else {
            continue;
        };
        let (Some(prob), Some(lo), Some(hi)) = (
            drop["chance"].as_f64(),
            drop["min"].as_u64(),
            drop["max"].as_u64(),
        ) else {
            continue;
        };
        let mentioned = already
            .iter()
            .any(|(idx, _, _)| d.species[*idx]["key"] == key.as_str());
        options.push((mentioned, prob, lo, hi, row));
    }
    // Show a stable passive option first, and prefer a different Pal when the
    // player has already described their current ranch.
    options.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.total_cmp(&a.1)));
    let mut r = AssistantReply::new("", &[]);
    let mut lines = vec![];
    let facilities = array(&d.get("facilities-data")["facilities"]);
    if !options.is_empty()
        && let Some(level) = facilities
            .iter()
            .find(|f| f["id"] == "Ranch")
            .and_then(|f| f["level"].as_u64())
    {
        lines.push(format!(
            "从科技 Lv.{level} 的家畜牧场就能开始，挑一只会直接产金币的帕鲁即可。"
        ));
        link(&mut r, "家畜牧场", "#work/facilities/Ranch".into());
    }
    for (_, prob, lo, hi, row) in options.into_iter().take(2) {
        let name = text(row, "zh");
        let amount = if lo == hi {
            lo.to_string()
        } else {
            format!("{lo}–{hi}")
        };
        lines.push(format!(
            "把{name}（{}）分配到家畜牧场。基础档每次产出判定有 {prob}% 概率获得 {amount} 金币。",
            text(row, "en")
        ));
        link(
            &mut r,
            format!("{name}图鉴"),
            format!("#paldex/{}", text(row, "key")),
        );
    }
    if let Some(shop) = array(&money["sellers"])
        .iter()
        .find(|s| s["currency"] == "BountyProof_1" && s["qty"].as_u64().is_some_and(|n| n > 0))
    {
        let seller = text(&game["shops"][shop["shop"].as_str()?], "name");
        lines.push(format!(
            "跑图时也可讨伐通缉犯，去{seller}用 {} 个{}换 {} 金币。",
            shop["cost"].as_u64()?,
            d.item_name(shop["currency"].as_str()?),
            shop["qty"].as_u64()?
        ));
        link(&mut r, "金币兑换货单", "#items/Money/shops".into());
    }
    if let Some(factory) = facilities
        .iter()
        .find(|f| f["id"] == "Gold_Coin_Assembly_Line")
        && let Some(level) = factory["level"].as_u64()
        && let Some(recipe) = array(&money["recipes"])
            .iter()
            .find(|r| array(&r["stations"]).contains(&factory["name"]))
        && let Some(batch) = recipe["qty"].as_u64()
    {
        let costs = array(&recipe["materials"])
            .iter()
            .filter_map(|m| {
                Some(format!(
                    "{} 个{}",
                    m[1].as_u64()?,
                    d.item_name(m[0].as_str()?)
                ))
            })
            .collect::<Vec<_>>();
        if !costs.is_empty() {
            lines.push(format!(
                "后期解锁 Lv.{level} {}后，{}可制 {batch} 金币。",
                text(factory, "name"),
                costs.join("、")
            ));
            link(&mut r, "金币制作配方", "#items/Money".into());
        }
    }
    if lines.is_empty() {
        return None;
    }
    lines.push("牧场适合持续积累金币，讨伐适合主动跑图。要比较每小时收益，还得算产出间隔和你一轮实际耗时。".into());
    r.text = lines.join("\n");
    Some(r)
}
