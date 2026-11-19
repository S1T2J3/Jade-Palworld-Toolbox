use super::{AssistantReply, Data, Value, array, has, item_candidates, link, loot, occurs, text};

// Keep every tier when a player names a family, e.g. "ancient relics".
fn requested<'a>(items: &'a [Value], phrase: &str) -> Option<Vec<&'a Value>> {
    let exact = item_candidates(items, phrase, false);
    if exact.len() == 1
        && ["name", "en"]
            .iter()
            .any(|key| occurs(phrase, &super::field_text(text(exact[0], key))))
    {
        return Some(exact);
    }
    let suffix = [
        ("遗物", "遗物"),
        ("ancient relic", "遗物"),
        ("holy water", "圣水"),
        ("圣水", "圣水"),
        ("器官", "器官"),
        ("organs", "器官"),
    ]
    .iter()
    .find(|(alias, _)| occurs(phrase, alias))?
    .1;
    let family: Vec<_> = items
        .iter()
        .filter(|r| text(r, "name").ends_with(suffix))
        .collect();
    (1..=12).contains(&family.len()).then_some(family)
}

pub(super) fn answer(d: &Data, items: &[Value], q: &str) -> Option<AssistantReply> {
    if !d.ready("paldex-data")
        || has(
            q,
            &[
                "不打",
                "不用打",
                "不靠打",
                "without",
                "except",
                "不要",
                "除了",
                "只看",
                "商人",
                "配方",
                "recipe",
                "only from",
            ],
        )
    {
        return None;
    }
    let (left, right) = [
        "以及", "还有", "和", "跟", "与", "、", " and ", " & ", " plus ",
    ]
    .iter()
    .find_map(|s| q.split_once(s))?;
    let goals = [requested(items, left)?, requested(items, right)?];
    if goals[0]
        .iter()
        .any(|i| goals[1].iter().any(|j| i["id"] == j["id"]))
    {
        return None;
    }
    let (names, _) = crate::assistant_breeding::species_mentions(&d.species, q);
    if names.len() > 1 || has(q, &["头目", "狂暴", "首领", "alpha", "boss", "predator"]) {
        return None;
    }
    let named = names.first().map(|(i, _, _)| text(&d.species[*i], "key"));
    let mut reply = AssistantReply::new("", &[]);
    let mut examples = Vec::new();
    for pal in &d.species {
        let key = text(pal, "key");
        if named.is_some_and(|id| id != key) {
            continue;
        }
        let Some((level, drops)) =
            array(&d.get("paldex-data")[key]["drops"])
                .iter()
                .find_map(|group| {
                    let rows = array(&group[1]);
                    if !goals.iter().all(|goal| {
                        goal.iter()
                            .any(|item| rows.iter().any(|r| r[0] == item["id"]))
                    }) {
                        return None;
                    }
                    let details = rows
                        .iter()
                        .filter(|r| goals.iter().flatten().any(|item| item["id"] == r[0]))
                        .filter_map(|r| loot(d, r))
                        .collect::<Vec<_>>()
                        .join("、");
                    Some((group[0].as_u64()?, details))
                })
        else {
            continue;
        };
        let name = text(pal, "zh");
        let tier = if level == 0 {
            "基础档".into()
        } else {
            format!("Lv.{level} 起")
        };
        examples.push(format!("{name}（{tier}）：{drops}"));
        link(&mut reply, name, format!("#paldex/{key}"));
        if examples.len() == 2 {
            break;
        }
    }
    if examples.is_empty() {
        if let Some((idx, _, _)) = names.first() {
            let key = text(&d.species[*idx], "key");
            if array(&d.get("paldex-data")[key]["drops"]).is_empty() {
                return None;
            }
            let mut reply = super::pal_drops(d, &d.species[*idx], None, q);
            reply.text = format!(
                "普通个体的同一掉落档没有同时列出这两种材料。\n{}",
                reply.text
            );
            return Some(reply);
        }
        return None;
    }
    reply.text = format!(
        "想一起刷这两种材料，可以找下面的帕鲁。每项列出各自的数量和掉率。\n{}",
        examples.join("\n")
    );
    Some(reply)
}
