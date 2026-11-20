//! Offline gameplay guides. Aliases in each group describe the same concept; a
//! topic must match every required concept before its guidance can be returned.
use super::AssistantReply;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Guide {
    topic: String,
    groups: Vec<Vec<String>>,
    // The subject may be established in a preceding sentence. In that case the
    // actual question must still ask for a relationship this guide answers.
    #[serde(default)]
    followup_groups: Vec<Vec<String>>,
    #[serde(default)]
    exclude: Vec<String>,
    zh: String,
    en: String,
    source: String,
    #[serde(default)]
    also: Vec<String>,
    #[serde(default)]
    route: String,
}

fn book() -> &'static [Guide] {
    static BOOK: OnceLock<Vec<Guide>> = OnceLock::new();
    BOOK.get_or_init(|| {
        serde_json::from_str(include_str!("mechanics.json")).expect("bundled mechanics guides")
    })
}

fn contains(q: &str, term: &str) -> bool {
    if !term.is_ascii() {
        return q.contains(term);
    }
    q.match_indices(term).any(|(at, _)| {
        let end = at + term.len();
        !q[..at].ends_with(|c: char| c.is_ascii_alphanumeric())
            && (!q[end..].starts_with(|c: char| c.is_ascii_alphanumeric())
                || q[end..]
                    .strip_prefix('s')
                    .is_some_and(|s| !s.starts_with(|c: char| c.is_ascii_alphanumeric())))
    })
}

fn score_groups(groups: &[Vec<String>], question: &str) -> Option<usize> {
    if groups.is_empty() { return None; }
    groups.iter().try_fold(0, |score, group| {
        let longest = group.iter().filter(|term| contains(question, term))
            .map(|term| term.chars().count()).max()?;
        Some(score + longest + 2)
    })
}

