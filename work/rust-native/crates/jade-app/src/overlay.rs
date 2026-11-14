//! Optional Windows hotkey overlay. The complete app stays in its own process.
//! No game code, saves, or process memory are changed or read.
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    rc::Rc,
    time::{Duration, Instant},
};

type Handle = *mut c_void;
type SubclassProc = unsafe extern "system" fn(Handle, u32, usize, isize, usize, usize) -> isize;
const SUBCLASS_ID: usize = 0x4a44_4f56;
const HOTKEY_ID: i32 = 0x4a44;
const TIMER_ID: usize = 0x4a44_4f54;
const WM_NCDESTROY: u32 = 0x0082;
const WM_HOTKEY: u32 = 0x0312;
const WM_TIMER: u32 = 0x0113;
const WM_ACTIVATE: u32 = 0x0006;
const WM_SHOWWINDOW: u32 = 0x0018;
const WM_SIZE: u32 = 0x0005;
const WM_MOVING: u32 = 0x0216;
const WM_SIZING: u32 = 0x0214;
const SW_HIDE: i32 = 0;
const SW_SHOWNORMAL: i32 = 1;
const SW_SHOWMAXIMIZED: i32 = 3;
const SW_RESTORE: i32 = 9;
const WS_EX_TOPMOST: i32 = 8;
const HWND_TOPMOST: Handle = -1isize as Handle;
const HWND_NOTOPMOST: Handle = -2isize as Handle;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
impl Rect {
    fn width(self) -> i32 {
        self.right.saturating_sub(self.left)
    }
    fn height(self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }
    fn positive(self) -> bool {
        self.width() > 0 && self.height() > 0
    }
    fn intersection(self, other: Self) -> Self {
        Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Placement {
    length: u32,
    flags: u32,
    show_cmd: u32,
    min: Point,
    max: Point,
    normal: Rect,
}
#[repr(C)]
#[derive(Default)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterHotKey(hwnd: Handle, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: Handle, id: i32) -> i32;
    fn GetForegroundWindow() -> Handle;
    fn GetWindowThreadProcessId(hwnd: Handle, pid: *mut u32) -> u32;
    fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn GetClientRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn ClientToScreen(hwnd: Handle, point: *mut Point) -> i32;
    fn GetDpiForWindow(hwnd: Handle) -> u32;
    fn GetWindowLongW(hwnd: Handle, index: i32) -> i32;
    fn GetWindowPlacement(hwnd: Handle, placement: *mut Placement) -> i32;
    fn SetWindowPlacement(hwnd: Handle, placement: *const Placement) -> i32;
    fn SetWindowPos(
        hwnd: Handle,
        after: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn SetForegroundWindow(hwnd: Handle) -> i32;
    fn ShowWindow(hwnd: Handle, command: i32) -> i32;
    fn IsWindow(hwnd: Handle) -> i32;
    fn IsWindowVisible(hwnd: Handle) -> i32;
    fn IsIconic(hwnd: Handle) -> i32;
    fn IsZoomed(hwnd: Handle) -> i32;
    fn GetWindow(hwnd: Handle, command: u32) -> Handle;
    fn MonitorFromWindow(hwnd: Handle, flags: u32) -> Handle;
    fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
    fn SetTimer(hwnd: Handle, id: usize, milliseconds: u32, callback: *const c_void) -> usize;
    fn KillTimer(hwnd: Handle, id: usize) -> i32;
    fn ClipCursor(rect: *const Rect) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        path: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
}
#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(hwnd: Handle, callback: SubclassProc, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Handle, callback: SubclassProc, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hotkey {
    #[default]
    AltJ,
    CtrlShiftJ,
    F8,
    F9,
}
impl Hotkey {
    pub fn key(self) -> &'static str {
        match self {
            Self::AltJ => "alt-j",
            Self::CtrlShiftJ => "ctrl-shift-j",
            Self::F8 => "f8",
            Self::F9 => "f9",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::AltJ => "Alt + J",
            Self::CtrlShiftJ => "Ctrl + Shift + J",
            Self::F8 => "F8",
            Self::F9 => "F9",
        }
    }
    pub fn from_key(value: &str) -> Option<Self> {
        let value = value
            .trim()
            .to_ascii_lowercase()
            .replace(['+', '-', '_', ' '], "");
        match value.as_str() {
            "altj" => Some(Self::AltJ),
            "ctrlshiftj" => Some(Self::CtrlShiftJ),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            _ => None,
        }
    }
    fn native(self) -> (u32, u32) {
        let (modifiers, key) = match self {
            Self::AltJ => (1, b'J' as u32),
            Self::CtrlShiftJ => (2 | 4, b'J' as u32),
            Self::F8 => (0, 0x77),
            Self::F9 => (0, 0x78),
        };
        (modifiers | 0x4000, key) // MOD_NOREPEAT
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub hotkey: Hotkey,
    pub auto_hide: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            hotkey: Hotkey::AltJ,
            auto_hide: true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub registered: bool,
    pub session: bool,
    pub visible: bool,
    pub status: String,
}

#[derive(Clone, Copy, Debug)]
struct Game {
    hwnd: isize,
    pid: u32,
}
struct Session {
    desktop: Placement,
    desktop_topmost: bool,
    game: Game,
    visible: bool,
    overlay_rect: Option<Rect>,
    bounds: Rect,
    min_size: (i32, i32),
    fallback: bool,
    game_closed: bool,
}
struct State {
    config: Config,
    registered: bool,
    session: Option<Session>,
    status: String,
    last_observed: Instant,
}
struct Bridge {
    hwnd: isize,
    ctx: egui::Context,
    state: RefCell<State>,
    alive: Cell<bool>,
    callback_ref: Cell<bool>,
}
struct Binding {
    bridge: Rc<Bridge>,
    hotkey: Option<Hotkey>,
    requested: Option<Hotkey>,
    attempted: Option<Instant>,
}
pub struct Overlay {
    binding: Option<Binding>,
    status: String,
}
impl Default for Overlay {
    fn default() -> Self {
        Self {
            binding: None,
            status: "未开启游戏悬浮窗".into(),
        }
    }
}
impl Overlay {
    pub fn sync(&mut self, ctx: &egui::Context, frame: &eframe::Frame, config: Config) {
        if !config.enabled {
            if let Some(binding) = self.binding.take() {
                binding.bridge.desktop("已关闭游戏悬浮窗");
                drop(binding);
            }
            if self.status != "未开启游戏悬浮窗" {
                self.status = "未开启游戏悬浮窗".into();
            }
            return;
        }
        let Ok(handle) = frame.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get();
        if self
            .binding
            .as_ref()
            .is_none_or(|b| b.bridge.hwnd != hwnd || !b.bridge.alive.get())
        {
            if let Some(old) = self.binding.take() {
                old.bridge.desktop("已恢复普通窗口");
            }
            match Binding::new(ctx.clone(), hwnd, config) {
                Ok(binding) => self.binding = Some(binding),
                Err(error) => {
                    self.status = error;
                    return;
                }
            }
        }
        if let Some(binding) = &mut self.binding {
            binding.configure(config);
            binding.bridge.observe();
        }
    }
    pub fn hide(&mut self) {
        if let Some(binding) = &self.binding {
            binding.bridge.hide(true);
        }
    }
    pub fn return_to_desktop(&mut self) {
        if let Some(binding) = &self.binding {
            binding.bridge.desktop("已返回普通窗口，游戏快捷键仍已启用");
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        if let Some(binding) = &self.binding {
            if let Ok(state) = binding.bridge.state.try_borrow() {
                return Snapshot {
                    registered: state.registered,
                    session: state.session.is_some(),
                    visible: state.session.as_ref().is_some_and(|s| s.visible),
                    status: state.status.clone(),
                };
            }
        }
        Snapshot {
            registered: false,
            session: false,
            visible: false,
            status: self.status.clone(),
        }
    }
}

impl Binding {
    fn new(ctx: egui::Context, hwnd: isize, config: Config) -> Result<Self, String> {
        let bridge = Rc::new(Bridge {
            hwnd,
            ctx,
            state: RefCell::new(State {
                config,
                registered: false,
                session: None,
                status: String::new(),
                last_observed: Instant::now(),
            }),
            alive: Cell::new(true),
            callback_ref: Cell::new(false),
        });
        let raw = Rc::into_raw(bridge.clone());
        if unsafe { SetWindowSubclass(hwnd as Handle, window_proc, SUBCLASS_ID, raw as usize) } == 0
        {
            unsafe {
                drop(Rc::from_raw(raw));
            }
            return Err("无法连接游戏悬浮窗的窗口事件，请重启工具箱后重试".into());
        }
        bridge.callback_ref.set(true);
        Ok(Self {
            bridge,
            hotkey: None,
            requested: None,
            attempted: None,
        })
    }
    fn configure(&mut self, config: Config) {
        if let Ok(mut state) = self.bridge.state.try_borrow_mut() {
            state.config = config;
        }
        if self.requested != Some(config.hotkey) {
            if self.hotkey.is_some() {
                unsafe {
                    UnregisterHotKey(self.bridge.hwnd as Handle, HOTKEY_ID);
                }
            }
            self.hotkey = None;
            self.attempted = None;
            self.requested = Some(config.hotkey);
            if let Ok(mut state) = self.bridge.state.try_borrow_mut() {
                state.registered = false;
            }
        }
        if self.hotkey == Some(config.hotkey) {
            return;
        }
        if self.hotkey.is_some() {
            unsafe {
                UnregisterHotKey(self.bridge.hwnd as Handle, HOTKEY_ID);
            }
            self.hotkey = None;
            self.attempted = None;
            if let Ok(mut state) = self.bridge.state.try_borrow_mut() {
                state.registered = false;
            }
        }
        if self
            .attempted
            .is_some_and(|time| time.elapsed() < Duration::from_secs(3))
        {
            return;
        }
        self.attempted = Some(Instant::now());
        let (modifiers, key) = config.hotkey.native();
        let registered =
            unsafe { RegisterHotKey(self.bridge.hwnd as Handle, HOTKEY_ID, modifiers, key) } != 0;
        let error = if registered {
            0
        } else {
            unsafe { GetLastError() }
        };
        if let Ok(mut state) = self.bridge.state.try_borrow_mut() {
            state.registered = registered;
            state.status = if registered {
                format!("已启用：在幻兽帕鲁中按 {} 呼出", config.hotkey.label())
            } else if error == 1409 {
                format!(
                    "{} 已被其他程序或工具箱窗口占用，请更换快捷键",
                    config.hotkey.label()
                )
            } else {
                format!(
                    "无法注册 {}（Windows 错误 {}），请更换快捷键后重试",
                    config.hotkey.label(),
                    error
                )
            };
        }
        if registered {
            self.hotkey = Some(config.hotkey);
        }
    }
}
impl Drop for Binding {
    fn drop(&mut self) {
        let hwnd = self.bridge.hwnd as Handle;
        // Stop queued events even if another component prevents detaching the
        // callback. Its independent raw reference then lives until NCDESTROY.
        if let Ok(mut state) = self.bridge.state.try_borrow_mut() {
            state.registered = false;
            state.config.enabled = false;
            state.session = None;
        }
        unsafe {
            if !self.bridge.alive.get() || IsWindow(hwnd) == 0 {
                return;
            }
            KillTimer(hwnd, TIMER_ID);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            if RemoveWindowSubclass(hwnd, window_proc, SUBCLASS_ID) != 0
                && self.bridge.callback_ref.replace(false)
            {
                drop(Rc::from_raw(Rc::as_ptr(&self.bridge)));
            }
        }
    }
}

fn window_pid(hwnd: Handle) -> u32 {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid
}
fn game_basename(path: &str) -> bool {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    name.eq_ignore_ascii_case("Palworld-Win64-Shipping.exe")
        || name.eq_ignore_ascii_case("Palworld-WinGDK-Shipping.exe")
}
fn foreground_game() -> Option<Game> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return None;
        }
        let pid = window_pid(hwnd);
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(0x1000, 0, pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if process.is_null() {
            return None;
        }
        let mut buffer = [0u16; 2048];
        let mut length = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) != 0;
        CloseHandle(process);
        if ok && game_basename(&String::from_utf16_lossy(&buffer[..length as usize])) {
            Some(Game {
                hwnd: hwnd as isize,
                pid,
            })
        } else {
            None
        }
    }
}
fn game_alive(game: Game) -> bool {
    unsafe { IsWindow(game.hwnd as Handle) != 0 && window_pid(game.hwnd as Handle) == game.pid }
}
fn own_window(candidate: Handle, owner: Handle) -> bool {
    if candidate.is_null() {
        return false;
    }
    if candidate == owner || window_pid(candidate) == window_pid(owner) {
        return true;
    }
    let mut current = candidate;
    for _ in 0..16 {
        current = unsafe { GetWindow(current, 4) }; // GW_OWNER
        if current.is_null() {
            break;
        }
        if current == owner {
            return true;
        }
    }
    false
}
fn window_rect(hwnd: Handle) -> Option<Rect> {
    let mut rect = Rect::default();
    (unsafe { GetWindowRect(hwnd, &mut rect) } != 0 && rect.positive()).then_some(rect)
}
fn available_region(game: Game) -> Option<(Rect, u32, bool)> {
    unsafe {
        let hwnd = game.hwnd as Handle;
        let mut client = Rect::default();
        let mut origin = Point::default();
        if GetClientRect(hwnd, &mut client) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return None;
        }
        let client = Rect {
            left: origin.x,
            top: origin.y,
            right: origin.x.saturating_add(client.width()),
            bottom: origin.y.saturating_add(client.height()),
        };
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(MonitorFromWindow(hwnd, 2), &mut info) == 0 {
            return None;
        }
        let dpi = GetDpiForWindow(hwnd).clamp(96, 768);
        choose_region(client.intersection(info.work), info.work, dpi)
            .map(|(region, fallback)| (region, dpi, fallback))
    }
}
fn px(logical: i32, dpi: u32) -> i32 {
    ((logical as i64 * dpi.clamp(96, 768) as i64 + 48) / 96) as i32
}
fn choose_region(game: Rect, work: Rect, dpi: u32) -> Option<(Rect, bool)> {
    let fits = |r: Rect| r.width() >= px(960, dpi) && r.height() >= px(620, dpi);
    if fits(game) {
        Some((game, false))
    } else if fits(work) {
        Some((work, true))
    } else {
        None
    }
}
fn fit_rect(desired: Rect, bounds: Rect, min_size: (i32, i32)) -> Rect {
    let width = desired.width().max(min_size.0).min(bounds.width());
    let height = desired.height().max(min_size.1).min(bounds.height());
    let left = desired.left.clamp(bounds.left, bounds.right - width);
    let top = desired.top.clamp(bounds.top, bounds.bottom - height);
    Rect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}
