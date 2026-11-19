//! Per-machine interactions. Recipe stations, build costs and reward rolls describe different operations.
//! Recycler boost cross-check (also shown by the bundled relic calculator):
//! https://progametalk.com/palworld/how-to-get-ancient-relic-recycler/
//! https://gamecodex.com.br/en/estrutura/conversor-de-reliquias-antigas/reciclagem
use super::{AssistantReply, field_text};
use crate::data::{Data, array, text};
use serde_json::Value;
mod overview;
pub(super) use overview::construction_line;

const RECYCLER: &str = "Ancient_Relic_Recycler";
const WATER: &str = "WorldTreeHolyWater";

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

fn english(q: &str) -> bool {
    !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
}

fn name<'a>(item: &'a Value, en: bool) -> &'a str {
    let english_name = text(item, "en");
    if en && !english_name.is_empty() {
        english_name
    } else {
        text(item, "name")
    }
}

// Exact words keep "ore" out of "more" and "mill" out of "million".
fn span(q: &str, term: &str) -> Option<(usize, usize)> {
    let term = field_text(term);
    if term.chars().count() < 2 {
        return None;
    }
    q.match_indices(&term).find_map(|(i, _)| {
        let mut end = i + term.len();
        if term.is_ascii() {
            if q[..i].ends_with(|c: char| c.is_ascii_alphanumeric()) {
                return None;
            }
            if q[end..].starts_with('s') {
                end += 1;
            }
            if q[end..].starts_with(|c: char| c.is_ascii_alphanumeric()) {
                return None;
            }
        }
        Some((i, end))
    })
}

pub(super) fn facilities<'a>(d: &'a Data, q: &str) -> Vec<(&'a Value, usize, usize)> {
    let mut found = Vec::new();
    for f in array(&d.get("facilities-data")["facilities"]) {
        let id = text(f, "id");
        let full = id.replace('_', " ");
        let mut words = vec![text(f, "name"), full.as_str()];
        match id {
            "Palbox" => words.extend(["pal box", "帕鲁箱", "据点终端"]),
            "Primitive_Furnace" => words.extend(["basic furnace", "原始炉子", "原始熔炉"]),
            "Stone_Pit" => words.extend(["stone quarry", "采石坑"]),
            "Logging_Site" => words.extend(["lumber site", "lumber camp", "伐木营地"]),
            "Ore_Mining_Site" => words.extend(["ore mine", "矿石采矿场"]),
            "Electric_Furnace" => words.extend(["electric smelter", "电炉", "电熔炉"]),
            "Improved_Furnace" => words.extend(["improved smelter", "改良炉", "改良熔炉"]),
            "Cold_Food_Box" => words.extend(["refrigerated feed box", "cold feed box", "冷藏饲料箱", "冷藏饲料盒"]),
            "Egg_Incubator" => words.extend(["egg hatcher", "孵蛋器", "孵蛋机"]),
            "Ancient_Farm" => words.extend(["ancient farm pod", "古代农场", "古代栽培器"]),
            "Ranch" => words.extend(["家畜场", "放牧场"]),
            _ => {}
        }
        if id == RECYCLER {
            words.extend([
                "古代遗物转换器",
                "遗物转换器",
                "遗物转化器",
                "遗物回收机",
                "遗物回收器",
                "遗物回收装置",
                "relic recycler",
                "relic converter",
                "relic processor",
            ]);
        }
        if let Some((a, b)) = words
            .iter()
            .filter_map(|w| span(q, w))
            .max_by_key(|(a, b)| b - a)
        {
            found.push((f, a, b));
        }
    }
    found.sort_by_key(|(_, a, b)| std::cmp::Reverse(b - a));
    let mut distinct = Vec::new();
    for (f, a, b) in found {
        if distinct.iter().all(|(_, x, y)| b <= *x || *y <= a) {
            distinct.push((f, a, b));
        }
    }
    distinct.sort_by_key(|(_, a, _)| *a);
    distinct
}

