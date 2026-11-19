use super::{AssistantReply, Data, Plan, has};
use crate::assistant_engine::quantities::integers;

fn base_count(q: &str) -> Option<u64> {
    integers(q).into_iter().find_map(|n| {
        let rest = q[n.end..].trim_start();
        let rest = rest
            .trim_start_matches(['个', '座', '处', '所'])
            .trim_start();
        (rest.starts_with("据点") || rest.starts_with("基地") || rest.starts_with("base"))
            .then_some(n.value)
    })
}

fn term(q: &str, options: &[&str]) -> bool {
    options.iter().any(|word| {
        if word.is_ascii() {
            q.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|v| v == *word)
        } else {
            q.contains(word)
        }
    })
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if !has(q, &["据点", "基地", "base"]) {
        return None;
    }
    let count = base_count(q);
    let zoning = has(
        q,
        &[
            "用途",
            "各自负责",
            "分别负责",
            "怎么分工",
            "职能",
            "分配比较好",
            "怎么规划",
            "如何规划",
            "怎么分配",
            "dedicate",
            "specialize",
            "specialise",
            "split production",
            "divide the work",
            "divide my",
            "focus each",
            "use each base",
            "each base for",
        ],
    );
    let minerals = [
        ("Ore_Mining_Site", &["金属矿", "铁矿", "ore"][..]),
        ("Coal_Mine", &["煤炭", "石炭", "coal"][..]),
        ("Sulfur_Mine", &["硫磺", "sulfur", "sulphur"][..]),
        ("Pure_Quartz_Quarry", &["纯水晶", "石英", "quartz"][..]),
    ];
    let wanted: Vec<_> = minerals
        .iter()
        .filter(|(_, synonyms)| term(q, synonyms))
        .collect();
    let shared_mining = wanted.len() >= 2
        && (count.is_some()
            || has(
                q,
                &[
                    "同一",
                    "同个",
                    "只有一个",
                    "同个基地",
                    "同一处",
                    "一处",
                    "放一起",
                    "同址",
                    "多种",
                    "same base",
                    "single base",
                    "one base",
                    "together",
                    "same spot",
                ],
            ))
        && has(
            q,
            &[
                "怎么",
                "如何",
                "能不能",
                "能否",
                "可以",
                "只",
                "不够",
                "how",
                "can",
                "only",
                "solve",
                "together",
            ],
        );
    if !shared_mining && (!zoning || !count.is_some_and(|n| (1..=10).contains(&n))) {
        return None;
    }
    if has(
        q,
        &[
            "遭袭",
            "入侵",
            "袭击",
            "raid",
            "坐标",
            "coordinates",
            "解锁几个",
            "上限设置",
            "base limit setting",
        ],
    ) {
        return None;
    }
    let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let mut plan = Plan {
        data,
        en,
        text: Vec::new(),
        links: Vec::new(),
    };
    if shared_mining {
        let stations = wanted
            .iter()
            .map(|(id, _)| plan.station(id))
            .collect::<Option<Vec<_>>>()?;
        let rows = stations.join(if en { ", " } else { "、" });
        plan.text.push(if en {
            format!("You can place several unlocked mining sites inside one base. The sites for the resources you named are {rows}. Assign Mining Pals and transporters, with storage near their work area. Until you unlock a site, gather its mineral from natural nodes or visit a separate mining location.")
        } else {
            format!("这些矿物可以共用一个据点。相应的开采设施是{rows}。已解锁的设施可以建在同一据点，安排采矿和搬运工，把箱子放在作业区附近。尚未解锁的矿物先到天然矿点采集。")
        });
    } else {
        let count = count?;
        let bench = plan.station("Primitive_Workbench")?;
        let furnace = plan.station("Primitive_Furnace")?;
        let ranch = plan.station("Ranch")?;
        let breeder = plan.station("Breeding_Farm")?;
        let mine = plan.station("Ore_Mining_Site")?;
        let roles: Vec<String> = if en {
            match count {
                1 => vec![format!(
                    "With one base, separate work areas inside it. Put {bench} and {furnace} near materials storage, fields and {ranch} by food storage, and reserve a clear area for {mine} or {breeder} when unlocked."
                )],
                2 => vec![
                    format!(
                        "Base 1: crafting and mining. Put {mine}, {furnace}, {bench} and storage together; assign Mining, Kindling and Transporting."
                    ),
                    format!(
                        "Base 2: food and breeding. Group crop fields, {ranch}, a kitchen and {breeder}. Carry Cake to the breeding farm's box."
                    ),
                ],
                _ => vec![
                    format!(
                        "Base 1: main workshop and stores. Keep {bench}, {furnace} and frequently used materials together."
                    ),
                    format!(
                        "Base 2: resources. Use natural deposits until you unlock {mine}. Assign miners and transporters."
                    ),
                    format!(
                        "Base 3: food and breeding. Group fields and {ranch} with a kitchen, then deliver Cake to {breeder}."
                    ),
                ],
            }
        } else {
            match count {
                1 => vec![format!(
                    "只有一处时可在据点内分区。在仓库附近放{bench}和{furnace}，农田和{ranch}靠近储粮处，有了{mine}或{breeder}再预留独立作业区。"
                )],
                2 => vec![
                    format!(
                        "第一处负责制作、冶炼和采矿。把{bench}、{furnace}、{mine}和仓库放在一起，安排采矿、生火和搬运。"
                    ),
                    format!(
                        "第二处负责供餐与繁殖。农田、{ranch}和厨房集中生产食材，把烤好的蛋糕送到{breeder}的箱子。"
                    ),
                ],
                _ => vec![
                    format!("第一处做主工坊和仓库。把{bench}、{furnace}和常用材料放在一起。"),
                    format!(
                        "第二处生产矿物。前期靠天然矿点，解锁{mine}后安排采矿和搬运，先就地存放。"
                    ),
                    format!(
                        "第三处供餐和繁殖。农田、{ranch}和厨房集中供料，把做好的蛋糕送到{breeder}。"
                    ),
                ],
            }
        };
        plan.text.extend(roles);
        if count > 3 {
            plan.text.push(if en {"Use the remaining bases for your current bottleneck, such as a second resource site or more breeding capacity."}
                else {"其余据点按当前瓶颈扩建，例如再设资源点或增加繁殖容量。"} .into());
        }
        if count > 1 {
            let chest = plan.station("Guild_Chest")?;
            plan.text.push(if en {format!("For transfers, move stock between bases yourself or put it into {chest} for access from a second guild chest.")}
                else {format!("跨据点调料可手动转运；有了{chest}，放进去的物资可从同公会另一处公会箱取出。")});
        }
    }
    Some(AssistantReply {
        text: plan.text.join("\n"),
        links: plan.links,
        suggestions: vec![],
    })
}
