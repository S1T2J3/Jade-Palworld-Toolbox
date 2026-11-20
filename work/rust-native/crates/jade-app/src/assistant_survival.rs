use crate::assistant_engine::AssistantReply;
use crate::data::{Data, array, text};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Climate {
    Cold,
    Hot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Subject {
    #[default]
    Player,
    Base,
    Egg,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SurvivalSession {
    climate: Option<Climate>,
    subject: Subject,
    level: Option<u32>,
}

fn mentions(q: &str, words: &[&str]) -> bool {
    words.iter().any(|word| q.contains(word))
}

fn stated_level(q: &str) -> Option<u32> {
    let (before, _) = q.split_once('级')?;
    let start = before
        .trim_end()
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .len();
    before.trim_end()[start..].parse().ok()
}

fn resistance(effects: &Value, tag: &str) -> Option<u32> {
    array(&effects["effects"])
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|line| {
            let (_, suffix) = line.split_once(tag)?;
            let digits: String = suffix
                .trim_start()
                .strip_prefix("Lv")?
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
        .max()
}

// Prefer the earliest unlocked recipe giving the requested protection level.
// Blueprint variants have no technology unlock and cannot be assumed available.
fn clothing<'a>(data: &'a Data, tag: &str, max_tech: Option<u32>) -> Option<(&'a Value, u32, u32)> {
    let effects = &data.get("item-effects-data")["items"];
    array(&data.get("item-catalog-data")["items"])
        .iter()
        .filter(|row| text(row, "category") == "防具" && row["other"] != true)
        .filter_map(|row| {
            let protection = resistance(&effects[text(row, "id")], tag)?;
            let tech = array(&row["recipes"])
                .iter()
                .filter_map(|r| r["tech"].as_u64())
                .min()? as u32;
            Some((row, protection, tech))
        })
        .filter(|(_, _, tech)| max_tech.is_none_or(|level| *tech <= level))
        .max_by_key(|(_, protection, tech)| (*protection, std::cmp::Reverse(*tech)))
}

impl SurvivalSession {
    pub(crate) fn has_context(&self) -> bool { self.climate.is_some() }

    pub(crate) fn can_show_alternative_clothing(&self) -> bool {
        self.subject == Subject::Player && self.level.is_none()
    }

    pub(crate) fn respond(&mut self, data: &Data, q: &str) -> Option<AssistantReply> {
        let climate = if mentions(
            q,
            &[
                "太冷",
                "好冷",
                "冷死",
                "寒冷",
                "冻死",
                "冻伤",
                "冻得",
                "冻到",
                "要冻",
                "温度太低",
                "很冷",
                "冷得",
                "还冷",
                "还是冷",
                "防寒怎么办",
                "怎么保暖",
                "怎么御寒",
            ],
        ) {
            Some(Climate::Cold)
        } else if mentions(
            q,
            &[
                "太热",
                "好热",
                "热死",
                "热得",
                "过热",
                "中暑",
                "温度太高",
                "暑热",
                "还是热",
                "怎么避暑",
            ],
        ) {
            Some(Climate::Hot)
        } else {
            None
        };
        let player_request = crate::assistant_engine::environmental_help::temperature(q);
        let climate = climate.or_else(|| player_request
            .map(|cold| if cold { Climate::Cold } else { Climate::Hot }));
        let subject = if mentions(q, &["孵化", "孵蛋", "帕鲁蛋", "蛋太", "蛋很", "是蛋"])
        {
            Some(Subject::Egg)
        } else if mentions(
            q,
            &["在据点", "在基地", "据点里", "基地里", "给据点", "给基地"],
        ) || player_request.is_some() && mentions(q, &["据点","基地"]) {
            Some(Subject::Base)
        } else if mentions(
            q,
            &[
                "在外面",
                "在野外",
                "我自己",
                "是我本人",
                "我身上",
                "人物",
                "角色",
                "雪山里",
                "在雪山",
                "在沙漠",
            ],
        ) {
            Some(Subject::Player)
        } else {
            None
        };
        if mentions(
            q,
            &[
                "电脑",
                "笔记本",
                "显卡",
                "CPU",
                "手机",
                "空调",
                "我家卧室",
                "现实里",
                "冰箱",
                "热度排行",
                "冷却时间",
            ],
        ) {
            *self = Self::default();
            return None;
        }
        let level = stated_level(q);
        let new_task = mentions(
            q,
            &[
                "配方",
                "怎么做",
                "怎么造",
                "怎么合成",
                "怎么获得",
                "哪里抓",
                "配种",
                "攻击力",
                "输出",
                "打谁",
            ],
        );
        let short_followup = self.climate.is_some()
            && !new_task
            && q.chars().count() <= 32
            && (subject.is_some()
                || level.is_some()
                || mentions(
                    q,
                    &[
                        "手上拿枪",
                        "不能举火把",
                        "已经拿着火把",
                        "拿着火把",
                        "还有别的办法",
                    ],
                ));
        if climate.is_none() && !short_followup {
            *self = Self::default();
            return None;
        }
        let climate = climate.or(self.climate)?;
        self.climate = Some(climate);
        if let Some(subject) = subject {
            self.subject = subject;
        }
        if let Some(level) = level {
            self.level = Some(level);
        }
        let cold = climate == Climate::Cold;
        let mut reply = AssistantReply {
            text: String::new(),
            links: Vec::new(),
            suggestions: Vec::new(),
        };
        if self.subject != Subject::Player {
            let key = if cold { "Heater" } else { "Cooler" };
            let facility = array(&data.get("facilities-data")["facilities"])
                .iter()
                .find(|f| text(f, "id") == key);
            if let Some(facility) = facility {
                let target = if self.subject == Subject::Egg {
                    "孵化器"
                } else {
                    "经常活动的位置"
                };
                let name = text(facility, "name");
                reply.text = format!("把{name}放在{target}附近。{}", text(facility, "purpose"));
                if let Some(job) = array(&facility["required"])
                    .first()
                    .and_then(|r| r[0].as_str())
                {
                    reply
                        .text
                        .push_str(&format!("安排有{job}适性的帕鲁到设施工作。"));
                }
                if self.subject == Subject::Egg {
                    reply
                        .text
                        .push_str("观察孵化器的温度提示是否改善，再决定是否要挪动位置。");
                } else {
                    reply
                        .text
                        .push_str("在设施附近查看温度，再走到活动区域边缘比较。");
                }
                reply
                    .links
                    .push((format!("查看{name}"), format!("#work/facilities/{key}")));
            } else {
                reply.text =
                    "先看当前温度提示。设施资料准备好后，可以核对适合这一温度的设备和分工。".into();
            }
            reply.suggestions.push("是我自己在野外".into());
            return Some(reply);
        }
        let tag = if cold { "耐寒" } else { "耐暑" };
        let has_torch = mentions(
            q,
            &[
                "已经拿着",
                "还冷",
                "还是冷",
                "手上拿枪",
                "不能举火把",
                "拿着火把",
            ],
        );
        if cold && !has_torch {
            reply.text =
                "先把携带火把拿在手上，看温度表是否恢复；还在掉血就先退回较暖的区域。".into();
            reply
                .links
                .push(("携带火把的做法".into(), "#items/Torch".into()));
        } else if cold {
            reply.text =
                "先退回较暖的区域，再换耐寒衣服。需要腾出手拿武器时，衣服和饰品更实用。".into();
        } else {
            reply.text =
                "先离开热源并退到温度较低的区域，收起手上的火把，观察温度和掉血状态。".into();
        }
        if let Some((row, protection, tech)) = clothing(data, tag, self.level.or(Some(9))) {
            let name = text(row, "name");
            reply.text.push_str(&format!(
                "\n出门前可以准备{name}（科技 {tech} 级，{tag} Lv.{protection}）。"
            ));
            reply.links.push((
                format!("{name}的配方"),
                format!("#items/{}", text(row, "id")),
            ));
        } else if cold && self.level.is_some() {
            reply
                .text
                .push_str("\n当前等级没有查到可解锁的耐寒衣服，优先用火把，等较暖的时候再出发。");
        }
        if self.level.is_none() || has_torch {
            let key = if cold {
                "Accessory_CoolResist_1"
            } else {
                "Accessory_HeatResist_1"
            };
            if let Some(protection) = resistance(&data.get("item-effects-data")["items"][key], tag)
            {
                let name = data.item_name(key);
                reply.text.push_str(&format!(
                    "已有{name}的话可以装进饰品栏，提供{tag} Lv.{protection}。"
                ));
                reply
                    .links
                    .push((format!("查看{name}"), format!("#items/{key}")));
            }
        }
        reply.suggestions = ["我只有 4 级", "是在据点里", "是孵化器的提示"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        Some(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use crate::store::Store;
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    #[test]
    fn four_player_questions_and_temperature_followups_use_packaged_data() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root, eframe::egui::Context::default()).unwrap();
        let start = Instant::now();
        while data.pending > 0 && start.elapsed() < Duration::from_secs(25) {
            std::thread::sleep(Duration::from_millis(10));
            data.poll();
        }
        assert!(data.errors.is_empty() && data.pending == 0);
        let temp = crate::store::tests::Temp::new();
        let store = Store::open(temp.0.clone());
        let mut convo = Conversation::default();
        let mut ask = |question| convo.ask(question, &data, &store);
        let equip = ask("输出饰品有什么推荐");
        assert!(equip.text.contains("汪宗师徽章"), "{}", equip.text);
        let rank = ask("最强帕鲁是谁");
        assert!(
            rank.text.contains("150") && rank.text.contains("贝菈"),
            "{}",
            rank.text
        );
        let recipe = ask("蛋糕怎么合成");
        assert!(
            recipe.text.contains("蜂蜜") && !recipe.text.contains("掉落"),
            "{}",
            recipe.text
        );
        let cold = ask("太冷了怎么办");
        assert!(
            cold.text.contains("携带火把") && cold.text.contains("寒冷地区民族服饰"),
            "{}",
            cold.text
        );
        let low = ask("我只有 4 级");
        assert!(
            low.text.contains("布衣服") && !low.text.contains("民族服饰"),
            "{}",
            low.text
        );
        let egg = ask("是孵化器的提示");
        assert!(
            egg.text.contains("孵化器附近")
                && egg.text.contains("取暖器")
                && egg.text.contains("生火"),
            "{}",
            egg.text
        );
        let hot = ask("这次是在野外，太热了怎么办");
        assert!(
            !hot.text.contains("孵化器") && hot.text.contains("收起手上的火把"),
            "{}",
            hot.text
        );
        let other = ask("蛋糕怎么合成");
        assert!(other.text.contains("蜂蜜") && !other.text.contains("火把"));
        let carrying = ask("探矿时总是得跑回家卸货，有什么能让我一趟多背点？我愿意换身上的装备");
        assert!(
            carrying.text.contains("负重")
                && carrying
                    .links
                    .iter()
                    .any(|(_, path)| path.starts_with("#items/")),
            "{}",
            carrying.text
        );
    }
}
