//! Source-specific queries read the actual item, ranch and combat loot records.
use super::{AssistantReply, field_text};
use crate::data::{Data, array, text};
use serde_json::Value;

mod drop_comparison;
mod economy;
mod shared_drops;
mod unattended;
mod ranch_substitutes;
mod ranch_loot;
mod item_families;
pub(super) use item_families::answer as family_sources;
pub(super) use unattended::answer as unattended_sources;
pub(super) use ranch_substitutes::answer as ranch_substitute_answer;
pub(super) use ranch_loot::answer as ranch_loot_answer;
pub(crate) use economy::EconomySession;

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

#[derive(Clone, Copy, PartialEq)]
enum Source {
    Any,
    Combat,
    Ranch,
    Chest,
    Fish,
    Shop,
    Gather,
    Expedition,
    Pool,
}

impl Source {
    fn detect(q: &str, d: &Data) -> Option<Vec<Self>> {
        if has(
            q,
            &[
                "掉落倍率",
                "掉率倍率",
                "提高掉",
                "提升掉",
                "增加掉",
                "让它多掉",
                "搭配",
                "叠加",
                "drop multiplier",
                "increase the drop",
                "increase drop",
                "boost drop",
                "bugged",
                "掉线",
                "掉帧",
                "掉血",
                "掉地底",
                "drop fps",
                "frame drop",
            ],
        ) {
            return None;
        }
        let ranch = q.match_indices("牧场").any(|(at, _)|
            !q[..at].ends_with("配种") && !q[..at].ends_with("繁殖"))
            || ["放牧", "ranch", "in the farm", "at the farm", "on the farm", "on a farm"]
                .iter().any(|word| occurs(q, word));
        let drop = has(
            q,
            &[
                "掉",
                "掉落",
                "掉宝",
                "掉什么",
                "掉啥",
                "谁掉",
                "会掉",
                "能掉",
                "哪里掉",
                "谁爆",
                "爆率",
                "drop",
                "loot from",
                "loot table",
                "loot does",
            ],
        );
        let find = english_acquisition_object(q).is_some()
            || has(
                q,
                &[
                    "怎么获得",
                    "如何获得",
                    "如何获取",
                    "怎么获取",
                    "在哪",
                    "哪里",
                    "哪里有",
                    "去哪",
                    "从哪",
                    "怎么刷",
                    "如何刷",
                    "怎么弄",
                    "怎么拿",
                    "获得途径",
                    "获取途径",
                    "获取的方法",
                    "获得的方法",
                    "获取方法",
                    "途径",
                    "拿到",
                    "能获得",
                    "能获取",
                    "怎么入手",
                    "来源",
                    "way to farm",
                    "ways to farm",
                    "way of getting",
                    "怎么得",
                    "哪里出",
                    "会出",
                    "取得",
                    "how to get",
                    "how do i get",
                    "how can i get",
                    "how do you get",
                    "where",
                    "source of",
                    "sources of",
                    "acquire",
                    "obtain",
                    "how to farm",
                    "how do i farm",
                ],
            );
        let making = has(
            q,
            &[
                "怎么制作",
                "怎么做",
                "怎么合成",
                "如何制作",
                "如何合成",
                "材料够",
                "多少材料",
                "需要哪些材料",
                "how to craft",
                "how do i craft",
                "crafting recipe",
                "materials needed",
            ],
        );
        if making && !drop {
            return None;
        }
        if has(q, &["卖给", "sell to", "selling to"]) {
            return None;
        }
        let pool_subject = has(
            q,
            &[
                "奖池",
                "奖励池",
                "loot pool",
                "reward pool",
                "野外拾取",
                "地面拾取",
                "ground pickup",
                "field pickup",
                "敌方营地",
                "敌人据点",
                "enemy camp",
                "dungeon chest",
            ],
        ) || array(&d.get("adventure-data")["chapters"])
            .iter()
            .any(|chapter| occurs(q, text(chapter, "name")));
        if pool_subject
            && has(
                q,
                &[
                    "只看",
                    "只查",
                    "只限",
                    "仅",
                    "几档",
                    "每档",
                    "数量档",
                    "概率",
                    "几率",
                    "栏位",
                    "每次",
                    "一次",
                    "几份",
                    "几个",
                    "多少个",
                    "掉率",
                    "odds",
                    "how many",
                    "per roll",
                    "only",
                    "chance",
                    "quantity",
                    "quantities",
                    "slots",
                    "tiers",
                ],
            )
        {
            return Some(vec![Self::Pool]);
        }
        let shop = crate::assistant_data::seller_query(q)
            || has(
                q,
                &[
                    "只看商店",
                    "只看商人",
                    "购买",
                    "能买到",
                    "有卖吗",
                    "每个多少钱",
                    "单价",
                    "purchase",
                    "unit price",
                    "price per",
                    "商店获得",
                    "商店买",
                    "商人买",
                    "商店出售",
                    "商人出售",
                    "商店和",
                    "商店或",
                    "shops only",
                    "which shops",
                    "any shop",
                    "sold by",
                ],
            )
            || (find || ranch || drop || has(q, &["只限", "只列", "只说", "only"]))
                && has(q, &["商店", "商人", "merchant", "shops", "vendor"]);
        let catch = has(q, &["宝箱", "chest"]);
        let fish = has(q, &["钓鱼", "垂钓", "fishing"]);
        let expedition = has(q, &["远征", "expedition"]);
        let gather = has(
            q,
            &[
                "野外采集",
                "野外拾取",
                "地面采",
                "gather in the wild",
                "ground pickup",
                "ground pick-up",
            ],
        );
        // A partner skill may mention a named item without asking for combat loot.
        let fighting = [
            "只打怪", "打怪掉", "战斗掉", "击败", "打怪",
            "kill", "killed", "killing", "defeat", "defeated", "defeating", "combat drop",
        ].iter().any(|word| occurs(q, word));
        let open = has(
            q,
            &[
                "哪些途径",
                "有哪些途径",
                "获取途径有哪些",
                "所有来源",
                "所有方式",
                "其他途径",
                "还有别",
                "别的途径",
                "其他方式",
                "all sources",
                "all the ways",
                "other ways",
                "only way",
                "only source",
                "只能靠",
                "只能从",
                "只能通过",
            ],
        ) || (find && asks_efficiency(q) && q.contains('?') && q.contains(" or "));
        if open && !has(q, &["只看", "只列", "只限"]) {
            return Some(vec![Self::Any]);
        }
        let mut selected = vec![];
        if ranch
            && (drop
                || find
                || shop
                || has(
                    q,
                    &[
                        "产", "produce", "give", "get", "yield", "only", "只看", "只列",
                    ],
                ))
        {
            selected.push(Self::Ranch);
        }
        if shop {
            selected.push(Self::Shop);
        }
        if catch && (drop || find) {
            selected.push(Self::Chest);
        }
        if fish && (drop || find) {
            selected.push(Self::Fish);
        }
        if expedition
            && (drop || find || has(q, &["只看", "远征获得", "远征能出", "expeditions only"]))
        {
            selected.push(Self::Expedition);
        }
        if gather && (drop || find || has(q, &["只看"])) {
            selected.push(Self::Gather);
        }
        if fighting || drop && selected.is_empty() {
            selected.push(Self::Combat);
        }
        selected.retain(|s| !s.excluded(q));
        if selected.is_empty() && find {
            selected.push(Self::Any);
        }
        (!selected.is_empty()).then_some(selected)
    }

    fn matches(self, method: &str) -> bool {
        match self {
            Self::Ranch => method == "牧场生产",
            Self::Combat => method.contains("掉落") || method == "解体产物",
            Self::Chest => method.contains("宝箱"),
            Self::Fish => has(method, &["钓鱼", "垂钓"]),
            Self::Shop => has(method, &["商店", "商人"]),
            Self::Gather => has(method, &["采集", "采摘", "拾取"]),
            Self::Expedition => method.contains("远征"),
            Self::Pool => false,
            Self::Any => true,
        }
    }
    fn excluded(self, q: &str) -> bool {
        let names: &[&str] = match self {
            Self::Any | Self::Pool => return false,
            Self::Ranch => &["牧场", "放牧", "ranch"],
            Self::Shop => &["商店", "商人", "购买", "shop", "merchant", "buy"],
            Self::Expedition => &["远征", "expedition"],
            Self::Gather => &["采集", "拾取", "gather"],
            Self::Chest => &["宝箱", "chest"],
            Self::Fish => &["钓鱼", "垂钓", "fish"],
            Self::Combat => &["打怪", "战斗", "击杀", "combat", "kill"],
        };
        [
            "不看",
            "不要",
            "不考虑",
            "除了",
            "不用",
            "不想",
            "不靠",
            "without ",
            "excluding ",
            "not counting ",
        ]
        .iter()
        .any(|m| {
            q.match_indices(m).any(|(i, _)| {
                let rest = q[i + m.len()..]
                    .trim_start()
                    .trim_start_matches("the ")
                    .trim_start_matches("a ");
                names.iter().any(|s| rest.starts_with(s))
            })
        })
    }
}

