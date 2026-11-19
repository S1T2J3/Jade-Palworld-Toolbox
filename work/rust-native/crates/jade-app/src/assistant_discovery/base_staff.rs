use super::{AssistantReply, Data, array, has, text};
use crate::assistant_breeding::species_mentions;

fn english(q: &str) -> bool {
    !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
}

/// A party bonus to the player's backpack is independent of the Transporting job.
pub(crate) fn carrying(data: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "carry weight",
            "carry capacity",
            "carrying capacity",
            "carry more",
            "carrying more",
            "backpack weight",
            "负重",
            "背更多",
            "多背点",
        ],
    ) || !has(
        q,
        &[" pal", "pals", "partner", "party", "帕鲁", "哪只", "随队"],
    ) || has(
        q,
        &[
            "衣服",
            "装备怎么",
            "饰品",
            "accessor",
            "per trip",
            "transporting",
            "hauling",
            "at my base",
            "仓库",
            "据点搬运",
            "每趟",
            "without ",
            "except ",
            "排除",
            "不要",
            "breed",
            "配种",
            "modded",
            "旧版",
        ],
    ) {
        return None;
    }
    let candidates = species_mentions(&data.species, q).0;
    let en = english(q);
    let table = data.get("partner-skills-data");
    let cargo = [
        ("weapon", "武器", "背包内武器", "weapons"),
        ("wood", "木材", "所有木材种类", "wood"),
        ("stone", "石头", "石头的重量", "stone"),
        ("coal", "石炭", "硫磺和石炭的重量", "sulfur and coal"),
        ("sulfur", "硫磺", "硫磺和石炭的重量", "sulfur and coal"),
        ("metal ore", "金属矿石", "金属矿石的重量", "metal ore"),
        ("ore", "矿石", "所有矿石种类的重量", "all ore types"),
        ("food", "食物", "食材和料理", "ingredients and cooked food"),
    ]
    .into_iter()
    .find(|(word, zh, _, _)| {
        q.contains(zh)
            || word.contains(' ') && q.contains(word)
            || q.split(|c: char| !c.is_ascii_alphabetic())
                .any(|w| w.strip_suffix('s').unwrap_or(w) == *word)
    });
    if let Some((_, _, marker, description)) = cargo {
        let mut lines = Vec::new();
        let mut links = Vec::new();
        for (index, pal) in data.species.iter().enumerate() {
            if !candidates.is_empty() && !candidates.iter().any(|p| p.0 == index) {
                continue;
            }
            let key = text(pal, "key");
            let ability = &table[key];
            let Some((_, clause)) = text(ability, "effect").split_once(marker) else {
                continue;
            };
            let weight_clause = clause.split(['。', '，']).next()?;
            let (_, values) = ability["curves"]
                .as_object()?
                .iter()
                .find(|(range, _)| weight_clause.contains(range.as_str()))?;
            let (low, high) = (values.get(0)?.as_u64()?, values.get(4)?.as_u64()?);
            let name = text(pal, if en { "en" } else { "zh" });
            lines.push(if en { format!("Keep {name} in the party to reduce the weight of {description} by {low}% at partner level 1 or {high}% at level 5.") }
                else { format!("把{name}放入随身队伍。图鉴效果：{}", text(ability, "effect")) });
            links.push((name.to_owned(), format!("#details/partner/{key}")));
        }
        if !lines.is_empty() {
            return Some(AssistantReply {
                text: lines.join("\n"),
                links,
                suggestions: vec![],
            });
        }
    }
    let mut rows = Vec::new();
    for (index, species) in data.species.iter().enumerate() {
        if !candidates.is_empty() && !candidates.iter().any(|p| p.0 == index) {
            continue;
        }
        let key = text(species, "key");
        let ability = &table[key];
        let effect = text(ability, "effect");
        if !effect.contains("玩家的负重上限") {
            continue;
        }
        let (_, values) = ability["curves"].as_object()?.iter().find(|(range, _)| {
            effect
                .split_once("玩家的负重上限")
                .is_some_and(|(_, part)| part.contains(range.as_str()))
        })?;
        let base = values.get(0)?.as_u64()?;
        let maximum = values.get(4)?.as_u64()?;
        rows.push((std::cmp::Reverse(maximum), base, species));
    }
    if rows.is_empty() || (!candidates.is_empty() && rows.len() != candidates.len()) {
        return None;
    }
    rows.sort_by_key(|(maximum, _, _)| *maximum);
    let mut links = Vec::new();
    let mut lines = vec![if en {
        "For a larger player backpack, keep one of these Pals in your active party. Their flat carry-weight bonuses at partner skill level 1 and 5 are:".to_owned()
    } else {
        "想让角色背更多东西，把下面的帕鲁放入随身队伍。伙伴技能 1 级和 5 级分别增加的负重上限如下。"
            .to_owned()
    }];
    for (std::cmp::Reverse(maximum), base, pal) in rows {
        let name = text(pal, if en { "en" } else { "zh" });
        let key = text(pal, "key");
        lines.push(format!("{name} · +{base} → +{maximum}"));
        links.push((name.to_owned(), format!("#details/partner/{key}")));
    }
    lines.push(if en {
        "Duplicates of the same partner skill do not stack. A base worker’s Transporting suitability controls how many items it moves per trip inside the base.".into()
    } else { "同种伙伴技能不可叠加。若要把据点内的货物搬进箱子，看的则是工人的搬运适性和每趟搬运量。".into() });
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

