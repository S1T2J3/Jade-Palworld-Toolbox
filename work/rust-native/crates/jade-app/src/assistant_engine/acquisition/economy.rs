use super::{
    AssistantReply, Data, Value, array, field_text, has, item_candidates, link, occurs,
    purchased_items, text,
};
use crate::assistant_breeding::species_mentions;
use crate::assistant_engine::quantities::integers;
use std::collections::BTreeMap;

mod drops;
mod focus;
mod income;

#[derive(Clone, Debug, Default)]
pub(crate) struct EconomySession {
    purchase: Option<Purchase>,
    choices: Vec<String>,
    choice_terms: Option<(String, Vec<String>)>,
    encounter: Option<drops::DropContext>,
}

#[derive(Clone, Debug, Default)]
struct Purchase {
    item: String,
    shops: Vec<String>,
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
    pub(crate) fn focused_purchase(&mut self, d: &Data, q: &str) -> Option<AssistantReply> {
        let selected = focus::resolve(d, q, self.purchase.as_ref(), &self.choices)?;
        let mut shops = named_shops(d, &focus::shopping_context(q));
        let previous_choices = std::mem::take(&mut self.choices);
        let prior_terms = self.choice_terms.take();
        let (reply, next) = match selected {
            focus::Focused::Item(item, mut calculation) => {
                if previous_choices.iter().any(|id| item["id"] == *id)
                    && let Some((quantity, old_shops)) = prior_terms
                {
                    if shops.is_empty() {
                        shops = old_shops;
                    }
                    if integers(q).is_empty() && !quantity.is_empty() {
                        calculation.push('，');
                        calculation.push_str(&quantity);
                    }
                }
                let current = income::purchase_input(&calculation);
                purchase(d, &current, self.purchase.as_ref(), Some(item), &shops)?
            }
            focus::Focused::Basket(items) => {
                self.choices = items
                    .iter()
                    .map(|(item, _)| text(item, "id").to_owned())
                    .collect();
                let mut result = AssistantReply::new("", &[]);
                let mut prices = Vec::new();
                for (item, calculation) in &items {
                    let (answer, state) = purchase(d, calculation, None, Some(item), &shops)?;
                    if !result.text.is_empty() {
                        result.text.push('\n');
                    }
                    result.text.push_str(&answer.text);
                    for (_, href) in answer.links {
                        link(&mut result, text(item, "name"), href);
                    }
                    let sellers: Vec<_> = array(&item["sellers"])
                        .iter()
                        .filter(|s| shops.is_empty() || shops.iter().any(|id| s["shop"] == *id))
                        .collect();
                    if let Some(first) = sellers.first()
                        && sellers.iter().all(|s| {
                            ["cost", "qty", "currency", "once"]
                                .iter()
                                .all(|k| s[k] == first[k])
                        })
                        && let (Some(cost), Some(qty), Some(currency)) = (
                            first["cost"].as_u64(),
                            first["qty"].as_u64(),
                            first["currency"].as_str(),
                        )
                        && qty > 0
                        && cost > 0
                        && first["once"] != true
                    {
                        let packs = state.and_then(|s| {
                            s.target
                                .map(|t| if s.target_packs { t } else { t.div_ceil(qty) })
                        });
                        prices.push((
                            text(item, "name"),
                            u128::from(cost),
                            u128::from(qty),
                            currency,
                            packs,
                        ));
                    }
                }
                if prices.len() == items.len() && prices.iter().all(|v| v.3 == prices[0].3) {
                    let currency = d.item_name(prices[0].3);
                    if let Some(total) = prices.iter().try_fold(0u128, |sum, v| {
                        sum.checked_add(v.1.checked_mul(u128::from(v.4?))?)
                    }) {
                        result
                            .text
                            .push_str(&format!("\n合计 {total} {currency}。"));
                    }
                    if let [a, b] = prices.as_slice() {
                        let comparison = match (a.1 * b.2).cmp(&(b.1 * a.2)) {
                            std::cmp::Ordering::Less => format!("{}的单件价格更便宜。", a.0),
                            std::cmp::Ordering::Greater => format!("{}的单件价格更便宜。", b.0),
                            std::cmp::Ordering::Equal => "两种商品的单件价格相同。".into(),
                        };
                        result.text.push('\n');
                        result.text.push_str(&comparison);
                    }
                }
                (result, None)
            }
            focus::Focused::Clarify(mut reply, choices, quantity) => {
                if choices.len() > 1 {
                    let items = array(&d.get("item-catalog-data")["items"]);
                    for id in &choices {
                        if let Some(item) = items.iter().find(|r| r["id"] == *id)
                            && let Some((quote, _)) =
                                purchase(d, &quantity, None, Some(item), &shops)
                        {
                            reply.text.push_str(&format!(
                                "\n如果选{}，{}",
                                text(item, "name"),
                                quote.text
                            ));
                            for (_, href) in quote.links {
                                link(&mut reply, text(item, "name"), href);
                            }
                        }
                    }
                }
                self.choices = choices;
                self.choice_terms = Some((quantity, shops));
                (reply, None)
            }
        };
        self.purchase = next;
        self.encounter = None;
        Some(reply)
    }

