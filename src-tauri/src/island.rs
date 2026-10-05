//! 原子岛：常驻置顶悬浮胶囊（不抢焦点、不进任务栏），
//! 显示 当前专注 / 下个任务倒计时 / 今日完成数，点击展开待办卡。
//!
//! 方案 A（固定窗口 + 窗口内动画）：窗口按最大展开尺寸一次创建，此后永不 resize，
//! 除拖动/改位置模式外也不挪位置。idle / normal / expanded / snap 四态形变全部由
//! island.html 的 CSS 过渡完成（浏览器合成器逐帧插值，取代旧的 8 步 set_size 插值——
//! 逐帧改窗口大小的路线在 WebView2 上天然卡顿，官方文档确认带效果的窗口 resize 开销更大）。
//!
//! 胶囊以外的窗口区域全透明。透明窗口的命中测试是整窗几何（不看像素 alpha），
//! 所以用 ~50ms 光标轮询 + set_ignore_cursor_events 动态切换点击穿透：
//! 光标进入胶囊矩形→关闭穿透（岛可交互），离开→开启穿透（不挡下层应用）。
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const ISLAND_W: f64 = 392.0;
pub const ISLAND_H_COLLAPSED: f64 = 56.0;
pub const ISLAND_H_EXPANDED: f64 = 316.0;
/// WinIsland 式三态：idle（无悬停缩小）/ normal（悬停完整）/ expanded（点击展开）
pub const ISLAND_W_IDLE: f64 = 240.0;
pub const ISLAND_H_IDLE: f64 = 40.0;
/// 窗口顶边固定在屏幕 y=0，胶囊顶边距窗口顶 8px（观感与旧窗口方案一致：胶囊悬在屏幕顶下 8px）
pub const TOP_GAP: f64 = 8.0;
/// 窗口固定尺寸：宽度=最大胶囊宽，高度=顶距+最大展开高（一次创建，不再改变）
pub const ISLAND_WINDOW_H: f64 = TOP_GAP + ISLAND_H_EXPANDED;

const ST_NORMAL: u8 = 0;
const ST_IDLE: u8 = 1;
const ST_EXPANDED: u8 = 2;
/// 吸附态按屏幕边细分：上/下/左/右（露出条各不相同）
const ST_SNAP_TOP: u8 = 3;
const ST_SNAP_BOTTOM: u8 = 4;
const ST_SNAP_LEFT: u8 = 5;
const ST_SNAP_RIGHT: u8 = 6;

/// 离屏幕边多近（逻辑 px）才算"靠边"；更远则吸附退回普通缩小
const SNAP_NEAR_PX: f64 = 48.0;

/// 前端状态机的镜像：仅用于计算光标命中区域（形变本身由前端 CSS 完成）
static STATE: AtomicU8 = AtomicU8::new(ST_NORMAL);
/// 光标轮询线程只启动一次（窗口重建后线程继续服务新窗口）
static POLL_STARTED: AtomicBool = AtomicBool::new(false);

/// 命中计算所需的个性化设置快照（1 秒缓存，避免 20Hz 查库）
#[derive(Clone)]
struct Layout {
    idle_w: f64,
    reveal: f64,
    pm: String,
    click_through: bool,
    wake_click: bool,
}
static LAYOUT_CACHE: std::sync::Mutex<Option<(i64, Layout)>> = std::sync::Mutex::new(None);

/// 区域动画代数：状态切换时先并集后收形，防快速切换时旧的收形线程覆盖新形状
static REGION_GEN: AtomicI64 = AtomicI64::new(0);

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

/// 位置模式对应的窗口 x（窗口宽固定 = ISLAND_W）
fn window_x_for_mode(mx: f64, sw: f64, pm: &str) -> f64 {
    match pm {
        "left" => mx + 8.0,
        "right" => mx + sw - ISLAND_W - 8.0,
        _ => mx + (sw - ISLAND_W) / 2.0,
    }
}