pub(crate) fn workers(data: &Data, q: &str) -> Option<AssistantReply> {
    let asks_for_workers = has(
        q,
        &[
            "who should",
            "which pal",
            "what pal",
            "best pal",
            "recommend",
            "worker lineup",
            "best worker",
            "best base",
            "base team",
            "base setup",
            "staff my",
            "which worker",
            "looking for a pal",
            "a pal that can",
            "any pal that",
            "cover both",
            "assign workers",
            "split the work",
            "staff a base",
            "who can",
            "谁去",
            "派谁",
            "阵容",
            "哪些帕鲁",
            "哪只帕鲁",
            "哪种帕鲁",
            "怎么分工",
            "分配工人",
            "怎么安排",
            "如何安排",
            "怎么派工",
            "怎么配人",
            "如何分配",
            "一只帕鲁",
            "有没有能",
            "推荐",
        ],
    );
    let mining = has(q, &["mining", "挖矿", "采矿", "矿石", "矿场"])
        || q.split(|c: char| !c.is_ascii_alphabetic())
            .any(|w| matches!(w, "ore" | "ores" | "mines"));
    let ranch = has(q, &["ranch", "牧场", "放牧"]);
    let hauling = has(
        q,
        &[
            "transporting",
            "hauling",
            "base transporter",
            "base hauler",
            "transport pal",
            "hauling pal",
            "搬运",
            "搬货",
        ],
    );
    let mut roles = crate::assistant_engine::JOBS
        .iter()
        .filter(|&&(role, _, aliases)| {
            aliases.iter().any(|term| {
                if term.is_ascii() {
                    q.match_indices(term).any(|(a, w)| {
                        !q[..a].ends_with(|ch: char| ch.is_ascii_alphabetic())
                            && !q[a + w.len()..].starts_with(|ch: char| ch.is_ascii_alphabetic())
                    })
                } else {
                    q.contains(term)
                }
            }) || match role {
                "采矿" => mining,
                "搬运" => hauling,
                "牧场" => ranch,
                "发电" => has(
                    q,
                    &[
                        "供电",
                        "发电机",
                        "generate electricity",
                        "generate power",
                        "power generation",
                        "powering",
                    ],
                ),
                "播种" => has(q, &["耕种", "播撒", "sow seeds", "sowing"]),
                "采集" => has(q, &["收割", "收获", "harvesting"]),
                "生火" => has(q, &["点火", "烧火", "熔炼"]),
                "伐木" => has(q, &["lumberjack", "木材采伐"]),
                _ => false,
            }
        })
        .map(|&(zh, en, _)| (zh, en))
        .collect::<Vec<_>>();
    let single_job_ranking = roles.len() == 1 && !ranch && !has(q, &[
        "combat", "attack", "damage", "capture", "drop", "base location", "saddle", "build a", "place a",
        "战斗", "攻击", "伤害", "捕获", "掉落", "坐标", "选址", "鞍具", "地方",
    ]) && has(q, &[
        "best pal", "best worker", "highest", "top worker", "best at", "best for",
        "最适合", "适性最高", "适应性最高", "最强", "最擅长",
    ]);
    if !asks_for_workers && !single_job_ranking
        || !(roles.len() >= 2 || hauling || single_job_ranking)
        || has(
            q,
            &[
                "配种",
                "排除",
                "不要",
                "前期",
                "only my",
                "without ",
                "except ",
                "early game",
                "for food",
                "what food",
                "best food",
                "which food",
                "location",
                "where to build",
                "where should i build",
                "nice spot",
                "选址",
                "在哪里建",
                "which meal",
                "喂什么",
                "仓库里",
                "level to unlock",
                "what level",
                "科技等级",
                "几级解锁",
                "建筑材料",
                "passive",
                "work speed",
                "per hour",
                "词条",
                "每小时",
                "slots",
                "个位置",
                "个工位",
                "只能派",
                "only have",
                "只剩",
                "mounted",
                "while riding",
                "partner skill",
                "骑乘",
                "伙伴技能",
                "nocturnal",
                "at night",
                "夜间",
                "夜行",
                "excluding",
                "can't get",
                "还抓不到",
            ],
        )
        || !species_mentions(&data.species, q).0.is_empty()
    {
        return None;
    }
    let en = english(q);
    let profiles = data.get("paldex-index-data")["profiles"].as_object()?;
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let same_pal = roles.len() > 1
        && has(
            q,
            &[
                "同一只",
                "一只帕鲁",
                "一只同时",
                "哪只兼",
                "兼任",
                "兼顾",
                "两种都会",
                "两项都会",
                "既会",
                "都会",
                "both jobs",
                "both roles",
                "do both",
                "does both",
                "cover both",
                "one pal",
                "single pal",
                "a pal that can",
            ],
        );
    if same_pal {
        let mut qualified = Vec::new();
        for pal in &data.species {
            if text(pal, "reason").contains("无法捕获") {
                continue;
            }
            let key = text(pal, "key");
            let Some(profile) = profiles.get(key) else {
                continue;
            };
            let ranks = roles
                .iter()
                .filter_map(|(job, _)| {
                    array(&profile["work"]).iter().find(|r| r[0] == *job)?[1].as_u64()
                })
                .collect::<Vec<_>>();
            if ranks.len() == roles.len() && ranks.iter().all(|&n| n > 0) {
                let minimum = *ranks.iter().min()?;
                qualified.push((
                    std::cmp::Reverse(minimum),
                    std::cmp::Reverse(ranks.iter().sum::<u64>()),
                    text(pal, if en { "en" } else { "zh" }),
                    key,
                    ranks,
                ));
            }
        }
        qualified.sort_unstable();
        lines.push(if qualified.is_empty() {
            if en { "No obtainable species in the handbook has every requested natural suitability. Use separate workers as follows." }
            else { "图鉴中没有一只可获取物种兼具这些基础适性，可按下列工种分别派工。" }
        } else if en { "These Pals cover all requested jobs. Ranked by the lowest requested natural suitability, then their sum:" }
        else { "以下物种都兼具所问工种，按其中最低基础适性优先、再按各等级合计排序。" }.to_owned());
        for (_, _, pal, key, ranks) in qualified.iter().take(3) {
            let stats = roles
                .iter()
                .zip(ranks)
                .map(|(&(zh, eng), level)| format!("{} Lv.{level}", if en { eng } else { zh }))
                .collect::<Vec<_>>();
            lines.push(format!("{pal} — {}", stats.join(" / ")));
            links.push((pal.to_string(), format!("#work/suitability/{key}")));
        }
        if !qualified.is_empty() {
            lines.push(if en { "One worker performs one task at a time. Assign specialists if the jobs need to run simultaneously." } else { "一只帕鲁一次只能执行一项工作；需要各岗位同时运作时，分别派工。" }.into());
            return Some(AssistantReply {
                text: lines.join("\n"),
                links,
                suggestions: vec![],
            });
        }
    }
    if mining && ranch && !roles.iter().any(|(role, _)| *role == "搬运") {
        roles.push(("搬运", "Transporting"));
    }
    for (role, english_name) in roles {
        if role == "牧场" {
            continue;
        }
        let mut candidates: Vec<_> = data
            .species
            .iter()
            .filter_map(|pal| {
                if text(pal, "reason").contains("无法捕获") {
                    return None;
                }
                let key = text(pal, "key");
                let level = array(&profiles.get(key)?["work"])
                    .iter()
                    .find(|row| row[0] == role)?[1]
                    .as_u64()?;
                Some((
                    std::cmp::Reverse(level),
                    text(pal, if en { "en" } else { "zh" }),
                    key,
                ))
            })
            .collect();
        candidates.sort_unstable();
        let top = candidates.first()?.0;
        let mut names = Vec::new();
        for &(_, name, key) in candidates.iter().take_while(|r| r.0 == top).take(3) {
            names.push(name);
            links.push((name.to_owned(), format!("#work/suitability/{key}")));
        }
        let label = if en { english_name } else { role };
        let mut line = format!("{label} Lv.{} — {}", top.0, names.join(" / "));
        if single_job_ranking {
            line.push_str(if en { ". These are examples at the highest natural suitability rank for this base job." } else { "。这些是图鉴中这项据点工作的最高基础适性物种示例。" });
        }
        if role == "搬运" {
            if let Some(per_trip) = data.get("work-efficiency-data")["levels"][role]
                .get(top.0 as usize - 1)
                .and_then(|v| v.as_u64())
            {
                line.push_str(&if en {
                    format!("; up to {per_trip} items per trip")
                } else {
                    format!("，每趟最多搬 {per_trip} 件")
                });
            }
        }
        lines.push(line);
    }
    if ranch {
        let mut products = Vec::new();
        for key in ["ChickenPal", "CowPal", "SoldierBee"] {
            let Some(pal) = data.species.iter().find(|s| text(s, "key") == key) else {
                continue;
            };
            let Some(item) = data.get("ranch-data")["pals"][key]["tiers"][0][0]["item"].as_str()
            else {
                continue;
            };
            let Some(product) = array(&data.get("item-catalog-data")["items"])
                .iter()
                .find(|p| p["id"] == item)
            else {
                continue;
            };
            let name = text(pal, if en { "en" } else { "zh" });
            products.push(format!(
                "{name} → {}",
                text(product, if en { "en" } else { "name" })
            ));
            links.push((name.to_owned(), format!("#work/ranch/{key}")));
        }
        if products.is_empty() {
            return None;
        }
        lines.push(format!(
            "{} — {}",
            if en {
                "Ranch: choose by product"
            } else {
                "牧场按需要的产物选"
            },
            products.join(" / ")
        ));
    }
    lines.push(if en { "Assign these as base workers. Put storage by the work area to shorten hauling trips; actual throughput also depends on travel time.".into() }
        else { "将采集或生产与搬运分工。把箱子放在工作点附近可缩短搬运路径，实际运量还取决于路程。".into() });
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}
