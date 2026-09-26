mod storage;
mod tracker;

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tracker::{Db, TrackerShared, TrayMenu};

#[derive(Serialize)]
struct TodaySummary {
    seconds: i64,
    paused: bool,
    recording: bool,
}

#[tauri::command]
fn today_summary(app: tauri::AppHandle) -> TodaySummary {
    let seconds = tracker::today_total(&app);
    let shared = app.state::<TrackerShared>();
    let paused = shared.paused.load(Ordering::Relaxed);
    let recording = shared.session.lock().map(|g| g.is_some()).unwrap_or(false);
    TodaySummary {
        seconds,
        paused,
        recording,
    }
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
        .plugin(tauri_plugin_opener::init())
        .manage(Db(Mutex::new(db)))
        .manage(TrackerShared {
            paused: AtomicBool::new(false),
            session: Mutex::new(None),
        })
        .manage(TrayMenu {
            today: OnceLock::new(),
            pause: OnceLock::new(),
        })
        .invoke_handler(tauri::generate_handler![today_summary])
        .setup(|app| {
            let today_item =
                MenuItem::with_id(app, "today", "今日累计 0 分钟", false, None::<&str>)?;
            let pause_item =
                CheckMenuItem::with_id(app, "pause", "暂停记录", true, false, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "打开面板", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu =
                Menu::with_items(app, &[&today_item, &pause_item, &sep1, &show, &sep2, &quit])?;

            let _ = app.state::<TrayMenu>().today.set(today_item);
            let _ = app.state::<TrayMenu>().pause.set(pause_item);

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

            tracker::spawn(app.handle().clone());
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