/// 读取保存的位置：pos2 = 窗口左上角（现行）；旧版 pos = 胶囊左上角（胶囊顶=窗口顶+TOP_GAP）
fn saved_pos(app: &AppHandle) -> Option<(f64, f64)> {
    if let Some(v) = with_setting(app, |c| crate::storage::get_setting(c, "island.pos2")).flatten()
    {
        if let Some((x, y)) = parse_pos(&v) {
            return Some((x, y));
        }
    }
    let v = with_setting(app, |c| crate::storage::get_setting(c, "island.pos")).flatten()?;
    parse_pos(&v).map(|(x, y)| (x, (y - TOP_GAP).max(0.0)))
}

fn parse_pos(v: &str) -> Option<(f64, f64)> {
    let (x, y) = v.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
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
    let always_top = with_setting(app, |c| always_top(c)).unwrap_or(true);
    let (x, y) = if pm == "custom" {
        saved_pos(app).unwrap_or((mx + (sw - ISLAND_W) / 2.0, 0.0))
    } else {
        (window_x_for_mode(mx, sw, &pm), 0.0)
    };

    let build = WebviewWindowBuilder::new(app, "island", WebviewUrl::App("island.html".into()))
        .title("拾刻 · 原子岛")
        .inner_size(ISLAND_W, ISLAND_WINDOW_H)
        .position(x.max(0.0), y.max(0.0))
        .decorations(false)
        .transparent(true) // 整窗透明：胶囊只是窗口内的 DOM 元素，材质走 CSS 半透明近似
        .always_on_top(always_top)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .shadow(false)
        .visible(false);

    match build.build() {
        Ok(win) => {
            // 灰框双保险之一：显式把 WebView 底色置为全透明（部分环境 builder 的
            // transparent 没落到 WebView2）。
            let _ = win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            // 灰框双保险之二（根治）：把窗口裁成与胶囊一模一样的区域（SetWindowRgn）——
            // 形状之外的部分在系统层面不存在，无论哪一层在画灰底都露不出来。
            #[cfg(windows)]
            apply_shapes(&win, &shapes_for_state(app, STATE.load(Ordering::Relaxed)));
            eprintln!(
                "[island] 窗口已创建（固定 {}x{}，不再 resize）",
                ISLAND_W, ISLAND_WINDOW_H
            );
            spawn_cursor_poll(app.clone());
        }
        Err(e) => eprintln!("[island] 创建失败: {e}"),
    }
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

pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.hide();
    }
}

/// 前端就绪：显示窗口（创建时隐藏，防首帧黑底闪现）
pub fn on_ready(app: &AppHandle) {
    eprintln!("[island] 前端就绪，显示窗口");
    if let Some(win) = app.get_webview_window("island") {
        // 显示前再兜一次透明底（部分环境在 show 后才合成）
        let _ = win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
        #[cfg(windows)]
        apply_shapes(&win, &shapes_for_state(app, STATE.load(Ordering::Relaxed)));
        let _ = win.show();
    }
}

/// ===== 窗口区域（SetWindowRgn）：把窗口裁成与胶囊一模一样的形状 =====
/// 形状之外的部分从系统层面不存在——灰底无法露出、命中也自然跟随形状。
/// 状态切换时先应用"旧形状 ∪ 新形状"（让 CSS 补间全程可见），320ms 后收成新形状。

#[cfg(windows)]
type Shape = (f64, f64, f64, f64, f64); // (x, y, w, h, 圆角半径) 逻辑 px，窗口内坐标

