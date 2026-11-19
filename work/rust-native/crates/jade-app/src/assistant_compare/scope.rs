use super::{AssistantReply, Candidate, Data, Goal, add_link, array, has, ranking, text, word};

#[derive(Clone, Debug, Default)]
pub(super) struct Scope {
    region: Option<String>,
    timing: Option<&'static str>,
    version: Option<String>,
    condition: Option<&'static str>,
}

impl Scope {
    pub(super) fn consume_named_exclusions(&mut self, q: &str) {
        if self.condition == Some("排除指定条件") && !has(q, &["raid", "legendary", "legend ", "传说", "神兽", "头目", "团体战", "突袭"]) {
            self.condition = None;
        }
    }

    pub(super) fn clear_region(&mut self) {
        self.region = None;
        self.timing = None;
        if self.condition == Some("岛屿范围") {
            self.condition = None;
        }
    }

    pub(super) fn refine(&mut self, new: Self) {
        if new.region.is_some() {
            self.region = new.region;
            self.timing = new.timing;
        }
        if new.version.is_some() {
            self.version = new.version;
        }
        if new.condition.is_some() {
            self.condition = new.condition;
        }
    }

    pub(super) fn parse(d: &Data, q: &str) -> Option<Self> {
        let region = [
            ("天坠之地", &["天坠", "feybreak"][..]),
            ("樱岛", &["樱岛", "sakurajima"][..]),
            ("世界树", &["世界树", "world tree"][..]),
        ]
        .iter()
        .find(|(_, names)| has(q, names))
        .map(|(region, _)| *region)
        .or_else(|| {
            array(&d.get("expedition-data")["stages"])
                .iter()
                .map(|s| text(s, "region"))
                .find(|region| !region.is_empty() && q.contains(region))
        })
        .map(str::to_owned);
        let version = q
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .map(|s| s.trim_matches('.'))
            .find(|s| s.contains('.') && s.split('.').all(|n| !n.is_empty() && n.len() < 4))
            .map(str::to_owned);
        let timing = if region.is_some() {
            if has(
                q,
                &[
                    "通关后",
                    "打完",
                    "通关了",
                    "after clearing",
                    "after beating",
                    "after defeating",
                ],
            ) {
                Some("通关")
            } else if word(q, &["after", "since", "post"])
                || has(
                    q,
                    &[
                        "之后",
                        "以后",
                        "通关后",
                        "更新后",
                        "打完",
                        "通关了",
                        "天坠后",
                    ],
                )
                || region
                    .as_deref()
                    .is_some_and(|r| q.contains(&format!("{r}后")))
            {
                Some("之后")
            } else if word(q, &["before", "pre"])
                || has(q, &["之前", "以前", "更新前", "天坠前"])
                || region
                    .as_deref()
                    .is_some_and(|r| q.contains(&format!("{r}前")))
            {
                Some("之前")
            } else if word(q, &["introduced", "added"]) || has(q, &["首发", "新增"]) {
                Some("新增")
            } else if word(q, &["update", "patch", "release"]) || has(q, &["版本", "更新"]) {
                Some("版本")
            } else {
                None
            }
        } else {
            None
        };
        let condition = [
            (
                "前期进度",
                &[
                    "early game",
                    "early-game",
                    "starter",
                    "beginner",
                    "新手",
                    "前期",
                    "开荒",
                ][..],
            ),
            (
                "排除指定条件",
                &[
                    "excluding ",
                    "except ",
                    "without ",
                    "no raid",
                    "no legendar",
                    "不含",
                    "排除",
                    "禁用",
                    "不考虑",
                    "不用",
                    "不要",
                ][..],
            ),
            (
                "等级或解锁限制",
                &[
                    "below level",
                    "under level",
                    "up to level",
                    "级以下",
                    "级之前",
                    "级以内",
                ][..],
            ),
            ("岛屿范围", &[" island", "岛上", "这座岛", "这个岛"][..]),
            ("进度范围", &["after ", "before ", "通关后", "击败后"][..]),
        ]
        .into_iter()
        .find(|(label, terms)| {
            has(q, terms) && (!matches!(*label, "进度范围" | "岛屿范围") || region.is_none())
        })
        .map(|(label, _)| label);
        (region.is_some() || version.is_some() || condition.is_some()).then_some(Self {
            region,
            timing,
            version,
            condition,
        })
    }

