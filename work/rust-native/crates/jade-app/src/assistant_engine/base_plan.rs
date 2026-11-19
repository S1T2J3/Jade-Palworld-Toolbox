use super::{AssistantReply, JOBS, facility_effect::facilities};
use crate::data::{Data, array, text};
use serde_json::Value;
mod interaction;
mod zoning;

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

struct Plan<'a> {
    data: &'a Data,
    en: bool,
    text: Vec<String>,
    links: Vec<(String, String)>,
}

fn job_name(key: &str, en: bool) -> &str {
    if en {
        JOBS.iter()
            .find(|job| job.0 == key)
            .map_or(key, |job| job.1)
    } else {
        key
    }
}

fn workstations<'a>(data: &'a Data, q: &str) -> Vec<&'a Value> {
    let mut terms = q.to_owned();
    for (alias, name) in [
        ("lumber farm", "logging site"),
        ("wood farm", "logging site"),
        ("logging farm", "logging site"),
        ("berry farm", "berry plantation"),
        ("wheat farm", "wheat plantation"),
        ("breeding pen", "breeding farm"),
        ("fish pond", "fishing pond"),
        ("配种场", "配种牧场"),
        ("繁殖场", "配种牧场"),
        ("繁育场", "配种牧场"),
        ("钓鱼池", "垂钓池"),
        ("浆果田", "野莓农园"),
        ("野莓田", "野莓农园"),
        ("小麦田", "小麦农园"),
    ] {
        if terms.contains(alias) {
            terms = terms.replace(alias, name);
        }
    }
    facilities(data, &terms)
        .into_iter()
        .map(|(row, _, _)| row)
        .filter(|row| !array(&row["required"]).is_empty() || !array(&row["jobs"]).is_empty())
        .collect()
}

