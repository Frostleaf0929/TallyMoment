use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const REMINDER_W: f64 = 380.0;
const REMINDER_H: f64 = 150.0;

/// 弹出右下角提醒小窗；窗口已存在则直接换内容置前
pub fn show(app: &AppHandle, title: &str, message: &str) {
    let payload = json!({ "title": title, "message": message });
    if let Some(win) = app.get_webview_window("reminder") {
        let _ = win.show();
        let _ = app.emit_to("reminder", "reminder-show", payload);
        return;
    }

    let (x, y) = bottom_right(app);
    let build = WebviewWindowBuilder::new(app, "reminder", WebviewUrl::App("index.html".into()))
        .title("拾刻提醒")
        .inner_size(REMINDER_W, REMINDER_H)
        .position(x, y)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false);

    if let Err(e) = build.build() {
        eprintln!("[reminder] 创建提醒窗口失败: {e}");
        return;
    }

    // 窗口隐藏创建，前端就绪并收到事件后自行 show；这里重试推送避免错过
    let app2 = app.clone();
    std::thread::spawn(move || {
        for delay in [400, 900, 1600, 2500] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            if app2
                .emit_to("reminder", "reminder-show", payload.clone())
                .is_ok()
            {
                break;
            }
        }
    });
}

pub fn close(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("reminder") {
        let _ = win.close();
    }
}

/// 主屏右下角（逻辑像素，避开托盘区）
fn bottom_right(app: &AppHandle) -> (f64, f64) {
    if let Ok(Some(m)) = app.primary_monitor() {
        let scale = m.scale_factor();
        let size = m.size();
        let lw = size.width as f64 / scale;
        let lh = size.height as f64 / scale;
        return (lw - REMINDER_W - 16.0, lh - REMINDER_H - 48.0);
    }
    (900.0, 600.0)
}
