use super::{
    AssistantReply, Data, Value, array, field_text, has, item_candidates, link, occurs,
    purchased_items, text,
};
use crate::assistant_breeding::species_mentions;
use crate::assistant_engine::quantities::integers;
use std::collections::BTreeMap;

mod drops;
mod income;

#[derive(Clone, Debug, Default)]
pub(crate) struct EconomySession {
    purchase: Option<Purchase>,
    encounter: Option<drops::DropContext>,
}

#[derive(Clone, Debug, Default)]
struct Purchase {
    item: String,
    currency: Option<String>,
    budget: Option<u64>,
    stock: Option<u64>,
    target: Option<u64>,
    stock_packs: bool,
    target_packs: bool,
    additional: bool,
}

fn followup(q: &str) -> bool {
    q.starts_with(['那', '若', '改', '换', '只'])
        || has(
            q,
            &[
                "改成",
                "改为",
                "换成",
                "增加到",
                "减少到",
                "如果",
                "变成",
                "呢",
                "what if",
                "what about",
                "how about",
                "instead",
                "change to",
                "make it",
                "and if",
            ],
        )
}
fn clause_before(s: &str) -> &str {
    s.rsplit([',', '，', '。', ';', '；', '?', '？'])
        .next()
        .unwrap_or(s)
        .trim()
}
fn starts(s: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|p| s.starts_with(p))
}
fn ends(s: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|p| s.ends_with(p))
}
fn purchase_words(q: &str) -> bool {
    has(
        q,
        &[
            "购买",
            "买",
            "采购",
            "兑换",
            "商店",
            "商人",
            "单价",
            "预算",
            "每包",
            "换购",
            "buy",
            "buying",
            "bought",
            "afford",
            "purchase",
            "budget",
            "price",
            "how much for",
            "exchange",
            "redeem",
            "vendor",
            "shop",
            "merchant",
        ],
    )
}

impl EconomySession {
    pub(crate) fn respond(&mut self, d: &Data, question: &str) -> Option<AssistantReply> {
        let q = field_text(question);
        if let Some(reply) = income::answer(d, &q) {
            self.purchase = None;
            self.encounter = None;
            return Some(reply);
        }
        if let Some((r, next)) = drops::respond(d, &q, self.encounter.as_ref()) {
            self.encounter = Some(next);
            self.purchase = None;
            return Some(r);
        }
        let current = income::purchase_input(&q);
        let (r, next) = purchase(d, &current, self.purchase.as_ref())?;
        self.purchase = next;
        self.encounter = None;
        Some(r)
    }
}

fn currency_names(d: &Data, id: &str) -> Vec<String> {
    let mut names = vec![field_text(d.item_name(id).as_ref())];
    if let Some(item) = array(&d.get("item-catalog-data")["items"])
        .iter()
        .find(|i| i["id"] == id)
    {
        names.extend([field_text(text(item, "en")), field_text(text(item, "name"))]);
    }
    if id == "Money" {
        names.extend(
            [
                "金币",
                "金幣",
                "gold coins",
                "gold coin",
                "coins",
                "gold",
                "金",
            ]
            .map(str::to_owned),
        );
    }
    names.retain(|n| !n.is_empty());
    names
}
fn currency_at(q: &str, d: &Data, id: &str) -> bool {
    currency_names(d, id)
        .iter()
        .any(|n| if n == "金" { false } else { occurs(q, n) })
}
fn player_sells(q: &str) -> bool {
    has(
        q,
        &[
            "卖给",
            "卖回",
            "卖出",
            "回收价",
            "卖掉",
            "换回多少",
            "卖多少钱",
            "转卖",
            "sell to",
            "sell my",
            "sell back",
            "selling my",
            "resell",
            "if i sell",
            "can i sell",
            "do i get for selling",
            "selling price",
            "for selling",
            "sell my",
            "i sell",
            "to sell",
        ],
    )
}
fn sale_answer(item: &Value) -> AssistantReply {
    let mut r = AssistantReply::new(
        &format!(
            "商人货单记录的是玩家购入{}的价格。当前资料没有玩家卖给商人的回收单价，因此无法核算卖出收入。",
            text(item, "name")
        ),
        &[],
    );
    link(&mut r, "查看物品", format!("#items/{}", text(item, "id")));
    r
}

