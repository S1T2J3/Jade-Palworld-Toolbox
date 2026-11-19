use super::{AssistantReply, Data, array, has, mentioned_traits, reply, species_mentions, text};

/// Being available in the general random pool, fixed on a species and more
/// frequent on a particular encounter are three different claims.
pub(in crate::assistant_engine) fn record(d: &Data, q: &str) -> Option<AssistantReply> {
    let natural = has(
        q,
        &[
            "必出",
            "固定带",
            "天生",
            "天生自带",
            "天生会带",
            "总会带",
            "自带",
            "必定有",
            "容易出",
            "更容易",
            "比较容易",
            "保底",
            "纯运气",
            "运气好",
            "靠运气",
            "随机刷出来",
            "能随机出",
            "随机词条",
            "more likely",
            "predisposition",
            "guaranteed",
            "always comes with",
            "always have",
            "always has",
            "fixed passive",
            "born with",
            "naturally get",
            "naturally come",
            "just luck",
            "got lucky",
            "pure luck",
            "how many tries",
            "抓多少只",
            "抓了几只",
        ],
    ) || q.contains("how many")
        && has(q, &["captur", "catch", "hatch", "eggs", "tries"]);
    if !natural
        || has(
            q,
            &[
                "继承率",
                "遗传率",
                "遗传概率",
                "chance to inherit",
                "inheritance chance",
                "伙伴技能",
                "partner skill",
                "modded",
                "with a mod",
                "装了模组",
                "装了mod",
            ],
        )
    {
        return None;
    }
    let first = q.split(['?', '？', '\n']).next()?.trim();
    let (mut owners, _) = species_mentions(&d.species, first);
    if owners.is_empty() {
        owners = species_mentions(&d.species, q).0;
    }
    owners.dedup_by_key(|r| r.0);
    let [(owner, _, _)] = owners.as_slice() else {
        return None;
    };
    let keys = mentioned_traits(d, q);
    if keys.is_empty() || keys.len() > 3 {
        return None;
    }
    let key = text(&d.species[*owner], "key");
    let data = d.get("pal-details-data");
    let fixed = data["profiles"][key]["fixed"].as_array()?;
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let label = if english { "en" } else { "zh" };
    let species = text(&d.species[*owner], label);
    let mut links = vec![(species.to_owned(), format!("#paldex/{key}"))];
    let mut lines = Vec::new();
    let mut random_roll = false;
    for id in keys {
        let trait_row = &data["passives"][id];
        let name = text(trait_row, label);
        let message = if fixed.iter().any(|f| f == id) {
            if english {
                format!("{name} is a fixed passive listed for {species}.")
            } else {
                format!("物种图鉴将「{name}」列为{species}的固定被动。")
            }
        } else if trait_row["random"].as_bool()? {
            random_roll = true;
            if english {
                format!(
                    "{name} can roll randomly and is not listed as a fixed passive of {species}. A {species} with it is consistent with a random roll."
                )
            } else {
                format!(
                    "「{name}」在可随机出现的词条池里。{species}的固定被动清单中没有它，遇到自带这个词条的个体符合随机结果。"
                )
            }
        } else {
            let examples = d
                .species
                .iter()
                .filter(|p| {
                    array(&data["profiles"][text(p, "key")]["fixed"])
                        .iter()
                        .any(|t| t == id)
                })
                .map(|p| text(p, label))
                .take(3)
                .collect::<Vec<_>>()
                .join(" / ");
            if english {
                format!(
                    "{name} is outside the general random pool and is not listed as a fixed passive for {species}.{}",
                    if examples.is_empty() {
                        String::new()
                    } else {
                        format!(" Examples of listed fixed holders: {examples}.")
                    }
                )
            } else {
                format!(
                    "「{name}」没有列入通用随机池，{species}的固定被动中也没有它。{}",
                    if examples.is_empty() {
                        String::new()
                    } else {
                        format!("图鉴列有固定持有者，例如{examples}。")
                    }
                )
            }
        };
        lines.push(message);
        links.push((name.to_owned(), format!("#details/passive/trait/{id}")));
    }
    if random_roll {
        lines.push(if english {
        "These records give neither encounter-specific odds nor a guaranteed capture or egg count. One or two specimens cannot establish a species bonus."
    }else{"这些表没有给该遭遇专属的出现率或保底只数。单凭一两只捕获或孵出的个体，也推不出这种帕鲁有额外加成。"}.into());
    }
    Some(reply(lines, links))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn species_and_random_pool_are_read_independently() {
        let d = super::super::tests::handbook();
        let q = |s: &str| record(&d, &crate::assistant_engine::field_text(s));
        assert!(
            q("Does a wild Frostallion always have Legend?")
                .unwrap()
                .text
                .contains("fixed passive listed for Frostallion")
        );
        let random = q("我连抓两只棉悠悠带自恋狂，是天生自带还是运气好？").unwrap();
        assert!(random.text.contains("随机") && random.text.contains("保底"));
        let other = q("棉悠悠能随机出炎帝吗？").unwrap();
        assert!(other.text.contains("没有列入通用随机池") && other.text.contains("焰煌"));
        assert!(q("棉悠悠的工作速度怎么增加？").is_none());
        assert!(q("两只带自恋狂的棉悠悠，遗传概率多少？").is_none());
    }
}
