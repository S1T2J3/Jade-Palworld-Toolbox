//! Base edition: answer a single handbook lookup from the shipped data.
use crate::assistant_search::SearchBox;
use crate::data::{Data, array, normalize, text};
use crate::{design, navigation, store::Store};
use eframe::egui::{self, RichText};
use serde_json::Value;

const EXAMPLES: &[&str] = &["蛋糕怎么做？", "阿努比斯是什么属性？", "伯乐眼镜有什么用？"];

#[derive(Clone, Copy, PartialEq)]
enum Lookup {
    Overview,
    Recipe,
    Obtain,
    Element,
    Work,
    Partner,
}

fn parse(question: &str) -> (Lookup, &str) {
    let trim = |c: char| c.is_whitespace() || "？?！!。，,：:".contains(c);
    let mut q = question.trim_matches(trim);
    q = q.strip_prefix("请问").unwrap_or(q).trim_matches(trim);
    q = q.trim_end_matches(['呢', '呀', '啊']).trim_matches(trim);
    for (kind, words) in [
        (
            Lookup::Recipe,
            &[
                "的制作配方是什么",
                "需要什么材料",
                "怎么合成",
                "怎么制作",
                "如何制作",
                "如何合成",
                "的配方",
                "怎么做",
                "配方",
            ][..],
        ),
        (
            Lookup::Obtain,
            &[
                "怎么获得",
                "如何获得",
                "获取方式",
                "哪里获得",
                "怎么获取",
                "哪里获取",
                "哪里掉落",
                "怎么采集",
                "怎么弄",
                "哪里有",
            ][..],
        ),
        (
            Lookup::Element,
            &["是什么属性", "属于什么属性", "什么属性", "的属性"][..],
        ),
        (
            Lookup::Work,
            &[
                "工作适应性是什么",
                "工作适性是什么",
                "适合什么工作",
                "会什么工作",
                "能干什么活",
                "工作适应性",
                "工作适性",
            ][..],
        ),
        (
            Lookup::Partner,
            &["伙伴技能是什么", "的伙伴技能", "伙伴技能"][..],
        ),
        (
            Lookup::Overview,
            &[
                "有什么效果",
                "有什么作用",
                "是干什么用的",
                "有什么用",
                "是什么",
                "的效果",
                "介绍一下",
                "介绍",
                "查询",
                "查看",
            ][..],
        ),
    ] {
        for word in words {
            if let Some(name) = q.strip_suffix(word).or_else(|| q.strip_prefix(word)) {
                return (kind, name.trim_matches(trim).trim_end_matches('的').trim());
            }
        }
    }
    (Lookup::Overview, q)
}

#[derive(Default)]
struct Reply {
    body: String,
    links: Vec<(String, String)>,
}

fn message(body: impl Into<String>) -> Reply {
    Reply {
        body: body.into(),
        links: Vec::new(),
    }
}

fn find_entries<'a>(data: &'a Data, name: &str, lookup: Lookup) -> Vec<&'a Value> {
    let target = normalize(name);
    if target.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for row in array(data.get("search-index-data")) {
        let kind = text(row, "kind");
        let priority = match (lookup, kind) {
            (Lookup::Obtain, "野外采集") => 0,
            (Lookup::Obtain | Lookup::Recipe, "物品") => 1,
            (Lookup::Element | Lookup::Work | Lookup::Partner, "帕鲁") => 0,
            (Lookup::Partner, "伙伴技能") => 1,
            (Lookup::Overview, "物品" | "帕鲁" | "被动词条" | "主动技能" | "伙伴技能") => {
                1
            }
            (Lookup::Overview, _) => 2,
            _ => continue,
        };
        let is_alias =
            if text(row, "name").eq_ignore_ascii_case(&target) || text(row, "name") == name {
                false
            } else if array(&row["aliases"])
                .iter()
                .filter_map(Value::as_str)
                .any(|s| s.eq_ignore_ascii_case(&target))
            {
                true
            } else {
                continue;
            };
        matches.push(((is_alias, priority), row));
    }
    matches.sort_by_key(|(score, _)| *score);
    if let Some((best, _)) = matches.first() {
        let best = *best;
        matches
            .into_iter()
            .take_while(|(score, _)| *score == best)
            .map(|(_, row)| row)
            .collect()
    } else {
        Vec::new()
    }
}

