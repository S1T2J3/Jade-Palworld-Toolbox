//! Native resizing and physical clipping for our undecorated Windows window.
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
    fn GetWindowLongW(hwnd: Handle, index: i32) -> i32;
    fn SetWindowRgn(hwnd: Handle, region: Handle, redraw: i32) -> i32;
    fn GetWindowRgn(hwnd: Handle, region: Handle) -> i32;
}
type SubclassProc = unsafe extern "system" fn(Handle, u32, usize, isize, usize, usize) -> isize;
#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(hwnd: Handle, callback: SubclassProc, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Handle, callback: SubclassProc, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
}

const RESIZE_SUBCLASS: usize = 0x4a414445;
const WM_NCHITTEST: u32 = 0x0084;
const WM_NCDESTROY: u32 = 0x0082;

/// Return Windows' resize direction only within a narrow border. Corners have
/// longer targets along the edge, so the rounded cutout does not hide the grip.
fn resize_hit(width: i32, height: i32, x: i32, y: i32, dpi: u32) -> Option<isize> {
    if x < 0 || y < 0 || x >= width || y >= height {
        return None;
    }
    let scale = |logical: u32| ((logical * dpi.clamp(96, 768) + 48) / 96) as i32;
    let edge = scale(7);
    let corner = scale(20);
    let left = x < edge;
    let right = x >= width - edge;
    let top = y < edge;
    let bottom = y >= height - edge;
    if (left && y < corner) || (top && x < corner) {
        Some(13) // HTTOPLEFT
    } else if (right && y < corner) || (top && x >= width - corner) {
        Some(14) // HTTOPRIGHT
    } else if (left && y >= height - corner) || (bottom && x < corner) {
        Some(16) // HTBOTTOMLEFT
    } else if (right && y >= height - corner) || (bottom && x >= width - corner) {
        Some(17) // HTBOTTOMRIGHT
    } else if left {
        Some(10) // HTLEFT
    } else if right {
        Some(11) // HTRIGHT
    } else if top {
        Some(12) // HTTOP
    } else if bottom {
        Some(15) // HTBOTTOM
    } else {
        None
    }
}

fn screen_point(lparam: isize) -> (i32, i32) {
    // WM_NCHITTEST uses signed screen coordinates (including left/top monitors).
    (lparam as i16 as i32, (lparam >> 16) as i16 as i32)
}

unsafe extern "system" fn resize_subclass(
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    _data: usize,
) -> isize {
    unsafe {
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, resize_subclass, id);
        } else if message == WM_NCHITTEST
            && IsZoomed(hwnd) == 0
            && IsIconic(hwnd) == 0
            && GetWindowLongW(hwnd, -16) as u32 & 0x0004_0000 != 0
        // GWL_STYLE / WS_SIZEBOX
        {
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let (x, y) = screen_point(lparam);
                if let Some(hit) = resize_hit(
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    x - rect.left,
                    y - rect.top,
                    GetDpiForWindow(hwnd),
                ) {
                    return hit;
                }
            }
        }
        // Keep winit's cursor, native size loop, minimum size, keyboard/system
        // commands, accessibility, and all existing client controls intact.
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
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
    subclass: Option<isize>,
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
            if self.subclass != Some(handle.hwnd.get())
                && SetWindowSubclass(hwnd, resize_subclass, RESIZE_SUBCLASS, 0) != 0
            {
                // This callback owns no App pointers and removes itself when
                // the HWND is destroyed. Only the UI thread installs it.
                self.subclass = Some(handle.hwnd.get());
            }
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
    fn native_resize_directions_scale_and_leave_client_controls_alone() {
        for dpi in [96, 120, 144, 192, 288] {
            let px = |v: i32| (v * dpi + 48) / 96;
            let (w, h) = (px(1200), px(800));
            for (x, y, hit) in [
                (3, 400, 10),
                (1197, 400, 11),
                (600, 3, 12),
                (600, 797, 15),
                (6, 6, 13),
                (1194, 6, 14),
                (6, 794, 16),
                (1194, 794, 17),
                (3, 17, 13),
                (1197, 783, 17),
            ] {
                assert_eq!(resize_hit(w, h, px(x), px(y), dpi as u32), Some(hit));
            }
            for (x, y) in [
                (600, 400),
                (600, 17),
                (1177, 17),
                (1131, 17),
                (1085, 17),
                (54, 242),
            ] {
                assert_eq!(resize_hit(w, h, px(x), px(y), dpi as u32), None);
            }
            assert_eq!(resize_hit(w, h, -1, px(400), dpi as u32), None);
            assert_eq!(resize_hit(w, h, w, px(400), dpi as u32), None);
        }
    }
    #[test]
    fn hit_test_coordinates_are_signed_on_other_monitors() {
        for (x, y) in [(-1800i16, -900i16), (-1, 900), (1800, -1), (1200, 800)] {
            let packed = (x as u16 as u32) | ((y as u16 as u32) << 16);
            assert_eq!(screen_point(packed as isize), (x as i32, y as i32));
        }
    }
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
