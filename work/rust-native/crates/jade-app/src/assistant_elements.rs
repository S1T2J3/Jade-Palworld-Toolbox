use crate::assistant_engine::AssistantReply;
use crate::data::{Data, array, normalize, text};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Direction {
    Incoming,
    Outgoing,
    #[default]
    Both,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ElementSession {
    direction: Direction,
    focus: Vec<String>,
}

struct Mention {
    start: usize,
    end: usize,
    kinds: Vec<String>,
    pal: Option<String>,
}

fn has(s: &str, options: &[&str]) -> bool {
    options.iter().any(|x| s.contains(x))
}

fn clean(q: &str) -> String {
    normalize(q)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn ignored(q: &str) -> bool {
    has(
        q,
        &[
            "饰品",
            "首饰",
            "徽章",
            "指挥棒",
            "词条",
            "掉落",
            "怎么抓",
            "怎么获得",
            "在哪里",
            "配种",
            "配方",
            "最强",
            "排名",
            "几级",
            "攻击力最高",
            "哪个更强",
        ],
    )
}

fn element_keys(book: &Value) -> impl Iterator<Item = (&String, &Value)> {
    book["elements"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.iter())
}

fn mentions(d: &Data, book: &Value, q: &str) -> Vec<Mention> {
    let mut found = Vec::new();
    for row in &d.species {
        let names = [text(row, "zh"), text(row, "en")]
            .into_iter()
            .chain(array(&row["aliases"]).iter().filter_map(Value::as_str));
        for name in names.map(clean).filter(|name| !name.is_empty()) {
            for (start, _) in q.match_indices(&name) {
                let kinds: Vec<_> = array(&row["types"])
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|k| book["elements"][*k].is_string())
                    .map(str::to_owned)
                    .collect();
                if !kinds.is_empty() {
                    found.push(Mention {
                        start,
                        end: start + name.len(),
                        kinds,
                        pal: Some(text(row, "key").into()),
                    });
                }
            }
        }
    }
    for (key, name) in element_keys(book) {
        let Some(label) = name.as_str() else { continue };
        let aliases: &[&str] = match key.as_str() {
            "neutral" => &["普通", "一般"],
            "ground" => &["岩", "土"],
            "electric" => &["电"],
            _ => &[],
        };
        for word in std::iter::once(label).chain(aliases.iter().copied()) {
            for (start, _) in q.match_indices(word) {
                let end = start + word.len();
                let before = &q[..start];
                let after = &q[end..];
                let suffix = ["属性", "系", "元素"]
                    .into_iter()
                    .find(|s| after.starts_with(s));
                let before_border = before.is_empty()
                    || [
                        "打",
                        "对付",
                        "用",
                        "上",
                        "拿",
                        "选",
                        "带",
                        "怕",
                        "克",
                        "克制",
                        "弱",
                        "对",
                        "对面是",
                        "对手是",
                        "只有",
                        "那",
                        "换成",
                        "反过来",
                        "相反",
                        "和",
                        "加",
                        "+",
                        "与",
                        "/",
                        "、",
                    ]
                    .iter()
                    .any(|p| before.ends_with(p));
                let after_border = after.is_empty()
                    || [
                        "呢",
                        "吗",
                        "?",
                        "。",
                        "双属性",
                        "双系",
                        "怕",
                        "弱",
                        "被",
                        "克",
                        "打",
                        "能打",
                        "用什么",
                        "用哪",
                        "用啥",
                        "可以打",
                        "怎么打",
                        "该",
                        "应该",
                        "行吗",
                        "可以吗",
                        "好用",
                        "有优势",
                        "占优",
                        "对",
                        "和",
                        "加",
                        "+",
                        "与",
                        "/",
                        "、",
                    ]
                    .iter()
                    .any(|p| after.starts_with(p));
                // A bare 火 or 地 inside an item or place name is not an element.
                let compound = after
                    .chars()
                    .next()
                    .is_some_and(|c| "龙水冰草火雷地暗电土".contains(c))
                    && has(after, &["双属性", "双系", "属性", "系"]);
                let follows_element = before
                    .chars()
                    .last()
                    .is_some_and(|c| "龙水冰草火雷地暗电土".contains(c));
                if suffix.is_some()
                    || before_border && (after_border || compound)
                    || follows_element && (after.starts_with("双属性") || after.starts_with("双系"))
                {
                    found.push(Mention {
                        start,
                        end: end + suffix.unwrap_or("").len(),
                        kinds: vec![key.clone()],
                        pal: None,
                    });
                }
            }
        }
    }
    found.sort_by_key(|m| (m.start, std::cmp::Reverse(m.end - m.start), m.pal.is_none()));
    let mut kept: Vec<Mention> = Vec::new();
    for m in found {
        if kept.last().is_none_or(|prior| prior.end <= m.start) {
            kept.push(m);
        }
    }
    kept
}

fn unique(mentions: &[Mention]) -> Vec<String> {
    let mut kinds = Vec::new();
    for k in mentions.iter().flat_map(|m| &m.kinds) {
        if !kinds.contains(k) {
            kinds.push(k.clone());
        }
    }
    kinds
}

fn list(book: &Value, values: &[String]) -> String {
    values
        .iter()
        .filter_map(|k| book["elements"][k].as_str())
        .map(|n| format!("{n}属性"))
        .collect::<Vec<_>>()
        .join("、")
}

fn beats(book: &Value, attack: &str, defense: &str) -> bool {
    array(&book["strong"][attack])
        .iter()
        .any(|t| t.as_str() == Some(defense))
}

fn card() -> AssistantReply {
    AssistantReply {
        text: String::new(),
        links: vec![("属性克制表".into(), "#guide/elements".into())],
        suggestions: vec![],
    }
}

/// Answer from the opponent's typing, never its party-only partner ability.
pub(crate) fn against_species(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(q, &["怎么打", "如何打", "打不过", "怎么对付", "如何击败", "怎么击败", "how do i beat ", "how to beat ", "how can i beat ", "how do i defeat ", "how to defeat ", "how to counter ", "tips for fighting "])
        || has(q, &["打工", "打孔", "打铁", "打出", "打断", "怎么打到"]) {
        return None;
    }
    let (names, _) = crate::assistant_breeding::species_mentions(&d.species, q);
    let [(index, _, _)] = names.as_slice() else { return None; };
    let pal = &d.species[*index];
    let key = text(pal, "key");
    let book = d.get("combat-guide-data");
    let types = array(&pal["types"]).iter().filter_map(Value::as_str)
        .filter(|t| book["elements"][*t].is_string()).map(str::to_owned).collect::<Vec<_>>();
    if types.is_empty() { return None; }
    let mut result = chart(d, book, &types, Direction::Incoming);
    let mut lines = vec![format!("{}（{}）是{}。{}", text(pal,"zh"), text(pal,"en"), list(book,&types), result.text)];
    result.links.insert(0, (text(pal,"zh").into(), format!("#paldex/{key}")));
    let details = d.get("pal-details-data");
    let known = array(&details["profiles"][key]["levels"]).iter().filter_map(|row| {
        let id = row[1].as_str()?;
        let mv = &details["moves"][id];
        (id.starts_with("Unique_") && mv["zh"].is_string()).then_some(mv)
    }).max_by_key(|mv| mv["power"].as_u64().unwrap_or(0));
    if let Some(mv) = known {
        lines.push(format!("该物种可学「{}」（{}属性）。遇到高威力招式时，可先收回正处于攻击范围的伙伴，躲开后再派出。",text(mv,"zh"),text(&book["elements"],text(mv,"type"))));
    }
    let mode = if has(q,&["hard mode","hardmode","高难","困难"]) { "高难" } else { "普通" };
    if let Some(enemy) = array(&d.get("enemy-data")["enemies"]).iter().find(|e|
        e["palKey"] == key && e["variant"] == mode && e["level"].is_u64()
    ) {
        lines.push(format!("如果你打的是{}「{}」，图鉴记录为 Lv.{}。",mode,text(enemy,"name"),enemy["level"]));
        result.links.push((text(enemy,"name").into(),format!("#enemies/{}",text(enemy,"id"))));
    }
    result.text = lines.join("\n");
    Some(result)
}

fn chart(d: &Data, book: &Value, subject: &[String], direction: Direction) -> AssistantReply {
    let mut reply = card();
    let mut lines = Vec::new();
    let mut examples = Vec::new();
    for kind in subject {
        let name = text(&book["elements"], kind);
        if direction != Direction::Outgoing {
            let counters: Vec<_> = element_keys(book)
                .filter(|(k, _)| beats(book, k, kind))
                .map(|(k, _)| k.clone())
                .collect();
            lines.push(if counters.is_empty() {
                format!("图鉴没有列出克制{name}属性的攻击。")
            } else {
                format!(
                    "打{name}属性对手，优先用{}攻击技能。",
                    list(book, &counters)
                )
            });
            examples.extend(counters);
        }
        if direction != Direction::Incoming {
            let targets: Vec<_> = array(&book["strong"][kind])
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            lines.push(if targets.is_empty() {
                format!("{name}属性攻击没有对任何单一属性的克制优势。")
            } else {
                format!("{name}属性攻击克制{}对手。", list(book, &targets))
            });
        }
    }
    if subject.len() > 1 && direction != Direction::Outgoing {
        lines.push("上面分别列出各属性的弱点。双属性的最终伤害倍率在这张表中没有记录。".into());
    }
    if subject.len() == 1 && direction == Direction::Incoming {
        if let Some(kind) = examples.first()
            && let Some((id, skill)) = d.get("pal-details-data")["moves"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, mv)| {
                    mv["type"].as_str() == Some(kind)
                        && !text(mv, "fruit").is_empty()
                        && !text(mv, "zh").is_empty()
                })
                .min_by_key(|(_, mv)| {
                    (
                        mv["cooldown"].as_u64().unwrap_or(u64::MAX),
                        mv["power"].as_u64().unwrap_or(u64::MAX),
                    )
                })
        {
            let name = text(skill, "zh");
            lines.push(format!(
                "例如「{name}」。出战前看看帕鲁已装备的主动技能，克制按实际放出的招式属性判断。"
            ));
            reply
                .links
                .push((name.into(), format!("#details/active/move/{id}")));
        }
    }
    reply.text = lines.join("\n");
    reply
}