pub(super) fn answer(q: &str) -> Option<AssistantReply> {
    // The guidebook documents standard 1.0 mechanics. A mod or an explicitly
    // older ruleset needs its own evidence before making a definitive claim.
    if !crate::assistant_discovery::handbook_version_matches(q) || [
        "with a mod",
        "with mods",
        "modded",
        "which mod",
        "what mod",
        "any mod",
        "using a mod",
        "early access version",
        "old version",
        "before 1.0",
        "version 0.",
        "v0.",
        "旧版本",
        "旧版的",
        "老版本",
        "抢先体验版",
        "装了模组",
        "装了mod",
        "有mod",
        "修改器",
        "模组服",
    ]
    .iter()
    .any(|term| {
        if *term == "version 0." || *term == "v0." {
            q.contains(term)
        } else {
            contains(q, term)
        }
    }) {
        return None;
    }
    let mut answers = Vec::new();
    let mut links = Vec::new();
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let (title, context) = q.split_once('\n').unwrap_or((q, ""));
    // The UI accepts free-form text. A community post may also have a short subject
    // and a separate body. Use the body to disambiguate an actual subject, never to
    // replace an unrelated title with one of the player's background examples.
    let has_question = title.contains(['?', '？']);
    for clause in title.split_inclusive(['?', '？', '。', '.', ';', '；']) {
        if has_question && !clause.ends_with(['?', '？']) {
            continue;
        }
        // "I know X, but how do I do Y?" asks about Y. Matching X in the
        // discarded premise used to return a confident but unrelated answer.
        let focus = [
            " but ",
            " instead i want to know ",
            " what i want to know is ",
            "但是",
            "但我想问",
            "不过我想问",
            "我想知道的是",
            "我问的是",
        ]
        .iter()
        .filter_map(|marker| clause.rfind(marker).map(|pos| pos + marker.len()))
        .max()
        .map_or(clause, |start| &clause[start..]);
        let guide = book()
            .iter()
            .filter_map(|g| {
                if g.exclude.iter().any(|term| contains(q, term)) {
                    return None;
                }
                let score = score_groups(&g.groups, focus).or_else(|| {
                    if focus == clause && score_groups(&g.followup_groups, focus).is_some() {
                        return score_groups(&g.groups, q);
                    }
                    if context.is_empty()
                        || focus != clause
                        || !g.groups.first()?.iter().any(|term| contains(clause, term))
                    {
                        return None;
                    }
                    score_groups(&g.groups, q)
                })?;
                Some((score, g))
            })
            .max_by_key(|(score, _)| *score);
        let Some((_, guide)) = guide else {
            // A compound question with an unsupported request needs the normal
            // reasoning path; one canned paragraph would appear to answer it all.
            if has_question && clause.chars().filter(|ch| ch.is_alphanumeric()).count() >= 3 {
                return None;
            }
            continue;
        };
        let text = if english { &guide.en } else { &guide.zh };
        if !answers.contains(&text.as_str()) {
            answers.push(text.as_str());
            if !guide.route.is_empty() && !links.iter().any(|(_, url)| url == &guide.route) {
                links.push((guide.topic.clone(), guide.route.clone()));
            }
            for source in std::iter::once(&guide.source).chain(&guide.also) {
                if !links.iter().any(|(_, url)| url == source) {
                    links.push((format!("{} · {}", if english {"Source"} else {"来源"}, guide.topic), source.clone()));
                }
            }
        }
    }
    if answers.is_empty() {
        return None;
    }
    Some(AssistantReply {
        text: answers.join("\n\n"),
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topics_require_all_concepts_and_preserve_source_details() {
        for g in book() {
            assert!(!g.groups.is_empty() && g.groups.iter().all(|parts| !parts.is_empty()));
            assert!(g.source.starts_with("https://") && !g.zh.is_empty() && !g.en.is_empty());
        }
        assert!(answer("what is the password for my email account").is_none());
        assert!(answer("which pal is fastest for travel").is_none());
        assert!(answer("0.6 版里，从据点能打开地图直接传送吗？").is_none());
        assert!(answer("in version 2.0 can i fast travel from my base?").is_none());
        assert!(answer("1.0 版之前，在基地里面能直接传送吗？").is_none());
        assert!(
            answer("can i fast travel directly to my base")
                .unwrap()
                .text
                .contains("Palbox")
        );
        assert!(
            answer("怎样给专服设置加入密码")
                .unwrap()
                .text
                .contains("ServerPassword")
        );
    }
    #[test]
    fn requires_a_matching_request_and_keeps_statements_out_of_question_routing() {
        assert!(answer("can a breeding farm be placed next to a stone pit?").is_none());
        assert!(answer("i know what ivs mean. where can i find a good mount?").is_none());
        assert!(answer("where can i get a yakumo saddle?").is_none());
        assert!(
            answer("how many lifmunk effigies are needed for the catch rate upgrade?")
                .unwrap()
                .text
                .contains("100")
        );
        assert!(
            answer("怎么让运输帕鲁把采石场的石头搬走？")
                .unwrap()
                .text
                .contains("储物箱")
        );
        assert!(
            answer("can i copy a mutation trait when catching with yakumo?")
                .unwrap()
                .text
                .contains("excluded")
        );
        assert!(answer("what rewards do i get?\ni killed a hard mode tower boss and need more technology points").is_none());
        assert!(
            answer("hard tower rematch?\nany more technology points when i clear it?")
                .unwrap()
                .text
                .contains("zero")
        );
        assert!(answer("据点传送怎么用？钢铁怎么获得？").is_none());
        assert!(
            answer("每条配种路线都必经同一个帕鲁吗？")
                .unwrap()
                .text
                .contains("空涡龙")
        );
        assert!(
            answer("when will a traveling merchant visit my base?")
                .unwrap()
                .text
                .contains("Small Settlement")
        );
        assert!(
            answer("incident with a wandering merchant?")
                .unwrap()
                .text
                .contains("road")
        );
    }

    #[test]
    fn exceptions_and_background_do_not_receive_standard_world_advice() {
        for q in [
            "what mod lets me fast travel to my base from any cave?",
            "v0.4 捕获力升级要多少个翠叶鼠雕像？",
            "my base has fast travel but where can i find an oil rig?",
            "野外头目会重生，但是团体战石板怎么做？",
            "when i was fighting blazamut, the chest disappeared. when will the chest respawn?",
            "is the blazamut ryu respawn timer tied to dawn?",
            "does a sealed realm boss respawn at the same time as an overworld boss?",
            "how do i get more coins for visiting wandering merchants?",
            "where can i capture yakumo to pass a mutation trait?",
            "what does an iv fruit do?",
            "what is the server password for my friend's dedicated server?",
            "why is my server password rejected?",
        ] {
            assert!(answer(q).is_none(), "{q}");
        }
    }
}