const CHANNELS: [Source; 7] = [
    Source::Shop,
    Source::Ranch,
    Source::Combat,
    Source::Fish,
    Source::Chest,
    Source::Gather,
    Source::Expedition,
];

fn asks_efficiency(q: &str) -> bool {
    has(
        q,
        &[
            "最快",
            "效率最高",
            "最省时",
            "更快",
            "highest yield",
            "most efficient",
            "fastest",
            "best way",
            "quickest",
        ],
    )
}

// Conversational questions often put the object after an infinitive rather
// than starting with "where". Stop at a constraint so a longer material or
// facility name in that constraint cannot replace the requested object.
fn english_acquisition_object(q: &str) -> Option<&str> {
    let (_, tail) = [
        " to get ",
        " to farm ",
        " way of farming ",
        " trouble finding ",
        " trouble getting ",
        " difficulty obtaining ",
    ]
    .iter()
    .filter_map(|marker| q.rfind(marker).map(|i| (i, &q[i + marker.len()..])))
    .max_by_key(|(i, _)| *i)?;
    let tail = tail.strip_prefix("for ").unwrap_or(tail);
    let stop = [
        " without ",
        " with ",
        " from ",
        " using ",
        " because ",
        " rather than ",
        " instead of ",
        " so that ",
        "?",
        "\n",
        ".",
    ]
    .iter()
    .filter_map(|s| tail.find(s))
    .min()
    .unwrap_or(tail.len());
    let target = tail[..stop].trim();
    // "get my X to work" describes a problem with X, not obtaining X.
    (!target.is_empty()
        && !has(
            target,
            &[
                " to ", " working", " unstuck", " fixed", " running", " off ", " out of ",
            ],
        ))
    .then_some(target)
}

fn normalized(q: &str) -> String {
    q.replace("蓝图", "设计图")
        .replace("图纸", "设计图")
        .replace("图样", "设计图")
        .replace("blueprints", "schematic")
        .replace("blueprint", "schematic")
        .replace("schematics", "schematic")
        .replace("的设计图", "设计图")
        .replace("电气器官", "发电器官")
        .replace("electricity organ", "electric organ")
        .replace("pal fluids", "水栖帕鲁的黏液")
        .replace("pal fluid", "水栖帕鲁的黏液")
        .replace("帕鲁体液", "水栖帕鲁的黏液")
        .replace("帕鲁的体液", "水栖帕鲁的黏液")
        .replace("帕鲁粘液", "水栖帕鲁的黏液")
        .replace("旺旺币", "汪汪币")
}

fn occurs(q: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    if !term.is_ascii() {
        return q.contains(term);
    }
    q.match_indices(term).any(|(start, _)| {
        !q[..start].ends_with(|c: char| c.is_ascii_alphanumeric()) && {
            let tail = &q[start + term.len()..];
            let tail = tail.strip_prefix('s').unwrap_or(tail);
            !tail.starts_with(|c: char| c.is_ascii_alphanumeric())
        }
    })
}

fn tier_number(token: &str) -> Option<u64> {
    match token.trim_matches(|c: char| !c.is_alphanumeric()) {
        "1" | "i" | "一" => Some(1),
        "2" | "ii" | "二" => Some(2),
        "3" | "iii" | "三" => Some(3),
        "4" | "iv" | "四" => Some(4),
        _ => None,
    }
}

fn rarity(q: &str) -> Option<u64> {
    if let Some(tier) = ["schematic", "设计图"].iter().find_map(|prefix| {
        let (_, rest) = q.split_once(prefix)?;
        let s = rest.trim_start().trim_start_matches('#');
        let s = ["tier ", "level ", "rank "]
            .iter()
            .find_map(|m| s.strip_prefix(m))
            .unwrap_or(s);
        let token = s.split(|c: char| !c.is_ascii_alphanumeric()).next()?;
        tier_number(token)
    }) {
        return Some(tier);
    }
    [
        (4, &["传说", "传奇", "金色", "legendary", "golden"][..]),
        (3, &["史诗", "紫色", "epic"]),
        (1, &["优秀", "绿色", "uncommon"]),
        (2, &["稀有", "蓝色", "rare"]),
        (0, &["普通品质", "common quality"]),
    ]
    .into_iter()
    .find(|(_, labels)| labels.iter().any(|s| occurs(q, s)))
    .map(|(n, _)| n)
}

fn item_candidates<'a>(items: &'a [Value], q: &str, blueprint: bool) -> Vec<&'a Value> {
    let tier = rarity(q);
    let mut best = (0, 0);
    let mut rows = vec![];
    'item: for row in items {
        if (text(row, "category") == "设计图") != blueprint
            || tier.is_some_and(|r| row["rarity"] != r)
        {
            continue;
        }
        let mut rank = (0, 0);
        for key in ["name", "en", "id"] {
            let alias = field_text(text(row, key));
            // Single CJK characters otherwise match ordinary phrases (e.g. 图纸中的“图”).
            if alias.chars().count() < 2 {
                if key == "name"
                    && !alias.is_empty()
                    && (q.trim() == alias
                        || q.match_indices(&alias).any(|(i, _)| {
                            let before = &q[..i];
                            let after = &q[i + alias.len()..];
                            ["掉落", "掉"].iter().any(|s| before.ends_with(s))
                                && after.chars().next().is_none_or(|c| {
                                    c.is_whitespace() || "?？。!！吗么呢".contains(c)
                                })
                        })
                        || alias == "蛋"
                            && has(q, &["鸡蛋", "禽蛋", "产蛋", "生产蛋", "掉蛋"])
                            && !q.contains("帕鲁蛋"))
                {
                    rank = rank.max((2, 1));
                }
                continue;
            }
            if occurs(q, &alias) {
                rank = rank.max((2, alias.chars().count()));
            } else if key == "en" {
                // The catalog names some goods in the plural (e.g. berries).
                let singular = alias
                    .strip_suffix("ies")
                    .map(|s| format!("{s}y"))
                    .or_else(|| {
                        alias
                            .strip_suffix('s')
                            .filter(|_| !alias.ends_with("ss"))
                            .map(str::to_owned)
                    });
                if singular
                    .as_ref()
                    .is_some_and(|s| s.len() > 2 && occurs(q, s))
                {
                    rank = rank.max((2, alias.chars().count()));
                }
            }
            if blueprint {
                let stem = alias
                    .split_once("设计图")
                    .or_else(|| alias.split_once(" schematic"))
                    .map(|x| x.0.trim());
                if let Some(stem) = stem
                    && stem.chars().count() >= 2
                    && occurs(q, stem)
                {
                    // Players also say "mechanical bow 4 schematic" or
                    // "assault rifle tier IV blueprint". A quantity in front
                    // of the item ("4 bow schematics") is not a rarity.
                    for (i, _) in q.match_indices(stem) {
                        let tail = &q[i + stem.len()..];
                        if let Some((between, _)) = tail
                            .split_once("schematic")
                            .or_else(|| tail.split_once("设计图"))
                        {
                            let between = between.trim();
                            let token = ["tier ", "level ", "rank ", "等级"]
                                .iter()
                                .find_map(|p| between.strip_prefix(p))
                                .unwrap_or(between);
                            if let Some(n) = tier_number(token)
                                && row["rarity"] != n
                            {
                                continue 'item;
                            }
                        }
                    }
                    rank = rank.max((1, stem.chars().count()));
                }
            }
        }
        if rank.0 == 0 || rank < best {
            continue;
        }
        if rank > best {
            rows.clear();
            best = rank;
        }
        rows.push(row);
    }
    rows
}

