//! Lexical passage retrieval over attributed article prose, separate from exact game data.
use super::{AssistantReply, has};
use crate::data::{Data, text};
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

// Short Chinese translations of the bundled article introductions. The topic
// key resolves to the original paragraph and attribution before an answer is returned.
const DEFINITIONS_ZH: &[(&str, &[&str], &str)] = &[
    ("alpha-pals", &["头目帕鲁", "头目个体", "alpha帕鲁", "alpha 帕鲁", "阿尔法帕鲁"], "头目（Alpha）是同一物种中的特殊帕鲁个体，通常比普通个体大。"),
    ("active-skills", &["主动技能", "主动技"], "主动技能是帕鲁在战斗中施放的能力，用来攻击敌人或触发特殊效果。"),
    ("passive-skills", &["被动技能", "被动技"], "被动技能是帕鲁自带的能力。帕鲁拥有该技能后就会持续获得其效果，无需手动施放。"),
    ("partner-skills", &["伙伴技能"], "每种帕鲁都有自己的伙伴技能。有些会被动生效，有些需要玩家按伙伴技能键发动。"),
    ("base-raids", &["据点袭击", "基地袭击"], "据点袭击是一种随机事件，帕鲁或其他阵营的敌人会入侵玩家的据点。"),
    ("raid-bosses", &["团战首领", "团体战首领"], "团战首领是可以在召唤祭坛召唤的强大帕鲁。"),
    ("dungeons", &["地下城", "地牢"], "地下城是分布在帕洛斯群岛上的独立区域，里面有敌人、宝箱，尽头还会有头目帕鲁。"),
    ("condensation", &["帕鲁浓缩", "浓缩"], "帕鲁浓缩是在帕鲁浓缩机里消耗同种帕鲁来强化目标帕鲁的机制。"),
    ("elements", &["属性", "元素属性"], "帕鲁和主动技能都有属性。属性会影响战斗中造成和承受的伤害。"),
    ("key-items", &["重要物品", "关键物品"], "重要物品存放在背包里的独立分类中，重量为零，玩家死亡时也不会掉落。"),
    ("breeding", &["配种", "帕鲁配种"], "配种是让帕鲁在配种牧场繁育后代、获得帕鲁蛋的机制。"),
];

fn names_alpha(q: &str) -> bool {
    has(q, &["头目","阿尔法"])
        || q.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|word| word == "alpha")
}

fn asks_about_alpha_mark(q: &str, allow_pronoun: bool) -> bool {
    let mark = has(q,&["图标","标记","标志","标识","记号","符号","图案","头像","icon","symbol","portrait"]);
    let size = has(q,&["体型","个头","块头","个子","大小","大只","大块头","size","larger"]);
    let identify = has(q,&["辨认","辨别","识别","认出","分辨"])
        || ["哪只是","哪一只是","哪个是","是不是","是否是"].iter().any(|cue| {
            q.split_once(cue).is_some_and(|(_, target)| {
                let target = target.trim_start();
                target.starts_with("头目") || target.starts_with("阿尔法")
                    || target.strip_prefix("alpha").is_some_and(|tail| !tail.starts_with(|c:char| c.is_ascii_alphanumeric() || c=='_'))
            })
        });
    let subject = names_alpha(q)
        || allow_pronoun && (mark || size || has(q, &["caught one","catch one","终端","详情页","抓到了一个"]));
    let identity = mark || size || identify || has(q, &["抓到","抓回","收服","捕获","捉到","终端","骷髅","个体详情","个体信息","是不是头目","是头目吗",
        "是头目么","分辨头目","辨认头目","识别头目","辨别头目","区分头目","如何识别","普通帕鲁","普通个体","闪光","掠食","普里莫","普利莫",
        "凭它大","光凭","只凭","算头目","认定","primo","alpha or","an alpha","regular pal","ordinary pal","caught","palbox","lucky","predator"]);
    subject && identity
        && (identify || mark && has(q,&["哪","什么","啥","怎样","找","用什么","代表","表示","有没有","哪个","which","what","where"])
          || size && has(q,&["凭","光看","单看","只看","就一定","就算","能确定","能判断","能分辨","就代表","说明","mean","tell"])
          || has(q, &["怎么看", "怎么认", "如何认", "怎么辨", "怎么分辨", "怎么区分", "怎么判断", "怎么知道", "如何分辨",
            "如何区分", "如何辨别", "如何识别", "如何判断", "怎样分辨", "怎样判断", "怎样辨认", "是不是头目", "是头目吗",
            "是头目么","是头目吧","算头目","算不算","就算","就能确定","认定","一定是","能断定","说明它是","说明是","是否为",
            "在哪看", "哪里看", "看哪","找哪","看啥","找啥","什么标记", "什么图标","什么标志","什么标识","什么符号",
            "哪个标志","哪个图标","相当于头目","标记长什么样", "图标长什么样", "什么区别", "有何区别", "difference between", "how do i know", "how can i tell",
            "how do i tell", "how to recognize", "how do i recognize", "how can i recognize", "how to identify","does that mean"]))
        && !has(q, &["在哪里抓", "在哪里刷", "哪里刷新", "复活", "刷新时间", "怎么打", "捕获率", "掉落率", "掉率", "的坐标",
            "克制", "弱点", "抗性", "个体值", "捕获概率", "配方","制作","capture rate", "respawn", "drop rate", "coordinates"])
}

