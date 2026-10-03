use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use std::sync::OnceLock;

const REMINDER_W: f64 = 392.0;
const REMINDER_MIN_H: f64 = 120.0;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDef {
    pub id: String,
    pub label: String,
}

impl ActionDef {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// 提醒卡内容包：一条 = 一张卡（多卡由前端栈维护）
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    pub id: String,
    /// rule | task
    pub kind: String,
    pub ref_id: i64,
    pub title: String,
    pub body: String,
    pub sticky: bool,
    pub duration_ms: u64,
    pub accent: Option<String>,
    pub actions: Vec<ActionDef>,
    /// card | fullscreen（全屏提醒方式）
    pub style: String,
}

impl Payload {
    pub fn new(kind: &str, ref_id: i64, title: &str, body: &str) -> Self {
        Self {
            id: format!("{kind}-{ref_id}-{}", chrono::Local::now().timestamp_millis()),
            kind: kind.into(),
            ref_id,
            title: title.into(),
            body: body.into(),
            sticky: false,
            duration_ms: 10_000,
            accent: None,
            actions: vec![ActionDef::new("ack", "知道了")],
            style: "card".into(),
        }
    }
}

/// 待取队列：提醒窗 webview 尚未就绪时先攒着，前端加载完主动拉取（确定性投递）
static PENDING: OnceLock<std::sync::Mutex<Vec<Payload>>> = OnceLock::new();

fn push_pending(p: Payload) {
    let q = PENDING.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut q) = q.lock() {
        // 同源去重：kind+ref_id 相同的旧卡原地刷新
        q.retain(|x| !(x.kind == p.kind && x.ref_id == p.ref_id));
        q.push(p);
        if q.len() > 8 {
            q.remove(0);
        }
    }
}

pub fn take_pending() -> Vec<Payload> {
    let q = PENDING.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    q.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

/// 弹出右下角提醒小窗；窗口已存在则直接追加卡片
pub fn show(app: &AppHandle, payload: Payload) {
    eprintln!("[reminder] show kind={} ref={}", payload.kind, payload.ref_id);
    if payload.style == "fullscreen" {
        show_fullscreen(app, payload);
        return;
    }
    if let Some(win) = app.get_webview_window("reminder") {
        eprintln!("[reminder] window exists, emit to webview");
        let _ = win.show();
        let _ = app.emit_to("reminder", "reminder-show", payload);
        return;
    }
    eprintln!("[reminder] no window, push pending + create");
    push_pending(payload);

    let (x, y) = bottom_right(app, REMINDER_MIN_H);
    let build = WebviewWindowBuilder::new(app, "reminder", WebviewUrl::App("index.html".into()))
        .title("拾刻提醒")
        .inner_size(REMINDER_W, REMINDER_MIN_H)
        .position(x, y)
        .decorations(false)
        // 卡片铺满窗口、窗口不透明：透明合成在部分机器（Win11 24H2）上不可靠，
        // 留白处会露出 WebView 画布底色（浅色=灰框、深色=黑框）。底色直接给卡片色，
        // 即使 WebView 尚未渲染也只是"一块深色卡片"，不会再出现黑/灰框。
        .transparent(false)
        .background_color(tauri::window::Color(22, 26, 34, 255))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(true)
        .visible(false);

    match build.build() {
        Ok(win) => {
            kill_border(&win);
            // 圆角区域只贴卡片窗；全屏窗走 show_fullscreen 自己的路径，不贴区域
            round_region(&win, CARD_RADIUS);
        }
        Err(e) => eprintln!("[reminder] 创建提醒窗口失败: {e}"),
    }
}

/// 前端按卡片数量上报内容高度，窗口随之缩放并保持右下角锚定
/// 去掉 Windows 给窗口画的 1px DWM 边框；圆角交给系统（窗口 = 卡片本身）
#[cfg(windows)]
fn kill_border(win: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
    };
    if let Ok(hwnd) = win.hwnd() {
        // DWMWA_COLOR_NONE = 0xFFFFFFFE；圆角偏好 DWMWCP_DONOTROUND = 1
        // （卡片窗的圆角由 SetWindowRgn 按 14px 裁剪，DWM 的 8px 圆角会跟区域取小打架；
        //   注意这里只管去边框，区域裁剪只在卡片窗路径里做，全屏窗不贴区域）
        let none: u32 = 0xFFFF_FFFE;
        let no_round: u32 = 1;
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
                &no_round as *const _ as *const std::ffi::c_void,
                4,
            );
        }
    }
}

/// 卡片圆角（逻辑 px），与 ToastStack 的窗口观感一致
const CARD_RADIUS: f64 = 14.0;