/// Extract the object of a purchase before resolving names. Currency, materials
/// offered in payment, and the amount the user is asking about have other roles.
fn purchased_items<'a>(items: &'a [Value], q: &str, blueprint: bool) -> Vec<&'a Value> {
    let clause = |s: &str| {
        [
            ",",
            "，",
            "。",
            "?",
            "？",
            "；",
            ";",
            "每个",
            "每份",
            "单价",
            "要多少",
            "多少钱",
            "找谁",
            "花多少",
            "支付",
            "需要",
            " with ",
            " using ",
            " for ",
            " cost",
            " per ",
        ]
        .iter()
        .filter_map(|part| s.find(part))
        .min()
        .unwrap_or(s.len())
    };
    for verb in ["购买", "买到", "买", "purchase ", "buy ", "sells ", "sell "] {
        for (i, _) in q.match_indices(verb) {
            let after = &q[i + verb.len()..];
            let direct = after[..clause(after)].trim();
            if !direct.is_empty() {
                let rows = item_candidates(items, direct, blueprint);
                if !rows.is_empty() {
                    return rows;
                }
            }
        }
    }
    let lead = q[..clause(q)].trim();
    let rows = item_candidates(items, lead, blueprint);
    if !rows.is_empty() {
        return rows;
    }
    // For a price-first question ("How many coins for X?") the name after
    // the price is still the requested item.
    if let Some((_, after)) = q.split_once(" for ") {
        let rows = item_candidates(items, after, blueprint);
        if !rows.is_empty() {
            return rows;
        }
    }
    item_candidates(items, q, blueprint)
}

/// Resolve the recipe's output before the generic location or unlock handler.
/// A batch recipe can answer a crafting-efficiency question even when the book
/// gives no timings for comparing two benches.
pub(super) fn batch_recipe(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(q,&["怎么做","如何做","怎么制作","如何制作","怎么批量","怎么量产"]) ||
        !has(q,&["比较快","快一点","更快","批量","量产","一次做"]) || q.contains('\n') { return None; }
    let catalog=array(&d.get("item-catalog-data")["items"]);
    let named=item_candidates(catalog,q,false);
    let item=match named.as_slice() {
        [item]=>*item,
        []=>{
            let subject=q.split_once("怎么").or_else(||q.split_once("如何"))?.0
                .trim_start_matches("请问").trim();
            if !matches!(subject,"鱼饵"|"钓饵") { return None; }
            catalog.iter().find(|row|text(row,"id")=="FishingBait_1")?
        }
        _=>return None,
    };
    let recipe=array(&item["recipes"]).iter().find(|r|r["qty"].as_u64().is_some_and(|v|v>1))?;
    let name=text(item,"name");
    let station=array(&recipe["stations"]).first()?.as_str()?;
    let materials:Vec<_>=array(&recipe["materials"]).iter().map(|m|Some(format!("{} × {}",d.item_name(m[0].as_str()?),m[1].as_u64()?))).collect::<Option<_>>()?;
    if materials.is_empty() { return None; }
    let msg=format!("以{name}为例，在{station}按组制作，一组产出 {} 个，材料是{}。要做多组可一次排入相应数量。配方表没有各工作台的完成时间，无法据此确定最快的台。",recipe["qty"],materials.join("、"));
    Some(AssistantReply{text:msg,links:vec![(name.into(),format!("#items/{}",text(item,"id")))],suggestions:vec![]})
}

pub(super) fn craft_station(d:&Data,q:&str)->Option<AssistantReply>{
    let location=has(q,&["在哪","哪里","哪边","哪个台","哪种台","哪个工作台","哪个设施","where","which station","what station","which bench","what bench"]);
    let title=q.lines().next()?;
    let hidden_recipe=has(title,&["找不到","不显示","看不到","没有显示","不在列表","available to craft",
        "not showing","not listed","doesn't show","does not show","can't find","cannot find","missing from"])
        && has(q,&["工作台","制作列表","配方","制作台","解锁","recipe","workbench","unlocked","crafting menu","crafting list"]);
    if !(location || hidden_recipe) || !has(q,&["制作","做出","生产","合成","配方","craft","make","build this","recipe"]) {return None;}
    // For a missing recipe, the requested output must appear in the question.
    // Other items in a player's description may only be ingredients or comparisons.
    let subject=if hidden_recipe {title} else {q};
    let items=item_candidates(array(&d.get("item-catalog-data")["items"]),subject,has(subject,&["设计图","schematic","blueprint"]));
    let [item]=items.as_slice() else{return None;};
    let recipes=array(&item["recipes"]);
    if recipes.is_empty(){return None;}
    let mut stations=Vec::new();
    for recipe in recipes{
        for station in array(&recipe["stations"]).iter().filter_map(Value::as_str){
            if !stations.contains(&station){stations.push(station);}
        }
    }
    if stations.is_empty(){return None;}
    let name=text(item,"name");
    let mut answer:AssistantReply=crate::assistant_data::recipe_answer(d,item).into();
    let unlock=recipes.iter().filter_map(|r|r["tech"].as_u64()).min()
        .map(|l|format!(" 本版该物品的制作科技为 {l} 级。")).unwrap_or_default();
    answer.text=format!("{name}可以在{}制作。{unlock}\n{}",stations.join("、"),answer.text);
    Some(answer)
}

fn pal_mentioned<'a>(d: &'a Data, q: &str) -> Option<&'a Value> {
    let mut found = None;
    let mut best = 0;
    for pal in &d.species {
        for name in [text(pal, "zh"), text(pal, "en"), text(pal, "key")]
            .into_iter()
            .chain(array(&pal["aliases"]).iter().filter_map(Value::as_str))
        {
            let name = field_text(name);
            if name.chars().count() > 1 && name.len() > best && occurs(q, &name) {
                best = name.len();
                found = Some(pal);
            }
        }
    }
    found
}

fn link(reply: &mut AssistantReply, label: impl Into<String>, href: String) {
    if !reply.links.iter().any(|(_, v)| *v == href) {
        reply.links.push((label.into(), href));
    }
}

fn loot(d: &Data, r: &Value) -> Option<String> {
    let (id, lo, hi, chance) = (
        r[0].as_str()?,
        r[1].as_u64()?,
        r[2].as_u64()?,
        r[3].as_f64()?,
    );
    let qty = if lo == hi {
        lo.to_string()
    } else {
        format!("{lo}–{hi}")
    };
    Some(format!("{} × {qty}（{chance}%）", d.item_name(id)))
}

fn enemy_line(d: &Data, e: &Value, target: Option<&str>) -> Option<String> {
    let list = |table: &Value| {
        array(table)
            .iter()
            .filter(|r| target.is_none_or(|id| r[0] == id))
            .filter_map(|r| loot(d, r))
            .collect::<Vec<_>>()
            .join("、")
    };
    let base = list(&e["drops"]);
    let mut drops = vec![];
    if !base.is_empty() {
        drops.push(format!("基础档 {base}"));
    }
    for tier in array(&e["dropTiers"]) {
        let list = list(&tier["drops"]);
        if !list.is_empty()
            && let Some(n) = tier["level"].as_u64()
        {
            drops.push(format!("Lv.{n} 起 {list}"));
        }
    }
    if drops.is_empty() {
        return None;
    }
    let site = array(&e["points"])
        .iter()
        .find_map(|p| {
            Some(format!(
                "，{}坐标 ({}, {})",
                if p["map"] == "worldtree" {
                    "世界树"
                } else {
                    "帕洛斯群岛"
                },
                p["x"].as_i64()?,
                p["y"].as_i64()?
            ))
        })
        .unwrap_or_default();
    Some(format!(
        "{}（{}{}）：{}",
        text(e, "name"),
        text(e, "group"),
        site,
        drops.join("、")
    ))
}

fn pal_drops(d: &Data, pal: &Value, target: Option<&Value>, q: &str) -> AssistantReply {
    let key = text(pal, "key");
    let name = text(pal, "zh");
    let id = target.and_then(|v| v["id"].as_str());
    let boss = has(q, &["头目", "领主", "boss", "alpha"]);
    let predator = has(q, &["狂暴", "掠食", "捕食者", "predator"]);
    let mut reply = AssistantReply::new("", &[]);
    let mut lines = vec![];
    if !(boss || predator) {
        for group in array(&d.get("paldex-data")[key]["drops"]) {
            let Some(level) = group[0].as_u64() else {
                continue;
            };
            let records: Vec<_> = array(&group[1])
                .iter()
                .filter(|r| id.is_none_or(|x| r[0] == x))
                .filter_map(|r| loot(d, r))
                .collect();
            if !records.is_empty() {
                lines.push(format!(
                    "{name}{}：{}",
                    if level == 0 {
                        "基础掉落".into()
                    } else {
                        format!(" Lv.{level} 起")
                    },
                    records.join("、")
                ));
            }
        }
    }
    if boss || predator || id.is_some() && lines.is_empty() {
        for e in array(&d.get("enemy-data")["enemies"]).iter().filter(|e| {
            e["palKey"] == key
                && (!boss || e["group"] == "头目帕鲁" || text(e, "group").contains("头目"))
                && (!predator || e["group"] == "狂暴帕鲁")
        }) {
            if let Some(detail) = enemy_line(d, e, id) {
                lines.push(detail);
                link(
                    &mut reply,
                    text(e, "name"),
                    format!("#enemies/{}", text(e, "id")),
                );
            }
        }
    }
    if lines.is_empty() {
        let scope = if predator {
            "狂暴个体"
        } else if boss {
            "头目个体"
        } else {
            ""
        };
        lines.push(format!(
            "当前战斗掉落表中未查到{name}{scope}{}的记录。",
            target
                .map(|v| format!("掉落「{}」", text(v, "name")))
                .unwrap_or_default()
        ));
    }
    link(&mut reply, format!("{name}图鉴"), format!("#paldex/{key}"));
    reply.text = lines.join("\n");
    reply
}