fn linked_alpha_questions(q: &str) -> bool {
    if !names_alpha(q) { return false; }
    if !has(q,&["头目","阿尔法"]) && has(q,&["alpha版","alpha 版","测试版","内测版","alpha test","alpha channel","透明度","透明通道","alpha 通道","alpha通道","alpha值","alpha 值"]) { return false; }
    if !has(q,&["头目","阿尔法"]) && q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
        && !has(q,&["帕鲁","终端","物种","个体","哪只","一只","这只","那只","捉到","抓到","抓回","捕获","收服","palbox"]) { return false; }
    let mut named = false;
    let mut answered = false;
    for part in q.split_inclusive(['。', '.', '？', '?', '\n', '；', ';', '!','！']) {
        let sentence = part.trim().trim_end_matches(['。', '.', '？', '?', '；', ';', '!', '！']);
        let question = part.trim_end().ends_with(['?', '？'])
            || has(sentence, &["怎么看","如何","什么是","怎么","how do","how can","what is","what's","which icon"]);
        if question {
            let glossary = definition_topic(sentence)
                .is_some_and(|(topic,_)| matches!(topic.as_str(), "alpha-pal"|"alpha-pals"));
            if !glossary && !asks_about_alpha_mark(sentence,named) { return false; }
            answered = true;
        }
        named |= names_alpha(sentence);
    }
    answered || asks_about_alpha_mark(q,false)
}

fn alpha_identification(data: &Data, q: &str, chinese: bool) -> AssistantReply {
    let mut body = if chinese {
        "头目（Alpha）是同一物种中的特殊帕鲁个体，通常比普通个体大。单看体型还不能确认。捕获后打开帕鲁终端或个体详情，查看名字或头像旁的红色带角头目标记。"
    } else {
        "Alpha Pals are Pals that grow larger than ordinary members of the same species. Size alone is inconclusive. After capture, a red horned Alpha icon is displayed by the name or portrait in the Palbox or details page."
    }.to_owned();
    let mut links = vec![
        ("ThePalWiki · Alpha Pals".into(), "https://www.thepalwiki.com/wiki/alpha-pals".into()),
        ("玩家实测 · 终端与详情页标记".into(), "https://www.reddit.com/r/Palworld/comments/1vd46pz/whats_an_alpha_pal/".into()),
    ];
    if has(q,&["野外","野怪","野生","没抓","还没捉","wild"]) && has(q,&["等级","高出","高十","骷髅","红标","红色","level","skull"]) {
        body.push_str(if chinese { " 野外等级远高于玩家的敌人也可能显示红色警示，须区分野外警示和捕获后的个体标记。" }
            else { " A wild enemy far above your level can also show a red warning. Confirm the individual’s status on its details page after capture." });
    }
    if has(q, &["闪光","稀有个体","幸运个体","lucky","shiny","掠食","predator"]) {
        body.push_str(if chinese { " 闪光（Lucky）有自己的标记，掠食也有单独的类别。判断已捕获个体的头目身份，要看头目标记。" }
            else { " Lucky Pals have their own mark, and Predator Pals are a separate category. Use the Alpha icon to identify a captured Alpha." });
    }
    if has(q, &["普里莫","普利莫","primo"]) {
        body.push_str(if chinese { " 普里莫（Primo）是物种变体的名称，可按帕鲁图鉴条目辨别；同一个体是否为头目另看头目标记。" }
            else { " Primo is a species variant name listed in the Paldeck. Check the species entry for its variant and the individual’s icon for Alpha status." });
        let mut examples = Vec::new();
        for pal in data.species.iter().filter(|r| text(r,"en").ends_with(" Primo")).take(2) {
            let label = if chinese { format!("{}（{}）", text(pal,"zh"), text(pal,"en")) } else { text(pal,"en").to_owned() };
            links.push((label.clone(), format!("#paldex/{}", text(pal,"key"))));
            examples.push(label);
        }
        if !examples.is_empty() {
            body.push_str(if chinese {" 图鉴中可见 "} else {" Examples include "});
            body.push_str(&examples.join(if chinese {"、"} else {" and "}));
            body.push(if chinese {'。'} else {'.'});
        }
    }
    AssistantReply { text: body, links, suggestions: Vec::new() }
}

