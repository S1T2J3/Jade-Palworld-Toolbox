use crate::assistant_data::ReferenceAnswer;
use crate::data::{Data, array, text};
use serde_json::{Value, json};
use std::collections::HashMap;
mod questions;
mod trait_transfer;

#[derive(Debug, Clone, Default)]
pub(crate) struct BreedingSession {
    target: Option<usize>,
    waiting_for_target: bool,
    parents: Option<[(usize, Option<char>); 2]>,
    confirmed: bool,
    english: bool,
}

impl BreedingSession {
    pub(crate) fn direct_result(&self) -> bool {
        self.confirmed
    }

    pub(crate) fn respond(
        &mut self,
        data: &Data,
        world: &Value,
        question: &str,
    ) -> Option<ReferenceAnswer> {
        let query = crate::assistant_engine::field_text(question.trim());
        let mentions = |terms: &[&str]| terms.iter().any(|term| query.contains(term));
        self.confirmed = false;
        // A player's build question may describe breeding attempts in the body.
        // Keep that background from turning a trait recommendation into a pair lookup.
        let first_question = query.split(['?', '？', '\n']).next().unwrap_or(&query);
        let asks_traits = ["passive", "trait", "词条", "詞條", "被动", "被動"]
            .iter().any(|word| first_question.contains(word));
        let asks_build = ["best", "recommend", "which", "should", "bring", "最", "推荐", "推薦",
            "带什么", "帶什麼", "怎么选", "怎麼選", "配什么", "配什麼"]
            .iter().any(|word| first_question.contains(word));
        let asks_inheritance = ["breed", "inherit", "pass down", "遗传", "遺傳", "配种", "配種"]
            .iter().any(|word| first_question.contains(word));
        if asks_traits && asks_build && !asks_inheritance {
            *self = Self::default();
            return None;
        }
        if let Some(answer) = self.known_pair(data, &query) {
            return Some(answer);
        }
        if mentions(&["牧场", "蛋糕", "孵化器"])
            && mentions(&["解锁", "材料", "建造", "制作", "怎么做", "在哪"])
        {
            *self = Self::default();
            return None;
        }
        let explicit_breeding = mentions(&[
            "配种",
            "配出",
            "配一只",
            "配个",
            "能配",
            "想配",
            "我要配",
            "孵出",
            "亲本",
            "双亲",
            "育种",
            "谁和谁配",
            "配出",
            "能配",
            "孵出",
            "生出",
            "后代",
            "子代",
            "孩子",
            "配對",
            "配種",
            "breed",
            "offspring",
        ]);
        let generic_pairing = mentions(&["怎么配", "如何配"])
            && !mentions(&[
                "据点", "阵容", "装备", "配队", "搭配", "技能", "饰品", "词条",
            ]);
        let breeding = explicit_breeding || generic_pairing;
        let planning = mentions(&[
            "词条", "被动", "几代", "多代", "少配", "最短", "路线", "遗传",
            "詞條", "被動", "遺傳", "passive", "trait", "inherit", "pass down",
        ]);
        let follow_up = self.target.is_some()
            && (planning
                || mentions(&[
                    "只用我",
                    "只用仓库",
                    "换成",
                    "换个目标",
                    "我的仓库呢",
                    "我仓库呢",
                    "哪两只",
                    "它怎么配",
                    "它如何配",
                ]));
        if !(breeding || follow_up || self.waiting_for_target) {
            *self = Self::default();
            return None;
        }
        let (spans, exact) = species_mentions(&data.species, &query);
        self.parents = None;
        let mut targets: Vec<_> = spans.into_iter().map(|(species, _, _)| species).collect();
        targets.dedup();
        if (self.waiting_for_target && !(breeding || follow_up || exact))
            || (generic_pairing && !explicit_breeding && !follow_up && targets.is_empty())
        {
            *self = Self::default();
            return None;
        }
        if targets.len() > 1 {
            *self = Self {
                target: None,
                waiting_for_target: true,
                ..Self::default()
            };
            return Some(ReferenceAnswer {
                text: "你提到了几种帕鲁。请先发想孵出的那一种，我会从当前仓库找亲本。".into(),
                links: vec![],
            });
        }
        let target = targets
            .first()
            .copied()
            .or_else(|| follow_up.then_some(self.target).flatten());
        let Some(target) = target else {
            *self = Self {
                target: None,
                waiting_for_target: true,
                ..Self::default()
            };
            return Some(ReferenceAnswer {
                text: "你想配出哪种帕鲁？发它的图鉴名称，我帮你找当前仓库里能直接开配的一对。"
                    .into(),
                links: vec![],
            });
        };
        self.target = Some(target);
        self.waiting_for_target = false;
        let species = &data.species[target];
        let name = text(species, "zh");
        let key = text(species, "key");
        let mut links = vec![(format!("规划{name}的配种路线"), format!("#planner/{key}"))];
        if planning {
            return Some(ReferenceAnswer {
                text: format!(
                    "目标是{name}。打开下方规划器，确认需要保留的词条后开始计算，即可按当前仓库比较代数并查看每一步的亲本。"
                ),
                links,
            });
        }
        let records = array(&world["inventory"]["records"]);
        let answer = if records.is_empty() {
            format!(
                "当前世界的仓库还是空的。想用现有帕鲁配出{name}，请先在配种仓库登记或导入帕鲁。也可以先查看图鉴列出的双亲配方。"
            )
        } else if let Some([a, b]) = direct_parents(&data.species, &data.pairs, records, target) {
            self.confirmed = true;
            format!(
                "把{}和{}放进配种牧场，这对亲本可以直接配出{name}。本次按子代种类和亲本性别筛选。",
                parent_label(data, &records[a]),
                parent_label(data, &records[b])
            )
        } else {
            let mut result = format!(
                "在当前世界已登记性别的帕鲁中，没有找到能一代配出{name}的组合。可以在规划器里继续查多代路线。"
            );
            if records.iter().any(|r| !matches!(text(r, "sex"), "M" | "F")) {
                result.push_str("仓库里还有性别未确认的帕鲁，补全性别后可重新查询。");
            }
            result
        };
        links.push(("查看双亲配方".into(), format!("#lookup/parents/{key}")));
        links.push(("打开配种仓库".into(), "#warehouse".into()));
        Some(ReferenceAnswer {
            text: answer,
            links,
        })
    }

