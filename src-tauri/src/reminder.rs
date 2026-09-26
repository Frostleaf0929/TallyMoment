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
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false);

    if let Err(e) = build.build() {
        eprintln!("[reminder] 创建提醒窗口失败: {e}");
        return;
    }
}

/// 前端按卡片数量上报内容高度，窗口随之缩放并保持右下角锚定
pub fn resize(app: &AppHandle, content_height: f64) {
    if let Some(win) = app.get_webview_window("reminder") {
        let h = (content_height + 24.0).max(REMINDER_MIN_H);
        let _ = win.set_size(tauri::LogicalSize::new(REMINDER_W, h));
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
