use super::{BreedingSession, Data, OffspringFormat, ReferenceAnswer, SpeciesSpan, array, sex_by_name, species_mentions, text};

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|s| q.contains(s))
}

pub(super) fn element_question(q: &str) -> bool {
    let asks_about_type = q.match_indices('系').any(|(i, s)|
        !q[..i].ends_with(['关', '联', '体', '血', '派']) && !q[i+s.len()..].starts_with(['统', '列', '数']));
    if asks_about_type && has(q, &["哪", "什", "啥", "几", "各", "分别", "两", "双", "单"]) { return true; }
    has(q, &["属性", "元素", "什么系", "啥系", "哪系", "哪个系", "哪些系", "哪一系", "哪种系", "哪两系", "哪两种系",
        "几系", "几种系", "单系", "双系", "系别", "系的", "水系", "火系", "草系", "冰系", "暗系",
        "龙系", "地系", "土系", "无系", "雷系", "电系", "element", " type", "typing"])
        || q.starts_with("type")
}

pub(super) struct OffspringFields {
    name: bool,
    types: Option<bool>,
    restricted: bool,
    pairing_details: bool,
}

impl OffspringFields {
    pub(super) fn format(&self, ordinary_child_lookup: bool) -> OffspringFormat {
        let compact = !self.pairing_details && (self.restricted || (ordinary_child_lookup && self.name && self.types == Some(true)));
        match (compact && (self.name || self.types == Some(true)), self.types) {
            (true, Some(true)) => OffspringFormat::NamesWithTypes,
            (true, _) => OffspringFormat::Names,
            (false, Some(false)) => OffspringFormat::WithoutTypes,
            _ => OffspringFormat::WithTypes,
        }
    }
}

pub(super) fn offspring_fields(q: &str) -> OffspringFields {
    let mut fields = OffspringFields { name: false, types: None, restricted: false, pairing_details: false };
    for clause in q.split(['，', ',', '。', '.', '？', '?', '；', ';', '\n']) {
        let excluded = excluded_parent(clause);
        if let Some(end) = excluded {
            fields.restricted = true;
            if end == 0 { continue; }
        }
        let clause = &clause[..excluded.unwrap_or(clause.len())];
        let name = has(clause, &["名字", "名称", "物种", "图鉴名", "叫啥", "叫什么", "宝宝名", "子代名", "幼崽名", "孩子名", "后代名",
            "名及", "名与", "名和", "名跟", "哪只", "哪一只", "哪一种", "是哪种", "哪个帕鲁", "是谁", "孵出谁", "会出谁", "name", "species"]);
        fields.name |= name;
        let elements = element_question(clause);
        let numbered_fields = clause.char_indices().any(|(i, unit)| "项件栏条格样点个列行".contains(unit)
            && clause[..i].trim_end().chars().next_back().is_some_and(|n| n.is_ascii_digit() || "一二两三四五六七八九十".contains(n)));
        let listing = numbered_fields || has(clause, &["字段", "填表", "答案", "答复", "回复", "记录", "结果", "输出", "清单"]);
        let restricted = ["只", "仅", "就"].iter().any(|marker| clause.match_indices(marker).any(|(i, _)| {
            let before = clause[..i].trim_end();
            let after = clause[i + marker.len()..].trim_start();
            !["不", "别", "不要", "不能", "不应", "并非", "一", "两", "这", "那", "哪", "几", "每"].iter().any(|s| before.ends_with(s))
                && !after.starts_with(['有', '能', '是', '会']) && (*marker != "就" || listing
                    || after.starts_with(['写', '列', '填', '给', '留', '报', '说', '记', '抄', '保']))
        }));
        fields.restricted |= ((name || elements || listing) && (restricted || has(clause, &["光说", "就行", "即可", "就好", "就够", "这两项", "两个字段", "only", "just"]))
            && !has(clause, &["not just", "not only"])) || has(clause, &["别附说明", "不加说明", "不用解释", "不要链接"]);
        fields.pairing_details |= asks_for_pairing_details(clause);
        if elements {
            let omit = has(clause, &["不用", "不要", "不必", "不需要", "无须", "无需", "不问", "不想", "不带", "不列", "不写", "不附", "免写", "免列", "别说", "别报", "别提", "省略", "忽略", "skip", "omit", "without", "no type", "no element"]);
            let keep = has(clause, &["别漏", "不要漏", "不能漏", "别省", "不要省", "别忽略", "不要忽略", "don't omit", "don't skip"]);
            fields.types = Some(!omit || keep);
        }
    }
    fields
}

