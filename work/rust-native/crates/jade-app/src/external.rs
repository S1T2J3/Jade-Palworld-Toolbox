//! The original optional online-entry directory. Indexing and searching are local;
//! only following a hyperlink opens the user's browser.
use crate::data::{Data, array, normalize, text};
use eframe::egui;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

const RELEASE_URL: &str = "https://github.com/S1T2J3/Jade-Palworld-Toolbox/releases";
const TABLES: [&str; 6] = [
    "item-catalog-data",
    "facilities-data",
    "journals-data",
    "ranch-data",
    "capture-data",
    "db",
];
const ORDER: [&str; 6] = [
    "团体战攻略",
    "物品资料",
    "设施资料",
    "牧场逐级产出表",
    "手记原文",
    "工具箱发布页",
];
const BATCH: usize = 48;

#[derive(Default)]
pub struct Directory {
    groups: Vec<Group>,
    query: String,
    signature: Option<[usize; 6]>,
    matched_query: Option<String>,
    matches: Vec<GroupMatches>,
    shown: HashMap<usize, usize>,
}
struct Group {
    section: String,
    name: String,
    rows: Vec<Entry>,
}
struct Entry {
    url: String,
    host: String,
    aliases: Vec<Alias>,
}
struct Alias {
    label: String,
    context: String,
    search: String,
}
struct GroupMatches {
    group: usize,
    rows: Vec<(usize, usize)>,
}
struct Sources<'a> {
    catalog: &'a Value,
    facilities: &'a Value,
    journals: &'a Value,
    ranch: &'a Value,
    capture: &'a Value,
    species: &'a [Value],
    release_url: &'a str,
}

impl Directory {
    pub fn show(&mut self, ui: &mut egui::Ui, d: &Data) {
        // CollapsingHeader does not invoke its body while closed. In particular,
        // no large table is accessed or cloned during ordinary settings visits.
        egui::CollapsingHeader::new("需要联网的条目")
            .id_salt("external-entry-directory")
            .show(ui, |ui| {
                if d.pending != 0 {
                    ui.label("正在准备本地链接目录…");
                    return;
                }
                let signature =
                    TABLES.map(|key| d.tables.get(key).map_or(0, |v| Arc::as_ptr(v) as usize));
                if self.signature != Some(signature) {
                    self.groups = build_groups(Sources {
                        catalog: d.get("item-catalog-data"),
                        facilities: d.get("facilities-data"),
                        journals: d.get("journals-data"),
                        ranch: d.get("ranch-data"),
                        capture: d.get("capture-data"),
                        species: &d.species,
                        release_url: RELEASE_URL,
                    });
                    self.signature = Some(signature);
                    self.matched_query = None;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("查找外部条目：名称、物品 ID 或网站")
                        .desired_width(ui.available_width().min(460.)),
                );
                let query = normalize(&self.query);
                if self.matched_query.as_ref() != Some(&query) {
                    self.matches = filter_groups(&self.groups, &query);
                    self.matched_query = Some(query.clone());
                    self.shown.clear();
                }
                let total: usize = self.matches.iter().map(|g| g.rows.len()).sum();
                ui.small(if total == 0 {
                    "没有匹配的外部链接。".into()
                } else {
                    format!(
                        "{} {total} 个分组链接",
                        if query.is_empty() { "共" } else { "找到" }
                    )
                });
                let mut previous_section = "";
                for found in &self.matches {
                    let group = &self.groups[found.group];
                    if previous_section != group.section {
                        ui.add_space(8.);
                        ui.strong(&group.section);
                        previous_section = &group.section;
                    }
                    egui::CollapsingHeader::new(format!(
                        "{} · {} 个链接",
                        group.name,
                        found.rows.len()
                    ))
                    .id_salt(("external-group", &group.name, !query.is_empty()))
                    .default_open(!query.is_empty())
                    .show(ui, |ui| {
                        let limit = *self.shown.entry(found.group).or_insert(BATCH);
                        for &(entry, alias) in found.rows.iter().take(limit) {
                            let entry = &group.rows[entry];
                            let alias = &entry.aliases[alias];
                            ui.vertical(|ui| {
                                ui.hyperlink_to(format!("{} ↗", alias.label), &entry.url)
                                    .on_hover_text(&entry.url);
                                ui.small(format!("{} · {}", alias.context, entry.host));
                            });
                            ui.add_space(5.);
                        }
                        if limit < found.rows.len()
                            && ui
                                .button(format!(
                                    "显示更多（{} / {}）",
                                    limit.min(found.rows.len()),
                                    found.rows.len()
                                ))
                                .clicked()
                        {
                            self.shown.insert(found.group, limit + BATCH);
                        }
                    });
                }
            });
    }
}

