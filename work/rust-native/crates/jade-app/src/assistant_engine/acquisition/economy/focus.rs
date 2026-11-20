//! Resolve the player's shopping list before any price or quantity calculation.
use super::{
    AssistantReply, Data, Purchase, Value, array, field_text, has, integers, occurs,
    purchase_words, text,
};

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Context,
    Buy,
    Focus,
    Exclude,
}

#[derive(Clone, Copy)]
struct Part<'a> {
    role: Role,
    whole: &'a str,
    body: &'a str,
}

const MARKERS: &[(Role, &[&str])] = &[
    (
        Role::Exclude,
        &[
            "别按",
            "别按照",
            "别算",
            "别拿",
            "别用",
            "别把",
            "别买",
            "不要按",
            "不要算",
            "不要拿",
            "不要用",
            "不要把",
            "不要买",
            "不用按",
            "不用算",
            "不要计入",
            "别计入",
            "不按",
            "不算",
            "不买",
            "不想买",
            "没打算买",
            "不是要买",
            "不是买",
            "不考虑",
            "不包括",
            "排除",
            "而不是",
            "不是",
            "不要",
            "don't count",
            "don't use",
            "don't buy",
            "do not use",
            "do not buy",
            "not the ",
            "rather than ",
            "instead of ",
        ],
    ),
    (
        Role::Focus,
        &[
            "只需要买",
            "只打算买",
            "只想买",
            "只想算",
            "只挑",
            "只选",
            "只取",
            "只考虑",
            "只购买",
            "只买",
            "只算",
            "只按",
            "只要",
            "仅购买",
            "仅买",
            "仅算",
            "就买",
            "就按",
            "改买",
            "改成买",
            "要买的是",
            "买的是",
            "只报",
            "要的是",
            "要买的商品是",
            "实际买",
            "而是买",
            "而是",
            "给我算",
            "我要算",
            "only buy ",
            "buy only ",
            "only the ",
            "only buying ",
        ],
    ),
    (
        Role::Buy,
        &["购买", "买", "采购", "兑换", "buy ", "purchase ", "redeem "],
    ),
];

/// Treat the polite/causative words between a negation and its verb as part of the
/// same instruction. This prevents the affirmative phrase inside "别帮我算" from firing.
fn negated_verb(s: &str) -> Option<&str> {
    let negation = ["不需要", "没必要", "不要", "不用", "不必", "别"]
        .into_iter()
        .find(|n| s.starts_with(n))?;
    let mut tail = &s[negation.len()..];
    for fillers in [
        &["再", "先", "特意"][..],
        &["给我", "帮我", "替我", "帮忙", "给咱"][..],
        &["再", "先", "去"][..],
    ] {
        if let Some(filler) = fillers.iter().find(|f| tail.starts_with(**f)) {
            tail = &tail[filler.len()..];
        }
    }
    let verb = [
        "算作", "算上", "按照", "计入", "计算", "购买", "挑选", "报价", "计价", "算", "按", "买",
        "选", "挑", "拿", "用", "把", "报",
    ]
    .into_iter()
    .find(|v| tail.starts_with(v))?;
    Some(&s[..s.len() - tail.len() + verb.len()])
}

fn pieces(q: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let (mut start, mut body, mut role) = (0, 0, Role::Context);
    let mut offset = 0;
    while offset < q.len() {
        let tail = &q[offset..];
        let c = tail.chars().next().unwrap();
        let comma_in_amount = c == ','
            && q[..offset].ends_with(|v: char| v.is_ascii_digit())
            && tail[1..].starts_with(|v: char| v.is_ascii_digit());
        let delimiter = "，,。；;？！?!\n".contains(c) && !comma_in_amount;
        let marker = MARKERS
            .iter()
            .flat_map(|(r, words)| words.iter().map(move |w| (*r, *w)))
            .filter(|(_, word)| {
                tail.starts_with(word)
                    && (!word.is_ascii()
                        || !q[..offset].ends_with(|v: char| v.is_ascii_alphanumeric()))
            })
            .chain(negated_verb(tail).map(|word| (Role::Exclude, word)))
            .max_by_key(|(_, word)| word.len());
        let marker = marker.filter(|(next, word)| {
            !(word.starts_with('别') && q[..offset].ends_with(['分', '区', '鉴', '判']))
                && (role != Role::Exclude
                    || *next != Role::Buy
                    || ["我", "我想", "我要", "但", "而是", "实际", "改为"]
                        .iter()
                        .any(|m| q[body..offset].ends_with(m)))
        });
        if !delimiter && marker.is_none() {
            offset += c.len_utf8();
            continue;
        }
        if start < offset {
            let mut previous_role = role;
            if let Some((Role::Exclude, word)) = marker {
                let tail = q[offset + word.len()..]
                    .split([',', '，', '。', ';', '；', '?', '？', '!', '！'])
                    .next()
                    .unwrap_or("")
                    .trim();
                if ["", "了", "算", "计价", "计算", "算了", "了谢谢"].contains(&tail) {
                    previous_role = Role::Exclude;
                }
            }
            parts.push(Part {
                role: previous_role,
                whole: &q[start..offset],
                body: &q[body..offset],
            });
        }
        if delimiter {
            offset += c.len_utf8();
            start = offset;
            body = offset;
            role = Role::Context;
        } else if let Some((next, word)) = marker {
            start = offset;
            offset += word.len();
            body = offset;
            role = next;
        }
    }
    if start < q.len() {
        parts.push(Part {
            role,
            whole: &q[start..],
            body: &q[body..],
        });
    }
    parts
}

