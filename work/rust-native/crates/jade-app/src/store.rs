use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashSet},
    path::{Path, PathBuf},
};

pub struct Store {
    pub root: PathBuf,
    pub state: Value,
    pub error: Option<String>,
    baseline: Value,
    dirty: bool,
    write_locked: bool,
}

fn checked_text(v: &Value, max: usize, empty: bool) -> bool {
    v.as_str().is_some_and(|s| {
        (empty || !s.trim().is_empty())
            && s.chars().count() <= max
            && !s
                .chars()
                .any(|c| c == '\0' || (c.is_control() && c != '\n' && c != '\t'))
    })
}
fn unique_strings(v: &Value, max: usize) -> bool {
    let Some(values) = v.as_array() else {
        return false;
    };
    let mut seen = HashSet::new();
    values
        .iter()
        .all(|s| checked_text(s, max, false) && seen.insert(s.as_str().unwrap()))
}
pub fn validate_world(w: &Value) -> Result<(), String> {
    if !checked_text(&w["id"], 512, false) || !checked_text(&w["name"], 256, false) {
        return Err("世界的编号或名称无效。".into());
    }
    let mut ids = HashSet::new();
    let inv = &w["inventory"];
    let limit = inv["limit"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| inv["limit"].as_u64().map(|n| n.to_string()))
        .unwrap_or_default();
    if limit != "all" && !limit.parse::<usize>().is_ok_and(|n| n > 0 && n <= 10000) {
        return Err("仓库步数筛选无效。".into());
    }
    for field in ["records", "archived"] {
        let rows = inv[field].as_array().ok_or("仓库或归档格式不正确。")?;
        if rows.len() > 1_000_000 {
            return Err("仓库个体数量过多。".into());
        }
        for r in rows {
            if !checked_text(&r["id"], 1024, false)
                || !ids.insert(r["id"].as_str().unwrap())
                || !checked_text(&r["speciesKey"], 160, false)
                || !checked_text(&r["name"], 256, true)
                || !matches!(r["sex"].as_str(), Some("M" | "F" | "unknown"))
                || !unique_strings(&r["traits"], 160)
                || r["traits"].as_array().unwrap().len() > 4
            {
                return Err("仓库个体的编号、物种、名字、性别或词条无效。".into());
            }
            for owner_key in ["terminalOwnerId", "terminalOwner"] {
                if !r[owner_key].is_null() && !r[owner_key].as_str().is_some_and(valid_uuid) {
                    return Err("终端个体的玩家编号无效。".into());
                }
            }
            if !r["terminalSpeciesKey"].is_null()
                && !checked_text(&r["terminalSpeciesKey"], 160, false)
            {
                return Err("终端物种编号无效。".into());
            }
            for flag in ["legacy", "terminalArchived"] {
                if !r[flag].is_null() && !r[flag].is_boolean() {
                    return Err("个体的归档或迁移标记无效。".into());
                }
            }
            if field == "records" && r["terminalArchived"] == true {
                return Err("已失去的终端个体不能出现在可用仓库。".into());
            }
        }
    }
    if !inv["removed"].is_null() {
        if !unique_strings(&inv["removed"], 1024) {
            return Err("仓库删除记录无效。".into());
        }
        let active: HashSet<_> = inv["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|r| r["id"].as_str())
            .collect();
        if inv["removed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| active.contains(r.as_str().unwrap()))
        {
            return Err("同一个体同时存在于仓库与删除记录中。".into());
        }
    }
    if !unique_strings(&w["surgery"], 160) {
        return Err("手术台词条记录无效。".into());
    }
    let plans = w["plans"].as_array().ok_or("配种计划格式无效。")?;
    let mut plan_ids = HashSet::new();
    for p in plans {
        if !checked_text(&p["id"], 512, false)
            || !plan_ids.insert(p["id"].as_str().unwrap())
            || !checked_text(&p["targetKey"], 160, false)
            || !unique_strings(&p["traits"], 160)
            || p["traits"].as_array().unwrap().len() > 4
            || (!p["initialRoute"].is_null()
                && (!p["initialRoute"]["steps"].is_array()
                    || !unique_strings(&p["initialRoute"]["surgery"], 160)
                    || p["initialRoute"]["generations"].as_u64().is_none()))
            || !p["log"].is_array()
            || !p["random"].is_boolean()
            || !p["surgery"].is_boolean()
        {
            return Err("配种计划或孵化进度无效。".into());
        }
        for step in p["initialRoute"]["steps"].as_array().into_iter().flatten() {
            if !checked_text(&step["speciesKey"], 160, false)
                || !unique_strings(&step["traits"], 160)
                || !step["parents"].is_array()
                || step["generation"].as_u64().is_none()
            {
                return Err("配种路线中的步骤无效。".into());
            }
        }
        for event in p["log"]
            .as_array()
            .unwrap()
            .iter()
            .chain(p.get("pending").filter(|v| !v.is_null()))
        {
            if !event.is_object()
                || !checked_text(&event["id"], 512, false)
                || !checked_text(&event["speciesKey"], 160, false)
                || !matches!(event["sex"].as_str(), Some("M" | "F" | "unknown"))
                || !unique_strings(&event["traits"], 160)
            {
                return Err("孵化记录无效。".into());
            }
            if let Some(stock_id) = event["stockId"].as_str() {
                for r in inv["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .chain(inv["archived"].as_array().unwrap())
                {
                    if r["id"] == stock_id && r["speciesKey"] != event["speciesKey"] {
                        return Err("计划的孵化记录与仓库个体不匹配。".into());
                    }
                }
            }
        }
    }
    if !w["mapNotes"].is_null() {
        if !unique_strings(&w["mapNotes"]["completed"], 1024) {
            return Err("地图完成记录无效。".into());
        }
        let mut pins = HashSet::new();
        for p in w["mapNotes"]["custom"]
            .as_array()
            .ok_or("自定义地图点位无效。")?
        {
            if !p["id"]
                .as_str()
                .is_some_and(|s| s.starts_with("custom-") && pins.insert(s))
                || !checked_text(&p["name"], 80, false)
                || !checked_text(&p["note"], 600, true)
                || !matches!(p["map"].as_str(), Some("palpagos" | "worldtree"))
                || !p["done"].is_boolean()
                || !["u", "v"].iter().all(|k| {
                    p[k].as_f64()
                        .is_some_and(|v| v.is_finite() && (0.0..=1.0).contains(&v))
                })
            {
                return Err("自定义点位的名称、坐标或地图无效。".into());
            }
        }
    }
    Ok(())
}
pub fn valid_uuid(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()) && s.bytes().any(|b| b != b'0')
}
fn normalize_world(w: &mut Value) {
    if w["inventory"]["removed"].is_null() {
        w["inventory"]["removed"] = json!([]);
    }
    if w["mapNotes"].is_null() {
        w["mapNotes"] = json!({"completed":[],"custom":[]});
    }
    if let Some(n) = w["inventory"]["limit"].as_u64() {
        w["inventory"]["limit"] = json!(n.to_string());
    }
}
fn validate_worlds(v: &Value) -> Result<(), String> {
    let worlds = v["worlds"].as_array().ok_or("世界列表无效。")?;
    if worlds.is_empty() || worlds.len() > 1000 {
        return Err("世界数量无效。".into());
    }
    let mut ids = HashSet::new();
    for w in worlds {
        validate_world(w)?;
        if !ids.insert(w["id"].as_str().unwrap()) {
            return Err("世界编号重复。".into());
        }
    }
    if !v["activeWorldId"]
        .as_str()
        .is_some_and(|id| ids.contains(id))
    {
        return Err("备份缺少原活动世界。".into());
    }
    Ok(())
}
fn read_state(path: &Path) -> Result<Value, String> {
    let mut state: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if state["format"] != "jade-rust-state" || state["version"] != 1 {
        return Err("不支持的数据格式。".into());
    }
    validate_worlds(&state)?;
    for w in state["worlds"].as_array_mut().unwrap() {
        normalize_world(w);
    }
    Ok(state)
}
pub fn unique_name(name: &str, worlds: &[Value]) -> String {
    let taken: HashSet<_> = worlds.iter().filter_map(|w| w["name"].as_str()).collect();
    if !taken.contains(name) {
        return name.to_owned();
    }
    for i in 2.. {
        let suffix = format!(" ({i})");
        let base: String = name.chars().take(256 - suffix.len()).collect();
        let candidate = format!("{base}{suffix}");
        if !taken.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!()
}
impl Store {
    pub fn open(root: PathBuf) -> Self {
        let file = root.join("state.json");
        let (state, error) = if file.exists() {
            match read_state(&file) {
                Ok(v) => (v, None),
                Err(e) => (
                    Self::initial(),
                    Some(format!(
                        "测试版数据无法读取，已停止保存；请导入备份恢复。{e}"
                    )),
                ),
            }
        } else {
            (Self::initial(), None)
        };
        Self {
            root,
            baseline: state.clone(),
            state,
            write_locked: error.is_some(),
            error,
            dirty: false,
        }
    }
    fn initial() -> Value {
        json!({"format":"jade-rust-state","version":1,"activeWorldId":"default","worlds":[Self::new_world("default","默认世界")],"theme":"moon","readAnnouncements":[],"autoDownload":false})
    }
    pub fn new_world(id: &str, name: &str) -> Value {
        json!({"id":id,"name":name,"inventory":{"records":[],"archived":[],"removed":[],"limit":"1"},"surgery":[],"plans":[],"mapNotes":{"completed":[],"custom":[]}})
    }
    pub fn world_index(&self) -> usize {
        let active = self.state["activeWorldId"].as_str().unwrap_or("default");
        self.state["worlds"]
            .as_array()
            .and_then(|w| w.iter().position(|w| w["id"].as_str() == Some(active)))
            .unwrap_or(0)
    }
    pub fn world(&self) -> &Value {
        &self.state["worlds"][self.world_index()]
    }
    pub fn world_mut(&mut self) -> &mut Value {
        self.dirty = true;
        let i = self.world_index();
        &mut self.state["worlds"][i]
    }
    pub fn changed(&mut self) {
        self.dirty = true;
    }
    pub fn create_checked(&mut self, name: &str) -> Result<(), String> {
        let mut next = self.state.clone();
        let id = uuid::Uuid::new_v4().to_string();
        let worlds = next["worlds"].as_array_mut().unwrap();
        worlds.push(Self::new_world(&id, &unique_name(name.trim(), worlds)));
        next["activeWorldId"] = json!(id);
        self.commit(next)
    }
    pub fn create(&mut self, name: &str) {
        if let Err(e) = self.create_checked(name) {
            self.error = Some(e);
        }
    }
    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), String> {
        let mut next = self.state.clone();
        let w = next["worlds"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|w| w["id"] == id)
            .ok_or("世界已不存在。")?;
        w["name"] = json!(name.trim());
        w["customName"] = json!(true);
        self.commit(next)
    }
    pub fn remove_world(&mut self, id: &str) -> Result<(), String> {
        let mut next = self.state.clone();
        let worlds = next["worlds"].as_array_mut().unwrap();
        if worlds.len() == 1 {
            return Err("请至少保留一个世界。".into());
        }
        if !worlds.iter().any(|w| w["id"] == id) {
            return Err("世界已不存在。".into());
        }
        worlds.retain(|w| w["id"] != id);
        let fallback = worlds[0]["id"].clone();
        if next["activeWorldId"] == id {
            next["activeWorldId"] = fallback;
        }
        self.commit(next)
    }
    pub fn unlink(&mut self, id: &str) -> Result<(), String> {
        let mut next = self.state.clone();
        let w = next["worlds"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|w| w["id"] == id)
            .ok_or("世界已不存在。")?;
        for field in ["nativeGame", "terminalOwner", "terminalSnapshot"] {
            w.as_object_mut().unwrap().remove(field);
        }
        self.commit(next)
    }
    pub fn backup(&self) -> Value {
        json!({"format":"pal-atlas-worlds","version":1,"activeWorldId":self.state["activeWorldId"],"worlds":self.state["worlds"]})
    }
    pub fn backup_current(&self) -> Value {
        json!({"format":"pal-atlas-worlds","version":1,"activeWorldId":self.world()["id"],"worlds":[self.world()]})
    }
    pub fn validate_import(bundle: &Value) -> Result<(), String> {
        if bundle["format"] != "pal-atlas-worlds" || bundle["version"] != 1 {
            return Err("这不是支持的世界档案备份。".into());
        }
        validate_worlds(bundle)
    }
    pub fn import(&mut self, bundle: Value) -> Result<usize, String> {
        Self::validate_import(&bundle)?;
        let worlds = bundle["worlds"].as_array().unwrap();
        let mut next = self.state.clone();
        for w in worlds {
            let mut copy = w.clone();
            normalize_world(&mut copy);
            let id = uuid::Uuid::new_v4().to_string();
            copy["id"] = json!(id);
            copy["name"] = json!(unique_name(
                w["name"].as_str().unwrap(),
                next["worlds"].as_array().unwrap()
            ));
            // A portable copy retains all records and plans but never silently reads another machine's save path.
            for key in ["nativeGame", "terminalOwner", "terminalSnapshot"] {
                copy.as_object_mut().unwrap().remove(key);
            }
            next["worlds"].as_array_mut().unwrap().push(copy);
            if w["id"] == bundle["activeWorldId"] {
                next["activeWorldId"] = json!(id);
            }
        }
        if self.write_locked {
            let _lock = StateLock::acquire(&self.root)?;
            validate_worlds(&next)?;
            self.write(&next)?;
            self.state = next.clone();
            self.baseline = next;
            self.write_locked = false;
            self.error = None;
            self.dirty = false;
        } else {
            self.commit(next)?;
        }
        Ok(worlds.len())
    }
    pub fn commit(&mut self, staged: Value) -> Result<(), String> {
        if self.write_locked {
            return Err(self
                .error
                .clone()
                .unwrap_or_else(|| "数据已锁定，请从备份恢复。".into()));
        }
        validate_worlds(&staged)?;
        let _lock = StateLock::acquire(&self.root)?;
        let next = if self.root.join("state.json").exists() {
            let disk = read_state(&self.root.join("state.json"))?;
            merge(&self.baseline, &staged, &disk, "档案")?
        } else {
            staged
        };
        validate_worlds(&next)?;
        self.write(&next)?;
        self.state = next.clone();
        self.baseline = next;
        self.error = None;
        self.dirty = false;
        Ok(())
    }
    fn write(&self, value: &Value) -> Result<(), String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let staged = self
            .root
            .join(format!("state-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            use std::io::Write;
            let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged)
                .map_err(|e| e.to_string())?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            drop(file);
            let target = self.root.join("state.json");
            if target.exists() {
                std::fs::copy(&target, self.root.join("state.previous.json"))
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                unsafe extern "system" {
                    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
                }
                let a: Vec<u16> = staged.as_os_str().encode_wide().chain(Some(0)).collect();
                let b: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
                if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 0x1 | 0x8) } == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            #[cfg(not(windows))]
            std::fs::rename(&staged, &target).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&staged);
        }
        result
    }
    pub fn flush(&mut self) {
        if self.write_locked {
            return;
        }
        if self.dirty {
            if let Err(e) = self.commit(self.state.clone()) {
                self.error = Some(format!("数据未能保存：{e}"));
            }
        } else if self.root.join("state.json").exists() {
            match read_state(&self.root.join("state.json")) {
                Ok(v) => {
                    self.baseline = v.clone();
                    self.state = v;
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("另一窗口的数据无法读取，保留当前档案：{e}")),
            }
        }
    }
}