    /// Forward species lookup reads the same rows (including their optional sex
    /// constraints) as the reverse warehouse planner. It never predicts inherited traits.
    pub(crate) fn known_pair(&mut self, data: &Data, q: &str) -> Option<ReferenceAnswer> {
        let has = |terms: &[&str]| terms.iter().any(|t| q.contains(t));
        if let Some(answer)=trait_transfer::answer(data,q).or_else(|| parents_for_target(data,q)){
            *self=Self {confirmed:true,..Self::default()};
            return Some(answer);
        }
        if let Some(answer) = questions::stage(data, q).or_else(|| questions::mate_choices(data, q)) {
            *self = Self { confirmed: true, ..Self::default() };
            return Some(answer);
        }
        let swap = has(&[
            "公母对调",
            "公母调换",
            "性别互换",
            "对调",
            "互换",
            "交换性别",
            "换性别",
            "反过来",
            "反着配",
            "倒过来",
            "swap",
            "revers",
            "switch genders",
            "switch sexes",
            "other way round",
            "other way around",
            "which one is male",
            "which is male",
            "谁公谁母",
            "谁是公",
            "谁当公",
            "公母顺序",
            "公母换",
            "换公母",
            "换一下公母",
            "公母反",
            "颠倒",
            "switch the genders",
            "switch sexes",
            "switch the parents",
            "does gender matter",
            "does sex matter",
            "which parent is male",
            "公母有讲究",
            "雌雄",
            "性别有影响",
        ]);
        let pairing = has(&[
            "配种",
            "配对",
            "交配",
            "繁殖",
            "繁育",
            "会生什么",
            "能生什么",
            "什么蛋",
            "what egg",
            "egg from",
            "配出",
            "能配",
            "配一只",
            "配个",
            "杂交",
            "孵出",
            "生出",
            "后代",
            "子代",
            "孩子",
            "亲本",
            "公母",
            "母本",
            "父本",
            "孵一只",
            "孵个",
            "孵一个",
            "这一对会出",
            "这对会出",
            "孵什么",
            "breed",
            "offspring",
            "child",
            "hatch",
            "crossbreed",
            "cross a ",
            "pairing",
            "pair my ",
            "pair a ",
            "pair the ",
            "parents",
            "♀",
            "♂",
            "×",
            " + ",
        ]);
        if !(pairing || swap) {
            return None;
        }
        let inherits = has(&[
            "遗传",
            "继承",
            "词条",
            "被动",
            "个体值",
            "比较",
            "哪个强",
            "哪只强",
            "哪个更",
            "哪个好",
            "inherit",
            "pass down",
            "passed down",
            "pass on",
            "passives",
            "passive skill",
            "trait",
            "stronger",
            "better",
            "ivs",
        ]);
        let explicit_child = has(&[
            "配出",
            "生出",
            "孵出",
            "后代是",
            "子代是",
            "哪种后代",
            "什么后代",
            "what hatches",
            "what will hatch",
            "what do i get",
            "what pal",
            "which pal hatches",
            "what offspring",
            "what species",
            "produce",
            "which species",
            "孵一只",
            "孵个",
            "这对会出",
            "这一对会出",
            "这一对实际会出",
            "trying for",
        ]);
        if inherits && !explicit_child {
            // Inherited passives are drawn from both parents' passives; see
            // https://palworldbreeding.gg/breeding-calculator/passives (1.0).
            let passive = has(&["被动", "词条", "passive", "trait"]);
            let transfer = has(&[
                "继承",
                "遗传",
                "传给",
                "inherit",
                "pass down",
                "passing down",
                "passed down",
                "pass on",
            ]);
            let sex = has(&[
                "父方",
                "母方",
                "公母",
                "母亲",
                "父亲",
                "性别",
                "父本",
                "母本",
                "gender",
                "father",
                "mother",
                "female",
                "which parent",
            ]);
            if passive && transfer && sex {
                *self = Self {
                    confirmed: true,
                    ..Self::default()
                };
                let en = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
                return Some(ReferenceAnswer{
                    text: if en {"Both parents contribute their inheritable passives to the same pool. Putting a desired passive on the father or mother does not give that passive a sex-based advantage. To compare the chances for a particular build, list the passives on both parents and the desired result."}else{"遗传被动时，双亲能遗传的词条进入同一候选池。把相同的目标词条换到父本或母本，不会因性别而提高它的遗传概率。要比较两组的具体机会，需要每个亲本的词条和想让子代保留的组合。"}.into(),
                    links:vec![]
                });
            }
            return None;
        }
        let (spans, _) = species_mentions(&data.species, q);
        let previous = self.parents;
        let english = !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
        let invert = |parents: [(usize, Option<char>); 2]| {
            parents.map(|(idx, sex)| (idx, sex.map(|s| if s == 'M' { 'F' } else { 'M' })))
        };
        let mut two = spans.as_slice();
        let mut desired = None;
        let resolved = family_roles(q, &spans);
        if spans.len() == 4
            && spans[2..]
                .iter()
                .all(|x| spans[..2].iter().any(|y| y.0 == x.0))
            && swap
        {
            two = &spans[..2];
        }
        if let Some((parents, child)) = &resolved {
            two = parents;
            desired = Some(*child);
        } else if spans.len() == 3 && explicit_child {
            let tail = &q[spans[1].2..spans[2].1];
            let head = &q[spans[0].2..spans[1].1];
            if [
                "产出", "孵出", "生出", "配出", "后代", "produce", "hatch", "give",
            ]
            .iter()
            .any(|s| tail.contains(s))
            {
                two = &spans[..2];
                desired = Some(spans[2].0);
            } else if ["by breeding", "from breeding", "by crossing"]
                .iter()
                .any(|s| head.contains(s))
            {
                two = &spans[1..];
                desired = Some(spans[0].0);
            }
        }
        let en_gender = has(&["gender", "sex", "male", "female"]);
        if two.is_empty() && !swap {
            return None;
        }
        if two.is_empty()
            && previous.is_none()
            && swap
            && (pairing || en_gender || has(&["性别", "公母"]))
        {
            // Give a real exception without assuming which two species the player owns.
            let special = data
                .pairs
                .iter()
                .find(|p| p.get(3).and_then(Value::as_str).is_some())?;
            self.english = english;
            let mut result = self.offspring(
                data,
                [
                    (special[0].as_u64()? as usize, None),
                    (special[1].as_u64()? as usize, None),
                ],
            );
            result.text = format!(
                "{}\n{}",
                if english {
                    "Most breeding pairs give the same species with either sex assignment, provided one is male and one is female. These sex-specific rows are the exception:"
                } else {
                    "普通组合需要一公一母，互换性别通常仍得到同种子代。当前表中有按性别区分的组合："
                },
                result.text
            );
            self.parents = None;
            return Some(result);
        }
        if two.is_empty() {
            let parents = previous?;
            self.english = english;
            return Some(self.offspring(data, invert(parents)));
        }
        let [a, b] = two else {
            return None;
        };
        let middle = &q[a.2..b.1];
        if !swap
            && !explicit_child
            && ["using", "from", "要用", "需要", "用什么", "怎么获得"]
                .iter()
                .any(|w| middle.contains(w))
        {
            return None;
        }
        if !explicit_child
            && !swap
            && !has(&[
                "配种",
                "配对",
                "交配",
                "繁殖",
                "繁育",
                "会生什么",
                "什么蛋",
                "what egg",
                "配出",
                "杂交",
                "breed",
                "offspring",
                "后代",
                "子代",
                "孩子",
                "♀",
                "♂",
                "×",
                " + ",
                "hatch",
                "父母",
                "亲本",
            ])
        {
            return None;
        }
        let mut parents =
            [*a, *b].map(|(idx, start, end)| (idx, sex_by_name(&q[..start], &q[end..])));
        if has(&["both male", "都是公", "都是雄", "两只公的"]) {
            parents[0].1 = Some('M');
            parents[1].1 = Some('M');
        }
        if has(&["both female", "都是母", "都是雌", "两只母的"]) {
            parents[0].1 = Some('F');
            parents[1].1 = Some('F');
        }
        self.english = english;
        if swap
            && parents.iter().all(|(_, s)| s.is_none())
            && let Some(old) = previous
            && old
                .iter()
                .map(|p| p.0)
                .all(|id| parents.iter().any(|p| p.0 == id))
        {
            return Some(self.offspring(data, invert(old)));
        }
        let mut result = self.offspring(data, parents);
        if swap && parents.iter().any(|(_, s)| s.is_some()) {
            let next = self.offspring(data, invert(parents));
            result.text.push_str(if english {
                "\nWith the sexes swapped: "
            } else {
                "\n公母对调后："
            });
            result.text.push_str(&next.text);
            for link in next.links {
                if !result.links.contains(&link) {
                    result.links.push(link);
                }
            }
        }
        if let Some(want) = desired {
            let species = &data.species[want];
            let route = format!("#lookup/parents/{}", text(species, "key"));
            let matched = result.links.iter().any(|(_, r)| r == &route);
            let label = text(species, if english { "en" } else { "zh" });
            result.text.push('\n');
            result.text.push_str(&match (english, matched) {
                (true, true) => {
                    format!("{label} is a recorded outcome for the assignment(s) shown.")
                }
                (true, false) => format!("{label} is not an outcome for the assignment(s) shown."),
                (false, true) => format!("上列组合中有能配出{label}的性别配置。"),
                (false, false) => format!("上列亲本与性别配置配不出{label}。"),
            });
            if !matched {
                let unrestricted = parents.map(|(id, _)| (id, None));
                let alternative = self.clone().offspring_for(data, unrestricted, Some(want));
                if !alternative.links.is_empty() {
                    result.text.push_str(&if english {
                        format!("\nTo breed {label} with these parents:\n")
                    } else {
                        format!("\n要用这两种亲本孵出{label}，按下面的性别配对：\n")
                    });
                    result.text.push_str(&alternative.text);
                    result.links.extend(alternative.links);
                }
            }
        }
        if inherits {
            result.text.push_str(if english {"\nThe pair table predicts the offspring species. It does not determine this egg’s inherited passives or IVs."} else {"\n配种表确定子代物种；这个蛋实际继承的被动词条、个体值须另行判断。"});
        }
        Some(result)
    }

