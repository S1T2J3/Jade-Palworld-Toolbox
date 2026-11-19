use super::{AssistantReply, Data, array, has, job_name, text};
use crate::assistant_engine::{facility_effect, field_text};

fn target_verb(before: &str, verbs: &[&str]) -> bool {
    let mut words = before.split_whitespace().rev();
    let word = words.find(|w| {
        ![
            "the", "a", "an", "my", "our", "this", "that", "new", "ordinary", "palworld",
        ]
        .contains(w)
    });
    word.is_some_and(|w| verbs.contains(&w))
}

fn unknown_prompt(q: &str, en: bool) -> Option<AssistantReply> {
    if !has(q, &["base", "camp", "据点", "基地"])
        || !has(
            q,
            &[
                "machine",
                "workstation",
                "facility",
                "station",
                "机器",
                "设备",
                "设施",
                "工位",
            ],
        )
        || !has(
            q,
            &[
                "no button prompt",
                "no prompt",
                "button does nothing",
                "can't interact",
                "cannot interact",
                "can't use",
                "没反应",
                "没提示",
                "按不动",
                "点不动",
                "无法互动",
                "不能互动",
                "不能操作",
            ],
        )
    {
        return None;
    }
    let text = if en {
        "Which facility is it, and is the interaction prompt missing or is a specific action greyed out? Face its working side and check the Interact binding for your current controller or keyboard. A manual crafting station needs an order; a passive structure may have no command to start. In multiplayer, compare the same action at the same facility with a guild member before attributing it to host permissions."
    } else {
        "这是哪种设施？靠近工作面时，是完全没有互动提示，还是能打开面板但某项操作是灰色的？先核对当前手柄或键鼠的互动绑定。加工设备通常要先下工单，被动生效的设施可能没有启动操作。联机时让同公会成员在同一设施做同样操作，再判断是否与权限有关。"
    };
    Some(AssistantReply {
        text: text.into(),
        links: vec![],
        suggestions: vec![],
    })
}

