//! Parse an accessory request into independent filters, then query the installed dex.
use crate::assistant_engine::AssistantReply;
use crate::assistant_equipment::{EquipmentQuery, Goal, Recipient, recommend};
use crate::data::{Data, array, normalize, text};

#[derive(Clone, Debug, Default)]
pub(crate) struct RecommendationSession {
    query: Option<EquipmentQuery>,
    items: Vec<String>,
    needs_goal: bool,
}

fn has(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| q.contains(term))
}

fn affirmed(q: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| {
        q.match_indices(term).any(|(i, _)| {
            ![
                "不要",
                "不想要",
                "不用",
                "不是",
                "不是给",
                "不考虑",
                "排除",
                "不追求",
            ]
            .iter()
            .any(|negative| q[..i].trim_end().ends_with(negative))
        })
    })
}

fn gear_slot(q: &str) -> bool {
    has(
        q,
        &[
            "饰品", "首饰", "配饰", "饰物", "首飾", "护符", "挂坠", "吊坠", "项链", "戒指", "指环",
            "挂件", "accessor",
        ],
    )
}

fn goals(q: &str) -> Vec<Goal> {
    [
        (Goal::WorkSpeed, &["工作", "干活", "干得快", "生产速度"][..]),
        (
            Goal::Carry,
            &[
                "负重",
                "载重",
                "背得多",
                "多背",
                "背更多",
                "背不动",
                "拿不动",
                "超重",
                "多扛",
                "扛更多",
                "多带点矿",
            ],
        ),
        (
            Goal::Defense,
            &[
                "防御", "防禦", "减伤", "抗伤", "坦度", "更肉", "变肉", "抗揍", "耐打", "扛打",
            ],
        ),
        (Goal::Health, &["生命", "血量", "血上限", "加血"]),
        (
            Goal::Attack,
            &[
                "输出",
                "增伤",
                "加伤",
                "提升伤害",
                "提高伤害",
                "伤害高",
                "伤害低",
                "攻击",
                "攻擊",
                "打人疼",
                "打得更疼",
                "火力",
                "开枪",
                "打伤害",
                "dps",
            ],
        ),
    ]
    .into_iter()
    .filter_map(|(goal, terms)| affirmed(q, terms).then_some(goal))
    .collect()
}

fn recipient(q: &str) -> Option<Recipient> {
    if has(q, &["都要", "两边", "两个都", "一起提升", "双修"]) {
        return Some(Recipient::Any);
    }
    let pal = affirmed(
        q,
        &[
            "帕鲁用",
            "给帕鲁",
            "让帕鲁",
            "靠帕鲁",
            "队伍帕鲁",
            "帕鲁输出",
            "帕鲁的",
            "帕鲁攻击",
            "宠物",
            "伙伴输出",
            "宠伤",
            "打手",
            "站场",
            "帕鲁呢",
            "带的帕鲁",
        ],
    );
    let player = affirmed(
        q,
        &[
            "玩家",
            "自身",
            "我自己",
            "我本人",
            "给我用",
            "自己用",
            "主角",
            "角色",
            "人物",
            "人伤",
            "开枪",
        ],
    );
    match (pal, player) {
        (true, false) => Some(Recipient::Pal),
        (false, true) => Some(Recipient::Player),
        (true, true) => Some(Recipient::Any),
        _ => None,
    }
}

fn element(q: &str) -> Option<&'static str> {
    ["暗", "火", "冰", "雷", "水", "地", "草", "无", "龙"]
        .into_iter()
        .find(|e| {
            ["系", "属性", "伤", "技能", "招式"]
                .iter()
                .any(|suffix| affirmed(q, &[&format!("{e}{suffix}")]))
        })
        .or_else(|| affirmed(q, &["电系"]).then_some("雷"))
}

fn player_level(q: &str) -> Option<u32> {
    q.match_indices('级').find_map(|(i, _)| {
        let prefix = q[..i].trim_end();
        let start = prefix.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        prefix[start..].parse().ok()
    })
}

fn clarify(message: &str, choices: &[&str]) -> AssistantReply {
    AssistantReply {
        text: message.to_owned(),
        links: vec![],
        suggestions: choices.iter().map(|s| (*s).to_owned()).collect(),
    }
}

