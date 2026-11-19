use crate::assistant_engine::field_text;

// A mention of these operations in an unrelated guide is not a reason to
// consume a Pal, remove a structure or replace a player's data.
const OPERATIONS: &[&[&str]] = &[
    &[
        "浓缩",
        "凝结度",
        "凝结等级",
        "升星",
        "凝缩",
        "合并帕鲁",
        "融合帕鲁",
        "condens",
        "sacrifice",
        "献祭",
    ],
    &[
        "拆解",
        "分解",
        "拆除",
        "拆掉",
        "拆毁",
        "解体",
        "disassembl",
        "dismantl",
        "demolish",
        "butcher",
        "肢解",
        "屠宰",
    ],
    &[
        "丢弃",
        "丢掉",
        "扔掉",
        "抛弃",
        "放生",
        "discard",
        "throw away",
        "release your pal",
    ],
    &[
        "删档",
        "删除",
        "清空",
        "覆盖存档",
        "覆盖原档",
        "覆盖原始",
        "覆盖文件",
        "覆盖你的",
        "覆盖游戏",
        "替换存档",
        "擦除",
        "overwrit",
        "delet",
        "wipe the",
        "wipe your",
    ],
];

fn compact(s: &str) -> String {
    field_text(s)
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '+' | '%' | '≤' | '≥'))
        .collect()
}

fn is_caution(prefix: &str) -> bool {
    let prefix = prefix
        .rsplit([',', '，', ';', '；', '但'])
        .next()
        .unwrap_or(prefix);
    let prefix = prefix.split("instead").last().unwrap_or(prefix);
    let prefix = prefix.split(" but ").last().unwrap_or(prefix);
    !["不要忘", "别忘", "不能不", "do not forget", "don't forget"]
        .iter()
        .any(|s| prefix.contains(s))
        && [
            "不要",
            "请勿",
            "切勿",
            "不得",
            "禁止",
            "不可",
            "不应",
            "别去",
            "避免",
            "不必",
            "无需",
            "不用",
            "不想",
            "不能靠",
            "不建议",
            "不推荐",
            "无法确认",
            "不能确认",
            "do not",
            "don't",
            "avoid ",
            "never ",
            "no need",
            "cannot confirm",
            "cannot recommend",
            "not necessary",
            "without ",
            "must not",
            "should not",
            "shouldn't",
            "not to ",
        ]
        .iter()
        .any(|s| prefix.contains(s))
}

fn mentions_question_subject(question: &str, record: &str) -> bool {
    // A full concrete noun from the player's question, or a Chinese phrase
    // of at least three characters. Generic "work/production/Pal" overlaps
    // do not establish that two devices, loot sources or saves are the same.
    let record = field_text(record);
    let words: Vec<_> = record.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    for token in question.split(|c: char| !c.is_alphanumeric()) {
        if token.is_ascii() {
            if token.len() >= 3
                && !matches!(
                    token,
                    "the"
                        | "this"
                        | "that"
                        | "are"
                        | "can"
                        | "what"
                        | "why"
                        | "does"
                        | "how"
                        | "which"
                        | "you"
                        | "your"
                        | "my"
                        | "have"
                        | "any"
                        | "could"
                        | "would"
                        | "need"
                        | "should"
                        | "from"
                        | "with"
                        | "for"
                        | "and"
                        | "pal"
                        | "pals"
                        | "palworld"
                        | "game"
                        | "work"
                        | "working"
                        | "world"
                        | "item"
                        | "items"
                        | "get"
                        | "speed"
                        | "increase"
                        | "more"
                        | "make"
                        | "production"
                        | "output"
                        | "settings"
                        | "setting"
                        | "help"
                        | "best"
                        | "base"
                        | "oil"
                )
                && words.contains(&token)
            {
                return true;
            }
        } else {
            let characters = token.chars().collect::<Vec<_>>();
            if characters.windows(3).any(|w| {
                let chunk: String = w.iter().collect();
                !w.iter()
                    .any(|c| c.is_ascii() || "的我你要想了这那个吗有没让在".contains(*c))
                    && ![
                        "提高产",
                        "升产量",
                        "帕鲁的",
                        "增加产",
                        "工作速",
                        "工作效",
                        "生产效",
                    ]
                    .contains(&chunk.as_str())
                    && record.contains(&chunk)
            }) {
                return true;
            }
        }
    }
    false
}

pub(super) fn supported(question: &str, answer: &str, records: &[&str]) -> bool {
    let question = field_text(question);
    let answer = field_text(answer);
    for sentence in answer.split(['。', '！', '？', '!', '?', '.', '\n', ';', '；']) {
        for group in OPERATIONS {
            let suggests_action = group.iter().any(|word| {
                sentence
                    .match_indices(word)
                    .any(|(i, _)| !is_caution(&sentence[..i]))
            });
            if !suggests_action {
                continue;
            }
            if group.iter().any(|word| {
                question
                    .match_indices(word)
                    .any(|(i, _)| is_caution(&question[..i]))
            }) {
                return false;
            }
            let claim = compact(sentence.trim().trim_start_matches(['-', '*', '•', ' ']));
            let scope_requested = group.iter().any(|word| question.contains(word));
            // Require the actual relationship in one cited record. An action
            // on one station plus a second record about a different station
            // cannot establish the proposed intervention.
            if !records.iter().any(|record| {
                let record = field_text(record);
                record
                    .split(['。', '！', '？', '!', '?', '.', '\n', ';', '；'])
                    .any(|line| {
                        let justified_scope =
                            scope_requested || mentions_question_subject(&question, line);
                        // Match the entire statement, including any backup or
                        // eligibility condition that appears before the operation.
                        justified_scope && compact(line) == claim
                    })
            }) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costly_actions_cannot_cross_object_or_invent_a_goal() {
        let ranch = "提高帕鲁的浓缩度可以增加牧场产物。";
        let pump = "The extractor requires power to produce crude oil.";
        assert!(!supported(
            "How can I increase my crude oil production?",
            ranch,
            &[ranch, pump]
        ));
        let ranch_oil = "Condense ranch Pals to obtain more high quality Pal Oil.";
        assert!(!supported(
            "How can I increase oil production?",
            ranch_oil,
            &[ranch_oil]
        ));
        assert!(!supported(
            "会不会提高原油产量？",
            "为了增加原油产量，建议浓缩帕鲁。",
            &[ranch, pump]
        ));
        assert!(!supported(
            "我的孵蛋器不工作了",
            "拆除孵蛋器就能找回里面的蛋。",
            &["拆除木箱会使其内容掉落。"]
        ));
        assert!(!supported(
            "How do I clear more inventory space?",
            "Discard your legendary pals.",
            &["Discard stones to clear inventory space."]
        ));
        assert!(!supported(
            "如何迁移世界？",
            "先不要删除备份，但覆盖原始存档。",
            &["新房主应复制存档备份。"]
        ));
        assert!(!supported(
            "如何覆盖存档？",
            "覆盖原档。",
            &["另外备份且验证能读之后，再覆盖原档。"]
        ));
        assert!(!supported(
            "Should I discard the blueprint?",
            "Discard the blueprint.",
            &["Never discard the blueprint."]
        ));
        assert!(supported("牧场里怎么浓缩？", ranch, &[ranch]));
        assert!(supported(
            "How do I unload the stone from my inventory?",
            "Discard stones to clear inventory space.",
            &["Discard stones to clear inventory space."]
        ));
        assert!(supported(
            "存档突然不见了",
            "不要覆盖原档。先检查备份。",
            &[]
        ));
    }
}