// Merge independent edits from separate windows; reject conflicting edits instead of overwriting either.
fn merge(base: &Value, ours: &Value, theirs: &Value, path: &str) -> Result<Value, String> {
    if ours == base {
        return Ok(theirs.clone());
    }
    if theirs == base || ours == theirs {
        return Ok(ours.clone());
    }
    if let (Some(b), Some(o), Some(t)) = (base.as_object(), ours.as_object(), theirs.as_object()) {
        let mut out = serde_json::Map::new();
        let keys: BTreeSet<_> = b.keys().chain(o.keys()).chain(t.keys()).collect();
        for key in keys {
            let v = merge(
                b.get(key).unwrap_or(&Value::Null),
                o.get(key).unwrap_or(&Value::Null),
                t.get(key).unwrap_or(&Value::Null),
                &format!("{path}.{key}"),
            )?;
            if !v.is_null() {
                out.insert(key.clone(), v);
            }
        }
        return Ok(Value::Object(out));
    }
    if let (Some(b), Some(o), Some(t)) = (base.as_array(), ours.as_array(), theirs.as_array()) {
        let has_ids = |v: &Vec<Value>| v.iter().all(|r| r["id"].is_string());
        if has_ids(b) && has_ids(o) && has_ids(t) {
            let mut ids = HashSet::new();
            let mut out = vec![];
            for row in o.iter().chain(t.iter()).chain(b.iter()) {
                let id = row["id"].as_str().unwrap();
                if !ids.insert(id) {
                    continue;
                }
                let get = |v: &Vec<Value>| {
                    v.iter()
                        .find(|r| r["id"] == id)
                        .cloned()
                        .unwrap_or(Value::Null)
                };
                let v = merge(&get(b), &get(o), &get(t), &format!("{path}[{id}]"))?;
                if !v.is_null() {
                    out.push(v);
                }
            }
            return Ok(json!(out));
        }
    }
    Err(format!(
        "另一窗口同时修改了 {path}，已保留原数据；请重新打开后重试。"
    ))
}

