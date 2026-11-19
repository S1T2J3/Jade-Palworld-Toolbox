//! Read-only handbook and warehouse answers for the offline assistant.
use crate::data::{Data, array, text};
use serde_json::Value;
use std::collections::HashMap;
pub(crate) mod recipe_editions;

#[derive(Debug, Clone, Default)]
pub struct ReferenceSession {
    topic: Option<Topic>,
    pub(crate) calculated: bool,
}

#[derive(Debug, Clone)]
enum Topic {
    Pal(String),
    Gathering(String),
    Item { id: String, drops_only: bool },
    OreWeight { owned: bool },
    Work(String),
}

#[derive(Debug, Clone)]
pub struct ReferenceAnswer {
    pub text: String,
    /// Visible label and canonical toolbox destination.
    pub links: Vec<(String, String)>,
}

impl ReferenceAnswer {
    fn new(message: impl Into<String>) -> Self {
        Self {
            text: message.into(),
            links: vec![],
        }
    }
    fn link(&mut self, label: impl Into<String>, route: impl Into<String>) {
        self.links.push((label.into(), route.into()));
    }
}

fn unavailable(d: &Data, title: &str) -> ReferenceAnswer {
    if d.pending > 0 {
        ReferenceAnswer::new(format!("{title}资料正在载入，稍后可以再问一次。"))
    } else {
        ReferenceAnswer::new(format!(
            "当前资料包缺少{title}资料。请检查工具箱的数据文件。"
        ))
    }
}

fn has_any(query: &str, words: &[&str]) -> bool {
    words.iter().any(|word| query.contains(word))
}

fn owned_query(q: &str) -> bool {
    has_any(
        q,
        &[
            "仓库",
            "我有",
            "我手里",
            "我现有",
            "已有",
            "我家的",
            "我养的",
            "我拥有",
        ],
    )
}

fn rate_query(q: &str) -> bool {
    has_any(q, &["概率", "几率", "掉率", "出率", "百分之", "多少趟"])
}

pub(crate) struct CraftCount<'a> {
    target: u64,
    before: &'a str,
    after: &'a str,
}

impl CraftCount<'_> {
    pub(crate) fn for_item(&self, name: &str, allow_followup: bool) -> Option<u64> {
        (self.after.starts_with(name)
            || self.before.ends_with(name)
            || allow_followup && self.implicit_target())
        .then_some(self.target)
    }

    fn implicit_target(&self) -> bool {
        let tail = self
            .after
            .trim_matches(|c: char| c.is_whitespace() || "，。？！,.?!".contains(c));
        (tail.is_empty()
            || ["呢", "的话", "需要", "要多少", "要什么", "要哪些"]
                .iter()
                .any(|p| tail.starts_with(p)))
            && [
                "",
                "那",
                "我",
                "我要",
                "我想",
                "如果",
                "那如果",
                "再",
                "要",
                "想",
                "那我",
                "那我想",
                "改成",
                "一共",
                "总共",
            ]
            .contains(&self.before)
    }
}

pub(crate) fn chinese_count(word: &str) -> Option<u64> {
    let (mut total, mut group, mut digit) = (0u64, 0u64, None);
    let mut last_unit = 10000;
    for c in word.chars() {
        if let Some(n) = "零一二三四五六七八九"
            .chars()
            .position(|v| v == c)
            .map(|n| n as u64)
            .or_else(|| (c == '两').then_some(2))
        {
            if n > 0 && digit.is_some_and(|previous| previous > 0) {
                return None;
            }
            digit = Some(n);
        } else if c == '万' && total == 0 {
            total = (group + digit.take().unwrap_or(0)).checked_mul(10000)?;
            group = 0;
            last_unit = 10000;
        } else {
            let unit = match c {
                '十' => 10,
                '百' => 100,
                '千' => 1000,
                _ => return None,
            };
            if unit >= last_unit {
                return None;
            }
            let n = digit
                .take()
                .or_else(|| (unit == 10 && group == 0).then_some(1))?;
            group += n * unit;
            last_unit = unit;
        }
    }
    Some(total + group + digit.unwrap_or(0))
}

