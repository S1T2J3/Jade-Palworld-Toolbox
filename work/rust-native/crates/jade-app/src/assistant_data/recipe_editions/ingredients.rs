//! Bind a crafting verb to its product. Other item mentions can then be checked
//! against the material IDs of that product's recipes, including alternatives.
use super::{Data, RecipeNames, ReferenceAnswer, Value, array, has_any, mentions, text};
use crate::assistant_engine::quantities::integers;
use std::collections::BTreeMap;

const VERBS: &[&str] = &[
    "制作", "製作", "合成", "打造", "生产", "加工", "烹饪", "凑够", "凑齐", "磨", "做", "造", "搓",
    "煮", "烤",
];
const UNITS: &str = "份副套双对件个枚支发把块瓶颗粒根袋包本盒箱张条只罐桶卷碗盘锅杯袋";

pub(super) fn question(q: &str) -> bool {
    let materials = has_any(q, &["材料", "原料", "配方", "用料", "耗材", "配料"]);
    let amounts = has_any(
        q,
        &[
            "多少",
            "几个",
            "几份",
            "几块",
            "几瓶",
            "几颗",
            "几枚",
            "几根",
            "几袋",
            "各要",
            "各需",
            "各用",
            "各几",
            "用量",
            "数量",
            "分别要",
            "分别需",
            "分别用",
            "要不要",
            "需要用",
            "需要哪些",
            "需要什么",
            "用不用",
            "需要吗",
            "要用到",
        ],
    );
    let craft = has_any(q, VERBS);
    // A crafting activity may only be background to a shopping, loot or staffing question.
    let other_domain = |clause: &str| {
        has_any(
            clause,
            &[
                "金币",
                "多少钱",
                "商人",
                "商店",
                "单价",
                "掉多少",
                "掉几个",
                "产几个",
                "产多少",
                "牧场",
                "放牧",
                "谁掉",
                "掉落",
                "会掉",
                "猎杀",
                "刷多少",
                "刷几只",
                "打几只",
                "多少只",
                "几只帕鲁",
                "派几",
                "几名",
                "哪只帕鲁",
                "工作适应性",
                "工种",
                "多少级生火",
                "几级生火",
                "工作适性",
                "几级浇水",
                "几个人",
                "几率",
                "概率",
            ],
        )
    };
    if q.split(BOUNDARY).any(other_domain)
        && !q.split(BOUNDARY).any(|s| {
            !other_domain(s)
                && has_any(
                    s,
                    &[
                        "材料",
                        "配方",
                        "用料",
                        "原料",
                        "需要",
                        "各要",
                        "各几",
                        "要多少",
                        "要几个",
                        "各用",
                    ],
                )
                && !has_any(s, &["怎么获得", "哪里获得", "怎么刷"])
        })
    {
        return false;
    }
    (materials && (amounts || craft) || craft && amounts)
        && !(q.contains("先") && has_any(q, &["再", "然后", "接着", "随后", "最后"]))
        && !has_any(
            q,
            &[
                "能做几个",
                "能做多少",
                "能做几份",
                "能造多少",
                "最多能",
                "能有多少",
                "最多有",
                "还差",
                "还剩",
                "还缺",
                "缺口",
                "补几",
                "再补",
                "还要补",
                "够不够",
                "够吗",
                "可以换",
                "预算",
                "成本",
                "一共花",
                "卖多少",
                "买多少",
            ],
        )
}

pub(super) struct Selection<'a> {
    pub products: Vec<&'a Value>,
    // Only the product clause may specify its grade: a "高品质" ingredient is not a grade selector.
    pub modifiers: String,
    pub quantity: Option<u64>,
    pub(super) inputs: Vec<&'a Value>,
    ambiguous: Vec<String>,
}

fn name_spans<'a>(
    rows: &'a [Value],
    names: &RecipeNames,
    q: &str,
) -> BTreeMap<(usize, usize), Vec<&'a Value>> {
    let mut spans: BTreeMap<_, Vec<&Value>> = BTreeMap::new();
    for (name, indices) in &names.0 {
        let mut hits = mentions(q, name);
        if name.chars().count() == 1 && !name.is_ascii() {
            hits.extend(q.match_indices(name).filter_map(|(a, _)| {
                let b = a + name.len();
                let left = &q[..a];
                let right = &q[b..];
                (role(q, a, b).0 > 0
                    || left.ends_with(['、', '和', '与', '跟', '及', '用'])
                    || ["各要", "各用", "各需", "分别", "和", "、", "与"]
                        .iter()
                        .any(|w| right.starts_with(w)))
                .then_some((a, b))
            }));
        }
        for span in hits {
            let records = spans.entry(span).or_default();
            for i in indices {
                let row = &rows[*i];
                if !records.iter().any(|r| std::ptr::eq(*r, row)) {
                    records.push(row);
                }
            }
        }
    }
    let all: Vec<_> = spans.keys().copied().collect();
    spans.retain(|&(a, b), _| {
        !all.iter()
            .any(|&(c, d)| c <= a && b <= d && (a, b) != (c, d))
    });
    spans
}