pub(super) fn answer(data: &Data, question: &str) -> Option<AssistantReply> {
    // Background may name unrelated stations; the action and object must both be in the request.
    let q = question.lines().next()?.trim();
    if has(
        q,
        &[
            "in terraria",
            "in minecraft",
            "worker thread",
            "不是帕鲁",
            "不是幻兽帕鲁",
        ],
    ) {
        return None;
    }
    if !has(
        q,
        &[
            "make",
            "craft",
            "build",
            "construct",
            "relocat",
            "reposition",
            "move",
            "start",
            "button",
            "prompt",
            "interact",
            "料",
            "挪",
            "搬",
            "改位置",
            "摆错",
            "换位置",
            "点不动",
            "按不动",
            "没反应",
            "互动",
            "开工",
            "怎么造",
            "如何造",
            "建造",
        ],
    ) {
        return None;
    }
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let Some((station, start, end)) = facility_effect::facility(data, q) else {
        return unknown_prompt(q, en);
    };
    let id = text(station, "id");
    let title = if en {
        id.replace('_', " ")
    } else {
        text(station, "name").into()
    };
    let before = q[..start].trim_end();
    let after = q[end..].trim_start();
    let version_specific = !crate::assistant_discovery::handbook_version_matches(q)
        || has(
            q,
            &[
                "旧版",
                "老版本",
                "模组服",
                "旧版本",
                "old version",
                "before 1.0",
                "pre-1.0",
                "v0.",
                "modded",
            ],
        );
    let linked = |body: String| {
        let body = if version_specific {
            let version = text(data.get("facilities-data"), "version");
            if en {
                format!(
                    "The bundled facility table covers v{version}. Your version or mods may differ.\n{body}"
                )
            } else {
                format!("随包设施表版本为 v{version}，你所用的旧版或模组可能不同。\n{body}")
            }
        } else {
            body
        };
        AssistantReply {
            text: body,
            links: vec![(title.clone(), format!("#work/facilities/{id}"))],
            suggestions: vec![],
        }
    };
    let moving = target_verb(before, &["move", "relocate", "reposition"])
        || ["挪动", "搬动", "移动", "迁移", "挪一下", "搬走"]
            .iter()
            .any(|v| before.ends_with(v))
        || has(
            after,
            &[
                "摆错",
                "挪到",
                "挪走",
                "挪个",
                "换位置",
                "改位置",
                "搬到",
                "搬走",
            ],
        );
    if moving && id != "Palbox" {
        let mut body = if en {
            format!(
                "Before relocating the {title}, collect its contents and finished output, and finish or cancel any active order after checking its status. Select it in Build mode. If your version offers no move action for this structure, dismantle the empty structure and rebuild it at the new site. Check the displayed material refund and keep the new footprint and approach clear."
            )
        } else {
            format!(
                "要挪动{title}，先取走里面的物品与成品，查看进行中的订单，等其完成或确认后取消。在建造模式中选中这座设施。如果当前版本没有移动选项，再拆除腾空的设施，到新位置重建。先看界面列出的材料返还，并留出占地和通道。"
            )
        };
        if station["energy"] == "Electric" {
            body.push_str(if en {
                " Verify that the new site has power in the same base."
            } else {
                " 重建后确认新位置在有供电的同一据点内。"
            });
        }
        if array(&station["required"]).iter().any(|r| r[0] == "冷却") {
            body.push_str(if en { "For refrigerated storage, confirm the Cooling worker is active before putting perishable food back." }
                else { "若用来保鲜，先确认有冷却工人到位，再放回易腐食物。" });
        }
        return Some(linked(body));
    }
    let building = target_verb(before, &["make", "craft", "build", "construct"])
        || ["建造", "制作", "新建", "搭建"]
            .iter()
            .any(|v| before.ends_with(v))
        || has(
            after,
            &[
                "怎么造出来",
                "怎么建",
                "如何建",
                "怎么造",
                "如何造",
                "怎么搭建",
                "建筑怎么制作",
            ],
        );
    let operating = has(
        q,
        &[
            "start making",
            "start an order",
            "load ingredients",
            "start crafting",
            "开工",
            "投料",
            "怎么开动",
            "启动订单",
            "开始加工",
        ],
    );
    if building
        && !operating
        && !has(
            q,
            &[
                "can't build",
                "cannot build",
                "cant build",
                "建不了",
                "放不下",
            ],
        )
        && !has(
            q,
            &[
                "where to build",
                "where should i build",
                "建议建在哪",
                "不想造",
                "不是要造",
            ],
        )
    {
        let catalog = array(&data.get("item-catalog-data")["items"]);
        let requirements = facility_effect::construction_line(station, &title, en, catalog);
        let next = if en {
            "Learn the technology, carry the materials or store them in this base, select the structure in the Build menu, place the blueprint and complete construction."
        } else {
            "先在科技页学会它，准备好材料，在据点的建造菜单选中该建筑、放置蓝图，再完成施工。"
        };
        return Some(linked(format!("{requirements}\n{next}")));
    }
    if !operating
        || ![
            "制作/修理",
            "磨粉/粉碎",
            "精炼",
            "烹饪",
            "制药",
            "帕鲁球",
            "武器",
        ]
        .contains(&text(station, "group"))
    {
        return None;
    }
    let zh_station = text(station, "name");
    let items = array(&data.get("item-catalog-data")["items"]);
    let mut products = items.iter().filter_map(|item| {
        let name = text(item, if en { "en" } else { "name" });
        if name.len() < 2 || !q.contains(&field_text(name)) {
            return None;
        }
        let recipe = array(&item["recipes"])
            .iter()
            .find(|r| array(&r["stations"]).iter().any(|s| s == zh_station))?;
        Some((item, recipe))
    });
    let target = products.next();
    if products.next().is_some() {
        return None;
    }
    let mut lines = vec![];
    if let Some((output, recipe)) = target {
        lines.push(facility_effect::recipe_line(data, output, recipe, en));
    }
    let jobs = array(&station["required"])
        .iter()
        .filter_map(|r| {
            Some(format!(
                "{} Lv.{}",
                job_name(r[0].as_str()?, en),
                r[1].as_u64()?
            ))
        })
        .collect::<Vec<_>>()
        .join(" / ");
    if jobs.is_empty() {
        return None;
    }
    lines.push(if en {format!("At the {title}, select the recipe and quantity, supply the listed ingredients and start the order. Assign a base worker with {jobs}, then collect the output.")}
        else {format!("在已建好的{title}打开配方面板，选择要做的物品和数量，备齐配方原料后启动工单。派有{jobs}的据点帕鲁工作，完成后收取成品。")});
    let mut result = linked(lines.join("\n"));
    if let Some((item, _)) = target {
        let name = text(item, if en { "en" } else { "name" });
        result
            .links
            .push((name.into(), format!("#items/{}", text(item, "id"))));
    }
    Some(result)
}