/// Count finished items only when a crafting verb or material question identifies their role.
/// An amount in inventory, a tech level, and the recipe's batch size are separate quantities.
pub(crate) fn counted_craft(q: &str) -> Option<CraftCount<'_>> {
    let number = |c: char| c.is_ascii_digit() || "零一二两三四五六七八九十百千万".contains(c);
    let mut leading_count = None;
    for (i, c) in q.char_indices().filter(|(_, c)| number(*c)) {
        if q[..i].ends_with(number) {
            continue;
        }
        let lead = q[..i].trim();
        let before = [
            "制作", "合成", "生产", "加工", "烹饪", "打造", "做", "造", "煮", "烤",
        ]
        .iter()
        .find_map(|verb| lead.strip_suffix(verb));
        if before.is_none()
            && !(matches!(lead, "" | "那" | "如果")
                && has_any(&q[i..], &["材料", "原料", "配方", "呢"]))
        {
            continue;
        }
        let tail = q[i..].trim_start_matches(number);
        let digits = &q[i..q.len() - tail.len()];
        let tail = tail.trim_start();
        let Some(classifier) = tail
            .chars()
            .next()
            .filter(|v| "个份枚支发把块瓶颗件根袋本盒张条只罐桶卷".contains(*v))
        else {
            continue;
        };
        let target = if c.is_ascii_digit() {
            digits.parse().ok()
        } else {
            chinese_count(digits)
        };
        if let Some(target) = target {
            let count = CraftCount {
                target,
                before: before.unwrap_or(lead).trim_end(),
                after: tail[classifier.len_utf8()..].trim_start(),
            };
            if before.is_some() {
                return Some(count);
            }
            // “十二份面粉能做几个蛋糕” gives stock; “十二个蛋糕要多少材料” gives a target.
            let clause = count
                .after
                .split(['，', ',', '。', '；', ';'])
                .next()
                .unwrap_or("");
            if !has_any(
                clause,
                &[
                    "能做",
                    "可做",
                    "够做",
                    "够不够",
                    "能合成",
                    "能制作",
                    "已经有",
                    "库存",
                ],
            ) && has_any(
                clause,
                &["需要", "要多少", "什么材料", "哪些材料", "配方", "呢"],
            ) {
                leading_count = Some(count);
            }
        }
    }
    leading_count
}

pub(crate) fn recipe_query(q: &str) -> bool {
    !has_any(
        q,
        &["除了制作", "除了合成", "除制作", "不想制作", "不想合成"],
    ) && has_any(
        q,
        &[
            "配方",
            "合成",
            "制作",
            "怎么做",
            "如何做",
            "需要哪些材料",
            "需要什么材料",
            "用什么做",
        ],
    )
}

/// The output in “我有十二块木材，箭怎么制作” takes precedence over its materials.
pub(crate) fn named_recipe<'a>(items: &'a [Value], q: &str) -> Option<&'a Value> {
    if !recipe_query(q) {
        return None;
    }
    items
        .iter()
        .filter(|row| {
            let name = text(row, "name");
            !name.is_empty()
                && q.match_indices(name).any(|(i, _)| {
                    let before = q[..i].trim_end();
                    let after = q[i + name.len()..].trim_start();
                    [
                        "怎么做",
                        "怎么合成",
                        "怎么制作",
                        "的配方",
                        "如何制作",
                        "如何合成",
                    ]
                    .iter()
                    .any(|p| after.starts_with(p))
                        || ["合成", "制作"].iter().any(|p| before.ends_with(p))
                })
        })
        .max_by_key(|row| text(row, "name").len())
}

fn use_query(q: &str) -> bool {
    has_any(
        q,
        &[
            "有什么用",
            "有啥用",
            "用处",
            "用途",
            "干嘛用",
            "做什么用",
            "干什么用",
            "作用",
            "什么效果",
        ],
    )
}

fn asks_for_drops(q: &str) -> bool {
    has_any(
        q,
        &[
            "只想打怪",
            "只打怪",
            "只刷怪",
            "通过打怪",
            "刷什么怪",
            "打什么掉",
            "谁会掉",
            "哪些怪掉",
            "掉落来源",
        ],
    )
}

fn lookup_query(q: &str) -> bool {
    rate_query(q)
        || seller_query(q)
        || use_query(q)
        || asks_for_drops(q)
        || has_any(
            q,
            &[
                "采集",
                "收集",
                "哪里",
                "在哪",
                "去哪",
                "怎么挖",
                "如何挖",
                "怎么获取",
                "如何获取",
                "怎么获得",
                "如何获得",
                "怎么拿",
                "怎么弄",
                "怎么得",
                "怎么刷",
                "在哪里",
                "怎么采",
                "怎么做",
                "需要什么",
                "解锁",
                "多少级",
                "配方",
                "制作",
                "合成",
                "掉落",
                "材料",
                "野外",
                "图鉴",
                "查一下",
                "查下",
            ],
        )
}

fn work_in(q: &str) -> Option<&'static str> {
    [
        "生火",
        "搬运",
        "采矿",
        "浇水",
        "播种",
        "发电",
        "制药",
        "冷却",
        "手工作业",
        "采集",
        "伐木",
    ]
    .into_iter()
    .find(|name| q.contains(name))
}