#[cfg(windows)]
struct StateLock(*mut std::ffi::c_void);
#[cfg(windows)]
unsafe extern "system" {
    fn CreateMutexW(
        a: *const std::ffi::c_void,
        owned: i32,
        name: *const u16,
    ) -> *mut std::ffi::c_void;
    fn WaitForSingleObject(h: *mut std::ffi::c_void, ms: u32) -> u32;
    fn ReleaseMutex(h: *mut std::ffi::c_void) -> i32;
    fn CloseHandle(h: *mut std::ffi::c_void) -> i32;
}
#[cfg(windows)]
impl StateLock {
    fn acquire(root: &Path) -> Result<Self, String> {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        root.to_string_lossy().to_lowercase().hash(&mut hash);
        let name: Vec<u16> = format!("Local\\JadeRustState-{:016x}", hash.finish())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if h.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        match unsafe { WaitForSingleObject(h, 5000) } {
            0 | 0x80 => Ok(Self(h)),
            _ => {
                unsafe { CloseHandle(h) };
                Err("另一窗口正在保存，请稍后重试。".into())
            }
        }
    }
}
#[cfg(windows)]
impl Drop for StateLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
struct StateLock;
#[cfg(not(windows))]
impl StateLock {
    fn acquire(_: &Path) -> Result<Self, String> {
        Ok(Self)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub struct Temp(pub PathBuf);
    impl Temp {
        pub fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("jade-native-state-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    pub fn record(id: &str) -> Value {
        json!({"id":id,"speciesKey":"SheepBall","name":"棉悠悠","sex":"M","traits":["Legend"]})
    }
    #[test]
    fn failed_durable_commit_keeps_memory_and_existing_worlds() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.join("blocked"));
        std::fs::write(&store.root, b"not a directory").unwrap();
        let before = store.state.clone();
        assert!(store.create_checked("新世界").is_err());
        assert_eq!(store.state, before);
    }
    #[test]
    fn backup_import_preserves_unknown_fields_and_scopes_names_active_world() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let mut w = Store::new_world("source", "默认世界");
        w["inventory"]["records"] = json!([record("first")]);
        w["futureMetadata"] = json!({"preserve":true});
        w["nativeGame"] = json!({"id":"foreign","path":"C:/foreign"});
        let bundle =
            json!({"format":"pal-atlas-worlds","version":1,"activeWorldId":"source","worlds":[w]});
        assert_eq!(store.import(bundle.clone()).unwrap(), 1);
        assert_eq!(store.world()["name"], "默认世界 (2)");
        assert_eq!(store.world()["futureMetadata"]["preserve"], true);
        assert!(store.world()["nativeGame"].is_null());
        assert_eq!(
            store.backup_current()["worlds"].as_array().unwrap().len(),
            1
        );
        store.import(bundle).unwrap();
        assert_eq!(store.world()["name"], "默认世界 (3)");
        let reopen = Store::open(temp.0.clone());
        assert_eq!(store.state, reopen.state);
    }
    #[test]
    fn malformed_backup_rejected_before_any_write() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let before = store.state.clone();
        for mode in 0..7 {
            let mut bundle = store.backup();
            match mode {
                0 => bundle["activeWorldId"] = json!("missing"),
                1 => {
                    let copy = bundle["worlds"][0].clone();
                    bundle["worlds"].as_array_mut().unwrap().push(copy)
                }
                2 => {
                    bundle["worlds"][0]["inventory"]["records"] =
                        json!([record("dup"), record("dup")])
                }
                3 => bundle["worlds"][0]["inventory"]["records"] = json!([{"id":"bad"}]),
                4 => bundle["worlds"][0]["surgery"] = json!(["Legend", "Legend"]),
                5 => bundle["worlds"][0]["plans"] = json!([{"id":"bad"}]),
                _ => {
                    bundle["worlds"][0]["mapNotes"]["custom"] = json!([{"id":"custom-test","name":"点位","map":"palpagos","u":2.,"v":0.2,"note":"","done":false}])
                }
            }
            assert!(store.import(bundle).is_err(), "mode {mode}");
            assert_eq!(store.state, before);
            assert!(!temp.0.join("state.json").exists());
        }
    }
    #[test]
    fn corrupt_data_is_not_overwritten_and_explicit_backup_recovers() {
        let temp = Temp::new();
        let path = temp.0.join("state.json");
        std::fs::write(&path, b"corrupted").unwrap();
        let mut store = Store::open(temp.0.clone());
        assert!(store.error.is_some());
        store.world_mut()["name"] = json!("attempt");
        store.flush();
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupted");
        let backup = Store::initial();
        let bundle = json!({"format":"pal-atlas-worlds","version":1,"activeWorldId":backup["activeWorldId"],"worlds":backup["worlds"]});
        store.import(bundle).unwrap();
        assert!(store.error.is_none());
        assert_eq!(
            std::fs::read(temp.0.join("state.previous.json")).unwrap(),
            b"corrupted"
        );
    }
    #[test]
    fn multiwindow_independent_edits_merge_and_conflicts_preserve_disk() {
        let temp = Temp::new();
        let mut a = Store::open(temp.0.clone());
        a.commit(a.state.clone()).unwrap();
        let mut b = Store::open(temp.0.clone());
        a.world_mut()["inventory"]["records"]
            .as_array_mut()
            .unwrap()
            .push(record("one"));
        b.world_mut()["inventory"]["records"]
            .as_array_mut()
            .unwrap()
            .push(record("two"));
        a.flush();
        b.flush();
        assert!(b.error.is_none(), "{:?}", b.error);
        assert_eq!(
            b.world()["inventory"]["records"].as_array().unwrap().len(),
            2
        );
        a.flush();
        assert_eq!(a.state, b.state);
        a.rename("default", "first edit").unwrap();
        assert!(b.rename("default", "conflicting edit").is_err());
        assert_eq!(Store::open(temp.0.clone()).world()["name"], "first edit");
    }
    #[test]
    fn world_rename_delete_unlink_and_last_world_guard() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        assert!(store.remove_world("default").is_err());
        store.create_checked("测试").unwrap();
        let id = store.world()["id"].as_str().unwrap().to_owned();
        store.rename(&id, "重命名").unwrap();
        assert_eq!(store.world()["customName"], true);
        store.world_mut()["nativeGame"] = json!({"id":"world"});
        store.world_mut()["terminalOwner"] = json!("00000000000000000000000000000001");
        store.unlink(&id).unwrap();
        assert!(store.world()["nativeGame"].is_null());
        store.remove_world(&id).unwrap();
        assert_eq!(store.world()["id"], "default");
    }
    #[test]
    fn official_plan_route_and_pending_roundtrip() {
        let temp = Temp::new();
        let mut store = Store::open(temp.0.clone());
        let parents = json!([{"speciesKey":"SheepBall","sex":"M","stockId":"a","name":"A"},{"speciesKey":"SheepBall","sex":"F","stockId":"b","name":"B"}]);
        let pending = json!({"id":"event","stockId":"hatch-event","kind":"hatch","at":1,"generation":1,"speciesKey":"SheepBall","name":"幼崽","sex":"F","traits":["Legend"],"expectedTraits":["Legend"],"expectedSexes":["F"],"parents":parents});
        let plan = json!({"id":"plan","created":1,"targetKey":"SheepBall","traits":["Legend"],"random":false,"surgery":true,"initialRoute":{"generations":1,"surgery":[],"steps":[{"speciesKey":"SheepBall","generation":1,"traits":["Legend"],"sexes":["F"],"parents":parents}]},"log":[],"pending":pending});
        store.world_mut()["plans"] = json!([plan.clone()]);
        store.flush();
        assert!(store.error.is_none(), "{:?}", store.error);
        let mut other = Store::open(temp.0.join("other"));
        other.import(store.backup()).unwrap();
        assert_eq!(other.world()["plans"][0], plan);
    }
}