pub(super) fn shopping_context(q: &str) -> String {
    pieces(q)
        .into_iter()
        .filter(|p| p.role != Role::Exclude)
        .map(|p| p.whole)
        .collect::<Vec<_>>()
        .join("，")
}

/// Keep separate mentions even if a short name also occurs *inside* the long one elsewhere.
fn mentions<'a>(items: &'a [Value], s: &str) -> Vec<&'a Value> {
    let mut spans = Vec::new();
    for item in items {
        for key in ["name", "en"] {
            let name = field_text(text(item, key));
            if name.is_empty() {
                continue;
            }
            for (start, _) in s.match_indices(&name) {
                let end = start + name.len();
                if name.is_ascii()
                    && (s[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
                        || s[end..]
                            .strip_prefix('s')
                            .unwrap_or(&s[end..])
                            .starts_with(|c: char| c.is_ascii_alphanumeric()))
                {
                    continue;
                }
                spans.push((start, end, item));
            }
        }
    }
    spans.sort_by_key(|(start, end, _)| (std::cmp::Reverse(end - start), *start));
    let mut picked = Vec::new();
    for (a, b, item) in spans {
        if picked
            .iter()
            .all(|(x, y, _)| b <= *x || *y <= a || a == *x && b == *y)
        {
            picked.push((a, b, item));
        }
    }
    picked.sort_by_key(|(a, _, _)| *a);
    let mut rows = Vec::new();
    for (_, _, item) in picked {
        if !rows.iter().any(|r: &&Value| r["id"] == item["id"]) {
            rows.push(item);
        }
    }
    rows
}

fn target_name(s: &str) -> &str {
    let s = s.trim().trim_start_matches(['的', '是']).trim();
    let units = [
        "份", "个", "枚", "瓶", "袋", "张", "块", "根", "包", "本", "件", "颗", "支", "发", "组",
        "套", "把", "盒", "斤", "只", "种", "粒",
    ];
    let s = if let Some(n) = integers(s).first().filter(|n| n.start == 0) {
        let rest = s[n.end..].trim_start();
        if let Some(after) = units.iter().find_map(|u| rest.strip_prefix(u)) {
            after.trim_start()
        } else {
            s
        }
    } else {
        s
    };
    let cutoff = [
        "要多少钱",
        "是多少钱",
        "多少钱",
        "什么价",
        "单价",
        "的售价",
        "的价格",
        "的价钱",
        "共需",
        "总价",
        "再算",
        "来算",
        "计算",
        "一共",
        "总共",
        "要用",
        "要花",
        "要付",
        "花多少",
        "需要多少",
        "够不够",
        " cost",
        " with ",
        " for ",
        "算一下",
        "来计价",
        "来报价",
        "来结账",
    ];
    let end = cutoff
        .iter()
        .filter_map(|m| s.find(m))
        .min()
        .unwrap_or(s.len());
    let mut name = s[..end].trim();
    if let Some(amount) = integers(name)
        .last()
        .filter(|n| n.start > 0 && units.contains(&name[n.end..].trim()))
    {
        name = name[..amount.start].trim_end_matches(['要', '来', '共']);
    }
    name.trim()
        .trim_matches(|c: char| c.is_whitespace() || "「」『』“”‘’：:".contains(c))
}

fn contextual_name(item: &Value, token: &str) -> bool {
    if token.chars().count() < 2 {
        return false;
    }
    ["name", "en"]
        .iter()
        .any(|k| occurs(&field_text(text(item, k)), token))
}