/// Rank species by base suitability recorded in the dex. The inventory stores
/// individual names and traits, but does not reliably store actual work levels.
pub fn warehouse_work(d: &Data, world: &Value, work: &str) -> ReferenceAnswer {
    if !d.ready("paldex-index-data") {
        return unavailable(d, "工作适性");
    }
    let inventory = array(&world["inventory"]["records"]);
    let mut answer = ReferenceAnswer::new("");
    answer.link("打开当前仓库", "#warehouse");
    if inventory.is_empty() {
        answer.text = format!(
            "当前世界的仓库还没有登记帕鲁。把已有的帕鲁导入或添加到仓库后，就可以筛选{work}候选。"
        );
        return answer;
    }
    let profiles = &d.get("paldex-index-data")["profiles"];
    let mut counts = HashMap::<&str, usize>::new();
    for rec in inventory {
        *counts.entry(text(rec, "speciesKey")).or_default() += 1;
    }
    let mut candidates: Vec<_> = counts
        .into_iter()
        .filter_map(|(key, count)| {
            let row = array(&profiles[key]["work"])
                .iter()
                .find(|entry| entry[0] == work)?;
            Some((row[1].as_u64()?, d.pal_name(key), key, count))
        })
        .collect();
    candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    if candidates.is_empty() {
        answer.text = format!("按物种图鉴的基础适性，当前仓库没有查到能{work}的帕鲁。");
        return answer;
    }
    let mut lines = vec![format!(
        "当前仓库有 {} 种帕鲁能{work}。按物种基础适性从高到低列出前 {} 种：",
        candidates.len(),
        candidates.len().min(5)
    )];
    for (level, name, key, count) in candidates.iter().take(5) {
        lines.push(format!("{name} · {work} Lv.{level} · 仓库 {count} 只"));
        answer.link(name, format!("#paldex/{key}"));
    }
    lines.push("具体个体的强化、工作词条和实际分工会影响表现。".into());
    answer.text = lines.join("\n");
    answer
}

fn ore_weight(d: &Data, world: &Value, owned: bool) -> ReferenceAnswer {
    if !d.ready("partner-skills-data") {
        return unavailable(d, "伙伴技能");
    }
    let mut answer = ReferenceAnswer::new("");
    let mut counts = HashMap::<&str, usize>::new();
    if owned {
        answer.link("打开当前仓库", "#warehouse");
        for rec in array(&world["inventory"]["records"]) {
            *counts.entry(text(rec, "speciesKey")).or_default() += 1;
        }
        if counts.is_empty() {
            answer.text = "当前世界的仓库还没有登记帕鲁。先导入或添加个体，就能筛选能减轻金属矿石重量的伙伴。".into();
            return answer;
        }
    }
    let skills = d.get("partner-skills-data");
    let mut matches = Vec::new();
    for pal in &d.species {
        let key = text(pal, "key");
        let count = counts.get(key).copied().unwrap_or(0);
        if owned && count == 0 {
            continue;
        }
        let skill = &skills[key];
        let effect = text(skill, "effect");
        if !effect.contains("重量会减轻")
            || !(effect.contains("金属矿石") || effect.contains("所有矿石"))
        {
            continue;
        }
        let percent = skill["curves"].as_object().and_then(|curves| {
            curves
                .iter()
                .find(|(token, _)| effect.contains(&format!("{token}%")))?
                .1[0]
                .as_i64()
        });
        matches.push((percent, text(pal, "zh"), key, effect, count));
    }
    matches.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    let mut lines = Vec::new();
    for (_, name, key, effect, count) in matches {
        let label = if owned {
            format!("{name}（仓库 {count} 只）")
        } else {
            name.into()
        };
        lines.push(format!("{label} · {effect}"));
        answer.link(
            format!("{name}的伙伴技能"),
            format!("#details/partner/{key}"),
        );
    }
    answer.text = if lines.is_empty() {
        if owned {
            "当前仓库没有查到能减轻金属矿石重量的伙伴。".into()
        } else {
            "图鉴里没有找到符合条件的伙伴技能。".into()
        }
    } else {
        format!(
            "按伙伴技一级的金属矿石减重效果排序，括号内的百分比范围随伙伴技等级变化。\n{}",
            lines.join("\n")
        )
    };
    answer
}

fn gathering(d: &Data, row: &Value, q: &str) -> ReferenceAnswer {
    let id = text(row, "id");
    let name = text(row, "name");
    let note = text(row, "note");
    let mut result = if rate_query(q) && !note.contains(['%', '％']) {
        ReferenceAnswer::new(format!("《{name}》的野外采集条目没有给出确切概率。{note}"))
    } else {
        ReferenceAnswer::new(format!("{name} · {note}"))
    };
    result.link(format!("{name}采集指南"), format!("#guide/gathering/{id}"));
    let guide = d.get("resource-guides-data");
    for pool_key in array(&row["pools"]).iter().filter_map(Value::as_str) {
        for pool in array(&guide["pools"][pool_key]) {
            let href = text(pool, "href");
            if href.starts_with("#map/") && !result.links.iter().any(|(_, other)| other == href) {
                result.link(format!("{}地图", text(pool, "label")), href);
            }
        }
    }
    result
}

pub(crate) fn seller_query(q: &str) -> bool {
    let q = q.to_lowercase();
    !has_any(&q, &["卖给", "賣給", "sell to", "selling to"])
        && (has_any(&q, &["哪买", "哪能买", "哪里买", "哪裡買", "在哪买", "卖不卖", "卖的", "誰賣", "谁卖", "售价", "售價"])
            || has_any(&q, &["商人", "货单", "貨單"]) && has_any(&q, &["卖", "賣", "买", "買", "价", "價"])
            || q.split(|c: char| !c.is_ascii_alphabetic()).any(|w| ["merchant", "merchants", "buy", "vendor", "vendors"].contains(&w)))
}

