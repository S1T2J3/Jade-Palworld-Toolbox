use super::{AssistantReply, field_text, quantities::integers};
use crate::data::{Data, array, text};

/// Keep each count with its machine. A technology level or a duration is not a machine count.
pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if let Some(answer) = automated_output(data, q) {
        return Some(answer);
    }
    if ![
        "耗电",
        "电量",
        "供电",
        "多少电",
        "需要的电力",
        "发电量",
        "power",
        "electricity",
        "energy draw",
    ]
    .iter()
    .any(|w| q.contains(w))
    {
        return None;
    }
    let en = q.is_ascii();
    let mut mentions = Vec::new();
    for row in array(&data.get("resource-guides-data")["production"]) {
        let Some(power) = row["power"].as_u64() else {
            continue;
        };
        let id = text(row, "facility");
        let mut names = vec![
            field_text(text(row, "name")),
            field_text(&id.replace('_', " ")),
        ];
        names.extend(
            array(&row["aliases"])
                .iter()
                .filter_map(|v| v.as_str())
                .map(field_text),
        );
        // Players routinely omit "crude" or put 高压型 before the machine name.
        if id.contains("Crude_Oil_Extractor") {
            if id.starts_with("High-Pressure") {
                names.extend(
                    [
                        "high-pressure oil extractor",
                        "high pressure oil extractor",
                        "high pressure crude oil extractor",
                        "高压原油提炼机",
                        "高压型原油提炼机",
                        "原油提炼机高压型",
                    ]
                    .map(str::to_owned),
                );
            } else {
                names.extend(
                    [
                        "oil extractor",
                        "普通原油提炼机",
                        "普通型原油提炼机",
                        "regular oil extractor",
                        "standard oil extractor",
                    ]
                    .map(str::to_owned),
                );
            }
        }
        for name in names.iter().filter(|w| !w.is_empty()) {
            for (i, _) in q.match_indices(name) {
                let end = i + name.len();
                if name.is_ascii()
                    && (q[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
                        || q[end..]
                            .trim_start_matches('s')
                            .starts_with(|c: char| c.is_ascii_alphabetic()))
                {
                    continue;
                }
                mentions.push((i, end, row, power));
            }
        }
    }
    mentions.sort_by_key(|(a, b, _, _)| (*a, std::cmp::Reverse(b - a)));
    let mut seen_until = 0;
    let mut loads = Vec::new();
    let nums = integers(q);
    let mut links = Vec::new();
    for (a, b, row, power) in mentions {
        if a < seen_until {
            continue;
        }
        let count = nums.iter().rev().find(|n| {
            n.end <= a
                && n.start >= seen_until
                && matches!(
                    q[n.end..a].trim(),
                    "" | "台" | "个" | "座" | "台的" | "台普通" | "台普通型" | "x" | "×"
                )
        });
        seen_until = b;
        let name = if en {
            text(row, "facility").replace('_', " ")
        } else {
            text(row, "name").to_owned()
        };
        let route = format!("#work/production/{}", text(row, "id"));
        if !links.iter().any(|(_, r)| r == &route) {
            links.push((name.clone(), route));
        }
        loads.push((name, u128::from(power), count.map(|n| u128::from(n.value))));
    }
    if loads.is_empty() {
        return None;
    }
    // Installed machines and machines drawing power may be different counts.
    if loads.len() == 1 {
        let mut running = None;
        let mut off = None;
        for n in &nums {
            let prefix = q[..n.start].trim_end();
            let suffix = q[n.end..].trim_start();
            if [
                "只开",
                "只开了",
                "仅开",
                "仅运行",
                "只运行",
                "only running",
                "running only",
            ]
            .iter()
            .any(|v| prefix.ends_with(v))
                || [
                    "台开着",
                    "台在运转",
                    "台在运行",
                    "are running",
                    "is running",
                    "running now",
                ]
                .iter()
                .any(|v| suffix.starts_with(v))
            {
                running = Some(u128::from(n.value));
            } else if [
                "台停机",
                "台没开",
                "台关闭",
                "台关了",
                "is off",
                "are off",
                "are idle",
            ]
            .iter()
            .any(|v| suffix.starts_with(v))
            {
                off = Some(u128::from(n.value));
            }
        }
        let total = loads[0].2;
        if let Some(n) = running {
            if total.is_some_and(|t| n > t) {
                return None;
            }
            loads[0].2 = Some(n);
        } else if let (Some(t), Some(n)) = (total, off) {
            loads[0].2 = Some(t.checked_sub(n)?);
        }
    }
    let lines: Vec<_> = loads
        .iter()
        .map(|(name, rate, count)| match count {
            Some(n) => format!("{name}: {n} × {rate} = {} /s", n * rate),
            None => {
                if en {
                    format!("{name}: {rate} /s per machine")
                } else {
                    format!("{name}：每台 {rate} /秒")
                }
            }
        })
        .collect();
    let mut answer = lines.join(if en { ". " } else { "；" });
    if loads.len() > 1 && loads.iter().all(|v| v.2.is_some()) {
        let sum = loads
            .iter()
            .map(|(_, r, n)| r * n.unwrap())
            .try_fold(0u128, u128::checked_add)?;
        answer.push_str(&if en {
            format!(". Combined draw: {sum} /s.")
        } else {
            format!("。所列设备合计每秒耗电 {sum}。")
        });
    }
    let times: Vec<_> = nums
        .iter()
        .filter_map(|n| {
            let tail = q[n.end..].trim_start();
            let scale = [
                ("小时", 3600),
                ("分钟", 60),
                ("秒", 1),
                ("hours", 3600),
                ("hour", 3600),
                ("minutes", 60),
                ("minute", 60),
                ("seconds", 1),
                ("second", 1),
            ]
            .into_iter()
            .find(|(unit, _)| tail.starts_with(unit))?
            .1;
            Some(u128::from(n.value) * scale)
        })
        .collect();
    if let [seconds] = times.as_slice() {
        let rate = loads
            .iter()
            .try_fold(0u128, |sum, (_, r, n)| sum.checked_add(r * n.unwrap_or(1)))?;
        let total = rate.checked_mul(*seconds)?;
        let per_unit = loads.iter().any(|(_, _, n)| n.is_none());
        let detail = match (en, per_unit) {
            (true, true) => " (one of each listed model)",
            (false, true) => "（未写台数的机型按一台）",
            _ => "",
        };
        answer.push_str(&if en {
            format!(". For {seconds} s, total energy = {rate} × {seconds} = {total}{detail}.")
        } else {
            format!("。持续运转 {seconds} 秒，总耗电为 {rate} × {seconds} = {total}{detail}。")
        });
    }
    answer.push_str(if en {
        " These are game power units while running. Other base equipment adds to the load."
    } else {
        " 以上是设备运转时的游戏电力单位；同据点其他用电设备还要另计。"
    });
    Some(AssistantReply {
        text: answer,
        links,
        suggestions: vec![],
    })
}