    fn offspring(&mut self, data: &Data, parents: [(usize, Option<char>); 2]) -> ReferenceAnswer {
        self.offspring_for(data, parents, None)
    }

    fn offspring_for(&mut self, data: &Data, parents: [(usize, Option<char>); 2], goal: Option<usize>) -> ReferenceAnswer {
        self.target = None;
        self.waiting_for_target = false;
        self.parents = Some(parents);
        self.confirmed = true;
        let [(a, sex_a), (b, sex_b)] = parents;
        let en = self.english;
        let name = |i: usize| text(&data.species[i], if en { "en" } else { "zh" });
        if sex_a.is_some() && sex_a == sex_b {
            return ReferenceAnswer {
                text: if en {
                    "These two are the same sex. Breeding requires one male and one female."
                } else {
                    "这两只的性别相同。配种需要一公一母。"
                }
                .into(),
                links: vec![],
            };
        }
        let gender = |s| match (s, en) {
            ('M', true) => "male",
            (_, true) => "female",
            ('M', false) => "公",
            _ => "母",
        };
        let mut lines = Vec::new();
        let mut links = Vec::new();
        let mut locked = false;
        for pair in &data.pairs {
            let first = pair[0].as_u64().unwrap() as usize;
            let second = pair[1].as_u64().unwrap() as usize;
            let reversed = if first == a && second == b {
                false
            } else if first == b && second == a {
                true
            } else {
                continue;
            };
            let sex = |slot| {
                pair.get(slot)
                    .and_then(Value::as_str)
                    .and_then(|v| v.chars().next())
            };
            let (req_a, req_b) = if reversed {
                (sex(4usize), sex(3usize))
            } else {
                (sex(3usize), sex(4usize))
            };
            locked |= req_a.is_some() || req_b.is_some();
            if sex_a.zip(req_a).is_some_and(|(x, y)| x != y)
                || sex_b.zip(req_b).is_some_and(|(x, y)| x != y)
            {
                continue;
            }
            let child = pair[2].as_u64().unwrap() as usize;
            if goal.is_some_and(|goal| child != goal) {
                continue;
            }
            let label = |species, sex: Option<char>| {
                format!(
                    "{}{}",
                    name(species),
                    sex.map(|s| format!("（{}）", gender(s)))
                        .unwrap_or_default()
                )
            };
            let detail = format!(
                "{} × {} → {}",
                label(a, req_a.or(sex_a)),
                label(b, req_b.or(sex_b)),
                name(child)
            );
            if !lines.contains(&detail) {
                lines.push(detail);
                links.push((
                    format!("查看{}", name(child)),
                    format!("#lookup/parents/{}", text(&data.species[child], "key")),
                ));
            }
        }
        let answer = if lines.is_empty() {
            if en {
                "No row in the current breeding table matches that pair and sex assignment."
            } else {
                "当前配种表没有匹配这对亲本和性别的记录。"
            }
            .into()
        } else {
            if !locked {
                lines.push(if en {"This pair requires one male and one female; swapping their sexes produces the same offspring species."}else{"这对需要一公一母，互换公母仍得到相同的子代物种。"}.into());
            }
            lines.join("\n")
        };
        ReferenceAnswer {
            text: answer,
            links,
        }
    }
}