fn ranch(d: &Data, item: &Value, pal: Option<&Value>) -> AssistantReply {
    let (id, name) = (text(item, "id"), text(item, "name"));
    let mut reply = AssistantReply::new("", &[]);
    let mut keep = vec![];
    let mut rates = vec![];
    if let Some(records) = d.get("ranch-data")["pals"].as_object() {
        for (key, v) in records {
            if pal.is_some_and(|p| p["key"] != *key) {
                continue;
            }
            if array(&v["tiers"])
                .iter()
                .flat_map(array)
                .any(|r| r["item"] == id)
            {
                let who = d.pal_name(key);
                keep.push(who.clone());
                if let Some(index) = v["base"].as_u64().and_then(|n| n.checked_sub(1))
                    && let Some(row) = array(&v["tiers"][index as usize])
                        .iter()
                        .find(|r| r["item"] == id)
                    && let (Some(lo), Some(hi), Some(chance)) = (
                        row["min"].as_u64(),
                        row["max"].as_u64(),
                        row["chance"].as_f64(),
                    )
                {
                    let amount = if lo == hi {
                        lo.to_string()
                    } else {
                        format!("{lo}–{hi}")
                    };
                    rates.push(format!("{who} {amount} 个（{chance}%）"));
                }
                link(&mut reply, who, format!("#paldex/{key}"));
            }
        }
    }
    reply.text = if keep.is_empty() {
        format!(
            "牧场产出表中未查到{}通过放牧产出{name}的记录。",
            pal.map(|p| text(p, "zh")).unwrap_or("帕鲁")
        )
    } else {
        format!("将{}分派到家畜牧场，可以产出{name}。", keep.join("、"))
    };
    if !rates.is_empty() {
        reply
            .text
            .push_str(&format!("自然适性的每次产出记录：{}。", rates.join("、")));
    }
    link(&mut reply, format!("{name}图鉴"), format!("#items/{id}"));
    if !keep.is_empty() {
        link(&mut reply, "牧场产出表", format!("#work/ranch/item/{id}"));
    }
    reply
}

fn wants_noncrafting(d: &Data, item: &Value, q: &str) -> bool {
    let markers = [
        "without ",
        "no access to ",
        "don't have ",
        "don’t have ",
        "do not have ",
        "除了",
        "不想",
        "没有",
        "不用",
        "不用靠",
        "不靠",
    ];
    let stations: Vec<_> = array(&item["recipes"])
        .iter()
        .flat_map(|recipe| array(&recipe["stations"]))
        .filter_map(Value::as_str)
        .collect();
    markers
        .iter()
        .flat_map(|m| q.match_indices(m).map(move |(i, _)| &q[i + m.len()..]))
        .any(|tail| {
            let excluded = tail
                .split([',', '，', '。', '?', '!', '\n'])
                .next()
                .unwrap_or("")
                .trim();
            if has(
                excluded,
                &["craft", "smelt", "制作", "制造", "合成", "冶炼"],
            ) {
                return true;
            }
            array(&d.get("facilities-data")["facilities"])
                .iter()
                .filter(|f| stations.contains(&text(f, "name")))
                .any(|f| {
                    let id = text(f, "id").replace('_', " ").to_ascii_lowercase();
                    let matching_station = occurs(excluded, text(f, "name"))
                        || occurs(excluded, &id)
                        || id
                            .split_whitespace()
                            .next_back()
                            .is_some_and(|w| w.len() > 3 && occurs(excluded, w));
                    matching_station
                        || array(&f["required"]).iter().any(|job| {
                            let name = job[0].as_str().unwrap_or("");
                            if !name.is_empty() && excluded.contains(name) {
                                return true;
                            }
                            let terms: &[&str] = match name {
                                "生火" => &["kindling", "fire pal"],
                                "手工作业" => &["handiwork"],
                                "制药" => &["medicine production"],
                                "浇水" => &["watering"],
                                "冷却" => &["cooling"],
                                "发电" => &["generating electricity"],
                                _ => &[],
                            };
                            terms.iter().any(|w| occurs(excluded, w))
                        })
                })
        })
}

fn best_pool(d: &Data, id: &str, source: Source) -> Option<(String, String)> {
    let mut highest = None;
    for chapter in array(&d.get("adventure-data")["chapters"])
        .iter()
        .filter(|c| source.matches(text(c, "name")))
    {
        for pool in array(&chapter["pools"]) {
            for row in array(&pool["rows"]).iter().filter(|r| r[0] == id) {
                let Some(chance) = row[3].as_f64() else {
                    continue;
                };
                if highest.is_none_or(|(best, _, _)| chance > best) {
                    highest = Some((chance, chapter, pool));
                }
            }
        }
    }
    let (chance, c, p) = highest?;
    let label = format!(
        "在已收录的探索奖池中，{}的「{}」单项概率最高，为 {chance}%。",
        text(c, "name"),
        text(p, "name")
    );
    Some((
        label,
        format!("#guide/loot/{}/{}", text(c, "id"), text(p, "id")),
    ))
}

fn matched_labels<'a>(labels: impl Iterator<Item = &'a str>, q: &str) -> Vec<&'a str> {
    let mut matches = labels
        .filter(|s| s.chars().count() > 1 && occurs(q, s))
        .collect::<Vec<_>>();
    matches.sort_by_key(|s| std::cmp::Reverse(s.len()));
    let mut distinct = Vec::new();
    for label in matches {
        if !distinct.iter().any(|long: &&str| long.contains(label)) {
            distinct.push(label);
        }
    }
    distinct
}