pub(crate) fn seller_reference(d: &Data, item: &Value) -> ReferenceAnswer {
    let book = d.get("item-catalog-data");
    let name = text(item, "name");
    let offers: Vec<_> = array(&item["sellers"]).iter().filter_map(|s| {
        let shop = &book["shops"][s["shop"].as_str()?];
        Some(format!("{}：{} {} / {} 件{}", shop["name"].as_str()?, s["cost"].as_u64()?,
            d.item_name(s["currency"].as_str()?), s["qty"].as_u64()?,
            if s["once"] == true { "（限购一次）" } else { "" }))
    }).collect();
    let mut answer = ReferenceAnswer::new(if offers.is_empty() {
        format!("当前图鉴未收录出售{name}的货单。")
    } else {
        format!("{name}在当前图鉴中有以下商人货单：{}", offers.join("；"))
    });
    answer.link("查看商人、售价与坐标", format!("#items/{}/shops", text(item, "id")));
    answer
}

fn item_reference(d: &Data, row: &Value, q: &str, drops_only: bool) -> ReferenceAnswer {
    if seller_query(q) { return seller_reference(d, row); }
    let id = text(row, "id");
    let name = text(row, "name");
    let mut result = ReferenceAnswer::new("");
    result.link(format!("{name}物品图鉴"), format!("#items/{id}"));
    let mut lines = Vec::new();
    if use_query(q) {
        if !d.ready("item-effects-data") {
            result.text = unavailable(d, "物品效果").text;
            return result;
        }
        let effect = &d.get("item-effects-data")["items"][id];
        let description = text(effect, "description").trim();
        if !description.is_empty() && description != text(row, "en") {
            lines.push(description.to_owned());
        }
        lines.extend(
            array(&effect["effects"])
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
        if lines.is_empty() {
            lines.push(format!("当前物品图鉴尚未收录{name}的具体用途。"));
        }
    } else if rate_query(q) {
        lines.push(format!(
            "需要指定掉落{name}的帕鲁或敌人，才能核对对应概率。物品图鉴列出了已收录的获取途径。"
        ));
    } else if drops_only {
        let methods = array(&row["methods"])
            .iter()
            .filter(|method| method[0].as_str().is_some_and(|kind| kind.contains("掉落")))
            .filter_map(|method| method[1].as_str());
        let enemies = array(&d.get("item-catalog-data")["enemyDrops"][id])
            .iter()
            .filter_map(|enemy| enemy[1].as_str());
        let targets: std::collections::BTreeSet<_> = methods.chain(enemies).collect();
        if targets.is_empty() {
            lines.push(format!(
                "{name}的物品条目里没有列出可通过战斗获取的掉落来源。"
            ));
        } else {
            lines.push(format!(
                "只看战斗掉落，{name}的来源有{}。",
                targets.into_iter().collect::<Vec<_>>().join("、")
            ));
        }
    } else {
        if recipe_query(q) {
            return recipe_answer(d, row);
        }
        if !array(&row["recipes"]).is_empty() {
            let recipe = recipe_answer(d, row);
            lines.push(recipe.text);
            result.links = recipe.links;
        }
        let methods: Vec<_> = array(&row["methods"])
            .iter()
            .take(6)
            .filter_map(|method| Some(format!("{} · {}", method[0].as_str()?, method[1].as_str()?)))
            .collect();
        if !methods.is_empty() {
            lines.push(format!("图鉴收录的获取途径包括{}。", methods.join("，")));
        }
    }
    result.text = if lines.is_empty() {
        format!("已找到{name}的物品图鉴，可以查看当前版本收录的信息。")
    } else {
        lines.join("\n")
    };
    result
}

pub(crate) fn recipe_answer(d: &Data, row: &Value) -> ReferenceAnswer {
    scaled_recipe(d, row, None)
}

pub(crate) fn scaled_recipe(d: &Data, row: &Value, target: Option<u64>) -> ReferenceAnswer {
    let name = text(row, "name");
    let recipes = array(&row["recipes"]);
    let mut result = ReferenceAnswer::new("");
    result.link(
        format!("{name}物品图鉴"),
        format!("#items/{}", text(row, "id")),
    );
    let Some(recipe) = recipes.first() else {
        result.text = format!("当前物品图鉴没有收录{name}的制作配方。");
        return result;
    };
    let qty = u128::from(recipe["qty"].as_u64().unwrap_or(1));
    if qty == 0 {
        result.text = format!("{name}的配方缺少有效产量，暂时无法计算用量。");
        return result;
    }
    let batches = target.map_or(1, |amount| u128::from(amount).div_ceil(qty));
    let materials = array(&recipe["materials"])
        .iter()
        .filter_map(|v| {
            Some(format!(
                "{} × {}",
                d.item_name(v[0].as_str()?),
                u128::from(v[1].as_u64()?) * batches
            ))
        })
        .collect::<Vec<_>>()
        .join("、");
    let mut lines = vec![if let Some(target) = target {
        format!(
            "要做 {target} 份{name}，图鉴配方每批产出 {qty} 份，需做 {batches} 批，实际产出 {} 份。\n总共需要{materials}。",
            batches * qty
        )
    } else {
        format!("{name}配方（{qty} 份）需要{materials}。")
    }];
    if let Some(tech) = recipe["tech"].as_u64() {
        lines.push(format!("配方科技等级为 {tech}。"));
    }
    let stations: Vec<_> = array(&recipe["stations"])
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if !stations.is_empty() {
        lines.push(format!("可用设施有{}。", stations.join("、")));
        let first = array(&d.get("facilities-data")["facilities"])
            .iter()
            .filter(|f| stations.contains(&text(f, "name")))
            .filter_map(|f| Some((f["level"].as_u64()?, f)))
            .min_by_key(|(level, _)| *level);
        if let Some((level, f)) = first {
            lines.push(format!("其中{}在科技 {level} 级解锁。", text(f, "name")));
            result.link(
                format!("查看{}", text(f, "name")),
                format!("#work/facilities/{}", text(f, "id")),
            );
        }
    }
    if recipes.len() > 1 {
        lines.push(format!(
            "图鉴共收录 {} 个配方，可打开查看其余材料组合。",
            recipes.len()
        ));
    }
    result.text = lines.join("\n");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn crafting_amounts_distinguish_output_from_levels_and_stock() {
        for question in [
            "我十二级能做蛋糕吗",
            "我 12 级，蛋糕配方是什么",
            "我有12个蜂蜜做蛋糕",
            "十二份面粉能做几个蛋糕，还需要什么材料？",
            "制作12级装备",
        ] {
            assert!(counted_craft(question).is_none(), "{question}");
        }
        for question in [
            "做十二个蛋糕要哪些材料",
            "那做12份呢",
            "十二个蛋糕要多少材料",
            "我 12 级，有八个蜂蜜，想做十二个蛋糕",
        ] {
            assert_eq!(
                counted_craft(question).unwrap().for_item("蛋糕", true),
                Some(12),
                "{question}"
            );
        }
    }

    #[test]
    fn published_guides_and_current_inventory_are_used_for_followups() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "paldex-index-data",
            "resource-guides-data",
            "partner-skills-data",
            "item-catalog-data",
            "item-effects-data",
        ] {
            let entry = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            d.tables.insert(
                id.into(),
                std::sync::Arc::new(serde_json::from_slice(&entry).unwrap()),
            );
        }
        let world = json!({"inventory":{
            "records":[{"speciesKey":"BluePlatypus_Fire"},{"speciesKey":"VolcanicMonster"}],
            "archived":[{"speciesKey":"TentacleTurtle"}]
        }});
        let mut session = ReferenceSession::default();
        let mut ask = |q| session.respond(&d, &world, q).unwrap();
        let gathered = ask("帕鲁矿碎块怎么获得？");
        assert!(gathered.text.contains("蓝色晶体"));
        assert!(
            gathered
                .links
                .iter()
                .any(|(_, href)| href == "#guide/gathering/Pal_crystal_S")
        );
        assert!(
            gathered
                .links
                .iter()
                .any(|(_, href)| href == "#map/category/paldium/palpagos")
        );
        assert!(
            ask("我附近只有普通石头，还能获得它吗？")
                .text
                .contains("普通岩石")
        );
        assert!(ask("掉率多少？").text.contains("没有给出确切概率"));
        assert!(ask("硫磺怎么采集？").text.contains("硫磺矿脉"));
        assert!(ask("伯乐眼镜怎么做？").text.contains("优质作业台"));
        assert!(ask("多少级？").text.contains("34"));
        let usage = ask("伯乐眼镜有什么用？");
        assert!(usage.text.contains("隐藏才能") && usage.text.contains("满分100分"));
        assert!(!usage.text.contains("配方"));
        assert!(ask("用处是什么？").text.contains("隐藏才能"));
        assert!(ask("蛋糕怎么做？").text.contains("配方"));
        let drops = ask("我只想打怪");
        assert!(drops.text.contains("博爱蜥"));
        assert!(!drops.text.contains("料理锅"));
        assert!(ask("还是告诉我配方吧").text.contains("料理锅"));
        let pal = ask("介绍一下绵悠悠");
        assert!(pal.text.contains("棉悠悠"));
        assert!(pal.text.contains("茸茸盾牌"));
        assert!(pal.links.iter().any(|(_, uri)| uri == "#paldex/SheepBall"));
        assert!(ask("它有哪些工作适性？").text.contains("搬运 Lv.1"));
        let all = ask("谁可以减金属负重？");
        let turtle = d.pal_name("TentacleTurtle");
        assert!(all.text.contains(&turtle));
        assert!(
            all.links
                .iter()
                .any(|(_, href)| href == "#details/partner/TentacleTurtle")
        );
        let owned = ask("那我仓库里有哪些？");
        assert!(owned.text.contains(&d.pal_name("VolcanicMonster")));
        assert!(!owned.text.contains(&turtle));
        assert!(!ask("哪只减得最多？").text.contains(&turtle));
        let work = ask("我仓库里谁适合生火？");
        assert!(work.text.contains(&d.pal_name("BluePlatypus_Fire")));
        assert!(work.text.contains("物种基础适性"));
        assert_eq!(work.text, ask("哪种最好？").text);
        assert!(session.respond(&d, &world, "外汇汇率").is_none());
        assert!(session.respond(&d, &world, "写一首关于木材的诗").is_none());
        assert!(
            warehouse_work(&d, &json!({"inventory":{"records":[]}}), "搬运")
                .text
                .contains("没有登记")
        );
    }
}

