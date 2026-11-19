use super::{
    AssistantReply, Data, METRICS, has, mentioned_traits, modeled_line, modifier, percent_or_rank,
    related, reply, text,
};
use crate::assistant_breeding::species_mentions;

struct Binding<'a> {
    start: usize,
    effects_start: usize,
    label: &'a str,
}

fn bind<'a>(q: &str, start: usize, end: usize, label: &'a str) -> Option<Binding<'a>> {
    let after = q[end..].trim_start();
    let verbs = [
        "只有",
        "只带",
        "带着",
        "带了",
        "带",
        "身上有",
        "拥有",
        "有",
        "装了",
        "装备",
        "没有任何词条",
        "没带任何词条",
        "无词条",
        "没有词条",
        "没有被动",
        "携带",
        "的词条是",
        "的被动是",
        "的词条为",
        "配的是",
        "只装",
        "has only ",
        "only has ",
        "has ",
        "is running ",
        "with ",
        "runs ",
        "carries ",
        "gets ",
        "uses ",
        ":",
        "：",
    ];
    let verb = verbs.iter().find(|v| after.starts_with(**v))?;
    Some(Binding {
        start,
        effects_start: q.len() - after.len() + verb.len(),
        label,
    })
}

fn bindings<'a>(data: &'a Data, q: &str) -> Vec<Binding<'a>> {
    let mut marks = Vec::new();
    for (forms, label) in [
        (
            &[
                "第一只帕鲁",
                "第一只",
                "前一只",
                "前者",
                "一只帕鲁",
                "其中一只",
                "一只",
                "the first pal",
                "the first",
                "one pal",
                "one",
            ][..],
            "第一只",
        ),
        (
            &[
                "第二只帕鲁",
                "第二只",
                "后一只",
                "后者",
                "另一只帕鲁",
                "另一只",
                "the second pal",
                "the second",
                "the other pal",
                "the other",
                "another pal",
                "another",
            ],
            "第二只",
        ),
        (&["甲", "甲方", "a组", "a只", "帕鲁a", "pal a", "a"], "甲"),
        (&["乙", "乙方", "b组", "b只", "帕鲁b", "pal b", "b"], "乙"),
    ] {
        for form in forms {
            for (i, _) in q.match_indices(*form) {
                let end = i + form.len();
                if form.is_ascii()
                    && (q[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
                        || q[end..].starts_with(|c: char| c.is_ascii_alphanumeric()))
                {
                    continue;
                }
                if let Some(mark) = bind(q, i, end, label) {
                    marks.push(mark);
                }
            }
        }
    }
    for (idx, start, end) in species_mentions(&data.species, q).0 {
        if let Some(mark) = bind(q, start, end, text(&data.species[idx], "zh")) {
            marks.push(mark);
        }
    }
    marks.sort_by_key(|m| (m.start, std::cmp::Reverse(m.effects_start)));
    let mut selected = Vec::<Binding>::new();
    for mark in marks {
        if selected
            .last()
            .is_none_or(|prev| mark.start >= prev.effects_start)
        {
            selected.push(mark);
        }
    }
    selected
}