/// Reverse the same game pair table as the warehouse solver for a public-dex request.
fn parents_for_target(d:&Data,q:&str)->Option<ReferenceAnswer>{
    let has=|words:&[&str]|words.iter().any(|w|q.contains(w));
    if !has(&["亲本","双亲","谁和谁","哪两个","哪两只","怎么配出","怎么配种","怎么配出来","breed to get","breed for","parents for","parents of","breed a ","breed an ","how to breed","how do i breed","what pals do i need to breed"]){return None;}
    if has(&["我的","仓库","现有","已拥有","with my","with what","inventory","i own","词条","被动","遗传","passive","inherit","trait","without","except","排除","不用","随机","random","mod","多代","最短","route","几代"]){return None;}
    let (names,_)=species_mentions(&d.species,q);
    let [(target,_,_)]=names.as_slice() else{return None;};
    let mut rows:Vec<_>=d.pairs.iter().filter(|r|r[2].as_u64()==Some(*target as u64)).collect();
    rows.sort_by_key(|r|{
        let a=r[0].as_u64().unwrap_or(u64::MAX);
        let b=r[1].as_u64().unwrap_or(u64::MAX);
        (a==*target as u64 || b==*target as u64, a.max(b), a.min(b))
    });
    let child=d.species.get(*target)?;
    let name=|p:&Value|format!("{} ({})",text(p,"zh"),text(p,"en"));
    let mut lines=Vec::new();
    for r in rows.iter().take(3){
        let parent=|i:usize|{
            let p=d.species.get(r[i].as_u64()? as usize)?;
            let sex=match r.get(i+3).and_then(Value::as_str) {Some("M")=>" ♂",Some("F")=>" ♀",_=>""};
            Some(format!("{}{sex}",name(p)))
        };
        lines.push(format!("{} × {} → {}",parent(0)?,parent(1)?,name(child)));
    }
    if lines.is_empty(){return None;}
    lines.push("每组取一公一母，放进配种牧场并放入蛋糕。有性别标记的特殊配方要按标记选亲本。".into());
    Some(ReferenceAnswer{text:lines.join("\n"),links:vec![(format!("{}的全部亲本",name(child)),format!("#lookup/parents/{}",text(child,"key")))]})
}

