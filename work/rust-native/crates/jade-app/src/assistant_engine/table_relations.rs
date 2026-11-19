//! Queries which require joining independent columns rather than matching a single name.
use super::AssistantReply;
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, array, text};
use serde_json::Value;

mod forms;
mod passive_scope;
#[cfg(test)]
mod tests;
mod wild_spawns;

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|t| q.contains(t))
}

fn english(q: &str) -> bool {
    !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
}

fn answer_lines(lines: Vec<String>, links: Vec<(String, String)>) -> AssistantReply {
    AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    }
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if has(
        q,
        &[
            "old version",
            "prior to 1.0",
            "before 1.0",
            "pre-1.0",
            "v0.",
            "0.6.",
            "0.7.",
            "旧版",
            "早期版本",
            "改版前",
            "modded",
            "修改器",
        ],
    ) {
        return None;
    }
    forms::compare(d, q)
        .or_else(|| wild_spawns::answer(d, q))
        .or_else(|| offspring_skill(d, q))
        .or_else(|| passive_scope::answer(d, q))
        .or_else(|| skills(d, q))
}

fn offspring_skill(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "配种",
            "子代",
            "后代",
            "孩子",
            "孵蛋",
            "breed",
            "offspring",
            "hatch",
        ],
    ) || !has(
        q,
        &[
            "遗传",
            "继承",
            "传给",
            "inherit",
            "pass down",
            "passed down",
        ],
    ) || !has(
        q,
        &[
            "伙伴技",
            "被动",
            "词条",
            "partner skill",
            "passive",
            "traits",
        ],
    ) || has(
        q,
        &[
            "哪些",
            "多少",
            "概率",
            "几率",
            "只传",
            "不能传",
            "哪些不",
            "怎么配",
            "路线",
            "chance",
            "odds",
            "how many",
            "which",
            "cannot",
            "can't",
            "except",
            "how to",
            "only",
        ],
    ) {
        return None;
    }
    let (spans, _) = species_mentions(&d.species, q);
    let [(i, _, _)] = spans.as_slice() else {
        return None;
    };
    let owner = &d.species[*i];
    let key = text(owner, "key");
    let partner = d.get("partner-skills-data").get(key)?;
    // An ordinary named passive has its own transfer/implant rules. Don't
    // displace that lookup with a general explanation of partner abilities.
    if !super::skill_query::mentioned_traits(d, q).is_empty() {
        return None;
    }
    let en = english(q);
    let specific = has(q, &["伙伴技", "partner skill", "partner ability"]);
    if !specific && !has(q, &["八云犬", "yakumo", "八云狗"]) {
        return None;
    }
    let who = text(owner, if en { "en" } else { "zh" });
    let skill = if en && key == "GuardianDog" {
        "Birds of a Feather"
    } else {
        text(partner, "name")
    };
    let line = if en {
        format!(
            "{who}'s partner ability ({skill}) is tied to the offspring's species. A different species does not inherit that ability. Ordinary transferable passive traits on a parent can be inherited when breeding, with no guarantee for an individual egg."
        )
    } else {
        format!(
            "{who}的伙伴技能「{skill}」由子代物种决定，其他物种的孩子不会通过配种继承它。亲本被动栏中可传的普通词条则有机会遗传，每颗蛋都要单独查看。"
        )
    };
    Some(answer_lines(
        vec![line],
        vec![
            (who.into(), format!("#details/partner/{key}")),
            (
                if en { "Passive skills" } else { "被动技能" }.into(),
                "#details/passive".into(),
            ),
        ],
    ))
}