fn named_row<'a>(rows: &'a [Value], query: &str) -> Option<&'a Value> {
    let question = query.trim_end_matches(['？', '?', '！', '!', '。']);
    let lookup = lookup_query(question);
    rows.iter()
        .filter(|row| {
            let name = text(row, "name");
            if name.is_empty() || name == "-" {
                return false;
            }
            if name == question {
                return true;
            }
            if !lookup {
                return false;
            }
            if name.chars().count() == 1 {
                return question.strip_prefix(name).is_some_and(|rest| {
                    [
                        "怎么",
                        "如何",
                        "在哪",
                        "去哪里",
                        "的配方",
                        "图鉴",
                        "需要什么",
                    ]
                    .iter()
                    .any(|p| rest.starts_with(p))
                }) || question.strip_suffix(name).is_some_and(|rest| {
                    [
                        "获取",
                        "获得",
                        "制作",
                        "合成",
                        "采集",
                        "收集",
                        "掉落",
                        "查一下",
                        "查下",
                    ]
                    .iter()
                    .any(|p| rest.ends_with(p))
                });
            }
            query.contains(name)
        })
        .max_by_key(|row| text(row, "name").len())
}

fn location_query(q: &str) -> bool {
    has_any(
        q,
        &[
            "在哪",
            "哪里",
            "去哪",
            "上哪",
            "什么地方",
            "出没",
            "坐标",
            "点位",
            "栖息地",
            "刷新点",
            "怎么抓",
            "哪里抓",
            "夜间出",
            "白天出",
            "晚上出",
        ],
    )
}