/// 窗口区域裁剪成圆角矩形（SetWindowRgn，同原子岛的灰框根治思路）：
/// 形状之外不存在窗口——彻底杜绝角落残留底色，DWM 投影也会贴合圆角。
/// ⚠️ 岛的教训必须先做：tao 顶层无边框窗口样式表里保留 WS_CAPTION，
/// SetWindowRgn(bRedraw) 会触发非客户区重绘把它画出来（=用户实拍的白色边框），
/// 所以贴区域前必须先 strip_caption。
#[cfg(windows)]
fn round_region(win: &tauri::WebviewWindow, radius: f64) {
    let Ok(hwnd) = win.hwnd() else { return };
    crate::island::strip_caption(win);
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateRoundRectRgn, SetWindowRgn,
    };
    let Ok(sc) = win.scale_factor() else { return };
    let Ok(size) = win.inner_size() else { return };
    let (w, h) = (size.width as i32, size.height as i32);
    let e = (2.0 * radius * sc).round() as i32;
    unsafe {
        let rgn = CreateRoundRectRgn(0, 0, w, h, e.max(2), e.max(2));
        let _ = SetWindowRgn(HWND(hwnd.0), Some(rgn), true);
    }
}

#[cfg(not(windows))]
fn round_region(_win: &tauri::WebviewWindow, _radius: f64) {}

#[cfg(not(windows))]
fn kill_border(_win: &tauri::WebviewWindow) {}

/// 全屏提醒：铺满主屏的置顶无边框窗口（"该休息了"那一类）
/// 窗口先隐藏、等前端就绪再显示：睡眠唤醒后 WebView 首帧可能迟迟不渲染，
/// 立即显示会露出默认底色（半夜"全黑一片+滚动条"的来源）
fn show_fullscreen(app: &AppHandle, payload: Payload) {
    if let Some(win) = app.get_webview_window("reminder_full") {
        FULL_SHOWN.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit_to("reminder_full", "reminder-show", payload);
        return;
    }
    push_pending(payload);
    let (w, h) = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let sc = m.scale_factor();
            (m.size().width as f64 / sc, m.size().height as f64 / sc)
        })
        .unwrap_or((1280.0, 800.0));
    FULL_SHOWN.store(false, std::sync::atomic::Ordering::Relaxed);
    let build = WebviewWindowBuilder::new(app, "reminder_full", WebviewUrl::App("index.html".into()))
        .title("拾刻 · 休息提醒")
        .inner_size(w, h)
        .position(0.0, 0.0)
        .decorations(false)
        .transparent(false)
        // 与全屏页底色一致：即使首帧未渲染也不会黑屏闪变
        .background_color(tauri::window::Color(13, 16, 22, 255))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build();
    match build {
        Ok(win) => {
            kill_border(&win);
            let handle = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(60));
                if FULL_SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if let Some(w) = handle.get_webview_window("reminder_full") {
                    eprintln!("[reminder] 全屏提醒窗 60s 未就绪，销毁等待下次重建");
                    let _ = w.close();
                }
            });
        }
        Err(e) => eprintln!("[reminder] 全屏提醒窗创建失败: {e}"),
    }
}

/// 全屏提醒窗前端就绪后由 Rust 侧显示
pub fn show_full_ready(app: &AppHandle) {
    FULL_SHOWN.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(win) = app.get_webview_window("reminder_full") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

static FULL_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 关闭全屏提醒窗
pub fn close_full(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("reminder_full") {
        let _ = win.hide();
    }
}

pub fn resize(app: &AppHandle, content_height: f64) {
    if let Some(win) = app.get_webview_window("reminder") {
        // 卡片铺满窗口：窗口高度 = 内容高度（不再留 24px 余量，那正是灰框的温床）
        let h = content_height.max(REMINDER_MIN_H);
        let _ = win.set_size(tauri::LogicalSize::new(REMINDER_W, h));
        // 高度变了，圆角区域跟着重贴
        round_region(&win, CARD_RADIUS);
        if let Ok(Some(m)) = app.primary_monitor() {
            let scale = m.scale_factor();
            let size = m.size();
            let x = size.width as f64 / scale - REMINDER_W - 16.0;
            let y = size.height as f64 / scale - h - 48.0;
        if let Ok(pos) = win.outer_position() {
            if pos.x != x as i32 || pos.y != y as i32 {
                let _ = win.set_position(tauri::LogicalPosition::new(x, y));
            }
        }
        }
    }
}

pub fn close(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("reminder") {
        let _ = win.close();
    }
}

/// 主屏右下角（逻辑像素，避开托盘区）
fn bottom_right(app: &AppHandle, height: f64) -> (f64, f64) {
    if let Ok(Some(m)) = app.primary_monitor() {
        let scale = m.scale_factor();
        let size = m.size();
        let lw = size.width as f64 / scale;
        let lh = size.height as f64 / scale;
        return (lw - REMINDER_W - 16.0, lh - height - 48.0);
    }
    (900.0, 600.0)
}