fn skills(d: &Data, q: &str) -> Option<AssistantReply> {
    let passive = has(q, &["被动", "词条", "特性", "passive", "trait", "perk"]);
    let active = has(q, &["主动", "招式", " attacks", " moves", " attack skills"]);
    let yakumo = has(
        q,
        &[
            "八云犬",
            "八云狗",
            "物以类聚",
            "yakumo",
            "birds of a feather",
        ],
    );
    if !(passive || active || yakumo) {
        return None;
    }
    let list = has(
        q,
        &[
            "哪些",
            "列表",
            "列出",
            "清单",
            "名单",
            "全套",
            "所有",
            "哪几",
            "which ",
            "what traits",
            "what passiv",
            "what moves",
            "list ",
            "every ",
            "all the ",
        ],
    );
    let breeding = has(
        q,
        &[
            "遗传",
            "配种",
            "繁殖",
            "继承",
            "offspring",
            "breeding",
            "breed",
            "inherit",
        ],
    );
    let no_fruit = has(
        q,
        &[
            "没技能果",
            "没果实",
            "没有技能果",
            "没有果实",
            "无果实",
            "无技能果",
            "without a skill fruit",
            "without skill fruit",
            "no skill fruit",
            "without fruit",
            "no fruit",
            "no corresponding fruit",
        ],
    );
    let exclusive = has(
        q,
        &[
            "唯一",
            "只能",
            "仅能",
            "只有",
            "only",
            "instead of",
            "no other",
            "not by any other",
        ],
    );
    let transfer = list && breeding && passive && active && exclusive;
    let moves = list && active && no_fruit && breeding;
    let random = has(q, &["随机", "random", "naturally roll"]);
    let implant = has(q, &["植入", "植体", "手术台", "implant", "surgery"]);
    let copying = has(
        q,
        &[
            "野捕", "野生", "捕获", "抓", "同步", "传", "复制", "copy", "transfer", "catch",
        ],
    ) || q
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|s| matches!(s, "pass" | "passes" | "passed"));
    if !(transfer || moves || passive && list && (random || implant) || yakumo && copying) {
        return None;
    }
    if has(
        q,
        &[
            "工作速度",
            "攻击力",
            "防御力",
            "面板",
            "速度最快",
            "哪种属性",
            "best",
            "recommend",
            "strongest",
            "fastest",
            "increase ",
            "improve ",
            "boost ",
            "buff ",
            " improve",
            " reduce",
            " damage",
            "速度加成",
            "提高",
            "提升",
            "加攻",
            "how many eggs",
            "概率多少",
            "几率多少",
            "chance of",
            "what level",
            "几级",
            "modded",
            "修改器",
            "1.0之前",
            "before 1.0",
            "for attack",
            "for defense",
            "for speed",
            "for damage",
            "rainbow",
            "彩虹",
            "金色",
            "rank ",
            "tier ",
            "cannot inherit",
            "can't inherit",
            "not inherit",
            "不可遗传",
            "不能遗传",
            "无法遗传",
            "不遗传",
        ],
    ) {
        return None;
    }
    if (moves || transfer)
        && [
            "fire", "water", "grass", "electric", "ground", "ice", "dark", "dragon", "neutral",
        ]
        .iter()
        .any(|element| {
            q.split(|c: char| !c.is_ascii_alphabetic())
                .any(|w| w == *element)
        })
        || has(
            q,
            &[
                "火系",
                "雷系",
                "冰系",
                "水系",
                "草系",
                "地系",
                "暗系",
                "龙系",
                "火属性",
                "水属性",
                "草属性",
                "雷属性",
                "冰属性",
                "地属性",
                "暗属性",
                "龙属性",
                "无属性",
            ],
        )
    {
        return None;
    }
    if yakumo
        && (breeding
            || has(
                q,
                &[
                    "孵蛋",
                    "蛋里",
                    "mutation egg",
                    "hatch",
                    "from an egg",
                    "在哪",
                    "哪里抓八云",
                    "where can i catch yakumo",
                    "where to catch yakumo",
                    "saddle",
                    "鞍具",
                ],
            ))
    {
        return None;
    }
    // The two documented flags can establish an intersection. They cannot
    // establish exclusivity across every source or satisfy a disjunction.
    if !transfer
        && !moves
        && (exclusive
            || has(
                q,
                &[
                    " either ",
                    " or ",
                    "或者",
                    "或是",
                    "或能",
                    "except ",
                    "apart from",
                    "除了",
                    "以外的",
                ],
            ))
    {
        return None;
    }
    if yakumo && (implant || random && has(q, &["but not", "同时", "还要", "而且"])) {
        return None;
    }
    if !yakumo && !transfer && !moves && breeding {
        return None;
    }
    let (pals, _) = species_mentions(&d.species, q);
    if pals
        .iter()
        .any(|(i, _, _)| !yakumo || d.species[*i]["key"] != "GuardianDog")
    {
        return None;
    }
    let en = english(q);
    let lang = if en { "en" } else { "zh" };
    let details = d.get("pal-details-data");
    let mut lines = Vec::new();
    let mut links = Vec::new();
    if transfer {
        // Fixed carriers and absence of implants are independent filters. Only the
        // verified elemental/Legend families here imply normal downward transfer.
        let mut facts = Vec::new();
        for (id, row) in details["passives"].as_object()? {
            if id != "Legend" && !(id.starts_with("ElementBoost_") && id.ends_with("_2_PAL")) {
                continue;
            }
            if row["random"] != false || row["surgery"].is_object() {
                continue;
            }
            let Some(parent) = d.species.iter().find(|s| {
                array(&details["profiles"][text(s, "key")]["fixed"])
                    .iter()
                    .any(|t| t == id.as_str())
            }) else {
                continue;
            };
            facts.push(format!("{} ({})", text(row, lang), text(parent, lang)));
            links.push((
                text(row, lang).into(),
                format!("#details/passive/trait/{id}"),
            ));
        }
        if facts.is_empty() {
            return None;
        }
        lines.push(if en{format!("Passives with a verified donor route include {}. The name in parentheses is a species with that fixed trait. Obtain a carrying parent and breed it down. These traits are outside the random pool and have no listed implant.",facts.join(", "))}
            else{format!("有固定亲本、可向后代传的被动示例为{}。括号中是图鉴登记的携带物种。先取得实际带词条的个体，再选作亲本。这些被动均不在通用随机池中，且未列植入体。",facts.join("、"))});
    }
    if transfer || moves {
        // Never treat a missing fruit alone as evidence for inheritance. Require
        // a positive flag and at least one actual species/learn-level relation.
        let mut facts = Vec::new();
        for (id, row) in details["moves"].as_object()? {
            if row["inherit"] != true || row["fruit"].as_str().is_some_and(|s| !s.is_empty()) {
                continue;
            }
            let Some((parent, level)) = d.species.iter().find_map(|s| {
                let entry = array(&details["profiles"][text(s, "key")]["levels"])
                    .iter()
                    .find(|r| r[1] == id.as_str())?;
                Some((s, entry[0].as_u64()?))
            }) else {
                continue;
            };
            facts.push(format!(
                "{} ({} Lv.{level})",
                text(row, lang),
                text(parent, lang)
            ));
            links.push((text(row, lang).into(), format!("#details/active/move/{id}")));
        }
        if facts.is_empty() {
            return None;
        }
        lines.push(if en {format!("Inheritable attacks with no listed skill fruit, with one natural learner and level for each: {}. The donor species can learn its own move by leveling. The inheritance route is for passing it to a different Pal; special encounter sources are outside this filter.",facts.join(", "))}
            else{format!("图鉴标为可遗传、没有对应技能果实的主动招式有{}。括号中是一种自然习得的亲本及等级；该物种自己升级就能学，繁殖用于传给其他帕鲁。筛选范围不包含特殊遭遇授招。",facts.join("、"))});
        return Some(answer_lines(lines, links));
    }
    let names = super::skill_query::mentioned_traits(d, q);
    if !list && (names.len() != 1 || !yakumo) || list && !names.is_empty() {
        return None;
    }
    let not_random = has(
        q,
        &[
            "不随机",
            "不能随机",
            "不会随机",
            "不能靠随机",
            "不靠随机",
            "随机不出",
            "非随机",
            "随机池外",
            "不在随机",
            "outside the random",
            "outside of the random",
            "cannot roll",
            "can't roll",
            "never roll",
            "not random",
            "not roll",
            "out of the random",
            "aren't random",
            "non-random",
            "nonrandom",
            "excluded from the random",
        ],
    );
    let not_implant = has(
        q,
        &[
            "没有植入",
            "没植入",
            "不能植入",
            "不可植入",
            "无法植入",
            "无植入",
            "没有对应植入",
            "no implant",
            "without implant",
            "without an implant",
            "cannot be implanted",
            "can't be implanted",
            "not implantable",
            "lack an implant",
        ],
    );
    let neg_copy = has(
        q,
        &[
            "不能",
            "无法",
            "不支持",
            "不可",
            "传不了",
            "同步不了",
            "排除",
            "哪些不",
            "例外",
            "cannot",
            "can not",
            "can't",
            "not pass",
            "not copy",
            "unable",
            "exclude",
            "exception",
            "ineligible",
        ],
    );
    let random_expected = if yakumo {
        if list { Some(!neg_copy) } else { None }
    } else {
        random.then_some(!not_random)
    };
    let implant_expected = implant.then_some(!not_implant);
    let rows = array(&d.get("trait-data")["traits"]);
    if rows.is_empty() {
        return None;
    }
    let mut matches = Vec::new();
    for row in rows {
        if !names.is_empty() && !names.contains(&text(row, "key")) {
            continue;
        }
        let random = row["random"].as_bool()?;
        let surgery = row["surgery"].as_bool()?;
        if random_expected.is_some_and(|v| random != v)
            || implant_expected.is_some_and(|v| surgery != v)
        {
            continue;
        }
        matches.push((text(row, lang), random));
        if !list {
            links.push((
                text(row, lang).into(),
                format!("#details/passive/trait/{}", text(row, "key")),
            ));
        }
    }
    if matches.is_empty() {
        return None;
    }
    let eligible = matches[0].1;
    let labels = matches
        .iter()
        .map(|(s, _)| *s)
        .collect::<Vec<_>>()
        .join(if en { ", " } else { "、" });
    if yakumo {
        links.push((
            if en { "Yakumo" } else { "八云犬" }.into(),
            "#details/partner/GuardianDog".into(),
        ));
        let qualifier = if en {
            if eligible { "eligible" } else { "excluded" }
        } else if eligible {
            "可参与判定"
        } else {
            "不参与判定"
        };
        let rule = match (en, eligible) {
            (true, true) => {
                "Deploy Yakumo carrying the desired trait to increase the chance on capture."
            }
            (true, false) => {
                "Even if Yakumo carries the listed trait, its partner ability cannot copy it onto a capture."
            }
            (false, true) => "捕捉前让携带目标词条的八云犬出战，可提高该词条出现的机会。",
            (false, false) => {
                "即使出战的八云犬带有上列词条，也无法通过这个伙伴技能将其复制给捕获目标。"
            }
        };
        lines.push(if en {format!("Birds of a Feather on capture uses the ordinary random passive pool. {} {}: {labels}. {rule}",matches.len(),qualifier)}
            else{format!("八云犬「物以类聚」看普通野捕的随机被动池。{}的词条共 {} 项：{labels}。{rule}",qualifier,matches.len())});
    } else {
        let mut criteria = Vec::new();
        if let Some(v) = random_expected {
            criteria.push(match (en, v) {
                (true, true) => "in the random pool",
                (true, false) => "outside the random pool",
                (false, true) => "可随机产生",
                (false, false) => "不在通用随机池",
            });
        }
        if let Some(v) = implant_expected {
            criteria.push(match (en, v) {
                (true, true) => "a listed implant",
                (true, false) => "no listed implant",
                (false, true) => "有植入体",
                (false, false) => "未列植入体",
            });
        }
        lines.push(if en{format!("{} passives match {}: {labels}. These columns describe generation and surgery; they do not measure parent-to-egg inheritance.",matches.len(),criteria.join(" + "))}
            else{format!("按{}筛到 {} 条被动：{labels}。随机产生和能否植入分别取自图鉴对应栏；亲本遗传另有规则。",criteria.join("、"),matches.len())});
    }
    links.push((
        if en { "Passive skills" } else { "被动技能" }.into(),
        "#details/passive".into(),
    ));
    Some(answer_lines(lines, links))
}