/// A row is an individual award entry, not a pool total. Preserve repeated
/// item IDs, both quantity bounds, and each recorded slot in the requested scope.
fn pool_details(d: &Data, item: &Value, question: &str) -> AssistantReply {
    let (id, name) = (text(item, "id"), text(item, "name"));
    let mut reply = AssistantReply::new("", &[]);
    link(&mut reply, format!("{name}图鉴"), format!("#items/{id}"));
    let q = question
        .replace(text(item, "name"), " ")
        .replace(&text(item, "en").to_lowercase(), " ")
        .replace("敌方营地", "敌人据点")
        .replace("feybreak", "天坠之地")
        .replace("sakurajima", "樱岛")
        .replace("ground pickups", "野外拾取")
        .replace("ground pickup", "野外拾取")
        .replace("field pickup", "野外拾取")
        .replace("地面拾取", "野外拾取")
        .replace("enemy camp", "敌人据点")
        .replace("dungeon chest", "地下城宝箱");
    let chapters = array(&d.get("adventure-data")["chapters"]);
    let pool_names: Vec<_> = chapters
        .iter()
        .flat_map(|c| array(&c["pools"]))
        .map(|p| text(p, "name"))
        .collect();
    let chapters_named = matched_labels(chapters.iter().map(|c| text(c, "name")), &q);
    let regions = matched_labels(
        pool_names
            .iter()
            .map(|s| s.split('·').next().unwrap_or("").trim()),
        &q,
    );
    // A sub-pool word can also occur inside a chapter title (e.g. 据点 in
    // 敌人据点). Only use separately requested sub-pool names as filters.
    let activity_question = chapters_named
        .iter()
        .fold(q.clone(), |rest, title| rest.replace(title, " "));
    let activities = matched_labels(
        pool_names
            .iter()
            .filter_map(|s| {
                s.split_once('·').map(|(_, n)| {
                    n.trim()
                        .trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace())
                })
            })
            .filter(|s| !["奖池", "第"].contains(s)),
        &activity_question,
    );
    let named_pools = matched_labels(
        pool_names
            .iter()
            .filter_map(|s| s.split_once('·').map(|(_, sub)| sub.trim()))
            .filter(|s| s.ends_with(|c: char| c.is_ascii_digit()) || s.ends_with('档')),
        &activity_question,
    );
    if regions.is_empty()
        && ["只看", "只查", "只限", "仅限"].iter().any(|prefix| {
            q.split_once(prefix).is_some_and(|(_, s)| {
                s.split(['，', ',', '。', '?'])
                    .next()
                    .and_then(|s| s.split_once('的'))
                    .is_some_and(|(region, _)| {
                        !region.is_empty()
                            && !chapters_named.contains(&region.trim())
                            && !activities.contains(&region.trim())
                    })
            })
        })
    {
        reply.text = format!("无法在奖池表中定位给定地区，暂时无法核对{name}的数量档和概率。");
        return reply;
    }
    let mut details = Vec::new();
    let mut quantities = std::collections::BTreeSet::new();
    let mut pools = 0;
    for chapter in chapters {
        let chapter_name = text(chapter, "name");
        if !chapters_named.is_empty() && !chapters_named.contains(&chapter_name) {
            continue;
        }
        for p in array(&chapter["pools"]) {
            let pool_name = text(p, "name");
            let (region, sub) = pool_name
                .split_once('·')
                .map_or((pool_name, ""), |(a, b)| (a.trim(), b.trim()));
            if !regions.is_empty() && !regions.contains(&region)
                || !activities.is_empty() && !activities.iter().any(|s| sub.starts_with(s))
                || !named_pools.is_empty() && !named_pools.contains(&sub)
            {
                continue;
            }
            let mut found = false;
            for row in array(&p["rows"]).iter().filter(|r| r[0] == id) {
                let (Some(lo), Some(hi), Some(chance)) =
                    (row[1].as_u64(), row[2].as_u64(), row[3].as_f64())
                else {
                    continue;
                };
                let slot = row[4]
                    .as_u64()
                    .map(|s| format!(" · 栏位 {s}"))
                    .unwrap_or_default();
                let amount = if lo == hi {
                    lo.to_string()
                } else {
                    format!("{lo}–{hi}")
                };
                details.push(format!("{chapter_name} · {pool_name}{slot}：一次 {amount} 个，表内单项获得率 {chance}%。"));
                quantities.insert((lo, hi));
                found = true;
            }
            if found {
                pools += 1;
                link(
                    &mut reply,
                    pool_name,
                    format!("#guide/loot/{}/{}", text(chapter, "id"), text(p, "id")),
                );
            }
        }
    }
    if details.is_empty() {
        reply.text = format!("当前限定的地区与奖池中，没有查到{name}的奖励记录。");
    } else {
        reply.text = format!(
            "{name}在所查范围有 {pools} 个奖池、{} 种数量档、{} 条奖励记录。\n{}",
            quantities.len(),
            details.len(),
            details.join("\n")
        );
    }
    reply
}

fn add_source_tables(
    d: &Data,
    id: &str,
    source: Source,
    excluded: &[Source],
    lines: &mut Vec<String>,
    reply: &mut AssistantReply,
) -> Vec<String> {
    let mut covered = vec![];
    if matches!(source, Source::Any | Source::Gather) && !excluded.contains(&Source::Gather) {
        if let Some(guide) = array(&d.get("resource-guides-data")["gathering"])
            .iter()
            .find(|v| v["id"] == id)
        {
            if let Some(note) = guide["note"].as_str() {
                lines.push(format!("野外采集：{note}"));
            }
            link(reply, "野外采集指南", format!("#guide/gathering/{id}"));
            if let (Some(href), Some(label)) =
                (guide["areaLink"][0].as_str(), guide["areaLink"][1].as_str())
            {
                link(reply, label, href.to_owned());
            }
            for pool in array(&guide["pools"]).iter().filter_map(Value::as_str) {
                for point in array(&d.get("resource-guides-data")["pools"][pool]) {
                    if let Some(href) = point["href"].as_str() {
                        link(reply, text(point, "label"), href.to_owned());
                    }
                }
            }
        }
    }
    // A chapter's display name is a category. Its pool names describe the
    // actual region and activity (including field pickups as distinct from chests).
    let book = d.get("item-catalog-data");
    if let Some(groups) = book["guideLinks"][id].as_object() {
        for entry in groups.values().flat_map(array) {
            let Some(href) = entry[0].as_str() else {
                continue;
            };
            let Some(chapter_id) = href
                .strip_prefix("#guide/loot/")
                .and_then(|s| s.split('/').next())
            else {
                continue;
            };
            let Some(chapter) = array(&d.get("adventure-data")["chapters"])
                .iter()
                .find(|v| v["id"] == chapter_id)
            else {
                continue;
            };
            let title = text(chapter, "name");
            // Expeditions have a forward table with duration and rewards.
            if title.contains("远征") && d.ready("expedition-data") {
                continue;
            }
            let mut samples = vec![];
            let mut total = 0;
            for pool in array(&chapter["pools"]) {
                let name = text(pool, "name");
                if !(source.matches(title) || source.matches(name))
                    || excluded.iter().any(|s| s.matches(title) || s.matches(name))
                {
                    continue;
                }
                let entries: Vec<_> = array(&pool["rows"])
                    .iter()
                    .filter(|r| r[0] == id)
                    .filter_map(|r| {
                        let (lo, hi, pct) = (r[1].as_u64()?, r[2].as_u64()?, r[3].as_f64()?);
                        let amount = if lo == hi {
                            lo.to_string()
                        } else {
                            format!("{lo}–{hi}")
                        };
                        let slot = r[4]
                            .as_u64()
                            .map(|n| format!("栏位{n} "))
                            .unwrap_or_default();
                        Some(format!("{slot}{amount}个，单项 {pct}%"))
                    })
                    .collect();
                if !entries.is_empty() {
                    total += 1;
                    if samples.len() < 12 {
                        samples.push(format!("{name}（{}）", entries.join("；")));
                    }
                }
            }
            if !samples.is_empty() {
                let rest = if total > samples.len() {
                    format!("；另有 {} 个奖池，见「{title}」条目", total - samples.len())
                } else {
                    String::new()
                };
                lines.push(format!("{title}：{}{rest}", samples.join("、")));
                covered.push(
                    match title {
                        "垂钓池" => "钓鱼池",
                        "远征所" => "帕鲁远征",
                        _ => title,
                    }
                    .to_owned(),
                );
                link(reply, title, href.to_owned());
            }
        }
    }
    if matches!(source, Source::Any | Source::Expedition) && !excluded.contains(&Source::Expedition)
    {
        let mut stages = vec![];
        for stage in array(&d.get("expedition-data")["stages"]) {
            if let Some(entry) = array(&stage["rewards"]).iter().find(|r| r[0] == id)
                && let Some(result) = loot(d, entry)
            {
                let title = text(stage, "name");
                let minutes = stage["minutes"]
                    .as_u64()
                    .map(|m| format!("，基础{m}分钟"))
                    .unwrap_or_default();
                stages.push(format!("{title}{minutes}，{result}"));
                link(
                    reply,
                    title,
                    format!("#work/expeditions/{}", text(stage, "id")),
                );
            }
        }
        if !stages.is_empty() {
            lines.push(format!("远征：{}", stages.join("；")));
            covered.push("帕鲁远征".into());
        }
    }
    covered
}