fn build_groups(input: Sources<'_>) -> Vec<Group> {
    let mut groups: BTreeMap<String, (String, BTreeMap<String, Entry>)> = BTreeMap::new();
    let mut store =
        |section: &str, name: &str, url: &str, label: &str, context: &str, keywords: &str| {
            let Some((url, host)) = https_url(url) else {
                return;
            };
            let group = groups
                .entry(name.into())
                .or_insert_with(|| (section.into(), BTreeMap::new()));
            let entry = group.1.entry(url.clone()).or_insert_with(|| Entry {
                url,
                host,
                aliases: vec![],
            });
            let search = normalize(&format!("{label} {context} {keywords}"));
            if !entry.aliases.iter().any(|a| a.search == search) {
                entry.aliases.push(Alias {
                    label: label.into(),
                    context: context.into(),
                    search,
                });
            }
        };
    let optional = "在线词条对照";
    for item in array(&input.catalog["items"]) {
        let slug = text(item, "slug");
        if !slug.is_empty() {
            store(
                optional,
                "物品资料",
                &format!("https://paldb.cc/cn/{slug}"),
                text(item, "name"),
                text(item, "id"),
                text(item, "en"),
            );
        }
    }
    for key in array(&input.capture["raidEggOnly"])
        .iter()
        .filter_map(Value::as_str)
    {
        if array(&input.capture["enemiesByPal"][key]).is_empty() {
            let name = input
                .species
                .iter()
                .find(|p| p["key"] == key)
                .map(|p| text(p, "zh"))
                .unwrap_or(key);
            store(
                "获取与战斗资料",
                "团体战攻略",
                "https://palpedia.com/guides/raid-bosses",
                "团体战头目攻略",
                "召唤、胜利奖励及孵化",
                &format!("{name} {key}"),
            );
        }
    }
    for journal in array(&input.journals["catalog"]) {
        store(
            optional,
            "手记原文",
            text(journal, "source"),
            text(journal, "title"),
            text(journal, "group"),
            text(journal, "id"),
        );
    }
    for facility in array(&input.facilities["facilities"]) {
        let id = text(facility, "id");
        if !id.is_empty() {
            store(
                optional,
                "设施资料",
                &format!("https://paldb.cc/cn/{}", encode_component(id)),
                text(facility, "name"),
                text(facility, "group"),
                id,
            );
        }
    }
    if let Some(pals) = input.ranch["pals"].as_object() {
        for (key, pal) in pals {
            let name = input
                .species
                .iter()
                .find(|p| p["key"] == *key)
                .map(|p| text(p, "zh"))
                .unwrap_or(key);
            store(
                optional,
                "牧场逐级产出表",
                text(pal, "source"),
                name,
                "放牧产物",
                key,
            );
        }
    }
    if !input.release_url.is_empty() {
        store(
            "软件更新",
            "工具箱发布页",
            input.release_url,
            "Jade 幻兽帕鲁工具箱",
            "有仓库访问权限的账号可查看",
            "GitHub 公告 下载 更新",
        );
    }
    ORDER
        .into_iter()
        .filter_map(|name| {
            let (section, rows) = groups.remove(name)?;
            let mut rows: Vec<_> = rows.into_values().collect();
            rows.sort_by(|a, b| a.aliases[0].label.cmp(&b.aliases[0].label));
            Some(Group {
                section,
                name: name.into(),
                rows,
            })
        })
        .collect()
}

fn filter_groups(groups: &[Group], query: &str) -> Vec<GroupMatches> {
    let tokens: Vec<_> = query.split_whitespace().collect();
    groups
        .iter()
        .enumerate()
        .filter_map(|(group, value)| {
            let rows: Vec<_> = value
                .rows
                .iter()
                .enumerate()
                .filter_map(|(entry, value)| {
                    let common = normalize(&format!("{} {}", groups[group].name, value.host));
                    value
                        .aliases
                        .iter()
                        .position(|a| {
                            tokens
                                .iter()
                                .all(|token| a.search.contains(token) || common.contains(token))
                        })
                        .map(|alias| (entry, alias))
                })
                .collect();
            (!rows.is_empty()).then_some(GroupMatches { group, rows })
        })
        .collect()
}