fn matchup(book: &Value, attacking: &[String], defending: &[String]) -> AssistantReply {
    let mut reply = card();
    let mut lines = Vec::new();
    for attack in attacking {
        for defense in defending {
            let a = text(&book["elements"], attack);
            let b = text(&book["elements"], defense);
            lines.push(if beats(book, attack, defense) {
                format!("{a}属性攻击克制{b}属性对手，可以用。")
            } else if beats(book, defense, attack) {
                format!("{a}属性攻击对{b}属性没有克制优势。反过来，{b}属性攻击克制{a}属性。")
            } else {
                format!("图鉴没有列出{a}属性攻击对{b}属性的克制优势。")
            });
        }
    }
    if defending.len() > 1 {
        lines.push("这是按每个防守属性分别查表的结果，图鉴未记录双属性的最终倍率。".into());
    }
    reply.text = lines.join("\n");
    reply
}

fn type_name(key: &str) -> String {
    let mut name = key.to_owned();
    name.get_mut(..1).map(str::make_ascii_uppercase);
    name
}

fn english_chart(book: &Value, focus: &str, direction: Direction) -> AssistantReply {
    let mut reply = card();
    let name = type_name(focus);
    let mut lines = Vec::new();
    if direction != Direction::Outgoing {
        let attacks = element_keys(book)
            .filter(|(key, _)| beats(book, key, focus))
            .map(|(key, _)| type_name(key))
            .collect::<Vec<_>>();
        lines.push(if attacks.is_empty() {
            format!("The chart lists no type advantage against {name}.")
        } else {
            format!(
                "Against a {name} defender, use {} attacks.",
                attacks.join(" or ")
            )
        });
    }
    if direction != Direction::Incoming {
        let targets = array(&book["strong"][focus])
            .iter()
            .filter_map(Value::as_str)
            .map(type_name)
            .collect::<Vec<_>>();
        lines.push(if targets.is_empty() {
            format!("{name} attacks have no listed type advantage.")
        } else {
            format!(
                "{name} attacks are strong against {}.",
                targets.join(" and ")
            )
        });
    }
    reply.text = lines.join("\n");
    reply
}