fn item_answer(data: &Data, id: &str, lookup: Lookup) -> String {
    let Some(item) = array(&data.get("item-catalog-data")["items"])
        .iter()
        .find(|v| text(v, "id") == id)
    else {
        return "当前图鉴缺少此物品的资料。".into();
    };
    let name = text(item, "name");
    if lookup == Lookup::Recipe {
        let recipes = array(&item["recipes"]);
        let Some(recipe) = recipes.first() else {
            return format!("物品图鉴没有收录{name}的制作配方。");
        };
        let ingredients: Vec<_> = array(&recipe["materials"])
            .iter()
            .filter_map(|v| {
                Some(format!(
                    "{} × {}",
                    data.item_name(v[0].as_str()?),
                    v[1].as_u64()?
                ))
            })
            .collect();
        if ingredients.is_empty() {
            return format!("当前配方没有收录{name}的材料数量。");
        }
        let mut answer = format!(
            "制作一批{name}（{} 份）需要{}。",
            recipe["qty"].as_u64().unwrap_or(1),
            ingredients.join("、")
        );
        let stations: Vec<_> = array(&recipe["stations"])
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if !stations.is_empty() {
            answer.push_str(&format!("\n可用设施有{}。", stations.join("、")));
        }
        if recipes.len() > 1 {
            answer.push_str(" 打开物品图鉴可查看其他配方。");
        }
        return answer;
    }
    if lookup == Lookup::Obtain {
        let mut methods: Vec<_> = array(&item["methods"])
            .iter()
            .filter_map(|v| Some(format!("{} · {}", v[0].as_str()?, v[1].as_str()?)))
            .take(5)
            .collect();
        if !array(&item["recipes"]).is_empty() {
            methods.insert(0, "制作（可在物品图鉴查看配方）".into());
        }
        return if methods.is_empty() {
            format!("物品图鉴没有收录{name}的获取途径。")
        } else {
            format!("{name}的图鉴来源包括\n{}", methods.join("\n"))
        };
    }
    let effects = &data.get("item-effects-data")["items"][id];
    let mut lines = vec![format!("{name} · {}", text(item, "category"))];
    if !text(effects, "description").is_empty() {
        lines.push(text(effects, "description").into());
    }
    lines.extend(
        array(&effects["effects"])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned),
    );
    lines.join("\n")
}

fn answer(data: &Data, question: &str) -> Reply {
    if data.pending > 0
        && [
            "search-index-data",
            "item-catalog-data",
            "item-effects-data",
            "partner-skills-data",
            "spawn-data",
            "combat-guide-data",
            "resource-guides-data",
        ]
        .iter()
        .any(|key| !data.ready(key))
    {
        return message("图鉴正在载入，请稍后再发送这条问题。");
    }
    let (lookup, name) = parse(question);
    let matches = find_entries(data, name, lookup);
    let Some(row) = matches.first() else {
        return message(
            "基础问答可以查单个帕鲁、物品、词条或技能。例如“蛋糕怎么做”或“阿努比斯的伙伴技能”。",
        );
    };
    let mut reply = Reply::default();
    for result in matches.iter().take(8) {
        let label = format!("{} · {}", text(result, "name"), text(result, "kind"));
        reply.links.push((label, text(result, "href").to_owned()));
    }
    if matches.len() > 1 {
        reply.body = "图鉴中有多个匹配条目，请选择要查看的对象。".into();
        return reply;
    }
    let name = text(row, "name");
    let id = text(row, "href").rsplit('/').next().unwrap_or("");
    reply.body = match text(row, "kind") {
        "物品" => item_answer(data, id, lookup),
        "野外采集" => array(&data.get("resource-guides-data")["gathering"])
            .iter()
            .find(|v| text(v, "id") == id)
            .map(|v| text(v, "note"))
            .unwrap_or("")
            .to_owned(),
        "伙伴技能" => text(&data.get("partner-skills-data")[id], "effect").into(),
        "帕鲁" => {
            let profile = &data.get("spawn-data")["profiles"][id];
            let elements = &data.get("combat-guide-data")["elements"];
            let mut lines = Vec::new();
            if matches!(lookup, Lookup::Element | Lookup::Overview)
                && let Some(pal) = data.pal(id)
            {
                let kinds: Vec<_> = array(&pal["types"])
                    .iter()
                    .filter_map(|v| elements[v.as_str()?].as_str())
                    .collect();
                if !kinds.is_empty() {
                    lines.push(format!("{name}是{}属性帕鲁。", kinds.join("、")));
                }
            }
            if matches!(lookup, Lookup::Work | Lookup::Overview) {
                let jobs: Vec<_> = array(&profile["work"])
                    .iter()
                    .filter_map(|v| Some(format!("{} Lv.{}", v[0].as_str()?, v[1].as_u64()?)))
                    .collect();
                if !jobs.is_empty() {
                    lines.push(format!("工作适性有{}。", jobs.join("、")));
                }
            }
            if matches!(lookup, Lookup::Partner | Lookup::Overview)
                && !text(profile, "partner").is_empty()
            {
                lines.push(format!("伙伴技能为「{}」。", text(profile, "partner")));
                if lookup == Lookup::Partner {
                    lines.push(text(&data.get("partner-skills-data")[id], "effect").into());
                }
            }
            lines.join("\n")
        }
        _ => text(row, "hint").to_owned(),
    };
    if reply.body.trim().is_empty() {
        reply.body = format!("请打开{name}的图鉴条目查看详情。");
    }
    reply
}