pub(crate) fn facility<'a>(d: &'a Data, q: &str) -> Option<(&'a Value, usize, usize)> {
    let matches = facilities(d, q);
    (matches.len() == 1).then(|| matches[0])
}

fn items<'a>(d: &'a Data, q: &str) -> Vec<&'a Value> {
    let mut matches = Vec::new();
    for item in array(&d.get("item-catalog-data")["items"]) {
        let mut words = vec![text(item, "name"), text(item, "en")];
        match text(item, "id") {
            WATER => words.extend(["holy water", "圣水"]),
            "Wood" => words.push("木头"),
            "Stone" => words.push("石材"),
            _ => {}
        }
        if let Some((a, b)) = words
            .iter()
            .filter_map(|w| span(q, w))
            .max_by_key(|(a, b)| b - a)
        {
            matches.push((item, a, b));
        }
    }
    matches.sort_by_key(|(_, a, b)| std::cmp::Reverse(b - a));
    let mut chosen = Vec::new();
    for (item, a, b) in matches {
        if chosen.iter().all(|(_, x, y)| b <= *x || *y <= a) {
            chosen.push((item, a, b));
        }
    }
    chosen.sort_by_key(|(_, a, _)| *a);
    chosen.into_iter().map(|(item, _, _)| item).collect()
}

fn relationship(q: &str) -> bool {
    if has(
        q,
        &[
            "多久",
            "持续",
            "几分钟",
            "长时间",
            "duration",
            "how long",
            "lasts",
            "last for",
            "stack",
            "叠加",
        ],
    ) {
        return true;
    }
    has(
        q,
        &[
            "放",
            "塞",
            "投入",
            "投进",
            "加进",
            "加入",
            "添加",
            "装进",
            "喂",
            "加速",
            "效率",
            "耗时",
            "用",
            "建",
            "做",
            "加工",
            "材料",
            "产出",
            "生产",
            "获得",
            "掉率",
            "概率",
            "区别",
            "差别",
            "差异",
            "作用",
            "效果",
            "消耗",
            "转成",
            "配方",
            "制作",
            "提炼",
            "转换",
            "制成",
            "能出",
            "能开",
            "开出",
            "会出",
            "合成",
            "put",
            "plac",
            "insert",
            "feed",
            "use",
            "using",
            "with or without",
            "loot",
            "reward",
            "boost",
            "speed",
            "faster",
            "chance",
            "odds",
            "differen",
            "affect",
            "effect",
            "improv",
            "work",
            "ingredient",
            "material",
            "build",
            "construct",
            "input",
            "output",
            "process",
            "craft",
            "convert",
            "make",
            "produce",
            "need",
            "require",
            "get from",
            "get a",
            "turn into",
            "turns into",
            "consume",
            "yield",
            "power",
            "fuel",
        ],
    )
}

fn quantity_request(q: &str) -> bool {
    has(
        q,
        &[
            "最多",
            "还差",
            "够做",
            "几份",
            "做几",
            "够不够",
            "一共需要",
            "批量",
            "how many can",
            "how many could",
            "how many more",
            "short of",
            "how much more",
            "in total",
            "batches",
        ],
    )
}

pub(super) fn recipe_line(d: &Data, output: &Value, recipe: &Value, en: bool) -> String {
    let all = array(&d.get("item-catalog-data")["items"]);
    let ingredients: Vec<_> = array(&recipe["materials"])
        .iter()
        .map(|m| {
            let key = m[0].as_str().unwrap_or("");
            let label = all
                .iter()
                .find(|i| text(i, "id") == key)
                .map(|i| name(i, en))
                .unwrap_or(key);
            format!("{} × {}", label, m[1])
        })
        .collect();
    format!(
        "{} → {} × {}",
        ingredients.join(" + "),
        name(output, en),
        recipe["qty"]
    )
}

