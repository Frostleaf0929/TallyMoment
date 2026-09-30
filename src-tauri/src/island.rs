//! 原子岛：常驻置顶胶囊窗（不抢焦点、不进任务栏），
//! 显示 当前专注 / 下个任务倒计时 / 今日完成数，点击展开待办卡。
//! 默认关闭（个性化里开）；拖动后位置持久化；窗口不透明 + DWM 圆角
//! （透明合成不可靠——提醒窗的教训）。
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const ISLAND_W: f64 = 392.0;
pub const ISLAND_H_COLLAPSED: f64 = 56.0;
pub const ISLAND_H_EXPANDED: f64 = 316.0;
/// WinIsland 式三态：idle（无悬停缩小）/ normal（悬停完整）/ expanded（点击展开）
pub const ISLAND_W_IDLE: f64 = 240.0;
pub const ISLAND_H_IDLE: f64 = 40.0;

/// 动画代数：新动画开始时 +1，旧动画线程发现代数不一致即自行退出（防鼠标快速进出堆叠）
static ANIM_GEN: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// 主屏逻辑几何：(原点x, 原点y, 宽, 高)——多屏时原点非 0,0，定位必须加偏移
pub fn logical_screen(app: &AppHandle) -> (f64, f64, f64, f64) {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let sc = m.scale_factor();
            let pos = m.position();
            let size = m.size();
            (
                pos.x as f64 / sc,
                pos.y as f64 / sc,
                size.width as f64 / sc,
                size.height as f64 / sc,
            )
        })
        .unwrap_or((0.0, 0.0, 1280.0, 800.0))
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
    let (mx, _my, sw, _sh) = logical_screen(app);
    // 位置模式：有手动拖动记录视为 custom，否则默认顶部居中
    let pm = with_setting(app, |conn| pos_mode(conn)).unwrap_or_else(|| "center".into());
    let (x, y) = if pm == "custom" {
        saved_pos(app).unwrap_or((mx + (sw - ISLAND_W) / 2.0, 8.0))
    } else {
        let px = match pm.as_str() {
            "left" => mx + 8.0,
            "right" => mx + sw - ISLAND_W - 8.0,
            _ => mx + (sw - ISLAND_W) / 2.0,
        };
        (px, 8.0)
    };
    // 初始一律 normal 高度；idle/expanded 由前端状态机切换
    let h = ISLAND_H_COLLAPSED;
    let mat = with_setting(app, |conn| material(conn)).unwrap_or_else(|| "solid".into());

    let mut build = WebviewWindowBuilder::new(app, "island", WebviewUrl::App("island.html".into()))
        .title("拾刻 · 原子岛")
        .inner_size(ISLAND_W, h)
        .position(x.max(0.0), y.max(0.0))
        .decorations(false)
        .background_color(tauri::window::Color(16, 17, 21, 255))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false);
    build = match effects_for(&mat) {
        Some(cfg) => build.transparent(true).effects(cfg),
        None => build.transparent(false),
    };

    match build.build() {
        Ok(win) => {
            round_corners(&win);
            eprintln!("[island] 窗口已创建");
            // 兜底：前端就绪信号 2.5 秒内没来（加载失败/事件丢失）也强制显示，
            // 宁可短暂黑底也不能"开关打开了却什么都不出现"
            let handle = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(2500));
                if let Some(w) = handle.get_webview_window("island") {
                    match w.is_visible() {
                        Ok(false) => {
                            eprintln!("[island] 就绪信号超时，强制显示");
                            let _ = w.show();
                        }
                        _ => {}
                    }
                }
            });
        }
        Err(e) => eprintln!("[island] 创建失败: {e}"),
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.hide();
    }
}

/// 材质切换需重建窗口（系统效果不能热切换）：关闭后按新设置重开
pub fn rebuild(app: &AppHandle) {
    eprintln!("[island] rebuild: 关闭旧窗口并按新材质重建");
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.close();
        for _ in 0..60 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            if app.get_webview_window("island").is_none() {
                break;
            }
        }
    }
    show(app);
}

/// 前端就绪：显示窗口（创建时隐藏，防首帧黑底闪现）
pub fn on_ready(app: &AppHandle) {
    eprintln!("[island] 前端就绪，显示窗口");
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.show();
    }
}