#[cfg(windows)]
fn shapes_for_state(app: &AppHandle, st: u8) -> Vec<Shape> {
    let lay = layout(app);
    let iw = lay.idle_w.min(ISLAND_W);
    let cap_r = ISLAND_H_IDLE / 2.0;
    match st {
        ST_EXPANDED => vec![
            (0.0, TOP_GAP, ISLAND_W, ISLAND_H_COLLAPSED, ISLAND_H_COLLAPSED / 2.0),
            (
                0.0,
                TOP_GAP + ISLAND_H_COLLAPSED + 8.0,
                ISLAND_W,
                ISLAND_H_EXPANDED - ISLAND_H_COLLAPSED - 8.0,
                18.0,
            ),
        ],
        ST_SNAP_TOP => vec![(idle_x(&lay.pm, iw), lay.reveal - ISLAND_H_IDLE, iw, ISLAND_H_IDLE, cap_r)],
        ST_SNAP_BOTTOM => vec![(
            idle_x(&lay.pm, iw),
            TOP_GAP + ISLAND_H_COLLAPSED - lay.reveal,
            iw,
            ISLAND_H_IDLE,
            cap_r,
        )],
        ST_SNAP_LEFT => vec![(lay.reveal - iw, TOP_GAP, iw, ISLAND_H_IDLE, cap_r)],
        ST_SNAP_RIGHT => vec![(ISLAND_W - lay.reveal, TOP_GAP, iw, ISLAND_H_IDLE, cap_r)],
        ST_IDLE => vec![(idle_x(&lay.pm, iw), TOP_GAP, iw, ISLAND_H_IDLE, cap_r)],
        _ => vec![(0.0, TOP_GAP, ISLAND_W, ISLAND_H_COLLAPSED, ISLAND_H_COLLAPSED / 2.0)],
    }
}

/// 去掉 WS_CAPTION：tao 的无边框窗口样式表里仍保留 WS_CAPTION（平时靠
/// WM_NCCALCSIZE 把客户区撑满整窗"藏"住它），但 SetWindowRgn / SWP_FRAMECHANGED
/// 会触发非客户区重绘把它画出来——表现为岛上方出现"标题栏白条"（用户实拍图一/图二）。
/// 每次贴区域前都确保干净；轮询不再频繁切换穿透标志，避免 apply_diff 反复把帽子加回来。
#[cfg(windows)]
pub fn strip_caption(win: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GWL_STYLE, SetWindowLongPtrW, SetWindowPos, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION,
    };
    let Ok(hwnd) = win.hwnd() else { return };
    unsafe {
        let hwnd = HWND(hwnd.0);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if style & WS_CAPTION.0 != 0 {
            SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_CAPTION.0) as isize);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn strip_caption(_win: &tauri::WebviewWindow) {}

#[cfg(windows)]
fn apply_shapes(win: &tauri::WebviewWindow, shapes: &[Shape]) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, HGDIOBJ, HRGN, RGN_OR,
        SetWindowRgn,
    };
    if shapes.is_empty() {
        return;
    }
    strip_caption(win);
    let Ok(hwnd) = win.hwnd() else { return };
    let Ok(sc) = win.scale_factor() else { return };
    let to_rgn = |s: &Shape| -> HRGN {
        let (x, y, w, h, r) = *s;
        // 内缩 1 物理像素：保证区域 ⊆ 胶囊渲染区（浏览器设备像素取整可能差 1px），
        // 即使 WebView 底色没透明也绝无底色镶边
        let (l, t) = ((x * sc).round() as i32 + 1, (y * sc).round() as i32 + 1);
        let (rt, b) = (((x + w) * sc).round() as i32 - 1, ((y + h) * sc).round() as i32 - 1);
        let e = ((2.0 * r * sc).round() as i32 - 2).max(2); // CreateRoundRectRgn 的椭圆参数 = 2×圆角
        unsafe { CreateRoundRectRgn(l, t, rt, b, e, e) }
    };
    unsafe {
        let combined = if shapes.len() == 1 {
            to_rgn(&shapes[0])
        } else {
            let acc = CreateRectRgn(0, 0, 0, 0);
            for s in shapes {
                let r = to_rgn(s);
                CombineRgn(Some(acc), Some(acc), Some(r), RGN_OR);
                let _ = DeleteObject(HGDIOBJ(r.0));
            }
            acc
        };
        // 成功后区域归系统所有，不要再 DeleteObject(combined)
        let _ = SetWindowRgn(HWND(hwnd.0), Some(combined), true);
    }
}

