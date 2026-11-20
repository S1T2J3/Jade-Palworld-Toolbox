use crate::assistant_breeding::BreedingSession;
use crate::assistant_compare::PalComparison;
use crate::assistant_data::{ReferenceAnswer, ReferenceSession, warehouse_work};
use crate::assistant_elements::ElementSession;
use crate::assistant_intent::classify;
use crate::assistant_recommend::RecommendationSession;
use crate::assistant_survival::SurvivalSession;
use crate::data::{Data, normalize};
use std::collections::HashMap;
use std::sync::OnceLock;
#[cfg(test)]
use crate::store::Store;

mod acquisition;
pub(crate) use acquisition::sources_for_recipe;
mod base_plan;
mod battle_combos;
mod cake;
mod catalog_properties;
pub(crate) mod environmental_help;
mod field;
mod facility_effect;
mod food_bonus;
pub(crate) use facility_effect::facility as named_facility;
mod habitat;
mod mechanics;
pub(crate) use field::{HandbookField, model_field_evidence};
mod planner;
mod pal_growth;
mod numeric_goal;
mod partner_levels;
mod production_power;
mod partner_loot;
mod progression;
pub(crate) mod quantities;
mod recipe_stock;
mod revival;
mod skill_fruit;
mod skill_query;
mod subject_interaction;
mod table_relations;
pub(crate) use skill_query::party_stat_sources;
mod tower_location;
mod troubleshoot;
mod work_query;
pub(crate) use work_query::JOBS;

pub(crate) fn field_text(value: &str) -> String {
    static FOLD: OnceLock<HashMap<char, char>> = OnceLock::new();
    let normalized = normalize(value);
    if normalized.is_ascii() {
        return normalized;
    }
    let fold = FOLD.get_or_init(|| {
        let [traditional, simplified]: [&str; 2] = serde_json::from_str(include_str!(
            "../../../../../scripts/pal-assistant/community-zh-fold.json"
        )).expect("bundled Chinese character mapping");
        traditional.chars().zip(simplified.chars()).collect()
    });
    normalized.chars().map(|ch| *fold.get(&ch).unwrap_or(&ch)).collect()
}

#[derive(Debug, Clone)]
pub struct AssistantReply {
    pub text: String,
    pub links: Vec<(String, String)>,
    pub suggestions: Vec<String>,
}

impl AssistantReply {
    fn new(text: &str, suggestions: &[&str]) -> Self {
        Self {
            text: text.into(),
            links: vec![],
            suggestions: suggestions.iter().map(|s| (*s).into()).collect(),
        }
    }
}

