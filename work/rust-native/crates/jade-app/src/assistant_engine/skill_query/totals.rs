use super::{AssistantReply, Data, has, mentioned_traits, reply, text};
use std::collections::BTreeMap;
mod compare;

struct Metric {
    key: &'static str,
    title: &'static str,
    terms: &'static [&'static str],
    effect_names: &'static [&'static str],
}

// These keys are the additive fields actually used by the panel and work calculators.
const METRICS: &[Metric] = &[
    Metric {
        key: "work",
        title: "工作速度",
        terms: &[
            "工作速度",
            "工作效率",
            "作业速度",
            "干活",
            "work speed",
            "working speed",
            "crafting speed",
        ],
        effect_names: &["工作速度"],
    },
    Metric {
        key: "attack",
        title: "面板攻击",
        terms: &["攻击", "面攻", "attack", " atk"],
        effect_names: &["攻击", "攻击力"],
    },
    Metric {
        key: "defense",
        title: "面板防御",
        terms: &["防御", "defense", "defence", " def"],
        effect_names: &["防御", "防御力"],
    },
    Metric {
        key: "hp",
        title: "最大生命",
        terms: &["最大生命", "生命值", "血量", "最大hp", " hp", "health"],
        effect_names: &["生命值", "最大hp"],
    },
    Metric {
        key: "movement",
        title: "移动速度",
        terms: &[
            "移速",
            "移动速度",
            "跑速",
            "奔跑速度",
            "movement speed",
            "run speed",
        ],
        effect_names: &["移动速度"],
    },
    Metric {
        key: "ranch",
        title: "牧场工作适性等级",
        terms: &["牧场", "放牧", "ranch", "farming suitability"],
        effect_names: &["牧场的工作适应性"],
    },
];

fn modifier(data: &Data, id: &str, key: &str) -> Option<i64> {
    // Work speed exists in both tables. Count this shared modifier once.
    ["stat-data", "work-efficiency-data"]
        .iter()
        .find_map(|table| data.get(table)["modifiers"][id][key].as_i64())
}

fn modeled_line(data: &Data, id: &str, line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    METRICS.iter().any(|m| {
        modifier(data, id, m.key).is_some()
            && m.effect_names.iter().any(|prefix| {
                line.strip_prefix(prefix).is_some_and(|tail| {
                    let value = tail.trim().trim_start_matches("提升").trim();
                    value.starts_with(['+', '-']) || value.starts_with(|c: char| c.is_ascii_digit())
                })
            })
    })
}

fn related(metric: &Metric, line: &str) -> bool {
    metric.effect_names.iter().any(|part| line.contains(part))
        || metric.key == "hp" && line.contains("生命")
}

fn percent_or_rank(v: i64, key: &str) -> String {
    if key == "ranch" {
        format!("{v:+} 级")
    } else {
        format!("{v:+}%")
    }
}

// For effects outside the calculators only compare identical numeric statements,
// including their subject and activation qualifier. A sum of displayed values
// does not establish how the game applies them.
fn displayed_percent<'a>(
    data: &Data,
    id: &str,
    effect: &'a str,
) -> Option<(&'a str, &'a str, f64)> {
    let mut lines = effect.lines().filter(|l| !modeled_line(data, id, l));
    let line = lines.next()?.trim().trim_end_matches('。');
    let end = line.strip_suffix('%')?.trim_end();
    let start = end
        .trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, '.' | '+' | '-'))
        .len();
    let value = end[start..].parse::<f64>().ok()?;
    let label = end[..start].trim();
    let condition = lines.next().unwrap_or("");
    if label.is_empty()
        || !condition.is_empty() && !condition.starts_with('※')
        || lines.next().is_some()
    {
        return None;
    }
    Some((label, condition, value))
}

