use super::{AssistantReply, any, field_text};
use crate::assistant_data::chinese_count;
use crate::data::{Data, array, text};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
mod chain;

#[derive(Default, Debug, Clone)]
pub(super) struct RecipeStock {
    product: String,
    stock: HashMap<String, u64>,
    target: Option<u64>,
    chain: Option<chain::Production>,
}

const VERBS: &[&str] = &[
    "制作", "製作", "合成", "打造", "烹饪", "冶炼", "生产", "做", "造", "搓", "烤", "煮", "craft",
    "make", "produce", "cook",
];
const CAPACITY: &[&str] = &[
    "最多",
    "上限",
    "能出几",
    "能出多",
    "能做",
    "能造",
    "能制作",
    "能合成",
    "能搓",
    "够做",
    "可做",
    "可以做",
    "够几",
    "做几",
    "做多少",
    "造多少",
    "how many",
    "maximum",
    "how much can",
];
const DEFICIT: &[&str] = &[
    "还缺",
    "差多少",
    "差哪些",
    "差什么",
    "缺多少",
    "缺什么",
    "缺啥",
    "缺几",
    "还差",
    "补齐",
    "补足",
    "再补",
    "够不够",
    "够吗",
    "missing",
    "short",
    "need more",
];
const GOAL: &[&str] = &[
    "凑到",
    "凑够",
    "凑满",
    "做到",
    "改成",
    "改为",
    "改到",
    "总共",
    "一共想要",
    "目标",
    "想要",
    "我要",
    "提高到",
    "上调到",
    "换成",
    "至少",
    "达到",
    "instead",
    "target",
    "want",
];
const REMAINDER: &[&str] = &[
    "还剩",
    "剩多少",
    "剩下多少",
    "剩几",
    "剩下几",
    "余下",
    "余料",
    "剩余",
    "结余",
    "left over",
    "leftover",
    "left?",
    "left after",
    "remaining materials",
];
const UNITS: &str = "个個份枚支发把块瓶颗件根袋本盒张条只罐桶卷颗顆斤";

fn numeric(c: char) -> bool {
    c.is_ascii_digit() || "零一二两三四五六七八九十百千万".contains(c)
}

fn number(s: &str) -> Option<u64> {
    if s.is_empty() {
        None
    } else if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        chinese_count(s)
    }
}

// Consume a whole, nonnegative integer adjacent to an ingredient or goal.
// Refuse fragments of signed numbers, decimals, or numbers outside u64.
fn leading_number(s: &str) -> Option<u64> {
    let s = s.trim_start();
    let rest = s.trim_start_matches(numeric);
    if rest.starts_with(['.', '点']) && rest.chars().nth(1).is_some_and(numeric) {
        return None;
    }
    number(&s[..s.len() - rest.len()])
}

fn annotations(c: char) -> bool {
    c.is_whitespace() || UNITS.contains(c) || "×x*：:=".contains(c)
}

fn quantity(q: &str, start: usize, end: usize) -> Option<u64> {
    let before = q[..start].trim_end_matches(annotations);
    let prefix = before.trim_end_matches(numeric);
    let left = (!prefix.ends_with(['.', '点', '-', '负']))
        .then(|| number(&before[prefix.len()..]))
        .flatten();
    let suffix = q[end..].trim_start_matches(annotations);
    let suffix = ["现在有", "现有", "有", "剩", "是", "为"]
        .iter()
        .find_map(|s| suffix.strip_prefix(s))
        .unwrap_or(suffix);
    left.or_else(|| leading_number(suffix))
}

