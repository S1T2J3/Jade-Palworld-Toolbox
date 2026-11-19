use super::{AssistantReply, field_text, quantities::integers};
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, array, text};
use serde_json::Value;

fn any(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|s| q.contains(s))
}

fn levels(q: &str) -> Option<(u64, u64)> {
    // Keep "Lv2" and "Lv.2" usable by the same quantity parser as "等级 2".
    let q = q
        .replace("lvl.", "等级 ")
        .replace("lv.", "等级 ")
        .replace("lvl", "等级 ")
        .replace("lv", "等级 ");
    let mut levels = Vec::new();
    for n in integers(&q) {
        let before = q[..n.start].trim_end();
        let after = q[n.end..].trim_start_matches([' ', '-']);
        let star = ["星", "★", "star"].iter().any(|s| after.starts_with(s));
        let level_marker = ["等级", "级别", "level", "rank", "等级为", "技能等级是"]
            .iter()
            .any(|s| before.ends_with(s))
            || after.starts_with('级');
        let second = levels.len() == 1
            && [
                "升到", "升至", "到", "变为", "变成", "降到", "→", "to", "through", "至", "和",
                " vs", " and",
            ]
            .iter()
            .any(|s| before.ends_with(s));
        if !(star || level_marker || second) {
            continue;
        }
        if [
            "玩家",
            "我",
            "角色",
            "player level",
            "i'm level",
            "i am level",
        ]
        .iter()
        .any(|s| before.ends_with(s))
        {
            continue;
        }
        let level = if star {
            n.value.checked_add(1)?
        } else {
            n.value
        };
        levels.push(level);
    }
    if let [a, b] = levels.as_slice() {
        Some((*a, *b))
    } else {
        None
    }
}

fn field_match(q: &str, caption: &str) -> bool {
    let term = caption.split('（').next().unwrap_or(caption);
    q.contains(term)
        || match term {
            "冷却时间" => any(
                q,
                &[
                    "冷却",
                    "间隔",
                    "要等",
                    "等多久",
                    "等待",
                    "多久一次",
                    "cooldown",
                    "cool-down",
                    "recharge",
                    "wait",
                    "reuse",
                ],
            ),
            "持续时间" => any(q, &["持续", "维持", "duration", "lasts", "last for"]),
            "技能威力倍率" | "技能威力" => any(q, &["威力", "power", "倍率"]),
            "可开启宝箱等级" => {
                any(q, &["开锁", "开启宝箱", "解锁宝箱", "unlock", "lockpick"])
            }
            _ => false,
        }
}

