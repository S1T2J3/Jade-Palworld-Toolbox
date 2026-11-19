use crate::assistant_engine::field_text;

fn canonical(s: &str) -> String {
    let mut s = field_text(s);
    if let Some(target) = s
        .strip_prefix("只有")
        .and_then(|s| s.strip_suffix("才有效"))
    {
        s = format!("仅对{target}有效");
    }
    for (from, to) in [
        ("唯一的", "唯一"),
        ("唯一方法", "唯一方式"),
        ("唯一办法", "唯一方式"),
        ("唯一途径", "唯一方式"),
        ("仅能", "只能"),
        ("只可以", "只能"),
        ("仅可以", "只能"),
        ("仅可", "只能"),
        ("其它", "其他"),
        ("没有别的", "没有其他"),
        ("不存在其他", "没有其他"),
        ("sole method", "only way"),
        ("only method", "only way"),
        ("sole way", "only way"),
        ("sole source", "only source"),
        ("exclusively", "only"),
        ("can only be obtained", "can only be acquired"),
    ] {
        s = s.replace(from, to);
    }
    s.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '+' | '-' | '%' | '<' | '>'))
        .collect()
}

fn claim(s: &str) -> bool {
    if [
        "无法确认",
        "无法证实",
        "没有证据",
        "没有足够",
        "不能断定",
        "不能据此",
        "不能排除",
        "不代表",
        "并非",
        "不是唯一",
        "无法确定",
        "不能确认",
        "尚不清楚",
        "无法判断",
        "不确定",
        "是否存在",
        "是否为唯一",
        "有没有",
        "未提及",
        "未收录",
        "没有列",
        "未列出",
        "资料未",
        "文档未",
        "资料没有",
        "资料中没有",
        "尚未证实",
        "cannot confirm",
        "cannot rule out",
        "not the only",
        "not necessarily",
        "no evidence",
        "insufficient evidence",
        "not documented",
        "does not list",
        "does not mention",
        "not enough information",
        "no information",
        "no indication",
        "no evidence",
        "unclear whether",
        "unknown whether",
    ]
    .iter()
    .any(|marker| s.contains(marker))
    {
        return false;
    }
    [
        "唯一",
        "只能",
        "仅能",
        "仅可",
        "仅有",
        "只有",
        "仅限",
        "仅对",
        "没有其他",
        "没有别的",
        "再无其他",
        "别无",
        "除此之外没有",
        "不存在",
        "没有这种",
        "没这种",
        "没有这个机制",
        "无此机制",
        "不支持",
        "没有任何",
        "没有办法",
        "没有方法",
        "无法通过任何",
        "任何方式都不能",
        "不能以任何",
        "一律不能",
        "不能调整",
        "无法调整",
        "无法设置",
        "无法更改",
        "不允许调整",
        "并不提供",
        "没有设置",
        "没有药",
        "没有道具",
        "没有该选项",
        "the only ",
        "only way",
        "only method",
        "only option",
        "only source",
        "only available",
        "only possible",
        "can only",
        "possible only",
        "sole ",
        "exclusively",
        "no other",
        "no alternative",
        "no such ",
        "there is no ",
        "there are no ",
        "there isn't ",
        "there aren't ",
        "no medicine can",
        "no item can",
        "has no option",
        "not possible to",
        "no way to",
        "cannot be adjusted",
        "cannot be changed",
        "cannot be set",
        "cannot be obtained",
        "can't be changed",
        "does not exist",
        "doesn't exist",
        "does not support",
        "not a feature",
    ]
    .iter()
    .any(|marker| s.contains(marker))
        || s.starts_with("only ")
        || (["没有", "并无", "没法", "并未提供"]
            .iter()
            .any(|marker| s.contains(marker))
            && [
                "机制", "途径", "办法", "设定", "设置", "调整", "选项", "功能", "方法", "道具",
                "药品",
            ]
            .iter()
            .any(|thing| s.contains(thing)))
}

// Keep clauses intact, except at transitions that introduce an independently
// asserted conclusion. An uncertainty in a preceding sentence isn't evidence.
fn clauses(s: &str) -> impl Iterator<Item = &str> {
    s.split(['。', '！', '？', '!', '?', ';', '；', '\n', '.', '，', ','])
        .map(str::trim)
        .filter(|c| !c.is_empty())
}

pub(super) fn unsupported(answer: &str, facts: &str) -> bool {
    let answer = field_text(answer);
    let facts = field_text(facts);
    clauses(&answer).any(|s| {
        if !claim(s) {
            return false;
        }
        let target = canonical(
            s.trim_start_matches("根据资料")
                .trim_start_matches("因此")
                .trim_start_matches("所以"),
        );
        !clauses(&facts).any(|record| claim(record) && canonical(record).contains(&target))
    })
}

/// If a grounded answer appended an unwarranted "only way", keep its actual
/// cited positive statements. Do not turn a rejected negative into a positive.
pub(super) fn positive_options(answer: &str, records: &[&str]) -> Option<String> {
    let records = records.join("\n");
    if !unsupported(answer, &records) {
        return None;
    }
    let evidence = field_text(&records);
    let mut kept = Vec::new();
    for line in answer
        .split(['。', '！', '？', '!', '?', ';', '；', '\n', '.'])
        .map(str::trim)
    {
        let normal = field_text(line);
        if normal.chars().count() < 7 || clauses(&normal).any(claim) {
            continue;
        }
        let needle = canonical(&normal);
        if clauses(&evidence).any(|source| canonical(source) == needle) && !kept.contains(&line) {
            kept.push(line);
        }
    }
    if kept.is_empty() {
        return None;
    }
    let punctuation = if answer.is_ascii() { ". " } else { "。" };
    Some(format!(
        "{}{}",
        kept.join(punctuation),
        punctuation.trim_end()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_entry_does_not_establish_exclusivity_or_nonexistence() {
        let source =
            "Put an incapacitated Pal into the Palbox to recover.\n据点帕鲁可以去饲料箱吃饭。";
        for claim in [
            "Putting it in a Palbox is your only option.",
            "There are no revival items in the game.",
            "There is no option to adjust the timer.",
            "供餐的唯一办法是饲料箱。",
            "原版没有加快复活的设置。",
            "只能等它自己恢复。",
        ] {
            assert!(unsupported(claim, source), "{claim}");
        }
        assert!(unsupported(
            "专用饲料只能喂给帕鲁。",
            "资料无法确认专用饲料只能喂给帕鲁。"
        ));
        assert!(!unsupported("尚未证实有没有调整复活时间的选项。", source));
        assert!(!unsupported("This is not the only way to recover.", source));
        assert!(!unsupported(
            "这种物品仅能在夜间使用。",
            "这种物品只能在夜间使用。"
        ));
        assert!(!unsupported(
            "This station is the sole source of amber.",
            "This station is the only source of amber."
        ));
        assert!(unsupported(
            "This station is the sole source of amber.",
            "This station is the only source of honey."
        ));
        assert_eq!(
            positive_options(
                "据点帕鲁可以去饲料箱吃饭。除此之外没有其他供餐办法。",
                &[source]
            )
            .unwrap(),
            "据点帕鲁可以去饲料箱吃饭。"
        );
        assert!(positive_options("游戏没有其他复活办法。", &[source]).is_none());
    }
}
