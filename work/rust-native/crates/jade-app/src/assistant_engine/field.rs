use super::{AssistantReply, acquisition, facility_effect, field_text, habitat, skill_query, work_query};
use crate::assistant_discovery::relevant_names;
use crate::data::{Data, array, text};

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HandbookField {
    Acquisition,
    Work,
    Passive,
    Partner,
    Habitat,
    Facility,
}

impl HandbookField {
    fn source_heading(self) -> &'static str {
        match self {
            Self::Acquisition => "游戏图鉴 · 获取途径",
            Self::Work => "游戏图鉴 · 工作适性",
            Self::Passive => "游戏图鉴 · 被动词条",
            Self::Partner => "游戏图鉴 · 伙伴技能",
            Self::Habitat => "游戏图鉴 · 出没地点",
            Self::Facility => "游戏图鉴 · 据点设施",
        }
    }

    pub(crate) fn label_evidence(self, reply: &mut AssistantReply) {
        reply.text = format!("{}\n{}", self.source_heading(), reply.text);
    }

    // The provider process that verifies the answer need not have run its plan.
    // Re-resolve the selected column against this turn and its actual history.
    pub(crate) fn verified_excerpt(
        data: &Data, question: &str, history: &[(String, String)], facts: &str,
    ) -> Option<String> {
        let fields = [Self::Acquisition, Self::Work, Self::Passive, Self::Partner, Self::Habitat, Self::Facility];
        let (index, field) = facts.lines().enumerate().filter_map(|(i, line)| {
            fields.iter().find(|field| line.trim() == field.source_heading()).map(|field| (i, *field))
        }).last()?;
        let selected = facts.lines().skip(index + 1).collect::<Vec<_>>().join("\n");
        let record = model_field_evidence(data, field, question, history)?;
        if record.text.trim() != selected.trim() {
            return None;
        }
        // A fallback is a complete, concise record. Long lists stay in the sources panel.
        if record.text.chars().count() > 700 || record.text.lines().count() > 8 {
            return None;
        }
        Some(record.text)
    }

    pub(crate) fn partner_excerpt(data: &Data, question: &str, history: &[(String, String)], facts: &str) -> Option<String> {
        let raw = field_text(question);
        let prior = history.last().map(|(q, _)| field_text(q));
        let q = resolve_followup(data, &raw, prior.as_deref());
        let (mentions, _) = crate::assistant_breeding::species_mentions(&data.species, &q);
        let index = mentions.first()?.0;
        if mentions.iter().any(|m| m.0 != index) { return None; }
        let pal = &data.species[index];
        let zh = text(pal, "zh");
        let entry = &data.get("partner-skills-data")[text(pal, "key")];
        let name = entry["name"].as_str()?;
        let effect = entry["effect"].as_str()?.trim();
        if effect.is_empty() || !facts.contains(effect) || !facts.lines().any(|line|
            line.contains(zh) && line.contains(name) && line.contains(effect.lines().next().unwrap_or(effect))) {
            return None;
        }
        let has = |s: &str, terms: &[&str]| terms.iter().any(|term| s.contains(term));
        if has(&q, &["where can i catch", "where to catch", "where do i find", "where does", "where to farm",
            "how to breed", "breed into", "parent", "how to obtain", "怎么配", "怎么抓", "哪里抓", "在哪抓",
            "哪里找", "怎么获得", "配种", "遗传", "几级解锁", "配方", "recipe", "schematic"]) { return None; }
        let explicit = has(&q, &["partner skill", "partner ability", "伙伴技能", "伙伴能力"]);
        let with_player = has(&q, &["in my party", "in the party", "on my team", "in my team", "in your party",
            "alongside", "while riding", "when mounted", "随队", "队伍", "队里", "同行", "并肩", "骑乘", "带着", "带在"]);
        const GOALS: &[(&[&str], &[&str])] = &[
            (&["catch", "capture", "sphere", "捕捉", "捕获", "抓捕", "帕鲁球"], &["帕鲁球", "捕获", "捕捉"]),
            (&["carry weight", "carrying", "capacity", "负重"], &["负重"]),
            (&["glid", "fall damage", "if i fall", "滑翔", "摔伤", "坠落"], &["滑翔", "坠落"]),
            (&["heal", "recover health", "回血", "治疗", "恢复生命"], &["生命值", "恢复", "治疗"]),
            (&["mining", "harvest", "mine ore", "采矿", "采集"], &["采矿", "采集"]),
            (&["move faster", "movement speed", "run faster", "移速", "移动速度"], &["移动速度", "快速移动"]),
            (&["extra drops", "more loot", "掉落", "掉宝"], &["掉落"]),
            (&["double jump", "extra jump", "二段跳", "跳跃"], &["跳跃", "段跳"]),
        ];
        if !explicit && !(with_player && GOALS.iter().any(|(ask, record)| has(&q, ask) && has(effect, record))) {
            return None;
        }
        Some(format!("{zh}的伙伴技能「{name}」：{effect}"))
    }

    pub(crate) fn can_answer_directly(self, question: &str) -> bool {
        if matches!(self, Self::Habitat | Self::Facility) {
            // Both handlers return an answer only for a verified game-data query.
            return true;
        }
        if self != Self::Acquisition {
            return false;
        }
        let question = field_text(question);
        [
            "获取",
            "获得",
            "取得",
            "拿到",
            "哪刷",
            "怎么刷",
            "怎么弄",
            "途径",
            "来源",
            "掉落",
            "哪里买",
            "哪儿买",
            "obtain",
            "source",
            "where can",
            "where to",
            "way to get",
            "way to farm",
            "come by",
            "pick up",
            "acquire",
            "anywhere to buy",
            "where does",
            "how do i get",
        ]
        .iter()
        .any(|term| question.contains(term))
    }
}

