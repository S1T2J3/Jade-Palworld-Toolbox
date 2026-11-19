use super::{AssistantReply, Data, Value, array, handbook, has, text};
pub(super) mod filters;
mod named_moves;

/// Pick a single named record. Nested item names such as "石头" in a longer
/// item name do not compete, but two independent mentioned names do.
fn select<'a>(rows: impl Iterator<Item = &'a Value>, q: &str, keys: &[&str]) -> Option<&'a Value> {
    let mut hits = Vec::new();
    for row in rows {
        for key in keys {
            let name = crate::assistant_engine::field_text(text(row, key));
            if name.chars().count() < 2 {
                continue;
            }
            for (start, _) in q.match_indices(&name) {
                let end = start + name.len();
                if name.is_ascii()
                    && (q[..start].ends_with(|c: char| c.is_ascii_alphanumeric())
                        || q[end..].starts_with(|c: char| c.is_ascii_alphanumeric()))
                {
                    continue;
                }
                hits.push((start, end, row));
            }
        }
    }
    let &(from, to, record) = hits.iter().max_by_key(|(start, end, _)| end - start)?;
    hits.iter()
        .all(|&(a, b, v)| std::ptr::eq(v, record) || (a, b) != (from, to) && a >= from && b <= to)
        .then_some(record)
}

fn card(message: String, name: &str, route: String) -> AssistantReply {
    AssistantReply {
        text: message,
        links: vec![(name.into(), route)],
        suggestions: Vec::new(),
    }
}

fn expedition(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "远征",
            "派遣",
            "派队",
            "战力",
            "多少分钟",
            "多久回来",
            "expedition",
            "dispatch",
            "recommended power",
        ],
    ) || !has(
        q,
        &[
            "时间",
            "多久",
            "用时",
            "分钟",
            "战力",
            "战斗力",
            "条件",
            "解锁",
            "奖励",
            "几率",
            "数量",
            "掉落",
            "多少",
            "minutes",
            "time",
            "power",
            "require",
            "reward",
            "chance",
            "unlock",
        ],
    ) {
        return None;
    }
    let stage = select(
        array(&d.get("expedition-data")["stages"]).iter(),
        q,
        &["name", "en", "id"],
    )?;
    let id = text(stage, "id");
    let title = text(stage, "name");
    let route = format!("#work/expeditions/{id}");
    let row = serde_json::json!({"kind":"远征","href":route});
    let facts = handbook::summary(d, &row, q)?;
    Some(card(format!("{title}：{facts}"), title, route))
}

fn move_fields(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "冷却",
            "クールタイム",
            "再使用時間",
            "属性は",
            "威力",
            "招式属性",
            "什么属性",
            "哪种属性",
            "几秒",
            "继承",
            "可遗传",
            "power",
            "cooldown",
            "element",
            "move type",
            "inherit",
        ],
    ) || has(
        q,
        &[
            "最强",
            "比较",
            "哪个",
            "哪些",
            "几级",
            "升级",
            "学会",
            "推荐",
            "配队",
            "叠加",
            "变强",
            "缩短",
            "学习等级",
            "怎么减",
            "best",
            "learns at",
            "at what level",
            "reduce",
            "faster",
            "compare",
            "which pals",
            "which moves",
        ],
    ) {
        return None;
    }
    let (id, mv) = match named_moves::resolve(d,q)? {
        named_moves::Resolution::One(id,row) => (id,row),
        named_moves::Resolution::Shared(answer) => return Some(answer),
    };
    let title = text(mv, "zh");
    let element = d.get("combat-guide-data")["elements"][text(mv, "type")].as_str()?;
    let mut facts = format!(
        "{title}（{}）是{element}属性主动招式，基础威力 {}，基础冷却 {} 秒。",
        text(mv, "en"),
        mv["power"].as_u64()?,
        mv["cooldown"].as_u64()?
    );
    let effects = array(&mv["effects"])
        .iter()
        .filter_map(|r| r[0].as_str())
        .collect::<Vec<_>>();
    if !effects.is_empty() {
        facts.push_str(&format!("附带效果：{}。", effects.join("、")));
    }
    if let Some(fruit) = mv["fruit"].as_str() {
        facts.push_str(&format!("图鉴收录的教学道具是「{fruit}」。"));
    } else if has(q, &["果实", "fruit"]) {
        facts.push_str("招式表没有列出对应的技能果实。");
    }
    if has(q, &["遗传", "继承", "inherit"]) {
        facts.push_str(if mv["inherit"].as_bool()? {
            "此招式可遗传。"
        } else {
            "此招式不可遗传。"
        });
    }
    Some(card(facts, title, format!("#details/active/move/{id}")))
}