type SpeciesSpan = (usize, usize, usize);

// One species can be named more than once: first in the proposed pairing and
// again as a sexed individual in the player's box. Bind each role before deduplicating.
fn family_roles(q: &str, spans: &[SpeciesSpan]) -> Option<([SpeciesSpan; 2], usize)> {
    if spans.len() < 3 { return None; }
    let is_goal = |span: SpeciesSpan| questions::goal_at(&q[..span.1]);
    let mut goals = spans.iter().copied().filter(|s| is_goal(*s));
    let goal = goals.next()?.0;
    if goals.any(|s| s.0 != goal) { return None; }
    let candidates: Vec<_> = spans.iter().copied().filter(|s| !is_goal(*s)).collect();
    let sex = |s: SpeciesSpan| sex_by_name(&q[..s.1], &q[s.2..]);
    let sexed: Vec<_> = candidates.iter().copied().filter(|s| sex(*s).is_some()).collect();
    if let [first, second] = sexed.as_slice() {
        return Some(([*first, *second], goal));
    }
    if let [first, second] = candidates.as_slice() {
        return Some(([*first, *second], goal));
    }
    let mut parents: Vec<SpeciesSpan> = Vec::new();
    for candidate in candidates {
        if let Some(existing) = parents.iter_mut().find(|s| s.0 == candidate.0) {
            if sex(candidate).is_some() {
                if sex(*existing).is_some() && sex(*existing) != sex(candidate) { return None; }
                *existing = candidate;
            }
        } else {
            parents.push(candidate);
        }
    }
    let [first, second] = parents.as_slice() else { return None };
    if first.1 < second.1 { Some(([*first, *second], goal)) } else { Some(([*second, *first], goal)) }
}