const BOUNDARY: &[char] = &['，', ',', '。', '！', '？', '；', ';', '?', '!', '\n'];
fn unquote(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || "「」『』“”‘’".contains(c))
}

fn excluded(q: &str, at: usize, end: usize) -> bool {
    let before = q[..at].rsplit(BOUNDARY).next().unwrap_or("");
    let after = q[end..].split(BOUNDARY).next().unwrap_or("").trim_start();
    let recipe_clause = ["配方", "做法", "那套", "那条路线", "那版用料"]
        .iter()
        .any(|s| before.contains(s) || after.contains(s));
    let direct = [
        "别按",
        "不按",
        "别照",
        "不照",
        "不要照",
        "不要按",
        "别套用",
        "别套",
        "别算",
        "别用",
        "别拿",
        "排除",
        "不要做",
        "不做",
    ]
    .iter()
    .filter_map(|s| before.rfind(s))
    .max();
    let recipe_negation = recipe_clause
        .then(|| {
            [
                "别给我",
                "不要给我",
                "不用给我",
                "别查",
                "不要查",
                "不用查",
                "不看",
                "别写",
            ]
            .iter()
            .filter_map(|s| before.rfind(s))
            .max()
        })
        .flatten();
    let last = direct.max(recipe_negation);
    if let Some(tail) = ["的配方", "那套配方", "这套配方"]
        .iter()
        .find_map(|s| after.strip_prefix(s))
        && [
            "不要算",
            "别算",
            "就别算",
            "不用查",
            "不考虑",
            "不要给我",
            "先不看",
            "先排除",
        ]
        .iter()
        .any(|s| tail.trim_start().starts_with(s))
    {
        return true;
    }
    last.is_some_and(|i| {
        !has_any(
            &before[i..],
            &["只做", "我要做", "实际做", "我做的是", "改做"],
        )
    })
}

fn role(q: &str, start: usize, end: usize) -> (u8, Option<u64>) {
    let before = unquote(q[..start].rsplit(BOUNDARY).next().unwrap_or(""));
    let after = unquote(q[end..].split(BOUNDARY).next().unwrap_or(""));
    let graded = super::GRADE_NAMES
        .iter()
        .flat_map(|words| words.iter().copied())
        .find_map(|grade| before.trim_end_matches('的').strip_suffix(grade))
        .unwrap_or(before)
        .trim_end();
    let no_unit = graded.trim_end_matches(|c| UNITS.contains(c)).trim_end();
    let amount = integers(no_unit)
        .last()
        .filter(|n| n.end == no_unit.len())
        .copied();
    let verb_prefix = amount.map_or(graded, |n| no_unit[..n.start].trim_end());
    let verb = VERBS.iter().any(|v| {
        verb_prefix.ends_with(v)
            || verb_prefix
                .trim_end_matches("的是")
                .trim_end_matches(['够', '出', '到', '成', '满'])
                .ends_with(v)
    }) || ["凑够", "凑齐", "凑到", "目标是"]
        .iter()
        .any(|v| verb_prefix.ends_with(v));
    if verb {
        let trailing = integers(after)
            .first()
            .filter(|n| {
                n.start == 0
                    && after[n.end..]
                        .trim_start()
                        .starts_with(|c| UNITS.contains(c))
            })
            .map(|n| n.value);
        return (2, amount.map(|n| n.value).or(trailing));
    }
    if [
        "的配方",
        "配方",
        "的材料",
        "的制作材料",
        "的原料",
        "的配料",
        "要用",
        "要多少",
        "需要",
        "一批",
        "每批",
        "每份",
        "用料",
        "里面",
        "当中",
        "要哪些",
    ]
    .iter()
    .any(|prefix| after.starts_with(prefix))
    {
        return (1, amount.map(|n| n.value));
    }
    (0, None)
}

