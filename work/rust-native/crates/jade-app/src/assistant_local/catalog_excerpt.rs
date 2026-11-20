//! Give the player the part that can be reproduced from a named, offered catalog row.
use crate::assistant_engine::field_text;
use crate::assistant_engine::environmental_help::{only_phrases, temperature};
use crate::data::{Data, array, text};
mod partner;
pub(super) use partner::owners_match;

pub(super) struct Answer {
    pub text: String,
    pub complete: bool,
}

fn any(input: &str, terms: &[&str]) -> bool {
    terms.iter().any(|word| input.contains(word))
}


fn plain_item_use(q: &str, name: &str) -> bool {
    let Some((left, right)) = q.split_once(name) else { return false };
    let request = format!("{left}{right}");
    any(&request, &["有什么用", "什么作用", "有啥用", "什么效果", "做什么", "干什么", "用途", "有什么功能"])
        && only_phrases(&request, &["请问一下", "请问", "这个", "这种", "道具", "物品", "装备", "到底",
            "具体", "主要", "是", "能", "有", "的", "呢", "呀", "吗", "有什么用", "什么作用",
            "有啥用", "什么效果", "做什么", "干什么", "用途", "有什么功能"])
}

/// Read only a full description attached to its own inline label or adjacent ID header.
fn item(data: &Data, q: &str, facts: &str) -> Option<String> {
    let climate = temperature(q);
    let mut lines = facts.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((name, tail)) = line.trim().split_once("（物品）") else { continue };
        let name = name.trim();
        if name.is_empty() || (climate.is_none() && !plain_item_use(q, &field_text(name))) {
            continue;
        }
        let (offered, id) = match tail.trim() {
            inline if inline.starts_with('：') => (inline['：'.len_utf8()..].trim(), None),
            header if header.starts_with("· #items/") => (lines.peek().copied().unwrap_or("").trim(), header.strip_prefix("· #items/")),
            _ => continue,
        };
        if offered.is_empty() || offered.chars().count() > 650 { continue; }
        let record = array(&data.get("item-catalog-data")["items"]).iter().find(|r| {
            let key = text(r, "id");
            text(r, "name") == name && id.is_none_or(|wanted| wanted == key)
                && text(&data.get("item-effects-data")["items"][key], "description").trim() == offered
        });
        let Some(record) = record else { continue };
        if let Some(cold) = climate {
            let protection: &[&str] = if cold { &["抵御使身体冻僵", "为人驱逐寒意", "耐寒效果", "御寒", "耐寒性能"] }
                else { &["耐热性能", "耐热效果", "耐热、耐寒效果", "耐暑效果", "防暑", "抵御酷热"] };
            if !any(offered, protection) { continue; }
            return Some(format!("可以用{name}。{offered}"));
        }
        return Some(format!("{}：{offered}", text(record, "name")));
    }
    None
}

fn one(data: &Data, question: &str, facts: &str) -> Option<String> {
    let q = field_text(question);
    partner::answer(data, &q, facts).or_else(|| item(data, &q, facts))
}

fn partial(known: String, unknown: &str) -> Answer {
    let unknown = unknown.trim().trim_end_matches(['？', '?', '。']);
    let boundary = if unknown.chars().count() <= 65 { format!("现有条目还不能确认「{unknown}」。") }
        else { "另一问还缺少能直接核对的资料。".to_owned() };
    Answer { text: format!("{known}\n{boundary}"), complete: false }
}

