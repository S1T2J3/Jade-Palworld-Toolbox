//! Native single-instance activation. Explicit new windows share the locked store.
use sha2::{Digest, Sha256};
use std::{ffi::c_void, path::Path};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(a: *const c_void, owner: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
    fn CloseHandle(h: *mut c_void) -> i32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class: *const u16, name: *const u16) -> *mut c_void;
    fn ShowWindow(h: *mut c_void, command: i32) -> i32;
    fn SetForegroundWindow(h: *mut c_void) -> i32;
    fn MessageBoxW(h: *mut c_void, text: *const u16, title: *const u16, style: u32) -> i32;
}
pub const TITLE: &str = "Jade · 幻兽帕鲁工具箱 · Rust 测试版";
pub struct Guard(*mut c_void);
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn acquire(root: &Path, new_window: bool) -> Option<Guard> {
    let hash = format!(
        "{:x}",
        Sha256::digest(root.to_string_lossy().to_lowercase().as_bytes())
    );
    let name: Vec<u16> = format!("Local\\JadeRustWindow-{}", &hash[..24])
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if h.is_null() {
        return Some(Guard(h));
    }
    if unsafe { GetLastError() } == 183 && !new_window {
        let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
        let w = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if !w.is_null() {
            unsafe {
                ShowWindow(w, 9);
                SetForegroundWindow(w);
                CloseHandle(h);
            }
            return None;
        }
    }
    Some(Guard(h))
}
pub fn show_error(error: &str) {
    let body:Vec<u16>=format!("无法打开 Rust 测试版：\n{error}\n\n请确认 data 文件夹完整，显卡驱动支持 OpenGL 2.0。测试版未修改正式版数据。").encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(std::ptr::null_mut(), body.as_ptr(), title.as_ptr(), 0x10);
    }
}
