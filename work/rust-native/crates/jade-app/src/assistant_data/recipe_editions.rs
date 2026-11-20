use super::{
    ReferenceAnswer, ReferenceSession, Topic, array, counted_craft, has_any, scaled_recipe, text,
};
use crate::{assistant_engine::field_text, data::Data};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;
mod ingredients;

const GRADES: [&str; 5] = ["普通", "少见", "稀有", "史诗", "传说"];
const GRADE_NAMES: [&[&str]; 5] = [
    &["普通", "白色", "白装", "common", "white"],
    &["少见", "罕见", "精良", "绿色", "绿装", "uncommon", "green"],
    &["稀有", "蓝色", "蓝装", "rare", "blue"],
    &["史诗", "紫色", "紫装", "epic", "purple"],
    &[
        "传说",
        "传奇",
        "金色",
        "橙色",
        "金装",
        "legendary",
        "gold",
        "orange",
    ],
];

// Imported by exact UnlockItemID -> Product_Id from the pinned game recipe table.
// Neither the blueprint rarity nor suffix arithmetic identifies every output.
fn outputs() -> &'static BTreeMap<&'static str, &'static str> {
    static MAP: OnceLock<BTreeMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str(include_str!("blueprint_outputs.json"))
            .expect("bundled blueprint recipe outputs")
    })
}

fn mentions(q: &str, name: &str) -> Vec<(usize, usize)> {
    if name.is_empty() || name == "en text" {
        return vec![];
    }
    q.match_indices(name)
        .filter_map(|(i, _)| {
            let end = i + name.len();
            let before = &q[..i];
            let after = &q[end..];
            if name.is_ascii()
                && (before.ends_with(|c: char| c.is_ascii_alphanumeric())
                    || after.starts_with(|c: char| c.is_ascii_alphanumeric()))
            {
                return None;
            }
            if name.chars().count() == 1
                && !(before.ends_with(['「', '『', '“', '"', ' ', ':'])
                    || [
                        "做", "制作", "打造", "合成", "的", "品级", "品质", "普通", "传说", "史诗",
                        "稀有", "少见",
                    ]
                    .iter()
                    .any(|w| before.ends_with(w))
                    || before.is_empty()
                        && ["的配方", "需要", "要用", "怎么做"]
                            .iter()
                            .any(|w| after.starts_with(w)))
            {
                return None;
            }
            Some((i, end))
        })
        .collect()
}

fn blueprint_spelling(s: &str) -> String {
    let mut name = s
        .replace("图纸", "设计图")
        .replace("蓝图", "设计图")
        .replace("blueprint", "schematic");
    if let Some((before, after)) = name.split_once("号设计图") {
        let stem = before.trim_end_matches(|c: char| c.is_ascii_digit());
        if stem.len() < before.len() {
            name = format!("{stem}设计图{}{after}", &before[stem.len()..]);
        }
    }
    name
}

pub(crate) struct RecipeNames(BTreeMap<String, Vec<usize>>);

impl RecipeNames {
    fn build(items: &[Value]) -> Self {
        let mut names = BTreeMap::<String, Vec<usize>>::new();
        for (i, row) in items.iter().enumerate() {
            if row["id"]=="Egg" {
                for alias in ["鸡蛋","禽蛋","鸟蛋"] {names.entry(alias.into()).or_default().push(i);}
            }
            for key in ["name", "en"] {
                let name = blueprint_spelling(&field_text(text(row, key)));
                if name.is_empty() || name == "en text" {
                    continue;
                }
                if key == "en"
                    && name.ends_with(|c: char| c.is_ascii_alphabetic())
                    && !name.ends_with('s')
                {
                    names.entry(format!("{name}s")).or_default().push(i);
                }
                if text(row, "category") == "设计图" {
                    let stem = name
                        .trim_end_matches(|c: char| c.is_ascii_digit())
                        .trim_end();
                    if stem != name && (stem.ends_with("设计图") || stem.ends_with("schematic"))
                    {
                        names.entry(stem.into()).or_default().push(i);
                    }
                }
                names.entry(name).or_default().push(i);
            }
        }
        Self(names)
    }
}