fn add_link(links: &mut Vec<(String, String)>, label: &str, route: String) {
    if !links.iter().any(|(_, r)| r == &route) {
        links.push((label.to_owned(), route));
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct FacilitySession {
    // At present the recycler water is the only documented machine-specific boost.
    // Never inherit its bonus when another machine or another item is named.
    recycler_water: bool,
}

impl FacilitySession {
    pub(super) fn respond(&mut self, d: &Data, q: &str) -> Option<AssistantReply> {
        let followup = std::mem::take(&mut self.recycler_water);
        if let Some(reply) = answer(d, q) {
            self.recycler_water = reply
                .links
                .iter()
                .any(|(_, r)| r == &format!("#work/facilities/{RECYCLER}"))
                && reply
                    .links
                    .iter()
                    .filter(|(_, r)| r.starts_with("#items/"))
                    .count()
                    == 1
                && reply
                    .links
                    .iter()
                    .any(|(_, r)| r == "#items/WorldTreeHolyWater");
            return Some(reply);
        }
        if !followup || !modifier_followup(q) {
            return None;
        }
        let resolved = if english(q) {
            format!("Put World Tree Holy Water in the Ancient Relic Recycler: {q}")
        } else {
            format!("世界树圣水放入古代文明遗物转换器：{q}")
        };
        let reply = answer(d, &field_text(&resolved))?;
        self.recycler_water = true;
        Some(reply)
    }
}

// Only a short, elliptical question about the last modifier can retain context.
// Any explicit noun outside this vocabulary (including another Pal/item/station)
// starts a new subject, even when it also asks about odds or speed.
fn modifier_followup(q: &str) -> bool {
    if q.chars().count() > 110
        || !has(
            q,
            &[
                "奖励", "稀有", "掉率", "概率", "几率", "加速", "速度", "效率", "耗时", "持续",
                "叠加", "多久", "boost", "speed", "faster", "chance", "odds", "loot", "reward",
                "drop", "last", "stack", "duration", "how long",
            ],
        )
    {
        return false;
    }
    let words = concat!(
        "那么 这样 这个 那些 它们 那 这 它 会不会 是否 可以 能否 会 也 能 要 不 有 每件 每次 一件 影响 改变 提升 提高 增加 变 更 稀有 奖励 掉落 掉率 概率 几率 数量 ",
        "产出 产量 加速 速度 效率 加工 耗时 持续 叠加 多久 多长 时间 的 吗 呢 啊 多少 快 ",
        "and what about the a an its their those it they that this does do is are at also will would can ",
        "how much many long take for each per relic some any get give work processing process time make ",
        "higher increases increase improves improve boosts boost speed faster chances chance odds of ",
        "rare loot rewards reward drops drop rates rate lasts last stack duration change affect quantity ",
        "quantities amount more better ",
    );
    let mut rest = q;
    loop {
        rest = rest.trim_start_matches(|c: char| {
            c.is_whitespace() || matches!(c, '?' | '？' | ',' | '，' | '.' | '。' | '!' | '！')
        });
        if rest.is_empty() {
            return true;
        }
        let Some(word) = words
            .split_whitespace()
            .filter(|w| rest.starts_with(*w))
            .max_by_key(|w| w.len())
        else {
            return false;
        };
        rest = &rest[word.len()..];
    }
}

pub(super) fn answer(d: &Data, question: &str) -> Option<AssistantReply> {
    let (title, body) = question.split_once('\n').unwrap_or((question, ""));
    // Several explicit building questions may occupy separate lines; ordinary
    // community-post background must not contribute incidental station names.
    if !body.is_empty() && body.chars().count() <= 190
        && body.lines().filter(|line| !line.trim().is_empty()).all(|line| line.trim_end().ends_with(['?', '？']))
        && !facilities(d, title).is_empty()
        && let Some(reply) = overview::answer(d, question)
    {
        return Some(reply);
    }
    if let Some(reply) = overview::answer(d, title) { return Some(reply); }
    // A background list of materials and existing facilities does not ask how to
    // use those materials in a machine. Require a station in the actual request.
    if title != question && facility(d, title).is_none() {
        return None;
    }
    let q = question;
    if !relationship(q) || quantity_request(q) {
        return None;
    }
    let (f, a, b) = facility(d, q)?;
    let machine = text(f, "id");
    // Avoid reading the "stone" in "Stone Pit" as the player's named material.
    let outside = format!("{} {}", &q[..a], &q[b..]);
    if !relationship(&outside)
        || has(
            q,
            &[
                "工作适应性",
                "哪个帕鲁",
                "什么帕鲁",
                "哪种帕鲁",
                "哪个商人",
                "坐标",
                "which pal",
                "what pal",
                "best pal",
                "coordinates",
                "which merchant",
                "work suitability",
            ],
        )
    {
        return None;
    }
    if has(q, &["base layout", "base setup", "base locations", "base location", "where to build", "place to build", "where should i build", "starting a base", "location for my base", "set up my base", "looking for a spot", "optimize food", "optimizing food", "best food", "据点选址", "基地选址", "建在哪里", "基地布局", "据点布局", "供餐", "推荐食物", "喂什么"]) {
        return None;
    }
    let targets = items(d, &outside);
    if targets.is_empty() || targets.len() > 3 {
        return None;
    }
    let en = english(q);
    let station = if en {
        machine.replace('_', " ")
    } else {
        text(f, "name").to_owned()
    };
    let mut links = vec![(station.clone(), format!("#work/facilities/{machine}"))];
    let mut lines = Vec::new();
    if has(q, &["几级","多少级","解锁等级","科技等级","what level","unlock level","level to unlock"])
        && let Some(level) = f["level"].as_u64()
    {
        lines.push(if en { format!("{station} unlocks at technology level {level}.") }
            else { format!("{station}的科技解锁等级是 Lv.{level}。") });
    }
    let catalog = d.get("item-catalog-data");
    let relic = d.get("relic-data");
    let tier = array(&relic["tiers"])
        .iter()
        .find(|t| targets.iter().any(|i| i["id"] == t["id"]));
    for item in &targets {
        let id = text(item, "id");
        let label = name(item, en);
        add_link(&mut links, label, format!("#items/{id}"));
        if machine == RECYCLER && id == WATER {
            lines.push(if en {
                "Place World Tree Holy Water in the Recycler’s optional boost slot. Relics go in the main input. With water it processes the same work at 1.5× speed (+50%), taking about two thirds of the time under the same working conditions. The boost does not increase reward quantities or rare-drop odds per relic. Drinking the water has a separate player effect: trees and ore in the World Tree area temporarily stay visible when approached."
            } else {
                "把世界树圣水放入转换器的加速槽，古代文明遗物放入主投入槽。圣水可选；加水后处理速度为原来的 1.5 倍（+50%），相同作业条件下耗时约为原来的三分之二。每件遗物的产物数量和稀有奖励概率不因加速而增加。玩家直接喝圣水时，世界树区域的树木和矿石会暂时在靠近时保持可见。"
            }.to_owned());
            if has(
                q,
                &[
                    "多久",
                    "持续",
                    "几分钟",
                    "叠加",
                    "duration",
                    "how long",
                    "lasts",
                    "last for",
                    "stack",
                    "per hour",
                    "每小时",
                ],
            ) {
                lines.push(if en { "Community measurements report about five minutes of active processing per water (about 12 per hour of continuous work). Extra water stocks the next boost without stacking the multiplier." } else { "社区资料记录每份圣水可维持约 5 分钟的实际加工，连续运转一小时约消耗 12 份；多放是补充库存，加速倍率不会叠加。" }.to_owned());
            }
            add_link(
                &mut links,
                if en {
                    "Recycler calculator"
                } else {
                    "遗物转换计算器"
                },
                "#work/relic".into(),
            );
            continue;
        }
        let start = lines.len();
        if let Some(cost) = array(&f["materials"]).iter().find(|r| r[0] == item["id"]) {
            let qty = &cost[1];
            lines.push(if en {
                format!("Building one {station} requires {label} × {qty}.")
            } else {
                format!("建造一台{station}需要{label} × {qty}。")
            });
        }
        if machine == RECYCLER {
            if let Some(t) = tier.filter(|t| t["id"] == item["id"]) {
                lines.push(if en {format!("{label} goes in the main relic input and takes {} work units. Its grade determines the reward table.",t["work"])} else {format!("{label}是主槽投入的遗物，每件需要 {} 作业量，按该档遗物的奖励表结算。",t["work"])});
                add_link(&mut links, label, format!("#work/relic/{id}"));
            }
            if array(&relic["outputs"]).contains(&item["id"]) {
                if let Some(t) = tier {
                    let odds = jade_core::relic_product_odds(t, &relic["pools"]);
                    let chance = array(&odds)
                        .iter()
                        .find(|r| r["id"] == item["id"])
                        .and_then(|r| r["probability"].as_f64())
                        .unwrap_or(0.)
                        * 100.;
                    lines.push(if en {format!("Chance of obtaining {label} at least once from one relic of the named grade: {chance:.3}%.")} else {format!("每件上述档次遗物至少开出一次{label}的概率为 {chance:.3}%。")});
                } else {
                    lines.push(if en {format!("{label} appears in the Recycler reward tables. The chance depends on the relic grade.")} else {format!("{label}列在转换器的产物表内；实际获得概率取决于投入的遗物档次。")});
                }
                add_link(&mut links, label, format!("#work/relic/item/{id}"));
            }
        }
        for prod in array(&d.get("resource-guides-data")["production"]) {
            if text(prod, "facility") == machine && text(prod, "item") == id {
                lines.push(if en {
                    format!("The {station} produces {label} at the base.")
                } else {
                    format!("{station}在据点生产{label}。")
                });
            }
        }
        let mut recipes = Vec::new();
        for output in array(&catalog["items"]) {
            for recipe in array(&output["recipes"]) {
                if !(array(&recipe["stations"])
                    .iter()
                    .any(|s| s.as_str() == Some(text(f, "name")))
                    && (output["id"] == item["id"]
                        || array(&recipe["materials"])
                            .iter()
                            .any(|m| m[0] == item["id"])))
                {
                    continue;
                }
                // If both a source and an output were named, show that recipe first.
                let relevance = targets
                    .iter()
                    .filter(|i| {
                        i["id"] == output["id"]
                            || array(&recipe["materials"]).iter().any(|m| m[0] == i["id"])
                    })
                    .count();
                recipes.push((std::cmp::Reverse(relevance), output, recipe));
            }
        }
        recipes.sort_by_key(|r| r.0);
        let mut shown = 0;
        for (_, output, recipe) in recipes {
            let row = recipe_line(d, output, recipe, en);
            let line = if en {
                format!("{station} recipe: {row}.")
            } else {
                format!("{station}的加工配方：{row}。")
            };
            if !lines.contains(&line) {
                lines.push(line);
            }
            shown += 1;
            if shown == 2 {
                break;
            }
        }
        if lines.len() == start && !lines.iter().any(|s| s.contains(label)) {
            if !has(q, &["put ", "place ", "insert", "feed ", "add ", "boost", "affect", "convert", "turn into", "produce", "ingredient", "recipe", "get from", "go in", "made with", "放入", "放进", "放到", "添加", "加速", "提高", "会产", "产出", "转成", "加工", "配方", "原料", "投入"]) {
                continue;
            }
            lines.push(if en {
                format!("The handbook has no verified interaction for {label} with the {station}.")
            } else {
                format!("图鉴暂未收录{label}与{station}之间可核实的用途。")
            });
        } else if has(
            q,
            &[
                "加速", "效率", "更快", "boost", "speed", "faster", "fuel", "燃料",
            ],
        ) {
            lines.push(if en {format!("The recorded uses above do not establish a speed or fuel effect for {label} in the {station}.")} else {format!("这些记录未说明{label}能让{station}加速或作为该设施燃料。")});
        }
    }
    if lines.is_empty() { return None; }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    fn handbook() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for key in [
            "facilities-data",
            "item-catalog-data",
            "resource-guides-data",
            "relic-data",
            "item-effects-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{key}.json"))).unwrap();
            d.tables.insert(
                key.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        d
    }
    #[test]
    fn distinguishes_relic_boost_from_rewards_and_personal_effect() {
        let d = handbook();
        for q in [
            "Does adding World Tree Holy Water to the Ancient Relic Recycler improve schematic odds?",
            "把世界樹聖水放進遺物轉換器有什麼差別？",
            "古代文明遗物转换器加圣水作用？",
        ] {
            let reply = Conversation::default().ask_world(q, &d, &serde_json::json!({}));
            assert!(
                reply.text.contains("1.5") && reply.text.contains("50%"),
                "{}",
                reply.text
            );
            assert!(reply.links.iter().any(|(_, x)| x == "#work/relic"));
            assert!(
                reply
                    .links
                    .iter()
                    .any(|(_, x)| x == "#items/WorldTreeHolyWater")
            );
        }
        let timed = answer(
            &d,
            &field_text("How long does holy water last for in the relic recycler?"),
        )
        .unwrap();
        assert!(timed.text.contains("active processing"));
        let furnace = answer(&d, &field_text("世界树圣水能给古代文明炉加速吗？")).unwrap();
        assert!(
            furnace.text.contains("帕鲁树晶锭")
                && furnace.text.contains("未说明")
                && !furnace.text.contains("1.5"),
            "{}",
            furnace.text
        );
    }
    #[test]
    fn follows_exact_facility_and_material_roles() {
        let d = handbook();
        let crush = answer(
            &d,
            &field_text("Can I put Wood in the Crusher to make Fiber?"),
        )
        .unwrap();
        assert!(
            crush.text.contains("Wood × 50") && crush.text.contains("Wood × 1 → Fiber × 2"),
            "{}",
            crush.text
        );
        let cold = answer(&d, &field_text("在冷凍粉碎機用木材能做纖維嗎？")).unwrap();
        assert!(
            cold.text.contains("冷冻粉碎机")
                && cold
                    .links
                    .iter()
                    .any(|(_, x)| x == "#work/facilities/Refrigerated_Crusher"),
            "{}",
            cold.text
        );
        let stone = answer(&d, &field_text("Is Stone an output of the Stone Pit?")).unwrap();
        assert!(
            stone.text.contains("Stone × 50") && stone.text.contains("produces Stone"),
            "{}",
            stone.text
        );
        let relic = answer(
            &d,
            &field_text("Can a Decayed Ancient Relic produce Mythical Wood in the Relic Recycler?"),
        )
        .unwrap();
        assert!(
            relic.text.contains("6000") && relic.text.contains("100.000%"),
            "{}",
            relic.text
        );
        let unknown = answer(&d, &field_text("Will holy water boost my Crusher?")).unwrap();
        assert!(
            unknown.text.contains("no verified interaction") && !unknown.text.contains("1.5"),
            "{}",
            unknown.text
        );
        for q in [
            "Where do I get World Tree Holy Water?",
            "What's the best Pal for producing fiber at the Crusher?",
            "Does the Crusher work faster than the Refrigerated Crusher on Wood?",
            "Is the Stone Pit working?",
            "I collected more stones; which Pal should mine them?",
            "How should I lay out a new base?\nWe use Ore and a Ranch and want a nice view.",
            "Any advice on my base layout with ore near the ranch?",
        ] {
            assert!(answer(&d, &field_text(q)).is_none(), "{q}");
        }
    }

    #[test]
    fn shared_crop_nouns_and_base_construction_use_each_building_record() {
        let data = handbook();
        for (q, required) in [
            ("How do I set up tomato and lettuce plots?", &["Tomato Plantation", "level 21", "Lettuce Plantation", "level 25", "Gathering", "Transporting"][..]),
            ("西红柿和莴苣田怎么种？", &["番茄农园", "生菜农园", "番茄种子 × 3", "生菜种子 × 3", "浇水", "搬运"][..]),
            ("怎样分步开始种植西红柿田和莴苣田？", &["番茄农园", "生菜农园", "浇水"][..]),
            ("How do carrot and onion fields work?", &["Carrot Plantation", "Onion Plantation", "Gathering"][..]),
            ("How do I establish a base?", &["Palbox", "level 2", "Wood × 8", "base worker slots"][..]),
            ("原始熔炉怎么用？", &["原始的炉子", "科技等级 10", "生火 Lv.1"][..]),
            ("电气炉和伐木场分别几级解锁，用途和需要的工种呢？", &["电气炉在科技等级 44", "熔炼", "生火 Lv.1", "供电", "伐木场在科技等级 7", "伐木 Lv.1", "木材"][..]),
            ("What level unlocks the Ancient Farm and what jobs does it need?", &["Ancient Farm", "level 78", "indoors", "Planting → Watering → Gathering"][..]),
            ("On version 0.1, what level unlocks the Egg Incubator?", &["does not verify", "v1.0.5 only", "Egg Incubator"][..]),
            ("大型工具箱要什么工种，是什么用途？", &["大型工具箱在科技等级 20", "手工作业工作速度", "图鉴未列出需要指派帕鲁操作的工种", "+20%"][..]),
        ] {
            let mut dialog = Conversation::default();
            let reply = dialog.ask_world(q, &data, &serde_json::json!({}));
            assert!(required.iter().all(|word| reply.text.contains(word)) && dialog.direct_result(), "{q}: {}", reply.text);
            assert!(reply.links.iter().all(|(_, r)| r.starts_with("#work/facilities/")));
        }
        assert!(overview::answer(&data, "where to buy seeds for a tomato plantation?").is_none());
    }

    #[test]
    fn carries_only_the_last_explicit_modifier_and_clears_on_new_subject() {
        let d = handbook();
        let mut session = FacilitySession::default();
        let first = field_text("世界樹聖水放進遺物轉換器有什麼用？");
        assert!(session.respond(&d, &first).is_some());
        let odds = session
            .respond(&d, &field_text("那会改变稀有奖励的概率吗？"))
            .unwrap();
        assert!(odds.text.contains("稀有奖励概率不因加速而增加"));
        assert!(
            session
                .respond(&d, "does it stack?")
                .unwrap()
                .text
                .contains("without stacking")
        );
        assert!(
            session
                .respond(&d, &field_text("那捕获概率也会变吗？"))
                .is_none()
        );
        assert!(
            session
                .respond(&d, &field_text("那会改变奖励概率吗？"))
                .is_none()
        );
        assert!(session.respond(&d, &first).is_some());
        assert!(
            session
                .respond(&d, &field_text("那粉碎机速度呢？"))
                .is_none()
        );
        assert!(session.respond(&d, &first).is_some());
        assert!(
            session
                .respond(&d, &field_text("那木材的掉率呢？"))
                .is_none()
        );
        let mut conversation = Conversation::default();
        let world = serde_json::json!({"id":"facility-test"});
        conversation.ask_world(&first, &d, &world);
        let answer = conversation.ask_world("Does that change the rare loot?", &d, &world);
        assert!(
            conversation.direct_result() && answer.text.contains("per relic"),
            "{}",
            answer.text
        );
    }
}