impl RecommendationSession {
    /// Resolve a reference only when the player points at a result we actually displayed.
    pub(crate) fn referenced_question(&self, q: &str, data: &Data) -> Option<String> {
        let intent = if has(
            q,
            &[
                "怎么做",
                "怎么制作",
                "怎么合成",
                "配方",
                "哪些材料",
                "什么材料",
            ],
        ) {
            "怎么合成"
        } else if has(
            q,
            &[
                "怎么拿",
                "哪儿拿",
                "哪儿弄",
                "怎么弄",
                "哪里刷",
                "在哪刷",
                "怎么获得",
                "怎么获取",
                "哪儿掉",
                "哪里出",
                "哪来",
            ],
        ) {
            "怎么获得"
        } else if has(q, &["效果", "有什么用", "具体加多少"]) {
            "有什么用"
        } else {
            return None;
        };
        let ordinal = [
            &["第一个", "第一件", "第一种", "第1个", "第1件", "首选"][..],
            &["第二个", "第二件", "第二种", "第2个", "第2件"],
            &["第三个", "第三件", "第三种", "第3个", "第3件"],
        ]
        .iter()
        .position(|forms| has(q, forms));
        let index = ordinal
            .or_else(|| (self.items.len() == 1 && has(q, &["这个", "它", "这件"])).then_some(0))?;
        let key = self.items.get(index)?;
        Some(format!("{}{intent}？", data.item_name(key)))
    }