fn item_sources(d: &Data, item: &Value, source: Source, q: &str) -> AssistantReply {
    let (id, name) = (text(item, "id"), text(item, "name"));
    let book = d.get("item-catalog-data");
    let mut reply = AssistantReply::new("", &[]);
    link(&mut reply, format!("{name}图鉴"), format!("#items/{id}"));
    let mut lines = vec![];
    let exclude_craft = wants_noncrafting(d, item, q);
    let bosses_only = source == Source::Combat && has(q, &["头目", "首领", "boss", "alpha"]);
    let excluded: Vec<_> = CHANNELS.into_iter().filter(|s| s.excluded(q)).collect();
    let covered = add_source_tables(d, id, source, &excluded, &mut lines, &mut reply);
    if !matches!(source, Source::Combat | Source::Ranch)
        && (has(q, &["highest chance", "最高概率", "最高几率"])
            || text(item, "category") == "设计图" && asks_efficiency(q))
        && excluded.is_empty()
        && let Some((summary, href)) = best_pool(d, id, source)
    {
        lines.push(summary);
        link(&mut reply, "查看探索奖池", href);
    }
    // Enumerate examples per acquisition method; never turn sample rows into an exhaustive list.
    let mut groups: Vec<(&str, Vec<&str>)> = vec![];
    for row in array(&item["methods"]) {
        let (Some(kind), Some(detail)) = (row[0].as_str(), row[1].as_str()) else {
            continue;
        };
        if !source.matches(kind)
            || covered.iter().any(|name| name == kind)
            || excluded.iter().any(|s| s.matches(kind))
            || bosses_only && !has(kind, &["头目", "塔主", "狂暴", "召唤战"])
            || exclude_craft && has(kind, &["制作", "烹饪", "据点生产"])
        {
            continue;
        }
        if let Some((_, examples)) = groups.iter_mut().find(|(k, _)| *k == kind) {
            if !examples.contains(&detail) {
                examples.push(detail);
            }
        } else {
            groups.push((kind, vec![detail]));
        }
    }
    for (kind, mut examples) in groups {
        // The index can list an additional species even when its individual
        // loot table has no matching item. Preserve the name with that scope.
        if kind == "帕鲁掉落" && d.ready("paldex-data") {
            let (verified, summary): (Vec<_>, Vec<_>) = examples.into_iter().partition(|&name| {
                d.species.iter().filter(|p| text(p, "zh") == name).any(|p| {
                    array(&d.get("paldex-data")[text(p, "key")]["drops"])
                        .iter()
                        .any(|group| array(&group[1]).iter().any(|row| row[0] == id))
                })
            });
            if !summary.is_empty() {
                lines.push(format!(
                    "物品来源汇总另列{}；这些个体的物种掉落表未收录此物的数量和概率。",
                    summary.join("、")
                ));
            }
            examples = verified;
            if examples.is_empty() {
                continue;
            }
        }
        let limit = if examples.len() <= 50 {
            examples.len()
        } else {
            15
        };
        let details = examples
            .iter()
            .copied()
            .take(limit)
            .collect::<Vec<_>>()
            .join("、");
        lines.push(if examples.len() > limit {
            format!(
                "{kind}：{details}；另有 {} 条，完整名单见「{name}图鉴」。",
                examples.len() - limit
            )
        } else {
            format!("{kind}：{details}")
        });
    }
    if matches!(source, Source::Combat | Source::Any) && !excluded.contains(&Source::Combat) {
        if !bosses_only && d.ready("paldex-data") {
            let mut examples = 0;
            for pal in &d.species {
                let key = text(pal, "key");
                let Some((level, row)) = array(&d.get("paldex-data")[key]["drops"]).iter().find_map(|g| {
                    Some((g[0].as_u64()?, array(&g[1]).iter().find(|r| r[0] == id)?))
                }) else {
                    continue;
                };
                if let Some(amount) = loot(d, row) {
                    let tier = if level == 0 { "基础战斗掉落".into() } else { format!(" Lv.{level} 起的战斗掉落") };
                    lines.push(format!("{}的{tier}：{amount}", text(pal, "zh")));
                    link(&mut reply, text(pal, "zh"), format!("#paldex/{key}"));
                    examples += 1;
                    if examples == 2 {
                        break;
                    }
                }
            }
        }
        let enemies = array(&d.get("enemy-data")["enemies"]);
        let mut count = 0;
        for entry in array(&book["enemyDrops"][id]) {
            let Some(enemy) = enemies.iter().find(|e| e["id"] == entry[0]) else {
                continue;
            };
            if bosses_only && !has(text(enemy, "group"), &["头目", "首领", "塔主", "狂暴"])
            {
                continue;
            }
            if let Some(detail) = enemy_line(d, enemy, Some(id)) {
                lines.push(detail);
                link(
                    &mut reply,
                    text(enemy, "name"),
                    format!("#enemies/{}", text(enemy, "id")),
                );
                count += 1;
                if count == 2 {
                    break;
                }
            }
        }
    }
    if source == Source::Any {
        if !excluded.contains(&Source::Shop) && !array(&item["sellers"]).is_empty() {
            let sellers = crate::assistant_data::seller_reference(d, item);
            lines.push(sellers.text);
            for (label, href) in sellers.links {
                link(&mut reply, label, href);
            }
        }
        if !array(&item["recipes"]).is_empty() && !exclude_craft {
            lines.push(crate::assistant_data::recipe_answer(d, item).text);
        }
        if d.ready("ranch-data") && !excluded.contains(&Source::Ranch) {
            let result = ranch(d, item, None);
            if result.links.iter().any(|(_, v)| v.starts_with("#paldex/")) {
                lines.retain(|s| !s.starts_with("牧场生产："));
                lines.push(result.text);
                for (name, href) in result.links {
                    link(&mut reply, name, href);
                }
            }
        }
    }
    if let Some(groups) = book["guideLinks"][id].as_object() {
        for row in groups.values().flat_map(array) {
            if let (Some(href), Some(label)) = (row[0].as_str(), row[1].as_str())
                && source.matches(label)
                && !excluded.iter().any(|s| s.matches(label))
            {
                link(&mut reply, label, href.to_owned());
            }
        }
    }
    if (source == Source::Combat || source == Source::Any)
        && has(
            q,
            &[
                "坐标",
                "在哪里找",
                "哪出现",
                "where do i find",
                "coordinates",
                "spawn location",
            ],
        )
        && !array(&book["enemyDrops"][id]).is_empty()
        && !lines.iter().any(|s| s.contains("坐标"))
    {
        lines.push("上述敌方个体记录中未查到可核实的刷新坐标。".into());
    }
    if !lines.is_empty() && asks_efficiency(q) {
        lines.push(
            "图鉴没有覆盖全部路线的往返、刷新和作业耗时，无法确定哪条路线单位时间产量最高。".into(),
        );
    }
    reply.text = if lines.is_empty() {
        format!("当前图鉴未收录{name}符合这个条件的获取途径。")
    } else {
        let heading = if exclude_craft {
            "无需制作的获取途径"
        } else {
            "图鉴记录"
        };
        format!("{name}的{heading}：\n{}", lines.join("\n"))
    };
    reply
}

pub(crate) fn sources_for_recipe(d: &Data, item: &Value, question: &str) -> AssistantReply {
    let sources=Source::detect(question,d).unwrap_or_default();
    match sources.as_slice() {
        [Source::Shop] => crate::assistant_data::seller_reference(d,item).into(),
        [Source::Ranch] if d.ready("ranch-data") => ranch(d,item,None),
        [Source::Pool] if d.ready("adventure-data") => pool_details(d,item,question),
        [source] => item_sources(d,item,*source,question),
        _ => item_sources(d,item,Source::Any,question),
    }
}

pub(super) fn resolved_schematic_reply(d: &Data, question: &str, previous: Option<&str>) -> Option<String> {
    let previous = normalized(previous?);
    if !has(&previous, &["设计图", "schematic"]) || question.chars().count() > 90 { return None; }
    let query = normalized(question);
    if !has(&query, &["设计图", "schematic"]) { return None; }
    let compact = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let short = compact(query.trim_end_matches(['。', '？', '?', '.', '!']));
    let exact = array(&d.get("item-catalog-data")["items"]).iter().any(|row| {
        text(row, "category") == "设计图" && ["name", "en", "id"].iter()
            .any(|key| compact(&super::field_text(text(row, key))) == short)
    });
    exact.then(|| format!("{query} 获取途径"))
}

