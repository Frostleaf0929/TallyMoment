mod input_hook;
mod insights;
mod reminder;
mod storage;
mod tracker;

use chrono::{TimeZone as _, Timelike as _};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tracker::{Db, TrackerShared, TrayMenu};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentApp {
    display_name: String,
    seconds: i64,
    start_ts: i64,
    /// focused | fragmented | rest | paused
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DayReport {
    date: String,
    total_seconds: i64,
    app_count: usize,
    recording: bool,
    paused: bool,
    current: Option<CurrentApp>,
    /// 今日第几个专注块（含进行中）
    block_index: usize,
    keys: i64,
    clicks: i64,
    apps: Vec<storage::AppUsage>,
    hourly: Vec<storage::HourSlice>,
    segments: Vec<storage::SegSlice>,
}

/// 今日回顾报表：库内数据 + 进行中会话实时合并，前端一次拉全
#[tauri::command]
fn today_report(app: tauri::AppHandle) -> Result<DayReport, String> {
    let shared = app.state::<TrackerShared>();
    let paused = shared.paused.load(Ordering::Relaxed);
    let session = shared.session.lock().map_err(|_| "会话锁不可用")?;
    let now = chrono::Local::now().timestamp();

    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    let date = storage::today_date();
    let day_start = storage::today_start_ts();
    let day_end = day_start + 86_400;

    let mut apps = storage::today_app_usage(&conn, &date)?;
    let mut hourly = storage::today_hourly(&conn, &date)?;
    let mut segments = storage::today_segments(&conn, day_start, day_end)?;

    // 进行中的会话并入报表，让面板实时
    let mut current = None;
    let cur_hour = chrono::Local::now().hour() as i32;
    if let Some(s) = session.as_ref() {
        let secs = (now - s.seg_start).max(0);
        let display = apps
            .iter()
            .find(|a| a.name == s.app_key)
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| s.app_key.trim_end_matches(".exe").to_string());
        match apps.iter_mut().find(|a| a.name == s.app_key) {
            Some(a) => a.seconds += secs,
            None => apps.push(storage::AppUsage {
                name: s.app_key.clone(),
                display_name: display.clone(),
                seconds: secs,
            }),
        }
        match hourly
            .iter_mut()
            .find(|h| h.hour == cur_hour && h.app_name == s.app_key)
        {
            Some(h) => h.seconds += secs,
            None => hourly.push(storage::HourSlice {
                hour: cur_hour,
                app_name: s.app_key.clone(),
                seconds: secs,
            }),
        }
        segments.push(storage::SegSlice {
            app_name: s.app_key.clone(),
            start_ts: s.seg_start,
            end_ts: now,
            title: s.title.clone(),
        });
        // 状态推断：近 30 分钟内切换 ≥6 次视为碎片化
        let recent_switches = segments
            .iter()
            .filter(|g| g.end_ts > now - 1800)
            .count();
        let status = if paused {
            "paused"
        } else if recent_switches >= 6 {
            "fragmented"
        } else {
            "focused"
        };
        current = Some(CurrentApp {
            display_name: display,
            seconds: secs,
            start_ts: s.seg_start,
            status: status.into(),
        });
    }
    // 有意义的专注块：只统计 ≥20 秒的段
    let block_index = segments
        .iter()
        .filter(|g| g.end_ts - g.start_ts >= 20)
        .count();
    apps.sort_by(|a, b| b.seconds.cmp(&a.seconds));
    let total_seconds = apps.iter().map(|a| a.seconds).sum::<i64>();
    let app_count = apps.len();
    let recording = session.is_some() && !paused;
    let (keys, clicks) = storage::input_for_date(&conn, &date)?;
    Ok(DayReport {
        date,
        total_seconds,
        app_count,
        recording,
        paused,
        current,
        block_index,
        keys,
        clicks,
        apps,
        hourly,
        segments,
    })
}