fn pick<'a>(items: &'a [Value], names: &RecipeNames, q: &str) -> Option<(Vec<&'a Value>, String)> {
    let mut hits = Vec::new();
    for (name, candidates) in &names.0 {
        for span in mentions(q, name) {
            for &i in candidates {
                hits.push((span, items.get(i)?));
            }
        }
    }
    let &(span, _) = hits.iter().max_by_key(|((a, b), _)| b - a)?;
    let labels: Vec<_> = hits
        .iter()
        .filter(|(s, _)| s == &span)
        .map(|(_, r)| text(r, "name"))
        .collect();
    if hits
        .iter()
        .any(|((a, b), r)| (*b <= span.0 || *a >= span.1) && !labels.contains(&text(r, "name")))
    {
        return None;
    }
    let mut matches: Vec<&Value> = Vec::new();
    for (s, row) in hits {
        if s == span && !matches.iter().any(|r| std::ptr::eq(*r, row)) {
            matches.push(row);
        }
    }
    let mut outside = q.to_owned();
    outside.replace_range(span.0..span.1, " ");
    Some((matches, outside))
}

fn title(row: &Value) -> String {
    let grade = row["rarity"]
        .as_u64()
        .and_then(|n| GRADES.get(n as usize))
        .unwrap_or(&"未标品级");
    let plus = text(row, "en")
        .rsplit_once(" +")
        .and_then(|(_, n)| n.parse::<u64>().ok());
    match plus {
        Some(n) => format!("{} +{n} · {grade}", text(row, "name")),
        None => format!("{} · {grade}", text(row, "name")),
    }
}

fn schematic_number(q: &str) -> Option<u64> {
    ["设计图", "schematic"].iter().find_map(|tag| {
        let (_, rest) = q.split_once(tag)?;
        let number = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '#');
        let digits = number.trim_start_matches(|c: char| c.is_ascii_digit());
        number[..number.len() - digits.len()].parse().ok()
    })
}

fn output_count(q: &str) -> Option<u64> {
    if let Some(count) = counted_craft(q) {
        return Some(count.target);
    }
    let mut words = q.split_ascii_whitespace();
    while let Some(word) = words.next() {
        if matches!(word, "craft" | "make" | "build" | "produce" | "crafting")
            && let Some(amount) = words.clone().next().and_then(|w| w.parse::<u64>().ok())
        {
            return Some(amount);
        }
    }
    None
}

fn crafting_with_blueprint(q: &str) -> bool {
    if has_any(
        q,
        &[
            "图本身",
            "图纸本身",
            "schematic itself",
            "schematic paper",
            "paper itself",
        ],
    ) {
        return false;
    }
    if has_any(
        q,
        &[
            "做成品",
            "做出成品",
            "打造",
            "解锁的",
            "做出来",
            "做出来的",
            "lets me craft",
            "unlocks",
            "using it",
            "from it",
        ],
    ) {
        return true;
    }
    if let Some((before, _)) = q.split_once("设计图") {
        let clause = before
            .rsplit(['，', ',', '。', '.', '？', '?'])
            .next()
            .unwrap_or(before);
        return has_any(
            clause,
            &[
                "照",
                "按",
                "持有",
                "我有",
                "已经有",
                "手里有",
                "有了",
                "拿着",
                "拿到",
                "用",
            ],
        );
    }
    q.split_once("schematic").is_some_and(|(before, _)| {
        let clause = before.rsplit(['.', ',', '?']).next().unwrap_or(before);
        has_any(
            clause,
            &[
                "using ",
                "with ",
                "from ",
                "already have ",
                "i have ",
                "i found ",
                "i own ",
                "after finding ",
                "having found ",
            ],
        )
    })
}

fn grade_followup(q: &str) -> bool {
    let mut rest = q.to_owned();
    let mut matched = false;
    for word in GRADE_NAMES.iter().flat_map(|names| names.iter().copied()) {
        if !mentions(q, word).is_empty() {
            rest = rest.replace(word, " ");
            matched = true;
        }
    }
    matched && {
        for word in [
            "那",
            "如果",
            "换成",
            "改成",
            "换",
            "版本",
            "品级",
            "品质",
            "款",
            "版",
            "的",
            "呢",
            "和",
            "and",
            "what about",
            "the",
            "version",
            "instead",
        ] {
            rest = rest.replace(word, " ");
        }
        rest.trim_matches(|c: char| c.is_whitespace() || "，。！？、?,.".contains(c))
            .is_empty()
    }
}