fn asks_for_pairing_details(clause: &str) -> bool {
    let subjects = ["搭配", "亲本", "双亲", "父母", "公母", "性别", "父本", "母本", "组合", "条件",
        "配方", "配种过程", "路线", "步骤", "情况", "parents", "conditions", "sexes", "genders"];
    subjects.iter().any(|subject| clause.match_indices(subject).any(|(at, word)| {
        let before = clause[..at].trim_end();
        let after = clause[at + word.len()..].trim_start();
        ["列出", "列上", "写出", "写上", "也要", "加上", "带上", "附上", "说明", "注明", "展示", "给出", "说清",
            "包括", "包含", "对照", "对调", "互换", "list", "show", "swap"].iter().any(|verb| before.ends_with(verb))
            || ["怎么", "怎样", "如何", "有什么要求", "各是什么", "分别是", "也要", "也写", "也列", "一起写", "一起列",
                "对调", "互换", "和各自", "与各自"].iter().any(|predicate| after.starts_with(predicate))
            || (word == "搭配" && before.ends_with("两种") && has(clause, &["列", "各", "分别", "哪些"]))
    }))
}

/// Only resolve an omitted species against the previous pair for a question about the child.
/// A parent's typing, battle weakness, or move element has a different subject.
pub(super) fn child_followup(q: &str) -> bool {
    let mut child_subject = None;
    let mut asks_child = false;
    for clause in q.split(['，', ',', '。', '.', '？', '?', '！', '!', '；', ';', '\n']).map(str::trim).filter(|s| !s.is_empty()) {
        if has(clause, &["不问", "不是问", "别讲", "不用讲", "先不管", "不用说", "不想知道"]) {
            continue;
        }
        if has(clause, &["克制", "弱点", "招式", "攻击", "技能", "父本", "母本", "亲本", "双亲", "父母",
            "父方", "母方", "男方", "女方", "父亲", "母亲", "爸", "妈", "爹", "娘", "家长", "生它的",
            "工作", "装备", "道具", "材料", "招数", "对手", "那只公的", "那只母的", "opponent", "weak", "move", "attack", "parent", "mother", "father"]) {
            child_subject = Some(false);
        } else if has(clause, CHILD) || has(clause, &["幼体", "小帕鲁",
            "孵出", "孵的", "配出的", "配出来", "刚出生", "下一代", "孩子", "刚才那只", "刚刚那只", "它", "这只", "那只",
            "offspring", "hatchling", "baby", "the child", "its type", "its element", "is it", "does it"]) {
            child_subject = Some(true);
        }
        if element_question(clause) {
            let bare = clause.trim_end_matches('呢');
            asks_child = child_subject.unwrap_or(matches!(bare, "什么属性" | "属性" | "什么系" | "哪几系" | "哪两系" | "什么元素" | "单系还是双系" | "what type" | "what element")
                || ["两系", "这两系", "系别", "双系", "两种属性"].iter().any(|stem| bare.starts_with(stem)));
        }
    }
    asks_child
}

pub(super) fn describes_parents(q: &str) -> bool {
    has(q, &["亲本是", "亲本分别", "作亲本", "当亲本", "作为亲本", "父本是", "母本是", "配给", "×", "+"])
        || (has(q, &["配种", "配对", "一起配"]) && q.split(['，', ',', '。', '.', '？', '?', '；', ';', '\n']).any(|clause|
            has(clause, &["和", "跟", "与"]) && (has(clause, &["配种", "配对", "一起配"]) || (!element_question(clause) && has(clause, &["用", "拿", "放入", "放进"])))))
}

const MOTHER: &[&str] = &["母本", "母方", "雌方", "女方", "母亲", "妈", "娘", "mother"];
const FATHER: &[&str] = &["父本", "父方", "雄方", "男方", "父亲", "爸", "爹", "father"];
const PARENTS: &[&str] = &["亲本", "双亲", "父母", "生它的", "家长", "parent"];

fn parent_position(clause: &str) -> Option<usize> {
    MOTHER.iter().chain(FATHER).chain(PARENTS).filter_map(|word| clause.rfind(word)).max()
}

