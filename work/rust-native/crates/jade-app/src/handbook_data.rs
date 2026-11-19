//! Read-only handbook values used by both the native UI and its optional headless kernel.
use super::Data;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

static EMPTY: Value = Value::Null;

impl Data {
    pub fn get(&self, id: &str) -> &Value {
        self.tables.get(id).map(AsRef::as_ref).unwrap_or(&EMPTY)
    }
    pub fn ready(&self, id: &str) -> bool {
        self.tables.contains_key(id)
    }
    pub fn pal(&self, key: &str) -> Option<&Value> {
        self.species.iter().find(|p| text(p, "key") == key)
    }
    pub fn pal_name(&self, key: &str) -> String {
        self.pal(key)
            .map(|p| text(p, "zh"))
            .filter(|s| !s.is_empty())
            .unwrap_or(key)
            .to_owned()
    }
    pub fn item_name(&self, id: &str) -> String {
        self.get("item-catalog-data")["items"]
            .as_array()
            .and_then(|a| a.iter().find(|i| text(i, "id") == id))
            .map(|v| text(v, "name").to_owned())
            .unwrap_or_else(|| id.to_owned())
    }
    pub fn trait_name(&self, key: &str) -> String {
        self.get("trait-data")["traits"]
            .as_array()
            .and_then(|a| a.iter().find(|i| text(i, "key") == key))
            .map(|v| text(v, "zh").to_owned())
            .unwrap_or_else(|| key.to_owned())
    }
}

pub(super) fn validate_core_data(species: &[Value], pairs: &[Value]) -> Result<(), String> {
    if species.is_empty() {
        return Err("帕鲁资料为空或格式损坏，请重新解压完整的测试版程序。".into());
    }
    let mut keys = std::collections::HashSet::new();
    for pal in species {
        let key = text(pal, "key");
        if key.trim().is_empty() || !keys.insert(key) {
            return Err("帕鲁资料包含缺失或重复的编号，请重新解压完整的测试版程序。".into());
        }
    }
    if pairs.is_empty() {
        return Err("配种表为空，请重新解压完整的测试版程序。".into());
    }
    for (row, pair) in pairs.iter().enumerate() {
        let Some(values) = pair.as_array() else {
            return Err(format!("配种表第 {} 条记录格式损坏。", row + 1));
        };
        if ![3, 5].contains(&values.len())
            || values
                .iter()
                .take(3)
                .any(|v| v.as_u64().is_none_or(|index| index >= species.len() as u64))
        {
            return Err(format!(
                "配种表第 {} 条记录包含无效的帕鲁编号，请重新解压完整程序。",
                row + 1
            ));
        }
        if values.len() == 5
            && values
                .iter()
                .skip(3)
                .any(|v| !matches!(v.as_str(), Some("M" | "F")))
        {
            return Err(format!("配种表第 {} 条记录的性别条件无效。", row + 1));
        }
    }
    Ok(())
}
pub fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
pub fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
/// The same comparison key for full-width input, aliases, and mixed Latin case.
pub fn normalize(s: &str) -> String {
    s.nfkc().flat_map(char::to_lowercase).collect()
}