fn centered_rect(bounds: Rect, dpi: u32) -> Rect {
    let width = px(1100, dpi).min(bounds.width());
    let height = px(740, dpi).min(bounds.height());
    let left = bounds.left + (bounds.width() - width) / 2;
    let top = bounds.top + (bounds.height() - height) / 2;
    Rect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

impl Bridge {
    fn toggle(&self) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if !state.registered {
            return;
        }
        if state.session.as_ref().is_some_and(|s| s.visible) {
            hide_native(self.hwnd as Handle, &mut state, true);
        } else if let Some(game) = foreground_game() {
            open_native(self.hwnd as Handle, &mut state, game);
        } else if state.session.is_some() {
            desktop_native(
                self.hwnd as Handle,
                &mut state,
                "已返回普通窗口，请切回幻兽帕鲁后再呼出",
            );
        } else {
            state.status = "请先切到幻兽帕鲁游戏窗口，再按快捷键呼出".into();
        }
        drop(state);
        self.ctx.request_repaint();
    }
    fn hide(&self, return_game: bool) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            hide_native(self.hwnd as Handle, &mut state, return_game);
        }
        self.ctx.request_repaint();
    }
    fn desktop(&self, status: &str) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            desktop_native(self.hwnd as Handle, &mut state, status);
        }
        self.ctx.request_repaint();
    }
    fn observe(&self) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if state.last_observed.elapsed() < Duration::from_millis(500) {
            return;
        }
        state.last_observed = Instant::now();
        if let Some(session) = &state.session {
            if !session.visible && unsafe { IsWindowVisible(self.hwnd as Handle) } != 0 {
                desktop_native(self.hwnd as Handle, &mut state, "已恢复普通窗口");
                drop(state);
                self.ctx.request_repaint();
                return;
            }
            if session.game_closed {
                return;
            }
            if !game_alive(session.game) {
                game_closed_native(self.hwnd as Handle, &mut state);
                drop(state);
                self.ctx.request_repaint();
                return;
            }
        }
        if let Some(session) = state.session.as_mut().filter(|s| s.visible) {
            if let Some((bounds, dpi, fallback)) = available_region(session.game) {
                session.bounds = bounds;
                session.min_size = (px(960, dpi), px(620, dpi));
                session.fallback = fallback;
                if unsafe { IsZoomed(self.hwnd as Handle) } == 0 {
                    if let Some(rect) = window_rect(self.hwnd as Handle) {
                        let fitted = fit_rect(rect, bounds, session.min_size);
                        if fitted != rect {
                            unsafe {
                                SetWindowPos(
                                    self.hwnd as Handle,
                                    std::ptr::null_mut(),
                                    fitted.left,
                                    fitted.top,
                                    fitted.width(),
                                    fitted.height(),
                                    0x0004 | 0x0010,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
fn open_native(hwnd: Handle, state: &mut State, game: Game) {
    let Some((bounds, dpi, fallback)) = available_region(game) else {
        state.status =
            "游戏窗口和当前屏幕都不足以容纳 960×620 的工具箱，请调整分辨率或显示缩放".into();
        return;
    };
    let min_size = (px(960, dpi), px(620, dpi));
    let new_session = state.session.is_none();
    if state.session.is_none() {
        let mut desktop = Placement {
            length: std::mem::size_of::<Placement>() as u32,
            ..Default::default()
        };
        if unsafe { GetWindowPlacement(hwnd, &mut desktop) } == 0 {
            state.status = "无法保存普通窗口的位置，未进入悬浮模式".into();
            return;
        }
        state.session = Some(Session {
            desktop,
            desktop_topmost: unsafe { GetWindowLongW(hwnd, -20) } & WS_EX_TOPMOST != 0,
            game,
            visible: false,
            overlay_rect: None,
            bounds,
            min_size,
            fallback,
            game_closed: false,
        });
    }
    // Deferred game-exit recovery stops the old timer. Reopening a surviving
    // session for a new game must establish the observer again as well.
    if unsafe { SetTimer(hwnd, TIMER_ID, 1000, std::ptr::null()) } == 0 {
        if new_session {
            state.session = None;
        }
        state.status = "无法创建游戏状态检测，未进入悬浮模式，请重启工具箱重试".into();
        return;
    }
    let Some(session) = state.session.as_mut() else {
        return;
    };
    session.game = game;
    session.bounds = bounds;
    session.min_size = min_size;
    session.fallback = fallback;
    session.game_closed = false;
    let rect = fit_rect(
        session
            .overlay_rect
            .unwrap_or_else(|| centered_rect(bounds, dpi)),
        bounds,
        min_size,
    );
    session.visible = true; // Set before ShowWindow so nested WM_SHOWWINDOW is not an external restore.
    unsafe {
        if IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            0x0040,
        ) == 0
        {
            desktop_native(hwnd, state, "无法显示游戏悬浮窗，已恢复普通窗口");
            return;
        }
        ShowWindow(hwnd, SW_SHOWNORMAL);
        // Native unit tests use private desktops and must not alter the user's
        // current desktop's cursor confinement.
        #[cfg(not(test))]
        ClipCursor(std::ptr::null());
        let focused = SetForegroundWindow(hwnd) != 0;
        state.status = if fallback {
            "游戏窗口较小，已按当前屏幕显示".into()
        } else if !focused {
            "悬浮窗已显示，请点击窗口进行操作".into()
        } else {
            format!(
                "游戏悬浮窗已打开 · {} 收起并返回游戏",
                state.config.hotkey.label()
            )
        };
    }
}
fn hide_native(hwnd: Handle, state: &mut State, return_game: bool) {
    let Some(session) = &mut state.session else {
        return;
    };
    if session.visible && unsafe { IsIconic(hwnd) } == 0 && unsafe { IsZoomed(hwnd) } == 0 {
        session.overlay_rect = window_rect(hwnd);
    }
    session.visible = false;
    unsafe {
        SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0010);
        ShowWindow(hwnd, SW_HIDE);
        if return_game && game_alive(session.game) {
            SetForegroundWindow(session.game.hwnd as Handle);
        }
    }
    state.status = format!(
        "已收起 · 在幻兽帕鲁中按 {} 再次打开",
        state.config.hotkey.label()
    );
}
fn game_closed_native(hwnd: Handle, state: &mut State) {
    let Some(session) = state.session.as_ref() else {
        return;
    };
    if session.visible && unsafe { GetForegroundWindow() } == hwnd {
        desktop_native(hwnd, state, "游戏已关闭，已恢复普通窗口");
    } else {
        // A background game's exit must not show or focus the toolbox over
        // whatever the user switched to. Keep the original placement until an
        // explicit hotkey or launcher activation restores the ordinary window.
        hide_native(hwnd, state, false);
        unsafe { KillTimer(hwnd, TIMER_ID) };
        if let Some(session) = state.session.as_mut() {
            session.game_closed = true;
        }
        state.status = format!(
            "游戏已关闭，按 {} 返回普通窗口",
            state.config.hotkey.label()
        );
    }
}
fn desktop_native(hwnd: Handle, state: &mut State, status: &str) {
    let Some(session) = state.session.take() else {
        return;
    };
    unsafe {
        KillTimer(hwnd, TIMER_ID);
        let mut placement = session.desktop;
        // Reopening a formerly minimized ordinary window must leave a usable window.
        if placement.show_cmd != SW_SHOWMAXIMIZED as u32 {
            placement.show_cmd = SW_SHOWNORMAL as u32;
            placement.flags &= !2; // WPF_RESTORETOMAXIMIZED must not re-minimize/re-maximize it.
        }
        SetWindowPlacement(hwnd, &placement);
        SetWindowPos(
            hwnd,
            if session.desktop_topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            0,
            0,
            0,
            0,
            0x0001 | 0x0002 | 0x0010,
        );
        ShowWindow(
            hwnd,
            if placement.show_cmd == SW_SHOWMAXIMIZED as u32 {
                SW_SHOWMAXIMIZED
            } else {
                SW_SHOWNORMAL
            },
        );
        SetForegroundWindow(hwnd);
    }
    state.status = status.into();
}

unsafe extern "system" fn window_proc(
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    data: usize,
) -> isize {
    if data == 0
        || !matches!(
            message,
            WM_HOTKEY
                | WM_TIMER
                | WM_ACTIVATE
                | WM_SHOWWINDOW
                | WM_SIZE
                | WM_MOVING
                | WM_SIZING
                | WM_NCDESTROY
        )
    {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    // Keep a temporary independent reference across synchronous native re-entry.
    let pointer = data as *const Bridge;
    let bridge = unsafe {
        Rc::increment_strong_count(pointer);
        Rc::from_raw(pointer)
    };
    if message == WM_NCDESTROY {
        bridge.alive.set(false);
        if let Ok(mut state) = bridge.state.try_borrow_mut() {
            state.registered = false;
            state.session = None;
        }
        unsafe {
            KillTimer(hwnd, TIMER_ID);
            UnregisterHotKey(hwnd, HOTKEY_ID);
            RemoveWindowSubclass(hwnd, window_proc, id);
            if bridge.callback_ref.replace(false) {
                drop(Rc::from_raw(pointer));
            }
        }
    } else if message == WM_HOTKEY && wparam == HOTKEY_ID as usize {
        bridge.toggle();
        return 0;
    } else if message == WM_TIMER && wparam == TIMER_ID {
        // One native message per second only during a session. No redraw for a
        // live game: hidden overlays do not run a paint or polling thread.
        if let Ok(mut state) = bridge.state.try_borrow_mut() {
            if state
                .session
                .as_ref()
                .is_some_and(|s| !s.game_closed && !game_alive(s.game))
            {
                game_closed_native(hwnd, &mut state);
                drop(state);
                bridge.ctx.request_repaint();
            }
        }
        return 0;
    } else if message == WM_ACTIVATE && wparam & 0xffff == 0 {
        if let Ok(mut state) = bridge.state.try_borrow_mut() {
            if state.config.auto_hide
                && state.session.as_ref().is_some_and(|s| s.visible)
                && !own_window(lparam as Handle, hwnd)
            {
                hide_native(hwnd, &mut state, false);
                drop(state);
                bridge.ctx.request_repaint();
            }
        }
    } else if message == WM_SHOWWINDOW && wparam != 0 {
        if let Ok(mut state) = bridge.state.try_borrow_mut() {
            if state.session.as_ref().is_some_and(|s| !s.visible) {
                desktop_native(hwnd, &mut state, "已恢复普通窗口");
                drop(state);
                bridge.ctx.request_repaint();
            }
        }
    } else if message == WM_SIZE && wparam == 1 {
        if let Ok(mut state) = bridge.state.try_borrow_mut() {
            if state.session.as_ref().is_some_and(|s| s.visible) {
                hide_native(hwnd, &mut state, false);
                drop(state);
                bridge.ctx.request_repaint();
            }
        }
    } else if matches!(message, WM_MOVING | WM_SIZING) && lparam != 0 {
        if let Ok(state) = bridge.state.try_borrow() {
            if let Some(session) = state.session.as_ref().filter(|s| s.visible) {
                let rect = unsafe { &mut *(lparam as *mut Rect) };
                *rect = fit_rect(*rect, session.bounds, session.min_size);
                return 1;
            }
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[repr(C)]
    #[derive(Default)]
    struct NativeMessage {
        hwnd: Handle,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: Point,
        private: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn CreateWindowExW(
            ex: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: Handle,
            menu: Handle,
            instance: Handle,
            param: *const c_void,
        ) -> Handle;
        fn DestroyWindow(hwnd: Handle) -> i32;
        fn CreateDesktopW(
            name: *const u16,
            device: *const u16,
            mode: *const c_void,
            flags: u32,
            access: u32,
            security: *const c_void,
        ) -> Handle;
        fn SetThreadDesktop(desktop: Handle) -> i32;
        fn GetThreadDesktop(thread: u32) -> Handle;
        fn CloseDesktop(desktop: Handle) -> i32;
        fn SendMessageW(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
        fn PeekMessageW(
            message: *mut NativeMessage,
            hwnd: Handle,
            first: u32,
            last: u32,
            remove: u32,
        ) -> i32;
        fn DispatchMessageW(message: *const NativeMessage) -> isize;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    fn invisible_window() -> Handle {
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                960,
                620,
                -3isize as Handle,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        hwnd
    }
    #[test]
    fn defaults_and_hotkey_settings_are_stable_and_do_not_register_input() {
        assert_eq!(
            Config::default(),
            Config {
                enabled: false,
                hotkey: Hotkey::AltJ,
                auto_hide: true
            }
        );
        for key in [Hotkey::AltJ, Hotkey::CtrlShiftJ, Hotkey::F8, Hotkey::F9] {
            assert_eq!(Hotkey::from_key(key.key()), Some(key));
            assert_eq!(Hotkey::from_key(key.label()), Some(key));
            assert_ne!(key.native().0 & 0x4000, 0);
        }
        assert_eq!(Hotkey::from_key("CtrlShiftJ"), Some(Hotkey::CtrlShiftJ));
        assert_eq!(Hotkey::from_key("Win + L"), None);
        let overlay = Overlay::default();
        assert!(!overlay.snapshot().registered && !overlay.snapshot().session);
    }
    #[test]
    fn game_detection_is_exact_and_case_insensitive() {
        assert!(game_basename(
            r"D:\Games\Palworld\Palworld-Win64-Shipping.exe"
        ));
        assert!(game_basename("C:/XboxGames/PALWORLD-WINGDK-SHIPPING.EXE"));
        assert!(!game_basename("Palworld.exe"));
        assert!(!game_basename("Palworld-Win64-Shipping-Cmd.exe"));
        assert!(!game_basename("OtherPalworld-Win64-Shipping.exe"));
    }
    #[test]
    fn monitor_fallback_and_negative_coordinate_bounds_preserve_minimum_size() {
        let game = Rect {
            left: -1700,
            top: 30,
            right: -100,
            bottom: 1030,
        };
        let work = Rect {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        assert_eq!(choose_region(game, work, 144), Some((game, false)));
        let tiny = Rect {
            left: -1000,
            top: 30,
            right: -100,
            bottom: 600,
        };
        assert_eq!(choose_region(tiny, work, 144), Some((work, true)));
        assert_eq!(choose_region(tiny, tiny, 144), None);
        for dpi in [96, 120, 144, 192] {
            let bounds = Rect {
                left: -5000,
                top: -2000,
                right: 0,
                bottom: 3000,
            };
            let desired = Rect {
                left: 100,
                top: 100,
                right: 101,
                bottom: 101,
            };
            let fit = fit_rect(desired, bounds, (px(960, dpi), px(620, dpi)));
            assert_eq!(fit.width(), px(960, dpi));
            assert_eq!(fit.height(), px(620, dpi));
            assert!(
                fit.left >= bounds.left
                    && fit.right <= bounds.right
                    && fit.top >= bounds.top
                    && fit.bottom <= bounds.bottom
            );
        }
    }
    #[test]
    fn subclass_reference_is_released_exactly_once_on_destroy_and_detach() {
        let hwnd = invisible_window();
        let binding =
            Binding::new(egui::Context::default(), hwnd as isize, Config::default()).unwrap();
        let observer = Rc::downgrade(&binding.bridge);
        assert_eq!(Rc::strong_count(&binding.bridge), 2);
        unsafe {
            assert_ne!(DestroyWindow(hwnd), 0);
        }
        assert!(!binding.bridge.alive.get() && !binding.bridge.callback_ref.get());
        assert_eq!(Rc::strong_count(&binding.bridge), 1);
        drop(binding);
        assert!(observer.upgrade().is_none());

        let hwnd = invisible_window();
        let binding =
            Binding::new(egui::Context::default(), hwnd as isize, Config::default()).unwrap();
        let observer = Rc::downgrade(&binding.bridge);
        drop(binding);
        assert!(observer.upgrade().is_none());
        unsafe {
            assert_ne!(DestroyWindow(hwnd), 0);
        }
    }
    #[test]
    #[ignore = "Creates actual test windows on a private Win32 desktop; never registers a global hotkey"]
    fn private_desktop_native_session_restores_window_and_recovers_after_game_exit() {
        // A new UI thread can select a private desktop before creating HWNDs.
        std::thread::spawn(|| unsafe {
            struct Desktop(Handle, Handle);
            impl Drop for Desktop {
                fn drop(&mut self) {
                    unsafe {
                        SetThreadDesktop(self.1);
                        CloseDesktop(self.0);
                    }
                }
            }
            struct Window(Handle);
            impl Drop for Window {
                fn drop(&mut self) {
                    unsafe {
                        if IsWindow(self.0) != 0 {
                            DestroyWindow(self.0);
                        }
                    }
                }
            }
            let original = GetThreadDesktop(GetCurrentThreadId());
            let name: Vec<u16> = format!(
                "JadeOverlayNativeTest-{}-{}",
                std::process::id(),
                GetCurrentThreadId()
            )
            .encode_utf16()
            .chain(Some(0))
            .collect();
            let handle = CreateDesktopW(
                name.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                0x01ff,
                std::ptr::null(),
            );
            assert!(!handle.is_null());
            let _desktop = Desktop(handle, original);
            assert_ne!(SetThreadDesktop(handle), 0);
            let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
            let make = |x, y, width, height| {
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0x8000_0000,
                    x,
                    y,
                    width,
                    height,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                );
                assert!(!hwnd.is_null());
                Window(hwnd)
            };
            let game_window = make(50, 50, 1920, 1080);
            let tool_window = make(300, 200, 1200, 800);
            ShowWindow(game_window.0, SW_SHOWNORMAL);
            ShowWindow(tool_window.0, SW_SHOWNORMAL);
            let game = Game {
                hwnd: game_window.0 as isize,
                pid: window_pid(game_window.0),
            };
            let binding = Binding::new(
                egui::Context::default(),
                tool_window.0 as isize,
                Config::default(),
            )
            .unwrap();
            let mut saved = Placement {
                length: std::mem::size_of::<Placement>() as u32,
                ..Default::default()
            };
            assert_ne!(GetWindowPlacement(tool_window.0, &mut saved), 0);
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, game);
                assert!(state.session.as_ref().is_some_and(|s| s.visible));
            }
            assert_ne!(IsWindowVisible(tool_window.0), 0);
            assert_ne!(GetWindowLongW(tool_window.0, -20) & WS_EX_TOPMOST, 0);
            binding.bridge.hide(true);
            assert_eq!(IsWindowVisible(tool_window.0), 0);
            assert_eq!(GetWindowLongW(tool_window.0, -20) & WS_EX_TOPMOST, 0);
            assert!(
                binding
                    .bridge
                    .state
                    .borrow()
                    .session
                    .as_ref()
                    .is_some_and(|s| !s.visible)
            );
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, game);
            }
            assert_ne!(IsWindowVisible(tool_window.0), 0);
            binding.bridge.desktop("test desktop");
            let mut restored = Placement {
                length: std::mem::size_of::<Placement>() as u32,
                ..Default::default()
            };
            assert_ne!(GetWindowPlacement(tool_window.0, &mut restored), 0);
            assert_eq!(restored.normal, saved.normal);
            assert_eq!(GetWindowLongW(tool_window.0, -20) & WS_EX_TOPMOST, 0);
            assert!(binding.bridge.state.borrow().session.is_none());

            // A previously maximized ordinary window restores maximized.
            ShowWindow(tool_window.0, SW_SHOWMAXIMIZED);
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, game);
            }
            assert_eq!(IsZoomed(tool_window.0), 0);
            binding.bridge.desktop("test maximized");
            assert_ne!(IsZoomed(tool_window.0), 0);

            // A previously minimized ordinary window comes back usable/normal.
            ShowWindow(tool_window.0, 2);
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, game);
            }
            binding.bridge.desktop("test minimized");
            assert_eq!(IsIconic(tool_window.0), 0);
            assert_eq!(IsZoomed(tool_window.0), 0);
            assert_ne!(IsWindowVisible(tool_window.0), 0);

            // External launcher/taskbar activation restores the ordinary window.
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, game);
            }
            binding.bridge.hide(false);
            ShowWindow(tool_window.0, SW_RESTORE);
            assert!(binding.bridge.state.borrow().session.is_none());

            // A hidden game's exit must stay hidden and preserve the original
            // placement until the user explicitly returns to the desktop.
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, game);
            }
            binding.bridge.hide(false);
            assert_ne!(DestroyWindow(game_window.0), 0);
            SendMessageW(tool_window.0, WM_TIMER, TIMER_ID, 0);
            assert!(
                binding
                    .bridge
                    .state
                    .borrow()
                    .session
                    .as_ref()
                    .is_some_and(|s| s.game_closed && !s.visible)
            );
            assert_eq!(IsWindowVisible(tool_window.0), 0);
            assert_eq!(GetWindowLongW(tool_window.0, -20) & WS_EX_TOPMOST, 0);
            assert_eq!(
                KillTimer(tool_window.0, TIMER_ID),
                0,
                "deferred exit must stop its timer"
            );

            // The same session may be reopened for a new game before the
            // ordinary window is restored; it needs a new real native timer.
            let replacement_window = make(50, 50, 1920, 1080);
            ShowWindow(replacement_window.0, SW_SHOWNORMAL);
            let replacement = Game {
                hwnd: replacement_window.0 as isize,
                pid: window_pid(replacement_window.0),
            };
            {
                let mut state = binding.bridge.state.borrow_mut();
                open_native(tool_window.0, &mut state, replacement);
                assert!(
                    state
                        .session
                        .as_ref()
                        .is_some_and(|s| s.visible && !s.game_closed)
                );
            }
            binding.bridge.hide(false);
            assert_ne!(DestroyWindow(replacement_window.0), 0);
            let mut saw_timer = false;
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline && !saw_timer {
                let mut message = NativeMessage::default();
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, 1) != 0 {
                    saw_timer |= message.hwnd == tool_window.0
                        && message.message == WM_TIMER
                        && message.wparam == TIMER_ID;
                    DispatchMessageW(&message);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                saw_timer,
                "reopened session must restart its native observer"
            );
            assert_eq!(IsWindowVisible(tool_window.0), 0);
            assert!(
                binding
                    .bridge
                    .state
                    .borrow()
                    .session
                    .as_ref()
                    .is_some_and(|s| s.game_closed)
            );
            binding.bridge.desktop("explicit desktop after game exit");
            assert!(binding.bridge.state.borrow().session.is_none());
            assert_ne!(IsWindowVisible(tool_window.0), 0);
            let mut restored = Placement {
                length: std::mem::size_of::<Placement>() as u32,
                ..Default::default()
            };
            assert_ne!(GetWindowPlacement(tool_window.0, &mut restored), 0);
            assert_eq!(restored.normal, saved.normal);
            drop(binding);
        })
        .join()
        .unwrap();
    }
}