fn sex_by_name(before: &str, after: &str) -> Option<char> {
    let before = before.trim_end_matches(|c: char| c.is_whitespace() || "的性)）】]".contains(c));
    let after = after.trim_start_matches(|c: char| c.is_whitespace() || "（(【[：:".contains(c));
    for (sex, terms) in [
        ('F', &["母", "雌", "♀", "female"][..]),
        ('M', &["公", "雄", "♂", "male"]),
    ] {
        if terms
            .iter()
            .any(|s| before.ends_with(s) || after.starts_with(s))
        {
            return Some(sex);
        }
    }
    None
}

// A form can contain the entire base species name (e.g. Frostallion Noct).
// Keep separate mentions, preferring the longest name at overlapping positions.
const FORM_NAMES: &[&str] = &["ignis", "noct", "lux", "cryst", "botan", "aqua", "terra", "primo"];

fn form_after(tail: &str) -> Option<(&str, usize)> {
    let word = tail.trim_start_matches(|c: char| c.is_whitespace() || "-_([{（【".contains(c));
    if word.len() == tail.len() { return None; }
    let n = word.bytes().take_while(u8::is_ascii_alphabetic).count();
    let name = word.get(..n)?;
    if !FORM_NAMES.contains(&name) || word[n..].starts_with(|c:char| c.is_ascii_alphanumeric()) { return None; }
    let rest = word[n..].trim_start_matches([')', ']', '}', '）', '】']);
    Some((name,tail.len()-rest.len()))
}

pub(crate) fn species_mentions(species: &[Value], question: &str) -> (Vec<(usize, usize, usize)>, bool) {
    let mut spans = Vec::new();
    let mut exact = false;
    for (index, entry) in species.iter().enumerate() {
        for name in [text(entry, "zh"), text(entry, "en"), text(entry, "key")]
            .into_iter()
            .chain(array(&entry["aliases"]).iter().filter_map(Value::as_str))
        {
            let name = crate::assistant_engine::field_text(name);
            if name.is_empty() {
                continue;
            }
            exact |= name == question.trim_matches(['？', '?', '。', ' ']);
            for (start, _) in question.match_indices(&name) {
                let end = start + name.len();
                if (name.as_bytes()[0].is_ascii_alphanumeric()
                    && question[..start].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric()))
                    || (name.as_bytes()[name.len() - 1].is_ascii_alphanumeric()
                        && question[end..].chars().next().is_some_and(|c| c.is_ascii_alphanumeric()))
                {
                    continue;
                }
                if let Some((form, bytes)) = form_after(&question[end..]) {
                    let base = text(entry, "en");
                    if let Some((variant, _)) = species.iter().enumerate().find(|(_, other)| {
                        text(other,"en").rsplit_once(' ').is_some_and(|(stem, suffix)|
                            stem.eq_ignore_ascii_case(base) && suffix.eq_ignore_ascii_case(form))
                    }) {
                        spans.push((variant,start,end+bytes));
                    }
                    continue;
                }
                spans.push((index, start, start + name.len()));
            }
        }
    }
    spans.sort_by_key(|(_, start, end)| (std::cmp::Reverse(end - start), *start));
    let mut accepted = Vec::new();
    for (index, start, end) in spans {
        if accepted.iter().any(|(_, a, b)| start < *b && *a < end) {
            continue;
        }
        accepted.push((index, start, end));
    }
    accepted.sort_by_key(|(_, start, _)| *start);
    (accepted, exact)
}

/// Keep exclusions for comparison handlers, but omit them when naming the
/// subject of a species record (e.g. “Fuack, not Fuack Ignis”).
pub(crate) fn excluded_record_subject(before: &str) -> bool {
    let prefix = before.trim_end_matches(|ch: char| ch.is_whitespace() || "{[（(「【\"".contains(ch));
    ["而不是", "不是", "不考虑", "不包括", "排除", "not", "rather than", "excluding", "not asking about"].iter().any(|word| {
        prefix.strip_suffix(word).is_some_and(|s| !word.is_ascii() || !s.ends_with(|ch:char| ch.is_ascii_alphabetic()))
    })
}

#[cfg(test)]
fn named_species(species: &[Value], question: &str) -> (Vec<usize>, bool) {
    let (spans, exact) = species_mentions(species, question);
    (spans.into_iter().map(|(idx, _, _)| idx).collect(), exact)
}