pub(super) fn select<'a>(rows: &'a [Value], names: &RecipeNames, q: &str) -> Option<Selection<'a>> {
    let mut spans = name_spans(rows, names, q);
    // Crossed catalogue names such as 红色野莓 + 野莓种子 do not identify a single entry.
    // Merge the ambiguous ranges before slicing any UTF-8 text.
    let mut groups: Vec<(usize, usize, bool)> = Vec::new();
    for &(a, b) in spans.keys() {
        if let Some(last) = groups.last_mut()
            && a < last.1
        {
            last.1 = last.1.max(b);
            last.2 = true;
        } else {
            groups.push((a, b, false));
        }
    }
    let overlap: Vec<_> = groups.into_iter().filter(|g| g.2).collect();
    spans.retain(|&(a, b), _| !overlap.iter().any(|&(c, d, _)| a < d && c < b));
    let ambiguous: Vec<_> = overlap
        .iter()
        .map(|&(a, b, _)| q[a..b].to_owned())
        .collect();
    let mut ranked: Vec<_> = spans
        .iter()
        .filter_map(|(&(start, end), rs)| {
            if excluded(q, start, end) {
                return None;
            }
            let (rank, quantity) = role(q, start, end);
            let unique_recipe = spans.len() == 1
                && has_any(q, &["配方", "用料", "材料", "原料"])
                && rs.iter().any(|r| !array(&r["recipes"]).is_empty());
            (rank > 0 || unique_recipe).then_some((rank.max(1), (start, end), quantity, rs))
        })
        .collect();
    ranked.sort_by_key(|(rank, _, _, _)| std::cmp::Reverse(*rank));
    let (rank, (a, b), quantity, products) = *ranked.first()?;
    if ranked
        .iter()
        .skip(1)
        .any(|(r, _, _, other)| *r == rank && other != &products)
    {
        return None;
    }
    if products.iter().all(|r| array(&r["recipes"]).is_empty()) {
        return None;
    }
    let mut modifiers = q.to_owned();
    let mut inputs = Vec::new();
    let has_exclusions = spans.keys().any(|(start, end)| excluded(q, *start, *end));
    for (&(c, d), records) in spans.iter().rev() {
        modifiers.replace_range(c..d, " ");
        let before_mention = q[..c].rsplit(BOUNDARY).next().unwrap_or("");
        let stock_background = c < a
            && has_any(
                before_mention,
                &[
                    "我有",
                    "手里有",
                    "现有",
                    "库存",
                    "已有",
                    "手上有",
                    "包里有",
                    "仓库有",
                ],
            );
        if (c, d) != (a, b) && !excluded(q, c, d) && !stock_background {
            for row in records {
                if !products.contains(row)
                    && !inputs.iter().any(|it: &&Value| it["id"] == row["id"])
                {
                    inputs.push(*row);
                }
            }
        }
    }
    inputs.reverse();
    let unusual_quantity = quantity.is_some() && super::counted_craft(q).is_none();
    if inputs.is_empty()
        && !has_exclusions
        && ambiguous.is_empty()
        && !unusual_quantity
        && products.iter().all(|p| array(&p["recipes"]).len() < 2)
        && !has_any(
            q,
            &[
                "耗时",
                "多久",
                "几秒",
                "多少秒",
                "分钟",
                "时间",
                "成功率",
                "失败率",
                "成功概率",
                "失败概率",
                "用量",
                "各几",
            ],
        )
    {
        return None;
    }
    Some(Selection {
        products: products.clone(),
        modifiers: modifiers
            .split(BOUNDARY)
            .filter(|s| !excluded(s, s.len(), s.len()))
            .collect::<Vec<_>>()
            .join("，"),
        quantity,
        inputs,
        ambiguous,
    })
}