fn purchase_object<'a>(items: &'a [Value], q: &str) -> Vec<&'a Value> {
    let blueprint = has(q, &["设计图", "schematic"]);
    for verb in [
        "购买",
        "买",
        "兑换",
        "换取",
        "换成",
        "换到",
        "buy ",
        "purchase ",
        "redeem ",
        "exchange for ",
        "in exchange for ",
    ] {
        for (i, _) in q.match_indices(verb) {
            let tail = &q[i + verb.len()..];
            let cut = [
                ",",
                "，",
                "?",
                "？",
                "。",
                "；",
                ";",
                "做",
                "用来",
                "用于",
                "拿去",
                "来制作",
                "来做",
                "去做",
                "要多少",
                "得花",
                "需花",
                "花多少",
                "得付",
                "需要支付",
                "需要多少",
                "支付",
                "是多少钱",
                "的价格",
                " cost",
                " with ",
                " to make ",
                " to craft ",
                " using ",
                " for ",
            ]
            .iter()
            .filter_map(|end| tail.find(end))
            .min()
            .unwrap_or(tail.len());
            let rows = item_candidates(items, &tail[..cut], blueprint);
            if !rows.is_empty() {
                return rows;
            }
        }
    }
    purchased_items(items, q, blueprint)
}

fn purchase(
    d: &Data,
    q: &str,
    prior: Option<&Purchase>,
) -> Option<(AssistantReply, Option<Purchase>)> {
    let follow = prior.is_some() && followup(q);
    let sale = player_sells(q);
    if !purchase_words(q) && !follow && !sale {
        return None;
    }
    let nums = integers(q);
    let math = has(
        q,
        &[
            "最多",
            "够不够",
            "还差",
            "总价",
            "花费",
            "预算",
            "总共多少钱",
            "要花",
            "多少包",
            "几包",
            "多少瓶",
            "几瓶",
            "能买几个",
            "cost",
            "afford",
            "budget",
            "how many",
            "short",
            "left over",
            "how much",
            "how much more",
        ],
    );
    if nums.is_empty() && !sale && !follow {
        return None;
    }
    let cat = d.get("item-catalog-data");
    let items = array(&cat["items"]);
    let candidates = purchase_object(items, q);
    let nonpayment: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|i| cat["currencies"].get(text(i, "id")).is_none())
        .collect();
    let picked = if let [item] = nonpayment.as_slice() {
        Some(*item)
    } else if let [item] = candidates.as_slice() {
        Some(*item)
    } else {
        None
    };
    let previous_item = prior.and_then(|p| items.iter().find(|i| i["id"] == p.item));
    let item = if follow
        && picked.is_some_and(|p| cat["currencies"].get(text(p, "id")).is_some())
        && !has(
            q,
            &["买金币", "兑换金币", "buy gold", "buy coins", "redeem gold"],
        ) {
        previous_item
    } else {
        picked.or(if follow { previous_item } else { None })
    }?;
    if sale {
        return Some((sale_answer(item), None));
    }
    if !math && !follow && !(purchase_words(q) && !nums.is_empty()) {
        return None;
    }
    let id = text(item, "id");
    let previous = if follow {
        prior.filter(|p| p.item == id)
    } else {
        None
    };
    let switching = follow
        && has(
            q,
            &[
                "改买",
                "换买",
                "换成购买",
                "buy instead",
                "instead buy",
                "same budget",
            ],
        );
    let mut request = previous.cloned().unwrap_or_else(|| Purchase {
        item: id.into(),
        budget: prior.filter(|_| switching).and_then(|p| p.budget),
        currency: prior.filter(|_| switching).and_then(|p| p.currency.clone()),
        ..Default::default()
    });
    let sellers = array(&item["sellers"]);
    let mut currencies: Vec<_> = sellers
        .iter()
        .filter_map(|s| s["currency"].as_str())
        .collect();
    currencies.sort_unstable();
    currencies.dedup();
    let mentioned: Vec<_> = currencies
        .iter()
        .copied()
        .filter(|c| currency_at(q, d, c))
        .collect();
    if let [currency] = mentioned.as_slice() {
        request.currency = Some((*currency).into());
    }
    // An explicit gold budget never buys a listing denominated solely in tokens.
    if id != "Money" && currency_at(q, d, "Money") && mentioned.is_empty() {
        request.currency = Some("Money".into());
    }
    let currency = request
        .currency
        .as_deref()
        .or(if let [only] = currencies.as_slice() {
            Some(*only)
        } else {
            None
        });
    let mut recognized = switching && request.budget.is_some();
    for n in &nums {
        let before = clause_before(&q[..n.start]);
        let after = q[n.end..].trim_start();
        let stripped = after
            .strip_prefix('个')
            .or_else(|| after.strip_prefix('枚'))
            .unwrap_or(after)
            .trim_start();
        let money_marker = currency.is_some_and(|c| {
            currency_names(d, c)
                .iter()
                .any(|name| stripped.starts_with(name))
        });
        let held = ends(
            before,
            &[
                "已有",
                "现有",
                "我有",
                "已经有",
                "已经买了",
                "手上有",
                "库存",
                "还剩",
                "i have",
                "already have",
                "on hand",
                "have",
                "holding",
            ],
        );
        let budget = money_marker
            || ends(
                before,
                &[
                    "预算",
                    "预算为",
                    "预算是",
                    "预算改为",
                    "预算改成",
                    "带着",
                    "预算增加到",
                    "budget",
                    "budget of",
                    "budget is",
                    "budget to",
                ],
            );
        if budget {
            request.budget = Some(n.value);
            recognized = true;
            continue;
        }
        if id != "Money"
            && currency_names(d, "Money")
                .iter()
                .any(|c| stripped.starts_with(c))
        {
            continue;
        }
        let packs = starts(after, &["包", "组", "套", "pack", "bundle", "package"]);
        if held {
            request.stock = Some(n.value);
            request.stock_packs = packs;
            recognized = true;
            continue;
        }
        let units = starts(
            after,
            &[
                "瓶", "个", "份", "枚", "支", "件", "块", "发", "颗", "包", "袋", "只", "单位",
                "bottles", "units", "pieces",
            ],
        );
        let wanted = ends(
            before,
            &[
                "买",
                "购买",
                "想要",
                "凑足",
                "凑齐",
                "目标",
                "目标是",
                "攒够",
                "要有",
                "攒到",
                "换取",
                "兑换",
                "换到",
                "想凑",
                "凑到",
                "want",
                "need",
                "buy",
                "reach",
                "target",
                "target of",
                "for",
                "another",
                "total of",
            ],
        );
        let is_object = occurs(
            after
                .split([',', '，', '?', '？', '.', '。'])
                .next()
                .unwrap_or(after),
            &field_text(text(item, "en")),
        ) || after.starts_with(text(item, "name"));
        if wanted
            || packs
            || units
            || is_object
            || (follow && nums.len() == 1 && request.budget.is_none() && request.target.is_some())
        {
            request.target = Some(n.value);
            request.target_packs = packs;
            request.additional = has(
                before,
                &["再买", "另买", "再要", "额外", "another", "additional"],
            ) || starts(after, &["more ", " extra "]);
            recognized = true;
        } else if follow && nums.len() == 1 && request.budget.is_some() && request.target.is_none()
        {
            request.budget = Some(n.value);
            recognized = true;
        }
    }
    if !recognized
        && follow
        && nums.len() == 1
        && has(
            q,
            &["改为", "改成", "换成", "make it", "change to", "instead"],
        )
        && request.budget.is_some()
        && request.target.is_some()
    {
        return Some((
            AssistantReply::new(
                &format!(
                    "你想把预算改为 {}，还是把{}的目标数量改为 {}？",
                    nums[0].value,
                    text(item, "name"),
                    nums[0].value
                ),
                &[],
            ),
            Some(request),
        ));
    }
    if !recognized && (!follow || !math) {
        return None;
    }
    let mut r = AssistantReply::new("", &[]);
    link(&mut r, "查看商店货单", format!("#items/{id}/shops"));
    let mut offers: BTreeMap<(&str, u64, u64, bool), Vec<&str>> = BTreeMap::new();
    let named_shops: Vec<_> = cat["shops"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, s)| {
            let merchant = text(s, "name").split(" · ").next().unwrap_or("").trim();
            (merchant.chars().count() >= 3 && occurs(q, &field_text(merchant)))
                || array(&s["places"])
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|loc| loc.chars().count() >= 2 && occurs(q, &field_text(loc)))
        })
        .map(|(id, _)| id.as_str())
        .collect();
    for seller in sellers {
        let (Some(c), Some(qty), Some(cost), Some(shop)) = (
            seller["currency"].as_str(),
            seller["qty"].as_u64(),
            seller["cost"].as_u64(),
            seller["shop"].as_str(),
        ) else {
            continue;
        };
        if qty == 0
            || cost == 0
            || request.currency.as_deref().is_some_and(|want| want != c)
            || !named_shops.is_empty() && !named_shops.contains(&shop)
        {
            continue;
        }
        offers
            .entry((c, qty, cost, seller["once"] == true))
            .or_default()
            .push(shop);
    }
    let name = text(item, "name");
    if offers.is_empty() {
        r.text = format!(
            "当前商店货单未列出{}购买{name}的报价，无法计算可购数量或所需金额。",
            request
                .currency
                .as_deref()
                .map(|c| format!("用{}", d.item_name(c)))
                .unwrap_or_default()
        );
        return Some((r, Some(request)));
    }
    let mut lines = Vec::new();
    if currencies.len() > 1 && request.currency.is_none() {
        lines.push("以下按不同货币分别计算，同一预算数额不能跨货币相加。".into());
    }
    for ((c, qty, cost, once), shops) in offers {
        let unit = d.item_name(c);
        let shop = text(&cat["shops"][shops[0]], "name");
        let pricing = if qty == 1 {
            format!("每件 {cost} {unit}")
        } else {
            format!("每包 {qty} 件，每包 {cost} {unit}")
        };
        lines.push(format!("{name}在{shop}的购入价是{pricing}。"));
        if once {
            lines.push("该货单限购一次，以下按本存档尚未买过计算。".into());
        }
        let qty = u128::from(qty);
        let price = u128::from(cost);
        if let Some(b) = request.budget {
            let b = u128::from(b);
            let packs = (b / price).min(if once { 1 } else { u128::MAX });
            let leftover = b - packs * price;
            lines.push(format!(
                "预算 {b} {unit}最多买 {} 件（{packs} 包），花 {} {unit}，余 {leftover} {unit}。",
                packs * qty,
                packs * price
            ));
        }
        if let Some(t) = request.target {
            let stock =
                u128::from(request.stock.unwrap_or(0)) * if request.stock_packs { qty } else { 1 };
            let t = u128::from(t) * if request.target_packs { qty } else { 1 };
            let want = if request.additional {
                t
            } else {
                t.saturating_sub(stock)
            };
            let packs = want.div_ceil(qty);
            if once && packs > 1 {
                lines.push(format!("目标 {t} 件、已有 {stock} 件，需要 {packs} 包。此货单只能买 1 包，仅靠这里无法凑齐。"));
                continue;
            }
            let total = packs * price;
            lines.push(format!("{} {t} 件{}，还需购买 {want} 件，按整包装至少买 {packs} 包（{} 件），共需 {total} {unit}。",if request.additional{"另买"}else{"目标"},if stock>0{format!("、已有 {stock} 件")}else{String::new()},packs*qty));
            if let Some(b) = request.budget {
                let b = u128::from(b);
                if total > b {
                    lines.push(format!("凑齐还差 {} {unit}。", total - b));
                } else {
                    lines.push(format!("预算足够，达到目标后还剩 {} {unit}。", b - total));
                }
            }
        }
    }
    r.text = lines.join("\n");
    Some((r, Some(request)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};
    fn fixture() -> Data {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        for table in [
            "item-catalog-data",
            "paldex-data",
            "ranch-data",
            "facilities-data",
        ] {
            d.tables.insert(
                table.into(),
                Arc::new(
                    serde_json::from_slice(
                        &std::fs::read(dir.join(format!("{table}.json"))).unwrap(),
                    )
                    .unwrap(),
                ),
            );
        }
        d
    }
    #[test]
    fn money_methods_and_current_wallet_are_distinct_from_historical_sales() {
        let d = fixture();
        let world = serde_json::json!({"id":"money"});
        for (q, expected, excluded) in [
            (
                "Easiest money farm mid game? One full breeding farm of Dumud Gild just sold for 900k at the pal merchant. Anyone else have other easy early/mid game money farms?",
                vec![
                    "家畜牧场",
                    "喵丝特",
                    "100%",
                    "10 金币",
                    "通缉犯讨伐证明",
                    "2000 金币",
                ],
                vec!["目标 900000", "450 包"],
            ),
            (
                "新手想刷金币，我上次卖材料赚了三万金币。这阶段有什么赚钱路子？",
                vec!["家畜牧场", "喵丝特", "通缉犯讨伐证明"],
                vec!["目标 30000"],
            ),
            (
                "我昨天买鸡蛋花了9000金币，现在只有650金币。这次想买牛奶，最多买几瓶？",
                vec!["牛奶", "最多买 3 件", "余 50 金币"],
                vec!["最多买 45", "鸡蛋在"],
            ),
            (
                "I sold milk for 3,000 gold. My budget is 550 gold. How many eggs can I buy?",
                vec!["蛋", "最多买 2 件", "余 150 金币"],
                vec!["最多买 15", "牛奶在"],
            ),
            (
                "这次买牛奶的预算是600金币。另外有3000金币还没到账，最多买几瓶？",
                vec!["最多买 3 件"],
                vec!["最多买 15"],
            ),
            (
                "我卖了材料赚了1100金币，想用这笔钱买牛奶，最多几瓶？",
                vec!["最多买 5 件", "余 100 金币"],
                vec!["目标 1100"],
            ),
        ] {
            let mut c = crate::assistant_engine::Conversation::default();
            let result = c.ask_world(q, &d, &world);
            assert!(
                c.direct_result()
                    && expected.iter().all(|s| result.text.contains(s))
                    && excluded.iter().all(|s| !result.text.contains(s)),
                "{q}\n{}",
                result.text
            );
        }
    }
    #[test]
    fn ordinary_shops_use_budget_stock_package_and_payment_currency() {
        let d = fixture();
        let mut s = EconomySession::default();
        let milk=s.respond(&d,"我带 1950 金币去买牛奶做蛋糕，想买 12 瓶。按普通商店的单价，现在最多买几瓶，凑齐 12 瓶还差多少钱？").unwrap();
        for part in [
            "每件 200 金币",
            "最多买 9 件",
            "余 150 金币",
            "共需 2400 金币",
            "还差 450 金币",
        ] {
            assert!(milk.text.contains(part), "{part}: {}", milk.text);
        }
        let change = s.respond(&d, "那预算改成2500金币呢？").unwrap();
        assert!(change.text.contains("最多买 12 件") && change.text.contains("还剩 100 金币"));
        let egg=s.respond(&d,"I have 3 eggs and 1000 gold. I want to buy enough eggs to reach 10. How much am I short?").unwrap();
        assert!(
            egg.text.contains("已有 3 件")
                && egg.text.contains("共需 1400 金币")
                && egg.text.contains("还差 400 金币"),
            "{}",
            egg.text
        );
        let bundles = s
            .respond(
                &d,
                "我有3枚通缉犯讨伐证明，兑换金币，目标是4500个，够不够？",
            )
            .unwrap();
        assert!(
            bundles.text.contains("每包 2000 件")
                && bundles.text.contains("最多买 6000 件")
                && bundles.text.contains("至少买 3 包"),
            "{}",
            bundles.text
        );
        assert!(
            s.respond(&d, "把5瓶牛奶卖给商人能赚多少钱？")
                .unwrap()
                .text
                .contains("回收单价")
        );
        assert!(s.respond(&d, "普通商店牛奶多少钱？").is_none());
        let packs = s
            .respond(&d, "直接买2包金币要多少通缉犯讨伐证明？")
            .unwrap();
        assert!(
            packs.text.contains("4000 件") && packs.text.contains("共需 2 通缉犯讨伐证明"),
            "{}",
            packs.text
        );
        let once = s
            .respond(
                &d,
                "我有45枚通缉犯讨伐证明，想购买3个植入体：工匠精神，够不够？",
            )
            .unwrap();
        assert!(
            once.text.contains("限购一次")
                && once.text.contains("最多买 1 件")
                && once.text.contains("无法凑齐"),
            "{}",
            once.text
        );
    }
    #[test]
    fn combat_expectations_use_repeated_trials_real_rows_and_quantity_ranges() {
        let d = fixture();
        let mut s = EconomySession::default();
        let souls=s.respond(&d,"按普通噬魂兽的基础掉落表，我连打 20 只，平均能拿多少个小型帕鲁之魂？是不是打满 20 只就保底有这么多？").unwrap();
        for part in [
            "小型帕鲁之魂",
            "15%",
            "期望 3 个",
            "保底 0 个",
            "最多 20 个",
        ] {
            assert!(souls.text.contains(part), "{part} {}", souls.text);
        }
        let q = s.respond(&d, "What about killing 7 instead?").unwrap();
        assert!(q.text.contains("期望 1.05 个"));
        let bone = s
            .respond(&d, "击败20只普通噬魂兽，骨头期望是多少？")
            .unwrap();
        assert!(bone.text.contains("期望在 20–40 个之间") && bone.text.contains("保底 20 个"));
        let fixed = s
            .respond(
                &d,
                "If I kill 8 Maraith, how many crude oil drops should I expect?",
            )
            .unwrap();
        assert!(fixed.text.contains("期望 4 个") && fixed.text.contains("最多 8 个"));
        let alternate = s
            .respond(&d, "打100只70级念影喵，漂亮的古代文明遗物平均掉多少？")
            .unwrap();
        assert!(
            alternate.text.contains("Lv.70")
                && alternate.text.contains("3.33%")
                && alternate.text.contains("3.33–9.99")
                && alternate.text.contains("保底 0")
        );
        let tiers = s
            .respond(&d, "普通念影喵打100只，漂亮的古代文明遗物平均掉多少？")
            .unwrap();
        assert!(
            tiers.text.contains("这一档未列")
                && tiers.text.contains("Lv.70")
                && tiers.text.contains("以下各档分别计算")
        );
        assert!(s.respond(&d, "那我的据点怎么配种呢？").is_none());
        assert!(s.respond(&d, "牛奶有几种来源？").is_none());
    }

    #[test]
    fn chance_thresholds_are_not_encounter_counts_or_expected_items() {
        let d = fixture();
        let mut s = EconomySession::default();
        let answer=s.respond(&d,"普通黑鸦隐士只打三只，按基础掉落、每次独立，至少出一个小型帕鲁之魂的机会能超过 5% 吗？算给我看。").unwrap();
        assert!(
            answer.text.contains("2.9701%") && answer.text.contains("低于 5%"),
            "{}",
            answer.text
        );
        let again = s.respond(&d, "那概率是否高于2.5%呢？").unwrap();
        assert!(
            again.text.contains("计算 3 次") && again.text.contains("高于 2.5%"),
            "{}",
            again.text
        );
        let expected = s
            .respond(
                &d,
                "打20只普通噬魂兽，按基础掉落，小型帕鲁之魂的期望数量能超过4个吗？",
            )
            .unwrap();
        assert!(
            expected.text.contains("期望数量低于 4 个"),
            "{}",
            expected.text
        );
        let multiple = s
            .respond(
                &d,
                "基础掉落，击败3只黑鸦隐士，至少出2个小型帕鲁之魂的机会是多少？",
            )
            .unwrap();
        assert!(
            !multiple.text.contains("至少掉一次的概率") && multiple.text.contains("累计多件"),
            "{}",
            multiple.text
        );
    }
}