fn item_recipe(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "哪些材料",
            "什么材料",
            "需要的材料",
            "原材料",
            "配方用料",
            "配方是什么",
            "which materials",
            "what materials",
            "ingredients",
            "crafting recipe",
        ],
    ) || has(
        q,
        &[
            "解锁条件",
            "还差",
            "我有",
            "库存",
            "最多",
            "怎么获得",
            "掉落",
            "不用",
            "只靠",
            "instead",
            "already have",
            "without",
            "left over",
        ],
    ) {
        return None;
    }
    let items = array(&d.get("item-catalog-data")["items"]);
    let item = select(items.iter(), q, &["name", "en"])?;
    let name = text(item, "name");
    let mut editions: Vec<_> = items
        .iter()
        .filter(|r| text(r, "name") == name && array(&r["recipes"]).len() == 1)
        .collect();
    if editions.is_empty() {
        return None;
    }
    editions.sort_by_key(|r| r["rarity"].as_u64());
    let unquoted = q.replace(['「', '」', '『', '』', '“', '”', '"'], "");
    let amount = crate::assistant_data::counted_craft(&unquoted);
    let qty = match amount {
        Some(n) => Some(
            n.for_item(text(item, "name"), false)
                .or_else(|| n.for_item(text(item, "en"), false))?,
        ),
        None => None,
    };
    if editions.len() == 1 {
        return Some(crate::assistant_data::scaled_recipe(d, editions[0], qty).into());
    }
    let tiers = ["普通", "少见", "稀有", "史诗", "传说"];
    let target_tier = tiers
        .iter()
        .position(|s| q.contains(s))
        .or_else(|| q.contains("罕见").then_some(1))
        .or_else(|| {
            ["common", "uncommon", "rare", "epic", "legendary"]
                .iter()
                .position(|s| q.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w == *s))
        });
    let mut lines = Vec::new();
    let mut links = Vec::new();
    for row in editions {
        let tier = row["rarity"].as_u64()? as usize;
        if target_tier.is_some_and(|t| t != tier) {
            continue;
        }
        let recipe = crate::assistant_data::scaled_recipe(d, row, qty);
        lines.push(format!(
            "{}品级：{}",
            tiers.get(tier)?,
            recipe
                .text
                .lines()
                .take(if qty.is_some() { 2 } else { 1 })
                .collect::<Vec<_>>()
                .join(" ")
        ));
        links.push((
            format!("{name} · {}", tiers[tier]),
            format!("#items/{}", text(row, "id")),
        ));
    }
    if lines.is_empty() {
        return None;
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

fn work(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "哪些工作",
            "什么工作",
            "工种有哪些",
            "适性有哪些",
            "有哪些工种",
            "各项适性",
            "what jobs",
            "which jobs",
            "work suitabilities",
        ],
    ) || has(
        q,
        &[
            "谁更",
            "比较",
            "分配",
            "为什么",
            "不能",
            "推荐",
            "升星",
            "浓缩",
            "双倍",
            "加成",
            "哪些帕鲁",
            "which pals",
            "compared",
            "boost",
            "assignment",
            "why",
        ],
    ) {
        return None;
    }
    let (names, _) = crate::assistant_breeding::species_mentions(&d.species, q);
    let [(i, _, _)] = names.as_slice() else {
        return None;
    };
    let pal = &d.species[*i];
    let (key, name) = (text(pal, "key"), text(pal, "zh"));
    let jobs: Vec<_> = array(&d.get("paldex-index-data")["profiles"][key]["work"])
        .iter()
        .filter_map(|r| Some(format!("{} Lv.{}", r[0].as_str()?, r[1].as_u64()?)))
        .collect();
    if jobs.is_empty() {
        return None;
    }
    Some(card(
        format!("{name}的基础工作适性为{}。", jobs.join("、")),
        name,
        format!("#paldex/{key}"),
    ))
}