fn excluded_parent(clause: &str) -> Option<usize> {
    let parent = parent_position(clause).or_else(|| ["公母", "亲代", "配对过程", "配种过程"].iter().filter_map(|word| clause.find(word)).min())?;
    let omit = ["不用", "不必", "无须", "无需", "不要", "不报", "不列", "不说", "不问", "不谈", "不看", "不写", "不提", "不带", "不需要", "不想",
        "别讲", "别说", "别提", "别列", "别报", "别给", "别把", "别算", "别再", "别管", "别附", "先不管", "排除", "省略", "略过", "跳过", "免写", "忽略"]
        .iter().flat_map(|term| clause.match_indices(term).filter_map(move |(i, _)|
            (!term.starts_with('别') || !clause[..i].ends_with(['分', '区', '特', '类', '性', '个'])).then_some(i))).min()?;
    Some(omit.min(parent))
}

pub(super) fn profile_field(q: &str) -> bool {
    let skill = has(q, &["伙伴技能", "搭档技能", "partner skill"]);
    let speculative = has(q, &["推断", "猜"]) && !has(q, &["不猜", "别猜", "不要猜", "不用猜", "无需猜", "不用推断"]);
    (skill || element_question(q)) && !speculative && !has(q, &["遗传", "继承", "克制", "弱点", "opponent", "inherit"])
        && (skill || !has(q, &["技能", "招式", "攻击", "工作", "装备", "对手", "move", "attack"]))
}

const CHILD: &[&str] = &["子代", "后代", "下一代", "孩子", "宝宝", "小家伙", "小不点", "幼崽", "幼体", "崽", "孵出", "配出的", "配出来", "刚出生", "蛋里", "蛋中", "蛋内",
    "这窝", "这一窝", "窝里", "这胎", "这一胎", "offspring", "baby", "hatches"];

pub(super) fn parent_field_request(q: &str) -> bool {
    if !profile_field(q) { return false; }
    let mut parent_subject = false;
    let mut requested = false;
    for clause in q.split(['，', ',', '。', '.', '?', '？', '；', ';', '\n']) {
        let clause = &clause[..excluded_parent(clause).unwrap_or(clause.len())];
        let child = CHILD.iter().filter_map(|word| clause.rfind(word)).max();
        if let Some(parent) = parent_position(clause) {
            let after = &clause[parent..];
            let breeding_action = has(after, &["配种", "配对", "交配", "繁殖", "breed"])
                && !has(after, &["本身", "自己", "自身"]);
            parent_subject = !breeding_action && !child.is_some_and(|position| position > parent);
        } else if child.is_some() || has(clause, &["配种", "配对", "交配", "繁殖", "孵化", "breed", "hatch"]) {
            parent_subject = false;
        }
        if profile_field(clause) { requested = parent_subject; }
    }
    requested
}

pub(super) fn forward_then_parent_field<'a>(d: &Data, q: &'a str) -> Option<(&'a str, &'a str, bool)> {
    let child_outcome = |s: &str| has(s, &["子代", "后代", "孩子", "宝宝", "配出", "孵出", "孵什么", "生出", "会生", "会出", "结果", "offspring", "hatch"]);
    if !(child_outcome(q) || has(q, &["配种", "配对", "配给", "交配", "breed"])) { return None; }
    // Keep the complete forward question, including an outcome or no-record clause, before the subject switch.
    for (i, separator) in q.rmatch_indices(['，', ',', '。', '.', '？', '?', '；', ';', '\n']) {
        let (first, later) = (&q[..i], q[i+separator.len()..].trim());
        if !(child_outcome(first) || has(first, &["配种", "配对", "配给", "交配", "breed"])) || !profile_field(later) { continue; }
        let explicit_parent = parent_field_request(later);
        if !explicit_parent && (has(later, CHILD) || excluded_parent(later).is_some()
            || species_mentions(&d.species, later).0.is_empty()) { continue; }
        if species_mentions(&d.species, first).0.len() < 2 { continue; }
        let parent_only = has(later, &["只问", "只想", "改问", "单问", "单说", "只关心"])
            || has(first, &["不问子代", "不问孩子", "不看子代", "先不管子代", "先不看子代", "不关心子代"]);
        return Some((first, later, child_outcome(first) && !parent_only));
    }
    None
}