pub(super) fn quantities(d: &Data, row: &Value, selection: &Selection, q: &str) -> ReferenceAnswer {
    let name = text(row, "name");
    let recipes = array(&row["recipes"]);
    if recipes.is_empty() {
        return super::scaled_recipe(d, row, selection.quantity);
    }
    let mut result = ReferenceAnswer::new("");
    result.link(
        format!("{name}物品图鉴"),
        format!("#items/{}", text(row, "id")),
    );
    let route_inputs: Vec<_> = selection
        .inputs
        .iter()
        .filter(|r| {
            let label = text(r, "name");
            recipes.len() > 1
                && q.match_indices(label).any(|(i, _)| {
                    let before = &q[..i];
                    let after = &q[i + label.len()..];
                    ["选用", "选择", "采用", "拿", "用", "按"]
                        .iter()
                        .any(|v| before.ends_with(v))
                        || ["路线", "方案", "那套", "那条", "的路线", "的方案"]
                            .iter()
                            .any(|v| after.starts_with(v))
                })
        })
        .filter(|r| {
            recipes
                .iter()
                .any(|p| array(&p["materials"]).iter().any(|m| m[0] == r["id"]))
        })
        .collect();
    let only_named = !selection.inputs.is_empty()
        && route_inputs.is_empty()
        && !has_any(
            q,
            &[
                "完整",
                "所有材料",
                "全部材料",
                "还需要哪些",
                "其他材料",
                "其它材料",
                "还要什么",
                "还要哪些",
                "其余材料",
            ],
        );
    let mut lines = Vec::new();
    for (i, recipe) in recipes.iter().enumerate() {
        let qty = recipe["qty"].as_u64().filter(|n| *n > 0);
        let batches = match (selection.quantity, qty) {
            (Some(n), Some(y)) => Some(u128::from(n).div_ceil(u128::from(y))),
            _ => None,
        };
        let material_rows = array(&recipe["materials"]);
        if !route_inputs.is_empty()
            && !material_rows
                .iter()
                .any(|m| route_inputs.iter().any(|r| m[0] == r["id"]))
        {
            continue;
        }
        let is_requested = |m: &Value| selection.inputs.iter().any(|r| r["id"] == m[0]);
        // For ingredients outside every alternative, show the confirmed recipe alongside the missing entries.
        let matched = material_rows.iter().any(is_requested);
        if recipes.len() > 1
            && only_named
            && !matched
            && recipes
                .iter()
                .any(|other| array(&other["materials"]).iter().any(is_requested))
        {
            continue;
        }
        let shown: Vec<_> = material_rows
            .iter()
            .filter(|m| !only_named || !matched || is_requested(m))
            .filter_map(|m| {
                let id = m[0].as_str()?;
                let label = d.item_name(id);
                Some(match m[1].as_u64() {
                    Some(n) => format!("{label} × {}", u128::from(n) * batches.unwrap_or(1)),
                    None => format!("{label}（用量未收录）"),
                })
            })
            .collect();
        let heading = if recipes.len() > 1 {
            format!("{name}方案{}", i + 1)
        } else {
            name.to_owned()
        };
        let production = match (selection.quantity, qty, batches) {
            (Some(count), Some(y), Some(b)) => format!(
                "目标 {count} 份，每批产出 {y} 份，需做 {b} 批，实际产出 {} 份",
                b * u128::from(y)
            ),
            (_, Some(y), _) => format!("每批产出 {y} 份"),
            _ => "每批产量未收录，以下为每批用料".into(),
        };
        let ingredients = if shown.is_empty() {
            "材料记录不足".into()
        } else {
            shown.join("、")
        };
        lines.push(format!("{heading}（{production}）需要{ingredients}。"));
        if let Some(tech) = recipe["tech"].as_u64() {
            lines.push(format!("配方科技等级为 {tech}。"));
        } else if has_any(q, &["几级", "科技等级", "多少级", "解锁等级"]) {
            lines.push("该配方的科技等级未收录。".to_owned());
        }
        let stations: Vec<_> = array(&recipe["stations"])
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if !stations.is_empty() {
            lines.push(format!("可用设施有{}。", stations.join("、")));
        } else if has_any(q, &["工作台", "工位", "设施", "在哪里做", "哪个台"]) {
            lines.push("该配方的制作设施未收录。".into());
        }
    }
    let unlisted: std::collections::BTreeSet<_> = selection
        .inputs
        .iter()
        .filter(|r| {
            !recipes.iter().any(|p| {
                array(&p["materials"]).iter().any(|m| {
                    selection
                        .inputs
                        .iter()
                        .any(|other| other["name"] == r["name"] && m[0] == other["id"])
                })
            })
        })
        .map(|r| text(r, "name"))
        .collect();
    if !unlisted.is_empty() {
        lines.push(format!(
            "这个成品的配方未列入{}。",
            unlisted.into_iter().collect::<Vec<_>>().join("、")
        ));
    }
    if !selection.ambiguous.is_empty() {
        lines.push(format!(
            "「{}」对应多个相近条目，暂无法核实这个名称所指材料的用量。",
            selection.ambiguous.join("、")
        ));
    }
    for (terms, label) in [
        (
            &[
                "多久",
                "多少秒",
                "几秒钟",
                "多长时间",
                "几秒",
                "分钟",
                "耗时",
                "制作时间",
                "用时",
            ][..],
            "制作耗时",
        ),
        (
            &["成功率", "失败率", "成功概率", "失败概率"][..],
            "制作成功率",
        ),
    ] {
        if has_any(q, terms) {
            lines.push(format!("配方没有收录{label}。"));
        }
    }
    result.text = lines.join("\n");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn scoped_recipe_answers_from_the_product_and_preserves_separate_inputs_and_alternatives() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(dir.clone(), eframe::egui::Context::default()).unwrap();
        data.tables.insert(
            "item-catalog-data".into(),
            Arc::new(
                serde_json::from_slice(&std::fs::read(dir.join("item-catalog-data.json")).unwrap())
                    .unwrap(),
            ),
        );
        for (q, id, facts) in [
            (
                "准备做一副伯乐眼镜。古代文明部件和古代文明核心要几个？",
                "Accessory_TalentChecker",
                vec!["古代文明部件 × 10", "古代文明核心 × 5"],
            ),
            (
                "做11份简陋钓饵要多少面粉和红色野莓？另外要做多久？",
                "FishingBait_1",
                vec![
                    "需做 2 批",
                    "实际产出 20",
                    "面粉 × 4",
                    "红色野莓 × 8",
                    "没有收录制作耗时",
                ],
            ),
            (
                "我打算做两个碳纤维，石炭和木炭分别要多少？",
                "CarbonFiber",
                vec!["方案1", "石炭 × 4", "方案2", "木炭 × 10"],
            ),
            (
                "制作两份聚合物时优质帕鲁油和牛奶分别要几瓶？",
                "Polymer",
                vec!["优质帕鲁油 × 4", "未列入牛奶"],
            ),
            (
                "我手上有牛奶10份和蜂蜜5份，要做两个蛋糕，各还差多少？",
                "Cake",
                vec!["牛奶 × 4", "蜂蜜的现有数量已足够"],
            ),
            (
                "我想做够三碗蛋糕，只问牛奶和蛋各用多少。做完要多少秒？",
                "Cake",
                vec!["牛奶 × 21", "蛋 × 24", "没有收录制作耗时"],
            ),
            (
                "做魔渊龙的鞍具，精炼金属锭和金属锭各要几块？",
                "SkillUnlock_BlackMetalDragon",
                vec!["精炼金属锭 × 50", "金属锭 × 50"],
            ),
            (
                "想磨三袋面粉，有七份小麦。还缺多少小麦，放到哪个设施？",
                "Flour",
                vec!["小麦 × 2", "磨粉机"],
            ),
            (
                "做蛋糕要多少红色野莓种子？",
                "Cake",
                vec!["红色野莓种子", "暂无法核实"],
            ),
            (
                "用木炭那套配方做两个碳纤维，要准备哪些材料？",
                "CarbonFiber",
                vec!["木炭 × 10", "喷火器官 × 2"],
            ),
        ] {
            let mut chat = Conversation::default();
            let answer = chat.ask_world(q, &data, &serde_json::json!({}));
            assert!(chat.direct_result(), "{q}: {}", answer.text);
            assert_eq!(
                answer.links.first().map(|link| link.1.as_str()),
                Some(format!("#items/{id}").as_str()),
                "{q}: {}",
                answer.text
            );
            for fact in facts {
                assert!(
                    answer.text.contains(fact),
                    "{q}: {} (expected {fact})",
                    answer.text
                );
            }
        }
        for q in [
            "我打算做蛋糕，买两瓶牛奶要多少金币？",
            "做一副伯乐眼镜，古代文明部件要刷多少只头目？",
            "据点在磨面粉，要多少只会浇水的帕鲁才能开工？",
            "我在做沙拉。制作台安排哪只帕鲁帮忙，要多少级生火？",
        ] {
            assert!(!question(q), "Quantity of a different subject: {q}");
        }
        for q in [
            "做两份普通布，别照优质的布的配方算。羊毛要多少？",
            "做两份普通布，不用给我优质的布的配方。羊毛要多少？",
            "做两份普通布。优质的布的配方先不看。羊毛要多少？",
        ] {
            let mut chat = Conversation::default();
            let answer = chat.ask_world(q, &data, &serde_json::json!({}));
            assert!(
                chat.direct_result()
                    && answer.text.contains("羊毛 × 4")
                    && !answer.text.contains("优质的布"),
                "{q}: {}",
                answer.text
            );
        }
    }
}