/// 模块配置：胶囊条显示哪些信息、什么顺序（settings 里存 JSON 数组）
pub fn modules(conn: &rusqlite::Connection) -> Vec<String> {
    const DEFAULT: [ &str; 3 ] = ["focus", "next", "done"];
    match crate::storage::get_setting(conn, "island.modules") {
        Some(v) => {
            let arr: Vec<String> = serde_json::from_str(&v).unwrap_or_default();
            if arr.is_empty() {
                DEFAULT.iter().map(|s| s.to_string()).collect()
            } else {
                arr
            }
        }
        None => DEFAULT.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn set_modules(conn: &rusqlite::Connection, modules: &[String]) {
    let _ = crate::storage::set_setting(
        conn,
        "island.modules",
        &serde_json::to_string(modules).unwrap_or_else(|_| "[]".into()),
    );
}

/// 个性化改完模块配置后通知岛刷新（island.ts 监听）
pub fn notify_modules(app: &AppHandle, modules: &[String]) {
    use tauri::Emitter;
    let _ = app.emit_to("island", "island-modules", modules.to_vec());
}

/// 窗口三态切换：带插值动画（整体放大、水平中心对齐、顶边不动）
/// 目标位置按位置模式：center/left/right 对齐屏幕，custom 以当前中心为锚对称扩展
pub fn set_state(app: &AppHandle, state: &str) {
    let (tw, th) = match state {
        "idle" => (ISLAND_W_IDLE, ISLAND_H_IDLE),
        "expanded" => (ISLAND_W, ISLAND_H_EXPANDED),
        _ => (ISLAND_W, ISLAND_H_COLLAPSED),
    };
    let Some(win) = app.get_webview_window("island") else { return };
    let sc = win.scale_factor().unwrap_or(1.0);
    let (cx, cy, cw, ch) = (
        win.outer_position().ok().map(|p| p.x as f64 / sc).unwrap_or(0.0),
        win.outer_position().ok().map(|p| p.y as f64 / sc).unwrap_or(8.0),
        win.inner_size().ok().map(|s| s.width as f64 / sc).unwrap_or(ISLAND_W),
        win.inner_size().ok().map(|s| s.height as f64 / sc).unwrap_or(ISLAND_H_COLLAPSED),
    );
    let pm = with_setting(app, |conn| pos_mode(conn)).unwrap_or_else(|| "center".into());
    let (mx, _my, sw, _sh) = logical_screen(app);
    let (tx, ty) = match pm.as_str() {
        "left" => (mx + 8.0, 8.0),
        "right" => (mx + sw - tw - 8.0, 8.0),
        "center" => (mx + (sw - tw) / 2.0, 8.0),
        _ => (cx + (cw - tw) / 2.0, cy.min(8.0)), // custom：水平中心锚定，顶边不动
    };
    if (cw - tw).abs() < 0.5 && (ch - th).abs() < 0.5 {
        let _ = win.set_size(tauri::LogicalSize::new(tw, th));
        let _ = win.set_position(tauri::LogicalPosition::new(tx, ty));
        return;
    }
    let gen = ANIM_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    std::thread::spawn(move || {
        // 8 步 × 16ms ≈ 128ms：步数多了窗口 resize IPC 反而卡（Tauri 官方文档：
        // 透明/带效果窗口的 resize 在 Win10 1903+ / Win11 上开销大，尽量少动）
        let steps = 8;
        for i in 1..=steps {
            if ANIM_GEN.load(std::sync::atomic::Ordering::Relaxed) != gen {
                return; // 有新动画接管，本线程退出
            }
            let t = i as f64 / steps as f64;
            let e = 1.0 - (1.0 - t) * (1.0 - t); // easeOutQuad
            let _ = win.set_size(tauri::LogicalSize::new(cw + (tw - cw) * e, ch + (th - ch) * e));
            let _ = win.set_position(tauri::LogicalPosition::new(cx + (tx - cx) * e, cy + (ty - cy) * e));
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        let _ = win.set_size(tauri::LogicalSize::new(tw, th));
        let _ = win.set_position(tauri::LogicalPosition::new(tx, ty));
    });
}

/// 位置模式：center | left | right | custom；没存过时，有手动拖动记录视为 custom，否则 center
pub fn pos_mode(conn: &rusqlite::Connection) -> String {
    if let Some(v) = crate::storage::get_setting(conn, "island.pos_mode") {
        return v;
    }
    if crate::storage::get_setting(conn, "island.pos").is_some() {
        "custom".into()
    } else {
        "center".into()
    }
}

pub fn set_pos_mode(conn: &rusqlite::Connection, mode: &str) {
    let _ = crate::storage::set_setting(conn, "island.pos_mode", mode);
}

pub fn idle_enabled(conn: &rusqlite::Connection) -> bool {
    crate::storage::get_setting(conn, "island.idle_enabled")
        .map(|v| v == "1")
        .unwrap_or(true)
}

/// 材质：solid（纯色）/ acrylic（毛玻璃）/ mica（云母）
pub fn material(conn: &rusqlite::Connection) -> String {
    crate::storage::get_setting(conn, "island.material").unwrap_or_else(|| "solid".into())
}

pub fn set_material(conn: &rusqlite::Connection, mat: &str) {
    let _ = crate::storage::set_setting(conn, "island.material", mat);
}

/// 强调色模式：endfield（终末地黄绿）/ accent（跟随主程序强调色）
pub fn accent_mode(conn: &rusqlite::Connection) -> String {
    crate::storage::get_setting(conn, "island.accent_mode").unwrap_or_else(|| "endfield".into())
}

pub fn set_accent_mode(conn: &rusqlite::Connection, mode: &str) {
    let _ = crate::storage::set_setting(conn, "island.accent_mode", mode);
}

/// 窗口效果配置（Acrylic=毛玻璃 / Mica=云母，均需透明窗口）
fn effects_for(mat: &str) -> Option<tauri::utils::config::WindowEffectsConfig> {
    use tauri::window::Effect;
    let effect = match mat {
        "acrylic" => Effect::Acrylic,
        "mica" => Effect::Mica,
        _ => return None,
    };
    Some(tauri::window::EffectsBuilder::new().effect(effect).build())
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
    let (mx, _my, sw, _sh) = logical_screen(app);
    let x = mx + (sw - ISLAND_W) / 2.0;
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.set_position(tauri::LogicalPosition::new(x, 8.0));
    }
    with_setting(app, |conn| {
        crate::storage::set_setting(conn, "island.pos", &format!("{x:.0},8"));
        set_pos_mode(conn, "center");
    });
}

/// 窗口移动后保存位置（逻辑像素）
pub fn save_pos(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        if let Ok(pos) = win.outer_position() {
            let sc = win.scale_factor().unwrap_or(1.0);
            let (x, y) = (pos.x as f64 / sc, pos.y as f64 / sc);
            with_setting(app, |conn| {
                crate::storage::set_setting(conn, "island.pos", &format!("{x:.0},{y:.0}"));
                // 手动拖过后退出位置模式对齐（否则尺寸变化会被拉回预设位置）
                set_pos_mode(conn, "custom");
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
pub fn with_setting<T>(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> T) -> Option<T> {
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