/// Limit broad location and item lookups to an independently identifiable question.
pub(super) fn lookup_question(q: &str) -> &str {
    let Some((title, body)) = q.trim().split_once('\n') else { return q };
    let title = title.trim();
    let details = body.trim_start().to_ascii_lowercase();
    if details.is_empty() || title.is_empty() { return q; }
    if ["only ", "specifically ", "at night", "during the day", "excluding ", "under level ",
        "只看", "只想看", "只考虑", "仅限", "排除", "夜间", "只在", "不要列", "不考虑"].iter()
        .any(|prefix| details.starts_with(prefix)) {
        return q;
    }
    let lower = title.to_ascii_lowercase();
    let words: Vec<_> = lower.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).collect();
    // Short references such as "Where can I catch it?" need the species named
    // in the next line. A self-contained title already establishes its subject.
    let deictic = words.iter().any(|w| matches!(*w, "this"|"that"|"these"|"those"|"it"|"them"|"here"|"there"|"they"|"their"|"its"))
        || ["它", "这只", "那只", "这件", "那个", "这些", "这种", "这个", "如下", "以下"].iter().any(|w| title.contains(w));
    let self_contained = words.len() >= 4 || title.chars().filter(|c| ('\u{3400}'..='\u{9fff}').contains(c)).count() >= 6;
    let asks = title.ends_with(['?', '？']) || words.first().is_some_and(|w| matches!(*w,"where"|"how"|"what"|"which"|"why"|"does"|"can"|"are"|"is"))
        || ["怎么", "如何", "怎样", "什么", "哪里", "在哪", "哪种", "能否", "是否"].iter().any(|w| title.contains(w));
    if asks && self_contained && !deictic { title } else { q }
}

/// Prefer an explicit title when a forum post also mentions other mechanics in its body.
pub(super) fn main_field(q: &str) -> Option<HandbookField> {
    let (title, rest) = q.split_once(['?', '？', '\n'])?;
    if rest.trim().is_empty() {
        return None;
    }
    let has = |words: &[&str]| words.iter().any(|word| title.contains(word));
    if has(&["partner skill", "partner ability", "伙伴技能", "夥伴技能"]) {
        Some(HandbookField::Partner)
    } else if has(&["passive", "词条", "被动", "implant", "surgery"]) {
        Some(HandbookField::Passive)
    } else if has(&[
        "work speed",
        "work suitability",
        "working speed",
        "工作速度",
        "工作适性",
        "工作效率",
    ]) {
        Some(HandbookField::Work)
    } else if has(&[
        "how to get",
        "how do i get",
        "where can i get",
        "where to find",
        "where do i find",
        "how to farm",
        "way to farm",
        "哪里刷",
        "哪里掉",
        "如何获取",
        "怎么获取",
        "怎么获得",
    ]) {
        Some(HandbookField::Acquisition)
    } else {
        None
    }
}

fn subjects(data: &Data, question: &str) -> Vec<String> {
    relevant_names(array(data.get("search-index-data")), question)
        .into_iter()
        .filter(|row| matches!(text(row, "kind"), "物品" | "帕鲁" | "伙伴技能" | "被动词条"))
        .map(|row| field_text(text(row, "name")))
        .collect()
}

