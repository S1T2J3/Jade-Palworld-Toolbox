use super::AssistantReply;
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, text};
use serde_json::Value;
mod individual;
pub(super) use individual::individual_job;

// The first two entries are the handbook key and the English display name.
pub(crate) const JOBS: &[(&str, &str, &[&str])] = &[
    (
        "生火",
        "Kindling",
        &["kindling", "smelting", "生火", "烧炉", "冶炼"],
    ),
    ("浇水", "Watering", &["watering", "浇水", "灌溉"]),
    ("播种", "Planting", &["planting", "播种"]),
    (
        "发电",
        "Generating Electricity",
        &["generating electricity", "electricity generation", "发电"],
    ),
    ("手工作业", "Handiwork", &["handiwork", "手工"]),
    ("采集", "Gathering", &["gathering", "采集"]),
    (
        "伐木",
        "Lumbering",
        &["lumbering", "logging", "伐木", "砍树"],
    ),
    ("采矿", "Mining", &["mining", "采矿", "挖矿"]),
    (
        "制药",
        "Medicine Production",
        &["medicine production", "制药"],
    ),
    (
        "冷却",
        "Cooling",
        &["cooling", "refrigeration", "冷却", "冷藏"],
    ),
    (
        "搬运",
        "Transporting",
        &["transporting", "hauling", "carrying capacity", "搬运"],
    ),
    (
        "牧场",
        "Farming",
        &[
            "ranching",
            "ranch work",
            "farming suitability",
            "牧场",
            "放牧",
        ],
    ),
];

// Keep Latin names and job words whole, even next to Chinese text.
fn whole(q: &str, start: usize, end: usize) -> bool {
    let b = q.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    !(start > 0 && word(b[start]) && word(b[start - 1])
        || end < b.len() && word(b[end - 1]) && word(b[end]))
}

fn mentions(q: &str, words: &[&str]) -> bool {
    words
        .iter()
        .any(|&word| q.match_indices(word).any(|(s, w)| whole(q, s, s + w.len())))
}

/// An omitted subject is safe to restore when the remaining words select a
/// job. A new question about a mine, pickaxe or material keeps its own scope.
pub(super) fn job_refinement(q: &str) -> bool {
    let mut subject = q.trim_matches(|c: char| c.is_whitespace() || "。？！？，,.?!".contains(c));
    for prefixes in [
        &["要是", "那如果", "如果", "那么", "那", "and ", "what about ", "how about "][..],
        &["只想", "只", "仅", "just ", "only "],
        &["比较一下", "比较", "考虑", "看", "比", "按", "compare "],
    ] {
        if let Some(rest) = prefixes.iter().find_map(|prefix| subject.strip_prefix(prefix)) {
            subject = rest.trim();
        }
    }
    let subject = subject.trim_end_matches('呢').trim();
    JOBS.iter().any(|(_, _, terms)| terms.contains(&subject))
}

struct Pal<'a> {
    key: &'a str,
    name: &'a str,
    work: &'a [Value],
}

impl Pal<'_> {
    fn rank(&self, job: &str) -> Option<usize> {
        let record = self.work.iter().find(|entry| entry[0] == job)?;
        usize::try_from(record[1].as_u64()?).ok().filter(|n| *n > 0)
    }
}

fn quantity_label(job: &str, english: bool) -> &'static str {
    match (job, english) {
        ("搬运", true) => "items per trip",
        ("搬运", false) => "单趟搬运上限（件）",
        ("牧场", true) => "base ranch parameter",
        ("牧场", false) => "牧场等级基础参数",
        (_, true) => "standard work amount",
        (_, false) => "标准工作量",
    }
}

fn states_modified_rank(q: &str, job_terms: &[&str]) -> bool {
    job_terms
        .iter()
        .chain(["work suitability", "适性", "适应性"].iter())
        .any(|&term| {
            q.match_indices(term).any(|(start, _)| {
                let before = q[..start].trim_end();
                let after = q[start + term.len()..].trim_start_matches([' ', ':', '：']);
                let numeric = |s: &str| {
                    s.trim_start_matches([' ', '.'])
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit() || "一二三四五六七八九十".contains(c))
                };
                ["lvl", "lv", "level", "rank", "等级"]
                    .iter()
                    .any(|&tag| after.strip_prefix(tag).is_some_and(numeric))
                    || numeric(after)
                    || before.strip_suffix('级').is_some_and(|s| {
                        s.chars().next_back().is_some_and(|c| {
                            c.is_ascii_digit() || "一二三四五六七八九十".contains(c)
                        })
                    })
            })
        })
}