#[derive(Default)]
pub struct AssistantView {
    world_id: String,
    turns: Vec<(String, Reply)>,
}

impl AssistantView {
    pub fn reset_session(&mut self) {
        *self = Self::default();
    }

    fn select_world(&mut self, id: &str) {
        if self.world_id != id {
            *self = Self {
                world_id: id.to_owned(),
                ..Default::default()
            };
        }
    }

    fn ask(&mut self, question: &str, data: &Data) {
        let question = question.trim();
        if !question.is_empty() {
            self.turns
                .push((question.to_owned(), answer(data, question)));
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        data: &Data,
        store: &Store,
        submitted: Option<String>,
        entry: &mut SearchBox,
    ) -> Option<String> {
        self.select_world(text(store.world(), "id"));
        let mut route = None;
        if let Some(question) = submitted {
            self.ask(&question, data);
        }
        ui.horizontal_wrapped(|ui| {
            ui.weak("随包图鉴 · 离线可用");
            if !self.turns.is_empty() && design::small_action(ui, "reset", "清空对话").clicked()
            {
                self.turns.clear();
                entry.clear_input();
            }
        });
        navigation::ScrollArea::vertical()
            .id_salt(("pal-basic-history", &self.world_id))
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.turns.is_empty() {
                    design::section(ui, "试着问");
                    for &example in EXAMPLES {
                        if ui.button(example).clicked() {
                            entry.clear_input();
                            self.ask(example, data);
                            ui.ctx().request_repaint();
                        }
                    }
                }
                for (q, reply) in &self.turns {
                    ui.add_space(6.);
                    ui.label(
                        RichText::new(format!("你　{q}"))
                            .strong()
                            .color(design::palette(ui.ctx()).accent),
                    );
                    design::card(ui).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(&reply.body);
                        ui.horizontal_wrapped(|ui| {
                            for (label, href) in &reply.links {
                                if ui.link(label).clicked() {
                                    route = Some(href.to_owned());
                                }
                            }
                        });
                    });
                }
            });
        route
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handbook() -> Data {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root, egui::Context::default()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while data.pending > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            data.poll();
        }
        assert_eq!(data.pending, 0);
        data
    }

    #[test]
    fn base_questions_resolve_bundled_handbook_entries() {
        let data = handbook();
        for (question, expected, href) in [
            ("蛋糕怎么做？", "蜂蜜 × 2", "#items/Cake"),
            ("请问如何制作蛋糕？", "面粉 × 5", "#items/Cake"),
            (
                "帕鲁矿碎块怎么采集？",
                "普通岩石",
                "#guide/gathering/Pal_crystal_S",
            ),
            (
                "伯乐眼镜有什么用？",
                "潜力",
                "#items/Accessory_TalentChecker",
            ),
            ("阿努比斯是什么属性？", "地属性", "#paldex/Anubis"),
            ("Anubis的伙伴技能", "沙漠守护神", "#paldex/Anubis"),
            (
                "工匠精神的效果",
                "工作速度",
                "#details/passive/trait/CraftSpeed_up2",
            ),
        ] {
            let reply = answer(&data, question);
            assert!(reply.body.contains(expected), "{question}: {}", reply.body);
            assert!(reply.links.iter().any(|(_, r)| r == href), "{question}");
        }
        assert!(
            answer(&data, "我仓库里哪两只能配出阿努比斯")
                .links
                .is_empty()
        );
    }

    #[test]
    fn basic_page_sends_prefill_and_renders_the_recipe_and_source() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let data = handbook();
        let profile = crate::store::tests::Temp::new();
        let store = Store::open(profile.0.clone());
        let mut view = AssistantView::default();
        let mut entry = SearchBox::default();
        entry.prefill("蛋糕怎么做？".into());
        let frame = |view: &mut AssistantView, entry: &mut SearchBox, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(820., 620.),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let submitted = match entry.ui(ui, &data, true) {
                            Some(crate::assistant_search::SearchAction::Ask(question)) => {
                                Some(question)
                            }
                            Some(crate::assistant_search::SearchAction::Open(route)) => {
                                panic!("unexpected navigation: {route}")
                            }
                            None => None,
                        };
                        view.ui(ui, &data, &store, submitted, entry);
                    });
                },
            )
        };
        frame(&mut view, &mut entry, vec![]);
        frame(
            &mut view,
            &mut entry,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(view.turns.len(), 1);
        let render = frame(&mut view, &mut entry, vec![]);
        assert_eq!(view.turns.len(), 1);
        let visible: Vec<&str> = render
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(visible.iter().any(|line| line.contains("蜂蜜 × 2")));
        assert!(visible.contains(&"蛋糕 · 物品"));
        view.select_world("another-world");
        assert!(view.turns.is_empty());
        view.ask("工匠精神的效果", &data);
        view.reset_session();
        assert!(view.turns.is_empty());
        assert!(view.world_id.is_empty());
    }
}
