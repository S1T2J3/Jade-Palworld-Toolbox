use super::{BreedingSession, Data, ReferenceAnswer, SpeciesSpan, array, sex_by_name, species_mentions, text};

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|s| q.contains(s))
}

pub(super) fn goal_at(before: &str) -> bool {
    let mut s = before.trim_end();
    for suffixes in [&["普通的", "普通", "normal", "regular"][..], &["一只", "一个", " a", " an", " the"][..]] {
        s = suffixes.iter().find_map(|v| s.strip_suffix(v)).unwrap_or(s).trim_end();
    }
    ["目标是", "目标为", "想得到", "想获得", "想孵", "孵出", "孵", "配出", "生出",
     "如果要", "如果想要", "如果想", "要的是", "想要", "目标子代是", "子代是", "后代是",
     "能得到", "能获得", "能不能得到", "会不会出", "能不能出", "目标想要",
     "to get", "can i get", "could i get", "would i get", "will i get", "give me",
     "i want", "to breed", "to hatch", "to produce", "trying for", "trying to get",
     "goal is", "offspring is", "produce"].iter().any(|word| s.ends_with(word))
}

pub(super) fn mate_choices(data: &Data, q: &str) -> Option<ReferenceAnswer> {
    let breeding = has(q, &["配偶", "亲本", "配种", "繁殖", "配对", "交配", "孵出", "breed", "offspring", "mate for", "mating"]);
    let choosing = has(q, &["候选", "挑", "选", "分别", "各自", "还是", "或", "配谁", "哪只", "哪一只",
        " or ", "each", "candidate", "choose", "which mate", "which partner"]);
    if !breeding || !choosing { return None; }
    let (spans, _) = species_mentions(&data.species, q);
    if spans.len() < 3 { return None; }
    let is_goal = |s: SpeciesSpan| goal_at(&q[..s.1]) && !(q[..s.1].trim_end().ends_with("to breed")
        && q[s.2..].trim_start().starts_with("with "));
    let mut goals = spans.iter().copied().filter(|s| is_goal(*s));
    let goal = goals.next().map(|s| s.0);
    if goals.any(|s| Some(s.0) != goal) { return None; }
    let parents: Vec<_> = spans.into_iter().filter(|s| !is_goal(*s)).collect();
    if parents.len() < 3 { return None; }
    // "Give A a mate from B and C", "breed A with B or C", or
    // "choose between B and C to breed with A". All retain the same fixed parent.
    let fixed = parents.iter().position(|(_, start, end)| {
        let before = q[..*start].trim_end();
        let after = q[*end..].trim_start();
        let before = ["my", "a", "这只", "一只"].iter().find_map(|p| before.strip_suffix(p)).unwrap_or(before).trim_end();
        ["给", "替", "作为固定亲本的是", "固定亲本是", "固定亲本", "mate for", "partner for", "breed with", "to pair with"]
            .iter().any(|v| before.ends_with(v))
        || ["分别跟", "分别与", "分别和", "分别配", "的配偶", "当固定", "为固定", "跟谁配", "with either ", "with one of "]
            .iter().any(|v| after.starts_with(v))
    }).or_else(|| {
        let (_, start, end) = parents[0];
        let prefix = q[..start].trim_end();
        let bridge = q[end..parents[1].1].trim();
        (["breed", "breed my", "breed a", "cross", "交配时用", "想用"].iter().any(|w| prefix.ends_with(w))
            && ["with", "with my", "with a", "跟", "与", "和"].contains(&bridge)).then_some(0)
    })?;
    let mut options = Vec::new();
    for (i, span) in parents.iter().copied().enumerate() {
        if i != fixed && !options.iter().any(|s: &SpeciesSpan| s.0 == span.0) { options.push(span); }
    }
    if options.len() < 2 || options.len() > 8 { return None; }
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let name = |id: usize| text(&data.species[id], if english { "en" } else { "zh" });
    let gendered = |(id, start, end): SpeciesSpan| (id, sex_by_name(&q[..start], &q[end..]));
    let parent = gendered(parents[fixed]);
    let mut session = BreedingSession { english, ..BreedingSession::default() };
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut matches = Vec::new();
    for option in options {
        let result = session.offspring(data, [parent, gendered(option)]);
        if let Some(want) = goal {
            let route = format!("#lookup/parents/{}", text(&data.species[want], "key"));
            if result.links.iter().any(|(_, href)| href == &route) { matches.push(name(option.0)); }
        }
        lines.push(result.text);
        for link in result.links { if !links.contains(&link) { links.push(link); } }
    }
    if let Some(want) = goal {
        let label = name(want);
        let chosen = matches.join(if english { ", " } else { "、" });
        lines.push(match (english, matches.is_empty()) {
            (true,false) => format!("For {label}, the matching mate options with the sex assignments shown are {chosen}."),
            (true,true) => format!("None of these pairings matches {label} with the specified sexes."),
            (false,false) => format!("要得到{label}，按上列性别配置，可选的配偶是{chosen}。"),
            (false,true) => format!("以上候选与指定亲本、性别都配不出{label}。"),
        });
    }
    Some(ReferenceAnswer { text: lines.join("\n"), links })
}