/// Compare named species' natural suitability at equal modifiers. `q` is field-normalized.
pub(super) fn respond(q: &str, data: &Data) -> Option<AssistantReply> {
    let general = mentions(
        q,
        &[
            "work speed",
            "work speeds",
            "working speed",
            "work efficiency",
            "working efficiency",
            "work suitability",
            "work suitabilities",
            "work faster",
            "works faster",
            "base worker",
            "better worker",
            "工作效率",
            "工作速度",
            "工作适性",
            "工作适应性",
            "工作能力",
            "打工",
            "干活",
        ],
    );
    let requested: Vec<_> = JOBS
        .iter()
        .filter(|(_, _, terms)| mentions(q, terms))
        .collect();
    if !general && requested.is_empty() {
        return None;
    }
    // Named entities and job words also appear in breeding, combat and optimization questions.
    // A species comparison cannot answer those from base suitability alone.
    if requested
        .iter()
        .any(|(_, _, terms)| states_modified_rank(q, terms))
        || mentions(
            q,
            &[
                "breed",
                "breeding",
                "offspring",
                "passive",
                "passives",
                "condensed",
                "suitability manual",
                "attack",
                "damage",
                "combat",
                "cooldown",
                "cooldowns",
                "per hour",
                "drop rate",
                "drops",
                "how to increase",
                "how to raise",
                "won't",
                "won’t",
                "stopped working",
                "refuses",
                "配种",
                "繁殖",
                "后代",
                "词条",
                "浓缩",
                "手册",
                "伤害",
                "攻击",
                "战斗",
                "冷却时间",
                "如何提升",
                "怎么提高",
                "不工作",
                "不干活",
                "每小时",
                "掉率",
                "配方",
            ],
        )
        || q.contains('%')
    {
        return None;
    }
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let profiles = data.get("paldex-index-data")["profiles"].as_object()?;
    let efficiency = &data.get("work-efficiency-data")["levels"];
    let mut pals: Vec<Pal<'_>> = Vec::new();
    for (index, _, _) in species_mentions(&data.species, q).0 {
        let species = &data.species[index];
        let key = text(species, "key");
        if pals.iter().any(|p| p.key == key) {
            continue;
        }
        let work = profiles.get(key)?["work"].as_array()?;
        if work
            .iter()
            .any(|r| r[0].as_str().is_none() || r[1].as_u64().is_none_or(|n| n == 0))
        {
            return None;
        }
        pals.push(Pal {
            key,
            name: text(species, if english { "en" } else { "zh" }),
            work,
        });
    }
    if pals.len() < 2 {
        return None;
    }
    let selected = if requested.is_empty() {
        // Compare every shared task. With no common task, show the actual division of labor.
        let shared: Vec<_> = JOBS
            .iter()
            .filter(|(key, _, _)| pals.iter().all(|p| p.rank(key).is_some()))
            .collect();
        if shared.is_empty() {
            JOBS.iter()
                .filter(|(key, _, _)| pals.iter().any(|p| p.rank(key).is_some()))
                .collect()
        } else {
            shared
        }
    } else {
        requested
    };
    if selected.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for &(job, label, _) in selected {
        let curve = efficiency[job].as_array()?;
        let quantity = quantity_label(job, english);
        let mut entries = Vec::new();
        let mut values = Vec::new();
        for p in &pals {
            let name = p.name;
            let item = if let Some(rank) = p.rank(job) {
                let value = curve.get(rank - 1)?.as_u64()?;
                values.push((name, value));
                format!("{name} Lv.{rank} ({quantity} {value})")
            } else if english {
                format!("{name} has no {label} suitability")
            } else {
                format!("{name}没有{job}适性")
            };
            entries.push(item);
        }
        let mut line = format!(
            "{} — {}",
            if english { label } else { job },
            entries.join(" / ")
        );
        if pals.len() == 2
            && let [(first, a), (second, b)] = values[..]
        {
            if a == b {
                line.push_str(if english {
                    ". Both have the same base value."
                } else {
                    "，两者基础值相同。"
                });
            } else {
                let (higher, high, low) = if a > b { (first, a, b) } else { (second, b, a) };
                if low > 0 {
                    let ratio = format!("{:.2}", high as f64 / low as f64);
                    let ratio = ratio.trim_end_matches('0').trim_end_matches('.');
                    line.push_str(&if english {
                        format!(
                            ". The base value for {higher} is approximately {ratio} times as high."
                        )
                    } else {
                        format!("，{higher}的这项基础值约为另一只的 {ratio} 倍。")
                    });
                }
            }
        }
        lines.push(line);
    }
    let note = if english {
        "These are the species’ natural work suitability levels. Pals at the same character level can have different suitability for the same job. The figures use base values before individual and base bonuses."
    } else {
        "以上按物种的基础工作适性比较。帕鲁等级相同，同一岗位的适性等级仍可不同。表中基础值未计入个体与据点加成。"
    };
    lines.push(note.to_owned());
    Some(AssistantReply {
        text: lines.join("\n"),
        links: pals
            .iter()
            .map(|p| (p.name.to_owned(), format!("#work/suitability/{}", p.key)))
            .collect(),
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    fn data() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["paldex-index-data", "work-efficiency-data", "partner-skills-data"] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            d.tables.insert(
                name.to_owned(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        d
    }

    #[test]
    fn individual_suitability_and_partner_skills_keep_distinct_conditions() {
        let d = data();
        for (q, facts) in [
            ("Can Mammorest chop wood?", &["Lumbering Lv.4", "While mounted", "220–500%"][..]),
            ("碎岩龟适合采矿吗？", &["采矿适性为 Lv.4", "甲壳回旋", "800~2000"][..]),
            ("Can Anubis generate electricity?", &["no natural Generating Electricity"][..]),
            ("Can Chillet do cooling and gathering?", &["Cooling Lv.2", "Gathering Lv.1"][..]),
            ("哪只帕鲁浇水适性最高？", &["最高基础适性", "浇水 Lv."][..]),
        ] {
            let mut c = super::super::Conversation::default();
            let a = c.ask_world(q, &d, &serde_json::json!({"id":"work"}));
            assert!(c.direct_result() && facts.iter().all(|f| a.text.contains(f)), "{q} -> {}", a.text);
        }
        for q in ["why won't anubis do mining?", "who can breed digtoise for mining?", "where can i find a digtoise for mining?", "is anubis better than cattiva at mining?", "疾旋鼬的招式冷却时间是多少？"] {
            assert!(individual_job(&d, q).is_none(), "{q}");
        }
    }

    #[test]
    fn compares_named_forms_using_real_records_and_units() {
        let d = data();
        let ask = |s: &str| {
            let mut conv = super::super::Conversation::default();
            let reply = conv.ask_world(s, &d, &serde_json::json!({"id":"work"}));
            assert!(conv.direct_result(), "{s}: {}", reply.text);
            reply.text
        };
        let q = ask("Fuack Ignis and Fuack are both level 12. Which has better watering?");
        assert!(
            q.contains("Fuack Ignis Lv.2 (standard work amount 70)")
                && q.contains("Fuack Lv.1 (standard work amount 50)"),
            "{q}"
        );
        let q = ask("企丸王和捣蛋猫比较搬运效率");
        assert!(
            q.contains("单趟搬运上限（件） 10") && q.contains("单趟搬运上限（件） 2"),
            "{q}"
        );
        let q = ask("碧海龍和企丸王的澆水適性哪個好？");
        assert!(
            q.contains("碧海龙 Lv.4") && q.contains("企丸王 Lv.2"),
            "{q}"
        );
    }

    #[test]
    fn narrowing_a_work_comparison_reuses_the_same_pals_and_curve() {
        let d = data();
        let world = serde_json::json!({"id":"mine"});
        let mut conv = super::super::Conversation::default();
        let first = conv.ask_world("企丸王和捣蛋猫的工作能力相比如何？", &d, &world);
        let next = conv.ask_world("要是只比较采矿呢？", &d, &world);
        assert!(conv.direct_result() && next.links == first.links);
        assert!(next.text.contains("企丸王 Lv.3 (标准工作量 100)") && next.text.contains("捣蛋猫 Lv.1 (标准工作量 50)"));
        assert!(!next.text.contains("搬运") && !next.text.contains("十字镐"));
        let again = conv.ask_world("只看搬运", &d, &world);
        assert!(conv.direct_result() && again.text.contains("单趟搬运上限") && !again.text.contains("采矿"));
        assert!(!job_refinement("想知道这把采矿镐哪里可以获取？"));
        assert!(job_refinement("what about mining?"));
    }

    #[test]
    fn avoids_unrelated_goals_and_false_species_matches() {
        let d = data();
        for q in [
            "cattivarius and lamball work speed?",
            "fuack ignis or fuack for combat?",
            "how do i breed fuack and azurobe for watering?",
            "how much damage do fuack and azurobe do?",
            "compare fuack with watering level 5 against azurobe at watering 4",
        ] {
            assert!(respond(q, &d).is_none(), "{q}");
        }
        let q = respond("azurobe and azurobe cryst work speed", &d)
            .unwrap()
            .text;
        assert!(
            q.contains("Azurobe Cryst has no Watering") && q.contains("Azurobe has no Cooling"),
            "{q}"
        );
    }
}