fn pal_topic(q: &str) -> bool {
    has_any(
        q,
        &[
            "介绍",
            "图鉴",
            "工作",
            "干活",
            "适性",
            "伙伴技",
            "能干什么",
            "能做什么",
        ],
    ) || work_in(q).is_some() && has_any(q, &["几级", "多少级", "能不能", "能做", "会不会", "可以"])
}

fn named_pal<'a>(d: &'a Data, q: &str) -> Option<(&'a Value, usize)> {
    let question = q.trim_end_matches(['？', '?', '！', '!', '。']);
    let overview = pal_topic(question) || location_query(question);
    d.species
        .iter()
        .filter_map(|pal| {
            let length = [text(pal, "zh"), text(pal, "en"), text(pal, "key")]
                .into_iter()
                .chain(array(&pal["aliases"]).iter().filter_map(Value::as_str))
                .filter(|name| {
                    !name.is_empty()
                        && (name.eq_ignore_ascii_case(question)
                            || overview && question.contains(*name))
                })
                .map(str::len)
                .max()?;
            Some((pal, length))
        })
        .max_by_key(|(_, len)| *len)
}

fn pal_locations(d: &Data, pal: &Value) -> ReferenceAnswer {
    let key = text(pal, "key");
    let name = text(pal, "zh");
    if !d.ready("spawn-data") {
        return unavailable(d, "出没地点");
    }
    let spawns = d.get("spawn-data");
    let records = array(&spawns["bySpecies"][key]);
    let mut result = ReferenceAnswer::new("");
    result.link(format!("{name}图鉴"), format!("#paldex/{key}"));
    if records.is_empty() {
        result.text = format!("当前出没图没有收录{name}的野外点位。");
        if array(&d.get("capture-data")["raidEggOnly"])
            .iter()
            .any(|v| v == key)
        {
            result.text.push_str("捕捉资料将它列为突袭蛋获取的物种。");
            result.link("捕捉资料", format!("#details/capture/{key}"));
        }
        return result;
    }
    let mut lines = vec![format!("{name}的出没地点举例")];
    let maps = array(&d.get("map-data")["maps"]);
    for entry in records {
        let Some(site) = entry[0]
            .as_u64()
            .and_then(|id| spawns["sites"].get(id as usize))
        else {
            continue;
        };
        let Some(region) = site[0]
            .as_u64()
            .and_then(|i| spawns["mapIds"][i as usize].as_str())
        else {
            continue;
        };
        let (Some(x), Some(y)) = (site[3].as_i64(), site[4].as_i64()) else {
            continue;
        };
        let route = format!("#map/coords/{region}/{x}/{y}");
        if result.links.iter().any(|(_, target)| target == &route) {
            continue;
        }
        let region = maps
            .iter()
            .find(|m| text(m, "id") == region)
            .and_then(|m| m["name"].as_str())
            .unwrap_or(region);
        let kind = if entry[4] == 1 { "头目" } else { "野外" };
        let time = match entry[3].as_u64() {
            Some(0) => "昼夜均可",
            Some(1) => "仅夜间",
            _ => "时段未标注",
        };
        let level = match (entry[1].as_u64(), entry[2].as_u64()) {
            (Some(low), Some(high)) if low == high => format!(" · Lv.{low}"),
            (Some(low), Some(high)) => format!(" · Lv.{low}–{high}"),
            _ => String::new(),
        };
        lines.push(format!("{region}（{x}, {y}） · {kind} · {time}{level}"));
        result.link(format!("在地图上看（{x}, {y}）"), route);
        if lines.len() == 4 {
            break;
        }
    }
    result.link(format!("查看{name}的全部出没点"), format!("#spawns/{key}"));
    result.text = lines.join("\n");
    result
}

