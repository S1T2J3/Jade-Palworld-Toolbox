use super::{any, only_phrases};
use crate::assistant_engine::field_text;
use crate::data::{Data, text};
use serde_json::Value;

fn owner<'a>(data: &'a Data, q: &str) -> Option<&'a Value> {
    let (mentions, _) = crate::assistant_breeding::species_mentions(&data.species, q);
    if let Some(&(first, _, _)) = mentions.first() {
        return mentions.iter().all(|m| m.0 == first).then(|| &data.species[first]);
    }
    // Several species can share the same displayed ability name. Without an
    // owner, only an unambiguous name can identify the origin of its effect.
    let skills = data.get("partner-skills-data");
    let mut matches = data.species.iter().filter(|pal| {
        let name = text(&skills[text(pal, "key")], "name");
        !name.is_empty() && q.contains(&field_text(name))
    });
    let single = matches.next()?;
    matches.next().is_none().then_some(single)
}

fn unnumber(line: &str) -> &str {
    let line = line.trim();
    if let Some((n, body)) = line.strip_prefix('[').and_then(|s| s.split_once(']'))
        && !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) { return body.trim_start(); }
    line
}

fn offered(facts: &str, species: &Value, name: &str, effect: &str) -> bool {
    let zh = text(species, "zh");
    let id = text(species, "key");
    let dex = text(species, "dex");
    let en = text(species, "en");
    let skill_field = format!("伙伴技能「{name}」：{effect}");
    let title = format!("{zh}（帕鲁）：");
    let partner = format!("{zh}的{skill_field}");
    let catalog = format!("{zh} · #{dex} · {name}（伙伴技能）：{effect}");
    let description = format!("{zh} ({en}) 随队时可用伙伴技能「{name}」。{effect}");
    let header = format!("{name}（伙伴技能） · #details/partner/{id}");
    let mut lines = facts.lines().map(unnumber).peekable();
    while let Some(line) = lines.next() {
        if line == partner || line == catalog || line == description || (line == header && lines.peek() == Some(&effect)) { return true; }
        if let Some(fields) = line.strip_prefix(&title)
            && fields.strip_suffix(&skill_field).is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('；')) { return true; }
    }
    false
}

fn party_question(q: &str) -> bool {
    any(q, &["队伍", "队里", "随队", "带队", "队中"])
        && any(q, &["有用", "没用", "没什么用", "作用", "值不值", "值得", "能做什么", "有啥用"])
        && only_phrases(q, &["在", "带", "着", "放进", "放", "到", "进", "我的", "我", "一个", "它",
            "队伍", "队里", "队中", "随队", "带队", "里", "中", "后期", "前期", "中期", "困难模式", "极限难度",
            "普通模式", "困难", "极限", "模式", "难度", "和", "或", "打", "玩", "还有", "还", "有用", "没用",
            "没什么用", "作用", "值不值", "值得", "能做什么", "有啥用", "是不是", "有什么", "什么", "吗", "呢", "的"])
}

fn trigger_question(q: &str, effect: &str) -> bool {
    if !any(q, &["触发", "发动", "生效", "激活", "主动", "按键"]) { return false; }
    let mut request = q.to_owned();
    let goals: &[(&[&str], &[&str])] = &[
        (&["治疗", "回血", "加血"], &["治疗", "生命值恢复", "恢复生命值"]),
        (&["增伤", "伤害加成"], &["伤害提升", "伤害会增加", "攻击将提升"]),
        (&["加负重", "增加负重", "提高负重"], &["负重"]),
        (&["加防御", "增加防御"], &["防御"]),
        (&["加掉落", "增加掉落"], &["掉落道具增加"]),
    ];
    for &(aliases, source_words) in goals {
        if any(effect, source_words) {
            for word in aliases { request = request.replace(word, ""); }
        }
    }
    only_phrases(&request, &["请问一下", "请问", "它", "它的", "的", "这", "个", "这个", "技能", "伙伴技能", "伙伴能力",
            "怎样", "怎么", "如何", "哪些", "什么", "什么时候", "条件", "触发", "发动", "生效", "激活",
            "主动", "被动", "手动", "自动", "按键", "按", "下", "常驻", "一直", "在", "里", "队伍", "队中",
            "队里", "随队", "带在", "带着", "上场", "出战", "并肩作战", "骑乘", "骑着", "骑上", "骑",
            "必须", "只要", "是不是", "能不能", "是否", "才", "是", "要", "需要", "就", "能", "会", "才能",
            "还是", "然后", "时", "后", "吗", "呢", "呀", "或", "和", "与", "一定", "用", "得"])
}

/// Check explicit owner-to-ability claims in generated prose. A target or a
/// beneficiary mentioned in the same passage is not necessarily its owner.
pub(in crate::assistant_local) fn owners_match(data: &Data, answer: &str) -> bool {
    let normalized = field_text(answer);
    for sentence in normalized.split(['。', '!', '?', ',', ';', '\n']) {
        for marker in ["的伙伴技能「", "的伙伴技能是「", "的伙伴技能为「", "的伙伴技能名为「"] {
            for (i, _) in sentence.match_indices(marker) {
                let prefix = sentence[..i].trim_end_matches(['」','】',']',' ']);
                let Some(pal) = data.species.iter().filter(|p| {
                    let name = text(p, "zh");
                    !name.is_empty() && prefix.ends_with(name)
                }).max_by_key(|p| text(p, "zh").len()) else { continue };
                let Some((claimed, _)) = sentence[i+marker.len()..].split_once('」') else { continue };
                let row = &data.get("partner-skills-data")[text(pal,"key")];
                if let Some(name) = row["name"].as_str() && field_text(claimed) != field_text(name) { return false; }
            }
        }
    }
    true
}

pub(super) fn answer(data: &Data, q: &str, facts: &str) -> Option<String> {
    if !any(q, &["队", "伙伴", "触发", "发动", "生效", "激活", "主动", "按键"]) { return None; }
    let pal = owner(data, q)?;
    let skill = &data.get("partner-skills-data")[text(pal, "key")];
    let name = skill["name"].as_str()?;
    let effect = skill["effect"].as_str()?.trim();
    let zh = text(pal, "zh");
    let mut rest = q.replace(&field_text(name), "");
    for field in ["zh", "en", "key"] {
        let alias = field_text(text(pal, field));
        if !alias.is_empty() { rest = rest.replace(&alias, ""); }
    }
    let party = party_question(&rest);
    if !(party || trigger_question(&rest, effect)) || effect.is_empty() || !offered(facts, pal, name, effect) { return None; }
    let mut answer = format!("{zh}的伙伴技能「{name}」：{effect}");
    if party {
        if let Some((_, tail)) = effect.split_once('以')
            && let Some((element, _)) = tail.split_once("属性攻击克制的属性时伤害提升")
            && ["草","火","水","雷","地","冰","暗","龙","无"].contains(&element) {
            answer.push_str(&format!("\n遇到不弱{element}属性的对手，{element}属性招式打不出克制，这项随队增伤也就发挥不了。需要打出这种克制时再考虑留队。"));
        } else if any(effect, &["将它分派到", "若它在据点"]) && !any(effect, &["队伍","发动后","骑乘"]) {
            answer.push_str("\n这个条目记录的是据点工作效果，未记载只放进随身队伍便有该效果。");
        } else {
            answer.push_str("\n选择队伍位置时，可以看你是否需要这项效果，以及能否满足条目写明的条件。");
        }
    }
    Some(answer)
}