/// 历史某日报表（无实时会话合并）
#[tauri::command]
fn day_report(app: tauri::AppHandle, date: String) -> Result<DayReport, String> {
    let day = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| "日期格式需为 YYYY-MM-DD")?;
    let naive = day.and_hms_opt(0, 0, 0).ok_or("日期异常")?;
    let day_start = chrono::Local
        .from_local_datetime(&naive)
        .single()
        .ok_or("日期异常")?
        .timestamp();
    let day_end = day_start + 86_400;

    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    let apps = storage::today_app_usage(&conn, &date)?;
    let total_seconds = apps.iter().map(|a| a.seconds).sum::<i64>();
    let app_count = apps.len();
    let hourly = storage::today_hourly(&conn, &date)?;
    let segments = storage::today_segments(&conn, day_start, day_end)?;
    let (keys, clicks) = storage::input_for_date(&conn, &date)?;
    Ok(DayReport {
        date,
        total_seconds,
        app_count,
        recording: false,
        paused: false,
        current: None,
        block_index: segments.len(),
        keys,
        clicks,
        apps,
        hourly,
        segments,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DailyTotal {
    date: String,
    seconds: i64,
}

#[tauri::command]
fn recent_daily(app: tauri::AppHandle, days: i32) -> Result<Vec<DailyTotal>, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::recent_daily(&conn, days.clamp(1, 60)).map(|v| {
        v.into_iter()
            .map(|(date, seconds)| DailyTotal { date, seconds })
            .collect()
    })
}

/// 洞察报表（专注块/心流推断/频率区间）
#[tauri::command]
fn insights_report(app: tauri::AppHandle) -> Result<insights::InsightReport, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    insights::report(&conn)
}

#[tauri::command]
fn task_list(app: tauri::AppHandle) -> Result<Vec<storage::Task>, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::task_list(&conn)
}

#[tauri::command]
fn task_add(app: tauri::AppHandle, content: String, priority: i32, due_ts: Option<i64>) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::task_add(&conn, &content, priority, due_ts)
}

#[tauri::command]
fn task_update(
    app: tauri::AppHandle,
    id: i64,
    content: String,
    priority: i32,
    due_ts: Option<i64>,
) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::task_update(&conn, id, &content, priority, due_ts)
}

#[tauri::command]
fn task_set_done(app: tauri::AppHandle, id: i64, done: bool) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::task_set_done(&conn, id, done)
}

#[tauri::command]
fn task_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::task_delete(&conn, id)
}

#[tauri::command]
fn todo_stats(app: tauri::AppHandle) -> Result<storage::TodoStats, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::todo_stats(&conn)
}

#[tauri::command]
fn rule_list(app: tauri::AppHandle) -> Result<Vec<storage::ReminderRule>, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::rule_list(&conn)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn rule_add(
    app: tauri::AppHandle,
    title: String,
    body: String,
    mode: String,
    interval_minutes: Option<i64>,
    daily_times: Vec<String>,
    sticky: bool,
    card_duration_sec: i32,
    accent_color: Option<String>,
) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::rule_add(
        &conn,
        &title,
        &body,
        &mode,
        interval_minutes,
        daily_times,
        sticky,
        card_duration_sec,
        accent_color,
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn rule_update(
    app: tauri::AppHandle,
    id: i64,
    title: String,
    body: String,
    mode: String,
    interval_minutes: Option<i64>,
    daily_times: Vec<String>,
    sticky: bool,
    card_duration_sec: i32,
    accent_color: Option<String>,
) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::rule_update(
        &conn,
        id,
        &title,
        &body,
        &mode,
        interval_minutes,
        daily_times,
        sticky,
        card_duration_sec,
        accent_color,
    )
}

#[tauri::command]
fn rule_set_enabled(app: tauri::AppHandle, id: i64, enabled: bool) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::rule_set_enabled(&conn, id, enabled)
}

#[tauri::command]
fn rule_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::rule_delete(&conn, id)
}

