use super::AssistantReply;
use crate::assistant_breeding::species_mentions;
use crate::data::{Data, array, text};
mod settings;
mod potential;

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|w| q.contains(w))
}

pub(super) fn answer(d: &Data, q: &str) -> Option<AssistantReply> {
    if let Some(answer) = settings::answer(q) {
        return Some(answer);
    }
    let en = q.is_ascii();
    // The same final IV configuration is evaluated identically by jade_core::calculate_pal_stats.
    if has(q, &["个体值", "潜力", " iv", "ivs"])
        && has(q, &["果实", "吃果", "fruit"])
        && has(
            q,
            &[
                "先天",
                "天生",
                "后天",
                "练高",
                "升级后",
                "练满",
                "一开始",
                "born",
                "later",
                "after leveling",
                "before leveling",
                "natural iv",
            ],
        )
    {
        let (pals, _) = species_mentions(&d.species, q);
        let links = pals
            .first()
            .map(|(i, _, _)| {
                vec![(
                    if en { "Pal stats" } else { "属性计算" }.into(),
                    format!("#details/stats/{}", text(&d.species[*i], "key")),
                )]
            })
            .unwrap_or_default();
        let msg = if en {
            "Given the same species, form, level, IVs and other bonuses, both end up with the same HP, attack and defense. The panel is recalculated using the current IV of each stat. A fruit used at a higher level does not cost you growth from earlier levels."
        } else {
            "在物种、形态、等级、词条及其他强化相同时，只要最终生命、攻击和防御三项潜力分别相同，面板也相同。天生达到这个值和练高等级后吃果实补上去，结果一样。面板按当前各项潜力重新计算，晚吃不会损失之前等级的成长。"
        };
        return Some(AssistantReply {
            text: msg.into(),
            links,
            suggestions: vec![],
        });
    }
    if let Some(answer) = potential::answer(d, q) { return Some(answer); }
    // PalDB's labor research table describes extra item chance per category; this is distinct from work speed.
    // https://paldb.cc/Pal_Labor_Research_Laboratory
    if has(
        q,
        &[
            "额外制作概率",
            "额外制作几率",
            "额外产出概率",
            "额外生产概率",
            "追加生产概率",
            "additional crafting probability",
            "bonus product chance",
            "extra crafting chance",
        ],
    ) {
        let lab = array(&d.get("facilities-data")["facilities"])
            .iter()
            .find(|r| r["id"] == "Pal_Labor_Research_Lab")?;
        let msg = if en {
            "Additional crafting probability is the research bonus that gives eligible recipes a chance to produce extra output. It applies to the product category in the research description. For example, a sphere research bonus applies to spheres and an ammunition bonus to ammunition. The displayed percentage is a chance, so crafting ten items does not guarantee a bonus. For your exact rate, check which category you are crafting and which research nodes you have completed."
        } else {
            "额外制作概率是工作研究带来的加成。制作对应类别的产品时，有机会拿到额外产物。例如帕鲁球研究对应帕鲁球，弹药研究对应弹药。它有随机性，做十次也没有固定多给一个。要看自己的具体概率，需要对照正在做的物品类别和已经完成的研究节点。"
        };
        return Some(AssistantReply {
            text: msg.into(),
            links: vec![(
                text(lab, "name").into(),
                "#work/facilities/Pal_Labor_Research_Lab".into(),
            )],
            suggestions: vec![],
        });
    }
    None
}
