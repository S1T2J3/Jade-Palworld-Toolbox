use super::*;
use crate::assistant_breeding::species_mentions;

fn average(n: u64, rate: u64) -> String {
    let product = u128::from(n) * u128::from(rate);
    let s = format!("{}.{:02}", product / 100, product % 100);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "平均",
            "期望",
            "预期",
            "保底",
            "保证",
            "一定",
            "必出",
            "expected",
            "on average",
            "guarantee",
            "expect",
        ],
    ) || !has(q, &["蛋", "帕鲁球", "egg", "sphere"])
    {
        return None;
    }
    let (mentions, _) = species_mentions(&d.species, q);
    let mut ids: Vec<_> = mentions
        .iter()
        .map(|(i, ..)| text(&d.species[*i], "key"))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let [id] = ids.as_slice() else { return None };
    let skill = &d.get("partner-skills-data")[*id];
    let effect = text(skill, "effect");
    let curves = skill["curves"].as_object()?;
    let (marker, values) = curves
        .iter()
        .find(|(key, _)| effect.contains(&format!("{key}%的概率")))?;
    let (_, outcome) = effect.split_once(&format!("{marker}%的概率"))?;
    let outcome = outcome.split('。').next()?;
    // Read the trigger and result from the same documented probability clause.
    let (touch, units, label) =
        if outcome.contains("头目帕鲁蛋") && has(q, &["头目", "alpha", "boss egg"]) {
            (
                &["捡", "拾", "撿", "捞", "pick", "collect"][..],
                &["颗", "枚", "个", "egg", "qualifying egg"][..],
                "变为头目蛋",
            )
        } else if outcome.contains("再捡到")
            && has(
                q,
                &[
                    "额外",
                    "多捡",
                    "多拿",
                    "多得",
                    "再捡",
                    "加起来",
                    "一共",
                    "extra",
                    "additional",
                    "total",
                ],
            )
        {
            (
                &["捡", "拾", "pick", "collect"][..],
                &["颗", "枚", "个", "egg", "qualifying egg"][..],
                "额外得到蛋",
            )
        } else if outcome.contains("不")
            && outcome.contains("消耗帕鲁球")
            && has(q, &["投", "扔", "丢", "掷", "throw", "throws", "toss"])
        {
            (
                &["投", "扔", "丢", "掷", "throw", "toss"][..],
                &["个", "枚", "颗", "次", "sphere", "throw", "attempt"][..],
                "免消耗帕鲁球",
            )
        } else {
            return None;
        };
    let q = rank_text(q);
    let nums = integers(&q);
    let mut levels = ranks(&q);
    levels.retain(|n| {
        !["玩家", "角色", "我", "player level", "i'm level"]
            .iter()
            .any(|s| q[..n.start].trim_end().ends_with(s))
    });
    if !has(
        &q,
        &[
            "伙伴技能",
            "搭档技能",
            "伙伴技",
            "技能等级",
            "级技能",
            "级的技能",
            "rank of its skill",
            "skill level",
            "partner skill",
            "partner ability",
        ],
    ) {
        levels.clear();
    }
    let mut rank = match levels.as_slice() {
        [n] => Some(n.value),
        [] => None,
        _ => return None,
    };
    if rank.is_none() {
        for n in &nums {
            if ["星", "★", "star"]
                .iter()
                .any(|s| q[n.end..].trim_start_matches([' ', '-']).starts_with(s))
            {
                if rank.is_some() {
                    return None;
                }
                rank = n.value.checked_add(1);
            }
        }
        if has(
            &q,
            &[
                "满星",
                "满级伙伴技能",
                "max rank",
                "max partner",
                "fully condensed",
            ],
        ) {
            rank = Some(5);
        }
    }
    let link = vec![(
        text(skill, "name").to_owned(),
        format!("#details/partner/{id}"),
    )];
    let mut counts = Vec::new();
    for n in &nums {
        if levels.iter().any(|l| l.start == n.start) {
            continue;
        }
        let before = q[..n.start]
            .rsplit(['。', '，', ',', ';', '；', '?', '？'])
            .next()?
            .trim();
        let after = q[n.end..].trim_start();
        if units.iter().any(|s| after.starts_with(s))
            && (has(before, touch)
                || has(
                    after
                        .split(['。', '，', ',', ';', '；', '?', '？'])
                        .next()?,
                    touch,
                ))
        {
            counts.push(n.value);
        }
    }
    let [n] = counts.as_slice() else {
        return None;
    };
    let owner = d.pal_name(id);
    let name = text(skill, "name");
    if has(
        &q,
        &[
            "没带进队",
            "没带在队",
            "没在队",
            "不在队",
            "不带它",
            "left at base",
            "not in my party",
        ],
    ) {
        return Some(reply(
            format!(
                "{owner}的「{name}」要求它在队伍中。这次没有带上它，该技能提供的额外触发次数为 0。"
            ),
            link,
        ));
    }
    if has(
        &q,
        &[
            "配种场",
            "配种牧场",
            "孵化",
            "孵蛋器",
            "breeding farm",
            "incubator",
            "hatching",
        ],
    ) && !has(&q, &["野外", "wild"])
    {
        return None;
    }
    let Some(rank) = rank else {
        return Some(reply(
            format!(
                "{owner}的「{name}」会随伙伴技能等级改变触发概率。你打算用 Lv.1–5 的哪一级来算这 {n} 次？"
            ),
            link,
        ));
    };
    let Some(rate) = rank
        .checked_sub(1)
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| values.get(i))
        .and_then(Value::as_u64)
        .filter(|v| *v <= 100)
    else {
        return Some(reply(
            "伙伴技能数据覆盖 Lv.1–5，对应浓缩 0–4 星。请确认技能等级。".into(),
            link,
        ));
    };
    let mut msg = format!(
        "{owner}在队伍中时，「{name}」Lv.{rank} 每次符合条件的操作有 {rate}% 概率{label}。{n} 次的期望为 {n} × {rate}% = {} 次。",
        average(*n, rate)
    );
    if *n == 0 || rate == 0 || rate == 100 {
        msg.push_str(&format!("按此概率实际触发 {} 次。", average(*n, rate)));
    } else {
        msg.push_str(&format!("这是平均值，不保证单次做完就拿到这个数量。图鉴没有列出保底次数，可能一次都不触发，最多可触发 {n} 次。"));
    }
    if effect.contains("不可叠加")
        && nums
            .iter()
            .any(|v| v.value > 1 && q[v.end..].trim_start().starts_with('只'))
    {
        msg.push_str("该效果标注为不可叠加，多带同种伙伴不会按只数叠乘上述概率。");
    }
    if outcome.contains("再捡到")
        && has(
            &q,
            &["一共", "合计", "总共", "加起来", "in total", "total eggs"],
        )
    {
        msg.push_str(&format!(
            "加上本来捡到的 {n} 颗，一共期望 {} 颗。",
            average(*n, 100 + rate)
        ));
    }
    if outcome.contains("消耗帕鲁球")
        && has(
            &q,
            &[
                "实际",
                "用掉",
                "会消耗",
                "总消耗",
                "会用掉",
                "consumed",
                "use up",
            ],
        )
    {
        msg.push_str(&format!(
            "实际消耗帕鲁球的期望为 {} 个。",
            average(*n, 100 - rate)
        ));
    }
    Some(reply(msg, link))
}
