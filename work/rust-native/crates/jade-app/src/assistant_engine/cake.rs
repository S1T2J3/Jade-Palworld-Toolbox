use super::{AssistantReply, any};
use crate::assistant_data::{
    counted_craft, named_recipe, recipe_query, scaled_recipe, warehouse_work,
};
use crate::data::{Data, array, text};
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub(super) enum CakeSession {
    #[default]
    Inactive,
    CheckMaterials,
    CountedRecipe,
    Ingredient(String),
    Cooking,
}

impl CakeSession {
    pub(super) fn respond(
        &mut self,
        q: &str,
        data: &Data,
        world: &Value,
    ) -> Option<AssistantReply> {
        let named = q.contains("蛋糕");
        let count = counted_craft(q);
        let target = count
            .as_ref()
            .and_then(|n| n.for_item("蛋糕", !matches!(self, Self::Inactive)));
        if count.is_some() && target.is_none()
            || named
                && count.is_none()
                && named_recipe(array(&data.get("item-catalog-data")["items"]), q)
                    .is_some_and(|row| text(row, "id") != "Cake")
        {
            *self = Self::Inactive;
            return None;
        }
        let issue = any(
            q,
            &[
                "断",
                "停",
                "不够",
                "不出",
                "不足",
                "缺",
                "太慢",
                "跟不上",
                "不动",
                "不做",
                "没做",
                "不稳定",
                "产量",
                "没人",
                "没了",
                "用完",
            ],
        );
        let wants_recipe =
            named && (recipe_query(q) || any(q, &["材料", "原料", "怎么生产", "做法"]));
        if matches!(self, Self::Inactive) && !(named && issue || wants_recipe || target.is_some()) {
            return None;
        }
        if !named
            && any(
                q,
                &[
                    "矿石",
                    "金属锭",
                    "铁锭",
                    "矿锭",
                    "熔炉",
                    "矿炉",
                    "精炼炉",
                    "烧矿",
                    "采矿",
                    "发电",
                    "伐木",
                    "配种",
                ],
            )
        {
            *self = Self::Inactive;
            return None;
        }
        let owns = any(q, &["我仓库", "我的仓库", "仓库里", "我有哪", "我现有"]);
        let followup = q.trim_matches(|c: char| c.is_whitespace() || "，。！？?!".contains(c));
        let generic_warehouse = [
            "我仓库里有谁",
            "我仓库里有吗",
            "仓库里有吗",
            "我仓库里哪只适合",
            "我仓库里有哪些",
            "我仓库呢",
            "我的仓库呢",
            "我仓库里有几只",
            "我有哪几只",
        ]
        .contains(&followup.trim_start_matches('那'));
        let subject = q.replace("蛋糕", "");
        // An explicit shortage after a correction ("牛奶够了，但缺蜂蜜") takes precedence.
        let shortage = subject
            .split(['，', '。', '、', ',', '但'])
            .filter(|part| any(part, &["缺", "不够", "不足", "没了", "用完"]))
            .filter(|part| !any(part, &["不缺", "没缺"]))
            .next_back();
        let ingredient_text = shortage.unwrap_or(&subject);
        let ingredient = [
            ("Milk", &["牛奶", "缺奶"][..]),
            ("Egg", &["鸡蛋", "禽蛋", "鸟蛋", "蛋"]),
            ("Honey", &["蜂蜜"]),
            ("Flour", &["面粉"]),
            ("Berries", &["野莓", "莓果", "浆果", "红莓"]),
        ]
        .into_iter()
        .find(|(_, names)| any(ingredient_text, names))
        .map(|(id, _)| id);
        let all_stocked = any(
            q,
            &[
                "都齐",
                "齐了",
                "齐全",
                "齐备",
                "都有",
                "都够",
                "材料够",
                "原料够",
                "都不缺",
                "材料全",
                "原料全",
            ],
        );
        let cooking = all_stocked && shortage.is_none() && ingredient.is_none()
            || any(
                q,
                &[
                    "没排订单",
                    "没下单",
                    "有订单",
                    "没人做",
                    "没人来",
                    "锅不动",
                    "锅没动",
                    "没人烧",
                    "没人煮",
                ],
            );
        let short_answer =
            subject.trim_matches(|c: char| c.is_whitespace() || "，。！？?!".contains(c));
        let simple_ingredient = [
            "牛奶",
            "蜂蜜",
            "面粉",
            "鸡蛋",
            "蛋",
            "野莓",
            "红色野莓",
            "莓果",
            "浆果",
            "红莓",
        ]
        .contains(&short_answer);
        if !named && !issue && !simple_ingredient && !owns && !cooking && target.is_none() {
            *self = Self::Inactive;
            return None;
        }
        let cake = array(&data.get("item-catalog-data")["items"])
            .iter()
            .find(|v| text(v, "id") == "Cake");
        let Some((cake, recipe)) =
            cake.and_then(|item| Some((item, array(&item["recipes"]).first()?)))
        else {
            *self = Self::CheckMaterials;
            return Some(AssistantReply::new(
                "尚未读到本地蛋糕配方。资料载入后可再问一次。",
                &["蛋糕的配方是什么？"],
            ));
        };
        if target.is_some() || wants_recipe && !issue && !cooking {
            *self = if target.is_some() { Self::CountedRecipe } else { Self::CheckMaterials };
            let mut reply: AssistantReply = scaled_recipe(data, cake, target).into();
            reply.suggestions.push("材料都够，但锅不动".into());
            return Some(reply);
        }
        if cooking && !(shortage.is_some() && ingredient.is_some()) {
            *self = Self::Cooking;
            let stations = array(&recipe["stations"])
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、");
            let mut reply = AssistantReply::new(
                &format!(
                    "先看正在用的设施是否支持蛋糕。图鉴收录的设施有{stations}。确认排了蛋糕订单，再看有生火适性的帕鲁是否到锅前工作、制作进度是否增加。"
                ),
                &["我仓库里谁能生火？"],
            );
            reply
                .links
                .push(("打开蛋糕配方".into(), "#items/Cake".into()));
            return Some(reply);
        }
        if owns
            && matches!(self, Self::Cooking)
            && !named
            && ingredient.is_none()
            && (generic_warehouse || q.contains("生火"))
        {
            return Some(warehouse_work(data, world, "生火").into());
        }
        let prior = match self {
            Self::Ingredient(id) if generic_warehouse && !named => Some(id.as_str()),
            _ => None,
        };
        if let Some(id) = ingredient.or(prior) {
            let id = id.to_owned();
            let entry = array(&recipe["materials"])
                .iter()
                .find(|entry| entry[0] == id)?;
            let name = data.item_name(&id);
            let mut reply =
                AssistantReply::new(&format!("每份蛋糕需要{}份{name}。", entry[1]), &[]);
            reply
                .links
                .push((format!("查看{name}"), format!("#items/{id}")));
            if id == "Flour" {
                let flour = array(&data.get("item-catalog-data")["items"])
                    .iter()
                    .find(|v| text(v, "id") == id);
                if let Some(process) = flour.and_then(|v| array(&v["recipes"]).first()) {
                    let materials = array(&process["materials"])
                        .iter()
                        .filter_map(|m| {
                            Some(format!(
                                "{}份{}",
                                m[1].as_u64()?,
                                data.item_name(m[0].as_str()?)
                            ))
                        })
                        .collect::<Vec<_>>()
                        .join("、");
                    if let Some(station) =
                        array(&process["stations"]).first().and_then(Value::as_str)
                    {
                        reply.text.push_str(&format!("{station}每加工一份面粉需要{materials}。先核对原料，再确认面粉订单有进度。"));
                    }
                }
            } else if let Some(pals) = data.get("ranch-data")["pals"].as_object() {
                let inventory = array(&world["inventory"]["records"]);
                let mut suppliers = Vec::new();
                for (key, profile) in pals {
                    if array(&profile["tiers"])
                        .iter()
                        .flat_map(array)
                        .any(|row| row["item"] == id)
                    {
                        let count = inventory
                            .iter()
                            .filter(|record| text(record, "speciesKey") == key)
                            .count();
                        let pal = data.pal_name(key);
                        suppliers.push(if inventory.is_empty() {
                            pal
                        } else {
                            format!("{pal}（仓库 {count} 只）")
                        });
                    }
                }
                if !suppliers.is_empty() {
                    reply.text.push_str(&format!("能在牧场产出{name}的帕鲁有{}。先看牧场里有没有对应帕鲁，再看产物是否被搬进了据点箱子。", suppliers.join("、")));
                    if inventory.is_empty() {
                        reply.text.push_str("当前世界的仓库没有登记帕鲁。");
                    }
                    reply.links.push((
                        format!("查看{name}的放牧来源"),
                        format!("#work/ranch/item/{id}"),
                    ));
                }
            } else {
                reply
                    .text
                    .push_str("尚未读到本地牧场资料，载入后可再查放牧来源。");
            }
            *self = Self::Ingredient(id);
            return Some(reply);
        }
        if named && issue
            || matches!(self, Self::CheckMaterials | Self::CountedRecipe)
                && any(q, &["缺材料", "材料不够", "原料不足", "缺原料"])
        {
            *self = Self::CheckMaterials;
            return Some(AssistantReply::new(
                "蛋糕停产的时候，配方里缺哪一种材料？如果材料全够，请告诉我锅里有没有蛋糕订单。",
                &["总缺牛奶", "总缺蜂蜜", "材料都够，但锅不动"],
            ));
        }
        *self = Self::Inactive;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn cake_uses_published_recipe_ranch_tables_and_current_world() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut data = Data::new(root.clone(), eframe::egui::Context::default()).unwrap();
        for id in ["item-catalog-data", "ranch-data"] {
            let value =
                serde_json::from_slice(&std::fs::read(root.join(format!("{id}.json"))).unwrap())
                    .unwrap();
            data.tables.insert(id.into(), std::sync::Arc::new(value));
        }
        let world = json!({"inventory":{"records":[{"speciesKey":"SoldierBee"}]}});
        let mut session = CakeSession::default();
        assert!(
            session
                .respond("蛋糕总断产", &data, &world)
                .unwrap()
                .text
                .contains("缺哪一种")
        );
        for (q, source, id, amount) in [
            ("牛奶够了，但缺蜂蜜", "骑士蜂（仓库 1 只）", "Honey", 2),
            ("缺牛奶", "波霸牛（仓库 0 只）", "Milk", 7),
            ("鸡蛋", "皮皮鸡（仓库 0 只）", "Egg", 8),
        ] {
            let reply = session.respond(q, &data, &world).unwrap();
            assert!(reply.text.contains(source), "{}", reply.text);
            assert!(reply.text.contains(&format!("{amount}份")));
            assert!(
                reply
                    .links
                    .iter()
                    .any(|(_, link)| link == &format!("#work/ranch/item/{id}"))
            );
        }
        assert!(
            session
                .respond("材料都够，但锅不动", &data, &world)
                .unwrap()
                .text
                .contains("生火")
        );
        assert!(
            session
                .respond("我仓库里哪些能发电？", &data, &world)
                .is_none()
        );
        assert!(session.respond("蛋糕断产，缺蜂蜜", &data, &world).is_some());
        assert!(
            session
                .respond("我仓库里谁能采矿？", &data, &world)
                .is_none()
        );
        assert!(session.respond("蛋糕总断产", &data, &world).is_some());
        assert!(
            session
                .respond("熔炉有订单，金属锭还是不够", &data, &world)
                .is_none()
        );
        assert!(session.respond("蛋糕总断产", &data, &world).is_some());
        assert!(
            session
                .respond("帕鲁矿碎块怎么获得？", &data, &world)
                .is_none()
        );
        assert!(session.respond("蜂蜜", &data, &world).is_none());
    }
}