/// 状态切换后更新区域：先并集（补间全程可见），320ms 后收成新形状（代数防串台）
#[cfg(windows)]
fn schedule_region(app: &AppHandle, old_st: u8, new_st: u8) {
    let Some(win) = app.get_webview_window("island") else { return };
    let mut union = shapes_for_state(app, old_st);
    union.extend(shapes_for_state(app, new_st));
    let gen = REGION_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    std::thread::spawn(move || {
        apply_shapes(&win, &union);
        std::thread::sleep(std::time::Duration::from_millis(320));
        if REGION_GEN.load(Ordering::Relaxed) != gen {
            return;
        }
        let Some(win) = win.app_handle().get_webview_window("island") else { return };
        let shapes = shapes_for_state(win.app_handle(), new_st);
        apply_shapes(&win, &shapes);
    });
}

#[cfg(not(windows))]
fn schedule_region(_app: &AppHandle, _old_st: u8, _new_st: u8) {}

/// 模块配置：胶囊条显示哪些信息、什么顺序（settings 里存 JSON 数组）
pub fn modules(conn: &rusqlite::Connection) -> Vec<String> {
    const DEFAULT: [&str; 3] = ["focus", "next", "done"];
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

/// 前端状态机切换：形变由前端 CSS 完成，这里记录状态 + 同步窗口区域 + 供光标命中计算；
/// "snap" 额外做靠边判定与窗口就位（单次位移），随后前端 CSS 把胶囊滑出露出条
pub fn set_state(app: &AppHandle, state: &str) {
    let new_st = match state {
        "idle" => ST_IDLE,
        "expanded" => ST_EXPANDED,
        "snap" => {
            enter_snap(app);
            return;
        }
        _ => ST_NORMAL,
    };
    let old_st = STATE.load(Ordering::Relaxed);
    STATE.store(new_st, Ordering::Relaxed);
    schedule_region(app, old_st, new_st);
}

/// 靠边吸附：判定窗口矩形离哪条屏幕工作区边（去掉任务栏）最近，
/// ≤SNAP_NEAR_PX 才算靠上 → 窗口一次位移贴边 + 通知前端滑出露出条；
/// 哪条边都不靠 → 退回普通缩小（island-snap 事件回 "none"）。
/// 选边时位置模式优先：靠左/靠右/居中各自吸附对应的边（角落上两条边都近时按摆位意图选）。
fn enter_snap(app: &AppHandle) {
    use tauri::Emitter;
    let fallback = |reason: &str| {
        eprintln!("[island] snap 退回缩小：{reason}");
        let old = STATE.load(Ordering::Relaxed);
        STATE.store(ST_IDLE, Ordering::Relaxed);
        schedule_region(app, old, ST_IDLE);
        let _ = app.emit_to("island", "island-snap", "none".to_string());
    };
    let Some(win) = app.get_webview_window("island") else { return };
    let Ok(pos) = win.outer_position() else {
        fallback("拿不到窗口位置");
        return;
    };
    let Ok(sc) = win.scale_factor() else {
        fallback("拿不到缩放比");
        return;
    };
    let Some(m) = app.primary_monitor().ok().flatten() else {
        fallback("拿不到显示器");
        return;
    };
    let (wax, way, waw, wah) = work_area_logical(&m);
    let (wx, wy) = (pos.x as f64 / sc, pos.y as f64 / sc);
    // (边, 距离[负=已越过], 贴边后的窗口 x, y)
    let cands: [(&str, f64, f64, f64); 4] = [
        ("top", wy - way, wx, way),
        (
            "bottom",
            (way + wah) - (wy + ISLAND_WINDOW_H),
            wx,
            way + wah - TOP_GAP - ISLAND_H_COLLAPSED,
        ),
        ("left", wx - wax, wax, wy),
        ("right", (wax + waw) - (wx + ISLAND_W), wax + waw - ISLAND_W, wy),
    ];
    let nearest = cands
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let Some(nearest) = nearest else {
        fallback("无边候选");
        return;
    };
    let pm = with_setting(app, |c| pos_mode(c)).unwrap_or_else(|| "center".into());
    let preferred: Option<&str> = match pm.as_str() {
        "left" => Some("left"),
        "right" => Some("right"),
        "center" => Some("top"),
        _ => None, // custom：纯按最近边
    };
    let chosen = preferred
        .and_then(|p| cands.iter().find(|c| c.0 == p))
        .filter(|c| c.1 <= SNAP_NEAR_PX)
        .unwrap_or(nearest);
    if chosen.1 > SNAP_NEAR_PX {
        fallback(&format!(
            "哪条边都不够近（最近 {} {:.0}px > {SNAP_NEAR_PX:.0}px）",
            nearest.0, nearest.1
        ));
        return;
    }
    eprintln!(
        "[island] snap → {}（距离 {:.0}px，位置模式 {pm}）",
        chosen.0, chosen.1
    );
    let st = match chosen.0 {
        "top" => ST_SNAP_TOP,
        "bottom" => ST_SNAP_BOTTOM,
        "left" => ST_SNAP_LEFT,
        _ => ST_SNAP_RIGHT,
    };
    let old = STATE.load(Ordering::Relaxed);
    STATE.store(st, Ordering::Relaxed);
    schedule_region(app, old, st);
    if (chosen.2 - wx).abs() > 0.5 || (chosen.3 - wy).abs() > 0.5 {
        let _ = win.set_position(tauri::LogicalPosition::new(chosen.2, chosen.3));
    }
    let _ = app.emit_to("island", "island-snap", chosen.0.to_string());
}

/// 工作区（去掉任务栏等）的逻辑几何
fn work_area_logical(m: &tauri::Monitor) -> (f64, f64, f64, f64) {
    let sc = m.scale_factor();
    let wa = m.work_area();
    let p = wa.position;
    let s = wa.size;
    (
        p.x as f64 / sc,
        p.y as f64 / sc,
        s.width as f64 / sc,
        s.height as f64 / sc,
    )
}

/// 命中计算所需的个性化设置（1 秒缓存）
fn layout(app: &AppHandle) -> Layout {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if let Ok(g) = LAYOUT_CACHE.lock() {
        if let Some((at, lay)) = g.as_ref() {
            if now - *at < 1000 {
                return lay.clone();
            }
        }
    }
    let lay = with_setting(app, |c| Layout {
        idle_w: idle_width(c),
        reveal: snap_reveal(c),
        pm: pos_mode(c),
        click_through: click_through(c),
        wake_click: snap_wake(c) == "click",
    })
    .unwrap_or(Layout {
        idle_w: ISLAND_W_IDLE,
        reveal: 8.0,
        pm: "center".into(),
        click_through: false,
        wake_click: false,
    });
    if let Ok(mut g) = LAYOUT_CACHE.lock() {
        *g = Some((now, lay.clone()));
    }
    lay
}

/// idle 态胶囊在窗口内的 x（与前端 pos-left/pos-right CSS 对齐方式一致）
fn idle_x(pm: &str, w: f64) -> f64 {
    match pm {
        "left" => 0.0,
        "right" => ISLAND_W - w,
        _ => (ISLAND_W - w) / 2.0,
    }
}

/// 当前状态下的胶囊矩形（窗口内逻辑坐标；吸附时可为负=上/左沿被屏幕裁掉）
fn pill_rect(app: &AppHandle) -> (f64, f64, f64, f64) {
    let lay = layout(app);
    let iw = lay.idle_w.min(ISLAND_W);
    match STATE.load(Ordering::Relaxed) {
        ST_EXPANDED => (0.0, TOP_GAP, ISLAND_W, ISLAND_H_EXPANDED),
        ST_SNAP_TOP => {
            // 上滑 (48 - reveal)：胶囊底只露 reveal 像素挂在屏幕上沿
            (idle_x(&lay.pm, iw), lay.reveal - ISLAND_H_IDLE, iw, ISLAND_H_IDLE)
        }
        ST_SNAP_BOTTOM => {
            // 下滑 (56 - reveal)：胶囊顶只露 reveal 像素挂在屏幕下沿
            (
                idle_x(&lay.pm, iw),
                TOP_GAP + ISLAND_H_COLLAPSED - lay.reveal,
                iw,
                ISLAND_H_IDLE,
            )
        }
        ST_SNAP_LEFT => {
            // 左移 (iw - reveal)：胶囊右缘只露 reveal 像素
            (lay.reveal - iw, TOP_GAP, iw, ISLAND_H_IDLE)
        }
        ST_SNAP_RIGHT => {
            // 右移 (iw - reveal)：胶囊左缘只露 reveal 像素
            (ISLAND_W - lay.reveal, TOP_GAP, iw, ISLAND_H_IDLE)
        }
        ST_IDLE => (idle_x(&lay.pm, iw), TOP_GAP, iw, ISLAND_H_IDLE),
        _ => (0.0, TOP_GAP, ISLAND_W, ISLAND_H_COLLAPSED),
    }
}

/// 光标轮询（50ms），只负责两件事：
/// ① 鼠标穿透开关的同步——区域裁剪后胶囊外"不存在窗口"，无需逐次动态穿透
///   （频繁切换会反复触发 tao 的样式重写，把 WS_CAPTION 加回来引发标题栏白条）；
/// ② 吸附态"靠近自动弹出"：光标靠近露出条（屏幕坐标外扩 24px）就唤醒，
///   不依赖窗口命中（露出条之外本来就没有窗口）。
fn spawn_cursor_poll(app: AppHandle) {
    if POLL_STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(move || {
        let mut ignoring = true;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let Some(win) = app.get_webview_window("island") else {
                continue;
            };
            let lay = layout(&app);
            let visible = win.is_visible().unwrap_or(false);
            let want_ignore = lay.click_through || !visible;
            if want_ignore != ignoring {
                let _ = win.set_ignore_cursor_events(want_ignore);
                #[cfg(windows)]
                strip_caption(&win);
                ignoring = want_ignore;
            }
            if want_ignore {
                continue;
            }
            let st = STATE.load(Ordering::Relaxed);
            if !matches!(
                st,
                ST_SNAP_TOP | ST_SNAP_BOTTOM | ST_SNAP_LEFT | ST_SNAP_RIGHT
            ) || lay.wake_click
            {
                continue;
            }
            let Some((cx, cy)) = cursor_pos() else {
                continue;
            };
            let Ok(pos) = win.outer_position() else {
                continue;
            };
            let Ok(sc) = win.scale_factor() else {
                continue;
            };
            let (ix, iy, iw, ih) = pill_rect(&app);
            let (px, py) = (pos.x as f64, pos.y as f64);
            let near = cx as f64 >= px + (ix - 24.0) * sc
                && cx as f64 <= px + (ix + iw + 24.0) * sc
                && cy as f64 >= py + (iy - 24.0) * sc
                && cy as f64 <= py + (iy + ih + 24.0) * sc;
            if near {
                use tauri::Emitter;
                let _ = app.emit_to("island", "island-wake", ());
            }
        }
    });
}