pub(super) fn species_elements(d: &Data, id: usize, english: bool) -> Option<String> {
    let elements = &d.get("combat-guide-data")["elements"];
    let labels: Vec<_> = array(&d.species[id]["types"]).iter().filter_map(serde_json::Value::as_str)
        .map(|key| if english { key.to_owned() } else { format!("{}属性", elements[key].as_str().unwrap_or(key)) }).collect();
    (!labels.is_empty()).then(|| labels.join(if english { " / " } else { "、" }))
}

/// A parent explicitly mentioned in a previous pair is a dex subject, not a reverse breeding goal.
pub(super) fn parent_reference(d: &Data, q: &str, parents: [(usize, Option<char>); 2], compared: bool, focus: Option<usize>, explicit: bool) -> Option<(ReferenceAnswer, Option<usize>)> {
    if !explicit && (has(q, CHILD) || has(q, &["配", "孵", "繁殖", "搭", "×", "+", "breed"])) { return None; }
    let (mentions, _) = species_mentions(&d.species, q);
    let mut named: Vec<_> = mentions.iter().map(|span| span.0).collect();
    let mut parents = parents;
    if mentions.iter().any(|&(id, _, end)| !parents.iter().any(|p| p.0 == id)
        || q[end..].strip_prefix('的').is_some_and(|tail| MOTHER.iter().chain(FATHER).chain(PARENTS).any(|role| tail.starts_with(role)))) { return None; }
    if !explicit && named.is_empty() {
        if !(has(q, &["它", "这只", "那只", "its "]) || q.ends_with("呢?") || q.ends_with('呢')) { return None; }
        named.push(focus?);
    }
    let sex = match (has(q, MOTHER), has(q, FATHER)) { (true, false) => Some('F'), (false, true) => Some('M'), _ => None };
    // One specified sex fixes the other for a valid pair. A comparison shows both assignments.
    if compared { parents = parents.map(|(id, _)| (id, None)); }
    if let [(_, Some(s)), (b, None)] = parents { parents[1] = (b, Some(if s == 'F' { 'M' } else { 'F' })); }
    if let [(a, None), (_, Some(s))] = parents { parents[0] = (a, Some(if s == 'F' { 'M' } else { 'F' })); }
    let mut ids: Vec<_> = parents.iter().filter(|(id, role)| {
        if !named.is_empty() { named.contains(id) } else { !role.zip(sex).is_some_and(|(a,b)| a!=b) }
    }).map(|p| p.0).collect();
    ids.dedup();
    let focus = (ids.len() == 1).then(|| ids[0]);
    let english = q.is_ascii();
    let mut lines = Vec::new();
    let mut links = Vec::new();
    if ids.is_empty() || (named.is_empty() && sex.is_some() && parents[0].1.is_none() && parents[0].0 != parents[1].0) {
        lines.push(if english { "The previous pair did not identify a parent in that role." } else { "上轮这对亲本没有确定该角色所属的物种。" }.to_owned());
    }
    for id in ids {
        let species = &d.species[id];
        let key = text(species, "key");
        let label = text(species, if english { "en" } else { "zh" });
        let link = (label.to_owned(), format!("#paldex/{key}"));
        if links.contains(&link) { continue; }
        links.push(link);
        if element_question(q) {
            let elements = species_elements(d, id, english).unwrap_or_else(|| if english { "element not recorded" } else { "图鉴未收录属性" }.into());
            lines.push(format!("{label} · {elements}"));
        }
        if has(q, &["伙伴技能", "搭档技能", "partner skill"]) {
            let skill = &d.get("partner-skills-data")[key];
            let name = text(skill, "name");
            if name.is_empty() { lines.push(format!("{label} · {}", if english { "partner skill not recorded" } else { "图鉴未收录伙伴技能" })); }
            else {
                lines.push(format!("{label} · {}「{name}」{}", if english { "Partner skill " } else { "伙伴技能" }, text(skill, "effect")));
                links.push((name.into(), format!("#details/partner/{key}")));
            }
        }
    }
    Some((ReferenceAnswer { text: lines.join("\n"), links }, focus))
}