// Restrict the existing solver to the chosen offspring's recipes and confirmed
// founders. A witness with depth one is sufficient, including for sex-locked recipes.
fn direct_parents(
    species: &[Value],
    pairs: &[Value],
    records: &[Value],
    target: usize,
) -> Option<[usize; 2]> {
    let keys: HashMap<_, _> = species
        .iter()
        .enumerate()
        .map(|(i, pal)| (text(pal, "key"), i))
        .collect();
    let stock: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let sex = text(row, "sex");
            if !matches!(sex, "M" | "F") {
                return None;
            }
            let species_id = keys.get(text(row, "speciesKey"))?;
            Some(json!({"id": i.to_string(), "species": species_id, "sex": sex, "traits": []}))
        })
        .collect();
    if stock.len() < 2 {
        return None;
    }
    let recipes: Vec<_> = pairs
        .iter()
        .filter(|r| r[2].as_u64() == Some(target as u64))
        .collect();
    let result = jade_core::trait_solve(&json!({
        "speciesCount": species.len(), "target": target, "stock": stock, "traits": [],
        "requireHatch": true, "pairs": recipes,
    }));
    let founders = array(&result["founders"]);
    if result["generations"].as_u64() != Some(1) || founders.len() != 2 {
        return None;
    }
    Some([
        text(&founders[0], "stockId").parse().ok()?,
        text(&founders[1], "stockId").parse().ok()?,
    ])
}

