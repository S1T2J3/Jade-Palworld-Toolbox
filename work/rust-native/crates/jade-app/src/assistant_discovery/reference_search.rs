//! Lexical passage retrieval over attributed article prose, separate from exact game data.
use super::AssistantReply;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Passage {
    topic: String,
    section: String,
    #[serde(default)]
    tags: String,
    text: String,
    source: String,
}

struct Document {
    passage: Passage,
    words: HashMap<String, u16>,
    length: f64,
}

struct Index {
    docs: Vec<Document>,
    frequency: HashMap<String, usize>,
    average: f64,
}

pub(super) fn tokens(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| s.len() > 2 && s.bytes().any(|c| c.is_ascii_alphabetic()))
        .filter_map(|word| {
            let word = word.to_ascii_lowercase();
            if matches!(
                word.as_str(),
                "the" | "play" | "playing" | "keep" | "running" | "world" | "someone" | "something" | "thing" | "really" | "actually" | "question"
                    | "and"
                    | "for"
                    | "are"
                    | "can"
                    | "use"
                    | "one"
                    | "how"
                    | "why"
                    | "you"
                    | "your"
                    | "was"
                    | "were"
                    | "with"
                    | "without"
                    | "have"
                    | "has"
                    | "from"
                    | "that"
                    | "this"
                    | "those"
                    | "does"
                    | "did"
                    | "which"
                    | "what"
                    | "where"
                    | "best"
                    | "good"
                    | "should"
                    | "could"
                    | "would"
                    | "get"
                    | "getting"
                    | "there"
                    | "pal"
                    | "pals"
                    | "palworld"
                    | "game"
                    | "way"
                    | "they"
                    | "about"
                    | "also"
                    | "when"
                    | "only"
                    | "but"
                    | "any"
                    | "them"
                    | "some"
                    | "like"
                    | "want"
                    | "need"
                    | "much"
                    | "been"
                    | "just"
                    | "anyone"
                    | "new"
                    | "than"
                    | "into"
                    | "other"
                    | "all"
                    | "not"
                    | "here"
                    | "being"
                    | "more"
                    | "help"
            ) {
                return None;
            }
            let stem = if word.len() > 5 && word.ends_with("ies") {
                format!("{}y", &word[..word.len() - 3])
            } else if word.len() > 4 && !word.ends_with("ss") && word.ends_with('s') {
                word[..word.len() - 1].to_owned()
            } else {
                word
            };
            Some(stem)
        })
}

fn index() -> &'static Index {
    static CORPUS: OnceLock<Index> = OnceLock::new();
    CORPUS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Book {
            passages: Vec<Passage>,
        }
        let book: Book =
            serde_json::from_str(include_str!("../../../../data/assistant-articles.json"))
                .expect("validated community guide prose");
        let mut docs = Vec::new();
        let mut frequency = HashMap::new();
        for passage in book.passages {
            let mut words: HashMap<String, u16> = HashMap::new();
            for token in tokens(&passage.text)
                .chain(tokens(&passage.section))
                .chain(tokens(&passage.topic))
            {
                *words.entry(token).or_default() += 1;
            }
            for token in words.keys() {
                *frequency.entry(token.clone()).or_default() += 1;
            }
            let length = words.values().map(|&n| f64::from(n)).sum();
            docs.push(Document {
                passage,
                words,
                length,
            });
        }
        let average = docs.iter().map(|d| d.length).sum::<f64>() / docs.len().max(1) as f64;
        Index {
            docs,
            frequency,
            average,
        }
    })
}

// Local terminology shared by Chinese player questions and English reference prose.
pub(super) fn passages() -> impl Iterator<Item = (&'static str, &'static str, &'static str, &'static str, &'static str)> {
    index().docs.iter().map(|doc| {
        let p = &doc.passage;
        (p.topic.as_str(), p.section.as_str(), p.tags.as_str(), p.text.as_str(), p.source.as_str())
    })
}