pub(super) fn resolve_followup(data: &Data, question: &str, previous: Option<&str>) -> String {
    let Some(previous) = previous else {
        return question.to_owned();
    };
    let short_refinement = question.chars().count() <= 65
        && [
            "只想",
            "只看",
            "只列",
            "只买",
            "那只",
            "那买",
            "呢",
            "only ",
            "instead",
            "how about",
            "what about",
        ]
        .iter()
        .any(|word| question.contains(word));
    let channel = if short_refinement {
        [
            (
                "商人购买",
                &["商店", "商人", "买", "shop", "merchant", "buy"][..],
            ),
            ("牧场产出", &["牧场", "放牧", "ranch"][..]),
            ("远征获取", &["远征", "expedition"][..]),
            (
                "战斗掉落",
                &["战斗", "击败", "掉落", "combat", "battle drop"][..],
            ),
        ]
        .into_iter()
        .find_map(|(label, terms)| terms.iter().any(|w| question.contains(w)).then_some(label))
    } else {
        None
    };
    let words = question.split(|c: char| !c.is_ascii_alphanumeric());
    let reference = words
        .into_iter()
        .any(|w| matches!(w, "it" | "its" | "them" | "their" | "those"))
        || [
            "它",
            "那只",
            "这只",
            "那两只",
            "这个",
            "那它",
            "掉率呢",
            "地点呢",
            "伙伴技能呢",
            "适性呢",
            "what about",
            "how about",
            "其他来源",
            "别的来源",
            "其他途径",
            "别的途径",
            "还有哪些途径",
            "有卖吗",
            "哪里买",
            "还有谁掉",
            "牧场呢",
            "商店呢",
            "other sources",
            "any other way",
        ]
        .iter()
        .any(|w| question.contains(w));
    if (!reference && channel.is_none()) || !subjects(data, question).is_empty() {
        return question.to_owned();
    }
    let names = subjects(data, previous);
    if names.is_empty() || names.len() > 2 {
        return question.to_owned();
    }
    if let Some(channel) = channel
        && names.len() == 1
        && array(&data.get("item-catalog-data")["items"])
            .iter()
            .any(|item| text(item, "name") == names[0])
    {
        let excluding = [
            "不要",
            "不想",
            "不买",
            "不用",
            "不靠",
            "除了",
            "排除",
            "without",
            "except",
            "apart from",
            "anything but",
        ]
        .iter()
        .any(|term| question.contains(term));
        return format!(
            "{} {} {}",
            names[0],
            if excluding { "获取途径" } else { channel },
            question
        );
    }
    format!("{} {}", names.join("、"), question)
}