// Model retrieval uses evidence for *both* directions when a plan supplies just a type.
pub(crate) fn reference(d: &Data, term: &str) -> Option<AssistantReply> {
    let q = clean(term);
    if q.is_empty() || q.chars().count() > 45 || ignored(&q) {
        return None;
    }
    let book = d.get("combat-guide-data");
    if !book["strong"].is_object() {
        return None;
    }
    if [
        "属性克制表",
        "属性克制",
        "元素克制表",
        "属性相克",
        "属性相克表",
        "属性关系",
        "属性关系表",
        "克制关系",
    ]
    .contains(&q.as_str())
    {
        let keys = element_keys(book)
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>();
        return Some(chart(d, book, &keys, Direction::Outgoing));
    }
    let found = mentions(d, book, &q);
    if found.is_empty() {
        return None;
    }
    if !has(
        &q,
        &["弱点", "弱", "怕", "克制", "相克", "打", "对付", "天敌"],
    ) && !(found.len() == 1
        && found[0].pal.is_none()
        && found[0].start == 0
        && found[0].end == q.len())
    {
        return None;
    }
    let mut reply = chart(d, book, &unique(&found), Direction::Both);
    for m in &found {
        if let Some(key) = &m.pal {
            let name = d.pal_name(key);
            reply.text = format!("{name}的属性为{}。\n{}", list(book, &m.kinds), reply.text);
            reply.links.push((name, format!("#paldex/{key}")));
        }
    }
    Some(reply)
}