#[cfg(windows)]
fn cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

#[cfg(not(windows))]
fn cursor_pos() -> Option<(i32, i32)> {
    None
}

/// 位置模式：center | left | right | custom；没存过时，有手动拖动记录视为 custom，否则 center
pub fn pos_mode(conn: &rusqlite::Connection) -> String {
    if let Some(v) = crate::storage::get_setting(conn, "island.pos_mode") {
        return v;
    }
    if crate::storage::get_setting(conn, "island.pos").is_some()
        || crate::storage::get_setting(conn, "island.pos2").is_some()
    {
        "custom".into()
    } else {
        "center".into()
    }
}

pub fn set_pos_mode(conn: &rusqlite::Connection, mode: &str) {
    let _ = crate::storage::set_setting(conn, "island.pos_mode", mode);
}

/// 自动隐藏形态：shrink（缩小胶囊）/ snap（靠边吸附只露一条）
pub fn hide_mode(conn: &rusqlite::Connection) -> String {
    crate::storage::get_setting(conn, "island.hide_mode").unwrap_or_else(|| "shrink".into())
}

pub fn set_hide_mode(conn: &rusqlite::Connection, mode: &str) {
    let _ = crate::storage::set_setting(conn, "island.hide_mode", mode);
}

/// 靠边吸附开关（独立于自动隐藏；旧"隐藏形态=snap"未显式设置时自动迁移为开）
pub fn snap_enabled(conn: &rusqlite::Connection) -> bool {
    match crate::storage::get_setting(conn, "island.snap_enabled") {
        Some(v) => v == "1",
        None => hide_mode(conn) == "snap",
    }
}