/// 提醒卡按钮回传：规则稍后 / 任务完成与延后
#[tauri::command]
fn reminder_action(app: tauri::AppHandle, kind: String, ref_id: i64, action: String) -> Result<(), String> {
    match kind.as_str() {
        "rule" => {
            if action == "snooze5" {
                tracker::snooze_rule(ref_id, chrono::Local::now().timestamp() + 300);
            }
            Ok(())
        }
        "task" => {
            let db = app.state::<Db>();
            let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
            match action.as_str() {
                "done" => storage::task_set_done(&conn, ref_id, true),
                "snooze10" => storage::task_push_due(&conn, ref_id, 600),
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

/// 提醒窗按内容高度自适应（前端上报）
#[tauri::command]
fn reminder_resize(app: tauri::AppHandle, height: f64) {
    reminder::resize(&app, height);
}

/// 提醒窗前端就绪后拉取积压的提醒，并由 Rust 侧显示窗口（绕开前端权限）
#[tauri::command]
fn reminder_pending(app: tauri::AppHandle) -> Vec<reminder::Payload> {
    eprintln!("[reminder] frontend fetched pending queue");
    if let Some(w) = app.get_webview_window("reminder") {
        let _ = w.show();
    }
    reminder::take_pending()
}

/// 前端事件路径下也由 Rust 侧负责显示
#[tauri::command]
fn reminder_show_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("reminder") {
        let _ = w.show();
    }
}

/// Tai 对齐导出：data.db + 每日/时段 CSV（path 为用户选择的 .db 位置）
#[tauri::command]
fn export_tai(app: tauri::AppHandle, path: String) -> Result<Vec<String>, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    let p = std::path::PathBuf::from(&path);
    let dir = p.parent().ok_or("导出路径异常")?.to_path_buf();
    let base = p
        .file_stem()
        .map(|f| f.to_string_lossy().to_string())
        .ok_or("导出路径异常")?;
    storage::export_tai(&conn, &dir, &base)
}

/// 删除时间记录数据（scope: today | all；app_id 可选）
#[tauri::command]
fn delete_data(
    app: tauri::AppHandle,
    scope: String,
    app_id: Option<i64>,
) -> Result<storage::DeleteSummary, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::delete_range(&conn, &scope, app_id)
}

#[tauri::command]
fn close_reminder(app: tauri::AppHandle) {
    reminder::close(&app);
}

/// 导入 Tai 的 data.db（同文件防重复导入：按 路径+修改时间 签名）
#[tauri::command]
fn import_tai(app: tauri::AppHandle, path: String) -> Result<storage::ImportSummary, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    let sig = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| format!("{t:?}|{path}"));
    if let (Some(s), Some(prev)) = (&sig, storage::get_setting(&conn, "tai_import_sig")) {
        if *prev == *s {
            return Err("这个文件已经导入过了（文件内容未变化）".into());
        }
    }
    let summary = storage::import_tai_data(&path, &conn)?;
    if let Some(s) = sig {
        let _ = storage::set_setting(&conn, "tai_import_sig", &s);
    }
    Ok(summary)
}

/// 全量导出 JSON：带 path 写到指定位置，否则写数据目录；返回文件路径
#[tauri::command]
fn export_json(app: tauri::AppHandle, path: Option<String>) -> Result<String, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    let dir = match path.as_ref() {
        Some(p) => std::path::PathBuf::from(p)
            .parent()
            .ok_or("导出路径异常")?
            .to_path_buf(),
        None => storage::resolve_db_path()?
            .parent()
            .ok_or("数据目录异常")?
            .to_path_buf(),
    };
    let file_name = match path.as_ref() {
        Some(p) => std::path::PathBuf::from(p)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .ok_or("导出路径异常")?,
        None => format!(
            "export-{}.json",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ),
    };
    storage::export_json(&conn, &dir, &file_name)
}

/// 从 JSON 恢复（仅空库）
#[tauri::command]
fn restore_json(app: tauri::AppHandle, path: String) -> Result<storage::ImportSummary, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::restore_json(&conn, &path)
}

#[tauri::command]
fn pet_stats() -> (u64, u64) {
    input_hook::stats()
}

/// 切换桌宠显隐（托盘与个性化页共用）
#[tauri::command]
fn toggle_pet(app: tauri::AppHandle) -> bool {
    if let Some(w) = app.get_webview_window("pet") {
        let vis = w.is_visible().unwrap_or(false);
        let _ = if vis { w.hide() } else { w.show() };
        if let Some(p) = app.state::<TrayMenu>().pet.get() {
            let _ = p.set_checked(!vis);
        }
        return !vis;
    }
    false
}