fn pal_reference(d: &Data, pal: &Value) -> ReferenceAnswer {
    let key = text(pal, "key");
    let name = text(pal, "zh");
    let mut answer = ReferenceAnswer::new("");
    answer.link(format!("{name}图鉴"), format!("#paldex/{key}"));
    let mut lines = vec![format!("{name} · 图鉴编号 {}", text(pal, "dex"))];
    if d.ready("paldex-index-data") {
        let work = array(&d.get("paldex-index-data")["profiles"][key]["work"])
            .iter()
            .filter_map(|v| Some(format!("{} Lv.{}", v[0].as_str()?, v[1].as_u64()?)))
            .collect::<Vec<_>>();
        if !work.is_empty() {
            lines.push(format!("基础工作适性有{}。", work.join("、")));
        }
    } else {
        lines.push(unavailable(d, "工作适性").text);
    }
    if d.ready("partner-skills-data") {
        let skill = &d.get("partner-skills-data")[key];
        if !text(skill, "name").is_empty() {
            lines.push(format!(
                "伙伴技能「{}」。{}",
                text(skill, "name"),
                text(skill, "effect")
            ));
            answer.link("查看伙伴技能", format!("#details/partner/{key}"));
        }
    } else {
        lines.push(unavailable(d, "伙伴技能").text);
    }
    answer.text = lines.join("\n");
    answer
}

impl ReferenceSession {
    pub(crate) fn remember_gathering(&mut self, id: &str) {
        self.topic = Some(Topic::Gathering(id.to_owned()));
    }