pub(super) enum Focused<'a> {
    Item(&'a Value, String),
    Basket(Vec<(&'a Value, String)>),
    Clarify(AssistantReply, Vec<String>, String),
}

fn basket<'a>(items: &'a [Value], rows: &[&'a Value], active: &str) -> Focused<'a> {
    let mut separated = active.to_owned();
    for join in [
        "以及", "还有", "另买", "再买", "、", "和", "跟", "与", " and ",
    ] {
        separated = separated.replace(join, "，");
    }
    let fragments: Vec<_> = separated
        .split(['，', ',', '。', ';', '；', '?', '？'])
        .map(str::trim)
        .collect();
    let shared = fragments.iter().find_map(|piece| {
        ["各买", "各要", "各购", "各来", "各", "每样", "每种", "每款"]
            .iter()
            .find_map(|mark| piece.split_once(mark).map(|(_, s)| s))
            .filter(|s| !integers(s).is_empty())
    });
    let mut result = Vec::new();
    for row in rows {
        let mut calculation = Vec::new();
        for piece in &fragments {
            let named = mentions(items, piece);
            if named.iter().any(|it| it["id"] == row["id"])
                && named
                    .iter()
                    .all(|it| !rows.iter().any(|r| r["id"] == it["id"]) || it["id"] == row["id"])
            {
                calculation.push(*piece);
            }
        }
        if let Some(count) = shared {
            calculation.push(count);
        }
        result.push((*row, calculation.join("，")));
    }
    Focused::Basket(result)
}

pub(super) fn resolve<'a>(
    data: &'a Data,
    q: &str,
    prior: Option<&Purchase>,
    previous_choices: &[String],
) -> Option<Focused<'a>> {
    // Leave non-shopping negation (such as a recipe or a combat comparison) to its original route.
    let pricing = has(
        q,
        &[
            "货单",
            "报价",
            "售价",
            "金币",
            "汪汪币",
            "价格",
            "价钱",
            "多少钱",
            "单价",
            "便宜",
            "更贵",
            "总价",
            "预算",
        ],
    );
    let both = has(
        q,
        &[
            "都要",
            "都买",
            "各买",
            "各要",
            "各来",
            "每样买",
            "每种买",
            "分别买",
            "分别要",
            "一起买",
            "各自",
            "分别多少",
            "分别是",
            "各多少钱",
            "各卖多少",
            "哪个便宜",
            "哪个更便宜",
            "谁更便宜",
            "哪个贵",
            "哪个更贵",
            "哪种便宜",
            "哪种更便宜",
            "比价",
            "比较价格",
        ],
    ) || q
        .split_once("各")
        .is_some_and(|(_, s)| integers(s).first().is_some_and(|n| n.start == 0));
    let ordinal = has(q, &["前者", "后者"]);
    let choice_phrase = has(
        q,
        &[
            "其中一",
            "二选一",
            "选其一",
            "两者之一",
            "任选一种",
            "任选一款",
            "挑一种",
            "选一种",
            "挑一款",
            "选一款",
        ],
    );
    let direct_price = has(
        q,
        &[
            "多少钱",
            "什么价",
            "什么价格",
            "单价",
            "商店价格",
            "商人的价格",
            "商人售价",
            "货单报价",
            "多少金币",
        ],
    );
    if !purchase_words(q) && prior.is_none() && previous_choices.is_empty() && !pricing {
        return None;
    }
    if !both
        && !ordinal
        && !choice_phrase
        && !direct_price
        && !MARKERS[..2].iter().any(|(_, words)| has(q, words))
        && !q
            .char_indices()
            .any(|(i, _)| negated_verb(&q[i..]).is_some())
    {
        return None;
    }
    let parts = pieces(q);
    let items = array(&data.get("item-catalog-data")["items"]);
    let currencies = &data.get("item-catalog-data")["currencies"];
    let prior_item = prior.and_then(|p| items.iter().find(|r| r["id"] == p.item));
    let mut excluded = Vec::new();
    let mut context = Vec::new();
    let mut positive = Vec::new();
    let mut calculation = Vec::new();
    for part in &parts {
        if part.role == Role::Exclude {
            let comparison = ["当作", "当成", "算成", "看成", "认成", "当"]
                .iter()
                .find_map(|marker| part.body.split_once(marker).map(|(_, other)| other))
                .unwrap_or(part.body);
            excluded.extend(mentions(items, comparison));
            continue;
        }
        calculation.push(part.whole);
        if part.role == Role::Focus || part.role == Role::Buy {
            positive.push(*part);
        }
        context.extend(
            mentions(items, part.whole)
                .into_iter()
                .filter(|r| currencies.get(text(r, "id")).is_none()),
        );
    }
    let mut background: Vec<_> = parts
        .iter()
        .take_while(|p| !has(p.whole, &["前者", "后者", "the former", "the latter"]))
        .filter(|p| p.role != Role::Exclude)
        .flat_map(|p| mentions(items, p.whole))
        .filter(|r| currencies.get(text(r, "id")).is_none())
        .fold(Vec::new(), |mut names, row| {
            if !names.iter().any(|r: &&Value| r["id"] == row["id"]) {
                names.push(row);
            }
            names
        });
    if background.is_empty() && (ordinal || has(q, &["这个", "那个", "只买", "只要"])) {
        background = previous_choices
            .iter()
            .filter_map(|id| items.iter().find(|r| r["id"] == *id))
            .collect();
        context.extend(background.iter().copied());
    }
    let prior_context = prior_item.filter(|_| context.is_empty());
    let mut pool = Vec::new();
    for item in context.into_iter().chain(prior_context) {
        if !excluded.iter().any(|r| r["id"] == item["id"])
            && !pool.iter().any(|r: &&Value| r["id"] == item["id"])
        {
            pool.push(item);
        }
    }
    let explicit = positive.iter().any(|p| p.role == Role::Focus);
    // "都在卖" describes stock. An explicit one-of request still needs the player's choice
    // even when they also ask for both sticker prices.
    let one_of = choice_phrase
        || positive.iter().any(|p| {
            p.role == Role::Focus
                && has(
                    p.body,
                    &[
                        "其中",
                        "一种",
                        "一款",
                        "一个品种",
                        "一个种类",
                        "这个",
                        "那个",
                    ],
                )
        });
    if pool.is_empty() && excluded.is_empty() && prior.is_none() {
        return None;
    }
    if !explicit && excluded.is_empty() && !both && !ordinal && !one_of {
        // A recipe can explain why the player is shopping. Only the named good in the
        // merchant's price question (and its quantity) belongs on the order.
        if direct_price && pool.len() > 1 && !super::player_sells(q) {
            let end = q.trim_end_matches(|c: char| c.is_whitespace() || "？！?!。".contains(c));
            if let Some((background, price)) = end.rsplit_once(['，', ',', '。', ';', '；', '\n'])
                && has(background, &["做", "制作", "合成", "加工", "烹饪", "打造", "研磨"])
                && !has(background, &["？", "?", "哪些材料", "要多少", "各需要", "各要多少"])
                && has(price, &["商人", "商店", "商队", "店里", "店老板", "商贩", "货单"])
                && has(price, &["多少钱", "多少金币", "什么价", "单价", "售价", "价格", "价钱"])
            {
                let quoted: Vec<_> = mentions(items, price).into_iter()
                    .filter(|row| currencies.get(text(row, "id")).is_none()).collect();
                if let [item] = quoted.as_slice() {
                    return Some(Focused::Item(*item, price.to_owned()));
                }
            }
        }
        if direct_price
            && pool.len() == 1
            && !super::player_sells(q)
            && !has(
                q,
                &[
                    "昨天",
                    "上次",
                    "之前",
                    "一共赚",
                    "赚钱",
                    "合成",
                    "制作费",
                    "手续费",
                    "开销",
                    "装备维修",
                    "维修费",
                ],
            )
        {
            return Some(Focused::Item(pool[0], q.to_owned()));
        }
        return None;
    }
    if positive.is_empty() && !pricing && prior.is_none() && previous_choices.is_empty() {
        return None;
    }
    let active = if excluded.is_empty() {
        q.to_owned()
    } else {
        calculation.join("，")
    };
    if !explicit && both && !one_of && pool.len() >= 2 {
        return Some(basket(items, &pool, &active));
    }
    let shared_quantity = || {
        parts
            .iter()
            .filter(|p| p.role != Role::Exclude)
            .filter_map(|p| {
                let s = if p.role == Role::Focus && one_of {
                    [
                        "一个品种",
                        "一个种类",
                        "一种",
                        "一款",
                        "一样",
                        "一类",
                        "一个",
                    ]
                    .iter()
                    .find_map(|marker| p.body.split_once(marker).map(|(_, rest)| rest))
                    .unwrap_or(p.whole)
                } else {
                    p.whole
                };
                (!integers(s).is_empty() && mentions(items, s).is_empty()).then_some(s)
            })
            .collect::<Vec<_>>()
            .join("，")
    };
    let mut chosen = Vec::new();
    for part in positive
        .iter()
        .filter(|p| !explicit || p.role == Role::Focus)
    {
        let name = target_name(part.body);
        if name.is_empty() {
            continue;
        }
        let mut matches: Vec<_> = mentions(items, name)
            .into_iter()
            .filter(|it| {
                currencies.get(text(it, "id")).is_none()
                    && !excluded.iter().any(|r| r["id"] == it["id"])
            })
            .collect();
        if matches.is_empty() {
            let suffix = name.trim_end_matches(['的', '款']);
            let pronoun = ["这个", "那个", "这种", "那种", "它", "这款", "那款"]
                .iter()
                .any(|p| name.starts_with(p));
            let order = if name.contains("前者") || name.contains("the former") {
                Some(0)
            } else if name.contains("后者") || name.contains("the latter") {
                Some(1)
            } else {
                None
            };
            matches = if let Some(index) = order {
                if background.len() == 2 {
                    pool.iter()
                        .copied()
                        .filter(|it| it["id"] == background[index]["id"])
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                pool.iter()
                    .copied()
                    .filter(|it| pronoun || contextual_name(it, suffix))
                    .collect()
            };
            if matches.is_empty() && explicit {
                // A choice was requested but no member of that list has been picked yet.
                if one_of && pool.len() > 1 && order.is_none() {
                    continue;
                }
                return Some(Focused::Clarify(
                    AssistantReply::new(
                        &format!(
                            "货单里还无法把「{name}」对应到唯一商品。请给出要买的商品全名，我会按它的价格计算。"
                        ),
                        &[],
                    ),
                    pool.iter().map(|r| text(r, "id").to_owned()).collect(),
                    shared_quantity(),
                ));
            }
        }
        for item in matches {
            if !chosen.iter().any(|r: &&Value| r["id"] == item["id"]) {
                chosen.push(item);
            }
        }
    }
    if chosen.is_empty() && ordinal && background.len() == 2 {
        let index = usize::from(q.contains("后者"));
        chosen = pool
            .iter()
            .copied()
            .filter(|it| it["id"] == background[index]["id"])
            .collect();
    } else if chosen.is_empty() {
        chosen = pool;
    }
    if chosen.len() >= 2 && both && !one_of {
        return Some(basket(items, &chosen, &active));
    }
    if let [item] = chosen.as_slice() {
        // A quantity attached to a different catalogue entry is not the target or the player's stock.
        let active: Vec<_> = parts
            .iter()
            .filter(|p| p.role != Role::Exclude)
            .filter(|p| {
                if p.role != Role::Context || integers(p.body).is_empty() {
                    return true;
                }
                let named: Vec<_> = mentions(items, p.body)
                    .into_iter()
                    .filter(|r| currencies.get(text(r, "id")).is_none())
                    .collect();
                named.is_empty() || named.iter().all(|it| it["id"] == item["id"])
            })
            .map(|p| p.whole)
            .collect();
        return Some(Focused::Item(*item, active.join("，")));
    }
    if chosen.is_empty() || chosen.len() > 1 {
        let names: Vec<_> = chosen.iter().map(|it| text(it, "name")).collect();
        let prompt = if names.is_empty() {
            "请告诉我要买的商品全名；当前只识别到了排除条件。".to_owned()
        } else {
            format!(
                "这段话里仍有多个可选商品（{}）。请写出这次要计算的商品及其数量。",
                names.join("、")
            )
        };
        let quantity = shared_quantity();
        return Some(Focused::Clarify(
            AssistantReply::new(&prompt, &[]),
            chosen.iter().map(|r| text(r, "id").to_owned()).collect(),
            quantity,
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn merchant_quotes_use_the_ingredient_and_count_from_the_price_clause() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        let bytes = std::fs::read(dir.join("item-catalog-data.json")).unwrap();
        d.tables.insert("item-catalog-data".into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        for (q, item, total) in [
            ("我准备做九个蛋糕，商人卖的牛奶一瓶多少金币？", "Milk", "200 金币"),
            ("我想做聚合物。优质帕鲁油在沙漠商人那里两瓶多少钱？", "PalOil", "600 金币"),
        ] {
            let mut c = Conversation::default();
            let r = c.ask_world(q, &d, &serde_json::json!({}));
            assert!(c.direct_result() && r.text.contains(total)
                && r.links.iter().any(|(_,href)|href == &format!("#items/{item}/shops")), "{q} {}", r.text);
        }
        assert!(resolve(&d,"准备做两个蛋糕，需要多少牛奶和蜂蜜？",None,&[]).is_none());
    }
}
