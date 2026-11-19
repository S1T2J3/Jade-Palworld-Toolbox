use super::{JOBS, mentions, quantity_label};
use crate::assistant_breeding::species_mentions;
use crate::assistant_engine::AssistantReply;
use crate::data::{Data, text};

// A base job and a partner skill that interacts with the same resource use
// different conditions and units. Include both when the handbook confirms both.
fn resource_skill(data: &Data, key: &str, job: &str, en: bool) -> Option<String> {
    let ability = &data.get("partner-skills-data")[key];
    let effect = text(ability, "effect");
    let (goal, target) = match job {
        "采矿" => ("矿石", "ore"),
        "伐木" => ("树木", "trees"),
        _ => return None,
    };
    let active = if effect.contains("骑乘期间") {
        "While mounted"
    } else if effect.contains("发动后") && effect.contains("甲壳回旋") {
        "With its spinning shell partner skill active"
    } else {
        return None;
    };
    let clauses: Vec<_> = effect
        .split(['，', '。'])
        .filter(|s| s.contains(goal))
        .collect();
    if clauses.is_empty() {
        return None;
    }
    if !en {
        return Some(format!(
            "野外采集时，伙伴技能「{}」的条件与效果为：{effect}",
            text(ability, "name")
        ));
    }
    let mut details = Vec::new();
    for clause in clauses {
        let range = clause.split_once('(')?.1.split_once(")%")?.0;
        let (low, high) = range.split_once('~')?;
        let (low, high) = (low.parse::<u32>().ok()?, high.parse::<u32>().ok()?);
        let what = if clause.contains("金属矿石的掉落量") {
            "metal ore yield".to_owned()
        } else if clause.contains("伤害") {
            format!("damage to {target}")
        } else if clause.contains("效率") {
            format!("efficiency at breaking {target}")
        } else {
            return None;
        };
        details.push(format!(
            "{what} increases by {low}–{high}% across partner skill levels"
        ));
    }
    Some(format!(
        "{active}, {}. The base work rank applies separately.",
        details.join(" and ")
    ))
}

pub(in crate::assistant_engine) fn individual_job(data: &Data, q: &str) -> Option<AssistantReply> {
    let roles: Vec<_> = JOBS
        .iter()
        .filter(|(role, _, terms)| {
            mentions(q, terms)
                || match *role {
                    "采矿" => mentions(
                        q,
                        &["mine ore", "mines ore", "for mining", "挖石头", "挖铁矿"],
                    ),
                    "伐木" => mentions(q, &["cut wood", "chop wood", "chop trees", "砍木头"]),
                    "发电" => mentions(q, &["generate electricity", "generate power", "供电"]),
                    "搬运" => mentions(q, &["haul items", "haul resources", "运货", "搬东西"]),
                    _ => false,
                }
        })
        .collect();
    if roles.is_empty()
        || !mentions(
            q,
            &[
                "can",
                "does",
                "able",
                "good",
                "useful",
                "suitable",
                "worth",
                "use",
                "worker",
                "suitability",
                "job",
                "jobs",
                "which level",
                "what rank",
                "how well",
                "at night",
                "能",
                "会",
                "适合",
                "适性",
                "适应性",
                "几级",
                "多少级",
                "好用",
                "怎么样",
                "值得",
                "厉害",
                "好不好",
                "打工",
                "晚上",
                "夜间",
            ],
        )
    {
        return None;
    }
    if mentions(
        q,
        &[
            "where",
            "wherever",
            "breed",
            "breeding",
            "passive",
            "passives",
            "per hour",
            "per minute",
            "how many",
            "how often",
            "manual",
            "condense",
            "condensed",
            "boss",
            "team",
            "replace",
            "best",
            "better",
            "fastest",
            "who else",
            "another pal",
            "other pals",
            "catch",
            "capture",
            "find",
            "location",
            "unlock",
            "saddle",
            "schematic",
            "food",
            "feed",
            "skill fruit",
            "build",
            "carry weight",
            "attack",
            "combat",
            "station",
            "cooldown",
            "recipe",
            "drop rate",
            "what level do i",
            "what level can i unlock",
            "stuck",
            "stopped",
            "refuses",
            "won't",
            "doesn't",
            "why",
            "配种",
            "培育",
            "词条",
            "每小时",
            "每分钟",
            "工位",
            "设施",
            "在哪",
            "哪里",
            "浓缩",
            "手册",
            "冷却时间",
            "配方",
            "招式",
            "战斗",
            "掉率",
            "队伍",
            "最适合",
            "哪只更",
            "替代",
            "推荐其他",
            "还有哪些",
            "怎么分工",
            "为什么",
            "不肯",
            "卡住",
            "不干活",
            "无法分配",
            "升到",
            "提升",
            "提高",
            "捕捉",
            "抓到",
            "鞍具",
            "解锁",
            "图纸",
            "食物",
            "负重",
        ],
    ) || q.contains('%')
    {
        return None;
    }
    let (subjects, _) = species_mentions(&data.species, q);
    let &(index, start, _) = subjects.first()?;
    if subjects.iter().any(|x| x.0 != index)
        || crate::assistant_breeding::excluded_record_subject(&q[..start])
    {
        return None;
    }
    let pal = &data.species[index];
    let key = text(pal, "key");
    let profile = &data.get("paldex-index-data")["profiles"][key];
    let work = profile["work"].as_array()?;
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let name = text(pal, if en { "en" } else { "zh" });
    let mut lines = Vec::new();
    let mut links = vec![(name.to_string(), format!("#work/suitability/{key}"))];
    for &(job, label, _) in roles {
        if let Some(row) = work.iter().find(|r| r[0] == job) {
            let rank = row[1].as_u64()?;
            let value = data.get("work-efficiency-data")["levels"][job]
                .get(usize::try_from(rank.checked_sub(1)?).ok()?)?
                .as_u64()?;
            let unit = quantity_label(job, en);
            lines.push(if en {
                format!("{name} has natural {label} Lv.{rank} ({unit}: {value}).")
            } else {
                format!("{name}在据点的基础{job}适性为 Lv.{rank}（{unit} {value}）。")
            });
        } else {
            lines.push(if en {
                format!("{name} has no natural {label} suitability for base work.")
            } else {
                format!("{name}没有据点{job}的基础适性。")
            });
        }
        if let Some(skill) = resource_skill(data, key, job, en) {
            if !lines.contains(&skill) {
                lines.push(skill);
            }
            if links.len() == 1 {
                links.push((
                    text(&data.get("partner-skills-data")[key], "name").into(),
                    format!("#details/partner/{key}"),
                ));
            }
        }
    }
    if let Some(night) = profile["nocturnal"].as_bool()
        && (night || mentions(q, &["night", "overnight", "晚上", "夜间", "夜行"]))
    {
        lines.push(
            match (en, night) {
                (true, true) => "This species is naturally nocturnal and can work at night.",
                (true, false) => {
                    "This species is not naturally nocturnal and normally sleeps at night."
                }
                (false, true) => "这个物种天生夜行，可以在夜间工作。",
                (false, false) => "这个物种天生需要夜间睡眠。",
            }
            .into(),
        );
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}