fn operation(q: &str) -> bool {
    has_any(
        q,
        &[
            "哪个台",
            "哪个工作台",
            "哪个工位",
            "什么工位",
            "哪个设施",
            "什么设施",
            "哪个炉",
            "哪种炉",
            "怎么操作",
            "如何操作",
            "具体步骤",
            "怎么开工",
            "怎么开做",
            "什么工种",
            "派什么帕鲁",
            "用哪个台",
            "which station",
            "what station",
            "which workstation",
            "which workbench",
            "what bench",
            "which bench",
            "which furnace",
            "what work suitability",
            "which work skill",
            "how do i operate",
            "how do i run",
            "how do i use the station",
            "what are the steps",
        ],
    )
}

fn source_request(q: &str) -> bool {
    has_any(
        q,
        &[
            "怎么得",
            "怎么获取",
            "怎么获得",
            "如何得到",
            "怎么拿",
            "想弄",
            "怎么弄",
            "谁会掉",
            "谁掉",
            "能买到",
            "哪里找",
            "哪里获得",
            "从哪",
            "获取途径",
            "来源",
            "哪里掉",
            "哪里刷",
            "怎么刷",
            "材料去哪",
            "在哪里弄",
            "how to get",
            "how do i get",
            "how can i get",
            "where to get",
            "where can i get",
            "where do i get",
            "where can i find",
            "where do i find",
            "where can i buy",
            "where does",
            "what drops",
            "how to obtain",
            "how do i obtain",
            "how can i obtain",
            "sources of",
        ],
    )
}

fn material_sources(q: &str) -> bool {
    has_any(
        q,
        &[
            "材料从哪",
            "材料在哪",
            "材料去哪",
            "材料怎么",
            "材料如何",
            "原料从哪",
            "原料在哪",
            "原料去哪",
            "原料怎么",
            "原料如何",
            "这些材料",
            "这些原料",
            "哪里找材料",
            "哪里找原料",
            "get the materials",
            "find the materials",
            "get those materials",
            "find those materials",
            "get the ingredients",
            "get those ingredients",
            "find the ingredients",
            "find the raw materials",
        ],
    )
}

fn station_steps(d: &Data, row: &Value) -> Option<String> {
    let recipe = array(&row["recipes"]).first()?;
    let stations = array(&recipe["stations"]);
    let facility = array(&d.get("facilities-data")["facilities"])
        .iter()
        .filter(|f| stations.contains(&f["name"]))
        .min_by_key(|f| f["level"].as_u64().unwrap_or(u64::MAX))?;
    let work = array(&facility["required"])
        .iter()
        .filter_map(|v| Some(format!("{} Lv.{} 及以上", v[0].as_str()?, v[1].as_u64()?)))
        .collect::<Vec<_>>();
    let mut result = format!(
        "操作时先在据点放置{}，与它交互，选择「{}」和制作数量，确认开工。",
        text(facility, "name"),
        text(row, "name")
    );
    if !work.is_empty() {
        result.push_str(&format!("派有{}的帕鲁到这里作业。", work.join("、")));
    }
    if facility["energy"].as_str().is_some_and(|e| {
        e.eq_ignore_ascii_case("electricity") || e.eq_ignore_ascii_case("electric")
    }) {
        result.push_str("这座设施还需要供电。");
    }
    result.push_str("完成后到设施领取成品。");
    Some(result)
}

pub(crate) fn answer(
    previous: &ReferenceSession,
    session: &mut ReferenceSession,
    d: &Data,
    question: &str,
) -> Option<ReferenceAnswer> {
    answer_recipe(previous,session,d,question,false)
}

pub(crate) fn scoped_ingredients(
    previous:&ReferenceSession, session:&mut ReferenceSession, d:&Data, question:&str,
) -> Option<ReferenceAnswer> {
    if !ingredients::question(question) {return None;}
    answer_recipe(previous,session,d,question,true)
}