fn names(row: &Value) -> Vec<String> {
    let english = text(row, "en").to_ascii_lowercase();
    let mut names = vec![text(row, "name").to_owned(), english.clone()];
    if !english.ends_with('s') {
        names.push(format!("{english}s"));
    }
    names.extend(
        match text(row, "id") {
            "Egg" => &["鸡蛋", "禽蛋", "鸟蛋", "雞蛋"][..],
            "Berries" => &["红莓", "野莓", "浆果", "莓果", "紅莓", "漿果", "berries"],
            "Wood" => &["木头", "木頭"],
            "Stone" => &["石料", "石頭"],
            "CopperIngot" => &["铁锭", "鐵錠", "金屬錠"],
            "Flour" => &["麵粉"],
            _ => &[],
        }
        .iter()
        .map(|s| (*s).to_owned()),
    );
    names.retain(|s| !s.is_empty());
    names.sort_by_key(|s| std::cmp::Reverse(s.len()));
    names
}

fn whole_name(q: &str, name: &str, pos: usize) -> bool {
    if name.is_ascii() {
        return !q[..pos].ends_with(|c: char| c.is_ascii_alphabetic())
            && !q[pos + name.len()..].starts_with(|c: char| c.is_ascii_alphabetic());
    }
    name != "蛋" || !q[pos + name.len()..].starts_with("糕")
}

fn target_quantity(q: &str, allow_short: bool) -> Option<u64> {
    let mut result = None;
    for marker in VERBS.iter().chain(GOAL) {
        for (i, _) in q.match_indices(marker) {
            let tail = q[i + marker.len()..]
                .trim_start_matches(|c: char| c.is_whitespace() || "到够为是有完：:".contains(c));
            if let Some(value) = leading_number(tail) {
                result = Some((i, value)).max(result);
            }
        }
    }
    result.map(|(_, v)| v).or_else(|| {
        let lead = q.trim_start_matches(['那', '要', '做', ' ']);
        let tail = lead
            .trim_start_matches(numeric)
            .trim_start_matches(annotations)
            .trim_matches(|c: char| c.is_whitespace() || "呢吗？！?!.。,".contains(c));
        (allow_short && (tail.is_empty() || DEFICIT.iter().any(|s| tail.starts_with(s))))
            .then(|| leading_number(lead))
            .flatten()
    })
}

fn select_product<'a>(items: &'a [Value], q: &str) -> Option<&'a Value> {
    let mut best = None;
    for row in items
        .iter()
        .filter(|row| !array(&row["recipes"]).is_empty())
    {
        for alias in names(row) {
            for (i, _) in q
                .match_indices(&alias)
                .filter(|(i, _)| whole_name(q, &alias, *i))
            {
                let before = q[..i]
                    .rsplit([',', '，', '、', '。', ';', '；'])
                    .next()
                    .unwrap_or("")
                    .trim_end();
                let after = q[i + alias.len()..]
                    .split([',', '，', '、', '。', ';', '；'])
                    .next()
                    .unwrap_or("")
                    .trim_start();
                let score = if VERBS
                    .iter()
                    .chain(GOAL)
                    .chain(CAPACITY)
                    .any(|s| before.ends_with(s))
                {
                    4
                } else if VERBS.iter().chain(CAPACITY).any(|s| {
                    before.rfind(s).is_some_and(|p| {
                        let tail = before[p + s.len()..].trim();
                        tail.chars()
                            .all(|c| numeric(c) || annotations(c) || "多少几点".contains(c))
                    })
                }) {
                    3
                } else if CAPACITY.iter().any(|s| after.starts_with(s))
                    || before.ends_with("how many")
                {
                    2
                } else {
                    1
                };
                let rank = (score, alias.chars().count(), std::cmp::Reverse(i));
                if best.as_ref().is_none_or(|(old, _)| rank > *old) {
                    best = Some((rank, row));
                }
            }
        }
    }
    best.map(|(_, row)| row)
}