pub(super) fn stage(d: &Data, q: &str) -> Option<ReferenceAnswer> {
    let first = q.split(['?', '？', '\n']).next()?;
    let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let hatched = has(q, &["已经孵", "已孵", "刚孵", "刚配出", "已经配出", "刚出生", "孵好的", "孵出来的",
        "bred pals", "newly bred", "after breeding", "already bred", "hatched", "newborn", "freshly bred"]);
    let train = has(first, &["练级", "练到", "升级", "升到", "刷经验", "培养等级", "level up", "level them",
        "level my", "level these", "level the", "leveling", "levelling", "gain exp", "gain xp", "train "]);
    if hatched && train {
        // Game survival guide: https://paldb.cc/en/Level_Up ; party battle EXP:
        // https://palworld.wiki.gg/wiki/Level . Bonuses use the shipped partner records.
        let mut lines = vec![if english {
            "Move several hatchlings from the Palbox to your active party. Keep one trained fighter or do the fighting yourself. Defeat or catch high-level Pals, or repeat an Alpha or tower boss you can finish quickly and reliably. The young Pals receive party battle EXP without needing to fight. At your target level, swap in the next batch. This works without training manuals."
        } else {
            "把要练的几只幼崽从仓库换进随身队伍，留一个位置给主力，或由玩家输出。反复挑战能快速稳定击败的野外头目、高塔首领，也可以击败或捕捉高等级野生帕鲁。幼崽待在队伍里就能分到战斗经验，不必上场。达到目标等级后再换下一批，训练书用完也能练。"
        }.to_owned()];
        let mut links = Vec::new();
        if let Some((key, row)) = d.get("partner-skills-data").as_object().into_iter().flatten().find(|(_, row)| {
            let effect = text(row, "effect");
            effect.contains("队伍帕鲁获得的经验值") && effect.contains("队伍中")
        }) && let Some(pal) = d.species.iter().find(|s| s["key"] == key.as_str()) {
            let name = text(pal, if english { "en" } else { "zh" });
            if english {
                lines.push(format!("If you have {name}, its partner skill boosts the party's Pal EXP while it occupies a party slot."));
            } else {
                lines.push(format!("还可以带{name}：「{}」{}",
                    text(row, "name"), text(row, "effect")));
            }
            links.push((name.into(), format!("#details/partner/{key}")));
        }
        return Some(ReferenceAnswer {text:lines.join("\n"),links});
    }
    if !has(first, &["how does breeding work", "how do i start breeding", "how to start breeding", "breeding basics",
                     "配种怎么玩", "配种入门", "如何开始配种", "配种流程", "配种怎么操作", "繁殖怎么操作", "配种是怎么回事"]) {return None;}
    let mut links = Vec::new();
    for id in ["Breeding_Farm", "Egg_Incubator"] {
        if let Some(facility) = array(&d.get("facilities-data")["facilities"]).iter().find(|f| f["id"] == id) {
            links.push((text(facility, "name").into(), format!("#work/facilities/{id}")));
        }
    }
    // In-game facility descriptions: https://paldb.cc/en/Pal_Breeding_Farm
    let answer = if english {
        "Build a Breeding Farm at your base. Assign a compatible male and female Pal, and place Cake in the farm's chest. When the egg appears, collect it, put it in an Egg Incubator and hatch it. To aim for a species, check its parent pairs; for a passive build, select the parents with the traits you want and check the offspring after hatching."
    } else {
        "在据点建配种牧场，把一公一母的适配亲本分配进去，将蛋糕放入牧场的箱子。等牧场产蛋后拾取帕鲁蛋，放入帕鲁蛋孵化器完成孵化。想孵指定物种就先查目标的双亲配方；想遗传指定词条，需要挑选带目标词条的亲本，再检查孵出的个体。"
    };
    Some(ReferenceAnswer {text:answer.into(),links})
}