pub(super) fn answer(
    data: &Data,
    q: &str,
    previous: &[String],
) -> Option<(AssistantReply, Vec<String>)> {
    if let Some(answer) = compare::answer(data, q) {
        return Some((answer, vec![]));
    }
    if has(
        q,
        &[
            "遗传",
            "继承",
            "配种概率",
            "植入",
            "手术",
            "推荐",
            "排除",
            "不用",
            "二选一",
            "哪个更",
            "哪一个更",
            "还是选",
            "分别有什么",
            "对比",
            "比较哪",
            "breed",
            "inherit",
            "surgery",
            "implant",
            "recommend",
            "instead of",
            "which is better",
            " versus ",
            " vs ",
        ],
    ) {
        return None;
    }
    let sum = has(
        q,
        &[
            "合计",
            "合起来",
            "合在一起",
            "总计",
            "叠加",
            "相加",
            "加起来",
            "加一起",
            "一起带",
            "一起用",
            "一起装",
            "一起上",
            "同时带",
            "同时拥有",
            "同时装备",
            "一起生效",
            "同时生效",
            "总共",
            "一共",
            "总加成",
            "总效果",
            "总提升",
            "净加成",
            "净增",
            "总的",
            "加成多少",
            "加多少",
            "会加多少",
            "最终",
            "乘区",
            "乘算",
            "加算",
            "+",
            "total",
            "together",
            "combined",
            "combine",
            "stack",
            "net bonus",
            "add up",
            "additive",
            "add or multiply",
            "added or multiplied",
            "multiplicative",
            "how much",
        ],
    );
    let asking = has(
        q,
        &[
            "是多少",
            "会是多少",
            "各是多少",
            "多少",
            "怎么算",
            "多少倍",
            "几个点",
            "百分点",
            "how much",
            "what is the",
            "what's the",
        ],
    );
    let target: Vec<_> = METRICS.iter().filter(|m| has(q, m.terms)).collect();
    let resume = !previous.is_empty()
        && (q.starts_with(['那', '再'])
            || q.starts_with("what about")
            || q.starts_with("and ")
            || q.starts_with("how about")
            || q.starts_with("只看"));
    if !sum && !(asking && !target.is_empty()) && !(resume && !target.is_empty()) {
        return None;
    }
    let mut ids: Vec<String> = mentioned_traits(data, q)
        .into_iter()
        .map(str::to_owned)
        .collect();
    if resume && ids.is_empty() {
        if !super::species_mentions(&data.species, q).0.is_empty()
            || has(
                q,
                &[
                    "武器",
                    "装备伤害",
                    "制作台",
                    "设计图",
                    "weapon",
                    "schematic",
                ],
            )
        {
            return None;
        }
        ids = previous.to_vec();
    } else if resume && has(q, &["再加", "再带", "再算上", "also add", "and add"]) {
        let added = std::mem::replace(&mut ids, previous.to_vec());
        for id in added {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    if ids.len() < 2 {
        return None;
    }
    let all = &data.get("pal-details-data")["passives"];
    let links = ids
        .iter()
        .map(|id| {
            (
                text(&all[id], "zh").to_owned(),
                format!("#details/passive/trait/{id}"),
            )
        })
        .collect();
    if ids.len() > data.get("trait-data")["maxSlots"].as_u64().unwrap_or(4) as usize {
        return Some((
            reply(
                vec![format!(
                    "已识别 {} 个不同词条，一只帕鲁最多装备 {} 个。请指定实际同时装备的词条。",
                    ids.len(),
                    data.get("trait-data")["maxSlots"].as_u64().unwrap_or(4)
                )],
                links,
            ),
            vec![],
        ));
    }
    if has(
        q,
        &[
            "不同帕鲁",
            "不同的帕鲁",
            "各一只",
            "另一只",
            "两只各",
            "一人一",
            "分别带",
            "各带",
            "different pals",
            "separate pals",
            "on two pals",
            "one pal has",
            "each pal",
        ],
    ) {
        let mut lines = vec![
            "不同帕鲁各自结算面板词条。需要知道每只帕鲁分别带哪些词条，以及所问效果的受益对象。"
                .into(),
        ];
        for id in &ids {
            lines.push(format!(
                "{}：{}",
                text(&all[id], "zh"),
                text(&all[id], "effect")
            ));
        }
        return Some((reply(lines, links), vec![]));
    }
    let player = has(q, &["玩家", "player", "trainer"]);
    let damage = has(q, &["伤害", "dps", "damage"]);
    // A question about a secondary effect (e.g. active-skill cooldown) must not
    // silently become an attack-panel calculation just because a passive also boosts attack.
    let other_metric = has(
        q,
        &[
            "冷却",
            "技能cd",
            "耐力",
            "体力",
            "抗性",
            "抗伤",
            "san",
            "饱腹",
            "产蛋",
            "孵化",
            "游泳",
            "水上",
            "掉落",
            "跳跃",
            "治疗",
            "回血",
            "交易价",
            "伤害",
            "cooldown",
            "stamina",
            "swimming",
            "damage",
            "healing",
            "hunger",
            "loot",
            "hatch",
            "jump",
        ],
    );
    let show_all = target.is_empty() && !other_metric;
    let mut lines = Vec::new();
    for m in METRICS {
        if !show_all && !target.iter().any(|t| t.key == m.key) && !(m.key == "attack" && damage) {
            continue;
        }
        let parts: Vec<_> = ids
            .iter()
            .filter_map(|id| modifier(data, id, m.key).map(|v| (text(&all[id], "zh"), v)))
            .collect();
        if parts.is_empty() {
            continue;
        }
        let total: i64 = parts.iter().map(|(_, v)| v).sum();
        let operands = parts
            .iter()
            .map(|(name, v)| format!("{name} {}", percent_or_rank(*v, m.key)))
            .collect::<Vec<_>>()
            .join("、");
        lines.push(format!(
            "帕鲁自身{}合计 {}（{}）。",
            m.title,
            percent_or_rank(total, m.key),
            operands
        ));
        if m.key == "ranch" {
            lines.push("已有牧场适性的帕鲁才生效，实际等级受牧场等级表上限约束。".into());
        } else if has(q, &["倍", "倍率", "multiplier", "times"]) {
            let factor = (100 + total).max(0) as f64 / 100.;
            lines.push(format!("{}的词条系数为 {factor} 倍。", m.title));
        }
    }
    let mut remaining = Vec::new();
    let mut display_totals = BTreeMap::new();
    for id in &ids {
        let row = &all[id];
        let description = text(row, "effect");
        let extra: Vec<_> = description
            .lines()
            .filter(|l| !modeled_line(data, id, l))
            .collect();
        // Preserve multi-line conditions alongside their values rather than treating e.g.
        // a rider-only stamina bonus or a player aura as a stat of the Pal itself.
        if !extra.is_empty()
            && (show_all
                || other_metric
                || extra.iter().any(|l| target.iter().any(|m| related(m, l))))
        {
            remaining.push(format!("{}：{}", text(row, "zh"), extra.join("，")));
            if let Some((label, condition, value)) = displayed_percent(data, id, description) {
                display_totals
                    .entry((label, condition))
                    .or_insert_with(Vec::new)
                    .push(value);
            }
        }
    }
    if lines.is_empty() && remaining.is_empty() {
        lines.push("所列词条没有记载该项数值加成。".into());
        for id in &ids {
            lines.push(format!(
                "{}：{}",
                text(&all[id], "zh"),
                text(&all[id], "effect")
            ));
        }
    }
    if !remaining.is_empty() {
        lines.extend(remaining);
        for ((label, condition), numbers) in display_totals {
            if numbers.len() > 1 {
                let display_sum: f64 = numbers.iter().sum();
                lines.push(format!(
                    "{label}的标示百分比算术和为 {display_sum:+}%。{condition}"
                ));
            }
        }
        lines.push("上列说明中的其他效果按各自的受益对象、条件和单位读取；图鉴没有给出它们之间通用的叠加公式。".into());
    }
    if player
        && !ids
            .iter()
            .any(|id| text(&all[id], "effect").contains("玩家"))
    {
        lines.push("这些词条没有记载给玩家的对应加成。".into());
    }
    if damage {
        lines.push(
            "攻击面板和属性伤害是不同计算项。求最终命中伤害还需要招式、目标与伤害公式。".into(),
        );
    }
    if has(q, &["帕鲁之魂", "浓缩", "pal soul", "condens"]) && !lines.is_empty() {
        lines.push("面板计算器先合计同一属性的被动加成，再应用帕鲁之魂和浓缩各自的系数；当前合计只计入点名的词条。".into());
    }
    Some((reply(lines, links), ids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    fn data() -> Data {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in [
            "pal-details-data",
            "trait-data",
            "stat-data",
            "work-efficiency-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            d.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        d
    }

    #[test]
    fn assigns_worn_traits_to_their_owners_before_comparing_panel_bonuses() {
        let d = data();
        let ask = |q: &str| {
            let mut c = Conversation::default();
            let answer = c.ask_world(q, &d, &serde_json::json!({"id":"grouped-traits"}));
            assert!(c.direct_result(), "{q}: {}", answer.text);
            answer.text
        };
        for q in [
            "两只同种帕鲁，除了词条其他条件相同。甲只带神速，乙带运动健将和灵活。只比较移动速度，谁快？差几个百分点？",
            "同条件第一只带神速，第二只带运动健将和灵活，移动速度相差多少？",
            "One Pal has Swift, the other has Runner and Nimble. Which has higher movement speed?",
        ] {
            let answer = ask(q);
            assert!(answer.contains("相差 0 个百分点") && answer.contains("两只一样快") && !answer.contains("+60%"), "{answer}");
        }
        let answer = ask("一只带神速和灵活，另一只带神速和运动健将，谁跑得快？");
        assert!(answer.contains("+40%") && answer.contains("+50%") && answer.contains("第二只更快"), "{answer}");
        let answer = ask("比较工作速度，棉悠悠带工匠精神，捣蛋猫带社畜，差多少？");
        assert!(answer.contains("棉悠悠的工作速度词条加成更高，相差 20 个百分点"), "{answer}");
        let answer = ask("甲带工匠精神和社畜，乙带未知词条，两者工作速度差多少？");
        assert!(answer.contains("没有认出乙") && !answer.contains("+80%"), "{answer}");
        let answer = ask("甲带神速和未命名词条，乙带灵活，两只移速差多少？");
        assert!(answer.contains("没有认出甲"), "{answer}");
        let answer = ask("甲有神速，乙没有任何词条。只比较移速，两者相差多少？");
        assert!(answer.contains("相差 30 个百分点") && answer.contains("甲更快"), "{answer}");
    }

    #[test]
    fn calculates_only_requested_panel_fields_and_keeps_signs() {
        let d = data();
        let mut conv = Conversation::default();
        let world = serde_json::json!({"id":"traits"});
        let first = conv.ask_world(
            "工匠精神和社畜的工作速度合计提升多少，是原来的几倍？",
            &d,
            &world,
        );
        assert!(
            conv.direct_result()
                && first.text.contains("+80%")
                && first.text.contains("1.8 倍")
                && !first.text.contains("攻击"),
            "{}",
            first.text
        );
        let next = conv.ask_world("那攻击呢？", &d, &world);
        assert!(
            conv.direct_result() && next.text.contains("-30%") && !next.text.contains("工作速度"),
            "{}",
            next.text
        );
        let second = conv.ask_world("腦筋和社畜一起带，攻击和工作速度各合计多少？", &d, &world);
        assert!(
            second.text.contains("面板攻击合计 +0%") && second.text.contains("工作速度合计 -20%"),
            "{}",
            second.text
        );
        let movement = conv.ask_world(
            "Swift and Legend together: total movement speed?",
            &d,
            &world,
        );
        assert!(
            movement.text.contains("+50%") && !movement.text.contains("攻击"),
            "{}",
            movement.text
        );
    }

    #[test]
    fn preserves_recipients_units_conditions_and_unmodeled_damage() {
        let d = data();
        let ask = |q: &str| {
            Conversation::default()
                .ask_world(q, &d, &serde_json::json!({"id":"limits"}))
                .text
        };
        let who = ask("工匠精神和啦啦队合计能加多少工作速度？");
        assert!(
            who.contains("帕鲁自身工作速度合计 +50%")
                && who.contains("玩家的工作速度提升25%")
                && !who.contains("+75%"),
            "{who}"
        );
        let grade = ask("牧场之子和牧场之主的牧场工作适性合计多少？");
        assert!(grade.contains("+3 级") && !grade.contains("+3%"), "{grade}");
        let damage = ask("凶猛和炎帝的最终火属性伤害加起来是多少？");
        assert!(
            damage.contains("面板攻击合计 +20%")
                && damage.contains("火属性攻击伤害增加30%")
                && !damage.contains("+50%"),
            "{damage}"
        );
        let stamina = ask("无限精力和家里蹲的最大耐力合计多少？");
        assert!(
            stamina.contains("+25%")
                && stamina.contains("仅对可骑乘的帕鲁有效")
                && stamina.contains("算术和"),
            "{stamina}"
        );
        assert!(answer(&d, "工匠精神和社畜一起带的遗传概率是多少", &[]).is_none());
        assert!(answer(&d, "工匠精神和社畜哪个更适合工作速度", &[]).is_none());
        let others = ask("一只带工匠精神，另一只带社畜，合计加多少工作速度？");
        assert!(
            others.contains("不同帕鲁") && !others.contains("+80%"),
            "{others}"
        );
        let current = vec!["CraftSpeed_up2".into(), "PAL_CorporateSlave".into()];
        assert!(answer(&d, "那空涡龙基础攻击多少？", &current).is_none());
        let too_many = ask("工匠精神、社畜、脑筋、神速、凶猛同时装备，总加成是多少？");
        assert!(
            too_many.contains("5 个不同词条") && too_many.contains("最多装备 4 个"),
            "{too_many}"
        );
    }
}
