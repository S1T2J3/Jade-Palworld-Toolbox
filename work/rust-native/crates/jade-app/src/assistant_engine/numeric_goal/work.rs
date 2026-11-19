use super::*;

fn aliases(job: &str) -> &[&str] {
    match job {
        "生火" => &["kindling", "smelting", "烧炉", "冶炼"],
        "浇水" => &["watering", "灌溉"],
        "播种" => &["planting"],
        "发电" => &["electricity generation", "generating electricity"],
        "手工作业" => &["handiwork", "手工"],
        "采集" => &["gathering"],
        "伐木" => &["lumbering", "logging", "砍树"],
        "采矿" => &["mining", "挖矿"],
        "制药" => &["medicine production"],
        "冷却" => &["cooling", "冷藏"],
        "搬运" => &["transporting", "hauling"],
        "牧场" => &["ranching", "放牧"],
        _ => &[],
    }
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if !has(
        q,
        &[
            "工作适性",
            "工作适应性",
            "基础效率",
            "基础工作",
            "工作效率",
            "work efficiency",
            "base efficiency",
            "work suitability",
            "suitability level",
            "适性", "单趟", "每趟", "每次搬", "搬运量", "per trip", "carry per",
        ],
    ) || !has(
        q,
        &[
            "增", "升", "比例", "倍", "差", "快", "百分", "比较", "更", "vs", "increas", "percent",
            "higher", "compar", "faster", "improv", "gain", "ratio", "times",
        ],
    ) {
        return None;
    }
    if has(
        q,
        &[
            "几只",
            "每小时",
            "每分钟",
            "顿饭",
            "叠加",
            "词条",
            "how many pals",
            "per hour",
            "passive",
            "stack",
        ],
    ) {
        return None;
    }
    let book = d.get("work-efficiency-data")["levels"].as_object()?;
    let q = rank_text(q);
    let mut jobs: Vec<_> = book
        .iter()
        .filter_map(|(k, v)| {
            let pos = std::iter::once(k.as_str())
                .chain(aliases(k).iter().copied())
                .filter_map(|s| q.find(s))
                .min()?;
            Some((pos, k, v))
        })
        .collect();
    jobs.sort_by_key(|(i, ..)| *i);
    if jobs.is_empty() {
        return None;
    }
    let mut levels = ranks(&q);
    levels.retain(|n| {
        !["玩家", "我现在", "player level", "i'm level"]
            .iter()
            .any(|s| q[..n.start].trim_end().ends_with(s))
    });
    if levels.len() < 2 {
        return None;
    }
    // "A 是 B 的几倍" asks for A/B. Earlier clauses often repeat both levels
    // in another order. Repetition must not turn a comparison into an upgrade.
    if let [(_,job,table)] = jobs.as_slice()
        && has(&q,&["几倍","多少倍","how many times","times as"])
    {
        for clause in q.rsplit(['。','?','？','；',';','，',',']) {
            let operands = ranks(clause);
            let [left,right] = operands.as_slice() else {continue};
            if !has(clause, &["几倍","多少倍","times"]) {continue;}
            let bridge = &clause[left.end..right.start];
            if !has(bridge,&["是","相当于","times","compared to","compared with","relative to"]) {continue;}
            let (Some(l),Some(r))=(level_value(table,left.value),level_value(table,right.value)) else {return None;};
            if r == 0 {return None;}
            let unit=if job.as_str()=="搬运" {"每趟基础搬运容量"}else{"基础表值"};
            return Some(reply(format!("{job}的{unit}：Lv.{} 为 {l}，Lv.{} 为 {r}。{l} ÷ {r} = {} 倍。",
                left.value,right.value,compact(l as f64/r as f64)),vec![("工作效率计算器".into(),"#work/efficiency".into())]));
        }
    }
    if jobs.len()==1 {
        let mut unique=Vec::new();
        for level in levels { if !unique.iter().any(|q:&Quantity|q.value==level.value) {unique.push(level);} }
        levels=unique;
    }
    // Two jobs can be given one CURRENT level each ("生火3级和浇水4级").
    // They are then two separate observations rather than a shared upgrade path.
    if let ([(p1, k1, t1), (p2, k2, t2)], [n1, n2]) = (jobs.as_slice(), levels.as_slice())
        && *p1 < n1.start
        && n1.end <= *p2
        && *p2 < n2.start
        && !has(
            &q[n1.end..n2.start],
            &["升到", "升级到", "升至", " → ", " to "],
        )
    {
        let at = |t: &Value, n: Quantity| {
            n.value
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| t.get(i))?
                .as_u64()
        };
        let (a, b) = (at(t1, *n1)?, at(t2, *n2)?);
        if has(
            &q,
            &[
                "增幅",
                "提升多少",
                "升幅",
                "提升比例",
                "increase",
                "improvement",
            ],
        ) {
            return Some(reply(
                format!(
                    "已找到{k1} Lv.{} 的值 {a} 和{k2} Lv.{} 的值 {b}。要比较各自的提升比例，还要知道各自从几级开始。",
                    n1.value, n2.value
                ),
                vec![("工作效率计算器".into(), "#work/efficiency".into())],
            ));
        }
        let mut msg = format!(
            "{k1} Lv.{} 对应 {a}，{k2} Lv.{} 对应 {b}。",
            n1.value, n2.value
        );
        if [k1.as_str(), k2.as_str()]
            .iter()
            .any(|j| matches!(*j, "搬运" | "牧场"))
        {
            msg.push_str("这些工种的计量含义不同，直接比较表值无法断定哪项工作完成更快。");
        } else {
            let cmp = match a.cmp(&b) {
                std::cmp::Ordering::Greater => k1.as_str(),
                std::cmp::Ordering::Less => k2.as_str(),
                std::cmp::Ordering::Equal => "",
            };
            msg.push_str(&if cmp.is_empty() {
                "二者相同。".into()
            } else {
                format!("{cmp}较高，基础数值相差 {}。", a.abs_diff(b))
            });
        }
        return Some(reply(
            msg,
            vec![("工作效率计算器".into(), "#work/efficiency".into())],
        ));
    }
    let same = levels.len() == 2;
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut factors = Vec::new();
    for (i, &(pos, job, table)) in jobs.iter().enumerate() {
        let part: Vec<_> = if same {
            levels.clone()
        } else {
            let end = jobs.get(i + 1).map_or(q.len(), |j| j.0);
            levels
                .iter()
                .copied()
                .filter(|n| n.start >= pos && n.end <= end)
                .collect()
        };
        let [from, to] = part.as_slice() else {
            return Some(reply(
                "各工种的起始和目标等级未能一一对应。请为每种工作列出从几级升到几级。".into(),
                links,
            ));
        };
        if !((1..=10).contains(&from.value) && (1..=10).contains(&to.value)) {
            return Some(reply(
                "本地工作适性表覆盖 Lv.1–10，请核对起点和终点。".into(),
                links,
            ));
        }
        let old = table[from.value as usize - 1].as_u64()?;
        let new = table[to.value as usize - 1].as_u64()?;
        if old == 0 {
            return None;
        }
        let factor = new as f64 / old as f64;
        let verb = if new >= old { "提升" } else { "下降" };
        let unit = match job.as_str() {
            "搬运" => "基础搬运量",
            "牧场" => "基础产出参数",
            _ => "基础工作效率",
        };
        lines.push(format!(
            "{job}的{unit}从 Lv.{} 的 {old} 变为 Lv.{} 的 {new}，相对初值{verb} {}%（{} 倍）。",
            from.value,
            to.value,
            compact((factor - 1.).abs() * 100.),
            compact(factor)
        ));
        if links.is_empty() {
            links.push(("工作效率计算器".to_string(), "#work/efficiency".to_string()));
        }
        factors.push((job.as_str(), new, old));
    }
    if factors.len() > 1 {
        let best = factors
            .iter()
            .max_by(|a, b| (a.1 as u128 * b.2 as u128).cmp(&(b.1 as u128 * a.2 as u128)))?;
        let winners: Vec<_> = factors
            .iter()
            .filter(|v| v.1 as u128 * best.2 as u128 == best.1 as u128 * v.2 as u128)
            .map(|v| v.0)
            .collect();
        lines.push(if winners.len() == factors.len() {
            "这些工种相对初值的变化比例相同。".into()
        } else {
            format!("按相对初值的变化比例比较，{}最大。", winners.join("、"))
        });
    }
    Some(reply(lines.join("\n"), links))
}

fn level_value(table: &Value, level: u64) -> Option<u64> {
    table.get(usize::try_from(level.checked_sub(1)?).ok()?)?.as_u64()
}