fn station_for_action(d: &Data, q: &str) -> Option<AssistantReply> {
    if has(
        q,
        &[
            "怎么获得皮肤",
            "解锁皮肤",
            "unlock the skin",
            "buy skins",
            "where to buy",
            "概率",
            "chance",
            "造价",
            "价格",
        ],
    ) {
        return None;
    }
    let (id, tips) = if has(q, &["skin", "外观", "装扮", "换装"])
        && has(q, &["帕鲁", "pal ", "pals", "pal's"])
        && has(
            q,
            &[
                "换", "改", "套用", "change", "equip", "apply", "dress", "wear",
            ],
        ) {
        (
            "Pal_Dressing_Facility",
            "建好后与装扮机互动，选择要更换外观的帕鲁，然后选择已获得且适用于该物种的外观。",
        )
    } else if has(
        q,
        &["生病", "疾病", "illness", "sick", "healing the other pals"],
    ) && has(
        q,
        &[
            "药箱",
            "放药",
            "药柜",
            "medical supplies",
            "medicine chest",
            "medicine cabinet",
            "store medicine",
        ],
    ) {
        (
            "Medicine_Rack",
            "把相应的药品放入药品柜，让患病的据点帕鲁取用。先按病症选择合适的药品。",
        )
    } else if has(q, &["孵化器", "incubator"])
        && has(
            q,
            &[
                "使用",
                "怎么用",
                "操作",
                "how to use",
                "how do i use",
                "use the incubator",
                "start the incubator",
            ],
        )
        && !has(
            q,
            &[
                "温度", "多久", "时间", "快", "设置", "speed", "time", "temp", "setting",
            ],
        )
    {
        (
            "Egg_Incubator",
            "先把捡到或配种所得的帕鲁蛋放入孵化器，等进度结束后与孵化器互动取出帕鲁。",
        )
    } else {
        return None;
    };
    let facility = array(&d.get("facilities-data")["facilities"])
        .iter()
        .find(|s| s["id"] == id)?;
    let name = text(facility, "name");
    Some(card(
        format!(
            "使用「{name}」，科技等级是 {}。{}{tips}",
            facility["level"].as_u64()?,
            text(facility, "purpose")
        ),
        name,
        format!("#work/facilities/{id}"),
    ))
}