pub(super) fn answer(d: &Data, question: &str) -> Option<AssistantReply> {
    let q = normalized(question);
    if let Some(comparison) = drop_comparison::answer(d, &q) {
        return Some(comparison);
    }
    if let Some(examples) = schematic_drop_examples(d,&q) {return Some(examples);}
    let sources = Source::detect(&q, d)?;
    if !d.ready("item-catalog-data") {
        return None;
    }
    let blueprint = has(&q, &["设计图", "schematic"]);
    let target = if sources.contains(&Source::Any) {
        english_acquisition_object(&q).unwrap_or(&q)
    } else {
        &q
    };
    let catalog = array(&d.get("item-catalog-data")["items"]);
    if !blueprint && matches!(sources.as_slice(), [Source::Combat] | [Source::Any])
        && let Some(result) = shared_drops::answer(d, catalog, target)
    {
        return Some(result);
    }
    let items = if sources.as_slice() == [Source::Shop] {
        purchased_items(catalog, target, blueprint)
    } else {
        item_candidates(catalog, target, blueprint)
    };
    if blueprint && items.is_empty()
        && has(&q, &["防具", "护甲", "盔甲", "武器", "armor", "armour", "weapon"])
        && !has(&q, &["所有", "全部", "有哪些", "all the", "all armor", "all weapon"])
        && (target == q.as_str() || item_candidates(catalog, &q, true).is_empty())
    {
        return Some(AssistantReply::new(
            "你想找哪件装备的设计图？不同装备和图纸等级的掉落地点不同。如果能看到游戏里的名称，请把名称和图纸等级一起发来。",
            &[],
        ));
    }
    if items.len() > 1 {
        if !blueprint {
            return None;
        }
        let choices = items
            .iter()
            .map(|v| text(v, "name"))
            .collect::<Vec<_>>()
            .join("、");
        return Some(AssistantReply::new(
            &format!("设计图的获取途径会随等级而变。你要找的是{choices}中的哪一张？"),
            &[],
        ));
    }
    let item = items.first().copied();
    let pal = if sources
        .iter()
        .any(|s| matches!(s, Source::Combat | Source::Ranch))
    {
        // Species names inside the requested object (e.g. Anubis's talisman)
        // don't name its loot source. Preserve separate mentions of the species.
        let mut other_words = q.clone();
        if let Some(item) = item {
            for key in ["name", "en"] {
                let name = text(item, key).to_lowercase();
                let stem = name
                    .split_once("设计图")
                    .or_else(|| name.split_once(" schematic"))
                    .map_or(name.as_str(), |s| s.0)
                    .trim();
                if !stem.is_empty() {
                    other_words = other_words.replace(stem, " ");
                }
            }
        }
        pal_mentioned(d, &other_words)
    } else {
        None
    };
    let mut reply = AssistantReply::new("", &[]);
    for source in sources {
        let part = match source {
            Source::Pool if d.ready("adventure-data") => pool_details(d, item?, &q),
            Source::Pool => return None,
            Source::Combat if pal.is_some() && d.ready("paldex-data") && d.ready("enemy-data") => {
                pal_drops(d, pal?, item, &q)
            }
            Source::Ranch if d.ready("ranch-data") => ranch(d, item?, pal),
            Source::Ranch => return None,
            Source::Shop => {
                let item = item?;
                // The merchant detail handler can also answer about a named
                // vendor's location; keep that normal path for vendor-specific requests.
                if array(&d.get("enemy-data")["people"]).iter().any(|p| {
                    let name = text(p, "name");
                    name.chars().count() > 1 && q.contains(name)
                }) {
                    return None;
                }
                crate::assistant_data::seller_reference(d, item).into()
            }
            _ => item_sources(d, item?, source, &q),
        };
        if !reply.text.is_empty() {
            reply.text.push('\n');
        }
        reply.text.push_str(&part.text);
        for (label, href) in part.links {
            link(&mut reply, label, href);
        }
    }
    Some(reply)
}

