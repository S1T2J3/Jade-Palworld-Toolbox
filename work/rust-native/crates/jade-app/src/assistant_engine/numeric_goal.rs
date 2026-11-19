use super::{
    AssistantReply, field_text,
    quantities::{Quantity, integers},
};
use crate::data::{Data, array, text};
use serde_json::Value;

mod expedition;
mod partner;
mod work;

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|s| q.contains(s))
}

fn reply(text: String, links: Vec<(String, String)>) -> AssistantReply {
    AssistantReply {
        text,
        links,
        suggestions: Vec::new(),
    }
}

fn compact(x: f64) -> String {
    format!("{x:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn rank_text(q: &str) -> String {
    q.replace("lvl.", "等级 ")
        .replace("lv.", "等级 ")
        .replace("lvl", "等级 ")
        .replace("lv", "等级 ")
}

fn ranks(q: &str) -> Vec<Quantity> {
    let nums = integers(q);
    let mut marked: Vec<_> = nums
        .iter()
        .filter(|n| {
            let tail = q[n.end..].trim_start();
            let lead = q[..n.start].trim_end();
            tail.starts_with('级')
                || ["level", "levels", "等级", "rank", "ranks"]
                    .iter()
                    .any(|s| lead.ends_with(s))
        })
        .copied()
        .collect();
    for pair in nums.windows(2) {
        let between = q[pair[0].end..pair[1].start]
            .trim()
            .trim_start_matches('级')
            .trim();
        if (marked.contains(&pair[0]) || marked.contains(&pair[1]))
            && [
                "升到",
                "升级到",
                "升至",
                "到",
                "至",
                "→",
                "-",
                "–",
                "to",
                "vs",
                "versus",
                "and",
            ]
            .contains(&between)
        {
            for n in pair {
                if !marked.contains(n) {
                    marked.push(*n);
                }
            }
        }
    }
    marked.sort_by_key(|n| n.start);
    marked
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    expedition::answer(data, q)
        .or_else(|| work::answer(data, q))
        .or_else(|| partner::answer(data, q))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::sync::Arc;

    #[test]
    fn quantitative_goals_use_owners_units_and_only_eligible_trials() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for key in [
            "partner-skills-data",
            "expedition-data",
            "item-catalog-data",
            "work-efficiency-data",
        ] {
            d.tables.insert(
                key.into(),
                Arc::new(
                    serde_json::from_slice(
                        &std::fs::read(root.join(format!("{key}.json"))).unwrap(),
                    )
                    .unwrap(),
                ),
            );
        }
        let cases: &[(&str, &[&str])] = &[
            (
                "同样从工作适性三级升到四级，一只只负责生火，另一只只负责浇水。其他条件相同，哪只的基础工作效率提升比例更大？各提升百分之多少？",
                &["140", "240", "71.43%", "40%", "生火最大"],
            ),
            (
                "工作效率的百分比怎么算？浇水从1级到3级，生火从3级到4级，谁提升更多？",
                &["100%", "71.43%", "浇水最大"],
            ),
            (
                "生火3级和浇水4级的基础工作效率差多少？",
                &["140", "二者相同"],
            ),
            (
                "我按普通难度先完成两次草原的洞穴，再完成一次森林的秘境。只算远征奖励的高级帕鲁球，三趟合计最少几个、最多几个？",
                &["3–7", "4–8", "最少 10 个、最多 22 个"],
            ),
            (
                "远征草原的洞穴刷3次，蘑菇最少和最多能拿几个？",
                &["50%", "最少 0 个、最多 30 个"],
            ),
            (
                "普通远征：草原和森林各两趟，高级帕鲁球一共最少和最多多少？",
                &["最少 14 个、最多 30 个"],
            ),
            (
                "Complete the Grass expedition twice and the Forest expedition once. How many Mega Spheres in total?",
                &["最少 10 个、最多 22 个"],
            ),
            (
                "我只带一只伙伴技能五级的连理龙，去野外捡20颗符合它技能条件的帕鲁蛋。按技能描述，平均会有几颗变成头目蛋？能保证这个数量吗？",
                &["45%", "= 9 次", "不保证"],
            ),
            (
                "海誓龙的伙伴技能是2级，野外捡10颗蛋时头目蛋的期望是多少？",
                &["47%", "4.7 次"],
            ),
            (
                "带五级伙伴技能的暗巫猫去投21个帕鲁球，平均能省多少个？实际会消耗几颗？",
                &["50%", "10.5 次", "10.5 个"],
            ),
            (
                "满星笑魇猫，野外捡8颗蛋，平均额外获得多少颗？合计是几颗？",
                &["75%", "6 次", "14 颗"],
            ),
            (
                "我带5级连理龙去捡20颗蛋，平均有什么概率变成头目蛋？",
                &["伙伴技能等级", "哪一级"],
            ),
            ("远征世界树地下都市遗址刷3次，只看遗物的最高伤害？", &[]),
        ];
        for &(q, wants) in cases {
            if wants.is_empty() {
                assert!(answer(&d, &field_text(q)).is_none());
                continue;
            }
            let mut c = Conversation::default();
            let r = c.ask_world(q, &d, &serde_json::json!({}));
            assert!(
                c.direct_result() && wants.iter().all(|s| r.text.contains(s)),
                "{q} -> {}",
                r.text
            );
        }
        for q in [
            "八云犬的伙伴技能五级，打三只帕鲁，期望掉多少帕鲁蛋？",
            "我带连理龙，五级被动有多少工作速度？",
            "采矿3级帕鲁哪里抓？",
            "远征草原的洞穴在哪解锁？",
        ] {
            assert!(answer(&d, &field_text(q)).is_none(), "{q}");
        }
    }
}