impl ElementSession {
    pub(crate) fn respond(&mut self, d: &Data, question: &str) -> Option<AssistantReply> {
        if let Some(reply) = self.pure_questions(d, question) {
            return Some(reply);
        }
        let q = clean(question);
        let book = d.get("combat-guide-data");
        if ignored(&q) || !book["strong"].is_object() {
            return None;
        }
        if has(&q, &["属性克制表", "属性相克表", "完整属性克制"]) {
            return reference(d, "属性克制表");
        }
        let found = mentions(d, book, &q);
        let active = !self.focus.is_empty();
        let mut outside_names = q.clone();
        for mention in found.iter().rev().filter(|m| m.pal.is_some()) {
            outside_names.replace_range(mention.start..mention.end, " ");
        }
        let explicit = has(
            &outside_names,
            &[
                "打",
                "对付",
                "挑战",
                "对抗",
                "对上",
                "敌人",
                "对面",
                "对手",
                "克",
                "弱点",
                "弱什么",
                "怕",
                "天敌",
                "擅长对付",
            ],
        );
        let short = active
            && q.chars().count() <= 16
            && (q.starts_with('那')
                || has(
                    &q,
                    &["换成", "呢", "可以吗", "行吗", "怎么样", "能用吗", "反过来"],
                ));
        if !explicit && !short {
            return None;
        }
        let pointed = active && has(&q, &["它", "这种属性", "它们", "反过来", "反过来说"]);
        if found.is_empty() && !pointed {
            return None;
        }
        let mut subject = unique(&found);
        if subject.is_empty() {
            subject = self.focus.clone();
        }
        // Two mentioned sides can appear in either order: 冰打龙 versus 打龙用冰.
        for split in 1..found.len() {
            let between = &q[found[split - 1].end..found[split].start];
            let prefix = &q[..found[0].start];
            let reversed = has(
                between,
                &["被", "怕", "弱点", "惧", "用什么", "怎么打", "该怎么打"],
            ) || (has(prefix, &["打", "对付", "挑战", "对上", "对抗"])
                && has(between, &["用", "带", "选", "拿", "换", "上"]));
            let compare_pair =
                found.len() == 2 && has(&q, &["谁克谁", "谁克制谁", "哪个克哪个", "互相克制"]);
            if reversed
                || compare_pair
                || has(between, &["打", "克", "对付", "对抗", "挑战", "对上"])
            {
                let left = unique(&found[..split]);
                let right = unique(&found[split..]);
                let (attack, defense) = if reversed {
                    (&right, &left)
                } else {
                    (&left, &right)
                };
                self.direction = Direction::Incoming;
                self.focus = defense.clone();
                let mut reply = matchup(book, attack, defense);
                if has(&q, &["几倍", "倍率", "加成多少"]) && defense.len() == 1 {
                    reply
                        .text
                        .push_str("\n图鉴只记录克制关系，没有给出具体伤害倍率。");
                }
                return Some(reply);
            }
        }
        let prefix = found.first().map_or("", |m| &q[..m.start]);
        let suffix = found.last().map_or(q.as_str(), |m| &q[m.end..]);
        let uses = ["用", "带", "选", "拿", "上", "派", "我是"]
            .iter()
            .any(|v| prefix.ends_with(v));
        let unusual = has(
            &q,
            &[
                "为什么叫",
                "掉什么",
                "打工",
                "工作",
                "建造",
                "打捞",
                "打包",
                "打不开",
                "打造",
                "打折",
            ],
        );
        if unusual {
            return None;
        }
        if active
            && (uses && (short || pointed) || has(suffix, &["能打吗", "可以打吗", "能克制吗"]))
            && !has(
                suffix,
                &["打谁", "克谁", "打什么", "克什么", "打哪些", "打哪"],
            )
        {
            return Some(matchup(book, &subject, &self.focus));
        }
        let direction = if has(
            suffix,
            &[
                "怕",
                "弱点",
                "被克",
                "被谁克",
                "弱什么",
                "被什么",
                "天敌",
                "怎么打",
                "好打吗",
                "用什么打",
            ],
        ) || has(
            prefix,
            &["怎么打", "对付", "对抗", "对上", "打", "挑战", "克制", "克"],
        ) && !uses
        {
            Direction::Incoming
        } else if uses
            || has(
                suffix,
                &[
                    "克谁",
                    "克制谁",
                    "克什么",
                    "克制什么",
                    "克哪些",
                    "克制哪些",
                    "克哪",
                    "克制哪",
                    "打谁",
                    "打什么",
                    "打哪些",
                    "打哪",
                    "擅长打",
                    "对付什么",
                ],
            )
        {
            Direction::Outgoing
        } else if q.contains("反过来") && active {
            match self.direction {
                Direction::Incoming => Direction::Outgoing,
                Direction::Outgoing => Direction::Incoming,
                Direction::Both => Direction::Both,
            }
        } else if short && !explicit {
            self.direction
        } else if has(
            &q,
            &[
                "应该用",
                "用什么",
                "用啥",
                "不好打",
                "打不过",
                "弱点",
                "怕什么",
                "哪种克",
                "什么克",
            ],
        ) {
            Direction::Incoming
        } else {
            Direction::Both
        };
        self.focus = subject.clone();
        self.direction = direction;
        let mut reply = chart(d, book, &subject, direction);
        if let Some((pal, key)) = found
            .iter()
            .find_map(|m| m.pal.as_ref().map(|key| (m, key)))
        {
            let name = d.pal_name(key);
            reply.text = format!("{name}是{}帕鲁。\n{}", list(book, &pal.kinds), reply.text);
            reply.links.push((name, format!("#paldex/{key}")));
        }
        Some(reply)
    }