impl RecipeStock {
    pub(super) fn respond(&mut self, question: &str, data: &Data) -> Option<AssistantReply> {
        let q = field_text(question);
        if let Some((plan, result)) = chain::answer(&q, data, self.chain.as_ref()) {
            *self = Self {
                chain: plan,
                ..Self::default()
            };
            return Some(result);
        }
        let cap = any(&q, CAPACITY);
        let deficit = any(&q, DEFICIT);
        let remainder = any(&q, REMAINDER);
        let active = !self.product.is_empty();
        let items = array(&data.get("item-catalog-data")["items"]);
        let mut target = target_quantity(&q, active);
        if !active && !cap && !deficit && target.is_none() {
            return None;
        }
        let previous = items.iter().find(|r| text(r, "id") == self.product);
        let candidate = select_product(items, &q);
        // When providing a missing ingredient (e.g. flour), don't switch to that ingredient's own recipe.
        let material_followup =
            active && target.is_none() && !any(&q, VERBS) && !any(&q, GOAL) && !cap && !deficit;
        let product = if material_followup {
            previous
        } else {
            candidate.or(previous)
        }?;
        let product_id = text(product, "id");
        let same = product_id == self.product;
        let recipes = array(&product["recipes"]);
        let ids: HashSet<_> = recipes
            .iter()
            .flat_map(|r| array(&r["materials"]))
            .filter_map(|m| m[0].as_str())
            .collect();
        let mut supplied = HashMap::new();
        let mut mentions_material = false;
        for row in items.iter().filter(|r| ids.contains(text(r, "id"))) {
            let qty = names(row).iter().find_map(|alias| {
                q.match_indices(alias)
                    .filter(|(i, _)| whole_name(&q, alias, *i))
                    .find_map(|(i, _)| {
                        mentions_material = true;
                        quantity(&q, i, i + alias.len())
                    })
            });
            if let Some(qty) = qty {
                supplied.insert(text(row, "id").to_owned(), qty);
            }
        }
        let remaining_followup = same
            && remainder
            && target.is_none()
            && !cap
            && !deficit
            && (any(
                &q,
                &["材料", "原料", "余料", "material", "left over", "leftover"],
            ) || mentions_material
                || ["还剩多少", "剩多少", "各剩多少", "还剩什么", "还剩哪些"].contains(
                    &q.trim_start_matches('那')
                        .trim_end_matches(['？', '?', '。', '呢', ' ']),
                ));
        if !(cap
            || deficit
            || remaining_followup
            || (same && target.is_some())
            || !supplied.is_empty() && (same || target.is_some()))
        {
            return None;
        }
        if cap
            && !same
            && supplied.is_empty()
            && (!any(&q, VERBS)
                || !any(
                    &q,
                    &[
                        "最多", "上限", "多少", "几", "how many", "maximum", "how much",
                    ],
                )
                || any(
                    &q,
                    &[
                        "几级",
                        "多少级",
                        "多少科技点",
                        "等级",
                        "多久",
                        "多长时间",
                        "how long",
                        "level",
                    ],
                ))
        {
            return None;
        }
        if !same && supplied.is_empty() && !cap {
            return None;
        }
        if material_followup && supplied.is_empty() && !remaining_followup {
            return None;
        }
        if any(
            &q,
            &[
                "又拿到",
                "又获得",
                "又获得了",
                "又加",
                "增加",
                "减少",
                "用掉",
                "消耗了",
                "多了",
                "少了",
                "新增",
                "补了",
                "再加",
                "又有",
                "再来",
                "additional",
                "gained",
                "used up",
            ],
        ) && !supplied.is_empty()
        {
            return Some(AssistantReply::new(
                "请给我变动后各材料的现有数量，我会用新库存继续计算。",
                &[],
            ));
        }
        if !same {
            self.stock.clear();
            self.target = None;
        }
        self.product = product_id.to_owned();
        self.chain = None;
        if (q.contains("只有")
            || q.contains("其余为0")
            || q.contains("其他为0")
            || q.contains("其余没有")
            || q.contains("其他都没有")
            || q.contains("i only have"))
            && !supplied.is_empty()
        {
            for id in &ids {
                self.stock.insert((*id).to_owned(), 0);
            }
        }
        self.stock.extend(supplied);
        if !cap && target.is_none() {
            target = self.target;
        }
        self.target = target;
        let mut lines = Vec::new();
        for (index, recipe) in recipes.iter().enumerate() {
            let batch = recipe["qty"].as_u64().filter(|n| *n > 0)?;
            let materials: Option<Vec<_>> = array(&recipe["materials"])
                .iter()
                .map(|m| Some((m[0].as_str()?, m[1].as_u64().filter(|n| *n > 0)?)))
                .collect();
            let materials = materials?;
            if materials.is_empty() {
                return None;
            }
            let line = self.calculate(data, text(product, "name"), batch, &materials);
            lines.push(if recipes.len() > 1 {
                format!("单用配方 {}：{line}", index + 1)
            } else {
                line
            });
        }
        Some(AssistantReply {
            text: lines.join("\n"),
            links: vec![("查看制作配方".into(), format!("#items/{product_id}"))],
            suggestions: vec![],
        })
    }

