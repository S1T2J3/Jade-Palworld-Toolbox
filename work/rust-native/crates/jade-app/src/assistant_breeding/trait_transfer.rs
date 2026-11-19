use super::{Data, ReferenceAnswer, Value, array, species_mentions, text};
use crate::assistant_engine::field_text;

fn contains_term(q: &str, term: &str) -> bool {
    !term.is_empty()
        && q.match_indices(term).any(|(pos, name)| {
            !term.is_ascii()
                || (!q[..pos].ends_with(|c: char| c.is_ascii_alphanumeric())
                    && !q[pos + name.len()..].starts_with(|c: char| c.is_ascii_alphanumeric()))
        })
}

pub(super) fn answer(d: &Data, q: &str) -> Option<ReferenceAnswer> {
    let title = q.split(['\n', '?', '？']).next()?.trim();
    let has = |words: &[&str]| words.iter().any(|s| title.contains(s));
    if !has(&[
        "怎么配",
        "配出",
        "配一只",
        "配个",
        "如何配",
        "怎么让",
        "如何让",
        "如何获得",
        "怎么获得",
        "how to get",
        "how do i get",
        "how can i get",
        "breed a",
        "breed for",
        "breed my",
        "pass down",
        "breed into",
    ]) || has(&[
        "概率",
        "几率",
        "几只",
        "几颗",
        "仓库",
        "我只有",
        "排除",
        "不想用",
        "具体几代",
        "chance",
        "odds",
        "percent",
        "how many",
        "without",
        "only",
        "inventory",
        "my box",
        "alpha",
        "gender",
        "性别",
        "异色",
        "幸運",
    ]) {
        return None;
    }
    let (targets, _) = species_mentions(&d.species, title);
    let [(target, _, _)] = targets.as_slice() else {
        return None;
    };
    let target = *target;
    let details = d.get("pal-details-data");
    // Fixed elemental lord traits and Legend are hereditary. Other passives
    // can have different acquisition rules and should use their own records.
    let mut matching = details["passives"].as_object()?.iter().filter(|(id, p)| {
        let en = field_text(text(p, "en"));
        (*id == "Legend" || id.starts_with("ElementBoost_") && id.ends_with("_2_PAL"))
            && (contains_term(title, &field_text(text(p, "zh")))
                || contains_term(title, &en)
                || en.split_once(' ').is_some_and(|(first, second)| {
                    second.len() >= 6
                        && !second.contains(' ')
                        && contains_term(title, &format!("{first} {}", &second[..3]))
                }))
    });
    let (id, passive) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let label = if title.is_ascii() { "en" } else { "zh" };
    let name = |i| text(&d.species[i], label);
    let trait_name = text(passive, label);
    let fixed = |i: usize| {
        array(&details["profiles"][text(&d.species[i], "key")]["fixed"])
            .iter()
            .any(|key| key == id.as_str())
    };
    let target_name = name(target);
    let mut links = vec![
        (
            trait_name.to_owned(),
            format!("#details/passive/trait/{id}"),
        ),
        (
            target_name.to_owned(),
            format!("#lookup/parents/{}", text(&d.species[target], "key")),
        ),
    ];
    if fixed(target) {
        let message = if label == "en" {
            format!(
                "{trait_name} is already listed as a fixed passive of {target_name}. Obtain that species and check the passive on the individual you want to keep."
            )
        } else {
            format!(
                "图鉴将「{trait_name}」列为{target_name}的固定被动。可以先获取这个物种，检查打算保留的个体的词条。"
            )
        };
        return Some(ReferenceAnswer {
            text: message,
            links,
        });
    }
    // Shortest examples use the same sex-aware pair records as the planner.
    // Each reached species can carry the trait after screening its offspring.
    let mut reached: Vec<Option<Vec<&Value>>> = (0..d.species.len())
        .map(|i| fixed(i).then(Vec::new))
        .collect();
    for _ in 0..2 {
        let previous = reached.clone();
        for pair in &d.pairs {
            let a = pair[0].as_u64()? as usize;
            let b = pair[1].as_u64()? as usize;
            let child = pair[2].as_u64()? as usize;
            if reached.get(child)?.is_some() {
                continue;
            }
            let Some(path) = previous.get(a)?.as_ref().or(previous.get(b)?.as_ref()) else {
                continue;
            };
            let mut next = path.clone();
            next.push(pair);
            reached[child] = Some(next);
        }
        if reached[target].is_some() {
            break;
        }
    }
    let steps = reached[target].as_ref()?;
    let first = steps.first()?;
    let donor = [first[0].as_u64()? as usize, first[1].as_u64()? as usize]
        .into_iter()
        .find(|&i| fixed(i))?;
    let mut lines = vec![if label == "en" {
        format!(
            "Start with a {} that actually has {trait_name}. Use opposite-sex parents.",
            name(donor)
        )
    } else {
        format!(
            "从实际带「{trait_name}」的{}起步，每一步用一公一母。",
            name(donor)
        )
    }];
    for pair in steps {
        let parent = |slot: usize| {
            let n = name(pair[slot].as_u64()? as usize);
            let sex = match (pair.get(slot + 3).and_then(Value::as_str), label) {
                (Some("F"), "en") => " (female)",
                (Some("M"), "en") => " (male)",
                (Some("F"), _) => "（母）",
                (Some("M"), _) => "（公）",
                _ => "",
            };
            Some(format!("{n}{sex}"))
        };
        let child = pair[2].as_u64()? as usize;
        lines.push(format!("{} × {} → {}", parent(0)?, parent(1)?, name(child)));
        let link = (
            name(child).to_owned(),
            format!("#lookup/parents/{}", text(&d.species[child], "key")),
        );
        if !links.contains(&link) {
            links.push(link);
        }
    }
    lines.push(if label=="en"{format!("The pair determines the species. Hatch and check the offspring at each step, using one that inherited {trait_name} before going on.")}
        else {format!("组合确定子代物种，但每个蛋的词条有随机性。每一代都先筛到继承「{trait_name}」的子代，再继续下一步。")});
    Some(ReferenceAnswer {
        text: lines.join("\n"),
        links,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_trait_uses_the_pair_table_and_respects_target_and_probability() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        let source = std::fs::read(root.join("pal-details-data.json")).unwrap();
        data.tables.insert(
            "pal-details-data".into(),
            std::sync::Arc::new(serde_json::from_slice(&source).unwrap()),
        );
        let ask = |s: &str| answer(&data, &field_text(s));
        let inheritance = ask("How to get a Flaracle with Flame Emperor?").unwrap();
        assert!(
            inheritance.text.contains("Blazamut")
                && inheritance.text.contains("→ Flaracle")
                && inheritance.text.contains("inherited")
        );
        let fixed = ask("怎么获得带岩帝的阿努比斯？").unwrap();
        assert!(fixed.text.contains("固定被动"));
        assert!(ask("How many eggs do I need to breed a Flaracle with Flame Emperor?").is_none());
        assert!(ask("How can I get a Flaracle with a passive skill I invented?").is_none());
        assert!(ask("How do I get a Jetragon and a Flaracle with Legend?").is_none());
    }
}