pub(super) fn goal_at(before: &str) -> bool {
    let mut s = before.trim_end();
    for suffixes in [&["普通的", "普通", "normal", "regular"][..], &["一只", "一个", " a", " an", " the"][..]] {
        s = suffixes.iter().find_map(|v| s.strip_suffix(v)).unwrap_or(s).trim_end();
    }
    ["目标是", "目标为", "想得到", "想获得", "想孵", "孵出", "孵", "配出", "生出",
     "如果要", "如果想要", "如果想", "要的是", "想要", "目标子代是", "子代是", "后代是",
     "能得到", "能获得", "能不能得到", "会不会出", "能不能出", "目标想要",
     "to get", "can i get", "could i get", "would i get", "will i get", "give me",
     "i want", "to breed", "to hatch", "to produce", "trying for", "trying to get",
     "goal is", "offspring is", "produce"].iter().any(|word| s.ends_with(word))
}

pub(super) fn mate_choices(data: &Data, q: &str) -> Option<ReferenceAnswer> {
    let breeding = has(q, &["配偶", "亲本", "配种", "繁殖", "配对", "交配", "孵出", "breed", "offspring", "mate for", "mating"]);
    let choosing = has(q, &["候选", "挑", "选", "分别", "各自", "还是", "或", "配谁", "哪只", "哪一只",
        " or ", "each", "candidate", "choose", "which mate", "which partner"]);
    if !breeding || !choosing { return None; }
    let (spans, _) = species_mentions(&data.species, q);
    if spans.len() < 3 { return None; }
    let is_goal = |s: SpeciesSpan| goal_at(&q[..s.1]) && !(q[..s.1].trim_end().ends_with("to breed")
        && q[s.2..].trim_start().starts_with("with "));
    let mut goals = spans.iter().copied().filter(|s| is_goal(*s));
    let goal = goals.next().map(|s| s.0);
    if goals.any(|s| Some(s.0) != goal) { return None; }
    let parents: Vec<_> = spans.into_iter().filter(|s| !is_goal(*s)).collect();
    if parents.len() < 3 { return None; }
    // "Give A a mate from B and C", "breed A with B or C", or
    // "choose between B and C to breed with A". All retain the same fixed parent.
    let fixed = parents.iter().position(|(_, start, end)| {
        let before = q[..*start].trim_end();
        let after = q[*end..].trim_start();
        let before = ["my", "a", "这只", "一只"].iter().find_map(|p| before.strip_suffix(p)).unwrap_or(before).trim_end();
        ["给", "替", "作为固定亲本的是", "固定亲本是", "固定亲本", "mate for", "partner for", "breed with", "to pair with"]
            .iter().any(|v| before.ends_with(v))
        || ["分别跟", "分别与", "分别和", "分别配", "的配偶", "当固定", "为固定", "跟谁配", "with either ", "with one of "]
            .iter().any(|v| after.starts_with(v))
    }).or_else(|| {
        let (_, start, end) = parents[0];
        let prefix = q[..start].trim_end();
        let bridge = q[end..parents[1].1].trim();
        (["breed", "breed my", "breed a", "cross", "交配时用", "想用"].iter().any(|w| prefix.ends_with(w))
            && ["with", "with my", "with a", "跟", "与", "和"].contains(&bridge)).then_some(0)
    })?;
    let mut options = Vec::new();
    for (i, span) in parents.iter().copied().enumerate() {
        if i != fixed && !options.iter().any(|s: &SpeciesSpan| s.0 == span.0) { options.push(span); }
    }
    if options.len() < 2 || options.len() > 8 { return None; }
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let name = |id: usize| text(&data.species[id], if english { "en" } else { "zh" });
    let gendered = |(id, start, end): SpeciesSpan| (id, sex_by_name(&q[..start], &q[end..]));
    let parent = gendered(parents[fixed]);
    let mut session = BreedingSession { english, ..BreedingSession::default() };
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut matches = Vec::new();
    let display = offspring_fields(q).format(false);
    for option in options {
        let result = session.offspring(data, [parent, gendered(option)], display);
        if let Some(want) = goal {
            let route = format!("#lookup/parents/{}", text(&data.species[want], "key"));
            if result.links.iter().any(|(_, href)| href == &route) { matches.push(name(option.0)); }
        }
        lines.push(result.text);
        for link in result.links { if !links.contains(&link) { links.push(link); } }
    }
    if let Some(want) = goal {
        let label = name(want);
        let chosen = matches.join(if english { ", " } else { "、" });
        lines.push(match (english, matches.is_empty()) {
            (true,false) => format!("For {label}, the matching mate options with the sex assignments shown are {chosen}."),
            (true,true) => format!("None of these pairings matches {label} with the specified sexes."),
            (false,false) => format!("要得到{label}，按上列性别配置，可选的配偶是{chosen}。"),
            (false,true) => format!("以上候选与指定亲本、性别都配不出{label}。"),
        });
    }
    Some(display.visible(ReferenceAnswer { text: lines.join("\n"), links }))
}