    pub(super) fn answer(
        &self,
        d: &Data,
        rows: &[Candidate<'_>],
        goal: Goal,
        named: &[String],
        owned: bool,
        q: &str,
    ) -> Option<AssistantReply> {
        let snapshot = d.get("spawn-data")["version"]
            .as_str()
            .or_else(|| d.get("expedition-data")["version"].as_str())
            .unwrap_or("未标版本");
        let historical = self.version.as_ref().filter(|v| *v != snapshot);
        if self.region.is_none() && historical.is_none() && self.condition.is_none() {
            return None;
        }
        let mut reply = AssistantReply {
            text: String::new(),
            links: Vec::new(),
            suggestions: Vec::new(),
        };
        let mut lines = Vec::new();
        if let Some(version) = historical {
            lines.push(format!("你指定的是 {version} 版。工具箱这组数据来自 {snapshot}，没有该旧版的完整物种与属性快照，无法验证当时的第一名。"));
        }
        if let Some(condition) = self.condition {
            if (!named.is_empty() || owned) && !rows.is_empty() && historical.is_none() {
                lines.push(format!("按你给出的候选比较 {snapshot} 图鉴数值。资料无法独立核实候选是否都满足「{condition}」。"));
                lines.extend(current_ranks(
                    d,
                    rows,
                    goal,
                    q,
                    !named.is_empty() || owned && rows.len() <= 10,
                    &mut reply,
                ));
                reply.text = lines.join("\n");
                return Some(reply);
            }
            let clarification = match condition {
                "前期进度" | "等级或解锁限制" | "进度范围" => {
                    "你目前解锁到哪座塔、有哪些可用的帕鲁？给出候选后可以按基础攻击、工作适性或骑乘能力比较。"
                }
                "岛屿范围" => {
                    "你指哪座岛？也可以给出岛上已经确认的候选，我可以比较它们的图鉴数值。"
                }
                _ => {
                    "请列出符合该限制的候选，或在已登记的仓库帕鲁里比较。当前物种表无法完整核对这个排除范围。"
                }
            };
            let region = self.region.as_deref().unwrap_or("");
            lines.push(format!("按{region}{condition}筛选时，{clarification}"));
            reply.text = lines.join("\n");
            return Some(reply);
        }
        let Some(region) = self.region.as_deref() else {
            lines.push(
                "如果按当前版本比较，请说明要比基础攻击、防御、哪项据点工作，还是骑乘。".into(),
            );
            reply.text = lines.join("\n");
            return Some(reply);
        };
        match self.timing {
            Some(timing @ ("之前" | "之后")) => lines.push(format!("如果问的是{region}更新{timing}的强度，工具箱保存的是 {snapshot} 版物种数据，无法复原旧补丁的排名。")),
            Some("通关") => lines.push(format!("按通关{region}后的进度来选，要看你已经能培养哪些帕鲁。当前图鉴版本是 {snapshot}。")),
            Some("新增") => lines.push(format!("若限定{region}版本新增的帕鲁，资料没有逐版本的完整收录名单。下文数值来自 {snapshot}。")),
            Some(_) => lines.push(format!("要复盘{region}更新时的排名，需要同版全部可用物种与属性。下文数值来自 {snapshot}。")),
            None => lines.push(format!("范围限定在{region}。{snapshot} 资料能核实高塔首领所属物种，缺少按这一区域完整筛选可捕获物种的名单，无法排出整区第一。")),
        }
        // An expedition's region and tower point are explicit. Match the encounter's
        // exact point ID and species key; a tower encounter alone does not prove a wild spawn.
        let stage = array(&d.get("expedition-data")["stages"])
            .iter()
            .find(|s| text(s, "region") == region && s["hard"] == false);
        let regional = stage
            .filter(|_| self.timing.is_none() || (named.is_empty() && !owned))
            .and_then(|s| {
                let point = s["point"].as_str()?;
                let enemy = array(&d.get("enemy-data")["enemies"]).iter().find(|e| {
                    e["variant"] == "普通"
                        && e["palKey"].is_string()
                        && array(&e["points"]).iter().any(|p| p["id"] == point)
                })?;
                let pal = rows.iter().find(|p| p.key() == text(enemy, "palKey"))?;
                Some((s, enemy, *pal))
            });
        if let Some((stage, enemy, pal)) = regional {
            lines.push(format!(
                "当地可核实的例子是{}（{}）：{}的首领记录为「{}」。以下是该物种的普通图鉴数据。",
                pal.name(),
                text(pal.species, "en"),
                text(stage, "tower"),
                text(enemy, "name")
            ));
            reply.links.push((
                text(stage, "tower").into(),
                format!("#enemies/{}", text(enemy, "id")),
            ));
            add_link(&mut reply, pal);
            match goal {
                Goal::Job(job) => {
                    if let Some(level) = pal.value(goal) {
                        lines.push(format!("{}的{job}适性是 Lv.{level}。", pal.name()));
                    } else {
                        lines.push(format!("它没有已登记的{job}适性。"));
                    }
                }
                Goal::Work => {
                    let jobs: Vec<_> = array(&pal.profile["work"])
                        .iter()
                        .filter_map(|j| Some(format!("{} Lv.{}", j[0].as_str()?, j[1].as_u64()?)))
                        .collect();
                    if !jobs.is_empty() {
                        lines.push(format!("据点工作可按工种选：{}。", jobs.join("、")));
                    }
                }
                Goal::MountSpeed => {
                    if let Some(effect) = d.get("partner-skills-data")[pal.key()]["effect"].as_str()
                    {
                        lines.push(format!("{}的伙伴技能写明：{effect}", pal.name()));
                    }
                    lines.push("当前没有可横向比较的坐骑速度表，无法给出速度冠军。".into());
                }
                Goal::Stats(stats) => {
                    let fields: Vec<_> = stats
                        .fields()
                        .map(|(goal, label)| {
                            pal.value(goal).map_or_else(
                                || format!("{label}未收录"),
                                |v| format!("{label} {v}"),
                            )
                        })
                        .collect();
                    lines.push(fields.join("、"));
                }
                _ => {
                    if let (Some(a), Some(h), Some(v)) = (
                        pal.value(Goal::Attack),
                        pal.value(Goal::Health),
                        pal.value(Goal::Defense),
                    ) {
                        lines.push(format!("按战斗基础面板，攻击 {a}、生命 {h}、防御 {v}。打首领还需看对方属性、技能和个体培养。"));
                    }
                }
            }
        } else if self.timing.is_none() && (!named.is_empty() || owned) {
            lines.push("目前的地区记录无法确认符合这些筛选条件的物种。".into());
        }
        if (matches!(self.timing, Some("之后" | "通关")) || named.len() >= 2)
            && historical.is_none()
            && !rows.is_empty()
        {
            let label = if !named.is_empty() {
                "你点名的候选"
            } else if owned {
                "你的仓库"
            } else {
                "当前图鉴中符合条件的物种"
            };
            let ranks = current_ranks(
                d,
                rows,
                goal,
                q,
                !named.is_empty() || owned && rows.len() <= 10,
                &mut reply,
            );
            if !ranks.is_empty() {
                lines.push(format!(
                    "另按 {snapshot} 版{label}比较（未按岛内出现或当时解锁进度筛选）：{}",
                    ranks.join(" ")
                ));
            }
        }
        if matches!(self.timing, Some("之前" | "之后")) && historical.is_none() {
            lines.push("你指特定补丁的全图鉴，还是通关后的当前存档？若是后者，可以按打哪个首领、做哪项工作或骑乘方式来选。".into());
        } else if self.timing.is_none()
            && !matches!(goal, Goal::MountSpeed)
            && !has(q, &["哪两只", "which two"])
        {
            lines.push("如果给出你在当地找到的候选，我可以逐只比较。".into());
        }
        reply.text = lines.join("\n");
        Some(reply)
    }
}

fn current_ranks(
    d: &Data,
    rows: &[Candidate<'_>],
    goal: Goal,
    q: &str,
    show_values: bool,
    reply: &mut AssistantReply,
) -> Vec<String> {
    let metrics = match goal {
        Goal::Stats(stats) => return stats.compare(rows, show_values, reply),
        Goal::Combat | Goal::Attack => vec![(Goal::Attack, "基础攻击")],
        Goal::Defense => vec![(goal, "基础防御")],
        Goal::Health => vec![(goal, "基础生命")],
        Goal::Job(job) => vec![(goal, job)],
        Goal::Work => vec![
            (Goal::Job("生火"), "冶炼"),
            (Goal::Job("采矿"), "采矿"),
            (Goal::Job("搬运"), "搬运"),
        ],
        Goal::MountSpeed => {
            let mut examples = vec!["资料尚无统一移动速度可排序。".into()];
            mount_examples(d, rows, q, reply, &mut examples);
            return examples;
        }
    };
    metrics
        .into_iter()
        .map(|(metric, label)| {
            ranking(rows, metric, label, reply)
                .unwrap_or_else(|| format!("暂无候选的{label}数据。"))
        })
        .collect()
}

pub(super) fn mount_examples(
    d: &Data,
    rows: &[Candidate<'_>],
    q: &str,
    reply: &mut AssistantReply,
    lines: &mut Vec<String>,
) {
    let groups = [
        ("飞行", "在空中飞行"),
        ("水上", "在水上移动"),
        ("陆地", "在它的背上移动"),
    ];
    let requested = if has(q, &["飞行", "飞得", "flying", "flyer"]) {
        Some("飞行")
    } else if has(q, &["水上", "游泳", "swimming", "water mount"]) {
        Some("水上")
    } else if has(q, &["地面", "陆地", "ground mount", "land mount"]) {
        Some("陆地")
    } else {
        None
    };
    for (mode, evidence) in groups {
        if requested.is_some_and(|want| want != mode) {
            continue;
        }
        if let Some(pal) = rows.iter().find(|p| {
            d.get("partner-skills-data")[p.key()]["effect"]
                .as_str()
                .is_some_and(|e| e.contains(evidence))
        }) {
            lines.push(format!("{mode}骑乘例子：{}。", pal.name()));
            add_link(reply, *pal);
        }
    }
}