pub fn set_snap_enabled(conn: &rusqlite::Connection, on: bool) {
    let _ = crate::storage::set_setting(conn, "island.snap_enabled", if on { "1" } else { "0" });
}

/// 鼠标穿透：开=整窗点击穿透（纯展示，不响应悬停；需回个性化关闭）
pub fn click_through(conn: &rusqlite::Connection) -> bool {
    crate::storage::get_setting(conn, "island.click_through")
        .map(|v| v == "1")
        .unwrap_or(false)
}

pub fn set_click_through(conn: &rusqlite::Connection, on: bool) {
    let _ = crate::storage::set_setting(conn, "island.click_through", if on { "1" } else { "0" });
}

/// 吸附唤醒方式：hover（靠近露出条自动弹出，默认）/ click（点击露出条才弹出）
pub fn snap_wake(conn: &rusqlite::Connection) -> String {
    match crate::storage::get_setting(conn, "island.snap_wake") {
        Some(v) if v == "click" => "click".into(),
        _ => "hover".into(),
    }
}

pub fn set_snap_wake(conn: &rusqlite::Connection, mode: &str) {
    let _ = crate::storage::set_setting(conn, "island.snap_wake", mode);
}

/// 窗口置顶（默认开；关=岛可被其他窗口遮挡）
pub fn always_top(conn: &rusqlite::Connection) -> bool {
    crate::storage::get_setting(conn, "island.always_top")
        .map(|v| v != "0")
        .unwrap_or(true)
}

