use eframe::egui;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const OFFICIAL_HASH: Option<&str> = option_env!("JADE_ASSISTANT_KERNEL_SHA256");
const EXE: &str = "Jade-Pal-Assistant-Kernel.exe";
const NOTICE: &str = "Palworld-Knowledge-MIT.txt";
const NOTICE_BYTES: &[u8] = include_bytes!("../../../licenses/Palworld-Knowledge-MIT.txt");
const MAX_EXE: u64 = 128 * 1024 * 1024;
const MAX_MANIFEST: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: u32,
    id: String,
    host_version: String,
    executable: String,
    sha256: String,
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn checked_executable(path: &Path, official_hash: Option<&str>) -> Result<File, String> {
    let expected = official_hash.ok_or("此构建未登记配套内核，请使用同一版本目录下的原版程序。")?;
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("无法读取内核文件：{e}"))?;
    if is_link(&metadata) || !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_EXE
    {
        return Err("内核文件无效，请重新导入配套内核包。".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    // Keep the verified file immutable while a Windows worker is being launched.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("无法打开内核文件：{e}"))?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash).map_err(|e| format!("读取内核失败：{e}"))?;
    if format!("{:x}", hash.finalize()) != expected {
        return Err("内核校验失败，请重新导入与本版工具箱配套的内核包。".into());
    }
    Ok(file)
}

pub(crate) fn verified_executable(path: &Path) -> Result<File, String> {
    checked_executable(path, OFFICIAL_HASH)
}

#[derive(Default, PartialEq)]
struct DiskState {
    executable: Option<(u64, Option<SystemTime>)>,
    enabled: bool,
}

pub struct ModuleManager {
    root: PathBuf,
    executable: PathBuf,
    official_hash: Option<String>,
    disk: DiskState,
    active: Option<File>,
    checked_at: Instant,
    message: String,
}

impl ModuleManager {
    pub fn new(root: PathBuf) -> Self {
        Self::with_hash(root, OFFICIAL_HASH.map(str::to_owned))
    }

    fn with_hash(root: PathBuf, official_hash: Option<String>) -> Self {
        let executable = root
            .join("mods")
            .join("pal-assistant")
            .join(VERSION)
            .join(EXE);
        let mut manager = Self {
            root,
            executable,
            official_hash,
            disk: DiskState::default(),
            active: None,
            checked_at: Instant::now(),
            message: String::new(),
        };
        manager.refresh();
        manager
    }

    pub fn active_path(&self) -> Option<&Path> {
        self.active.as_ref().map(|_| self.executable.as_path())
    }

    pub fn poll_deactivation(&mut self) {
        if self.active.is_none() || self.checked_at.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.checked_at = Instant::now();
        if !self.enabled_on_disk() {
            self.active = None;
            self.disk.enabled = false;
            self.message.clear();
        }
    }

    fn version_dir(&self) -> &Path {
        self.executable
            .parent()
            .expect("module path contains a version directory")
    }

    fn marker(&self) -> PathBuf {
        self.version_dir().join("enabled")
    }

    fn enabled_on_disk(&self) -> bool {
        fs::symlink_metadata(self.marker()).is_ok_and(|m| m.is_file() && !is_link(&m))
    }

    fn inspect(&self) -> DiskState {
        let executable = fs::symlink_metadata(&self.executable)
            .ok()
            .map(|m| (m.len(), m.modified().ok()));
        let enabled = self.enabled_on_disk();
        DiskState {
            executable,
            enabled,
        }
    }

    fn check_directories(&self, create: bool) -> Result<(), String> {
        let mut dir = self.root.clone();
        if create {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        for part in ["mods", "pal-assistant", VERSION] {
            dir.push(part);
            if create {
                match fs::create_dir(&dir) {
                    Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e.to_string()),
                    _ => {}
                }
            }
            let metadata = fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
            if !metadata.is_dir() || is_link(&metadata) {
                return Err("内核目录无效，请检查工具箱数据目录中的 mods 文件夹。".into());
            }
        }
        Ok(())
    }

    fn refresh(&mut self) {
        self.checked_at = Instant::now();
        let disk = self.inspect();
        if disk == self.disk {
            return;
        }
        self.active = None;
        self.message.clear();
        if disk.enabled {
            match self
                .check_directories(false)
                .and_then(|()| checked_executable(&self.executable, self.official_hash.as_deref()))
            {
                Ok(file) => self.active = Some(file),
                Err(error) => self.message = error,
            }
        }
        self.disk = disk;
    }

    fn enable(&mut self) -> Result<(), String> {
        self.check_directories(false)?;
        let file = checked_executable(&self.executable, self.official_hash.as_deref())?;
        // Creating an empty marker is atomic even if another window enables the module.
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.marker())
        {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && self.inspect().enabled => {}
            Err(e) => return Err(format!("无法保存内核设置：{e}")),
        }
        self.active = Some(file);
        self.disk = self.inspect();
        // Track our own action. If a different window toggled concurrently,
        // refresh() will pick up its newer on-disk preference on the next check.
        self.disk.enabled = true;
        self.message.clear();
        Ok(())
    }

    fn disable(&mut self) -> Result<(), String> {
        self.check_directories(false)?;
        match fs::remove_file(self.marker()) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                return Err(format!("无法停用内核：{e}"));
            }
            _ => {}
        }
        self.active = None;
        self.disk = self.inspect();
        self.disk.enabled = false;
        self.message.clear();
        Ok(())
    }

    fn install(&mut self, path: &Path) -> Result<(), String> {
        let expected = self
            .official_hash
            .clone()
            .ok_or("此构建未登记配套内核，请使用同一版本目录下的原版程序。")?;
        let package = File::open(path).map_err(|e| format!("无法打开内核包：{e}"))?;
        if package.metadata().map_err(|e| e.to_string())?.len() > MAX_EXE + 1024 * 1024 {
            return Err("内核包过大。".into());
        }
        let mut zip = zip::ZipArchive::new(package).map_err(|_| "无法读取 ZIP 内核包。")?;
        if !matches!(zip.len(), 2 | 3) {
            return Err("请选择包含内核清单、程序和配套授权声明的 ZIP。".into());
        }
        if zip.len() == 3 {
            let mut notice = Vec::new();
            zip.by_name(NOTICE).map_err(|_| "内核包的附加文件不是配套授权声明。")?
                .take(NOTICE_BYTES.len() as u64 + 1)
                .read_to_end(&mut notice).map_err(|e|e.to_string())?;
            if notice != NOTICE_BYTES {
                return Err("内核包的授权声明校验失败。".into());
            }
        }
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let limit = match entry.name() {
                "kernel.json" => MAX_MANIFEST,
                EXE => MAX_EXE,
                NOTICE => NOTICE_BYTES.len() as u64,
                _ => return Err("内核包包含不支持的文件。".into()),
            };
            if !entry.is_file()
                || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
                || entry.size() == 0
                || entry.size() > limit
            {
                return Err("内核包中的文件不完整或大小异常。".into());
            }
        }
        let mut manifest_bytes = Vec::new();
        zip.by_name("kernel.json")
            .map_err(|_| "内核包缺少清单。")?
            .take(MAX_MANIFEST + 1)
            .read_to_end(&mut manifest_bytes)
            .map_err(|e| e.to_string())?;
        if manifest_bytes.len() as u64 > MAX_MANIFEST {
            return Err("内核清单过大。".into());
        }
        let manifest: Manifest =
            serde_json::from_slice(&manifest_bytes).map_err(|_| "无法解析内核清单。")?;
        if manifest.schema != 1 || manifest.id != "pal-assistant" || manifest.executable != EXE {
            return Err("这不是受支持的帕鲁助手内核包。".into());
        }
        if manifest.host_version != VERSION {
            return Err(format!(
                "此内核适用于 {}，请选择 {VERSION} 的内核包。",
                manifest.host_version
            ));
        }
        if manifest.sha256 != expected {
            return Err("此内核与本版工具箱登记的校验值不符。".into());
        }

        self.check_directories(true)?;
        let stage = self
            .version_dir()
            .join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let staged = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)
            .map_err(|e| format!("无法暂存内核：{e}"))?;
        let result = (|| {
            let mut entry = zip.by_name(EXE).map_err(|_| "内核包缺少程序。")?;
            let declared = entry.size();
            let mut writer = staged;
            let n = io::copy(&mut entry.by_ref().take(MAX_EXE + 1), &mut writer)
                .map_err(|e| e.to_string())?;
            writer.sync_all().map_err(|e| e.to_string())?;
            drop(writer);
            if n != declared {
                return Err("内核包长度校验失败。".into());
            }
            drop(checked_executable(&stage, Some(&expected))?);
            if checked_executable(&self.executable, Some(&expected)).is_err() {
                // Publish only a fully validated executable; imports from two windows
                // can race harmlessly when either one has already installed this hash.
                if let Err(e) = fs::rename(&stage, &self.executable)
                    && checked_executable(&self.executable, Some(&expected)).is_err()
                {
                    return Err(format!(
                        "无法替换内核文件，请关闭正在使用它的窗口后重试：{e}"
                    ));
                }
            }
            self.enable()
        })();
        let _ = fs::remove_file(&stage);
        result
    }

    pub fn poll(&mut self) {
        if self.checked_at.elapsed() >= Duration::from_secs(1) {
            self.refresh();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        ui.horizontal_wrapped(|ui| {
            if self.active.is_some() {
                ui.strong("增强内核已启用");
                if ui.small_button("停用内核").clicked()
                    && let Err(e) = self.disable()
                {
                    self.message = e;
                }
            } else {
                ui.strong("基础问答");
                if self.disk.executable.is_some()
                    && ui.small_button("启用内核").clicked()
                    && let Err(e) = self.enable()
                {
                    self.message = e;
                }
            }
            if ui
                .small_button("导入内核包")
                .on_hover_text("选择同版本的帕鲁助手内核 ZIP")
                .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_title("导入帕鲁助手内核")
                    .add_filter("帕鲁助手内核包", &["zip"])
                    .pick_file()
                && let Err(e) = self.install(&path)
            {
                self.message = e;
            }
        });
        if !self.message.is_empty() {
            ui.label(&self.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::Temp;
    use std::io::Write;

    const PAYLOAD: &[u8] = b"kernel-fixture-content";

    fn hash() -> String {
        format!("{:x}", Sha256::digest(PAYLOAD))
    }

    fn make_zip(
        dir: &Path,
        version: &str,
        declared_hash: &str,
        name: &str,
        payload: &[u8],
    ) -> PathBuf {
        let zip_path = dir.join(format!("{}.zip", uuid::Uuid::new_v4()));
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("kernel.json", options).unwrap();
        let manifest = serde_json::json!({
            "schema":1,"id":"pal-assistant","hostVersion":version,"executable":EXE,"sha256":declared_hash
        });
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file(name, options).unwrap();
        zip.write_all(payload).unwrap();
        zip.finish().unwrap();
        zip_path
    }

    #[test]
    fn install_enable_disable_and_reopen_use_the_same_profile_and_exact_version() {
        let temp = Temp::new();
        let profile = temp.0.join("profile");
        let mut manager = ModuleManager::with_hash(profile.clone(), Some(hash()));
        assert!(manager.active_path().is_none() && !profile.exists());
        let package = make_zip(&temp.0, VERSION, &hash(), EXE, PAYLOAD);
        manager.install(&package).unwrap();
        let expected_path = profile.join("mods/pal-assistant").join(VERSION).join(EXE);
        assert_eq!(manager.active_path(), Some(expected_path.as_path()));
        let mut other = ModuleManager::with_hash(profile.clone(), Some(hash()));
        assert_eq!(other.active_path(), manager.active_path());
        other.install(&package).unwrap();
        manager.disable().unwrap();
        other.checked_at = Instant::now() - Duration::from_secs(2);
        other.poll_deactivation();
        assert!(other.active_path().is_none() && expected_path.is_file());
        let mut reopened = ModuleManager::with_hash(profile, Some(hash()));
        assert!(reopened.active_path().is_none());
        reopened.enable().unwrap();
        assert!(reopened.active_path().is_some());
        manager.checked_at = Instant::now() - Duration::from_secs(2);
        manager.poll_deactivation();
        assert!(manager.active_path().is_none());
        manager.refresh();
        assert_eq!(manager.active_path(), reopened.active_path());
    }

    #[test]
    fn failed_import_never_replaces_the_active_kernel_or_enables_an_unsigned_package() {
        let temp = Temp::new();
        let mut manager = ModuleManager::with_hash(temp.0.join("profile"), Some(hash()));
        let good = make_zip(&temp.0, VERSION, &hash(), EXE, PAYLOAD);
        manager.install(&good).unwrap();
        for package in [
            make_zip(&temp.0, "0.0.1", &hash(), EXE, PAYLOAD),
            make_zip(&temp.0, VERSION, &hash(), EXE, b"corrupted payload"),
            make_zip(&temp.0, VERSION, &"a".repeat(64), EXE, PAYLOAD),
            make_zip(&temp.0, VERSION, &hash(), "../unexpected", PAYLOAD),
        ] {
            assert!(manager.install(&package).is_err());
            assert_eq!(fs::read(manager.active_path().unwrap()).unwrap(), PAYLOAD);
        }
        let mut unpinned = ModuleManager::with_hash(temp.0.join("other"), None);
        assert!(unpinned.install(&good).is_err());
        assert!(unpinned.active_path().is_none());
    }

    #[test]
    fn corrupt_installed_file_falls_back_and_can_be_repaired() {
        let temp = Temp::new();
        let profile = temp.0.join("profile");
        let good = make_zip(&temp.0, VERSION, &hash(), EXE, PAYLOAD);
        let mut manager = ModuleManager::with_hash(profile.clone(), Some(hash()));
        manager.install(&good).unwrap();
        let installed = manager.active_path().unwrap().to_path_buf();
        drop(manager);
        fs::write(&installed, b"corrupt").unwrap();
        let mut reopened = ModuleManager::with_hash(profile, Some(hash()));
        assert!(reopened.active_path().is_none());
        assert!(reopened.enable().is_err());
        reopened.install(&good).unwrap();
        assert_eq!(fs::read(reopened.active_path().unwrap()).unwrap(), PAYLOAD);
    }
}