fn automated_output(data: &Data, q: &str) -> Option<AssistantReply> {
    let has=|words:&[&str]|words.iter().any(|w|q.contains(w));
    if !has(&["提高","增加","提速","增产","效率","生产速度","more","increase","speed up","faster","optimize"]) ||
        !has(&["产量","产出","产油","提炼","抽油","生产","production","output","extract","pump"]){return None;}
    let rows:Vec<_>=array(&data.get("resource-guides-data")["production"]).iter().filter(|r|{
        if !r["auto"].as_f64().is_some_and(|v|v>0.) || !array(&r["jobs"]).is_empty(){return false;}
        let name=text(r,"itemName");
        let en=text(r,"facility").replace('_'," ").to_lowercase();
        (!name.is_empty() && q.contains(name)) || (!en.is_empty() && q.contains(&en))
            || r["item"]=="CrudeOil" && has(&["oil production","oil output","oil pump","oil extractor","produce more oil","产油","抽油"])
    }).collect();
    let first=*rows.first()?;
    if !rows.iter().all(|r|r["item"]==first["item"]){return None;}
    let mut lines=Vec::new();
    let mut links=Vec::new();
    for row in rows{
        let name=text(row,"name");
        let condition=text(row,"condition");
        lines.push(format!("{name}：{condition}"));
        links.push((name.into(),format!("#work/production/{}",text(row,"id"))));
    }
    if has(&["setting","配置","倍率","设置"]){
        lines.insert(0,format!("Pocketpair 的服务器参数表没有列出直接调高{}提炼倍率的单独参数。",text(first,"itemName")));
        lines.push("参数说明：https://docs.palworldgame.com/settings-and-operation/configuration/".into());
    }
    lines.push("生产由设备自动完成。先查看设备是否持续通电、成品是否取走；要增加总产出，可在满足建造条件的位置增加同时运行的设备，并留足总供电。给生产帕鲁升星不会直接改变这台自动设备的基础产速。".into());
    Some(AssistantReply{text:lines.join("\n"),links,suggestions:vec![]})
}
