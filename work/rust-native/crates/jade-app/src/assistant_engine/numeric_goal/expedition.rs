use super::*;

fn all_spans<'a>(
    q: &str,
    labels: impl Iterator<Item = (&'a str, &'a Value)>,
) -> Vec<(usize, usize, &'a Value)> {
    let mut hits = Vec::new();
    for (raw, row) in labels {
        let name = field_text(raw);
        if name.is_empty() {
            continue;
        }
        for (i, _) in q.match_indices(&name) {
            let mut end = i + name.len();
            if name.is_ascii()
                && q[end..].starts_with('s')
                && !q[end + 1..].starts_with(|c: char| c.is_ascii_alphabetic())
            {
                end += 1;
            }
            if name.is_ascii()
                && (q[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
                    || q[end..].starts_with(|c: char| c.is_ascii_alphanumeric()))
            {
                continue;
            }
            hits.push((i, end, row));
        }
    }
    hits.sort_by_key(|(i, e, _)| (*i, std::cmp::Reverse(*e - *i)));
    let mut end = 0;
    hits.retain(|(s, e, _)| {
        if *s < end {
            return false;
        }
        end = *e;
        true
    });
    hits
}

fn repetitions(q: &str, start: usize, end: usize, nums: &[Quantity]) -> Option<u64> {
    let after = q[end..]
        .trim_start()
        .strip_prefix("expedition")
        .unwrap_or(q[end..].trim_start())
        .trim_start();
    for (label, num) in [("once", 1), ("twice", 2), ("three times", 3)] {
        if after.starts_with(label) {
            return Some(num);
        }
    }
    for n in nums.iter().rev().filter(|n| n.end <= start) {
        let span = q[n.end..start].trim();
        if matches!(
            span,
            "次" | "趟"
                | "轮"
                | "把"
                | "次普通"
                | "趟普通"
                | "次困难"
                | "run of"
                | "runs of"
                | "x"
                | "×"
        ) || span.is_empty() && q[end..].trim_start().starts_with("expedition")
        {
            return Some(n.value);
        }
    }
    for n in nums.iter().filter(|n| n.start >= end) {
        if [
            "",
            "x",
            "×",
            "刷",
            "跑",
            "完成",
            "各",
            "各刷",
            "刷了",
            "跑了",
            "完成了",
            "打了",
            "远征",
            "expedition",
            "expedition x",
        ]
        .contains(&q[end..n.start].trim())
            && ["次", "趟", "轮", "把", "run", "time"]
                .iter()
                .any(|s| q[n.end..].trim_start().starts_with(s))
        {
            return Some(n.value);
        }
    }
    None
}

fn item_mentions<'a>(catalog: &'a [Value], q: &str) -> Vec<&'a Value> {
    let mut ids = Vec::new();
    let labels = catalog
        .iter()
        .flat_map(|row| {
            [text(row, "name"), text(row, "en")]
                .into_iter()
                .map(move |s| (s, row))
        })
        .filter(|(s, _)| s.chars().count() >= 2);
    for (_, _, row) in all_spans(q, labels) {
        if !ids.iter().any(|v: &&Value| v["id"] == row["id"]) {
            ids.push(row)
        }
    }
    ids
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(q, &["远征", "expedition"])
        || !has(
            q,
            &[
                "最多", "最少", "至少", "至多", "区间", "范围", "总共", "合计", "保底", "上限",
                "下限", "总计", "一共", "how many", "in total", "maximum", "minimum", "at most",
                "at least", "range",
            ],
        )
        || has(
            q,
            &[
                "战力",
                "多久",
                "多少分钟",
                "解锁",
                "战斗力",
                "队伍",
                "team power",
                "duration",
                "unlock",
                "how long",
            ],
        )
    {
        return None;
    }
    let stages = array(&d.get("expedition-data")["stages"]);
    let labels = stages.iter().map(|s| (text(s, "name"), s));
    let mut hits = all_spans(q, labels);
    let hard = has(q, &["困难", "hard"]);
    let normal = has(q, &["普通", "normal"]);
    // A regional short name selects the indicated difficulty; write out the actual stage in the result.
    for row in stages
        .iter()
        .filter(|r| r["hard"].as_bool() == Some(hard && !normal))
    {
        let region = text(row, "region");
        if region.is_empty() {
            continue;
        }
        let english = text(row, "id")
            .strip_prefix("Expedition_")
            .unwrap_or("")
            .trim_end_matches("_Hard");
        for (a, b, r) in all_spans(q, [(region, row), (english, row)].into_iter()) {
            if !hits.iter().any(|(s, e, _)| a < *e && *s < b) {
                hits.push((a, b, r));
            }
        }
    }
    if hits.is_empty() {
        return None;
    }
    hits.sort_by_key(|v| v.0);
    let nums = integers(q);
    let shared = nums
        .iter()
        .find(|n| {
            ["各", "分别各", "每关", "各刷", "各跑", "各自完成"]
                .iter()
                .any(|s| q[..n.start].trim_end().ends_with(s))
                && ["次", "趟", "轮", "把"]
                    .iter()
                    .any(|s| q[n.end..].trim_start().starts_with(s))
        })
        .map(|n| n.value);
    let mut visits = Vec::new();
    let mut links = Vec::new();
    for (a, b, s) in &hits {
        let cnt = repetitions(q, *a, *b, &nums).or(shared).or_else(|| {
            (hits.len() == 1 && has(q, &["每次", "单次", "单趟", "per run", "one completion"]))
                .then_some(1)
        });
        let name = text(s, "name");
        if normal != hard && (s["hard"] == true) != hard {
            return Some(reply(
                format!("「{name}」与指定的远征难度不一致。请确认要算普通还是困难的哪一关。"),
                vec![("远征表".into(), "#work/expeditions".into())],
            ));
        }
        let Some(cnt) = cnt else {
            return Some(reply(
                format!("要算多趟奖励，请给出「{name}」的完成次数。"),
                vec![(name.into(), format!("#work/expeditions/{}", text(s, "id")))],
            ));
        };
        visits.push((*s, cnt));
        let link = (
            name.to_owned(),
            format!("#work/expeditions/{}", text(s, "id")),
        );
        if !links.contains(&link) {
            links.push(link);
        }
    }
    let catalog = array(&d.get("item-catalog-data")["items"]);
    // Stage names can contain an unrelated item name. Remove their spans before resolving rewards.
    let mut items_q = q.to_owned();
    for &(a, b, _) in hits.iter().rev() {
        items_q.replace_range(a..b, " ");
    }
    let items = item_mentions(catalog, &items_q);
    if items.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for item in items {
        let id = text(item, "id");
        let name = text(item, "name");
        let mut totals = (0u128, 0u128);
        let mut known = true;
        for &(s, n) in &visits {
            let stage = text(s, "name");
            let rows: Vec<_> = array(&s["rewards"]).iter().filter(|r| r[0] == id).collect();
            if rows.len() > 1 {
                lines.push(format!(
                    "「{stage}」中{name}有多个奖励槽，表中未说明各槽如何联动，无法确定合计范围。"
                ));
                known = false;
                continue;
            }
            let Some(r) = rows.first() else {
                lines.push(format!(
                    "「{stage}」的远征奖励未列{name}，按本表这一段计 0。"
                ));
                continue;
            };
            let (lo, hi, p) = (r[1].as_u64()?, r[2].as_u64()?, r[3].as_f64()?);
            if lo > hi || !(0.0..=100.0).contains(&p) {
                return None;
            }
            let low = if p == 100.0 { lo } else { 0 };
            let high = if p > 0.0 { hi } else { 0 };
            totals.0 = totals.0.checked_add(u128::from(n) * u128::from(low))?;
            totals.1 = totals.1.checked_add(u128::from(n) * u128::from(high))?;
            lines.push(format!(
                "「{stage}」每次 {p}% 概率得 {lo}–{hi} 个{name}，{n} 趟合计 {}–{} 个。",
                u128::from(n) * u128::from(low),
                u128::from(n) * u128::from(high)
            ));
        }
        if known {
            lines.push(format!(
                "按各次均成功完成远征，{name}合计最少 {} 个、最多 {} 个。",
                totals.0, totals.1
            ));
        }
        links.push((name.into(), format!("#items/{id}")));
    }
    Some(reply(lines.join("\n"), links))
}