pub fn set_always_top(conn: &rusqlite::Connection, on: bool) {
    let _ = crate::storage::set_setting(conn, "island.always_top", if on { "1" } else { "0" });
}

/// 吸附后露出的高度（px）
pub fn snap_reveal(conn: &rusqlite::Connection) -> f64 {
    crate::storage::get_setting(conn, "island.snap_reveal")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(8.0)
        .clamp(4.0, 24.0)
}

pub fn set_snap_reveal(conn: &rusqlite::Connection, px: f64) {
    let _ = crate::storage::set_setting(conn, "island.snap_reveal", &format!("{px:.0}"));
}

/// 鼠标离开后延迟多久缩成 idle（秒，0=立即）
pub fn hide_delay_sec(conn: &rusqlite::Connection) -> i64 {
    crate::storage::get_setting(conn, "island.hide_delay_sec")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1)
        .clamp(0, 30)
}

pub fn set_hide_delay_sec(conn: &rusqlite::Connection, sec: i64) {
    let _ = crate::storage::set_setting(conn, "island.hide_delay_sec", &sec.to_string());
}

/// idle 态的宽度（px，WinIsland 的"隐藏后宽度"）
pub fn idle_width(conn: &rusqlite::Connection) -> f64 {
    crate::storage::get_setting(conn, "island.idle_width")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(ISLAND_W_IDLE)
        .clamp(120.0, 360.0)
}

