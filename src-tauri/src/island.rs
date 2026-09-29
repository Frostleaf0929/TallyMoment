//! 原子岛：常驻置顶胶囊窗（不抢焦点、不进任务栏），
//! 显示 当前专注 / 下个任务倒计时 / 今日完成数，点击展开待办卡。
//! 默认关闭（个性化里开）；拖动后位置持久化；窗口不透明 + DWM 圆角
//! （透明合成不可靠——提醒窗的教训）。
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const ISLAND_W: f64 = 392.0;
pub const ISLAND_H_COLLAPSED: f64 = 56.0;
pub const ISLAND_H_EXPANDED: f64 = 316.0;

fn logical_screen(app: &AppHandle) -> (f64, f64) {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let sc = m.scale_factor();
            (m.size().width as f64 / sc, m.size().height as f64 / sc)
        })
        .unwrap_or((1280.0, 800.0))
}

/// 读取开关
pub fn enabled(conn: &rusqlite::Connection) -> bool {
    crate::storage::get_setting(conn, "island.enabled")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// 创建或显示原子岛；位置用保存的，没有则屏幕顶部居中
pub fn show(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.show();
        return;
    }
    let (sw, _sh) = logical_screen(app);
    let (x, y) = saved_pos(app).unwrap_or(((sw - ISLAND_W) / 2.0, 8.0));
    let expanded = with_setting(app, |conn| {
        crate::storage::get_setting(conn, "island.expanded")
            .map(|v| v == "1")
            .unwrap_or(false)
    })
    .unwrap_or(false);
    let h = if expanded { ISLAND_H_EXPANDED } else { ISLAND_H_COLLAPSED };

    let build = WebviewWindowBuilder::new(app, "island", WebviewUrl::App("index.html".into()))
        .title("拾刻 · 原子岛")
        .inner_size(ISLAND_W, h)
        .position(x.max(0.0), y.max(0.0))
        .decorations(false)
        .transparent(false)
        .background_color(tauri::window::Color(16, 17, 21, 255))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false);

    match build.build() {
        Ok(win) => {
            round_corners(&win);
            eprintln!("[island] 窗口已创建");
        }
        Err(e) => eprintln!("[island] 创建失败: {e}"),
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.hide();
    }
}

/// 前端就绪：显示窗口（创建时隐藏，防首帧黑底闪现）
pub fn on_ready(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.show();
    }
}

/// 展开/收起（保持顶边不动），并记住状态
pub fn set_expanded(app: &AppHandle, expanded: bool) {
    let h = if expanded { ISLAND_H_EXPANDED } else { ISLAND_H_COLLAPSED };
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.set_size(tauri::LogicalSize::new(ISLAND_W, h));
    }
    with_setting(app, |conn| {
        crate::storage::set_setting(conn, "island.expanded", if expanded { "1" } else { "0" })
    });
}

/// 位置重置：回屏幕顶部居中
pub fn reset_pos(app: &AppHandle) {
    let (sw, _sh) = logical_screen(app);
    let x = (sw - ISLAND_W) / 2.0;
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.set_position(tauri::LogicalPosition::new(x, 8.0));
    }
    with_setting(app, |conn| {
        crate::storage::set_setting(conn, "island.pos", &format!("{x:.0},8"))
    });
}

/// 窗口移动后保存位置（逻辑像素）
pub fn save_pos(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        if let Ok(pos) = win.outer_position() {
            let sc = win.scale_factor().unwrap_or(1.0);
            let (x, y) = (pos.x as f64 / sc, pos.y as f64 / sc);
            with_setting(app, |conn| {
                crate::storage::set_setting(conn, "island.pos", &format!("{x:.0},{y:.0}"))
            });
        }
    }
}

fn saved_pos(app: &AppHandle) -> Option<(f64, f64)> {
    let v = with_setting(app, |conn| crate::storage::get_setting(conn, "island.pos"))??;
    let (x, y) = v.split_once(',')?;
    let x: f64 = x.trim().parse().ok()?;
    let y: f64 = y.trim().parse().ok()?;
    Some((x, y))
}

/// 在 Db 锁内执行设置读写（island.rs 各函数共用；测试进程无 state 时返回 None）
fn with_setting<T>(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> T) -> Option<T> {
    let db = app.try_state::<crate::Db>()?;
    let guard = db.0.lock().ok()?;
    Some(f(&guard))
}

/// DWM 圆角（窗口 = 胶囊本体，与提醒窗同款做法）
#[cfg(windows)]
fn round_corners(win: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
    };
    if let Ok(hwnd) = win.hwnd() {
        let none: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
        let round: u32 = 2; // DWMWCP_ROUND
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd.0),
                DWMWA_BORDER_COLOR,
                &none as *const _ as *const std::ffi::c_void,
                4,
            );
            let _ = DwmSetWindowAttribute(
                HWND(hwnd.0),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &round as *const _ as *const std::ffi::c_void,
                4,
            );
        }
    }
}

#[cfg(not(windows))]
fn round_corners(_win: &tauri::WebviewWindow) {}