// An existence question has no named schematic to resolve. Supply examples
// from actual enemy drop rows for the requested rarity.
fn schematic_drop_examples(d:&Data,q:&str)->Option<AssistantReply> {
    if !has(q,&["传说","传奇","legendary"]) || !has(q,&["图","schematic","recipe"]) ||
        !has(q,&["击败","击杀","打怪","头目","掉落","kills","killing","defeat","from bosses","boss drops"]) ||
        has(q,&["only","唯一","只能","which weapon","现在还","still drop","best","最快"]) {return None;}
    let catalog=array(&d.get("item-catalog-data")["items"]);
    if !item_candidates(catalog,q,true).is_empty() {return None;}
    let subtype=["弓","枪","甲","头盔","bow","rifle","shotgun","armor","helmet"].into_iter().find(|t|occurs(q,t));
    let ids:std::collections::HashMap<_,_>=catalog.iter().filter(|item| item["category"]=="设计图" && item["rarity"]==4 &&
        subtype.is_none_or(|t|["en","name"].iter().any(|key|text(item,key).to_ascii_lowercase().contains(t))))
        .map(|item|(text(item,"id"),item)).collect();
    let mut parts=Vec::new();
    let mut result=AssistantReply::new("",&[]);
    for e in array(&d.get("enemy-data")["enemies"]) {
        let Some((item,rate))=array(&e["drops"]).iter().find_map(|r|Some((ids.get(r[0].as_str()?)?,r[3].as_f64()?))) else{continue;};
        let name=text(item,"name");
        parts.push(format!("{}：{}（基础概率 {rate}%）",text(e,"name"),name));
        link(&mut result,text(e,"name"),format!("#enemies/{}",text(e,"id")));
        link(&mut result,name,format!("#items/{}",text(item,"id")));
        if parts.len()==3{break;}
    }
    if parts.is_empty(){return None;}
    result.text=format!("可以。不同头目有各自的设计图掉落表，击败后按概率结算。图鉴中的例子有：\n{}",parts.join("\n"));
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn acquisition_uses_the_requested_source_and_exact_record() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "paldex-data",
            "ranch-data",
            "item-catalog-data",
            "enemy-data",
            "resource-guides-data",
            "facilities-data",
            "adventure-data",
            "expedition-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            d.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let ask = |q: &str| answer(&d, &field_text(q));
        assert!(ask("Which partner skill changes a picked-up egg?").is_none());
        assert!(ask("哪只伙伴能让配种牧场产蛋更快？").is_none());
        assert!(ask("哪只帕鲁在家畜牧场产蛋？").is_some());
        for action in ["kill", "kills", "killed", "killing", "defeat", "defeated", "defeating"] {
            let query = format!("leather from {action} enemies");
            assert!(matches!(Source::detect(&query, &d).as_deref(), Some([Source::Combat])));
        }
        let passive = |q: &str| unattended_sources(&d, &field_text(q));
        let pair = passive("请问铬铁矿和珊瑚矿有被动收入吗？").unwrap();
        assert!(pair.text.contains("天坠魔窟") && pair.text.contains("天阳乡浮岛"));
        assert!(!pair.text.contains("采矿场"), "unconfirmed buildings shown as buildable");
        let coal = passive("Can I get coal passively?").unwrap();
        assert!(coal.text.contains("石炭采矿场") && coal.text.contains("采矿 Lv.1"));
        let wool = passive("羊毛可以挂机获取吗？").unwrap();
        assert!(wool.text.contains("棉悠悠") && wool.text.contains("牧场"));
        assert!(passive("煤矿每小时挂机能出多少石炭？").is_none());
        assert!(passive("What passive trait helps a worker haul coal?").is_none());
        assert!(passive("Can I farm nonexistentium passively?").is_none());
        let ranch_alternative=|q:&str| ranch_substitute_answer(&d,&field_text(q));
        let unique=ranch_alternative("Does any other Pal do what Vixy does?").unwrap();
        assert!(unique.text.contains("帕鲁球") && unique.text.contains("没有第二种") && unique.text.contains("骨头"));
        let wool=ranch_alternative("什么帕鲁能替代棉悠悠在牧场放牧？").unwrap();
        assert!(wool.text.contains("米露菲") && wool.text.contains("美露帕") && wool.text.contains("羊毛"));
        assert!(ranch_alternative("哪种坐骑能在战斗中替代玉藻狐？").is_none());
        for question in ["遗物和圣水一起怎么刷？", "Where can I farm ancient relics and holy water?"] {
            let reply = ask(question).unwrap();
            assert!(reply.text.contains("Lv.70") && reply.text.contains("圣水") && reply.text.contains("遗物"), "{}", reply.text);
        }
        for q in [
            "哪隻帕魯能在牧場產出發電器官？",
            "Which Pal drops electric organs in the farm?",
        ] {
            let a = ask(q).unwrap();
            assert!(
                a.text.contains("伏特喵") && !a.text.contains("暴电熊"),
                "{}",
                a.text
            );
        }
        let yielded = ask("What does Sparkit drop?").unwrap();
        assert!(
            yielded.text.contains("发电器官 × 1–2（100%）"),
            "{}",
            yielded.text
        );
        for q in [
            "Which boss drops Legendary Assault Rifle blueprints?",
            "傳說突擊步槍圖紙是誰掉落？",
        ] {
            let a = ask(q).unwrap();
            assert!(
                a.text.contains("突击步枪设计图4")
                    && a.text.contains("焰煌")
                    && a.text.contains("3%")
                    && a.text.contains("-737, -332"),
                "{}",
                a.text
            );
            assert!(
                a.links
                    .iter()
                    .any(|(_, v)| v == "#enemies/BOSS_KingBahamut")
            );
            assert!(!a.text.contains("制图桌"));
        }
        let from_chest = ask("Where can I find Assault Rifle Schematic 3 in chests?").unwrap();
        assert!(
            from_chest.text.contains("地下城宝箱")
                && !from_chest.text.contains("制图桌")
                && !from_chest.text.contains("头目掉落"),
            "{}",
            from_chest.text
        );
        let unclear = ask("突击步枪设计图哪里出？").unwrap();
        assert!(
            unclear.text.contains("哪一张")
                && unclear.text.contains("设计图1")
                && unclear.text.contains("设计图4")
        );
        let checked = ask("头目焰煌掉落突击步枪设计图4的概率多少？").unwrap();
        assert!(
            checked.text.contains("3%") && checked.text.contains("焰煌"),
            "{}",
            checked.text
        );
        let unrelated = ask("What does Lamball drop?").unwrap();
        assert!(
            unrelated.text.contains("羊毛 × 1–3（100%）"),
            "{}",
            unrelated.text
        );
        let no_drop = ask("棉悠悠会掉布吗？").unwrap();
        assert!(
            no_drop.text.contains("未查到") && !no_drop.text.contains("羊毛 ×"),
            "{}",
            no_drop.text
        );
        let eggs = ask("哪种帕鲁放牧产蛋？").unwrap();
        assert!(eggs.text.contains("皮皮鸡"), "{}", eggs.text);
        assert!(ask("How to craft an assault rifle?").is_none());
        let shop = ask("发电器官商人哪里买？").unwrap();
        assert!(shop.text.contains("300") && !shop.text.contains("战斗掉落"));
        let gathered = ask("帕鲁矿碎块怎么获得？").unwrap();
        assert!(
            gathered.text.contains("蓝色晶体")
                && gathered
                    .links
                    .iter()
                    .any(|(_, v)| v == "#guide/gathering/Pal_crystal_S")
        );
        let bow = ask("What's the best way to farm for a mechanical bow 4 schematic?").unwrap();
        assert!(
            bow.text.contains("机械弓设计图4")
                && bow.text.contains("藏宝图 · 第 5 档")
                && bow.text.contains("0.29728725%"),
            "{}",
            bow.text
        );
        let core=ask("I’m I the only one who finds it extremely frustrating to get ancient civilization cores?").unwrap();
        assert!(
            core.links.iter().any(|(_, v)| v == "#items/AncientParts2")
                && core.text.contains("古代文明核心 × 1（100%）"),
            "{}",
            core.text
        );
        let ingots = ask("Easiest way to get Ingots without a kindling Pal?").unwrap();
        assert!(
            ingots.text.contains("浪刃武士")
                && ingots.text.contains("金属锭 × 2–3（100%）")
                && !ingots.text.contains("炉子"),
            "{}",
            ingots.text
        );
        let another = ask("Any way to get assault rifle tier III blueprints?").unwrap();
        assert!(
            another
                .links
                .iter()
                .any(|(_, v)| v == "#items/Blueprint_AssaultRifle_Default4")
        );
        let counted = ask("Any way to farm 4 assault rifle blueprints?").unwrap();
        assert!(counted.text.contains("哪一张"), "{}", counted.text);
        let rare = ask("How to get Assault Rifle schematic tier IV?").unwrap();
        assert!(
            rare.links
                .iter()
                .any(|(_, v)| v == "#items/Blueprint_AssaultRifle_Default5")
        );
        let unrelated_limit = ask("Best way to get ingots without electric organs?").unwrap();
        assert!(
            unrelated_limit.text.contains("炉子")
                && unrelated_limit.links.first().unwrap().1 == "#items/CopperIngot",
            "{}",
            unrelated_limit.text
        );
        assert!(ask("It is frustrating to get my furnace working.").is_none());
        let frags =
            ask("我需要50个暗黑碎片，哪些途径能拿到？按目前资料能确定哪条最快吗？").unwrap();
        for fact in [
            "帕鲁掉落",
            "天坠之地 · 野外拾取",
            "天阳乡 · 野外拾取",
            "无法确定",
        ] {
            assert!(frags.text.contains(fact), "{fact}: {}", frags.text);
        }
        let oil=ask("What’s the best way to farm high quality pal oil?\nIs there a pal that produces it in the ranch, or dose a merchant sell it? Or do I just have too kill pals for it?").unwrap();
        for fact in [
            "趴趴鲶",
            "梆梆鲶",
            "300 金币",
            "帕鲁掉落",
            "垂钓池",
            "远征",
            "无法确定",
        ] {
            assert!(oil.text.contains(fact), "{fact}: {}", oil.text);
        }
        for (id, response) in [("PalOil", &oil), ("PalDarkParts", &frags)] {
            let item = array(&d.get("item-catalog-data")["items"])
                .iter()
                .find(|v| v["id"] == id)
                .unwrap();
            for method in array(&item["methods"])
                .iter()
                .filter(|m| m[0] == "帕鲁掉落")
            {
                assert!(
                    response.text.contains(method[1].as_str().unwrap()),
                    "{}",
                    response.text
                );
            }
        }
        assert!(
            oil.text.contains("物品来源汇总另列枯星龙") && oil.text.contains("物种掉落表未收录"),
            "{}",
            oil.text
        );
        assert!(
            !oil.text.contains("帕鲁远征：") && !oil.text.contains("钓鱼池："),
            "{}",
            oil.text
        );
        let shroom = ask("What is the most efficient way to farm Cavern Mushrooms?").unwrap();
        for fact in ["地下城或矿井", "菇咚", "500 金币", "远征", "无法确定"] {
            assert!(shroom.text.contains(fact), "{fact}: {}", shroom.text);
        }
        let combined = ask("优质帕鲁油只看商店和牧场，分别怎么买、由谁产出？").unwrap();
        assert!(
            combined.text.contains("梆梆鲶")
                && combined.text.contains("300 金币")
                && !combined.text.contains("战斗掉落"),
            "{}",
            combined.text
        );
        let excluded = ask("除了放牧，洞穴蘑菇还有哪些途径能拿到？").unwrap();
        assert!(
            excluded.text.contains("地下城或矿井") && !excluded.text.contains("菇咚"),
            "{}",
            excluded.text
        );
        let buying = ask("洞穴蘑菇只考虑购买，是找谁、每个要多少金币？").unwrap();
        assert!(
            buying.text.contains("传说中的蔬果店老板")
                && buying.text.contains("500 金币")
                && !buying.text.contains("远征"),
            "{}",
            buying.text
        );
        let paid_with = ask("用古代文明部件购买发电器官，每份实际要多少金币？").unwrap();
        assert!(
            paid_with
                .links
                .iter()
                .any(|(_, v)| v == "#items/ElectricOrgan/shops")
                && paid_with.text.contains("300 金币")
                && !paid_with.text.contains("古代文明部件在当前图鉴"),
            "{}",
            paid_with.text
        );
        let single =
            ask("只看天坠之地的野外拾取奖池，暗黑碎片有几档？每档一次能拿几个、概率有多大？")
                .unwrap();
        assert!(
            single.text.contains("1 种数量档")
                && single.text.contains("一次 1 个")
                && single.text.contains("15.21780483%")
                && !single.text.contains("天阳乡"),
            "{}",
            single.text
        );
        let other = ask("只看天阳乡的野外拾取奖池，暗黑碎片有几档，各多少？").unwrap();
        let phrasing =
            ask("我只查天坠之地，暗黑碎片的野外拾取奖励一次是几份、表里概率多少？").unwrap();
        assert!(
            phrasing.text.contains("一次 1 个")
                && phrasing.text.contains("15.21780483%")
                && !phrasing.text.contains("天阳乡"),
            "{}",
            phrasing.text
        );
        assert!(
            other.text.contains("14.93373157%") && !other.text.contains("15.21780483%"),
            "{}",
            other.text
        );
        let multi = ask("只看天坠之地的敌人据点奖池，金币有哪些数量档和概率？").unwrap();
        for part in [
            "2 种数量档",
            "2 条奖励记录",
            "栏位 1",
            "5500–6000",
            "18.59657823%",
            "栏位 2",
            "4500–5000",
            "100%",
        ] {
            assert!(multi.text.contains(part), "{part}: {}", multi.text);
        }
        let absent = ask("只看沙漠的野外拾取奖池，暗黑碎片每档多少？").unwrap();
        assert!(
            absent.text.contains("没有查到") && !absent.text.contains("15.21780483%"),
            "{}",
            absent.text
        );
    }
}