const VOCAB: &[(&str, &str)] = &[
    ("据点", "base"),
    ("基地", "base"),
    ("工作", "work"),
    ("适性", "suitability"),
    ("工人", "worker"),
    ("负重", "weight"),
    ("词条", "passive"),
    ("被动", "passive"),
    ("绕过", "avoid"),
    ("躲避", "avoid"),
    ("疾病", "illness"),
    ("生病", "sickness"),
    ("建筑", "structure"),
    ("建造", "build"),
    ("损坏", "damage"),
    ("耐久", "durability"),
    ("修理", "repair"),
    ("套装", "kit"),
    ("商人", "merchant"),
    ("技能", "skill"),
    ("招式", "active skill"),
    ("遗传", "inheritance breeding"),
    ("继承", "inheritance"),
    ("双亲", "parent"),
    ("亲本", "parent"),
    ("配种", "breeding"),
    ("繁殖", "breeding"),
    ("孵化", "incubation"),
    ("蛋", "egg"),
    ("头目", "alpha boss"),
    ("首领", "boss"),
    ("洞窟", "dungeon"),
    ("地牢", "dungeon"),
    ("地城", "dungeon"),
    ("团体战", "raid"),
    ("刷新", "respawn"),
    ("传送", "fast travel"),
    ("夜晚", "night"),
    ("寒冷", "cold"),
    ("保暖", "cold temperature"),
    ("火把", "torch"),
    ("耐热", "heat resistance"),
    ("耐寒", "cold resistance"),
    ("降低", "reduce"),
    ("掉落", "drop"),
    ("概率", "chance"),
    ("捕捉", "capture"),
    ("捕获", "capture"),
    ("帕鲁球", "sphere"),
    ("远征", "expedition"),
    ("搬运", "transporting"),
    ("电力", "electricity"),
    ("发电", "electricity generation"),
    ("种植", "plantation"),
    ("食物", "food"),
    ("喂食", "feed"),
    ("腐坏", "spoilage"),
    ("冰箱", "refrigerator"),
    ("公会", "guild"),
    ("储存", "storage"),
    ("箱子", "chest"),
    ("濒死", "incapacitated"),
    ("复活", "revive"),
    ("飞行", "flying mount"),
    ("坐骑", "mount"),
    ("鞍", "saddle"),
    ("滑翔", "glider"),
    ("濃縮", "condensation"),
    ("浓缩", "condensation"),
    ("升星", "condensation rank"),
    ("出生", "spawn"),
    ("重生", "respawn"),
    ("克制", "element weakness"),
    ("元素", "element"),
    ("护盾", "shield"),
    ("甲", "armor"),
    ("高级", "level"),
    ("古代科技", "ancient technology"),
    ("经验", "experience"),
    ("钓鱼", "fishing"),
    ("生命", "health"),
    ("体力", "stamina"),
    ("公母", "male female"),
    ("通缉", "wanted"),
    ("袭击", "raid"),
    ("维修", "repair"),
];

pub(super) fn query(s: &str) -> BTreeSet<String> {
    let s = crate::assistant_engine::field_text(s);
    let mut result: BTreeSet<_> = tokens(&s).collect();
    for (cn, en) in VOCAB {
        if s.contains(cn) {
            result.extend(tokens(en));
        }
    }
    result
}

pub(super) fn lookup(question: &str, terms: &[String]) -> Option<AssistantReply> {
    let bounded: String = question.trim().chars().take(1600).collect();
    let (title, context) = bounded.split_once('\n').unwrap_or((&bounded, ""));
    let original = query(title);
    let mut search = original.clone();
    search.extend(query(context));
    for term in terms.iter().take(4) {
        search.extend(query(term));
    }
    let data = index();
    let mut scores = Vec::new();
    for (i, doc) in data.docs.iter().enumerate() {
        let mut score = 0.0;
        let mut matches = 0;
        let mut original_hits = 0;
        for word in &search {
            let Some(&count) = doc.words.get(word) else {
                continue;
            };
            let df = data.frequency[word] as f64;
            let idf = ((data.docs.len() as f64 + 1.0) / (df + 0.5)).ln();
            if idf < 0.75 {
                continue;
            }
            let tf = f64::from(count);
            let in_original = original.contains(word);
            matches += 1;
            original_hits += usize::from(in_original);
            score += idf * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * doc.length / data.average))
                * if in_original { 1.4 } else { 0.7 };
        }
        if matches >= 2 && (original_hits > 0 || original.is_empty()) && score >= 6.0 {
            scores.push((score, i));
        }
    }
    scores.sort_by(|a, b| b.0.total_cmp(&a.0));
    let best = scores.first()?.0;
    let mut snippets = Vec::new();
    let mut links = Vec::new();
    for (score, i) in scores.into_iter().take(3) {
        if score < best * 0.7 {
            break;
        }
        let p = &data.docs[i].passage;
        if !links.iter().any(|(_, url)| url == &p.source) {
            links.push((format!("ThePalWiki · {}", p.topic), p.source.clone()));
        }
        snippets.push(format!(
            "{} / {} — {}\n{}",
            p.topic, p.section, p.source, p.text
        ));
    }
    Some(AssistantReply {
        text: format!(
            "社区攻略资料（ThePalWiki / Palworld Wiki contributors，CC BY-SA 4.0）；涉及数值时优先使用本版游戏表。\n{}",
            snippets.join("\n")
        ),
        links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concrete_topics_retrieve_attributed_prose_and_unrelated_words_do_not() {
        let a = lookup("Can I inherit passive skills from both parents?", &[]).unwrap();
        assert!(a.text.contains("https://www.thepalwiki.com/wiki/") && a.text.contains("parent"));
        assert!(a.links.iter().all(|(_, url)| url.starts_with("https://www.thepalwiki.com/wiki/") && a.text.contains(url)));
        assert!(lookup("where is my lost email password?", &[]).is_none());
        assert!(lookup("what is the best pal?", &[]).is_none());
    }
}