fn encode_component(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn https_url(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim();
    let (scheme, rest) = raw.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") || rest.chars().any(|c| c.is_control() || c == '\\') {
        return None;
    }
    let (without_fragment, fragment) = rest
        .split_once('#')
        .map_or((rest, None), |(u, f)| (u, Some(f)));
    let uri: ureq::http::Uri = format!("https://{without_fragment}").parse().ok()?;
    let authority = uri.authority()?;
    if authority.as_str().contains('@') {
        return None;
    }
    let host = uri.host()?.to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    let authority = if uri.port_u16() == Some(443) {
        host.clone()
    } else {
        authority.as_str().to_ascii_lowercase()
    };
    let path = uri.path_and_query().map_or("/", |p| p.as_str());
    let mut url = format!("https://{authority}{path}");
    if let Some(fragment) = fragment {
        url.push('#');
        url.push_str(fragment);
    }
    Some((url, host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn directory_keeps_searchable_aliases_but_deduplicates_urls_per_group() {
        let groups = build_groups(Sources {
            catalog: &json!({"items":[
                {"name":"金属头盔","id":"Helmet_1","en":"Metal Helm","slug":"Metal_Helm"},
                {"name":"传说金属头盔","id":"Helmet_5","en":"Legendary Metal Helm","slug":"Metal_Helm"},
                {"name":"金属头盔","id":"Helmet_1","en":"Metal Helm","slug":"Metal_Helm"}
            ],"sources":[["只供引用","https://reference.example/source"]]}),
            facilities: &json!({"facilities":[{"id":"Table A/II","name":"作业台","group":"制作"}]}),
            journals: &json!({"catalog":[{"source":"https://wiki.example/day/","title":"第一天","group":"漂流者","id":"Day1"},{"source":"javascript:bad","title":"坏地址"}]}),
            ranch: &json!({"pals":{"SheepBall":{"source":"https://paldb.cc/cn/Lamball"}}}),
            capture: &json!({"raidEggOnly":["RaidOnly","AlreadyLocal"],"enemiesByPal":{"AlreadyLocal":[["RAID_A","已有本地详情"]]}}),
            species: &[
                json!({"key":"SheepBall","zh":"棉悠悠"}),
                json!({"key":"RaidOnly","zh":"只可孵化"}),
            ],
            release_url: RELEASE_URL,
        });
        assert_eq!(groups.len(), 6);
        let items = groups.iter().find(|g| g.name == "物品资料").unwrap();
        assert_eq!(items.rows.len(), 1);
        assert_eq!(items.rows[0].aliases.len(), 2);
        let found = filter_groups(&groups, &normalize("ＨＥＬＭＥＴ＿５ paldb.cc"));
        assert_eq!(found.len(), 1);
        let row = &found[0];
        assert_eq!(
            groups[row.group].rows[row.rows[0].0].aliases[row.rows[0].1].label,
            "传说金属头盔"
        );
        assert!(!filter_groups(&groups, "牧场 棉悠悠").is_empty());
        assert!(!filter_groups(&groups, "wiki.example day1").is_empty());
        assert!(filter_groups(&groups, "只供引用").is_empty());
        assert!(filter_groups(&groups, "alreadylocal").is_empty());
        assert!(
            groups.iter().find(|g| g.name == "设施资料").unwrap().rows[0]
                .url
                .ends_with("Table%20A%2FII")
        );
        assert!(
            groups
                .iter()
                .flat_map(|g| &g.rows)
                .all(|e| e.url.starts_with("https://"))
        );
    }

    #[test]
    fn only_valid_https_targets_are_kept_and_host_variants_normalize() {
        assert_eq!(
            https_url("HTTPS://PALDB.CC:443/cn/A"),
            Some(("https://paldb.cc/cn/A".into(), "paldb.cc".into()))
        );
        assert_eq!(
            https_url("https://example.com"),
            Some(("https://example.com/".into(), "example.com".into()))
        );
        assert!(
            https_url("https://example.com/a#section")
                .unwrap()
                .0
                .ends_with("#section")
        );
        for invalid in [
            "http://example.com",
            "file:///a",
            "javascript:alert(1)",
            "//example.com",
            "https://",
            "https://user@example.com/a",
            "https://example.com\\bad",
            "https://example.com/\nspoof",
        ] {
            assert!(https_url(invalid).is_none(), "{invalid:?}");
        }
    }
}