    pub(crate) fn respond(&mut self, d: &Data, question: &str) -> Option<AssistantReply> {
        let q = field_text(question);
        self.choices.clear();
        self.choice_terms = None;
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
        let (r, next) = purchase(
            d,
            &current,
            self.purchase.as_ref(),
            None,
            &named_shops(d, &current),
        )?;
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

// Keep all shop IDs matching the most specific place and merchant role mentioned.
// The catalogue stores some locations in the title, others in the places array.
fn named_shops(d: &Data, q: &str) -> Vec<String> {
    fn place_matches(q: &str, place: &str) -> bool {
        let short = place
            .strip_suffix("之镇")
            .or_else(|| place.strip_prefix("边远"))
            .or_else(|| place.strip_prefix("小型"))
            .unwrap_or(place);
        [place, short]
            .iter()
            .any(|s| s.chars().count() >= 2 && occurs(q, &field_text(s)))
    }
    let mut best = 0;
    let mut ids = Vec::new();
    for (id, shop) in d.get("item-catalog-data")["shops"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let name = text(shop, "name");
        let (head, role) = name.split_once(" · ").unwrap_or((name, ""));
        let place = (!role.is_empty() && !head.ends_with("商人") && place_matches(q, head))
            || array(&shop["places"])
                .iter()
                .filter_map(Value::as_str)
                .any(|p| place_matches(q, p));
        let title = occurs(q, &field_text(head));
        let role_match = !role.is_empty() && occurs(q, &field_text(role));
        let score = 4 * u8::from(place) + 2 * u8::from(role_match) + u8::from(title);
        if score == 0 || score < best {
            continue;
        }
        if score > best {
            best = score;
            ids.clear();
        }
        ids.push(id.clone());
    }
    ids
}

fn purchase(
    d: &Data,
    q: &str,
    prior: Option<&Purchase>,
    focused_item: Option<&Value>,
    shop_ids: &[String],
) -> Option<(AssistantReply, Option<Purchase>)> {
    let follow = prior.is_some() && followup(q);
    let sale = player_sells(q);
    if focused_item.is_none() && !purchase_words(q) && !follow && !sale {
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
    if focused_item.is_none() && nums.is_empty() && !sale && !follow {
        return None;
    }
    let cat = d.get("item-catalog-data");
    let items = array(&cat["items"]);
    let candidates = if focused_item.is_some() {
        Vec::new()
    } else {
        purchase_object(items, q)
    };
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
    let item = if focused_item.is_some() {
        focused_item
    } else if follow
        && picked.is_some_and(|p| cat["currencies"].get(text(p, "id")).is_some())
        && !has(
            q,
            &["买金币", "兑换金币", "buy gold", "buy coins", "redeem gold"],
        )
    {
        previous_item
    } else {
        picked.or(if follow { previous_item } else { None })
    }?;
    if sale {
        return Some((sale_answer(item), None));
    }
    if focused_item.is_none() && !math && !follow && !(purchase_words(q) && !nums.is_empty()) {
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
    if !shop_ids.is_empty() {
        request.shops = shop_ids.to_vec();
    }
    let sellers: Vec<_> = array(&item["sellers"])
        .iter()
        .filter(|s| request.shops.is_empty() || request.shops.iter().any(|id| s["shop"] == *id))
        .collect();
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
        if focused_item.is_some()
            && (ends(
                before,
                &["多少钱", "什么价", "每", "单价是", "每份", "每件"],
            ) || ["name", "en"]
                .iter()
                .map(|k| field_text(text(item, k)))
                .filter(|name| !name.is_empty())
                .any(|name| {
                    q.match_indices(&name)
                        .any(|(start, _)| start <= n.start && n.end <= start + name.len())
                }))
        {
            continue;
        }
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
    if focused_item.is_none() && !recognized && (!follow || !math) {
        return None;
    }
    let mut r = AssistantReply::new("", &[]);
    link(&mut r, "查看商店货单", format!("#items/{id}/shops"));
    let mut offers: BTreeMap<(&str, u64, u64, bool), Vec<&str>> = BTreeMap::new();
    for seller in sellers {
        let (Some(c), Some(qty), Some(cost), Some(shop)) = (
            seller["currency"].as_str(),
            seller["qty"].as_u64(),
            seller["cost"].as_u64(),
            seller["shop"].as_str(),
        ) else {
            continue;
        };
        if qty == 0 || cost == 0 || request.currency.as_deref().is_some_and(|want| want != c) {
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
            "{}货单未列出{}购买{name}的报价，无法计算可购数量或所需金额。",
            if request.shops.is_empty() {
                "当前商店".to_owned()
            } else {
                request
                    .shops
                    .iter()
                    .map(|id| text(&cat["shops"][id], "name"))
                    .collect::<Vec<_>>()
                    .join("、")
            },
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
    fn shopping_negation_covers_its_verb_and_keeps_only_the_affirmative_amount() {
        let d = fixture();
        let world = serde_json::json!({"id":"scope"});
        for verb in [
            "别给我算",
            "不要帮我算",
            "不用替我计算",
            "别先给我按",
            "不用再帮我买",
        ] {
            let q = format!("{verb}九颗绿宝石，我要买两颗红宝石，在沙漠商人那儿一共多少金币？");
            let mut session = crate::assistant_engine::Conversation::default();
            let r = session.ask_world(&q, &d, &world);
            assert!(
                r.text.contains("红宝石在沙漠之镇")
                    && r.text.contains("共需 40000 金币")
                    && !r.text.contains("绿宝石"),
                "{q}\n{}",
                r.text
            );
            let changed = session.ask_world("那改为四颗呢？", &d, &world);
            assert!(
                changed.text.contains("红宝石在沙漠之镇")
                    && changed.text.contains("共需 80000 金币"),
                "{}",
                changed.text
            );
        }
    }

    #[test]
    fn choosing_only_one_preserves_alternative_quotes_quantity_and_merchant() {
        let d = fixture();
        let world = serde_json::json!({"id":"scope"});
        for selection in [
            "我只挑其中一款买两颗",
            "我只选一种买两颗",
            "打算任选一种买两颗",
            "我只挑一种就好了，要两颗",
            "我只选一款就成，来两颗",
        ] {
            let q = format!("渔村的材料商人卖蓝宝石和绿宝石，{selection}，要多少金币？");
            let mut session = crate::assistant_engine::Conversation::default();
            let r = session.ask_world(&q, &d, &world);
            assert!(
                [
                    "如果选蓝宝石",
                    "如果选绿宝石",
                    "请写出",
                    "共需 60000 金币",
                    "共需 80000 金币",
                    "边远渔村 · 材料商人"
                ]
                .iter()
                .all(|s| r.text.contains(s))
                    && !r.text.contains("合计")
                    && !r.text.contains("聚落货单"),
                "{q}\n{}",
                r.text
            );
            let follow = session.ask_world("那就买后者。", &d, &world);
            assert!(
                follow.text.starts_with("绿宝石在边远渔村 · 材料商人")
                    && follow.text.contains("共需 80000 金币"),
                "{}",
                follow.text
            );
        }
        let mut c = crate::assistant_engine::Conversation::default();
        let named = c.ask_world(
            "渔村的材料商人卖红宝石和蓝宝石，我只挑其中的蓝宝石，来两颗。",
            &d,
            &world,
        );
        assert!(
            named.text.starts_with("蓝宝石在边远渔村")
                && named.text.contains("共需 60000 金币")
                && !named.text.contains("如果选"),
            "{}",
            named.text
        );
        let together = c.ask_world(
            "我只选红宝石和蓝宝石这两样都买，各买两颗，总共多少金币？",
            &d,
            &world,
        );
        assert!(
            together.text.contains("合计 100000 金币"),
            "{}",
            together.text
        );
        let specific = c.ask_world("我在沙漠的装备商人这儿，只买三颗蓝宝石。", &d, &world);
        assert!(
            specific.text.contains("沙漠之镇 · 装备商人")
                && specific.text.contains("共需 90000 金币"),
            "{}",
            specific.text
        );
        let missing = c.ask_world("沙漠商人的牛奶多少钱一瓶？", &d, &world);
        assert!(
            missing.text.contains("沙漠之镇")
                && missing.text.contains("未列出")
                && !missing.text.contains("聚落货单"),
            "{}",
            missing.text
        );
    }

    #[test]
    fn focused_shop_selection_uses_the_requested_catalogue_record_and_quantity() {
        let d = fixture();
        let world = serde_json::json!({"id":"shopping"});
        for (q, name, total) in [
            (
                "村里的商人卖小麦，也卖小麦种子。我只买三份种子，按货单一共要多少金币？别按三份小麦算。",
                "小麦种子",
                60,
            ),
            (
                "别按17份低品质药品算。我只买4份药品，一共多少金币？",
                "药品",
                7280,
            ),
            ("只买4份低品质药品，别按18份药品算。", "低品质药品", 3120),
            ("只买4份药品，9包低品质药品我不买。", "药品", 7280),
            ("12包药品不要算。我只买4份低品质药品。", "低品质药品", 3120),
            (
                "商店有药品和低品质药品，我只买三份后者，按货单要多少钱？",
                "低品质药品",
                2340,
            ),
            (
                "商店有药品和低品质药品，我只买三份前者，按货单要多少钱？",
                "药品",
                5460,
            ),
            (
                "商店货单有火箭和火箭弹，我只要这个，别算9个火箭弹。我要3个，总价多少钱？",
                "火箭",
                120,
            ),
        ] {
            let mut session = crate::assistant_engine::Conversation::default();
            let reply = session.ask_world(q, &d, &world);
            assert!(
                session.direct_result()
                    && reply.text.starts_with(name)
                    && reply.text.contains(&format!("共需 {total} 金币")),
                "{q}\n{}",
                reply.text
            );
            let next = session.ask_world("那改成5份呢？", &d, &world);
            assert!(
                next.text.starts_with(name) && next.text.contains("目标 5 件"),
                "{}",
                next.text
            );
        }
    }

    #[test]
    fn focused_shop_selection_keeps_two_purchases_and_questions_apart() {
        let d = fixture();
        let world = serde_json::json!({"id":"shopping"});
        for (q, required) in [
            (
                "药品和低品质药品都买，各买三件，总价多少金币？",
                vec!["药品在", "低品质药品在", "合计 7800 金币"],
            ),
            (
                "牛奶和蛋都买，2份牛奶和3份蛋一共多少金币？",
                vec!["目标 2 件", "目标 3 件", "合计 1000 金币"],
            ),
            (
                "药品和低品质药品哪个更便宜？",
                vec![
                    "每件 780 金币",
                    "每件 1820 金币",
                    "低品质药品的单件价格更便宜",
                ],
            ),
            (
                "商店有药品和低品质药品，我只买一种。多少钱？",
                vec!["药品", "低品质药品", "请写出"],
            ),
        ] {
            let mut session = crate::assistant_engine::Conversation::default();
            let answer = session.ask_world(q, &d, &world);
            assert!(
                session.direct_result() && required.iter().all(|s| answer.text.contains(s)),
                "{q}\n{}",
                answer.text
            );
        }
        let mut session = crate::assistant_engine::Conversation::default();
        let conditional = session.ask_world(
            "药品和低品质药品两种都在卖，我只买其中一种，打算买4份。分别多少钱？",
            &d,
            &world,
        );
        for fact in [
            "请写出",
            "如果选药品",
            "如果选低品质药品",
            "共需 7280 金币",
            "共需 3120 金币",
        ] {
            assert!(conditional.text.contains(fact), "{}", conditional.text);
        }
        assert!(!conditional.text.contains("合计"), "{}", conditional.text);
        let mut session = crate::assistant_engine::Conversation::default();
        let first = session.ask_world("药品和低品质药品各多少钱？", &d, &world);
        assert!(
            first.text.contains("每件 1820 金币") && first.text.contains("每件 780 金币"),
            "{}",
            first.text
        );
        let follow = session.ask_world("那只买3份后者呢？", &d, &world);
        assert!(follow.text.contains("共需 2340 金币"), "{}", follow.text);
        let mut session = crate::assistant_engine::Conversation::default();
        let first = session.ask_world("4份低品质药品多少钱一份？", &d, &world);
        assert!(
            first.text.contains("每件 780 金币") && first.text.contains("共需 3120 金币"),
            "{}",
            first.text
        );
        let follow = session.ask_world("那改成2份呢？", &d, &world);
        assert!(follow.text.contains("共需 1560 金币"), "{}", follow.text);
    }

    #[test]
    fn focused_shop_selection_preserves_budget_and_unsold_identity() {
        let d = fixture();
        let mut c = crate::assistant_engine::Conversation::default();
        let world = serde_json::json!({"id":"shopping"});
        let relation = c.ask_world(
            "商人同时卖小麦与小麦种子，我只买种子5袋，别把小麦种子当成小麦计价。",
            &d,
            &world,
        );
        assert!(
            relation.text.starts_with("小麦种子") && relation.text.contains("共需 100 金币"),
            "{}",
            relation.text
        );
        let budget = c.ask_world(
            "我有550金币，只买两瓶牛奶。别按9份蛋算，最后还剩多少？",
            &d,
            &world,
        );
        assert!(
            budget.text.contains("共需 400 金币") && budget.text.contains("还剩 150 金币"),
            "{}",
            budget.text
        );
        let items = array(&d.get("item-catalog-data")["items"]);
        let pair = items
            .iter()
            .find_map(|long| {
                let long_name = text(long, "name");
                if !array(&long["sellers"]).is_empty() || long_name.contains("设计图") {
                    return None;
                }
                items
                    .iter()
                    .find(|short| {
                        let short_name = text(short, "name");
                        short_name.chars().count() >= 2
                            && long_name != short_name
                            && long_name.contains(short_name)
                            && !array(&short["sellers"]).is_empty()
                            && items
                                .iter()
                                .filter(|r| text(r, "name") == long_name)
                                .count()
                                == 1
                    })
                    .map(|short| (long, short))
            })
            .unwrap();
        let (long, short) = (text(pair.0, "name"), text(pair.1, "name"));
        let q = format!("只买三件{long}，别按九件{short}计算，货单多少钱？");
        let reply = c.ask_world(&q, &d, &world);
        assert!(
            reply.text.contains(&format!("购买{long}的报价")) && !reply.text.contains("共需"),
            "{q}\n{}",
            reply.text
        );
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