fn offense_options(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "主动技",
            "攻击技能",
            "输出技能",
            "主动招式",
            "offensive skill",
            "attack moves",
            "attacking moves",
            "strong moves",
            "damage skills",
        ],
    ) || !has(
        q,
        &[
            "推荐",
            "怎么选",
            "哪些好",
            "最强",
            "好用",
            "best",
            "recommend",
            "good",
            "strongest",
        ],
    ) || has(
        q,
        &["遗传", "不吃果", "without fruit", "inherit", "boss", "pvp"],
    ) {
        return None;
    }
    let (species, _) = crate::assistant_breeding::species_mentions(&d.species, q);
    if !species.is_empty() {
        return None;
    }
    let elements = d.get("combat-guide-data")["elements"].as_object()?;
    let types: Vec<_> = elements
        .iter()
        .filter(|(en, zh)| {
            q.split(|c: char| !c.is_ascii_alphabetic())
                .any(|w| w == en.as_str())
                || zh.as_str().is_some_and(|s| {
                    q.contains(&format!("{s}系")) || q.contains(&format!("{s}属性"))
                })
        })
        .map(|(en, _)| en.as_str())
        .collect();
    if types.len() > 1 {
        return None;
    }
    let moves = d.get("pal-details-data")["moves"].as_object()?;
    let eligible = || {
        moves.iter().filter(|(_, m)| {
            m["fruit"].as_str().is_some_and(|f| !f.is_empty())
                && m["power"].as_u64().is_some_and(|v| v > 0)
                && m["cooldown"].as_u64().is_some_and(|v| v > 0)
                && types.first().is_none_or(|s| m["type"].as_str() == Some(*s))
        })
    };
    let (fast_id, fast) = eligible()
        .filter(|(_, m)| m["cooldown"].as_u64().unwrap() <= 6)
        .max_by_key(|(_, m)| {
            m["power"].as_u64().unwrap() * 100 / m["cooldown"].as_u64().unwrap()
        })?;
    let (burst_id, burst) = eligible()
        .filter(|(id, m)| id != &fast_id && m["cooldown"].as_u64().unwrap() > 6)
        .max_by_key(|(_, m)| m["power"].as_u64().unwrap())?;
    let mut lines = vec![
        "想给一般战斗帕鲁配进攻招式，可以从短冷却和高单次威力各挑一招。这两个例子都有技能果实。"
            .into(),
    ];
    let mut links = Vec::new();
    for (label, id, m) in [
        ("短冷却补输出", fast_id, fast),
        ("长冷却打爆发", burst_id, burst),
    ] {
        let zh = text(m, "zh");
        lines.push(format!(
            "{label}：{zh}（{}），{}属性，基础威力 {}，冷却 {} 秒。",
            text(m, "en"),
            text(&d.get("combat-guide-data")["elements"], text(m, "type")),
            m["power"],
            m["cooldown"]
        ));
        links.push((zh.into(), format!("#details/active/move/{id}")));
    }
    lines.push("面对具体敌人先看属性克制，再考虑招式是否容易命中。威力和冷却是面板数值，实际输出还受动作时长和目标移动影响。".into());
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

pub(super) fn combat_passive(d: &Data, q: &str) -> Option<AssistantReply> {
    let question = q.split(['?', '？', '\n']).next().unwrap_or(q);
    if !has(
        question,
        &[
            "真需要",
            "有必要",
            "必须",
            "一定要",
            "非要",
            "do i really need",
            "do i need",
            "is it necessary",
            "is it required",
        ],
    ) || !has(
        q,
        &[
            "打赢",
            "通关",
            "击败",
            "挑战",
            "打过",
            "beat ",
            "defeat ",
            "raid",
            "boss fight",
        ],
    ) {
        return None;
    }
    let table = d.get("pal-details-data")["passives"].as_object()?;
    let passive = select(table.values(), question, &["zh", "en"])?;
    let (key, _) = table.iter().find(|(_, v)| std::ptr::eq(*v, passive))?;
    let effect = passive["effect"].as_str()?;
    let name = text(passive, "zh");
    Some(card(
        format!(
            "「{name}」（{}）的词条效果是{}。它可以影响战斗表现，图鉴没有将这项词条列为挑战的入场条件。先用现有队伍试打：如果时间耗尽而队伍还能存活，可提升等级、克制招式和输出；若先倒下，则先改善生存与换宠。不必只为了进场一直配种。",
            text(passive, "en"),
            effect.replace('\n', "、")
        ),
        name,
        format!("#details/passive/trait/{key}"),
    ))
}

pub(super) fn lookup(d: &Data, q: &str) -> Option<AssistantReply> {
    if !super::handbook_version_matches(q) {
        return None;
    }
    expedition(d, q)
        .or_else(|| move_fields(d, q))
        .or_else(|| item_recipe(d, q))
        .or_else(|| work(d, q))
        .or_else(|| station_for_action(d, q))
        .or_else(|| offense_options(d, q))
}
