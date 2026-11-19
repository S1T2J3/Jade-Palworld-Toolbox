use super::{AssistantReply, Data, Value, array, english, facilities, has, name, span, text};

fn job_name(job: &str, en: bool) -> &str {
    if en {
        super::super::work_query::JOBS
            .iter()
            .find(|j| j.0 == job)
            .map_or(job, |j| j.1)
    } else {
        job
    }
}

fn crop_match(q: &str, row: &Value) -> bool {
    let Some(crop) = text(row, "id").strip_suffix("_Plantation") else {
        return false;
    };
    let zh = text(row, "name")
        .trim_end_matches("农园")
        .trim_end_matches('园');
    let aliases: &[&str] = match crop {
        "Berry" => &["berries", "berry", "浆果", "莓果"],
        "Tomato" => &["tomatoes", "西红柿"],
        "Potato" => &["potatoes", "马铃薯"],
        "Lettuce" => &["莴苣", "莴苣叶"],
        _ => &[],
    };
    span(q, crop).is_some() || q.contains(zh) || aliases.iter().any(|s| span(q, s).is_some())
}

pub(in crate::assistant_engine) fn construction_line(row: &Value, label: &str, en: bool, catalog: &[Value]) -> String {
    let level = row["level"].as_u64();
    let materials = array(&row["materials"])
        .iter()
        .filter_map(|m| {
            let key = m[0].as_str()?;
            let display = catalog
                .iter()
                .find(|i| i["id"] == key)
                .map(|i| name(i, en))
                .unwrap_or(key);
            Some(format!("{display} × {}", m[1].as_u64()?))
        })
        .collect::<Vec<_>>()
        .join(" + ");
    match (en, level) {
        (true, Some(level)) => {
            format!("{label} — unlock at technology level {level}; construction: {materials}.")
        }
        (false, Some(level)) => {
            format!("{label}在科技等级 {level} 解锁。建造材料为{materials}。")
        }
        (true, None) => format!(
            "{label} — the handbook lists no technology level. Construction: {materials}."
        ),
        (false, None) => format!("{label}的图鉴未列科技解锁等级。建造材料为{materials}。"),
    }
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    let founding = has(
        q,
        &[
            "start my first base",
            "start a base",
            "establish a base",
            "establish my base",
            "set up a base",
            "create a base",
            "build a base",
            "建立据点",
            "建立基地",
            "创建据点",
            "第一个据点怎么建",
            "第一个基地怎么建",
            "如何建据点",
        ],
    );
    let construction = has(
        q,
        &[
            "科技等级",
            "科技几级",
            "几级",
            "多少级",
            "解锁等级",
            "建造等级",
            "what level",
            "which level",
            "tech level",
            "technology level",
            "unlocked",
            "unlock them",
            "unlock it",
            "what materials",
        ],
    ) || has(
        q,
        &[
            "how to build",
            "how do i build",
            "how can i build",
            "how to unlock",
            "build cost",
            "cost to build",
            "materials to build",
            "materials for",
            "unlock level",
            "level to build",
            "set up",
            "requirements to build",
            "怎么建",
            "如何建",
            "建造条件",
            "建造材料",
            "建造需要",
            "造一个",
            "怎么解锁",
            "几级解锁",
            "什么材料",
            "盖起来",
            "怎么开",
        ],
    );
    let roles = has(
        q,
        &[
            "工种",
            "哪种适性",
            "哪些适性",
            "工作适性",
            "需要什么适性",
            "哪种工作",
            "谁来操作",
            "什么工作",
            "谁来干活",
            "work suitability",
            "work type",
            "which job",
            "what job",
            "which skill",
            "what skill",
            "operate it",
            "work at it",
            "needs a pal",
        ],
    );
    let purpose = has(
        q,
        &[
            "用途",
            "派什么用场",
            "做什么",
            "干嘛",
            "有什么功能",
            "用来干",
            "有啥用",
            "什么用",
            "能干什么",
            "what is it for",
            "what are they for",
            "what does it do",
            "what do they do",
            "used for",
            "purpose",
            "use them",
        ],
    );
    let named_worker = has(
        q,
        &[
            "which pal",
            "what pal",
            "who should",
            "who can",
            "派谁",
            "哪只帕鲁",
            "派什么帕鲁",
            "用什么帕鲁",
            "哪种帕鲁",
        ],
    );
    if named_worker
        && (!(construction || purpose || roles)
            || !crate::assistant_breeding::species_mentions(&d.species, q)
                .0
                .is_empty()
            || has(
                q,
                &["passive", "词条", "without ", "排除", "前期", "early game"],
            ))
    {
        return None;
    }
    let operation = purpose
        || roles
        || has(
            q,
            &[
                "how to use",
                "how do i use",
                "how do you use",
                "how to grow",
                "how can i grow",
                "how do i grow",
                "how do i plant",
                "how to run",
                "how to start",
                "steps",
                "怎么用",
                "怎么运作",
                "怎么种",
                "如何种",
                "如何使用",
                "种植流程",
                "怎么生产",
                "作物流程",
                "做什么用",
                "干什么用",
                "有什么用",
            ],
        )
        || has(q, &["怎样", "如何", "分步", "步骤"]) && has(q, &["种植", "播种", "种田"])
        || q.starts_with("how do") && has(q, &[" work", " operate", " grow", " plant", " unlock"])
        || q.starts_with("what do") && (q.ends_with(" do?") || q.ends_with(" do"));
    let farming = q
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| matches!(w, "field" | "fields"))
        || has(
            q,
            &[
                "farm",
                "plantation",
                "crop field",
                "crop bed",
                "plot",
                "garden",
                "planter",
                "grow",
                "plant ",
                "农园",
                "田",
                "农场",
                "种植",
                "菜园",
                "菜圃",
                "种",
            ],
        );
    if (!construction && !operation && !farming && !founding)
        || has(
            q,
            &[
                "per hour",
                "how long",
                "how fast",
                "how many",
                "best pal",
                "where can i buy",
                "where to buy",
                "buy seeds",
                "how do i get seeds",
                "how do i get the seeds",
                "where can i find",
                "what level pal",
                "种子哪里",
                "种子怎么获得",
                "哪里买",
                "多长时间",
                "要多久",
                "每小时",
                "掉率",
                "最佳配比",
                "配方里",
                "推荐食物",
                "best food",
                "which food",
                "base layout",
                "base location",
                "base spot",
                "where should",
                "where can i build",
                "near",
                "too close",
                "基地选址",
                "据点选址",
                "建在哪里",
                "附近",
                "距离",
                "基地上限",
                "据点数量",
                "for cooking",
                "in a salad",
            ],
        )
    {
        return None;
    }
    let rows = array(&d.get("facilities-data")["facilities"]);
    let mut targets: Vec<_> = if farming {
        rows.iter().filter(|row| crop_match(q, row)).collect()
    } else {
        Vec::new()
    };
    let crops = !targets.is_empty();
    if founding && !crops {
        targets.push(rows.iter().find(|r| r["id"] == "Palbox")?);
    } else if !crops {
        if !construction
            && !purpose
            && !roles
            && !has(
                q,
                &[
                    "how to use",
                    "how do i use",
                    "what is it used for",
                    "what do i use",
                    "what is the purpose",
                    "做什么用",
                    "干什么用",
                    "用途是什么",
                    "怎么用",
                    "如何使用",
                ],
            )
        {
            return None;
        }
        let machines = facilities(d, q);
        if machines.is_empty() {
            return None;
        }
        let mut other_words = q.to_string();
        for &(_, a, b) in machines.iter().rev() {
            other_words.replace_range(a..b, " ");
        }
        // A specific input, output or modifier belongs to the interactions table.
        if !super::items(d, &other_words).is_empty()
            || !construction
                && !roles
                && machines
                    .iter()
                    .any(|(row, _, _)| text(row, "id") == super::RECYCLER)
        {
            return None;
        }
        targets.extend(machines.into_iter().map(|(machine, _, _)| machine));
    } else if !construction && !operation {
        // Short noun headings such as “lettuce and tomato plots” are common.
        // Background prose that merely mentions a farm is insufficient.
        let crop_terms = [
            "tomato", "tomatoes", "lettuce", "wheat", "berry", "berries", "carrot", "carrots",
            "onion", "onions", "potato", "potatoes",
        ];
        let bare = if english(q) {
            q.split(|c: char| !c.is_ascii_alphabetic())
                .filter(|s| !s.is_empty())
                .all(|s| {
                    crop_terms.contains(&s)
                        || [
                            "and",
                            "or",
                            "the",
                            "farm",
                            "farms",
                            "field",
                            "fields",
                            "plot",
                            "plots",
                            "plantation",
                            "plantations",
                            "garden",
                            "gardens",
                        ]
                        .contains(&s)
                })
        } else {
            let mut rest = q.to_owned();
            for row in &targets {
                let zh = text(row, "name")
                    .trim_end_matches("农园")
                    .trim_end_matches('园');
                rest = rest.replace(zh, "");
            }
            for word in [
                "西红柿",
                "莴苣",
                "马铃薯",
                "浆果",
                "莓果",
                "农田",
                "农园",
                "农场",
                "菜园",
                "菜圃",
                "田",
                "园",
                "和",
                "与",
                "及",
                "的",
            ] {
                rest = rest.replace(word, "");
            }
            rest.chars()
                .all(|c| c.is_whitespace() || "、，,?？".contains(c))
        };
        if !bare {
            return None;
        }
    }
    if crops && (construction || operation) {
        for (row, _, _) in facilities(d, q) {
            if !targets.iter().any(|existing| existing["id"] == row["id"]) {
                targets.push(row);
            }
        }
    }
    let en = english(q);
    let catalog = array(&d.get("item-catalog-data")["items"]);
    let mut lines = Vec::new();
    if !crate::assistant_discovery::handbook_version_matches(q)
        || has(q,&["旧版", "老版本", "早期版本", "抢先体验版", "old version", "early access version", "before 1.0", "pre-1.0", "v0.", "modded"])
    {
        let version = text(d.get("facilities-data"), "version");
        lines.push(if en { format!("The facility handbook covers v{version}. It does not verify the rules for the version you asked about. The values below are for v{version} only.") }
            else { format!("设施图鉴只收录 v{version}，无法确认你所问版本的规则。下面仅列 v{version} 的记录供对照。") });
    }
    let mut links = Vec::new();
    for row in targets {
        let id = text(row, "id");
        let label = if en {
            id.replace('_', " ")
        } else {
            text(row, "name").to_owned()
        };
        lines.push(construction_line(row,&label,en,catalog));
        if !id.ends_with("_Plantation") {
            let purpose = text(row, "purpose");
            let jobs = array(&row["required"])
                .iter()
                .filter_map(|j| {
                    let key = j[0].as_str()?;
                    let display = job_name(key, en);
                    Some(format!("{display} Lv.{}", j[1].as_u64()?))
                })
                .collect::<Vec<_>>();
            if !purpose.is_empty() {
                lines.push(if en {
                    translated_purpose(purpose).unwrap_or(purpose).into()
                } else {
                    purpose.into()
                });
            }
            if !jobs.is_empty() {
                lines.push(format!(
                    "{} {}",
                    if en {
                        "Required work suitability:"
                    } else {
                        "派工具体需要"
                    },
                    jobs.join(" / ")
                ));
            } else if text(row, "group") != "效率强化" && !array(&row["jobs"]).is_empty() {
                let stages = array(&row["jobs"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|job| job_name(job, en))
                    .collect::<Vec<_>>()
                    .join(" → ");
                lines.push(if en {
                    format!("Production tasks: {stages}.")
                } else {
                    format!("生产工序为{stages}。")
                });
            } else if roles || named_worker {
                lines.push(if en {
                    "No operating work suitability is listed for this facility.".into()
                } else {
                    "图鉴未列出需要指派帕鲁操作的工种。".into()
                });
            }
            let effects = array(&row["effects"])
                .iter()
                .filter_map(Value::as_str)
                .map(|effect| {
                    if en {
                        effect.replace("工作速度", "work speed")
                    } else {
                        effect.to_string()
                    }
                })
                .collect::<Vec<_>>();
            if !effects.is_empty() {
                lines.push(effects.join(" / "));
            }
            if text(row, "energy") == "Electric" {
                lines.push(if en {
                    "Supply power from a generator at the base.".into()
                } else {
                    "需要据点发电设施供电。".into()
                });
            }
        }
        if named_worker && text(row, "group") != "效率强化" {
            let jobs = array(&row["required"])
                .iter()
                .filter_map(|v| Some((v[0].as_str()?, v[1].as_u64()?)))
                .chain(
                    array(&row["jobs"])
                        .iter()
                        .filter_map(|v| Some((v.as_str()?, 1))),
                )
                .collect::<Vec<_>>();
            for (job, min) in jobs {
                let profiles = &d.get("paldex-index-data")["profiles"];
                let Some((rank, pal, key)) = d
                    .species
                    .iter()
                    .filter(|p| !text(p, "reason").contains("无法捕获"))
                    .filter_map(|p| {
                        let key = text(p, "key");
                        let rank = array(&profiles[key]["work"]).iter().find(|r| r[0] == job)?[1]
                            .as_u64()?;
                        (rank >= min).then_some((rank, text(p, if en { "en" } else { "zh" }), key))
                    })
                    .max_by_key(|(rank, _, _)| *rank)
                else {
                    continue;
                };
                let name = job_name(job, en);
                lines.push(if en {
                    format!("An eligible {name} worker is {pal} (Lv.{rank}).")
                } else {
                    format!("{name}可派{pal}（基础适性 Lv.{rank}）。")
                });
                links.push((pal.to_owned(), format!("#work/suitability/{key}")));
            }
        }
        links.push((label, format!("#work/facilities/{id}")));
    }
    if crops {
        lines.push(if en { "Build each plot inside the base and assign Pals to cover Planting, Watering and Gathering. After planting and watering, the crop grows, then Gatherers harvest it. A Transporting Pal can take the harvest to storage; keep the storage or feed box nearby.".into() }
            else { "在据点内放好各自的农田，为农园分配具备播种、浇水、采集适性的帕鲁。播种浇水后等作物长成，再由采集帕鲁收获；可让搬运帕鲁把收成送往就近的箱子或饲料箱。".into() });
    }
    if founding {
        lines.push(if en { "Unlock the Palbox in Technology, then select it in the build menu and place it in a buildable area. Interact with it to move Pals into base worker slots.".into() }
            else { "在科技栏解锁帕鲁终端后，从建造菜单选中并放到可建位置。建成后与终端互动，把帕鲁放进据点工作栏。".into() });
    }
    Some(AssistantReply {
        text: lines.join("\n"),
        links,
        suggestions: vec![],
    })
}