    pub fn respond(&mut self, d: &Data, world: &Value, query: &str) -> Option<ReferenceAnswer> {
        self.calculated = false;
        let q = query.trim();
        if q.is_empty() {
            return None;
        }
        if let Some(work) = work_in(q).filter(|_| owned_query(q)) {
            self.topic = Some(Topic::Work(work.to_owned()));
            return Some(warehouse_work(d, world, work));
        }
        let ore_query = has_any(q, &["金属", "矿石", "金属矿"])
            && has_any(q, &["减重", "负重", "减轻", "重量", "减负"]);
        if ore_query
            || matches!(self.topic, Some(Topic::OreWeight { .. }))
                && (owned_query(q) || has_any(q, &["哪只减", "谁减", "按效果", "减得最多"]))
        {
            if has_any(q, &["锭", "铸块"]) {
                self.topic = None;
                return Some(ReferenceAnswer::new(
                    "伙伴技能资料要区分金属矿石和冶炼制品。可以问“谁能减金属矿石负重”来查矿石对应的技能。",
                ));
            }
            let was_owned =
                !ore_query && matches!(self.topic, Some(Topic::OreWeight { owned: true }));
            let owned = owned_query(q) || was_owned;
            self.topic = Some(Topic::OreWeight { owned });
            return Some(ore_weight(d, world, owned));
        }
        let guides = array(&d.get("resource-guides-data")["gathering"]);
        let items = array(&d.get("item-catalog-data")["items"]);
        if let Some(count) = counted_craft(q) {
            let named = items
                .iter()
                .filter(|row| {
                    let name = text(row, "name");
                    !name.is_empty()
                        && (count.after.starts_with(name) || count.before.ends_with(name))
                })
                .max_by_key(|row| text(row, "name").len());
            let previous = match &self.topic {
                Some(Topic::Item { id, .. }) if count.implicit_target() => {
                    items.iter().find(|row| text(row, "id") == id)
                }
                _ => None,
            };
            if let Some(row) = named.or(previous) {
                self.calculated = !array(&row["recipes"]).is_empty();
                self.topic = Some(Topic::Item {
                    id: text(row, "id").to_owned(),
                    drops_only: false,
                });
                return Some(scaled_recipe(d, row, Some(count.target)));
            }
        }
        // “只有普通石头，也能挖到它吗？” refers to the previous resource.
        // An explicit new resource name in a new question always takes priority.
        if let Some(Topic::Gathering(id)) = &self.topic {
            if id == "Pal_crystal_S"
                && has_any(q, &["普通岩石", "普通石头"])
                && !has_any(q, &["怎么获得石头", "怎么获得普通石头", "怎么采集石头"])
                && named_row(guides, q)
                    .is_none_or(|row| matches!(text(row, "id"), "Stone" | "Pal_crystal_S"))
            {
                if let Some(row) = guides.iter().find(|row| text(row, "id") == id) {
                    return Some(gathering(d, row, q));
                }
            }
        }
        let wants_drops = asks_for_drops(q);
        let recipe_item = named_recipe(items, q);
        let guide_match = if recipe_item.is_some() {
            None
        } else {
            named_row(guides, q)
        };
        let item_match = recipe_item.or_else(|| named_row(items, q));
        let guide_len = guide_match.map_or(0, |row| text(row, "name").len());
        let item_len = item_match.map_or(0, |row| text(row, "name").len());
        if let Some((pal, _)) = named_pal(d, q).filter(|(_, len)| *len >= guide_len.max(item_len)) {
            self.topic = Some(Topic::Pal(text(pal, "key").into()));
            return Some(if location_query(q) {
                pal_locations(d, pal)
            } else {
                pal_reference(d, pal)
            });
        }
        if let Some(row) = guide_match.filter(|_| {
            guide_len >= item_len
                && !((wants_drops || use_query(q) || recipe_query(q) || seller_query(q)) && item_match.is_some())
        }) {
            self.topic = Some(Topic::Gathering(text(row, "id").to_owned()));
            return Some(gathering(d, row, q));
        }
        if let Some(row) = item_match {
            self.topic = Some(Topic::Item {
                id: text(row, "id").to_owned(),
                drops_only: wants_drops,
            });
            return Some(item_reference(d, row, q, wants_drops));
        }
        if let Some(topic) = &self.topic {
            match topic {
                Topic::Pal(key)
                    if location_query(q)
                        && (has_any(q, &["它", "这只", "那只", "这个帕鲁"])
                            || ["那", ""].iter().any(|prefix| {
                                q.strip_prefix(prefix).is_some_and(|rest| {
                                    [
                                        "哪里抓",
                                        "在哪抓",
                                        "哪里找",
                                        "在哪",
                                        "坐标",
                                        "出没点",
                                        "去哪抓",
                                    ]
                                    .contains(&rest.trim_end_matches(['？', '?', '呢', '。']))
                                })
                            })) =>
                {
                    if let Some(pal) = d.pal(key) {
                        return Some(pal_locations(d, pal));
                    }
                }
                Topic::Pal(key)
                    if pal_topic(q)
                        && has_any(q, &["它", "这只", "那只", "伙伴技", "工作适性"]) =>
                {
                    if let Some(pal) = d.pal(key) {
                        return Some(pal_reference(d, pal));
                    }
                }
                Topic::Gathering(id) if wants_drops || use_query(q) || recipe_query(q) => {
                    if let Some(row) = items.iter().find(|row| text(row, "id") == id) {
                        let response = item_reference(d, row, q, wants_drops);
                        self.topic = Some(Topic::Item {
                            id: id.clone(),
                            drops_only: wants_drops,
                        });
                        return Some(response);
                    }
                }
                Topic::Gathering(id)
                    if rate_query(q)
                        || has_any(q, &["地图", "在哪挖", "怎么挖", "去哪采", "需要什么工具"]) =>
                {
                    if let Some(row) = guides.iter().find(|row| text(row, "id") == id) {
                        return Some(gathering(d, row, q));
                    }
                }
                Topic::Item { id, drops_only }
                    if rate_query(q)
                        || use_query(q)
                        || recipe_query(q)
                        || wants_drops
                        || has_any(
                            q,
                            &["怎么做", "怎么拿", "多少级", "配方", "怎么获得", "什么材料"],
                        ) =>
                {
                    if let Some(row) = items.iter().find(|row| text(row, "id") == id) {
                        let filter = wants_drops
                            || *drops_only && !recipe_query(q) && !has_any(q, &["所有", "全部"]);
                        let response = item_reference(d, row, q, filter);
                        self.topic = Some(Topic::Item {
                            id: id.clone(),
                            drops_only: filter,
                        });
                        return Some(response);
                    }
                }
                Topic::Work(work) if has_any(q, &["哪只", "哪种", "最好", "最高", "再查"]) =>
                {
                    return Some(warehouse_work(d, world, work));
                }
                _ => {}
            }
        }
        self.topic = None;
        if !(d.ready("resource-guides-data") && d.ready("item-catalog-data")) && lookup_query(q) {
            return Some(unavailable(d, "采集和物品"));
        }
        None
    }
}