    fn pure_questions(&mut self, d: &Data, question: &str) -> Option<AssistantReply> {
        let q = crate::assistant_engine::field_text(question);
        let book = d.get("combat-guide-data");
        if !book["strong"].is_object()
            || ignored(&q)
            || has(
                &q,
                &[
                    "招式",
                    "被动",
                    "敌人名字",
                    "倍率",
                    "几倍",
                    "和谁配",
                    "掉什么",
                    "弱点伤害",
                    "tower",
                    "weapon",
                    "multiplier",
                    "moveset",
                    "passive",
                    "skill fruit",
                    "damage",
                ],
            )
        {
            return None;
        }
        let mut split = q.replace(['？', '，', '。', '；'], "?");
        for (sep, replacement) in [
            ("以及", "?"),
            ("然后", "?"),
            ("另外", "?"),
            ("又被", "?被"),
            ("又克", "?克"),
            ("又怕", "?怕"),
            ("同时被", "?被"),
            (" and what", "?what"),
            (" and which", "?which"),
            (" and who", "?who"),
            (" and is ", "?is "),
            (" and does ", "?does "),
            (" and weak ", "?weak "),
            (" and strong ", "?strong "),
        ] {
            split = split.replace(sep, replacement);
        }
        let parts = split
            .split(['?', ',', ';', '!', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        let en = q.is_ascii();
        if !en && parts.len() < 2 {
            return None;
        }
        let mut focus = self.focus.clone();
        let mut direction = self.direction;
        let mut combined = card();
        for clause in parts {
            let compact = clean(clause);
            let found = if en {
                let mut found = Vec::new();
                let mut pos = 0;
                for word in clause
                    .split(|ch: char| !ch.is_ascii_alphabetic())
                    .filter(|w| !w.is_empty())
                {
                    let a = pos + clause[pos..].find(word)?;
                    pos = a + word.len();
                    let key = match word {
                        "earth" => "ground",
                        "lightning" => "electric",
                        "shadow" => "dark",
                        "normal" => "neutral",
                        "plant" => "grass",
                        other => other,
                    };
                    if book["elements"][key].is_string() {
                        found.push(Mention {
                            start: a,
                            end: pos,
                            kinds: vec![key.to_owned()],
                            pal: None,
                        });
                    } else if ![
                        "a",
                        "an",
                        "the",
                        "is",
                        "are",
                        "do",
                        "does",
                        "can",
                        "i",
                        "me",
                        "my",
                        "what",
                        "whats",
                        "which",
                        "who",
                        "how",
                        "would",
                        "use",
                        "using",
                        "against",
                        "strong",
                        "good",
                        "super",
                        "effective",
                        "strength",
                        "strengths",
                        "weak",
                        "weakness",
                        "weaknesses",
                        "vulnerable",
                        "to",
                        "lose",
                        "loses",
                        "beats",
                        "beat",
                        "counter",
                        "counters",
                        "countered",
                        "by",
                        "type",
                        "types",
                        "element",
                        "elements",
                        "attack",
                        "attacks",
                        "defend",
                        "defense",
                        "defender",
                        "advantage",
                        "over",
                        "it",
                        "its",
                        "them",
                        "that",
                        "this",
                        "and",
                        "or",
                        "other",
                        "way",
                        "round",
                        "reverse",
                        "please",
                        "then",
                        "also",
                        "pal",
                        "pals",
                        "in",
                        "palworld",
                        "s",
                    ]
                    .contains(&word)
                    {
                        return None;
                    }
                }
                found
            } else {
                mentions(d, book, &compact)
            };
            if found.iter().any(|m| m.pal.is_some()) {
                return None;
            }
            let s = if en { clause } else { compact.as_str() };
            let weak = has(
                s,
                &[
                    "weak to",
                    "weak against",
                    "weakness",
                    "vulnerable",
                    "countered by",
                    "loses to",
                    "被",
                    "怕",
                    "弱点",
                    "弱什么",
                    "天敌",
                ],
            );
            let strong = has(
                s,
                &[
                    "strong against",
                    "effective against",
                    "good against",
                    "beat",
                    "counter",
                    "advantage over",
                    "克",
                    "打",
                ],
            );
            let reverse = has(s, &["反过来", "反过来说", "other way round", "reverse"]);
            if !weak && !strong && !reverse {
                return None;
            }
            let subjects = unique(&found);
            if subjects.is_empty()
                && (focus.is_empty()
                    || !reverse
                        && !has(
                            s,
                            &[
                                "被", "克", "怕", "它", "weak ", "strong ", "its ", "it ", "it?",
                                "it.",
                            ],
                        ))
            {
                return None;
            }
            let result = if found.len() == 2 {
                let between = &s[found[0].end..found[1].start];
                if has(between, &[" and ", "和", "与", "、", " all "])
                    || !has(
                        between,
                        &[
                            "克", "打", "怕", "弱", "被", "对", "strong", "weak", "beat", "against",
                            "counter", "lose",
                        ],
                    )
                {
                    return None;
                }
                let (attack, defense) = if weak {
                    (&found[1].kinds, &found[0].kinds)
                } else {
                    (&found[0].kinds, &found[1].kinds)
                };
                focus = defense.clone();
                direction = Direction::Incoming;
                if en {
                    let mut result = card();
                    let (a, b) = (&attack[0], &defense[0]);
                    result.text = if beats(book, a, b) {
                        format!(
                            "{} attacks are strong against {}.",
                            type_name(a),
                            type_name(b)
                        )
                    } else {
                        format!(
                            "{} attacks have no listed type advantage against {}.",
                            type_name(a),
                            type_name(b)
                        )
                    };
                    result
                } else {
                    matchup(book, attack, defense)
                }
            } else {
                if found.len() > 2 {
                    return None;
                }
                let prefix = found.first().map_or("", |m| &s[..m.start]);
                direction = if reverse && !weak && !strong {
                    match direction {
                        Direction::Incoming => Direction::Outgoing,
                        Direction::Outgoing => Direction::Incoming,
                        Direction::Both => Direction::Both,
                    }
                } else if weak
                    && (has(
                        s,
                        &[
                            "strong against",
                            "effective against",
                            "能克",
                            "克制谁",
                            "克什么",
                        ],
                    ) && !s.contains("被谁克"))
                {
                    Direction::Both
                } else if weak {
                    if has(prefix, &["is weak to", "are weak to", "弱于", "怕", "被"])
                        && !has(s, &["弱点", "weakness"])
                    {
                        Direction::Outgoing
                    } else {
                        Direction::Incoming
                    }
                } else if has(
                    prefix,
                    &[
                        "beats ",
                        "beat ",
                        "counters ",
                        "counter ",
                        "against ",
                        "克",
                        "打",
                    ],
                ) {
                    Direction::Incoming
                } else {
                    Direction::Outgoing
                };
                if !subjects.is_empty() {
                    focus = subjects;
                }
                if en {
                    english_chart(book, &focus[0], direction)
                } else {
                    chart(d, book, &focus, direction)
                }
            };
            if !combined.text.is_empty() {
                combined.text.push('\n');
            }
            combined.text.push_str(&result.text);
            for link in result.links {
                if !combined.links.contains(&link) {
                    combined.links.push(link);
                }
            }
        }
        if combined.text.is_empty() {
            return None;
        }
        self.focus = focus;
        self.direction = direction;
        Some(combined)
    }
}