impl From<ReferenceAnswer> for AssistantReply {
    fn from(answer: ReferenceAnswer) -> Self {
        Self {
            text: answer.text,
            links: answer.links,
            suggestions: vec![],
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Conversation {
    diagnosis: Diagnosis,
    reference: ReferenceSession,
    breeding: BreedingSession,
    cake: cake::CakeSession,
    recipe_stock: recipe_stock::RecipeStock,
    economy: acquisition::EconomySession,
    habitat: habitat::HabitatSession,
    facility: facility_effect::FacilitySession,
    skill: skill_query::SkillSession,
    field_subject: Option<String>,
    equipment: RecommendationSession,
    comparison: PalComparison,
    elements: ElementSession,
    survival: SurvivalSession,
    world_id: Option<String>,
    model_context: Option<String>,
    direct_result: bool,
}

impl Conversation {
    pub fn direct_result(&self) -> bool {
        self.direct_result
    }

    pub fn model_context(&self) -> Option<&str> {
        self.model_context.as_deref()
    }

    #[cfg(test)]
    pub fn ask(&mut self, question: &str, data: &Data, store: &Store) -> AssistantReply {
        self.ask_world(question, data, store.world())
    }

    pub fn ask_world(&mut self, question: &str, data: &Data, world: &serde_json::Value) -> AssistantReply {
        let id = world["id"].as_str();
        // Keep context only for the handler that answers this turn.
        let mut previous = std::mem::take(self);
        if previous.world_id.as_deref() != id {
            previous = Self::default();
        }
        self.world_id = id.map(str::to_owned);
        let question = question.trim();
        let normalized = field_text(question);
        if let Some(answer) = previous.economy.focused_purchase(data, &normalized) {
            self.economy = previous.economy;
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = recipe_stock::building_budget(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_data::recipe_editions::scoped_ingredients(&previous.reference, &mut self.reference, data, &normalized) {
            self.field_subject = Some(normalized);
            self.direct_result = true;
            return answer.into();
        }
        if let Some(answer) = subject_interaction::answer(data, &normalized) {
            self.field_subject = Some(normalized);
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = recipe_stock::level_requirement(data, &normalized)
            .or_else(|| acquisition::ranch_loot_answer(data, &normalized)) {
            self.direct_result = true;
            return answer;
        }
        if let Some(mut answer) = environmental_help::answer(data, &normalized) {
            if environmental_help::temperature(&normalized).is_some()
                && let Some(mut native) = previous.survival.respond(data, &normalized) {
                // Keep the level-dependent advice, suggestions and next-turn
                // state. The extra links point to other verified handbook options.
                if previous.survival.can_show_alternative_clothing() {
                    for link in answer.links {
                        if !native.links.iter().any(|(_, route)| route == &link.1) { native.links.push(link); }
                    }
                }
                answer = native;
                self.survival = previous.survival;
            }
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = skill_query::compare_move_stats(data, &normalized)
            .or_else(|| catalog_properties::answer(data, &normalized))
            .or_else(|| food_bonus::answer(data, &normalized))
            .or_else(|| acquisition::batch_recipe(data, &normalized))
            .or_else(|| table_relations::answer(data, &normalized)) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = troubleshoot::combat_report(data,&normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::filtered_records(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = revival::answer(&normalized).or_else(|| mechanics::answer(&normalized)) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = battle_combos::answer(data, &normalized)
            .or_else(|| acquisition::ranch_substitute_answer(data, &normalized))
            .or_else(|| acquisition::unattended_sources(data, &normalized))
            .or_else(|| base_plan::answer(data, &normalized))
            .or_else(|| troubleshoot::work_report(&normalized))
            .or_else(|| numeric_goal::answer(data, &normalized)) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = work_query::individual_job(data, &normalized) {
            self.field_subject = Some(normalized);
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::article_definition(data, &normalized)
            .or_else(|| crate::assistant_discovery::verified_procedure(&normalized)) {
            self.direct_result=true;
            return answer;
        }
        if let Some(answer) = partner_levels::stack_effect(data, field::lookup_question(&normalized))
            .or_else(|| partner_levels::answer(data, &normalized)) {
            self.field_subject = Some(normalized);
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::carrying(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::workers(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = acquisition::family_sources(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_data::recipe_editions::answer(&previous.reference, &mut self.reference, data, &normalized) {
            self.direct_result = true;
            self.field_subject = Some(normalized);
            return answer.into();
        }
        if let Some(answer) = acquisition::craft_station(data, &normalized)
            .or_else(|| crate::assistant_discovery::recorded_fix(&normalized))
            .or_else(|| crate::assistant_elements::against_species(data, &normalized))
            .or_else(||skill_query::profile_question(data, &normalized))
            .or_else(|| partner_loot::answer(data, &normalized))
            .or_else(|| production_power::answer(data, &normalized))
            .or_else(|| progression::answer(data, &normalized))
        {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = previous.economy.respond(data, &normalized) {
            self.economy = previous.economy;
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = skill_query::innate_passive(data, &normalized)
            .or_else(|| crate::assistant_discovery::combat_preparation(data,&normalized)) {
            self.direct_result=true;
            return answer;
        }
        if let Some(answer) = previous.breeding.known_pair(data, &normalized) {
            self.breeding = previous.breeding;
            self.direct_result = true;
            return answer.into();
        }
        if let Some(answer) = pal_growth::answer(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = previous.recipe_stock.respond(question, data) {
            self.recipe_stock = previous.recipe_stock;
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::verified_record(data, &normalized) {
            self.field_subject = Some(normalized);
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = previous.facility.respond(data, &normalized) {
            self.facility = previous.facility;
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = previous.comparison.respond_constrained(data, world, &normalized) {
            self.comparison = previous.comparison;
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::staffing_evidence(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        let main_field = field::main_field(&normalized);
        let field_question = if previous.comparison.direct_result()
            && let Some(pals) = previous.field_subject.as_deref()
            && work_query::job_refinement(&normalized)
        {
            format!("{pals} {normalized}")
        } else if let Some(schematic) = acquisition::resolved_schematic_reply(data, &normalized, previous.field_subject.as_deref()) {
            schematic
        } else {
            field::resolve_followup(data, &normalized, previous.field_subject.as_deref())
        };
        if let Some(answer) = skill_fruit::answer(data, &field_question) {
            self.field_subject = Some(field_question);
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = tower_location::answer(data, &normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = previous.habitat.respond(data, field::lookup_question(&normalized)) {
            self.habitat = previous.habitat;
            self.direct_result = true;
            return answer;
        }
        if main_field.is_none_or(|field| field == HandbookField::Acquisition)
            && let Some(answer) = acquisition::answer(data, field::lookup_question(&field_question)) {
            if let Some(id) = answer.links.iter().find_map(|(_, route)| route.strip_prefix("#guide/gathering/")) {
                self.reference.remember_gathering(id);
            }
            // Prefer the actual item resolved by the catalog over incidental names
            // in the player's background, such as a seller, parent, or ingredient.
            let catalog_subject = answer.links.first().and_then(|(_, link)| {
                let id = link.strip_prefix("#items/")?.split('/').next()?;
                crate::data::array(&data.get("item-catalog-data")["items"]).iter()
                    .find(|item| item["id"] == id)
                    .and_then(|item| item["name"].as_str())
            });
            self.field_subject = Some(catalog_subject.map(field_text).unwrap_or(field_question));
            self.direct_result = true;
            return answer;
        }
        if main_field.is_none_or(|field| matches!(field, HandbookField::Passive | HandbookField::Partner))
            && let Some(answer) = previous.skill.respond(&field_question, data) {
            self.skill = previous.skill;
            self.field_subject = Some(field_question);
            self.direct_result = true;
            return answer;
        }
        if main_field.is_none_or(|field| field == HandbookField::Work)
            && let Some(answer) = work_query::respond(&field_question, data) {
            self.field_subject = Some(answer.links.iter().filter(|(_, href)| href.starts_with("#work/suitability/"))
                .map(|(name, _)| field_text(name)).collect::<Vec<_>>().join("、"));
            previous.comparison.remember_work(&answer);
            self.comparison = previous.comparison;
            self.direct_result = true;
            return answer;
        }
        if let Some(lookup) = previous.equipment.referenced_question(question, data)
            && let Some(answer) = previous.reference.respond(data, world, &lookup)
        {
            self.equipment = previous.equipment;
            self.reference = previous.reference;
            return answer.into();
        }
        if let Some(answer) = previous.equipment.respond(data, question) {
            self.equipment = previous.equipment;
            return answer;
        }
        let had_temperature = previous.survival.has_context();
        if let Some(answer) = previous.survival.respond(data, question) {
            self.direct_result = had_temperature && !answer.links.is_empty();
            self.survival = previous.survival;
            return answer;
        }
        if let Some(answer) = previous.elements.respond(data, field::lookup_question(question)) {
            self.elements = previous.elements;
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = crate::assistant_discovery::team_evidence(data, &normalized) {
            return answer;
        }
        if let Some(answer) = previous.comparison.respond(data, world, question) {
            self.direct_result = previous.comparison.direct_result();
            self.comparison = previous.comparison;
            return answer;
        }
        if let Some(answer) = previous.breeding.respond(data, world, question) {
            self.direct_result = previous.breeding.direct_result();
            self.breeding = previous.breeding;
            return answer.into();
        }
        if let Some(answer) = previous.cake.respond(question, data, world) {
            self.direct_result = matches!(previous.cake, cake::CakeSession::CountedRecipe);
            self.cake = previous.cake;
            return answer;
        }
        if let Some(mut answer) = previous.diagnosis.respond(question, classify(question)) {
            self.diagnosis = previous.diagnosis;
            if let Some(work) = answer.work {
                let stock = warehouse_work(data, world, work);
                answer.reply.text.push_str("\n\n");
                answer.reply.text.push_str(&stock.text);
                answer.reply.links.extend(stock.links);
            }
            return answer.reply;
        }
        if let Some(answer) = troubleshoot::answer(&normalized) {
            self.direct_result = true;
            return answer;
        }
        if let Some(answer) = previous.reference.respond(data, world, question) {
            self.direct_result = previous.reference.calculated || answer.links.iter().any(|(_, href)| href.starts_with("#items/") && href.ends_with("/shops"));
            self.reference = previous.reference;
            return answer.into();
        }
        if let Some(answer) = crate::assistant_discovery::discover(data, question) {
            self.direct_result = answer.links.iter().any(|(_, href)| href.starts_with("#items/") && href.ends_with("/shops"));
            return answer;
        }
        let terms = planner::terms(question);
        if !terms.is_empty()
            && let Some(mut answer) = crate::assistant_discovery::retrieve(data, &terms)
        {
            answer.text = format!(
                "按你的描述，图鉴里与{}相关的效果有\n{}",
                terms.join("、"),
                answer.text
            );
            return answer;
        }
        if let Some(guide) = crate::assistant_discovery::survival_guide_evidence(question)
            .or_else(|| crate::assistant_discovery::article_evidence(question))
        {
            return guide;
        }
        if any(
            question,
            &["帕鲁", "据点", "配种", "词条", "饰品", "图鉴", "伙伴技能"],
        ) {
            self.model_context = Some("已有图鉴和计算器尚未定位到相关条目。先澄清玩家遇到的现象或想比较的条件，涉及数值须由检索确认。".into());
        }
        AssistantReply::new(
            "你想查哪个游戏对象，或者遇到了什么情况？可以告诉我物品或帕鲁的名字；如果是在排查问题，说一下你做了什么、实际发生了什么。",
            &[],
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Inactive,
    ObserveFurnace,
    CheckOreSupply,
    CheckStockpile,
    CheckQueue,
    Smelting,
    Transport,
    Mining,
    Staffing,
}

#[derive(Clone, Debug, Default)]
struct Diagnosis {
    stage: Stage,
    no_slots: bool,
}

struct Advice {
    reply: AssistantReply,
    work: Option<&'static str>,
}

impl Stage {
    fn work(self) -> Option<&'static str> {
        match self {
            Self::Smelting | Self::Staffing => Some("生火"),
            Self::Transport => Some("搬运"),
            Self::Mining => Some("采矿"),
            _ => None,
        }
    }

    fn answer(self, no_slots: bool, show_warehouse: bool) -> Advice {
        let (text, suggestions): (&str, &[&str]) = match self {
            Self::ObserveFurnace => (
                "先看炉子的状态。缺金属锭时，它一直有人在烧，还是常停下来？这能区分冶炼速度和原料、排班问题。",
                &["炉子一直在烧，矿石还有积压", "炉子经常停下来"],
            ),
            Self::CheckOreSupply => (
                "炉子停下时，箱子里还有能用的金属矿石吗？有矿再查订单和生火分工，没矿就先查矿石有没有送回来。",
                &["箱子里还有矿石", "箱子里没矿了"],
            ),
            Self::CheckStockpile => (
                "看一下采矿点附近：地上是否堆着没送回来的矿？地面有积压就优先查搬运，地面和箱子都空才需要查采矿。",
                &["矿全堆在地上", "地上和箱子都没有矿"],
            ),
            Self::CheckQueue => (
                "矿石够时，先确认熔炉有金属锭订单。订单在等，有生火帕鲁到炉前工作吗？",
                &[
                    "有订单，但没人来生火",
                    "订单还没排",
                    "炉子一直在烧，矿石还有积压",
                ],
            ),
            Self::Smelting => (
                "炉子持续工作时，先让生火适性较高的一只接手这座炉子。保持同一配方和订单量，比较同样时间产出的金属锭数，确认换班是否有效。",
                &["我仓库里哪只适合？", "据点已经满员了"],
            ),
            Self::Transport => (
                "先在采矿点附近留一个能接收矿石、还有空位的箱子，让一只有搬运适性的帕鲁负责这段路。观察地上的矿有没有减少、箱子里的矿有没有增加。",
                &["我仓库里哪只适合？", "据点已经满员了"],
            ),
            Self::Mining => (
                "先看矿点是否在产出，采矿帕鲁能否到达并持续工作。确认开采速度不足后，再试换采矿适性较高的一只；比较同一段时间箱子新增的矿石。",
                &["我仓库里哪只适合？", "据点已经满员了"],
            ),
            Self::Staffing => (
                "有订单、有矿，先将一只有生火适性的帕鲁分配到炉子，并观察它能否到达、订单进度是否开始增加。",
                &["我仓库里哪只适合？", "据点已经满员了"],
            ),
            Self::Inactive => unreachable!("新话题交给图鉴"),
        };
        let mut reply = AssistantReply::new(text, suggestions);
        if no_slots {
            reply.text.push_str(
                "\n据点已满员。先给在岗帕鲁调整分工，需要换班时用一只替换一只，保持总数不变。",
            );
            reply.suggestions.retain(|s| !s.contains("满员"));
        }
        Advice {
            reply,
            work: if show_warehouse { self.work() } else { None },
        }
    }
}

fn any(s: &str, words: &[&str]) -> bool {
    words.iter().any(|word| s.contains(word))
}

impl Diagnosis {
    /// The classifier proposes an interpretation. Inventory lookups and production
    /// recommendations still require a game subject or an active diagnostic thread.
    fn respond(&mut self, q: &str, intent: Option<&str>) -> Option<Advice> {
        let active = self.stage != Stage::Inactive;
        // Require a production subject, a referential follow-up, or an explicit
        // observation before giving an imperfect intent prediction any weight.
        let intent = intent.filter(|_| {
            any(
                q,
                &[
                    "矿",
                    "锭",
                    "炉",
                    "烧",
                    "冶炼",
                    "炼铁",
                    "生火",
                    "搬运",
                    "箱子",
                    "原料",
                    "满员",
                    "名额",
                    "工位",
                    "岗位",
                    "加人",
                    "位置",
                    "它",
                    "还是这样",
                ],
            )
        });
        let supply = any(
            q,
            &[
                "金属锭",
                "金属錠",
                "铁锭",
                "熔炉",
                "精炼炉",
                "冶炼",
                "烧矿",
                "炼铁",
            ],
        ) || (q.contains("炉子") && any(q, &["据点", "帕鲁", "矿", "金属"]));
        let mining_site = any(q, &["矿区", "矿场", "采矿点", "矿点", "采矿场", "矿工"]);
        let mentions_ore = any(q, &["矿", "金属", "铁锭"]);
        let new_problem = supply
            && any(
                q,
                &[
                    "不够",
                    "不出",
                    "不动",
                    "不来",
                    "跟不上",
                    "缺",
                    "太少",
                    "太慢",
                    "老停",
                    "停工",
                    "断",
                    "排查",
                    "不足",
                    "慢",
                    "半天",
                    "中途停",
                    "开开停停",
                    "时有时无",
                ],
            );
        let on_ground = any(
            q,
            &[
                "堆在地上",
                "堆了一地",
                "堆一地",
                "满地",
                "铺满",
                "自己捡",
                "自己去捡",
                "没人搬",
                "没人运",
                "没人捡",
            ],
        );
        let new_transport =
            (mentions_ore || mining_site) && (on_ground || intent == Some("ore_transport"));
        if !active && !new_problem && !new_transport {
            return None;
        }
        // A named lookup should win over a previous production diagnosis.
        if !new_problem && any(q, &["在哪", "哪里", "去哪", "怎么抓", "怎么拿", "怎么挖"])
        {
            return None;
        }
        if any(
            q,
            &[
                "怎么获得",
                "怎么获取",
                "哪里获得",
                "如何获得",
                "掉率",
                "概率",
                "用途",
                "作用",
                "用处",
                "有什么用",
                "做什么用",
                "干嘛用",
                "研发",
                "科技",
                "解锁",
                "几级",
                "多少级",
                "扩容",
                "配方",
                "配种",
                "词条",
                "伙伴技能",
            ],
        ) {
            return None;
        }
        let capacity_available = any(q, &["没满", "未满", "不满员", "还有空位", "还有位置"]);
        let no_slots = !capacity_available
            && (any(
                q,
                &[
                    "满员",
                    "没空位",
                    "没位置",
                    "没名额",
                    "不能加人",
                    "不能再加",
                    "不想增加",
                    "不加人",
                ],
            ) || (active && intent == Some("no_base_slots")));
        if no_slots {
            self.no_slots = true;
        } else if capacity_available {
            self.no_slots = false;
        }
        let stock_query = any(
            q,
            &["仓库", "我有的", "我现有", "我手里", "用哪只", "换哪只"],
        );
        if stock_query && active && self.stage.work().is_some() {
            // Inherit the diagnosed job only for a referential question. New
            // named pals, jobs and items must pass to their own handlers.
            let cleaned = q.trim_matches(|c: char| !c.is_alphanumeric());
            let mut rest = cleaned;
            for prefixes in [
                &["那就", "那么", "那", "请问"][..],
                &["只用", "只看", "看看"],
                &[
                    "我的仓库里",
                    "我仓库里",
                    "我的仓库",
                    "我仓库",
                    "仓库里",
                    "我手里",
                    "我现有的",
                    "我有的",
                ],
            ] {
                if let Some(next) = prefixes.iter().find_map(|part| rest.strip_prefix(*part)) {
                    rest = next;
                }
            }
            let rest = rest
                .trim_matches(|c: char| !c.is_alphanumeric() || matches!(c, '呢' | '吗' | '啊'));
            if matches!(
                rest,
                "" | "哪只"
                    | "有谁"
                    | "谁"
                    | "有哪几只"
                    | "有哪只"
                    | "哪只适合"
                    | "哪只最适合"
                    | "哪只合适"
                    | "哪些适合"
                    | "谁适合"
                    | "谁最合适"
                    | "用哪只"
                    | "换哪只"
            ) {
                return Some(self.stage.answer(self.no_slots, true));
            }
            if any(
                q,
                &[
                    "哪", "谁", "适合", "负重", "效果", "数量", "生火", "搬运", "采矿",
                ],
            ) {
                return None;
            }
        }
        let empty_floor = any(
            q,
            &[
                "地上和箱子都没有",
                "地上和箱子都空",
                "地上没有",
                "地上没矿",
                "地上没堆",
                "地上也没",
                "没堆在地上",
                "没有堆在地上",
                "没堆一地",
                "没有积压",
                "地上是空",
            ],
        );
        let no_ore = any(
            q,
            &[
                "没矿",
                "没有矿",
                "空箱",
                "箱子空",
                "箱里空",
                "箱子里空",
                "矿石用完",
                "缺矿",
            ],
        );
        let has_ore = !no_ore && any(q, &["有矿", "矿石够", "原料够", "还有矿", "原料充足"]);
        let burning = !any(
            q,
            &["没有一直", "没一直", "没有人在烧", "不是一直", "不一直"],
        ) && any(
            q,
            &[
                "一直烧",
                "一直在烧",
                "一直有人",
                "没停过",
                "不停地烧",
                "烧得慢",
                "烧太慢",
                "烧得太慢",
                "一直工作",
                "一直在炼",
                "一直没停",
            ],
        );
        let idle = any(
            q,
            &[
                "老停",
                "经常停",
                "常停",
                "停下来",
                "开开停停",
                "停工",
                "没有一直",
                "没一直",
                "不是一直",
                "断断续续",
            ],
        );
        let is_furnace_topic = active || supply;
        let asks_for_diagnosis = any(q, &["是不是", "要不要", "要换", "应不应该", "需不需要"]);
        let stage = if empty_floor
            && ((q.contains("箱") && no_ore) || self.stage == Stage::CheckStockpile)
        {
            Stage::Mining
        } else if !empty_floor
            && (on_ground || (intent == Some("ore_transport") && !no_ore && !asks_for_diagnosis))
            && (active || new_transport)
        {
            Stage::Transport
        } else if is_furnace_topic
            && any(
                q,
                &["没下单", "没排订单", "订单还没排", "没有订单", "没放订单"],
            )
        {
            self.stage = Stage::CheckQueue;
            return Some(Advice {
                reply: AssistantReply::new(
                    "先排一个金属锭订单，看看炉子会不会开工。如果订单一直在等，再确认生火帕鲁能不能到达炉前。",
                    &["有订单，但没人来生火"],
                ),
                work: None,
            });
        } else if is_furnace_topic
            && any(
                q,
                &["没人来生火", "没人生火", "没人来烧", "无人值守", "没人烧"],
            )
        {
            if self.stage == Stage::CheckQueue || (has_ore && q.contains("订单")) {
                Stage::Staffing
            } else {
                Stage::CheckQueue
            }
        } else if is_furnace_topic && no_ore && !burning {
            Stage::CheckStockpile
        } else if is_furnace_topic
            && (burning || (intent == Some("smelting_slow") && !asks_for_diagnosis))
            && !idle
        {
            Stage::Smelting
        } else if has_ore && matches!(self.stage, Stage::CheckOreSupply | Stage::CheckStockpile) {
            Stage::CheckQueue
        } else if is_furnace_topic && (idle || intent == Some("furnace_idle")) {
            Stage::CheckOreSupply
        } else if is_furnace_topic && intent == Some("ore_shortage") {
            Stage::CheckStockpile
        } else if new_problem || (active && intent == Some("ingot_problem")) {
            Stage::ObserveFurnace
        } else if active && (no_slots || any(q, &["还有空位", "还有位置", "怎么验证", "怎么确认"]))
        {
            self.stage
        } else {
            return None;
        };
        self.stage = stage;
        Some(stage.answer(self.no_slots, stock_query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn queries_bind_power_to_machines_and_loot_to_the_defeated_target() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in ["resource-guides-data","partner-skills-data","paldex-data","pal-details-data","combat-guide-data",
                   "facilities-data","item-catalog-data","capture-data","spawn-data","map-data"] {
            let row = serde_json::from_slice(&std::fs::read(root.join(format!("{id}.json"))).unwrap()).unwrap();
            d.tables.insert(id.into(),Arc::new(row));
        }
        for (q, facts) in [
            ("How much power for two Oil Extractors?",vec!["4000","2000"]),
            ("三台普通原油提炼机和两台高压原油提炼机一共耗电多少？",vec!["12000","3000"]),
            ("三台原油提炼机，只开一台，另外两台停机，耗电多少？",vec!["1 × 2000 = 2000"]),
            ("两台原油提炼机连开30秒总共耗多少电？",vec!["120000"]),
            ("Does upgrading Cryolinx increase the chance of high quailty pal oil drops? (Dragon hunter)",vec!["dragon","High Quality Pal Oil","+80%","Eligible targets"]),
            ("我带冰棘兽击败伏特喵，能增加掉落吗？",vec!["伏特喵不符合"]),
            ("我带冰棘兽去打普通的精灵龙，能增加它的掉落吗？",vec!["猎龙者","精灵龙符合"]),
            ("提升掉落的伙伴技能，也能提高团战结算奖励吗？",vec!["属性帕鲁","单凭它无法确定","结算"]),
            ("暗巫猫的伙伴技能会加倍突袭掉落的蛋吗？",vec!["无属性","奖励蛋"]),
            ("請問魚餌怎麼做比較快",vec!["简陋钓饵","一组产出 10","黏液"]),
            ("水流射击和水枪两招的冷却和威力分别是多少？",vec!["水枪威力更高","两招相同","2秒"]),
            ("种子机关枪和种子地雷两招相比哪个冷却更久？",vec!["4秒","8秒","种子地雷冷却更久"]),
            ("What is the additional crafting probability?",vec!["research","product category"]),
            ("天生潜力100跟练高了吃果实再补到100一样吗？",vec!["面板也相同","重新计算"]),
            ("請問傑諾多蘭帕魯的座標?",vec!["固定的野生捕捉坐标","召唤祭坛"]),
        ] {
            let mut convo=Conversation::default();
            let answer=convo.ask_world(q,&d,&serde_json::Value::Null);
            assert!(convo.direct_result() && facts.iter().all(|v| answer.text.contains(v)), "{q}: {}", answer.text);
        }
        assert!(partner_loot::answer(&d,"does killing cryolinx drop oil?").is_none());
        assert!(acquisition::batch_recipe(&d,"怎么钓鱼比较快？").is_none());
        assert!(production_power::answer(&d,"where do you build two oil extractors?").is_none());
    }

    #[test]
    fn open_questions_reach_the_named_source_and_retrieve_grounded_effects() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "search-index-data",
            "enemy-data",
            "item-catalog-data",
            "item-effects-data",
            "resource-guides-data",
            "pal-details-data",
            "expedition-data",
            "adventure-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            d.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let dir = crate::store::tests::Temp::new();
        let store = Store::open(dir.0.clone());
        let mut session = Conversation::default();
        for (question, fact) in [
            ("流浪商人卖的蛋要多少钱", "200"),
            ("草原的洞穴远征得等多久", "30 分钟"),
            ("工匠精神手术要多少金币", "50000"),
            ("原油提炼机每秒耗电多少", "2000"),
            ("想换身上的东西，背包太重，总得来回卸货", "负重"),
        ] {
            let answer = session.ask(question, &d, &store);
            if question == "流浪商人卖的蛋要多少钱" {
                assert!(session.direct_result());
            }
            assert!(
                answer.text.contains(fact) && !answer.links.is_empty(),
                "{question}\n{}",
                answer.text
            );
        }
    }

    #[test]
    fn offline_direction_followups_and_locations_use_the_shipped_tables() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in [
            "combat-guide-data",
            "map-data",
            "spawn-data",
            "pal-details-data",
            "item-catalog-data",
            "resource-guides-data",
            "capture-data",
        ] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            d.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let dir = crate::store::tests::Temp::new();
        let store = Store::open(dir.0.clone());
        let mut session = Conversation::default();
        let mut ask = |q| session.ask(q, &d, &store);
        let counter = ask("打龙属性应该用什么");
        assert!(counter.text.contains("冰属性攻击技能"), "{}", counter.text);
        assert!(
            counter
                .links
                .iter()
                .any(|(_, route)| route == "#guide/elements")
        );
        assert!(
            ask("那用火系行吗？")
                .text
                .contains("没有列出火属性攻击对龙属性的克制优势")
        );
        assert!(ask("龙系技能打谁有优势").text.contains("克制暗属性"));
        let dragon = ask("空涡龙具体出没在哪，给我个点位");
        assert!(
            dragon
                .links
                .iter()
                .any(|(_, route)| route == "#spawns/JetDragon")
        );
        assert!(
            dragon
                .links
                .iter()
                .any(|(_, route)| route.starts_with("#map/coords/"))
        );
        assert!(ask("那它的坐标呢").text.contains("-553, -1332"));
        let raid = ask("贝菈露洁的栖息地在哪");
        assert!(!raid.text.contains("空涡龙"));
        assert!(raid.text.contains("突袭蛋"), "{}", raid.text);
    }

    #[test]
    fn production_counts_follow_the_item_and_round_up_whole_batches() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in ["item-catalog-data", "facilities-data"] {
            let bytes = std::fs::read(root.join(format!("{id}.json"))).unwrap();
            data.tables
                .insert(id.into(), Arc::new(serde_json::from_slice(&bytes).unwrap()));
        }
        let temp = crate::store::tests::Temp::new();
        let store = Store::open(temp.0.clone());
        let mut session = Conversation::default();
        let mut ask = |q| session.ask(q, &data, &store);
        let single = ask("蛋糕怎么合成");
        assert!(single.text.contains("料理锅") && single.text.contains("面粉 × 5"));
        let twelve = ask("那做十二个呢？");
        assert!(twelve.text.contains("需做 12 批") && twelve.text.contains("实际产出 12 份"));
        for material in ["面粉 × 60", "牛奶 × 84", "蜂蜜 × 24"] {
            assert!(twelve.text.contains(material), "{}", twelve.text);
        }
        assert_eq!(ask("做12个蛋糕要多少材料").text, twelve.text);
        assert!(twelve.links.iter().any(|(_, r)| r == "#items/Cake"));
        let arrows = ask("我有十二份蛋糕，做 23 支箭要多少材料？");
        assert!(
            arrows.text.contains("需做 3 批") && arrows.text.contains("实际产出 30 份"),
            "{}",
            arrows.text
        );
        assert!(arrows.text.contains("木材 × 6") && arrows.text.contains("石头 × 6"));
        assert!(arrows.links.iter().any(|(_, r)| r == "#items/Arrow"));
        let two = ask("那做两支呢？");
        assert!(two.text.contains("实际产出 10 份") && two.text.contains("木材 × 2"));
        let inventory = ask("我二十级，仓库有十二块木材，箭怎么制作？");
        assert!(!inventory.text.contains("实际产出"));
        let level = ask("我20级，蛋糕怎么合成？");
        assert!(level.text.contains("面粉 × 5") && !level.text.contains("实际产出"));
    }

    #[test]
    fn equipment_filters_follow_player_goal_and_release_unrelated_topics() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in [
            "item-catalog-data",
            "item-effects-data",
            "facilities-data",
            "resource-guides-data",
        ] {
            let entry = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            data.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&entry).unwrap()),
            );
        }
        let temp = crate::store::tests::Temp::new();
        let mut store = Store::open(temp.0.clone());
        let mut session = Conversation::default();
        let ask = |s: &mut Conversation, q| s.ask(q, &data, &store);
        let first = ask(&mut session, "输出饰品推荐");
        assert!(first.text.contains("汪宗师") && first.text.contains("自身攻击力"));
        assert_eq!(
            ask(&mut session, "有没有能加伤害的首饰？").links,
            first.links
        );
        let pal = ask(&mut session, "给帕鲁用呢？");
        assert!(pal.text.contains("队伍帕鲁"));
        let dark = ask(&mut session, "主打暗属性技能");
        assert_eq!(
            dark.links.first().unwrap().1,
            "#items/Otomo_ATDark_ElementBoost_1"
        );
        let crafted = ask(&mut session, "只看我能制作的");
        assert!(crafted.text.contains("配方"));
        assert!(
            ask(&mut session, "第一个怎么获得？")
                .links
                .iter()
                .any(|(_, r)| r == &crafted.links[0].1)
        );
        let lookup = ask(&mut session, "帕鲁矿碎块怎么获得？");
        assert!(
            lookup
                .links
                .iter()
                .any(|(_, r)| r == "#guide/gathering/Pal_crystal_S")
        );
        assert!(!ask(&mut session, "只看能制作的").text.contains("汪宗师"));

        ask(&mut session, "玩家想打得更疼，戴什么好？");
        let early = ask(&mut session, "只看18级能做的");
        assert!(early.text.contains("攻击吊坠") && !early.text.contains("汪宗师"));
        store.world_mut()["id"] = serde_json::json!("another-world");
        let answer = session.ask("只看能制作的", &data, &store);
        assert!(!answer.text.contains("攻击吊坠"));
    }

    #[test]
    fn switches_between_diagnosis_recipe_and_warehouse_using_real_data() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for name in ["item-catalog-data", "ranch-data", "paldex-index-data"] {
            let bytes = std::fs::read(root.join(format!("{name}.json"))).unwrap();
            data.tables.insert(
                name.into(),
                Arc::new(serde_json::from_slice(&bytes).unwrap()),
            );
        }
        let temp = crate::store::tests::Temp::new();
        let store = Store::open(temp.0.clone());
        let mut session = Conversation::default();
        let mut ask = |q| session.ask(q, &data, &store);
        let answer = ask("金属锭经常不够，要换生火帕鲁吗？");
        assert!(answer.text.contains("炉子的状态"));
        let answer = ask("蛋糕隔几天就断产");
        assert!(answer.text.contains("材料") && !answer.text.contains("炉子"));
        assert!(
            ask("总缺蜂蜜")
                .links
                .iter()
                .any(|(_, r)| r == "#work/ranch/item/Honey")
        );
        assert!(
            ask("伯乐眼镜有什么用？")
                .links
                .iter()
                .any(|(_, r)| r == "#items/Accessory_TalentChecker")
        );
        ask("矿点的矿堆了一地");
        let answer = ask("我仓库里的谁能浇水");
        assert!(answer.text.contains("浇水") && !answer.text.contains("搬运"));
        ask("金属锭好几次不够用");
        assert!(!ask("主机散热风扇一直在转怎么解决").text.contains("生火"));
    }

    #[test]
    fn idle_checks_supply_then_transport_and_honors_capacity() {
        let mut d = Diagnosis::default();
        for (q, stage) in [
            ("据点的金属锭总是不够", Stage::ObserveFurnace),
            ("炉子经常停下来", Stage::CheckOreSupply),
            ("箱子里没矿了", Stage::CheckStockpile),
            ("矿全堆在地上", Stage::Transport),
        ] {
            d.respond(q, None).unwrap();
            assert_eq!(d.stage, stage);
        }
        assert!(
            d.respond("但据点满员了", None)
                .unwrap()
                .reply
                .text
                .contains("替换")
        );
        assert_eq!(
            d.respond("那我仓库里哪只适合？", None).unwrap().work,
            Some("搬运")
        );
        assert!(d.respond("我仓库里哪些能生火", None).is_none());
    }

    #[test]
    fn checks_queue_and_inputs_before_naming_a_worker() {
        let mut d = Diagnosis::default();
        for (q, stage) in [
            ("金属锭总是不够", Stage::ObserveFurnace),
            ("没有一直烧，炉子经常停", Stage::CheckOreSupply),
            ("箱子还有矿", Stage::CheckQueue),
            ("有订单，但没人来生火", Stage::Staffing),
        ] {
            d.respond(q, None).unwrap();
            assert_eq!(d.stage, stage);
        }
        assert_eq!(d.respond("我仓库里有谁", None).unwrap().work, Some("生火"));
    }

    #[test]
    fn separates_missing_ore_from_missing_transport() {
        let mut d = Diagnosis::default();
        d.respond("金属锭总是不够", None).unwrap();
        d.respond("箱子里空了", None).unwrap();
        d.respond("地上和箱子都没有矿", None).unwrap();
        assert_eq!(d.stage, Stage::Mining);
        assert!(d.respond("矿碎块掉率是多少", None).is_none());
    }

    #[test]
    fn trained_intent_can_interpret_a_followup_but_needs_a_game_context() {
        let mut d = Diagnosis::default();
        assert!(
            d.respond("上午炉子停了两回", Some("furnace_idle"))
                .is_none()
        );
        d.respond("金属锭不够怎么办", None).unwrap();
        d.respond("烧一会儿就歇一会儿", Some("furnace_idle"))
            .unwrap();
        assert_eq!(d.stage, Stage::CheckOreSupply);
        assert!(
            d.respond("伯乐眼镜怎么获得", Some("furnace_idle"))
                .is_none()
        );
        assert!(
            Diagnosis::default()
                .respond("据点满员", Some("no_base_slots"))
                .is_none()
        );
    }
}