// 关闭主窗口时不退出，而是隐藏到托盘（记录在后台继续）
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn run() {
    let (db, db_path) = match storage::resolve_db_path()
        .and_then(|p| storage::open(&p).map(|c| (c, p)))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[tallymoment] 数据库初始化失败: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("[tallymoment] 数据库: {}", db_path.display());

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .manage(Db(Mutex::new(db)))
        .manage(TrackerShared {
            paused: AtomicBool::new(false),
            session: Mutex::new(None),
        })
        .manage(TrayMenu {
            today: OnceLock::new(),
            pause: OnceLock::new(),
            pet: OnceLock::new(),
        })
        .invoke_handler(tauri::generate_handler![
            today_report,
            task_list,
            task_add,
            task_update,
            task_set_done,
            task_delete,
            todo_stats,
            rule_list,
            rule_add,
            rule_update,
            rule_set_enabled,
            rule_delete,
            reminder_action,
            reminder_resize,
            reminder_pending,
            reminder_show_window,
            export_tai,
            delete_data,
            close_reminder,
            import_tai,
            export_json,
            restore_json,
            pet_stats,
            day_report,
            recent_daily,
            insights_report,
            toggle_pet
        ])
        .setup(|app| {
            let today_item =
                MenuItem::with_id(app, "today", "今日累计 0 分钟", false, None::<&str>)?;
            let pause_item =
                CheckMenuItem::with_id(app, "pause", "暂停记录", true, false, None::<&str>)?;
            let pet_item =
                CheckMenuItem::with_id(app, "pet", "显示桌宠", true, true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "打开面板", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&today_item, &pause_item, &pet_item, &sep1, &show, &sep2, &quit],
            )?;

            let _ = app.state::<TrayMenu>().today.set(today_item);
            let _ = app.state::<TrayMenu>().pause.set(pause_item);
            let _ = app.state::<TrayMenu>().pet.set(pet_item);

            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("拾刻 · TallyMoment")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "pause" => {
                        let shared = app.state::<TrackerShared>();
                        let new_state = !shared.paused.load(Ordering::Relaxed);
                        shared.paused.store(new_state, Ordering::Relaxed);
                        if let Some(p) = app.state::<TrayMenu>().pause.get() {
                            let _ = p.set_checked(new_state);
                        }
                    }
                    "quit" => app.exit(0),
                    "pet" => {
                        if let Some(w) = app.get_webview_window("pet") {
                            let vis = w.is_visible().unwrap_or(false);
                            let _ = if vis { w.hide() } else { w.show() };
                            if let Some(p) = app.state::<TrayMenu>().pet.get() {
                                let _ = p.set_checked(!vis);
                            }
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(false);
                            if visible {
                                let _ = window.hide();
                            } else {
                                show_main_window(app);
                            }
                        }
                    }
                })
                .build(app)?;

            // 桌宠窗：透明、置顶、无装饰，屏幕右下角（提醒窗上方留空间）
            let (mut px, mut py) = (900.0, 600.0);
            if let Ok(Some(m)) = app.primary_monitor() {
                let scale = m.scale_factor();
                let size = m.size();
                px = size.width as f64 / scale - 392.0;
                py = size.height as f64 / scale - 306.0;
            }
            let _ = WebviewWindowBuilder::new(app, "pet", WebviewUrl::App("index.html".into()))
                .title("拾刻桌宠")
                .inner_size(372.0, 226.0)
                .position(px, py)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .shadow(false)
                .build();

            tracker::spawn(app.handle().clone());
            input_hook::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        // 退出前结算未完会话与键鼠增量，避免丢数据
        if let tauri::RunEvent::ExitRequested { .. } = event {
            if let Some(shared) = app.try_state::<TrackerShared>() {
                if let Ok(mut guard) = shared.session.lock() {
                    if let Some(s) = guard.take() {
                        if let Some(db) = app.try_state::<Db>() {
                            if let Ok(conn) = db.0.lock() {
                                tracker::close_session(
                                    &conn,
                                    &s,
                                    chrono::Local::now().timestamp(),
                                );
                            }
                        }
                    }
                }
            }
            let (dk, dc) = input_hook::flush_delta();
            if dk > 0 || dc > 0 {
                if let Some(db) = app.try_state::<Db>() {
                    if let Ok(conn) = db.0.lock() {
                        let dt = chrono::Local::now();
                        let _ = storage::add_input_stats(
                            &conn,
                            &dt.format("%Y-%m-%d").to_string(),
                            dt.hour() as i32,
                            dk,
                            dc,
                        );
                    }
                }
            }
        }
    });
}