    fn calculate(&self, d: &Data, product: &str, batch: u64, materials: &[(&str, u64)]) -> String {
        let mut unknown = Vec::new();
        let mut limits = Vec::new();
        let mut missing = Vec::new();
        let mut completed = None;
        let goal = self
            .target
            .map(|t| u128::from(t).div_ceil(u128::from(batch)));
        for &(id, per_batch) in materials {
            let name = d.item_name(id);
            let Some(&owned) = self.stock.get(id) else {
                unknown.push(name);
                continue;
            };
            limits.push((owned / per_batch, name.clone()));
            if let Some(batches) = goal {
                let short = (batches * u128::from(per_batch)).saturating_sub(u128::from(owned));
                if short > 0 {
                    missing.push(format!("{name} × {short}"));
                }
            }
        }
        let mut answer = String::new();
        if let Some(batches) = goal {
            let output = batches * u128::from(batch);
            answer = format!(
                "目标 {} 个{product}，需要 {batches} 批，共产出 {output} 个。",
                self.target.unwrap()
            );
            if !missing.is_empty() {
                answer.push_str(&format!("还缺{}。", missing.join("、")));
            } else if unknown.is_empty() {
                answer.push_str("现有材料充足。");
                completed = Some(batches);
            }
        } else if let Some(limit) = limits.iter().map(|(n, _)| *n).min() {
            let output = u128::from(limit) * u128::from(batch);
            if unknown.is_empty() {
                let limiting: Vec<_> = limits
                    .iter()
                    .filter(|(n, _)| *n == limit)
                    .map(|(_, name)| name.as_str())
                    .collect();
                answer = format!(
                    "按现有材料最多能做 {output} 个{product}（{limit} 批，每批 {batch} 个）。限制材料：{}。",
                    limiting.join("、")
                );
                completed = Some(u128::from(limit));
            } else {
                answer = format!("仅按已报的材料，{product}的上限为 {output} 个。");
            }
        }
        if !unknown.is_empty() {
            answer.push_str(&format!(
                "请补充{}的现有数量（没有就填 0），才能确认完整结果。",
                unknown.join("、")
            ));
        }
        if let Some(batches) = completed {
            let left: Vec<_> = materials
                .iter()
                .map(|&(id, per_batch)| {
                    let remaining = u128::from(self.stock[id]) - batches * u128::from(per_batch);
                    format!("{} × {remaining}", d.item_name(id))
                })
                .collect();
            answer.push_str(&format!("做完后剩余材料：{}。", left.join("、")));
        }
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use serde_json::json;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn stock_and_targets_persist_in_world_and_reset_on_subject_change() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        let bytes = std::fs::read(root.join("item-catalog-data.json")).unwrap();
        d.tables.insert(
            "item-catalog-data".into(),
            Arc::new(serde_json::from_slice(&bytes).unwrap()),
        );
        let mut conv = Conversation::default();
        let world = json!({"id":"one"});
        let ask = |c: &mut Conversation, q: &str| c.ask_world(q, &d, &world).text;
        let q = "我有木材13、石头9，最多能造多少支箭？";
        let first = ask(&mut conv, q);
        assert!(
            first.contains("40 个箭") && first.contains("限制材料：石头"),
            "{first}"
        );
        assert!(conv.direct_result());
        let next = ask(&mut conv, "如果改成51支还差多少？");
        assert!(
            next.contains("6 批")
                && next.contains("60 个")
                && next.contains("石头 × 3")
                && !next.contains("木材 ×"),
            "{next}"
        );
        let enough = ask(&mut conv, "石头现在有12");
        assert!(enough.contains("材料充足"), "{enough}");
        conv.ask_world("改成八十支？", &d, &json!({"id":"two"}));
        assert!(!conv.direct_result());
        assert!(ask(&mut conv, q).contains("40 个"));
        ask(&mut conv, "你好");
        assert!(!conv.direct_result());
        ask(&mut conv, "那改成51支？");
        assert!(!conv.direct_result());
    }

