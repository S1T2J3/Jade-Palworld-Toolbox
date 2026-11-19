use super::*;

fn growth(row: &Value, form: &str) -> Option<[u64; 3]> {
    let values = row[form]["growth"].as_array()?;
    Some([
        values.first()?.as_u64()?,
        values.get(1)?.as_u64()?,
        values.get(2)?.as_u64()?,
    ])
}

fn values(v: [u64; 3]) -> String {
    format!("{}/{}/{}", v[0], v[1], v[2])
}

// Players often shorten a long Chinese species name before “和” or “的三围”.
// Resolve a prefix only in an explicit comparison slot and only when exactly
// one species has it. A shared stem must never silently select a form.
fn comparison_names(d: &Data, q: &str) -> Option<Vec<(usize, usize, usize)>> {
    let (mut names, _) = species_mentions(&d.species, q);
    if english(q) {
        return Some(names);
    }
    let mut short = std::collections::BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (i, pal) in d.species.iter().enumerate() {
        let zh = text(pal, "zh");
        for (end, _) in zh.char_indices().skip(2) {
            let prefix = &zh[..end];
            if !prefix
                .chars()
                .all(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
            {
                break;
            }
            for (start, _) in q.match_indices(prefix) {
                let end = start + prefix.len();
                let before = q[..start].trim_end();
                let after = q[end..].trim_start();
                if names.iter().any(|(_, a, b)| start < *b && *a < end)
                    || !(before.is_empty()
                        || [
                            "普通", "头目", "闪光", "比较", "对比", "和", "与", "跟", "、",
                        ]
                        .iter()
                        .any(|s| before.ends_with(s)))
                    || !(after.is_empty()
                        || [
                            "的",
                            "和",
                            "与",
                            "跟",
                            "谁",
                            "三围",
                            "种族值",
                            "基础",
                            "相比",
                            "、",
                            "?",
                            "？",
                        ]
                        .iter()
                        .any(|s| after.starts_with(s)))
                {
                    continue;
                }
                short.entry((start, end)).or_default().push(i);
            }
        }
    }
    for ((start, end), ids) in short {
        let [index] = ids.as_slice() else {
            return None;
        };
        names.push((*index, start, end));
    }
    names.sort_by_key(|(_, start, _)| *start);
    Some(names)
}

pub(super) fn compare(d: &Data, q: &str) -> Option<AssistantReply> {
    let alpha = has(q, &["头目", "领主个体", "alpha ", "alphas", "阿尔法"]);
    let lucky = has(q, &["lucky", "闪光", "幸运个体", "幸运帕鲁", "稀有个体"]);
    let stats = has(
        q,
        &[
            "base stat",
            "growth stat",
            "growth rate",
            "base hp",
            "base health",
            "base attack",
            "base def",
            "base atk",
            "种族值",
            "成长值",
            "成长系数",
            "基础数值",
            "基础属性",
            "基础面板",
            "基础攻",
            "基础防",
            "基础血",
            "基础生命",
            "三围",
            "攻防血",
        ],
    );
    let field = has(
        q,
        &[
            "health", "defense", "defence", "attack", " hp", "血量", "生命", "攻击", "防御", "面板",
        ],
    );
    if !(stats
        || (alpha || lucky)
            && field
            && has(
                q,
                &[
                    "higher",
                    "lower",
                    "more hp",
                    "different",
                    "better",
                    "stronger",
                    "更高",
                    "更低",
                    "区别",
                    "一样",
                    "差",
                ],
            ))
    {
        return None;
    }
    if has(
        q,
        &[
            "spawn",
            "inherit",
            "breeding",
            "breed",
            "drop",
            "where",
            "at level",
            "level up",
            "why is my",
            "my level",
            "should i",
            "best ",
            "strongest",
            "ranking",
            "faster",
            "stamina",
            "speed",
            "against",
            "boss fight",
            "damage",
            " iv",
            "souls",
            "percentile",
            "player",
            "passive",
            "skill",
            "捕捉率",
            "配种",
            "遗传",
            "出生",
            "孵蛋",
            "掉落",
            "在哪里",
            "伤害",
            "减伤",
            "速度",
            "耐力",
            "魂强",
            "个体值",
            "谁最",
            "最强",
            "哪个最",
            "推荐",
            "玩家",
            "技能",
            "词条",
        ],
    ) {
        return None;
    }
    let en = english(q);
    let lang = if en { "en" } else { "zh" };
    let spans = comparison_names(d, q)?;
    let mut ids = Vec::new();
    for (i, start, _) in spans {
        if crate::assistant_breeding::excluded_record_subject(&q[..start]) {
            return None;
        }
        if !ids.contains(&i) {
            ids.push(i);
        }
    }
    if ids.len() > 2
        || ids.len() == 2 && (alpha || lucky)
        || ids.len() == 2
            && !has(
                q,
                &[
                    "compare",
                    "versus",
                    " vs",
                    "difference",
                    "which",
                    "higher",
                    " and ",
                    "对比",
                    "比较",
                    "谁",
                    "差多少",
                    "差别",
                    "和",
                    "与",
                ],
            )
    {
        return None;
    }
    if !alpha && !lucky && ids.len() != 2 {
        return None;
    }
    let stats = d.get("stat-data");
    let profiles = stats["profiles"].as_object()?;
    let mut links = vec![(
        if en {
            "Stat calculator"
        } else {
            "属性计算器"
        }
        .into(),
        "#details/stats".into(),
    )];
    let mut lines = Vec::new();
    if ids.is_empty() {
        // An unknown proper name with no match must not turn into a world-wide ranking.
        if !has(
            q,
            &[
                "alpha pals",
                "lucky pals",
                "alphas ",
                "lucky pal stats",
                "头目帕鲁",
                "幸运帕鲁",
                "闪光帕鲁",
                "头目和普通",
                "头目与普通",
                "头目和闪光",
            ],
        ) {
            return None;
        }
        let ratios: Vec<_> = d
            .species
            .iter()
            .filter_map(|pal| {
                let p = profiles.get(text(pal, "key"))?;
                let normal = growth(p, "normal")?;
                let alpha = growth(p, "alpha")?;
                Some((normal, alpha))
            })
            .collect();
        let usual = ratios
            .iter()
            .filter(|(n, a)| a[0] * 5 == n[0] * 6 && a[1..] == n[1..])
            .count();
        if usual == 0 || usual * 2 <= ratios.len() {
            return None;
        }
        lines.push(if en {format!("Compared at the same level and potential, Alpha and naturally Lucky Pals use the Alpha growth profile. In {usual}/{} matched species profiles, HP growth is 20% higher while base Attack and Defense are unchanged. Some species have different growth values.",ratios.len())}
            else{format!("同种、同等级和同潜力下，头目与野生幸运个体使用头目成长表。在成功对齐的 {} 种资料中，有 {usual} 种的生命成长高 20%，基础攻击和防御相同；其余物种可按名字查实际差值。",ratios.len())});
    } else {
        lines.push(if en {
            "Base growth coefficients in HP / Attack / Defense order".into()
        } else {
            "下列为生命／攻击／防御的基础成长系数。".into()
        });
        for i in ids {
            let s = &d.species[i];
            let name = text(s, lang);
            let p = profiles.get(text(s, "key"))?;
            let normal = values(growth(p, "normal")?);
            let line = if alpha || lucky {
                let alt = values(growth(p, "alpha")?);
                if en {
                    format!("{name} — normal {normal}, Alpha / naturally Lucky {alt}.")
                } else {
                    format!("{name}：普通 {normal}，头目／野生幸运 {alt}。")
                }
            } else if en {
                format!("{name} — normal {normal}.")
            } else {
                format!("{name}：普通 {normal}。")
            };
            lines.push(line);
            links.push((name.into(), format!("#paldex/{}", text(s, "key"))));
        }
    }
    if lucky {
        let m = &stats["modifiers"]["Rare"];
        let (a, b, w) = (
            m["attack"].as_i64()?,
            m["defense"].as_i64()?,
            m["work"].as_i64()?,
        );
        lines.push(if en {format!("The Lucky passive separately adds Attack +{a}%, Defense +{b}% and Work Speed +{w}%. Its bonuses apply to the individual carrying the trait.")}
            else{format!("个体携带「稀有」被动还会获得攻击 +{a}%、防御 +{b}%、工作速度 +{w}%；这些是词条加成。")});
        links.push((
            if en { "Lucky" } else { "稀有" }.into(),
            "#details/passive/trait/Rare".into(),
        ));
    }
    lines.push(if en {
        "Level, potential and other individual bonuses determine the final displayed stats.".into()
    } else {
        "最终面板值还取决于等级、个体潜力和其他培养加成。".into()
    });
    Some(answer_lines(lines, links))
}