fn answer_recipe(previous:&ReferenceSession, session:&mut ReferenceSession, d:&Data, question:&str, need_product_and_inputs:bool) -> Option<ReferenceAnswer> {
    let mut q = question;
    let contextual;
    if let Some(Topic::Item {
        id,
        drops_only: false,
    }) = &previous.topic
        && (grade_followup(q) || has_any(q, &["照图", "照这张图", "用这张图", "那成品呢"]))
        && let Some(prior) = array(&d.get("item-catalog-data")["items"])
            .iter()
            .find(|r| text(r, "id") == id)
    {
        let name = text(prior, "name");
        if grade_followup(q) {
            contextual = format!("{name}的配方用料 {q}");
            q = &contextual;
        } else if text(prior, "category") == "设计图" {
            contextual = format!("照着「{name}」打造的成品材料清单 {q}");
            q = &contextual;
        }
    }
    let asks_station = operation(q)
        && has_any(
            q,
            &[
                "制作",
                "造",
                "合成",
                "加工",
                "熔炼",
                "做",
                "配方",
                "工作台",
                "工位",
                "台",
                "炉",
                "craft",
                "make",
                "station",
                "furnace",
                "bench",
                "work skill",
            ],
        );
    let wants_sources = source_request(q);
    let wants_amounts = ingredients::question(q);
    let mixed = wants_sources
        && (asks_station || wants_amounts
            || has_any(
                q,
                &[
                    "配方",
                    "材料",
                    "原料",
                    "怎么做",
                    "怎么制作",
                    "如何制作",
                    "如何合成",
                    "怎么合成",
                    "recipe",
                    "craft",
                    "ingredients",
                    "materials",
                    "how to make",
                    "how do i make",
                ],
            ));
    if !(asks_station || wants_amounts
        || has_any(
            q,
            &[
                "配方",
                "用料",
                "哪些材料",
                "什么材料",
                "多少材料",
                "材料清单",
                "制作材料",
                "所需材料",
                "需要的材料",
                "所需原料",
                "原料需要多少",
                "要啥材料",
                "些材料",
                "要啥原料",
                "还要啥",
                "用哪些原料",
                "需要材料",
                "用什么做",
                "怎么做",
                "怎么制作",
                "如何制作",
                "如何合成",
                "怎么合成",
                "哪个工作台",
                "在哪制作",
                "在哪里制作",
                "在哪里做",
                "where can i craft",
                "where do i craft",
                "which station",
                "what station",
                "recipe",
                "ingredients",
                "what materials",
                "which materials",
                "materials for",
                "materials to",
                "how to craft",
                "how do i craft",
                "how to make",
                "材料は",
                "材料が",
            ],
        ))
        || has_any(
            q,
            &[
                "库存",
                "还差",
                "还缺",
                "够不够",
                "最多",
                "替代",
                "不想制作",
                "除了制作",
                "总价",
                "还剩",
                "how many can",
                "left over",
                "drop rate",
                "without ",
                "instead",
                "substitute",
                "carry weight",
                "最快",
            ],
        )
        || q.match_indices("不用").any(|(at,_)| {
            !["不用给我","不用查","不用照","不用按","不用看"].iter().any(|phrase|q[at..].starts_with(phrase))
        })
        || !mixed
            && has_any(
                q,
                &[
                    "掉落",
                    "几率",
                    "怎么刷",
                    "怎么获得",
                    "哪里获得",
                    "where to get",
                    "farm for",
                    "drop from",
                ],
            )
    {
        return None;
    }
    if !crate::assistant_discovery::handbook_version_matches(q) {
        return None;
    }
    let norm = blueprint_spelling(q);
    if norm.match_indices("en text").any(|(i, tag)| {
        !norm[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
            && !norm[i + tag.len()..].starts_with(|c: char| c.is_ascii_alphanumeric())
    }) {
        return Some(ReferenceAnswer::new(
            "图鉴中有多条同名的 en text 占位条目。请给出图纸的英文全名、条目编号或在物品图鉴中打开具体条目。",
        ));
    }
    let rows = array(&d.get("item-catalog-data")["items"]);
    if rows.is_empty() {
        return None;
    }
    let names = d.recipe_names.get_or_init(|| RecipeNames::build(rows));
    let scoped = wants_amounts.then(|| ingredients::select(rows, names, &norm)).flatten();
    if need_product_and_inputs && scoped.is_none() {return None;}
    let (hits, rest) = if let Some(scope) = &scoped {
        (scope.products.clone(), scope.modifiers.clone())
    } else {
        pick(rows, names, &norm)?
    };
    let is_bp = hits.iter().all(|r| text(r, "category") == "设计图");
    let grades: Vec<_> = GRADE_NAMES
        .iter()
        .enumerate()
        .filter(|(_, names)| names.iter().any(|n| !mentions(&rest, n).is_empty()))
        .map(|(g, _)| g as u64)
        .collect();
    if !is_bp && scoped.is_none()
        && has_any(
            &norm,
            &[
                "我有",
                "手里有",
                "现有",
                "现成",
                "已有",
                "有了",
                "already have",
            ],
        )
    {
        return None;
    }
    if !is_bp && (norm.contains("设计图") || norm.contains("schematic")) {
        return None;
    }
    let want_all = grades.len() > 1
        || has_any(
            &rest,
            &[
                "各品级",
                "各个品级",
                "每种品级",
                "各品质",
                "每个品质",
                "all rarities",
                "each rarity",
                "all tiers",
                "different rarities",
            ],
        );
    let interpret_output = is_bp && crafting_with_blueprint(&norm);
    // Preserve distinct grade names such as numbered "+" accessories; an equal rarity can have several IDs.
    let mut candidates = if is_bp {
        hits.clone()
    } else {
        let label = text(hits[0], "name");
        let family: Vec<_> = rows
            .iter()
            .filter(|r| text(r, "name") == label && text(r, "category") != "设计图")
            .collect();
        if family.len() < 2 && grades.is_empty() && !asks_station && !mixed && scoped.is_none() {
            return None;
        }
        // Some translated catalogue labels explicitly name a numbered or "+" edition.
        if hits.len() == 1 && hits[0]["rarity"].as_u64().is_some_and(|n| n > 0) {
            hits.clone()
        } else {
            family
        }
    };
    candidates.sort_by_key(|r| (r["rarity"].as_u64(), text(r, "id")));
    if !is_bp && !mixed && candidates.iter().all(|r| array(&r["recipes"]).is_empty()) {
        return None;
    }
    let available = candidates.clone();
    if !grades.is_empty() {
        candidates.retain(|r| r["rarity"].as_u64().is_some_and(|g| grades.contains(&g)));
    }
    if !is_bp && let Some((_, tail)) = norm.split_once('+') {
        let n = tail
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if !n.is_empty() {
            candidates.retain(|r| text(r, "en").ends_with(&format!(" +{n}")));
        }
    }
    let number = schematic_number(&norm);
    if is_bp && let Some(requested) = number {
        candidates.retain(|r| {
            let name = text(r, "name");
            let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
            name[stem.len()..].parse::<u64>() == Ok(requested)
        });
    }
    if candidates.is_empty() {
        let mut result = ReferenceAnswer::new("当前图鉴没有收录这个品级的配方。可查：");
        for r in available {
            result.link(title(r), format!("#items/{}", text(r, "id")));
        }
        return Some(result);
    }
    if is_bp && number.is_none() && grades.is_empty() && candidates.len() > 1 && !want_all {
        let common: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|r| r["rarity"] == 0)
            .collect();
        if common.len() == 1 {
            candidates = common;
        } else if interpret_output {
            let supported: Vec<_> = candidates
                .iter()
                .copied()
                .filter(|r| outputs().contains_key(text(r, "id")))
                .collect();
            if supported.len() == 1
                && candidates
                    .iter()
                    .all(|r| outputs().contains_key(text(r, "id")) || r["other"] == true)
            {
                candidates = supported;
            }
        }
    }
    let other_grades = !is_bp && grades.is_empty() && !want_all && candidates.len() > 1;
    if other_grades {
        let common: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|r| r["rarity"] == 0 && !array(&r["recipes"]).is_empty())
            .collect();
        let craftable: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|r| !array(&r["recipes"]).is_empty())
            .collect();
        if common.len() == 1 {
            candidates = common;
        } else if craftable.len() == 1 {
            candidates = craftable;
        }
    }
    if candidates.len() > 1 && !want_all {
        let mut result = ReferenceAnswer::new("同名条目有多个版本，请指定图纸编号或具体品级：");
        for r in candidates {
            let label = title(r);
            result.text.push_str(&format!("\n{label}"));
            result.link(label, format!("#items/{}", text(r, "id")));
        }
        return Some(result);
    }
    let mut answer = ReferenceAnswer::new("");
    let unquoted = norm.replace(['「', '」', '『', '』', '“', '”', '"'], "");
    let quantity = scoped.as_ref().map_or_else(|| output_count(&unquoted), |s| s.quantity);
    for selected in &candidates {
        let row = if interpret_output {
            let key = text(selected, "id");
            let Some(product) = outputs()
                .get(key)
                .and_then(|id| rows.iter().find(|r| text(r, "id") == *id))
            else {
                answer.text.push_str(&format!(
                    "\n这张{}没有可核实的成品配方关联。",
                    text(selected, "name")
                ));
                answer.link(title(selected), format!("#items/{key}"));
                continue;
            };
            product
        } else {
            *selected
        };
        let recipe = if let Some(scope) = &scoped {
            ingredients::quantities(d, row, scope, q)
        } else {
            scaled_recipe(d, row, quantity)
        };
        if candidates.len() == 1 {
            session.topic = Some(Topic::Item {
                id: text(row, "id").to_owned(),
                drops_only: false,
            });
        }
        if !answer.text.is_empty() {
            answer.text.push('\n');
        }
        let heading = if interpret_output {
            format!("按「{}」制作的成品", text(selected, "name"))
        } else if is_bp {
            "制作图纸本身".into()
        } else {
            "制作".into()
        };
        answer
            .text
            .push_str(&format!("{heading}（{}）：\n{}", title(row), recipe.text));
        for (l, p) in recipe.links {
            answer.link(l, p);
        }
        if interpret_output {
            answer.link(title(selected), format!("#items/{}", text(selected, "id")));
        }
        if asks_station && let Some(steps) = station_steps(d, row) {
            answer.text.push_str(&format!("\n{steps}"));
        }
        if mixed {
            let mut subjects = vec![*selected];
            let named_sources = scoped.as_ref().map(|s|s.inputs.iter().copied().filter(|item|
                q.split(['，',',','。','；',';','？','?']).any(|clause|
                    source_request(clause) && ["name","en"].iter().any(|k|{
                        let name=field_text(text(item,k)); !name.is_empty() && clause.contains(&name)
                    }))).collect::<Vec<_>>()).unwrap_or_default();
            if !named_sources.is_empty(){
                subjects=named_sources;
            }else if material_sources(q) {
                subjects = array(&row["recipes"])
                    .first()
                    .map(|recipe| {
                        array(&recipe["materials"])
                            .iter()
                            .filter_map(|v| rows.iter().find(|item| item["id"] == v[0]))
                            .collect()
                    })
                    .unwrap_or_default();
            }
            for subject in subjects {
                let source = crate::assistant_engine::sources_for_recipe(d, subject, q);
                let detail = if row["id"] == subject["id"] && !recipe.text.is_empty() {
                    source.text.replace(&recipe.text, "").trim_end().to_owned()
                } else {
                    source.text
                };
                if !detail.is_empty() && !detail.ends_with('：') {
                    answer.text.push_str(&format!("\n{detail}"));
                }
                for (label, href) in source.links {
                    if !answer.links.iter().any(|(_, existing)| existing == &href) {
                        answer.link(label, href);
                    }
                }
            }
        }
    }
    for g in grades {
        if !candidates.iter().any(|r| r["rarity"].as_u64() == Some(g)) {
            answer.text.push_str(&format!(
                "\n图鉴没有收录该物品的{}版本。",
                GRADES[g as usize]
            ));
        }
    }
    if other_grades && available.len() > 1 {
        let others: Vec<_> = available
            .into_iter()
            .filter(|r| !candidates.iter().any(|s| std::ptr::eq(*s, *r)))
            .collect();
        if !others.is_empty() {
            answer.text.push_str(&format!(
                "\n其他版本可在图鉴查看：{}。",
                others
                    .iter()
                    .map(|r| title(r))
                    .collect::<Vec<_>>()
                    .join("、")
            ));
            for r in others {
                answer.link(title(r), format!("#items/{}", text(r, "id")));
            }
        }
    }
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn family_recipe_resolves_the_result_or_paper_and_keeps_the_players_grade() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        for table in ["item-catalog-data", "facilities-data"] {
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
        let mut chat = Conversation::default();
        let world = serde_json::json!({});
        for (q, first, fact) in [
            (
                "做轻便远古盔甲需要哪些材料？",
                "AncientArmorWeight",
                "帕鲁树晶锭 × 30",
            ),
            (
                "陨石发射器的金色版本需要什么材料？",
                "Launcher_Meteor_5",
                "陨石碎片 × 200",
            ),
            (
                "按这张复合弓设计图4打造一把成品，需要哪些材料？",
                "CompoundBow_5",
                "精炼金属锭 × 100",
            ),
            (
                "制作复合弓设计图4本身需要什么材料？",
                "Blueprint_CompoundBow_5",
                "设计图3 × 5",
            ),
            (
                "What materials do I need to craft 2 rare Compound Bows?",
                "CompoundBow_3",
                "精炼金属锭 × 150",
            ),
            (
                "Which materials do I need to make the launcher with Meteor Launcher Schematic 4?",
                "Launcher_Meteor_5",
                "陨石碎片 × 200",
            ),
            (
                "按复合弓4号图纸做一把，需要哪些材料？",
                "CompoundBow_5",
                "精炼金属锭 × 100",
            ),
            (
                "What materials do I need for Compound Bow Schematic 4 itself?",
                "Blueprint_CompoundBow_5",
                "设计图3 × 5",
            ),
            ("制作攻击吊坠+2需要什么材料？", "Accessory_AT_3", "没有收录"),
            ("普通剑的配方用料有哪些？", "Sword", "金属锭"),
        ] {
            let a = chat.ask_world(q, &d, &world);
            assert!(chat.direct_result(), "{q}: {}", a.text);
            assert_eq!(
                a.links.first().map(|p| p.1.as_str()),
                Some(format!("#items/{first}").as_str()),
                "{q}: {}",
                a.text
            );
            assert!(a.text.contains(fact), "{q}: {}", a.text);
        }
        let missing = chat.ask_world("陨石发射器设计图3本身需要哪些材料？", &d, &world);
        assert!(
            missing.text.contains("没有收录这个品级"),
            "{}",
            missing.text
        );
        assert!(
            answer(
                &ReferenceSession::default(),
                &mut ReferenceSession::default(),
                &d,
                "我有80份纤维，还缺几份才够做复合弓？"
            )
            .is_none()
        );
        let two = chat.ask_world("请分别列出蓝色和紫色复合弓的配方。", &d, &world);
        assert!(
            two.text.contains("复合弓 · 稀有") && two.text.contains("复合弓 · 史诗"),
            "{}",
            two.text
        );
        assert!(
            two.links.iter().any(|(_, p)| p == "#items/CompoundBow_3")
                && two.links.iter().any(|(_, p)| p == "#items/CompoundBow_4")
        );
        let placeholder = chat.ask_world("What materials do I need to craft en text?", &d, &world);
        assert!(placeholder.text.contains("占位条目") && placeholder.links.is_empty());
        chat.ask_world("制作稀有复合弓需要哪些材料？", &d, &world);
        let three = chat.ask_world("那做三把呢？", &d, &world);
        assert!(three.text.contains("精炼金属锭 × 225"), "{}", three.text);
        let epic = chat.ask_world("那紫色的呢？", &d, &world);
        assert!(
            epic.links.iter().any(|(_, p)| p == "#items/CompoundBow_4"),
            "{}",
            epic.text
        );
        let iron = chat.ask_world(
            "怎么获得金属锭，在据点用哪个炉做、该分配什么工种？",
            &d,
            &world,
        );
        for fact in ["金属矿石 × 2", "原始的炉子", "生火 Lv.1", "开工", "掉落"] {
            assert!(iron.text.contains(fact), "{fact}: {}", iron.text);
        }
        let inputs = chat.ask_world(
            "Which workbench can craft Polymer, and where can I find the raw materials?",
            &d,
            &world,
        );
        for (name, target) in [("优质帕鲁油", "PalOil"), ("硫磺", "Sulfur")] {
            assert!(
                inputs.text.contains(name)
                    && inputs
                        .links
                        .iter()
                        .any(|(_, p)| p == &format!("#items/{target}")),
                "{}",
                inputs.text
            );
        }
        let gem=chat.ask_world("How can I find ice shards to awaken my Pals?", &d, &world);
        assert!(gem.text.contains("冰之辉石 × 50") && gem.text.contains("世界树圣水 × 10") && gem.text.contains("古代文明工作台"));
        let books=chat.ask_world("How can I get a work suitability handbook for mining?", &d, &world);
        assert!(books.text.contains("应用挖矿学") && books.text.contains("空投补给") && books.text.contains("海上打捞"),"{}",books.text);
    }
}