fn parent_label(data: &Data, record: &Value) -> String {
    let name = data.pal_name(text(record, "speciesKey"));
    let sex = if text(record, "sex") == "M" {
        "雄"
    } else {
        "雌"
    };
    match text(record, "name") {
        "" => format!("{name}（{sex}）"),
        nickname if nickname == name => format!("{name}（{sex}）"),
        nickname => format!("{name}「{nickname}」（{sex}）"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_mates_and_completed_breeding_get_the_requested_next_step() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = Data::new(root, eframe::egui::Context::default()).unwrap();
        let ask = |q: &str| {
            let mut c = crate::assistant_engine::Conversation::default();
            let a=c.ask_world(q,&data,&json!({"id":"pair-options"}));
            assert!(c.direct_result(), "{q}: {}",a.text);
            a.text
        };
        let a = ask("我想给火绒狐在玉藻狐和壶小象里挑一个配偶，目标是普通电棘鼠。假定性别都能配，这两个选择分别会孵出谁？");
        assert!(a.contains("火绒狐 × 玉藻狐 → 电棘鼠") && a.contains("火绒狐 × 壶小象 → 电棘鼠") && a.contains("可选的配偶是玉藻狐、壶小象"),"{a}");
        let a = ask("I want to breed Foxparks with Vixy or Teafant. My goal is Jolthog. What hatches for each mate?");
        assert!(a.contains("Foxparks × Vixy → Jolthog") && a.contains("Foxparks × Teafant → Jolthog"),"{a}");
        let a = ask("How to effectively level up bred pals? I have used all my training manuals, and my newly hatched pals are for raids.");
        assert!(a.contains("party battle EXP") && a.contains("without training manuals") && !a.contains("规划器"),"{a}");
        let a = ask("How does breeding work? I've started the game.");
        assert!(a.contains("Cake") && a.contains("Incubator") && a.contains("male and female"),"{a}");
        let a = ask("刚孵出来的帕鲁怎么练级？经验书用完了。");
        assert!(a.contains("随身队伍") && a.contains("换下一批"),"{a}");
    }

    #[test]
    fn repeated_goal_and_owned_parents_bind_separately_in_desktop_conversation() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let d = Data::new(root, eframe::egui::Context::default()).unwrap();
        let ask = |q: &str| {
            let mut session = crate::assistant_engine::Conversation::default();
            let answer = session.ask_world(q, &d, &json!({"id":"pair-goal"}));
            assert!(session.direct_result(), "{}", answer.text);
            answer.text
        };
        let answer = ask("我想用焰巫狐和暗巫猫孵一只幽巫狐，手里是公焰巫狐和母暗巫猫。普通蛋、不考虑突变，这一对实际会出什么？如果要幽巫狐，该换成哪只当母本？");
        assert!(answer.contains("焰巫狐（公） × 暗巫猫（母） → 炽巫猫") && answer.contains("焰巫狐（母） × 暗巫猫（公） → 幽巫狐"), "{answer}");
        let english = ask("I want a Wixen Noct. If I breed my female Katress with my male Wixen, what will hatch? Which should be the mother to get Wixen Noct?");
        assert!(english.contains("Katress（female） × Wixen（male） → Katress Ignis") && english.contains("Katress（male） × Wixen（female） → Wixen Noct"), "{english}");
        let english = ask("If I breed my male Wixen and female Katress, can I get a Wixen Noct?");
        assert!(english.contains("Wixen（male） × Katress（female） → Katress Ignis") && english.contains("Wixen（female） × Katress（male） → Wixen Noct"), "{english}");
        let complete = ask("母焰巫狐和公暗巫猫配种能孵出幽巫狐吗？");
        assert!(complete.contains("→ 幽巫狐") && !complete.contains("→ 炽巫猫"), "{complete}");
    }

    #[test]
    fn named_pair_checks_both_directions_and_leaves_inheritance_questions() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = Data::new(root, eframe::egui::Context::default()).unwrap();
        let mut s = BreedingSession::default();
        let q = |t: &str| crate::assistant_engine::field_text(t);
        let result=s.known_pair(&data,&q("Male Wixen and female Katress: what offspring do I get when I breed them, and does swapping their genders change it?")).unwrap();
        assert!(
            result.text.contains("Katress Ignis") && result.text.contains("Wixen Noct"),
            "{}",
            result.text
        );
        let ordinary = s
            .known_pair(&data, &q("棉悠悠和捣蛋猫配对，公母有讲究吗？"))
            .unwrap();
        assert!(
            ordinary.text.contains("相同的子代物种"),
            "{}",
            ordinary.text
        );
        let diff = s
            .known_pair(&data, &q("Can Katress and Wixen breed if both male?"))
            .unwrap();
        assert!(diff.text.contains("same sex"));
        assert!(
            s.known_pair(
                &data,
                &q("Which is better for passing down passives when breeding Wixen and Katress?")
            )
            .is_none()
        );
        let generic = BreedingSession::default()
            .known_pair(&data, &q("Does it matter which one is male when breeding?"))
            .unwrap();
        assert!(generic.text.contains("Katress Ignis") && generic.text.contains("Wixen Noct"));
    }

    #[test]
    fn pairing_uses_forward_records_and_preserves_sexes_for_swap() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = Data::new(root, eframe::egui::Context::default()).unwrap();
        let mut session = BreedingSession::default();
        let world = json!({"id":"test"});
        for question in [
            "Which passives should Shaolong use? I already tried breeding with Yakumo.",
            "阿努比斯带什么被动最好？前面一直用八云犬配种。",
        ] {
            assert!(session.respond(&data, &world, question).is_none());
            assert!(!session.direct_result());
        }
        let all = session
            .respond(&data, &world, "焰巫狐和暗巫猫配种会出什么？")
            .unwrap();
        assert!(
            all.text.contains("幽巫狐") && all.text.contains("炽巫猫"),
            "{}",
            all.text
        );
        let first = session
            .respond(&data, &world, "公暗巫猫和母焰巫狐能配出什么？")
            .unwrap();
        assert!(
            first.text.contains("幽巫狐") && !first.text.contains("炽巫猫"),
            "{}",
            first.text
        );
        let second = session
            .respond(&data, &world, "如果把公母对调呢？")
            .unwrap();
        assert!(
            second.text.contains("炽巫猫") && !second.text.contains("幽巫狐"),
            "{}",
            second.text
        );
        assert!(session.direct_result());
    }

    #[test]
    fn warehouse_pair_and_follow_up_use_the_same_real_target() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = Data::new(root, eframe::egui::Context::default()).unwrap();
        let world = json!({"inventory": {"records": [
            {"speciesKey":"Kirin_Ice", "sex":"M", "name":"小白"},
            {"speciesKey":"KingBahamut", "sex":"F", "name":"小红"}
        ]}});
        let mut session = BreedingSession::default();
        let first = session
            .respond(&data, &world, "我仓库里哪两只现在能配出阿努比斯？")
            .unwrap();
        assert!(first.text.contains("小白") && first.text.contains("小红"));
        let follow_up = session
            .respond(&data, &world, "三个词条也行，少配几代")
            .unwrap();
        assert!(
            follow_up
                .links
                .iter()
                .any(|(_, href)| href == "#planner/Anubis")
        );
        assert!(session.respond(&data, &world, "挖矿用什么？").is_none());
        assert!(session.respond(&data, &world, "三个词条也行").is_none());
    }

    #[test]
    fn direct_pair_respects_reversed_special_recipe_and_ignores_unknown_sex() {
        let species = [json!({"key":"A"}), json!({"key":"B"}), json!({"key":"C"})];
        let recipes = [json!([0, 1, 2, "F", "M"])];
        let mut rows = [
            json!({"speciesKey":"A","sex":"M"}),
            json!({"speciesKey":"B","sex":"F"}),
        ];
        assert!(direct_parents(&species, &recipes, &rows, 2).is_none());
        rows[0]["sex"] = json!("F");
        rows[1]["sex"] = json!("unknown");
        assert!(direct_parents(&species, &recipes, &rows, 2).is_none());
        rows[1]["sex"] = json!("M");
        assert_eq!(direct_parents(&species, &recipes, &rows, 2), Some([0, 1]));
    }

    #[test]
    fn target_matching_prefers_variant_and_keeps_distinct_species() {
        let species = [
            json!({"zh":"唤冬兽","en":"Frostallion"}),
            json!({"zh":"唤夜兽","en":"Frostallion Noct"}),
            json!({"zh":"草泥马","aliases":["测试别名"]}),
        ];
        assert_eq!(
            named_species(&species, "怎么配frostallion noct？"),
            (vec![1], false)
        );
        assert_eq!(named_species(&species, "测试别名"), (vec![2], true));
        assert_eq!(named_species(&species, "唤冬兽和唤夜兽配种").0.len(), 2);
    }
}