/// The planner selects a column. Subject names come from the player and the local index.
/// Return facts for synthesis only when an actual handbook record is available.
pub(crate) fn model_field_evidence(
    data: &Data,
    field: HandbookField,
    question: &str,
    history: &[(String, String)],
) -> Option<AssistantReply> {
    let prior = history.last().map(|(q, _)| field_text(q));
    let current = field_text(question);
    let q = if matches!(field, HandbookField::Habitat | HandbookField::Facility) {
        current
    } else { resolve_followup(data, &current, prior.as_deref()) };
    // The item catalog resolves plural aliases and schematic grades itself.
    // The generic index only lists some word orders for those items.
    if matches!(field, HandbookField::Work | HandbookField::Passive | HandbookField::Partner) && subjects(data, &q).is_empty() {
        return None;
    }
    let reply = match field {
        HandbookField::Acquisition => acquisition::answer(data, &format!("{q} 获取途径")),
        HandbookField::Work => work_query::respond(&format!("{q} 工作适性"), data),
        HandbookField::Passive | HandbookField::Partner => {
            let label = if field == HandbookField::Passive {
                "被动词条"
            } else {
                "伙伴技能"
            };
            skill_query::SkillSession::default().respond(&format!("{q} {label}"), data)
        }
        HandbookField::Facility => {
            let mut facility = facility_effect::FacilitySession::default();
            if let Some(previous) = prior.as_deref() { facility.respond(data, previous); }
            facility.respond(data, &q)
        }
        HandbookField::Habitat => {
            let mut session = habitat::HabitatSession::default();
            if let Some(previous) = prior.as_deref() { session.respond(data, previous); }
            session.respond(data, &q).or_else(|| {
                // Expand implicit location requests without inventing a target from model terms.
                if ["想去抓", "想在野外找", "野外寻", "蹲一只", "tracking down", "track down",
                    "trying to find", "looking to catch", "looking for a wild"].iter().any(|word| q.contains(word)) {
                    session.respond(data, &format!("{q} 野外出没地点"))
                } else { None }
            })
        }
    }?;
    (!reply.links.is_empty()).then_some(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incidental_equipment_in_a_post_body_does_not_replace_its_subject() {
        let question = "Where should I look for rare fish?\nI already own deluxe bait.";
        assert_eq!(lookup_question(question), "Where should I look for rare fish?");
        let steps = "What to improve before hard towers\nA guide explained where to find my Pal.";
        assert_eq!(lookup_question(steps), "What to improve before hard towers");
        for q in ["Where can I find it?\nThe item is Pal Oil.", "My experience with early fishing\nWhere can I find the bait?",
            "Where can I find Anubis?\nOnly show nighttime spawns."] {
            assert_eq!(lookup_question(q), q);
        }
    }
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn focused_evidence_uses_player_entities_and_only_the_requested_field() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "search-index-data",
            "spawn-data",
            "map-data",
            "expedition-data",
            "item-catalog-data",
            "enemy-data",
            "paldex-data",
            "partner-skills-data",
            "ranch-data",
            "resource-guides-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            data.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let history = vec![("先看唤冬兽".to_owned(), "随意提到了八云犬。".to_owned())];
        let reply =
            model_field_evidence(&data, HandbookField::Partner, "那它呢？", &history).unwrap();
        assert!(
            reply
                .links
                .iter()
                .any(|(_, target)| target == "#details/partner/IceHorse")
        );
        assert!(!reply.text.contains("八云犬"));
        let mut labelled = reply.clone();
        HandbookField::Partner.label_evidence(&mut labelled);
        let facts = format!("其他检索记录\n{}", labelled.text);
        assert_eq!(HandbookField::verified_excerpt(&data, "那它呢？", &history, &facts).unwrap(), reply.text);
        assert!(HandbookField::verified_excerpt(&data, "那它呢？", &[], &facts).is_none());
        let other = vec![("我想了解八云犬".to_owned(), String::new())];
        assert!(HandbookField::verified_excerpt(&data, "那它呢？", &other, &facts).is_none());
        assert!(HandbookField::verified_excerpt(&data, "那它呢？", &history, &format!("{facts}伪造条件")).is_none());
        let skill = &data.get("partner-skills-data")["Mutant"];
        let effect = text(skill, "effect");
        let species_row = format!("秘斯媞雅（帕鲁）：伙伴技能「{}」：{effect}", text(skill, "name"));
        let party = "Does a Lunaris in my party help my catch attempts?";
        assert!(HandbookField::partner_excerpt(&data, party, &[], &species_row).is_some());
        assert!(HandbookField::partner_excerpt(&data, party, &[], "秘斯媞雅，伙伴技能反重力").is_none());
        assert!(HandbookField::partner_excerpt(&data, "Where can I catch Lunaris for my party?", &[], &species_row).is_none());
        assert!(HandbookField::partner_excerpt(&data, "Does a Lunaris in my party help improve mining?", &[], &species_row).is_none());
        assert!(HandbookField::partner_excerpt(&data, "Does a Lunaris or Catbat in my party help with spheres?", &[], &species_row).is_none());
        assert!(model_field_evidence(&data, HandbookField::Partner, "那它呢？", &[]).is_none());
        let wild = model_field_evidence(&data, HandbookField::Habitat, "I'm looking to catch a wild Jolthog.", &[]).unwrap();
        assert!(wild.links.iter().any(|(_, route)| route.starts_with("#map/coords/")));
        assert!(model_field_evidence(&data, HandbookField::Habitat, "What are good defense traits?", &[]).is_none());
        let reply = model_field_evidence(
            &data,
            HandbookField::Acquisition,
            "I'm after an Ancient Civilization Core.",
            &history,
        )
        .unwrap();
        assert!(
            reply
                .links
                .iter()
                .any(|(_, target)| target.starts_with("#items/AncientParts2"))
        );
        let mut chat = crate::assistant_engine::Conversation::default();
        let world = serde_json::json!({"id":"source-followup"});
        for (q, expected) in [
            ("优质帕鲁油的获取途径有哪些？", "趴趴鲶"),
            ("只看牧场呢？", "梆梆鲶"),
            ("那只想买呢？", "300"),
            ("还有其他来源吗？", "趴趴鲶"),
        ] {
            let answer = chat.ask_world(q, &data, &world);
            assert!(
                chat.direct_result() && answer.text.contains(expected),
                "{q}: {}",
                answer.text
            );
        }
        let blank = chat.ask_world("那只想买呢？", &data, &serde_json::json!({"id":"other"}));
        assert!(!blank.text.contains("优质帕鲁油"));
        let world = serde_json::json!({"id":"schematic"});
        let general = chat.ask_world("想找火属性防具的设计图，去哪里拿？", &data, &world);
        assert!(chat.direct_result() && general.text.contains("哪件装备"), "{}", general.text);
        let named = chat.ask_world("机械弓设计图4", &data, &world);
        assert!(chat.direct_result() && named.links.iter().any(|(_, href)| href.starts_with("#items/")), "{}", named.text);
    }
}
