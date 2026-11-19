use super::{AssistantReply, Data, has, text};
use crate::assistant_breeding::species_mentions;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct FlightBook {
    source: String,
    mounts: Vec<(String, u64, u64)>,
}

fn book() -> &'static FlightBook {
    static BOOK: OnceLock<FlightBook> = OnceLock::new();
    BOOK.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/assistant-flight.json"))
            .expect("validated flying mount data")
    })
}

pub(super) fn lookup(d: &Data, q: &str) -> Option<AssistantReply> {
    let speed = has(
        q,
        &[
            "速度",
            "冲刺",
            "最快",
            "较快",
            "更快",
            "speed",
            "fast",
            "stamina",
            "endurance",
            "耐力",
            "体力",
            "续航",
        ],
    );
    let flight = has(
        q,
        &[
            "飞行",
            "飞骑",
            "飞得",
            "飞宠",
            "飞坐骑",
            "flying",
            "flyer",
            "aerial",
        ],
    );
    if !speed {
        return None;
    }
    let (names, _) = species_mentions(&d.species, q);
    if !flight && (names.is_empty() || !has(q, &["骑乘", "坐骑", "mount", "riding", "ride"])) {
        return None;
    }
    let en = q.is_ascii();
    let cap = crate::assistant_engine::quantities::integers(q)
        .into_iter()
        .find_map(|n| {
            let before = q[..n.start].trim_end();
            let after = q[n.end..].trim_start();
            let is_level = after.starts_with('级')
                || ["level", "lv.", "lvl", "lv"]
                    .iter()
                    .any(|marker| before.ends_with(marker));
            if !is_level {
                return None;
            }
            if before.ends_with("below level")
                || before.ends_with("under level")
                || after.starts_with("级之前")
            {
                n.value.checked_sub(1)
            } else {
                Some(n.value)
            }
        });
    let mut candidates = Vec::new();
    for (name, sprint, stamina) in &book().mounts {
        let Some((i, p)) = d
            .species
            .iter()
            .enumerate()
            .find(|(_, p)| text(p, "en") == name)
        else {
            continue;
        };
        let key = text(p, "key");
        if !names.is_empty() && !names.iter().any(|(idx, _, _)| *idx == i) {
            continue;
        }
        let tech = d.get("partner-skills-data")[key]["tech"].as_u64();
        if names.is_empty()
            && (tech.is_none() || cap.is_some_and(|limit| tech.is_some_and(|l| l > limit)))
        {
            continue;
        }
        candidates.push((p, *sprint, *stamina, tech));
    }
    if candidates.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    if names.is_empty() {
        let early = has(
            q,
            &["前期", "开荒", "早期", "early", "starter", "first mount"],
        );
        let endurance = has(q, &["stamina", "endurance", "耐力", "体力", "续航"])
            && !has(q, &["fast", "speed", "冲刺", "速"]);
        if early {
            candidates.sort_by_key(|(_, speed, stamina, level)| {
                (
                    level.unwrap_or(u64::MAX),
                    std::cmp::Reverse(if endurance { *stamina } else { *speed }),
                )
            });
            let mut best = 0;
            candidates.retain(|(_, speed, stamina, _)| {
                let value = if endurance { *stamina } else { *speed };
                let improvement = value > best;
                best = best.max(value);
                improvement
            });
        } else if endurance {
            candidates.sort_by_key(|(_, speed, stamina, level)| {
                (
                    std::cmp::Reverse(*stamina),
                    std::cmp::Reverse(*speed),
                    *level,
                )
            });
        } else {
            candidates.sort_by_key(|(_, speed, stamina, level)| {
                (
                    std::cmp::Reverse(*speed),
                    std::cmp::Reverse(*stamina),
                    *level,
                )
            });
        }
        if early && !endurance {
            lines.push(
                if en {
                    "Early saddle milestones with increasing base sprint speed:"
                } else {
                    "按鞍具解锁顺序，基础骑乘冲刺速度逐步提升的选择有"
                }
                .into(),
            );
        }
    }
    let mut links = Vec::new();
    for (p, sprint, stamina, tech) in candidates.into_iter().take(4) {
        let name = super::team::display_name(p);
        let level = if let Some(l) = tech {
            if en {
                format!(", equipment unlock Lv.{l}")
            } else {
                format!("，装备解锁 Lv.{l}")
            }
        } else {
            String::new()
        };
        lines.push(if en {
            format!("{name}: ride sprint {sprint}, stamina {stamina}{level}.")
        } else {
            format!("{name}：骑乘冲刺 {sprint}，耐力 {stamina}{level}。")
        });
        links.push((name, format!("#paldex/{}", text(p, "key"))));
    }
    lines.push(if en {"Base figures exclude passive and partner bonuses. Saddle level alone does not guarantee that the player can obtain that species."}else{"表中是基础值，实际飞行还受词条和伙伴加成影响。鞍具解锁等级只说明可制作装备的时间。"} .into());
    lines.push(book().source.clone());
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}
