mod input_hook;
mod reminder;
mod storage;
mod tracker;

use chrono::Timelike as _;
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
        current = Some(CurrentApp {
            display_name: display,
            seconds: secs,
        });
    }
    apps.sort_by(|a, b| b.seconds.cmp(&a.seconds));
    let total_seconds = apps.iter().map(|a| a.seconds).sum::<i64>();
    let app_count = apps.len();
    let recording = session.is_some() && !paused;
    Ok(DayReport {
        date,
        total_seconds,
        app_count,
        recording,
        paused,
        current,
        apps,
        hourly,
        segments,
    })
}

#[tauri::command]
fn checklist_list(app: tauri::AppHandle) -> Result<Vec<storage::ChecklistItem>, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::checklist_list(&conn)
}

#[tauri::command]
fn checklist_add(
    app: tauri::AppHandle,
    name: String,
    start_time: String,
    end_time: String,
    remind_start: bool,
    remind_end: bool,
) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::checklist_add(&conn, &name, &start_time, &end_time, remind_start, remind_end)
}

#[tauri::command]
fn checklist_set_enabled(app: tauri::AppHandle, id: i64, enabled: bool) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::checklist_set_enabled(&conn, id, enabled)
}

#[tauri::command]
fn checklist_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|_| "数据库锁不可用")?;
    storage::checklist_delete(&conn, id)
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
            checklist_list,
            checklist_add,
            checklist_set_enabled,
            checklist_delete,
            close_reminder,
            import_tai,
            export_json,
            restore_json,
            pet_stats
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
        // 退出前结算未完会话，避免丢数据
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
        }
    });
}
