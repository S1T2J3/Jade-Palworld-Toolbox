//! Derive party and base roles from the installed species and partner tables.
use super::{AssistantReply, Data, array, has, text};
use crate::assistant_breeding::species_mentions;

pub(super) fn display_name(row: &serde_json::Value) -> String {
    format!("{} ({})", text(row, "zh"), text(row, "en"))
}

fn named_role(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "值得练",
            "值得养",
            "值得培养",
            "好用吗",
            "厉害吗",
            "怎么样",
            "有什么用",
            "适合干什么",
            "有什么优势",
            "好不好用",
            "worth training",
            "worth keeping",
            "worth using",
            "any good",
            "actually good",
            "good for",
            "useful for",
            "strong points",
            "what role",
        ],
    ) {
        return None;
    }
    let (names, _) = species_mentions(&d.species, q);
    let [(idx, _, _)] = names.as_slice() else {
        return None;
    };
    let pal = &d.species[*idx];
    let key = text(pal, "key");
    let profile = &d.get("paldex-index-data")["profiles"][key];
    let partner = &d.get("partner-skills-data")[key];
    let effect = partner["effect"].as_str()?;
    let jobs = array(&profile["work"])
        .iter()
        .filter_map(|row| Some(format!("{} Lv.{}", row[0].as_str()?, row[1].as_u64()?)))
        .collect::<Vec<_>>();
    let name = display_name(pal);
    let mut lines = vec![format!(
        "{name} 随队时可用伙伴技能「{}」。{effect}",
        text(partner, "name")
    )];
    if !jobs.is_empty() {
        lines.push(format!("派驻据点可承担的工作为{}。", jobs.join("、")));
    }
    if let (Some(hp), Some(atk), Some(def)) = (
        profile["hp"].as_u64(),
        profile["attack"].as_u64(),
        profile["defense"].as_u64(),
    ) {
        lines.push(format!(
            "战斗基础种族值为生命 {hp}、攻击 {atk}、防御 {def}。"
        ));
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links: vec![(name, format!("#paldex/{key}"))],
        suggestions: vec![],
    })
}

fn party_bonus(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "提高", "提升", "增加", "增伤", "加攻", "加防", "增强", "加成", "buff", "boost",
            "increase", "amplif",
        ],
    ) {
        return None;
    }
    let defense = has(q, &["防御", "defense", "defence"]);
    let attack = has(q, &["攻击", "伤害", "加攻", "增伤", "attack", "damage"]);
    if defense == attack {
        return None;
    }
    let elements = d.get("combat-guide-data")["elements"].as_object()?;
    let mut requested = elements.iter().filter_map(|(en, zh)| {
        let zh = zh.as_str()?;
        let named = q
            .split(|c: char| !c.is_ascii_alphabetic())
            .any(|word| word == en)
            || q.contains(&format!("{zh}属性"))
            || q.contains(&format!("{zh}系"));
        named.then_some(zh)
    });
    let element = requested.next()?;
    if requested.next().is_some() {
        return None;
    }
    let field = if defense { "防御力" } else { "攻击力" };
    if has(
        q,
        &[
            "玩家",
            "player",
            "my weapon",
            "我的武器",
            "抗性",
            "resistance",
        ],
    ) {
        return None;
    }
    crate::assistant_engine::party_stat_sources(d, q, element, field)
}

pub(super) fn lookup(d: &Data, q: &str) -> Option<AssistantReply> {
    named_role(d, q).or_else(|| party_bonus(d, q))
}
