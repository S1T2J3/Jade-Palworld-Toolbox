//! Physical window clipping: round in normal mode, rectangular when maximized.
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
type Handle = *mut c_void;
#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn IsZoomed(hwnd: Handle) -> i32;
    fn IsIconic(hwnd: Handle) -> i32;
    fn GetDpiForWindow(hwnd: Handle) -> u32;
    fn SetWindowRgn(hwnd: Handle, region: Handle, redraw: i32) -> i32;
    fn GetWindowRgn(hwnd: Handle, region: Handle) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> Handle;
    fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> Handle;
    fn PtInRegion(region: Handle, x: i32, y: i32) -> i32;
    fn DeleteObject(object: Handle) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: Handle, flags: u32) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *mut c_void;
}
type DwmAttribute = unsafe extern "system" fn(Handle, u32, *const c_void, u32) -> i32;
fn dwm_attribute() -> Option<DwmAttribute> {
    static API: std::sync::OnceLock<Option<DwmAttribute>> = std::sync::OnceLock::new();
    *API.get_or_init(|| unsafe {
        let name: Vec<u16> = "dwmapi.dll".encode_utf16().chain(Some(0)).collect();
        let module = LoadLibraryExW(name.as_ptr(), std::ptr::null_mut(), 0x800);
        if module.is_null() {
            return None;
        }
        let address = GetProcAddress(module, c"DwmSetWindowAttribute".as_ptr().cast());
        if address.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut c_void, DwmAttribute>(address))
        }
    })
}
#[derive(Default)]
pub struct WindowChrome {
    last: Option<(isize, i32, i32, u32, bool)>,
}
impl WindowChrome {
    pub fn apply(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Ok(handle) = frame.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as Handle;
        unsafe {
            if IsIconic(hwnd) != 0 {
                return;
            }
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return;
            }
            let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
            if width <= 0 || height <= 0 {
                return;
            }
            let dpi = GetDpiForWindow(hwnd).max(96);
            let maximized = IsZoomed(hwnd) != 0;
            let signature = (handle.hwnd.get(), width, height, dpi, maximized);
            if self.last == Some(signature) {
                return;
            }
            // DWM is optional on Windows 10; the region also covers undecorated windows.
            let preference: u32 = if maximized { 1 } else { 2 };
            if let Some(set) = dwm_attribute() {
                set(hwnd, 33, (&preference as *const u32).cast(), 4);
            }
            let diameter = ((24 * dpi + 48) / 96) as i32;
            let region = if maximized {
                std::ptr::null_mut()
            } else {
                CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter)
            };
            if !maximized && region.is_null() {
                return;
            }
            if SetWindowRgn(hwnd, region, 1) == 0 {
                if !region.is_null() {
                    DeleteObject(region);
                }
                return;
            }
            // On success Windows owns the region. Do not delete it.
            self.last = Some(signature);
            if std::env::var_os("JADE_QA_OUTPUT").is_some() {
                let probe = CreateRectRgn(0, 0, 0, 0);
                if !probe.is_null() {
                    let kind = GetWindowRgn(hwnd, probe);
                    let proof = serde_json::json!({"width":width,"height":height,"dpi":dpi,
                        "maximized":maximized,"regionKind":kind,"cornerIncluded":PtInRegion(probe,0,0)!=0,
                        "centerIncluded":PtInRegion(probe,width/2,height/2)!=0});
                    ctx.data_mut(|m| m.insert_temp(egui::Id::new("jade-window-shape"), proof));
                    DeleteObject(probe);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_rounding_keeps_center_and_control_edges_at_multiple_dpis() {
        for dpi in [96, 120, 144, 192] {
            let diameter = (24 * dpi + 48) / 96;
            let region = unsafe { CreateRoundRectRgn(0, 0, 961, 621, diameter, diameter) };
            assert!(!region.is_null());
            unsafe {
                assert_eq!(PtInRegion(region, 0, 0), 0);
                assert_ne!(PtInRegion(region, 480, 310), 0);
                assert_ne!(
                    PtInRegion(region, 937, 18),
                    0,
                    "close button center must remain clickable"
                );
                DeleteObject(region);
            }
        }
    }
}