fn compact(v: f64) -> String {
    format!("{v:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn change(values: &Value, a: usize, b: usize, unit: &str) -> Option<String> {
    let old = values.get(a)?.as_f64()?;
    let new = values.get(b)?.as_f64()?;
    let delta = new - old;
    let difference = if delta == 0. {
        "没有变化".into()
    } else {
        format!(
            "{} {}{unit}",
            if delta < 0. { "减少" } else { "增加" },
            compact(delta.abs())
        )
    };
    Some(format!(
        "Lv.{} 为 {}{unit}，Lv.{} 为 {}{unit}，{difference}",
        a + 1,
        compact(old),
        b + 1,
        compact(new)
    ))
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let (from, to) = levels(q)?;
    let book = d.get("partner-skills-data").as_object()?;
    let partner = any(
        q,
        &[
            "伙伴技能",
            "伙伴技",
            "同伴技能",
            "搭档技能",
            "partner skill",
            "partner ability",
        ],
    );
    if !partner
        && any(
            q,
            &[
                "主动技能",
                "主动技",
                "主动招式",
                "active skill",
                "技能果实",
                "被动词条",
            ],
        )
    {
        return None;
    }
    let (named, _) = species_mentions(&d.species, q);
    let mut ids: Vec<_> = named
        .iter()
        .map(|(n, _, _)| text(&d.species[*n], "key"))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let named_skill: Vec<_> = book
        .iter()
        .filter(|(_, r)| {
            let name = field_text(text(r, "name"));
            name.chars().count() >= 3 && q.contains(&name)
        })
        .map(|(id, _)| id.as_str())
        .collect();
    let id = match (ids.as_slice(), named_skill.as_slice()) {
        ([id], []) if partner => *id,
        ([id], skills) if skills.contains(id) => *id,
        ([], [id]) => *id,
        _ => return None,
    };
    let row = book.get(id)?;
    let name = text(row, "name");
    let owner = d.pal_name(id);
    let link = (
        format!("{owner} · {name}"),
        format!("#details/partner/{id}"),
    );
    let mut reply = AssistantReply::new("", &[]);
    reply.links.push(link);
    if !(1..=5).contains(&from) || !(1..=5).contains(&to) {
        reply.text = format!(
            "{owner}的伙伴技能「{name}」按技能等级 1–5 记录，分别对应浓缩 0–4 星。请核对两个技能等级。"
        );
        return Some(reply);
    }
    let (a, b) = ((from - 1) as usize, (to - 1) as usize);
    let mut effect = text(row, "effect").to_owned();
    for (label, values) in row["curves"].as_object().into_iter().flatten() {
        let Some((_, after)) = effect.split_once(label) else {
            continue;
        };
        let percent = after.starts_with('%');
        let unit = if percent {
            "%"
        } else if after.starts_with('秒') {
            " 秒"
        } else {
            ""
        };
        let Some(mut details) = change(values, a, b, unit) else {
            continue;
        };
        if percent && details.ends_with('%') {
            details.pop();
            details.push_str(" 个百分点");
        }
        let placeholder = if percent {
            format!("{label}%")
        } else if unit == " 秒" {
            format!("{label}秒")
        } else {
            label.clone()
        };
        effect = effect.replace(&placeholder, &format!("（{details}）"));
    }
    let mut lines = vec![format!("{owner}的伙伴技能「{name}」：{effect}")];
    let stats = array(&row["stats"]);
    let matching: Vec<_> = stats
        .iter()
        .filter(|s| field_match(q, s[0].as_str().unwrap_or("")))
        .collect();
    if matching.is_empty() && field_match(q, "冷却时间") {
        lines.push("这项技能没有登记冷却的分级数值，无法算出冷却差。".into());
    } else {
        let selected = if matching.is_empty() {
            stats.iter().collect()
        } else {
            matching
        };
        for stat in selected {
            let caption = stat[0].as_str()?;
            let unit = if caption.contains("秒") { " 秒" } else { "" };
            if let Some(diff) = change(&stat[1], a, b, unit) {
                lines.push(format!(
                    "{}：{diff}。",
                    caption.split('（').next().unwrap_or(caption)
                ));
            }
        }
    }
    if stats.is_empty() && row["curves"].as_object().is_none() && lines.len() == 1 {
        lines.push("图鉴只记录了上述技能效果，缺少这两个等级可比较的数值。".into());
    }
    reply.text = lines.join("\n");
    Some(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    #[test]
    fn partner_level_changes_use_the_selected_species_stat_units_and_effect() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        d.tables.insert(
            "partner-skills-data".into(),
            std::sync::Arc::new(
                serde_json::from_slice(
                    &std::fs::read(root.join("partner-skills-data.json")).unwrap(),
                )
                .unwrap(),
            ),
        );
        for (q, parts) in [
            (
                "猫蝠怪的伙伴技能从 Lv.2 升到 Lv.4，发动一次后要等的时间缩短多少秒？我想拿它找宝箱，这个技能能探到吗？",
                ["50 秒", "40 秒", "减少 10 秒", "宝箱"],
            ),
            (
                "What changes in cooldown from rank 1 to 5 for Tombat's partner skill?",
                ["60 秒", "30 秒", "减少 30 秒", "猫妖感应"],
            ),
            (
                "朋克蜥的伙伴技能从1级到4级，持续时间增加多久？",
                ["持续时间", "10 秒", "16 秒", "增加 6 秒"],
            ),
            (
                "猫蝠怪的伙伴技能从0星升到2星，冷却怎样变化？",
                ["Lv.1", "Lv.3", "减少 15 秒", "猫蝠怪"],
            ),
            (
                "捣蛋猫的伙伴技能从1级升到5级，负重增加多少？",
                ["100", "200", "增加 100", "负重"],
            ),
        ] {
            let mut c = Conversation::default();
            let r = c.ask_world(q, &d, &serde_json::json!({}));
            assert!(
                c.direct_result() && parts.iter().all(|p| r.text.contains(p)),
                "{q}: {}",
                r.text
            );
        }
        assert!(answer(&d, &field_text("我20级，猫蝠怪技能3级能用吗？")).is_none());
        assert!(answer(&d, &field_text("猫蝠怪的主动技能从1级到3级有什么变化？")).is_none());
    }
}