impl Plan<'_> {
    fn link(&mut self, name: &str, route: String) {
        if !self.links.iter().any(|(_, old)| old == &route) {
            self.links.push((name.to_owned(), route));
        }
    }

    fn station(&mut self, id: &str) -> Option<String> {
        let row = array(&self.data.get("facilities-data")["facilities"])
            .iter()
            .find(|r| r["id"] == id)?;
        let name = if self.en {
            id.replace('_', " ")
        } else {
            text(row, "name").into()
        };
        self.link(&name, format!("#work/facilities/{id}"));
        Some(format!("{name} (Lv.{})", row["level"].as_u64()?))
    }

    fn item(&self, id: &str) -> Option<&Value> {
        array(&self.data.get("item-catalog-data")["items"])
            .iter()
            .find(|r| r["id"] == id)
    }

    fn recipe(&mut self, id: &str) -> Option<String> {
        let item = self.item(id)?;
        let lang = if self.en { "en" } else { "name" };
        let name = text(item, lang).to_owned();
        let recipe = array(&item["recipes"]).first()?;
        let ingredients = array(&recipe["materials"])
            .iter()
            .map(|pair| {
                Some(format!(
                    "{} × {}",
                    text(self.item(pair[0].as_str()?)?, lang),
                    pair[1].as_u64()?
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        self.link(&name, format!("#items/{id}"));
        Some(format!("{name} = {}", ingredients.join(" + ")))
    }

    fn food(&mut self, q: &str) -> Option<()> {
        let feed = self.station("Feed_Box")?;
        let early = has(
            q,
            &[
                "刚开局",
                "新手",
                "初期",
                "刚起步",
                "starting out",
                "early game",
                "early-game",
                "beginner",
            ],
        );
        let berries = self.recipe("Baked_Berries")?;
        let berry_plot = self.station("Berry_Plantation")?;
        let camp = self.station("Campfire")?;
        self.text.push(if self.en {
            format!("Start with {berry_plot}, cook at {camp} and stock the reachable {feed}. Early recipe: {berries}.")
        } else {format!("先用{berry_plot}供料，在{camp}烤制，再放入帕鲁够得到的{feed}。过渡配方为{berries}。")});
        if !early || has(q, &["以后", "之后", "后期", "later", "mid game", "midgame"]) {
            let tomato = self.station("Tomato_Plantation")?;
            let lettuce = self.station("Lettuce_Plantation")?;
            let cook = self.station("Cooking_Pot")?;
            let dish = self.recipe("Salad")?;
            self.text.push(if self.en {
                format!(
                    "After unlocking {tomato} and {lettuce}, use {cook} to prepare batches: {dish}."
                )
            } else {
                format!("解锁{tomato}和{lettuce}后，可改在{cook}批量做{dish}。")
            });
        }
        self.text.push(if self.en {"Cover planting, watering and gathering on the fields. Use transporting for the harvest and kindling for cooking. Watch one cycle: if harvested crops remain on the ground, improve hauling first; if the cooking station is waiting for input, check the field and storage."}
            else {"农园要有人播种、浇水和采集，收成还要搬运，做熟食需要生火。看一轮生产：地上堆满收成就先补搬运；锅里总是缺原料就先查农园与仓储。"} .into());
        Some(())
    }

    fn ranch_and_breed(&mut self) -> Option<()> {
        let ranch = self.station("Ranch")?;
        let mill = self.station("Mill")?;
        let cook = self.station("Cooking_Pot")?;
        let breeder = self.station("Breeding_Farm")?;
        let cake = self.recipe("Cake")?;
        let flour = self.recipe("Flour")?;
        let mut pairs = Vec::new();
        for (pal, ingredient) in [
            ("ChickenPal", "Egg"),
            ("CowPal", "Milk"),
            ("SoldierBee", "Honey"),
        ] {
            let output = &self.data.get("ranch-data")["pals"][pal]["tiers"];
            if !array(output)
                .iter()
                .any(|tier| array(tier).iter().any(|v| v["item"] == ingredient))
            {
                return None;
            }
            let name = if self.en {
                text(self.data.pal(pal)?, "en").to_owned()
            } else {
                self.data.pal_name(pal)
            };
            let product =
                text(self.item(ingredient)?, if self.en { "en" } else { "name" }).to_owned();
            self.link(&name, format!("#paldex/{pal}"));
            pairs.push(format!("{name} → {product}"));
        }
        let products = pairs.join(", ");
        self.text.push(if self.en {format!("Keep workers on existing supply lines at first. Start one {ranch} near food production and rotate producers to stock each ingredient: {products}. Grow berries and wheat; run {mill} with Watering. {flour}.")}
            else {format!("先保留维持现有生产的岗位，在食品生产附近建一个{ranch}，轮换产物不足的帕鲁：{products}。另种野莓和小麦，安排浇水工在{mill}加工，{flour}。")});
        self.text.push(if self.en {format!("Use Kindling at {cook}: {cake}. Put the cake in the {breeder} box, assign a male and a female, then incubate the resulting egg. Begin with one pair and increase production only when food and crafting keep pace.")}
            else {format!("在{cook}用生火工制作：{cake}。把蛋糕放入{breeder}的箱子，派一公一母，产蛋后孵化。先试一对，待供餐和制作跟得上再加人。")});
        Some(())
    }

    fn health(&mut self) -> Option<()> {
        let meds = self.station("Medicine_Rack")?;
        let clinic = self.station("Clinic")?;
        let food = self.station("Feed_Box")?;
        self.text.push(if self.en {format!("Stock the medicine for the actual illness in a reachable {meds}, so sick base Pals can take it. A regular supply chest stores items. {clinic} needs Medicine Production to maintain base hygiene; its worker does not carry medicines from a chest to each patient. Put meals for hunger in {food}.")}
            else {format!("看清疾病后，把对应药品放进帕鲁能到达的{meds}，生病的工人可自行取用。普通储物箱只存物品。{clinic}由制药适性帕鲁维持据点卫生；工人不会从普通箱取药逐只治疗。饥饿则把食物放入{food}。")});
        Some(())
    }

    fn explain_work(&mut self, row: &Value, monitor: &str) -> Option<()> {
        let id = text(row, "id");
        let station = self.station(id)?;
        let line = match id {
            "Breeding_Farm" => {
                if self.en {
                    format!(
                        "{station}: put one male and one female in base worker slots. Release existing fixed assignments. Pick up each parent and throw it at the farm, or use fixed assignments at {monitor}. Check the farm shows both occupants and put Cake in its own chest. Breeding has no required work suitability."
                    )
                } else {
                    format!(
                        "{station}需要一公一母。把双亲放入据点工作栏，解除其他固定岗位，再逐只抱起投到同一牧场，或在{monitor}的固定派工界面分配。确认牧场面板显示两只都已到位，把蛋糕放入牧场自己的箱子。配种没有工种适性要求。"
                    )
                }
            }
            "Ancient_Hatchery" => {
                if self.en {
                    format!(
                        "{station} handles breeding and hatching automatically. Check its electricity supply and the parents selected at the machine. No particular work suitability is required."
                    )
                } else {
                    format!(
                        "{station}自动完成产蛋和孵化。先查供电，再在设施界面核对选入的亲代。该设施没有指定工种适性。"
                    )
                }
            }
            _ if array(&row["required"]).iter().any(|pair| pair[0] == "任意") => {
                if self.en {
                    format!(
                        "{station} accepts a base Pal without a specific work suitability. Check that this Pal is deployed here, the facility is accessible and its interaction panel accepts the assignment."
                    )
                } else {
                    format!(
                        "{station}允许任意适性的帕鲁。核对它已在此据点出勤、设施可以到达，再到设施交互界面核对实际岗位。"
                    )
                }
            }
            _ => {
                let required = array(&row["required"])
                    .iter()
                    .filter_map(|v| {
                        Some(format!(
                            "{} Lv.{}",
                            job_name(v[0].as_str()?, self.en),
                            v[1].as_u64()?
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join(" / ");
                let stages = array(&row["jobs"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| job_name(s, self.en))
                    .collect::<Vec<_>>();
                if !required.is_empty() {
                    self.text.push(format!("{station}: {required}."));
                }
                if !stages.is_empty() {
                    let order = stages.join(" → ");
                    if self.en {
                        format!(
                            "{station} needs {order} in that order. Watch which stage is waiting, allow that job at {monitor} and provide a suitable worker. A transporter can clear the harvest to compatible storage."
                        )
                    } else {
                        format!(
                            "{station}按{order}依次工作。看当前停在哪一段，在{monitor}启用该工种并派有对应适性的工人。收成可由搬运工送到允许该类物品的箱子。"
                        )
                    }
                } else if row["group"] == "垂钓池" {
                    if self.en {
                        format!(
                            "Use the box beside the pond to choose the fish-shadow order, supply the bait it asks for and start the job. Then assign a Handiwork Pal; if still unavailable, check Handiwork at {monitor} and the route to the pond."
                        )
                    } else {
                        format!(
                            "先在池边操作箱选择鱼影对应的作业，投入界面所需的鱼饵并启动。再派手工作业帕鲁；仍无法分配时，查{monitor}是否启用手工及通往池边的路线。"
                        )
                    }
                } else if row["group"] == "伐木/挖掘" || id == "Ranch" {
                    if self.en {
                        format!(
                            "Deploy a Pal with the listed suitability at this site. Allow the job at {monitor} and remove other fixed assignments. Once it produces, clear the output or add transporting and storage."
                        )
                    } else {
                        format!(
                            "在此设施派有上述适性的据点工人，在{monitor}启用该工种并解除其他固定岗位。已有产物时，清空成品或安排搬运及收纳。"
                        )
                    }
                } else if self.en {
                    format!(
                        "At {monitor}, allow the listed job for this worker and release conflicting assignments. For a crafting station, select a recipe, supply its ingredients, start the order and check the output space."
                    )
                } else {
                    format!(
                        "在{monitor}启用这个工人的对应工种并解除其他固定派工。加工工位要选配方、备好原料并启动排程，同时查看成品空间。"
                    )
                }
            }
        };
        self.text.push(line);
        Some(())
    }

    fn diagnose(
        &mut self,
        q: &str,
        natural: bool,
        vertical: bool,
        stations: &[&Value],
    ) -> Option<()> {
        let monitor = self.station("Monitoring_Stand")?;
        if vertical {
            self.text.push(if self.en {"Trace the route from the worker’s current floor to the work side of the target. Check the stairs or ramp joins both floors, the landing and turns are wide enough, and there is headroom. Compare the same job at ground level with a smaller worker before changing the building."}
                else {"从工人所在楼层一直查看到工位互动面：楼梯或坡道与两层楼板要接通，转角、平台宽度和头顶空间也要足够。先用小体型工人并对照同一工位在一层是否可用，再决定是否改建。"} .into());
        }
        let tree = has(q, &["tree", "lumber", "树", "砍木", "伐木"]);
        if natural {
            let posted = self.station(if tree { "Logging_Site" } else { "Stone_Pit" })?;
            self.text.push(if self.en {format!("Start with one reachable {} inside the base boundary. Confirm the worker has {} suitability and that its job type is allowed at {monitor}. Release any other fixed assignment. Compare the worker on the natural node with the same worker at {posted}; the site tests the work assignment independently of the natural node.",if tree {"tree"} else {"mineral node"},if tree {"Lumbering"}else{"Mining"})}
                else {format!("挑一个在据点范围内、能走到的{}，先确认工人有{}适性，{monitor}允许这个工种，并解除它其他固定岗位。用同一只工人对照测试天然资源和{posted}；能在设施工作只能说明其作业适性可用。",if tree {"天然树"}else{"矿点"},if tree {"伐木"}else{"采矿"})});
        } else if stations.is_empty() {
            self.text.push(if self.en {format!("Place the worker in a base slot, clear conflicting assignments and check the available job at {monitor}.")}
                else {format!("先把工人放入据点工作栏，解除其他固定派工，再到{monitor}核对目标设施当前可用的岗位。")});
        } else {
            for row in stations {
                self.explain_work(row, &monitor)?;
            }
        }
        self.text.push(if self.en {"Try one change at a time. Compare an open, accessible spot with the original layout and check the route, doors and clearance. If the open layout also fails, revisit the station's own requirements and current task."}
            else {"一次只改一个条件：在开阔处试同一设施，对照原先的通道、门和空间。若开阔处也不行，再核对设施本身的岗位要求和当前任务。"} .into());
        Some(())
    }
}

pub(super) fn answer(data: &Data, q: &str) -> Option<AssistantReply> {
    if let Some(reply) = interaction::answer(data,q) { return Some(reply); }
    if let Some(reply) = zoning::answer(data, q) { return Some(reply); }
    let problem = has(
        q,
        &[
            "won't",
            "won’t",
            "wont ",
            "can't",
            "can’t",
            "cant ",
            "cannot",
            "unable to assign",
            "unable to work",
            "stuck",
            "idle",
            "not working",
            "none of",
            "cannot reach",
            "unreachable",
            "unreliable",
            "unreliably",
            "how to assign",
            "how do i assign",
            "how can i assign",
            "怎么派工",
            "派不上",
            "指派不了",
            "派不了",
            "分配不了",
            "不接活",
            "不肯干",
            "怎么分配工位",
            "无法",
            "卡住",
            "闲置",
            "不干活",
            "不工作",
            "不肯",
            "不去",
            "过不去",
            "不采",
            "不砍",
            "不动",
        ],
    );
    let worker = has(
        q,
        &[
            "帕鲁",
            "工人",
            "据点",
            "基地",
            "pal ",
            "pals ",
            "pal's ",
            "pal’s ",
            "worker",
            "base ",
            "assignment",
            "handiwork",
            "handywork",
            "派工",
        ],
    );
    let natural = has(
        q,
        &[
            "natural tree",
            "natural ore",
            "natural rock",
            "natural mineral",
            "天然树",
            "自然树",
            "天然矿",
            "野生矿",
            "矿点",
            "天然石",
            "树木",
        ],
    );
    let health = has(
        q,
        &[
            "medic", "heal", "curing", "cure", "治病", "药品", "药箱", "治疗", "生病",
        ],
    ) && has(
        q,
        &[
            "cabinet",
            "rack",
            "chest",
            "clinic",
            "box",
            "自助",
            "取药",
            "药柜",
            "药品柜",
            "诊疗所",
            "箱",
        ],
    );
    let goal = has(
        q,
        &[
            "how", "help", "set up", "setup", "balanc", "optimiz", "organis", "organiz", "arrange",
            "怎么", "如何", "搭配", "规划", "安排", "优化", "改善", "开局", "自动", "同时", "分工",
        ],
    );
    let raising = has(q, &["breed", "配种", "繁育", "繁殖"]);
    let ranch = has(q, &["ranch", "放牧", "养殖", "家畜", "牧场产出"]);
    let food = has(
        q,
        &[
            "base food",
            "food at base",
            "food for my base",
            "feed my workers",
            "worker food",
            "food production",
            "auto farm",
            "automatically plant",
            "farm setup",
            "据点供餐",
            "基地供餐",
            "据点伙食",
            "基地伙食",
            "自动种菜",
            "自动种田",
            "农园分工",
            "自动耕作",
            "供餐",
        ],
    );
    let feeding = worker
        && has(
            q,
            &[
                "feed box",
                "feeding box",
                "won't eat",
                "won’t eat",
                "not eating",
                "饲料箱",
                "饲料盒",
                "不肯吃",
                "不吃饭",
                "不进食",
            ],
        )
        && has(
            q,
            &[
                "hungry",
                "starving",
                "won't",
                "won’t",
                "not eating",
                "can't reach",
                "饥饿",
                "饿",
                "不肯",
                "不吃",
                "不进食",
                "够不到",
            ],
        );
    let mut plan = Plan {
        data,
        en: !q.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)),
        text: vec![],
        links: vec![],
    };
    if health
        && worker
        && !has(
            q,
            &[
                "多少材料",
                "怎么建造",
                "how to build",
                "build cost",
                "crafting cost",
                "recipe cost",
            ],
        )
    {
        plan.health()?;
    }
    if goal && ranch && raising {
        plan.ranch_and_breed()?;
    }
    if goal && food {
        plan.food(q)?;
    }
    if feeding {
        let feed = plan.station("Feed_Box")?;
        plan.text.push(if plan.en {format!("Put edible food in the {feed} used by this base. Try one hungry worker with a stocked box in the open at ground level. If it eats there, compare the original box’s access route. If several workers still cannot eat, verify the box, food supply and that the Pals are assigned to this base.")}
            else {format!("把可食用的食物放进这个据点的{feed}。先让一只饥饿的工人在平地开阔处试吃装有食物的箱子；如果能吃，再检查原箱通道。如果多只工人都不吃，核对箱子类型、存粮和它们所属的据点。")});
    }
    let vertical = has(
        q,
        &[
            "stairs",
            "staircase",
            "upstairs",
            "ramp",
            "upper floor",
            "second floor",
            "upper plots",
            "楼梯",
            "楼上",
            "上楼",
            "二楼",
            "上层",
            "坡道",
            "多层",
        ],
    );
    let upstairs_job = vertical
        && goal
        && has(
            q,
            &[
                "farm",
                "craft",
                "working",
                "work station",
                "workstation",
                "furnace",
                "assignment",
                "work at",
                "work on",
                "go to work",
                "干活",
                "工作",
                "种植",
                "农田",
                "采矿",
                "工位",
                "派工",
            ],
        );
    let restrict = has(
        q,
        &[
            "only",
            "nothing except",
            "专职",
            "只做",
            "只干",
            "只负责",
            "只搬",
            "只让",
            "只派",
        ],
    ) && has(
        q,
        &[
            "assign",
            "set ",
            "make ",
            "to do nothing except",
            "only do",
            "only work on",
            "only handle",
            "只做",
            "只干",
            "只负责",
            "只搬",
            "只让",
            "专职",
        ],
    );
    if restrict
        && worker
        && has(
            q,
            &[
                "base", "pal ", "pals", "pal's", "pal’s", "据点", "基地", "帕鲁",
            ],
        )
        && !has(
            q,
            &[
                "recommend",
                "which species",
                "which pals",
                "can only",
                "why can",
                "为什么只",
                "只能",
                "worker thread",
                "哪些帕鲁",
                "哪只帕鲁",
            ],
        )
    {
        let jobs = JOBS
            .iter()
            .filter(|(_, _, words)| {
                words
                    .iter()
                    .any(|word| *word != "carrying capacity" && q.contains(word))
            })
            .collect::<Vec<_>>();
        if let [job] = jobs.as_slice() {
            let monitor = plan.station("Monitoring_Stand")?;
            let name = job_name(job.0, plan.en);
            plan.text.push(if plan.en {format!("Deploy a worker with {name} at this base. At {monitor}, open the work suitability permissions for that individual, leave {name} on and turn its other work types off. Clear any fixed assignment that ties it to a different station. When no {name} work is available it may wait.")}
                else {format!("派一只有{name}适性的帕鲁到此据点。在{monitor}打开这只工人的工种权限，只保留{name}，关闭其他工种，并解除它原先冲突的固定岗位。没有{name}任务时它可能闲置。")});
        }
    }
    let assigning = has(
        q,
        &["assign", "派工", "指派", "派不上", "分配不了", "派不了"],
    );
    let construction = has(
        q,
        &[
            "can't build",
            "cant build",
            "cannot build",
            "can't place",
            "cannot place",
            "建不了",
            "无法建造",
            "造不出来",
            "造不了",
            "放不下",
        ],
    );
    if (worker || assigning) && (problem || upstairs_job) && !construction {
        let stations = workstations(data, q);
        let worksite = !stations.is_empty()
            || has(
                q,
                &[
                    "workstation",
                    "work station",
                    "fixed assignment",
                    "工位",
                    "固定岗位",
                    "固定派工",
                ],
            );
        if natural || ((worksite || vertical) && !feeding && !health) {
            plan.diagnose(q, natural, vertical, &stations)?;
        }
    }
    (!plan.text.is_empty()).then(|| AssistantReply {
        text: plan.text.join("\n"),
        links: plan.links,
        suggestions: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant_engine::Conversation;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn base_diagnostics_and_production_plans_use_the_right_records() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut d = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for table in ["facilities-data", "ranch-data", "item-catalog-data"] {
            let source = std::fs::read(root.join(format!("{table}.json"))).unwrap();
            d.tables.insert(
                table.into(),
                Arc::new(serde_json::from_slice(&source).unwrap()),
            );
        }
        for (q, needles, route) in [
            (
                "两座基地的用途怎么分配，才方便做饭和开矿？",
                vec!["第一处", "第二处", "公会箱", "采矿"],
                "#work/facilities/Ore_Mining_Site",
            ),
            (
                "How should I specialize three bases for my guild?",
                vec!["Base 1", "Base 2", "Base 3", "Guild Chest"],
                "#work/facilities/Guild_Chest",
            ),
            (
                "Can I farm sulfur and quartz together at a single base?",
                vec!["Sulfur Mine (Lv.46)", "Pure Quartz Quarry (Lv.52)", "natural nodes"],
                "#work/facilities/Pure_Quartz_Quarry",
            ),
            (
                "Can I craft a Repair Bench at my camp? It is missing from the build wheel.",
                vec!["technology level 2", "Wood × 10", "Build menu"],
                "#work/facilities/Repair_Bench",
            ),
            (
                "想给基地换个布局，料理锅摆错了要挪到靠墙的一边。",
                vec!["取走", "建造模式", "重建"],
                "#work/facilities/Cooking_Pot",
            ),
            (
                "At the mill, how do I start making flour and what work does the assigned Pal need?",
                vec!["Wheat × 3", "Flour × 1", "Watering Lv.1"],
                "#work/facilities/Mill",
            ),
            (
                "据点里的工人不砍天然树，我应该怎么排查？",
                vec!["天然树", "伐木", "开阔处"],
                "#work/facilities/Logging_Site",
            ),
            (
                "My worker is idle at the Stone Pit; how can I isolate the problem?",
                vec!["Mining Lv.1", "one change at a time"],
                "#work/facilities/Stone_Pit",
            ),
            (
                "My partner Pals wont work in this breeding pen. What should I check?",
                vec![
                    "one male and one female",
                    "Cake",
                    "no required work suitability",
                ],
                "#work/facilities/Breeding_Farm",
            ),
            (
                "我指派不了工人到钓鱼池，怎么开工？",
                vec!["手工作业 Lv.1", "操作箱", "鱼饵", "启动"],
                "#work/facilities/Fishing_Pond",
            ),
            (
                "My pal's cant work at either the wheat farm or the logging site.",
                vec!["Planting → Watering → Gathering", "Lumbering Lv.1"],
                "#work/facilities/Wheat_Plantation",
            ),
            (
                "Can I set a base worker to do nothing except mining?",
                vec!["Mining on", "other work types off", "may wait"],
                "#work/facilities/Monitoring_Stand",
            ),
            (
                "My base workers cannot reach a furnace on the second floor. How can I test the ramp?",
                vec!["both floors", "headroom", "ground level"],
                "#work/facilities/Monitoring_Stand",
            ),
            (
                "如何让据点工人上楼，到二楼的工位工作？",
                vec!["楼梯", "头顶空间", "一层"],
                "#work/facilities/Monitoring_Stand",
            ),
            (
                "据点里生病的工人会自己去药箱取药吗？",
                vec!["药品柜", "诊疗所", "饲料箱"],
                "#work/facilities/Medicine_Rack",
            ),
            (
                "为什么据点工人会饿着却不肯去饲料箱吃饭？",
                vec!["开阔处试吃", "原箱通道"],
                "#work/facilities/Feed_Box",
            ),
            (
                "How do I balance ranching and breeding with my existing outposts?",
                vec!["Egg", "Flour × 5", "Honey × 2", "Breeding Farm (Lv.19)"],
                "#items/Cake",
            ),
            (
                "据点供餐怎么从开局一直安排到后期？",
                vec!["烤野莓", "番茄", "生菜", "采集"],
                "#items/Salad",
            ),
        ] {
            let mut conversation = Conversation::default();
            let result = conversation.ask_world(q, &d, &Value::Null);
            assert!(
                conversation.direct_result() && needles.iter().all(|s| result.text.contains(s)),
                "{q}: {}",
                result.text
            );
            assert!(result.links.iter().any(|(_, url)| url == route));
        }
        assert!(answer(&d, "best pals to assign to mining").is_none());
        assert!(answer(&d, "my worker is idle at the crude oil extractor").is_none());
        assert!(answer(&d, "据点药品柜建造要多少材料？").is_none());
        assert!(answer(&d, "can you explain my pc cpu idle percentage?").is_none());
        assert!(answer(&d, "how can pals attack enemies upstairs?").is_none());
        assert!(interaction::answer(&d, "how to make cakes at a cooking pot?").is_none());
        assert!(interaction::answer(&d, "how do i move food from my refrigerator to a feed box?").is_none());
        assert!(interaction::answer(&d, "can i make a crusher in terraria?").is_none());
        assert!(interaction::answer(&d, "i have no button prompt in a spreadsheet").is_none());
        assert!(interaction::answer(&d, "our base machine has no button prompt").unwrap().text.contains("Which facility"));
        assert!(interaction::answer(&d, "how to build a crusher in v0.7?").unwrap().text.contains("may differ"));
        assert!(answer(&d, "my pal can't catch a fish with this fishing rod").is_none());
        assert!(answer(&d, "can't build a breeding farm inside my base").is_none());
        assert!(answer(&d, "click to assign the worker thread on my pc").is_none());
        assert!(answer(&d, "why can a base pal only do mining?").is_none());
        assert!(answer(&d, "can I set a worker thread to only do mining?").is_none());
        assert!(
            answer(&d, "怎么让据点帕鲁只负责搬运？")
                .unwrap()
                .text
                .contains("只保留搬运")
        );
        let breeding = answer(&d, "据点帕鲁派不上配种场该查哪里").unwrap();
        assert!(!breeding.text.contains("启用这个工人"));
    }
}