fn effect_list(data: &Data, ids: &[&str]) -> String {
    let rows = &data.get("pal-details-data")["passives"];
    ids.iter()
        .map(|id| {
            format!(
                "{}（{}）",
                text(&rows[*id], "zh"),
                text(&rows[*id], "effect")
            )
        })
        .collect::<Vec<_>>()
        .join("、")
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if has(q, &["遗传", "传给", "继承", "孵", "inherit", "breed"]) {
        return None;
    }
    if !has(
        q,
        &[
            "比",
            "谁",
            "哪只",
            "哪个",
            "差",
            "各",
            "合计",
            "总共",
            "多少",
            "快",
            "强",
            "which",
            "who",
            "faster",
            "compare",
            "difference",
            "total",
            "points",
            "how much",
        ],
    ) {
        return None;
    }
    let marks = bindings(data, q);
    let [left, right] = marks.as_slice() else {
        return None;
    };
    if left.label == right.label {
        return None;
    }
    let description = |span: &str| {
        let clause = span
            .split(['。', '；', ';', '!', '！', '?', '？', '\n'])
            .next()
            .unwrap_or("");
        let stop = [
            ", which",
            ", who",
            ", how",
            ". ",
            "只比较",
            "谁更",
            "谁快",
            "哪个更",
            "哪只更",
            "相差",
            "差多少",
            "差几个",
        ]
        .iter()
        .filter_map(|m| clause.find(m))
        .min()
        .unwrap_or(clause.len());
        stop
    };
    let spans = [
        &q[left.effects_start..right.start],
        &q[right.effects_start..],
    ];
    let spans = spans.map(|s| &s[..description(s)]);
    let two = spans.map(|s| mentioned_traits(data, s));
    if two.iter().all(Vec::is_empty) {
        return None;
    }
    let rows = &data.get("pal-details-data")["passives"];
    let mut links = Vec::new();
    for id in two.iter().flatten() {
        let link = (
            text(&rows[*id], "zh").into(),
            format!("#details/passive/trait/{id}"),
        );
        if !links.contains(&link) {
            links.push(link);
        }
    }
    let incomplete = (0..2).find(|&i| {
        let declared = &q[marks[i].start..marks[i].effects_start + spans[i].len()];
        let empty = has(
            declared,
            &[
                "无词条",
                "没有词条",
                "没有任何词条",
                "没带任何词条",
                "没有被动",
                "no traits",
                "no passives",
                "no passive",
                "none",
            ],
        );
        if two[i].is_empty() {
            return !empty;
        }
        // A conjunction with an unidentified member can change either total.
        ["和", "与", "、", "加上", "+", " and ", " & "]
            .iter()
            .any(|join| {
                spans[i].match_indices(*join).any(|(index, _)| {
                    let rhs = &spans[i][index + join.len()..];
                    let rhs = rhs
                        .split(['、', ',', '，', '+', '&'])
                        .next()
                        .unwrap_or("")
                        .trim();
                    !rhs.is_empty() && mentioned_traits(data, rhs).is_empty()
                })
            })
    });
    if let Some(index) = incomplete {
        return Some(reply(
            vec![format!(
                "没有认出{}实际装备的词条。请给出该只帕鲁的图鉴词条名，或说明它没有词条。",
                marks[index].label
            )],
            links,
        ));
    }
    let slots = data.get("trait-data")["maxSlots"].as_u64().unwrap_or(4) as usize;
    if let Some(i) = two.iter().position(|traits| traits.len() > slots) {
        return Some(reply(
            vec![format!(
                "{}列出了 {} 个不同词条，超出每只最多 {slots} 个词条的位置。请确认实际装备的组合。",
                marks[i].label,
                two[i].len()
            )],
            links,
        ));
    }
    let wanted: Vec<_> = METRICS.iter().filter(|m| has(q, m.terms)).collect();
    let secondary = has(
        q,
        &[
            "玩家",
            "骑手",
            "伤害",
            "冷却",
            "耐力",
            "抗性",
            "掉率",
            "掉落",
            "孵化",
            "player",
            "rider",
            "damage",
            "cooldown",
            "stamina",
            "resist",
            "drop rate",
        ],
    );
    let indirect_speed = wanted.is_empty() && has(q, &["跑得快", "谁快", "谁更快", "faster"]);
    let mut lines = Vec::new();
    for metric in METRICS {
        if secondary || indirect_speed && metric.key != "movement" {
            continue;
        }
        if !wanted.is_empty() && !wanted.iter().any(|m| m.key == metric.key) {
            continue;
        }
        if !two
            .iter()
            .flatten()
            .any(|id| modifier(data, id, metric.key).is_some())
        {
            continue;
        }
        let unresolved = two.iter().flatten().any(|id| {
            text(&rows[*id], "effect")
                .lines()
                .any(|line| related(metric, line) && !modeled_line(data, id, line))
        });
        if unresolved {
            for (i, traits) in two.iter().enumerate() {
                let effects = effect_list(data, traits);
                lines.push(format!("{}的词条说明：{effects}。", marks[i].label));
            }
            lines.push(format!(
                "{}涉及额外的生效条件或受益对象，无法从这组面板数值直接得出差值。",
                metric.title
            ));
            continue;
        }
        let parts = two.each_ref().map(|traits| {
            traits
                .iter()
                .filter_map(|id| {
                    modifier(data, id, metric.key).map(|v| (text(&rows[*id], "zh"), v))
                })
                .collect::<Vec<_>>()
        });
        let values = parts
            .each_ref()
            .map(|items| items.iter().map(|(_, v)| *v).sum::<i64>());
        for (index, traits) in parts.iter().enumerate() {
            let detail = traits
                .iter()
                .map(|(name, value)| format!("{name} {}", percent_or_rank(*value, metric.key)))
                .collect::<Vec<_>>()
                .join("、");
            let detail = if detail.is_empty() {
                "所列词条无此项加成"
            } else {
                &detail
            };
            lines.push(format!(
                "{}的{}词条加成 {}（{detail}）。",
                marks[index].label,
                metric.title,
                percent_or_rank(values[index], metric.key)
            ));
        }
        let diff = values[0].abs_diff(values[1]);
        let unit = if metric.key == "ranch" {
            "级"
        } else {
            "个百分点"
        };
        let comparison = if diff == 0 {
            format!("两者的{}词条加成相同，相差 0 {unit}。", metric.title)
        } else {
            let higher = if values[0] > values[1] {
                left.label
            } else {
                right.label
            };
            format!(
                "{higher}的{}词条加成更高，相差 {diff} {unit}。",
                metric.title
            )
        };
        lines.push(comparison);
        if metric.key == "movement" {
            lines.push(if diff == 0 {
                "基础移速和其他条件相同时，两只一样快。".into()
            } else {
                format!(
                    "基础移速和其他条件相同时，{}更快。",
                    if values[0] > values[1] {
                        left.label
                    } else {
                        right.label
                    }
                )
            });
        }
    }
    if lines.is_empty() {
        for (i, ids) in two.iter().enumerate() {
            let effects = effect_list(data, ids);
            lines.push(format!("{}带有{effects}。", marks[i].label));
        }
        lines.push("面板词条表没有足够的对应数值来计算所问效果的实际差值。".into());
    }
    lines.insert(0, "不同帕鲁分别计算自身的词条加成。".into());
    Some(reply(lines, links))
}