pub(super) fn stage(d: &Data, q: &str) -> Option<ReferenceAnswer> {
    let first = q.split(['?', '？', '\n']).next()?;
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let hatched = has(q, &["已经孵", "已孵", "刚孵", "刚配出", "已经配出", "刚出生", "孵好的", "孵出来的",
        "bred pals", "newly bred", "after breeding", "already bred", "hatched", "newborn", "freshly bred"]);
    let train = has(first, &["练级", "练到", "升级", "升到", "刷经验", "培养等级", "level up", "level them",
        "level my", "level these", "level the", "leveling", "levelling", "gain exp", "gain xp", "train "]);
    if hatched && train {
        // Game survival guide: https://paldb.cc/en/Level_Up ; party battle EXP:
        // https://palworld.wiki.gg/wiki/Level . Bonuses use the shipped partner records.
        let mut lines = vec![if english {
            "Move several hatchlings from the Palbox to your active party. Keep one trained fighter or do the fighting yourself. Defeat or catch high-level Pals, or repeat an Alpha or tower boss you can finish quickly and reliably. The young Pals receive party battle EXP without needing to fight. At your target level, swap in the next batch. This works without training manuals."
        } else {
            "把要练的几只幼崽从仓库换进随身队伍，留一个位置给主力，或由玩家输出。反复挑战能快速稳定击败的野外头目、高塔首领，也可以击败或捕捉高等级野生帕鲁。幼崽待在队伍里就能分到战斗经验，不必上场。达到目标等级后再换下一批，训练书用完也能练。"
        }.to_owned()];
        let mut links = Vec::new();
        if let Some((key, row)) = d.get("partner-skills-data").as_object().into_iter().flatten().find(|(_, row)| {
            let effect = text(row, "effect");
            effect.contains("队伍帕鲁获得的经验值") && effect.contains("队伍中")
        }) && let Some(pal) = d.species.iter().find(|s| s["key"] == key.as_str()) {
            let name = text(pal, if english { "en" } else { "zh" });
            if english {
                lines.push(format!("If you have {name}, its partner skill boosts the party's Pal EXP while it occupies a party slot."));
            } else {
                lines.push(format!("还可以带{name}：「{}」{}",
                    text(row, "name"), text(row, "effect")));
            }
            links.push((name.into(), format!("#details/partner/{key}")));
        }
        return Some(ReferenceAnswer {text:lines.join("\n"),links});
    }
    if !has(first, &["how does breeding work", "how do i start breeding", "how to start breeding", "breeding basics",
                     "配种怎么玩", "配种入门", "如何开始配种", "配种流程", "配种怎么操作", "繁殖怎么操作", "配种是怎么回事"]) {return None;}
    let mut links = Vec::new();
    for id in ["Breeding_Farm", "Egg_Incubator"] {
        if let Some(facility) = array(&d.get("facilities-data")["facilities"]).iter().find(|f| f["id"] == id) {
            links.push((text(facility, "name").into(), format!("#work/facilities/{id}")));
        }
    }
    // In-game facility descriptions: https://paldb.cc/en/Pal_Breeding_Farm
    let answer = if english {
        "Build a Breeding Farm at your base. Assign a compatible male and female Pal, and place Cake in the farm's chest. When the egg appears, collect it, put it in an Egg Incubator and hatch it. To aim for a species, check its parent pairs; for a passive build, select the parents with the traits you want and check the offspring after hatching."
    } else {
        "在据点建配种牧场，把一公一母的适配亲本分配进去，将蛋糕放入牧场的箱子。等牧场产蛋后拾取帕鲁蛋，放入帕鲁蛋孵化器完成孵化。想孵指定物种就先查目标的双亲配方；想遗传指定词条，需要挑选带目标词条的亲本，再检查孵出的个体。"
    };
    Some(ReferenceAnswer {text:answer.into(),links})
}