fn translated_purpose(zh: &str) -> Option<&'static str> {
    Some(match zh {
        "按配方制作物品。" => "Craft items from their recipes.",
        "消耗相应材料修复装备和工具。" => {
            "Spend the listed materials to repair equipment and tools."
        }
        "在据点持续取得石头。" => "Produce stone at the base.",
        "在据点持续取得木材。" => "Produce wood at the base.",
        "让采矿帕鲁在据点持续开采对应的矿物。" => {
            "Assign a Mining Pal to extract this site's ore at the base."
        }
        "熔炼矿石与其他金属材料。" => "Smelt ore into processed metals.",
        "将投入的原料加工成其他材料。" => {
            "Process the selected recipe's input into materials."
        }
        "将食材加工成料理。" => "Cook dishes from their ingredients.",
        "为据点中工作的帕鲁提供食物。" => "Store food for the base Pals to eat.",
        "制作治疗帕鲁的药品。" => "Craft medicine to treat Pals.",
        "建立据点，收纳帕鲁并分配据点成员。" => {
            "The Palbox establishes a base, stores Pals and lets you assign base workers."
        }
        "放入拥有牧场产出能力的帕鲁，收集其掉落的材料。" => {
            "Assign a Pal with a ranch-producing partner skill and collect its products."
        }
        "调整据点帕鲁的工作强度及工作分配。" => {
            "Set the base's working intensity and assign jobs."
        }
        "孵化帕鲁蛋。" => "Incubate Pal eggs.",
        "在据点栽培作物，可放在室内。" => "Grow crops indoors at the base.",
        "更换帕鲁的外观。" => "Change the appearance of compatible Pals.",
        "以技能果实为种，培育同类技能果实。" => {
            "Plant a skill fruit to grow more fruit of its kind."
        }
        "合成设计图。" => "Combine schematics.",
        "制作骑具及其他帕鲁装置。" => "Craft saddles and other Pal gear.",
        "制作帕鲁球。" => "Craft Pal Spheres.",
        "制作武器及对应的弹药。" => "Craft weapons and their ammunition.",
        "供帕鲁在据点垂钓。" => "Let base Pals fish in the pond.",
        "在据点制造金币。" => "Make gold coins at the base.",
        "提供睡眠和休息场所。" => "Provide a place for Pals to sleep and rest.",
        "让工作中的帕鲁休息并恢复 SAN 值。" => {
            "Let base workers rest and recover SAN."
        }
        "储存发电设施产生的电力。" => "Store electricity produced by generators.",
        "组建帕鲁队伍进行远征，完成后领取奖励。" => {
            "Send a team of Pals on expeditions and collect the rewards on return."
        }
        "放入雌雄亲代与蛋糕，等待产下帕鲁蛋。" => {
            "Assign a male and female parent, supply cake, and wait for an egg."
        }
        "在据点保存物品。" => "Store items inside the base.",
        "由发电帕鲁为据点设备供电。" => {
            "Assign an electricity-generating Pal to power the base."
        }
        _ => return None,
    })
}