fn definition_topic(heading: &str) -> Option<(String, Option<&'static str>)> {
    let query = heading.trim().trim_start_matches("请问").trim_start().trim_end_matches(['?', '？', '.', '。']).trim();
    let zh_subject = ["什么是", "什麽是", "什么叫", "什麽叫"].iter()
        .find_map(|prefix| query.strip_prefix(prefix))
        .or_else(|| ["是什么", "是什麽", "是什么意思", "是啥", "是啥意思"].iter().find_map(|suffix| query.strip_suffix(suffix)))
        .map(str::trim);
    if let Some(subject) = zh_subject {
        let &(topic, _, body) = DEFINITIONS_ZH.iter().find(|(_, names, _)| names.contains(&subject))?;
        Some((topic.to_owned(), Some(body)))
    } else {
        let subject = ["what is ", "what are ", "what's ", "what’s "].iter()
            .find_map(|prefix| query.strip_prefix(prefix))?.trim();
        let subject = ["a ", "an ", "the "].iter().find_map(|prefix| subject.strip_prefix(prefix)).unwrap_or(subject);
        Some((subject.replace(' ', "-"), None))
    }
}

pub(crate) fn article_definition(data: &Data, question: &str) -> Option<AssistantReply> {
    let normalized = crate::assistant_engine::field_text(question);
    let (heading, context) = normalized.trim().split_once(['\n','?','？']).unwrap_or((normalized.trim(), ""));
    let context = context.trim();
    if linked_alpha_questions(&normalized) {
        return Some(alpha_identification(data, &normalized, normalized.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))));
    }
    let (topic, translation) = definition_topic(heading)?;
    let plural = if topic.ends_with("ss") { format!("{topic}es") } else { format!("{topic}s") };
    let p = index().docs.iter().map(|d| &d.passage)
        .find(|p| p.section.is_empty() && (p.topic == topic || p.topic == plural))?;
    if p.topic == "alpha-pals" && context.is_empty() {
        return Some(alpha_identification(data, &normalized, translation.is_some()));
    }
    if !context.is_empty() { return None; }
    let body = translation.map(str::to_owned).unwrap_or_else(|| {
        p.text.split_inclusive(". ").take(2).collect::<String>().trim().to_owned()
    });
    Some(AssistantReply {
        text: body,
        links: vec![(format!("ThePalWiki · {}", p.topic), p.source.clone())],
        suggestions: Vec::new(),
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
    fn exact_glossary_queries_use_the_named_article_introduction() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = Data::new(root, eframe::egui::Context::default()).unwrap();
        let lookup = |q: &str| article_definition(&data, q);
        let english = lookup("What's an Alpha Pal?").unwrap();
        assert!(english.text.starts_with("Alpha Pals are Pals"));
        let chinese = lookup("什麼是主動技能？").unwrap();
        assert!(chinese.text.starts_with("主动技能") && chinese.links[0].1.ends_with("/active-skills"));
        assert!(lookup("What is a raid boss?").unwrap().text.starts_with("Raid Bosses"));
        assert!(lookup("What is an Alpha Pal's capture rate?").is_none());
        let with_context = lookup("What is an Alpha Pal?\nHow can I tell if I caught one?").unwrap();
        assert!(with_context.text.contains("icon is displayed"));
        assert!(lookup("What is an Alpha Pal?\nHow do I know my ore is ready?").is_none());
        assert!(lookup("帕鲁浓缩的材料是什么？").is_none());
        for query in ["怎么看野外头目的刷新时间？", "如何判断这是哪个头目的掉落率？"] {
            assert!(lookup(query).is_none(), "{query}");
        }
    }
    #[test]
    fn identification_and_meal_patterns_answer_the_relation_at_the_player_entrypoint() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = Data::new(root, eframe::egui::Context::default()).unwrap();
        for (q, terms, link) in [
            ("什么是头目帕鲁？\n闪光、掠食和普里莫我分不清，我要怎么看自己抓到的是头目还是普里莫？",
                &["同一物种","红色带角","闪光","蚀魂鱿","Primo"][..], "#paldex/NegativeOctopus_Neutral"),
            ("抓回來的阿爾法帕魯如何識別？", &["终端","红色带角"][..], "https://www.thepalwiki.com/wiki/alpha-pals"),
            ("我在基地看到工人一直往返食槽，只吃一口就走，怎麼回事？", &["解除饥饿","营养","沙拉"][..], "#items/Salad"),
            ("为什么帕鲁吃到饱食度 50% 就停了，一降到 45% 又马上跑回饲料箱？", &["营养","百分比"][..], "#items/Salad"),
            ("我的基地打工仔啃两口就走，隔一阵又蹿回饭盆。改喂耐饿的东西能少跑几趟吗？", &["回去工作","沙拉","营养"][..], "#items/Salad"),
            ("据点的家伙吃到半饱就去干活，没多久又回食槽。换种饭能少来几趟吗？", &["解除饥饿","沙拉"][..], "#items/Salad"),
            ("收服了一只块头很大的。只凭体型能认定是头目吗？在终端里该看啥记号？", &["单看体型还不能确认","红色带角"][..], "https://www.thepalwiki.com/wiki/alpha-pals"),
            ("在野外看到高出我十级的野怪有红骷髅，这是不是头目？", &["野外警示","捕获后的个体标记"][..], "https://www.thepalwiki.com/wiki/alpha-pals"),
            ("帕鲁终端里两只看着都很壮，我想辨认哪只是 Alpha。要认头像上的什么图案？光凭大小行不行？", &["单看体型还不能确认","头像旁","红色带角"][..], "https://www.thepalwiki.com/wiki/alpha-pals"),
            ("这两只帕鲁哪一只是ａＬＰＨａ？靠头像认还是凭个头判断？", &["单看体型还不能确认","红色带角"][..], "https://www.thepalwiki.com/wiki/alpha-pals"),
            ("怎麼辨認ALPHA帕魯？抓回來之後要找什麼圖標？", &["个体详情","红色带角"][..], "https://www.thepalwiki.com/wiki/alpha-pals"),
        ] {
            let mut c = crate::assistant_engine::Conversation::default();
            let r = c.ask_world(q,&data,&serde_json::json!({}));
            assert!(c.direct_result() && terms.iter().all(|term|r.text.contains(term))
                && r.links.iter().any(|(_,url)|url==link), "{q}: {}", r.text);
        }
        for q in ["帕鲁饿了，在食槽边来回走，似乎寻路卡在墙里，怎样才能走到饲料箱？",
            "据点帕鲁吃几口就走。我想知道的是，食槽在哪里制作？","我老是跑回据点吃饭，怎么调玩家自己的饥饿速度？"] {
            assert!(!crate::assistant_engine::Conversation::default().ask_world(q,&data,&serde_json::json!({}))
                .text.contains("足以解除饥饿"), "{q}");
        }
        for q in ["怎么确认终端里这只头目的个体值？", "刚抓了头目。接下来去哪里刷它的材料？",
            "头目帕鲁怎么分辨？我的矿点怎么知道什么时候刷新？","工具箱的Alpha测试版，头像用的什么图案？",
            "帕鲁头像贴图的 alpha 通道用什么表示透明度？","帕鲁终端怎么辨认叫Alphabet的头像？",
            "头目帕鲁是不是吃得更多？"] {
            assert!(article_definition(&data,q).is_none(),"{q}");
        }
    }
    #[test]
    fn concrete_topics_retrieve_attributed_prose_and_unrelated_words_do_not() {
        let a = lookup("Can I inherit passive skills from both parents?", &[]).unwrap();
        assert!(a.text.contains("https://www.thepalwiki.com/wiki/") && a.text.contains("parent"));
        assert!(a.links.iter().all(|(_, url)| url.starts_with("https://www.thepalwiki.com/wiki/") && a.text.contains(url)));
        assert!(lookup("where is my lost email password?", &[]).is_none());
        assert!(lookup("what is the best pal?", &[]).is_none());
    }
}