    #[test]
    fn incomplete_inventory_only_gives_bound_and_recovers_from_reply() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        let bytes = std::fs::read(root.join("item-catalog-data.json")).unwrap();
        d.tables.insert(
            "item-catalog-data".into(),
            Arc::new(serde_json::from_slice(&bytes).unwrap()),
        );
        let mut session = RecipeStock::default();
        let first = session
            .respond("How many arrows can I make with 11 wood?", &d)
            .unwrap();
        assert!(
            first.text.contains("上限为 50") && first.text.contains("补充石头"),
            "{}",
            first.text
        );
        let next = session.respond("七份石头", &d).unwrap();
        assert!(next.text.contains("30 个箭"), "{}", next.text);
        assert!(session.respond("你觉得配种怎么样？", &d).is_none());
    }

    #[test]
    fn inventory_after_whole_batches_uses_each_ingredient_and_the_requested_goal() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        let bytes = std::fs::read(root.join("item-catalog-data.json")).unwrap();
        d.tables.insert(
            "item-catalog-data".into(),
            Arc::new(serde_json::from_slice(&bytes).unwrap()),
        );
        let mut session = Conversation::default();
        let world = json!({"id":"bait"});
        let mut ask = |q: &str| session.ask_world(q, &d, &world).text;
        let incomplete =
            ask("我有16份水栖帕鲁的黏液、19份番茄，最多能做多少优质钓饵，做完材料各剩多少？");
        assert!(
            incomplete.contains("上限为 40")
                && incomplete.contains("补充面粉")
                && !incomplete.contains("做完后剩余材料"),
            "{incomplete}"
        );
        let complete = ask("我有25份面粉");
        for fact in [
            "40 个优质钓饵",
            "水栖帕鲁的黏液 × 4",
            "番茄 × 3",
            "面粉 × 13",
        ] {
            assert!(complete.contains(fact), "{complete}");
        }
        let target = ask("如果改成23份，材料还剩多少？");
        for fact in [
            "3 批",
            "产出 30 个",
            "水栖帕鲁的黏液 × 7",
            "番茄 × 7",
            "面粉 × 16",
        ] {
            assert!(target.contains(fact), "{target}");
        }
        let follow = ask("那番茄还剩多少？");
        assert!(follow.contains("番茄 × 7"), "{follow}");
        let insufficient = ask("目标61份够不够？");
        assert!(
            insufficient.contains("还缺") && !insufficient.contains("做完后剩余材料"),
            "{insufficient}"
        );
        let zero = ask("我只有1份番茄，最多能做多少优质钓饵？");
        for fact in ["0 个优质钓饵", "番茄 × 1", "面粉 × 0"] {
            assert!(zero.contains(fact), "{zero}");
        }
    }

    #[test]
    fn production_carries_actual_yields_and_inventory_across_steps() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        let bytes = std::fs::read(root.join("item-catalog-data.json")).unwrap();
        d.tables.insert(
            "item-catalog-data".into(),
            Arc::new(serde_json::from_slice(&bytes).unwrap()),
        );
        let world = json!({"id":"chain"});
        for (q, expected) in [
            (
                "库存只有 101 份小麦和 5 份现成面粉。把小麦尽量磨成面粉，再把所有面粉都烤成面包，最多有几个面包？最后剩几份小麦？",
                vec!["做出 33 个面粉", "做出 38 个面包", "小麦 × 2"],
            ),
            (
                "我有17木材、13石头，先造一把石制十字镐，再把能用的材料全部做普通箭。最多得到几支箭？最后木材和石头各剩多少？",
                vec![
                    "做出 1 个石制十字镐",
                    "做出 40 个箭",
                    "木材 × 4",
                    "石头 × 0",
                ],
            ),
            (
                "I have 26 wood and 19 stone. First craft a stone pickaxe, then make as many regular arrows as possible with the rest.",
                vec![
                    "做出 1 个石制十字镐",
                    "做出 70 个箭",
                    "木材 × 7",
                    "石头 × 0",
                ],
            ),
            (
                "库存只有 36 木材、23 石头。先造 2 把石制十字镐，再把余料全部做普通箭，然后用剩余木材烧成木炭。",
                vec![
                    "做出 2 个石制十字镐",
                    "做出 60 个箭",
                    "做出 7 个木炭",
                    "石头 × 1",
                ],
            ),
            (
                "库存只有 4 木材、9 石头。先造一把石制十字镐，再用余料全做普通箭。",
                vec!["先做 1 个石制十字镐还缺木材 × 1"],
            ),
            (
                "I have 101 wheat and 5 flour. Mill the wheat into flour, then bake all the flour into bread. How many can I make?",
                vec!["做出 38 个面包", "小麦 × 2"],
            ),
            (
                "How many bread can I make? I have 101 wheat and 5 flour. Mill the wheat into flour, then bake all the flour into bread.",
                vec!["做出 38 个面包", "小麦 × 2"],
            ),
            (
                "玩家等级30，只有小麦 101 份和面粉 5 份。先只用 60 份小麦磨成面粉，再烤成面包。最多多少？",
                vec!["做出 20 个面粉", "做出 25 个面包", "小麦 × 41"],
            ),
            (
                "库存只有101份小麦和5份面粉，先做10份面粉，再烤成面包，最多多少？",
                vec!["做出 10 个面粉", "做出 15 个面包", "小麦 × 71"],
            ),
            (
                "库存只有101份小麦和5份面粉，要保留11份小麦。先磨面粉，再把面粉烤成面包，最多多少？",
                vec!["做出 30 个面粉", "做出 35 个面包", "小麦 × 11"],
            ),
            (
                "我有101份小麦和5份面粉，最多能做几个面包？",
                vec!["原料可先加工", "做出 38 个面包", "小麦 × 2"],
            ),
            (
                "我只有21份木材和4个喷火器官。先烧木炭，然后用木炭做碳纤维。能有多少？",
                vec![
                    "做出 10 个木炭",
                    "做出 2 个碳纤维",
                    "木材 × 1",
                    "喷火器官 × 2",
                ],
            ),
            (
                "仓库只有23个木材、3个喷火器官、1个发电器官、2个精炼金属锭。先烧木炭，再用木炭做碳纤维，最后做生化电池。",
                vec![
                    "做出 11 个木炭",
                    "做出 2 个碳纤维",
                    "做出 1 个生化电池",
                    "碳纤维 × 1",
                ],
            ),
        ] {
            let mut c = Conversation::default();
            let r = c.ask_world(q, &d, &world);
            assert!(
                c.direct_result() && expected.iter().all(|s| r.text.contains(s)),
                "{q}\n{}",
                r.text
            );
        }
        let mut c = Conversation::default();
        let r = c.ask_world(
            "我有21个木材，先做木炭，再用木炭制作碳纤维，最多多少？",
            &d,
            &world,
        );
        assert!(r.text.contains("喷火器官的初始库存"), "{}", r.text);
        let r = c.ask_world("我有两个喷火器官。", &d, &world);
        assert!(r.text.contains("做出 2 个碳纤维"), "{}", r.text);
    }
}