pub(super) fn lookup(data: &Data, question: &str, facts: &str) -> Option<Answer> {
    if !crate::assistant_discovery::handbook_version_matches(question) { return None; }
    if let Some((title, context)) = question.trim().split_once('\n') {
        // The thread title names the skill owner. Opponents and teammates in the
        // body are context. A correction, historical version or a new question
        // in that body must not silently change the scope of a title-only reply.
        if any(&field_text(context), &["我说的是", "其实是问", "我指的是", "说错", "打错", "更正", "改问", "问的不是",
            "不是问", "不是在问", "不是这个", "笔误", "写错", "旧版", "以前版本", "模组", "mod", "补丁前",
            "改版前"]) { return None; }
        let known = partner::answer(data, &field_text(title), facts)?;
        return Some(if let Some((before, _)) = context.split_once(['？','?']) {
            partial(known, before.rsplit(['。','！','!','\n']).next().unwrap_or(before))
        } else { Answer { text: known, complete: true } });
    }
    let Some((first, second)) = super::passage_selection::parts(question) else {
        return one(data, question, facts).map(|text| Answer { text, complete: true });
    };
    let (known, unknown) = match (one(data, first, facts), one(data, second, facts)) {
        (Some(a), Some(b)) => return Some(Answer { text: if a == b { a } else { format!("{a}\n{b}") }, complete: true }),
        (Some(a), None) => (a, second),
        (None, Some(b)) => (b, first),
        (None, None) => return None,
    };
    Some(partial(known, unknown))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    fn handbook() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for key in ["item-catalog-data", "item-effects-data", "partner-skills-data"] {
            let bytes = std::fs::read(root.join(format!("{key}.json"))).unwrap();
            data.tables.insert(key.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        data
    }

    #[test]
    fn temperature_uses_only_a_matching_protective_item_and_goal() {
        let d = handbook();
        let cold = "耐寒毛皮盔甲（物品） · #items/FurArmorCold\n在寒冷地区生产的毛皮盔甲。 能够抵御使身体冻僵的寒意。";
        let hot = "耐热毛皮盔甲（物品）：在炎热地区生产的毛皮盔甲。 具备傲人的耐热性能。";
        let mixed = format!("{cold}\n{hot}");
        assert!(lookup(&d, "寒冷值怎麼辦？", &mixed).unwrap().text.contains("耐寒毛皮盔甲"));
        assert!(lookup(&d, "沙漠太热怎么办？", &mixed).unwrap().text.contains("耐热毛皮盔甲"));
        for q in ["孵化器太冷怎么办？", "冰属性攻击能用保暖装备挡住吗？", "拿什么给帕鲁降温？",
                  "要怎么同时防寒防暑？", "雪山太冷时什么温度必定不会掉血？"] {
            assert!(lookup(&d, q, &mixed).is_none(), "{q}");
        }
        assert!(lookup(&d, "寒冷值怎么办", hot).is_none());
        assert!(lookup(&d, "寒冷值怎么办", &cold.replace("FurArmorCold", "FurArmorHeat")).is_none());
        assert!(lookup(&d, "寒冷值怎么办", &cold.replace("耐寒毛皮盔甲", "耐热毛皮盔甲")).is_none());
    }

    #[test]
    fn named_item_and_partial_answers_keep_their_source_and_subject() {
        let d = handbook();
        let torch = "携带火把（物品） · #items/Torch\n照亮夜晚的黑暗，为人驱逐寒意。 可勉强用于近战。";
        assert!(lookup(&d, "携带火把有什么用？", torch).unwrap().text.starts_with("携带火把："));
        assert!(lookup(&d, "远古火把有什么用？", torch).is_none());
        assert!(lookup(&d, "火把卖多少钱？", torch).is_none());
        assert!(lookup(&d, "携带火把有什么用？", &torch.replace("驱逐寒意", "捕获帕鲁")).is_none());
        for q in ["太冷怎么办？这里一晚扣几滴血？", "这里一晚扣几滴血？太冷怎么办？"] {
            let answer = lookup(&d, q, torch).unwrap();
            assert!(!answer.complete && answer.text.contains("携带火把") && answer.text.contains("还不能确认「这里一晚扣几滴血」"));
        }
        let compound = lookup(&d, "携带火把有什么用，制作它还需要多少木材？", torch).unwrap();
        assert!(!compound.complete && compound.text.contains("驱逐寒意") && compound.text.contains("还不能确认"));
    }

    #[test]
    fn team_role_requires_the_requested_species_and_its_complete_partner_row() {
        let d = handbook();
        for (id, name) in [("PinkRabbit_Grass","艾小兔"), ("PinkCat","捣蛋猫")] {
            let skill = &d.get("partner-skills-data")[id];
            let row = format!("{name}（帕鲁）：伙伴技能「{}」：{}", text(skill, "name"), text(skill, "effect"));
            let q = format!("{name}在后期队伍里（困难模式和极限难度）是不是没什么用？");
            let answer = lookup(&d, &q, &row).unwrap();
            assert!(answer.text.contains(text(skill,"effect")));
            assert!(lookup(&d, &q, &row.replace(name, "棉悠悠")).is_none());
            assert!(lookup(&d, &format!("{name}在队伍里对钓鱼有什么作用？"), &row).is_none());
            assert!(lookup(&d, &format!("{name}和棉悠悠在队伍里哪个更有用？"), &row).is_none());
        }
    }

    #[test]
    fn thread_title_anchors_owner_even_when_opponents_appear_in_the_body() {
        let d = handbook();
        let skill = &d.get("partner-skills-data")["PinkRabbit_Grass"];
        let row = format!("[3] 艾小兔（帕鲁）：#044B · Ribbuny Botan；伙伴技能「暴力青草兔」：{}", text(skill,"effect"));
        let title = "【艾小兔】在后期队伍里（困难模式和极限难度）是不是没什么用？";
        let q = format!("{title}\n打头目阿努比斯和磐甲龙的时候我会带它。高塔我想腾一个队伍位置。");
        let answer = lookup(&d, &q, &row).unwrap();
        assert!(answer.complete && answer.text.contains("不弱草属性的对手") && !answer.text.contains("阿努比斯"));
        let extra = lookup(&d, &format!("{q}它的鞍具几级制作？"), &row).unwrap();
        assert!(!extra.complete && extra.text.contains("还不能确认"));
        let correction = format!("{title}\n我说的是捣蛋猫，标题写错了。");
        assert!(lookup(&d, &correction, &row).is_none());
        assert!(lookup(&d, &q, &row.replace("艾小兔（帕鲁）","阿努比斯（帕鲁）")).is_none());
    }

    #[test]
    fn active_mounted_and_party_conditions_stay_attached_to_the_offered_skill() {
        let d = handbook();
        for (id,q,condition) in [
            ("FlowerDoll","花丽娜的伙伴技能是随队就能回血，还是要主动发动？","发动后"),
            ("BlueDragon","碧海龙的伙伴技能必须骑乘才能触发吗？","骑乘期间玩家的攻击"),
            ("PinkCat","捣蛋猫的伙伴技能怎样生效？","若它在队伍中"),
        ] {
            let skill = &d.get("partner-skills-data")[id];
            let name = text(skill,"name");
            let effect = text(skill,"effect");
            let row = format!("[2] {name}（伙伴技能） · #details/partner/{id}\n{effect}");
            assert!(lookup(&d, q, &row).unwrap().text.contains(condition));
            assert!(lookup(&d, &format!("「{name}」怎么触发？"), &row).unwrap().text.contains(condition));
            assert!(lookup(&d, q, &row.replace(&format!("/{id}"),"/SheepBall")).is_none());
            let unrelated_owner = format!("棉悠悠（帕鲁）：伙伴技能「{name}」：{effect}");
            assert!(lookup(&d, q, &unrelated_owner).is_none());
        }
        let shared = "炸鱿鱼怎么触发？";
        assert!(lookup(&d, shared, "炸鱿鱼（伙伴技能） · #details/partner/One\n记录").is_none());
        assert!(!owners_match(&d, "棉悠悠的伙伴技能「暴力青草兔」可以给你加成。"));
        assert!(owners_match(&d, "棉悠悠的伙伴技能不是「暴力青草兔」，艾小兔的伙伴技能是「暴力青草兔」。"));
    }
}
