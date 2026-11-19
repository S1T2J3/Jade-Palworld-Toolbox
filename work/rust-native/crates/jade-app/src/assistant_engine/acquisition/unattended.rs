use super::{
    AssistantReply, Data, Value, array, field_text, has, link, loot, normalized, occurs, text,
};

// Only report routes which can run between visits. A production-table entry
// alone does not mean that the facility can be built by players.
pub(in crate::assistant_engine) fn answer(d: &Data, question: &str) -> Option<AssistantReply> {
    let q = normalized(question);
    if !has(
        &q,
        &[
            "被动收入",
            "被动产",
            "被动获",
            "挂机",
            "自动采",
            "自动产",
            "自动刷",
            "不想手挖",
            "不用手挖",
            "不用自己挖",
            "被动获取",
            "passively",
            "passive income",
            "passive source",
            "automate",
            "afk farm",
            "without mining by hand",
            "automatic source",
        ],
    ) || has(
        &q,
        &[
            "被动技能",
            "词条",
            "服务器",
            "离线",
            "下线",
            "mod ",
            "server",
            "offline",
            "logged out",
            "passive skill",
            "passive trait",
            "每小时",
            "per hour",
            "只要",
            "只用",
            "只能",
            "only",
            "without expeditions",
            "不要远征",
            "不靠远征",
            "不用远征",
            "不想远征",
            "不去远征",
            "不远征",
            "except",
            "除了",
            "不建",
            "without a base",
        ],
    ) || ![
        "resource-guides-data",
        "ranch-data",
        "expedition-data",
        "item-catalog-data",
    ]
    .iter()
    .all(|table| d.ready(table))
    {
        return None;
    }
    let catalog = array(&d.get("item-catalog-data")["items"]);
    let mut mentioned = Vec::new();
    for item in catalog {
        for key in ["name", "en"] {
            let name = field_text(text(item, key));
            let ore = name
                .strip_suffix("矿石")
                .filter(|s| !s.is_empty())
                .map(|s| format!("{s}矿"));
            for term in std::iter::once(name.as_str()).chain(ore.as_deref()) {
                if term.chars().count() < 2 || !occurs(&q, term) {
                    continue;
                }
                for (pos, _) in q.match_indices(term) {
                    mentioned.push((pos, pos + term.len(), item));
                }
            }
        }
    }
    mentioned.sort_by_key(|(start, end, _)| (*start, std::cmp::Reverse(*end)));
    let mut end_of_name = 0;
    let mut items = Vec::new();
    for (start, end, item) in mentioned {
        if start < end_of_name {
            continue;
        }
        end_of_name = end;
        if !items.iter().any(|other: &&Value| other["id"] == item["id"]) {
            items.push(item);
        }
    }
    if items.is_empty() || items.len() > 5 {
        return None;
    }
    let mut reply = AssistantReply::new("", &[]);
    for item in items {
        let id = text(item, "id");
        let mut routes = Vec::new();
        for station in array(&d.get("resource-guides-data")["production"])
            .iter()
            .filter(|s| s["item"] == id && s["facility"].is_string() && s["level"].is_number())
        {
            let name = text(station, "name");
            let jobs = array(&station["jobs"])
                .iter()
                .filter_map(|r| Some(format!("{} Lv.{}", r[0].as_str()?, r[1].as_u64()?)))
                .collect::<Vec<_>>();
            let operation = if jobs.is_empty() {
                text(station, "condition").to_string()
            } else {
                format!("指派{}的帕鲁。", jobs.join("、"))
            };
            routes.push(format!(
                "{name}（科技 Lv.{}）：{operation}",
                station["level"]
            ));
            link(
                &mut reply,
                name,
                format!("#work/facilities/{}", text(station, "facility")),
            );
        }
        if let Some(ranch) = d.get("ranch-data")["pals"].as_object() {
            let mut names = Vec::new();
            for (key, record) in ranch {
                let (Some(base), Some(max)) =
                    (record["base"].as_u64(), record["condensed"].as_u64())
                else {
                    continue;
                };
                let Some((level, _)) =
                    array(&record["tiers"])
                        .iter()
                        .enumerate()
                        .find(|(index, tier)| {
                            let level = *index as u64 + 1;
                            (base..=max).contains(&level)
                                && array(tier).iter().any(|r| {
                                    r["item"] == id && r["chance"].as_f64().unwrap_or(0.) > 0.
                                })
                        })
                else {
                    continue;
                };
                let Some(pal) = d.species.iter().find(|pal| pal["key"] == key.as_str()) else {
                    continue;
                };
                let name = text(pal, "zh");
                names.push(if level as u64 + 1 == base {
                    name.to_string()
                } else {
                    format!("{name}（见该物种的高档放牧产出）")
                });
            }
            if !names.is_empty() {
                routes.push(format!("指派{}到家畜牧场。", names.join("、")));
                link(&mut reply, "牧场产出表", format!("#work/ranch/item/{id}"));
            }
        }
        // Give one accessible expedition example. No claim about optimum yield.
        if let Some((stage, reward)) = array(&d.get("expedition-data")["stages"])
            .iter()
            .filter_map(|s| {
                Some((
                    s,
                    array(&s["rewards"])
                        .iter()
                        .find(|r| r[0] == id && r[3].as_f64().unwrap_or(0.) > 0.)?,
                ))
            })
            .min_by_key(|(s, _)| {
                (
                    s["hard"].as_bool().unwrap_or(true),
                    s["power"].as_u64().unwrap_or(u64::MAX),
                )
            })
        {
            let name = text(stage, "name");
            routes.push(format!(
                "派队去{name}，基础用时 {} 分钟，表列每次奖励 {}。",
                stage["minutes"],
                loot(d, reward)?
            ));
            link(
                &mut reply,
                name,
                format!("#work/expeditions/{}", text(stage, "id")),
            );
        }
        // If a named resource needs a different method (e.g. a crafting
        // recipe), keep the whole question available to the general search.
        if routes.is_empty() {
            return None;
        }
        if !reply.text.is_empty() {
            reply.text.push('\n');
        }
        reply
            .text
            .push_str(&format!("{}：{}", text(item, "name"), routes.join(" ")));
    }
    Some(reply)
}