pub fn set_idle_width(conn: &rusqlite::Connection, w: f64) {
    let _ = crate::storage::set_setting(conn, "island.idle_width", &format!("{w:.0}"));
}

pub fn idle_enabled(conn: &rusqlite::Connection) -> bool {
    crate::storage::get_setting(conn, "island.idle_enabled")
        .map(|v| v == "1")
        .unwrap_or(true)
}

/// 材质：solid（纯色）/ translucent（半透明近似；旧 acrylic/mica 值仅兼容读取）
pub fn material(conn: &rusqlite::Connection) -> String {
    crate::storage::get_setting(conn, "island.material").unwrap_or_else(|| "solid".into())
}

/// 透明度（40~100，%）：100=不透明深底。未设置时按旧材质迁移（半透明=80）
pub fn opacity(conn: &rusqlite::Connection) -> f64 {
    match crate::storage::get_setting(conn, "island.opacity") {
        Some(v) => v
            .parse::<f64>()
            .map(|x| x.clamp(40.0, 100.0))
            .unwrap_or(100.0),
        None => {
            if material(conn) == "solid" {
                100.0
            } else {
                80.0
            }
        }
    }
}

pub fn set_opacity(conn: &rusqlite::Connection, v: f64) {
    let _ = crate::storage::set_setting(conn, "island.opacity", &format!("{v:.0}"));
}

/// 强调色模式：endfield（终末地黄绿）/ accent（跟随主程序强调色）
pub fn accent_mode(conn: &rusqlite::Connection) -> String {
    crate::storage::get_setting(conn, "island.accent_mode").unwrap_or_else(|| "endfield".into())
}

pub fn set_accent_mode(conn: &rusqlite::Connection, mode: &str) {
    let _ = crate::storage::set_setting(conn, "island.accent_mode", mode);
}

/// 位置重置：回屏幕顶部居中（窗口顶=屏幕顶 y=0）
pub fn reset_pos(app: &AppHandle) {
    let (mx, _my, sw, _sh) = logical_screen(app);
    let x = mx + (sw - ISLAND_W) / 2.0;
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
    }
    with_setting(app, |conn| {
        let v = format!("{x:.0},0");
        let _ = crate::storage::set_setting(conn, "island.pos2", &v);
        let _ = crate::storage::set_setting(conn, "island.pos", &v);
        set_pos_mode(conn, "center");
    });
}

/// 窗口移动后保存位置（逻辑像素；窗口左上角，胶囊顶=窗口顶+TOP_GAP）
pub fn save_pos(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("island") {
        if let Ok(pos) = win.outer_position() {
            let sc = win.scale_factor().unwrap_or(1.0);
            let (x, y) = (pos.x as f64 / sc, pos.y as f64 / sc);
            let v = format!("{x:.0},{y:.0}");
            with_setting(app, |conn| {
                let _ = crate::storage::set_setting(conn, "island.pos2", &v);
                // 旧键同步写（pos_mode 的 custom 判定与旧版迁移都看它）
                let _ = crate::storage::set_setting(conn, "island.pos", &v);
                // 手动拖过后退出位置模式对齐（否则尺寸变化会被拉回预设位置）
                set_pos_mode(conn, "custom");
            });
        }
    }
}

/// 在 Db 锁内执行设置读写（island.rs 各函数共用；测试进程无 state 时返回 None）
pub fn with_setting<T>(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> T) -> Option<T> {
    let db = app.try_state::<crate::Db>()?;
    let guard = db.0.lock().ok()?;
    Some(f(&guard))
}

#[cfg(test)]
mod tests {
    use super::parse_pos;

    #[test]
    fn parse_pos_roundtrip() {
        assert_eq!(parse_pos("120,8"), Some((120.0, 8.0)));
        assert_eq!(parse_pos(" -3 , 0 "), Some((-3.0, 0.0)));
        assert_eq!(parse_pos("120"), None);
        assert_eq!(parse_pos("a,b"), None);
    }
}