    pub(crate) fn respond(&mut self, data: &Data, question: &str) -> Option<AssistantReply> {
        let q = normalize(question);
        let q = q.as_str();
        let slot = gear_slot(q);
        let worn = has(
            q,
            &[
                "戴什么",
                "带什么",
                "戴啥",
                "带啥",
                "戴哪",
                "带哪",
                "戴个",
                "该戴",
                "穿什么",
                "穿啥",
                "穿哪",
                "身上的装备",
                "身上装备",
                "换装备",
                "换件装备",
                "装备搭配",
                "装备推荐",
            ],
        );
        let direct_lookup = has(
            q,
            &[
                "有什么用",
                "什么效果",
                "怎么获取",
                "怎么获得",
                "哪里获得",
                "配方是什么",
                "怎么合成",
                "怎么制作",
                "怎么解锁",
                "介绍一下",
                "图鉴详情",
            ],
        );
        let unrelated_topic = has(
            q,
            &[
                "配种",
                "孵化",
                "配方是什么",
                "合成表",
                "蛋糕",
                "繁殖",
                "词条",
                "帕鲁球",
                "帮我写",
                "写一篇",
                "股票",
                "显卡",
                "手机",
                "现实里",
                "现实中",
            ],
        );
        if unrelated_topic || direct_lookup && !has(q, &["推荐", "哪些", "哪种"]) {
            return None;
        }
        if !slot
            && !worn
            && has(
                q,
                &[
                    "怎么",
                    "为什么",
                    "如何",
                    "怎么办",
                    "是哪只",
                    "配队",
                    "用什么武器",
                    "最强帕鲁",
                    "最强的帕鲁",
                ],
            )
        {
            return None;
        }
        if slot
            && !has(
                q,
                &[
                    "推荐",
                    "对比",
                    "怎么选",
                    "哪件",
                    "哪些",
                    "哪个",
                    "哪种",
                    "更强",
                    "替代",
                    "换成",
                ],
            )
        {
            let raw = q.trim_end_matches(['?', '？', '。', '！', '!']);
            if array(&data.get("item-catalog-data")["items"])
                .iter()
                .any(|row| text(row, "name") == raw)
            {
                return None;
            }
        }

        let mut targets = goals(q);
        let elem = element(q);
        if targets.is_empty() && (affirmed(q, &["伤害"]) || elem.is_some()) {
            targets.push(Goal::Attack);
        }
        let beneficiary = recipient(q);
        let all_sources = has(
            q,
            &[
                "不限制作",
                "不限来源",
                "不限制来源",
                "所有来源",
                "掉落也行",
                "不用能制作",
                "不要求能做",
                "不用能做",
                "取消制作",
            ],
        );
        let crafting = has(
            q,
            &[
                "能制作",
                "能做",
                "能合成",
                "能造",
                "可制作",
                "可做的",
                "自己做",
                "只看制作",
                "有配方",
                "自制",
                "做得出来",
                "做得了",
            ],
        );
        let all_elements = has(
            q,
            &[
                "不限属性",
                "不分属性",
                "取消属性",
                "全属性",
                "通用",
                "不限系",
            ],
        );
        let all_levels = has(q, &["不限等级", "忽略等级", "取消等级", "不管等级"]);
        let level = player_level(q);
        let change = has(
            q,
            &[
                "只看",
                "只要",
                "换成",
                "改成",
                "换个",
                "那如果",
                "我想要",
                "想要",
                "有没有",
                "呢",
                "只用",
                "主要",
                "主打",
                "不要",
                "还想",
                "不限",
            ],
        );
        let short = q.chars().count() <= 18;
        let relative = self.query.is_some()
            && (change || short)
            && (beneficiary.is_some()
                || !targets.is_empty()
                || crafting
                || all_sources
                || all_elements
                || all_levels
                || level.is_some()
                || has(q, &["暴击", "耐力", "移速", "便宜", "预算", "好拿", "爆伤"]));
        let explicit = slot || worn && !targets.is_empty();
        if !explicit && !relative {
            return None;
        }
        if !slot
            && has(
                q,
                &[
                    "武器",
                    "衣服",
                    "头盔",
                    "护甲",
                    "技能怎么",
                    "怎么抓",
                    "最强帕鲁",
                    "哪只帕鲁",
                    "怎么回血",
                    "怎么恢复",
                ],
            )
        {
            return None;
        }

        let mut query = self.query.clone().unwrap_or_default();
        let prior = self.query.is_some();
        if let Some(selected) = beneficiary {
            query.recipient = selected;
        } else if slot
            && data.species.iter().any(|p| {
                [text(p, "zh"), text(p, "en")]
                    .into_iter()
                    .chain(array(&p["aliases"]).iter().filter_map(|v| v.as_str()))
                    .any(|name| !name.is_empty() && question.contains(name))
            })
        {
            query.recipient = Recipient::Pal;
        }
        if all_elements {
            query.element = None
        } else if let Some(e) = elem {
            query.element = Some(e.to_owned())
        }
        if all_sources {
            query.craft_only = false
        } else if crafting {
            query.craft_only = true
        }
        if all_levels {
            query.level = None
        } else if let Some(n) = level {
            query.level = Some(n)
        }
        match targets.as_slice() {
            [goal] => {
                query.goal = *goal;
                self.needs_goal = false;
            }
            [] if prior => {}
            _ => self.needs_goal = true,
        }
        self.items.clear();
        self.query = Some(query.clone());
        if has(
            q,
            &[
                "不要制作",
                "只要掉落",
                "只看掉落",
                "不想制作",
                "只要商店",
                "只看商人",
                "最便宜",
                "预算",
                "最好拿",
                "好拿",
                "我材料够",
                "不想刷",
                "不要肝",
            ],
        ) {
            return Some(clarify(
                "图鉴有配方和制作设施的解锁等级。要按入手难度挑选的话，你现在多少级？也可以先看有配方的候选及所需材料。",
                &["只看能制作的", "不限来源"],
            ));
        }
        if has(
            q,
            &[
                "暴击", "暴傷", "暴伤", "爆伤", "耐力", "移速", "闪避", "吸血", "冷却",
            ],
        ) {
            self.needs_goal = true;
            return Some(clarify(
                "这项效果目前还没有可核对的饰品排序。你想优先比较攻击、防御、生命、负重还是工作速度？",
                &["想提高攻击", "想提高防御", "想提高负重"],
            ));
        }
        if targets.len() > 1 {
            let choices = targets
                .iter()
                .map(|goal| match goal {
                    Goal::Attack => "优先攻击",
                    Goal::Defense => "优先防御",
                    Goal::Health => "优先生命",
                    Goal::WorkSpeed => "优先工作速度",
                    Goal::Carry => "优先负重",
                })
                .collect::<Vec<_>>();
            return Some(clarify(
                "你提到了多种效果。最想优先提升哪一项？我会按这一项排序，再列出每件饰品的加成。",
                &choices,
            ));
        }
        if self.needs_goal {
            return Some(clarify(
                "想用饰品提升哪项能力？可以选玩家或帕鲁的攻击、防御，也可以查生命、负重或工作速度。",
                &["给玩家提高输出", "给帕鲁提高输出", "想提高防御"],
            ));
        }
        let answer = recommend(data, &query);
        self.items = answer
            .links
            .iter()
            .filter_map(|(_, route)| route.strip_prefix("#items/").map(str::to_owned))
            .collect();
        self.query = Some(query);
        Some(answer)
    }
}
