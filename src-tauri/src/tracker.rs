use active_win_pos_rs::get_active_window;
use chrono::Local;
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::{reminder, storage};

/// 空闲判定阈值：超过 60 秒无键鼠输入视为离开，停止计时
const IDLE_THRESHOLD_MS: u32 = 60_000;
/// 每 5 分钟做一次落库检查点，异常退出最多丢 5 分钟数据
const CHECKPOINT_SECS: i64 = 300;
/// 托盘状态刷新间隔（秒）
const TRAY_REFRESH_SECS: i64 = 30;

/// 跨线程共享的追踪状态（前端命令与退出钩子也要读写）
pub struct TrackerShared {
    pub paused: AtomicBool,
    pub session: Mutex<Option<Session>>,
}

/// 当前进行中的前台会话
pub struct Session {
    pub app_id: i64,
    pub app_key: String,
    pub title: String,
    pub seg_start: i64,
}

/// 全局数据库连接（追踪线程、命令、退出钩子共用；写入频率低，锁竞争可忽略）
pub struct Db(pub Mutex<Connection>);

/// 托盘菜单句柄，用于动态更新文字与暂停勾选
pub struct TrayMenu {
    pub today: OnceLock<tauri::menu::MenuItem<tauri::Wry>>,
    pub pause: OnceLock<tauri::menu::CheckMenuItem<tauri::Wry>>,
    pub pet: OnceLock<tauri::menu::CheckMenuItem<tauri::Wry>>,
}

/// Windows 空闲毫秒数（GetLastInputInfo，只读最后输入时刻，不读取输入内容）
#[cfg(windows)]
fn idle_ms() -> u32 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            GetTickCount().saturating_sub(info.dwTime)
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
fn idle_ms() -> u32 {
    0
}

/// 结束会话：写汇总 + 区间原始记录
pub fn close_session(conn: &Connection, session: &Session, end_ts: i64) {
    if end_ts <= session.seg_start {
        return;
    }
    if let Err(e) = storage::add_seconds(conn, session.app_id, session.seg_start, end_ts) {
        eprintln!("[tracker] 汇总落库失败: {e}");
    }
    if let Err(e) = storage::write_segment(conn, session.app_id, session.seg_start, end_ts, &session.title) {
        eprintln!("[tracker] 区间落库失败: {e}");
    }
}

/// 在数据库连接锁内执行操作
fn with_db<R>(app: &AppHandle, f: impl FnOnce(&Connection) -> R) -> Option<R> {
    let db = app.try_state::<Db>()?;
    let guard = db.0.lock().ok()?;
    Some(f(&guard))
}

/// 解析前台窗口 -> (进程标识, 展示名, exe 路径, 窗口标题)
fn foreground() -> Option<(String, String, String, String)> {
    let win = match get_active_window() {
        Ok(w) => w,
        Err(_) => return None,
    };
    let exe = win.process_path.to_string_lossy().to_string();
    let path = std::path::Path::new(&win.process_path);
    let app_key = path
        .file_name()
        .map(|f| f.to_string_lossy().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| win.app_name.to_lowercase());
    let display = path
        .file_stem()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| app_key.clone());
    let title = win.title.chars().take(200).collect::<String>();
    Some((app_key, display, exe, title))
}

/// 追踪主循环：每秒一次，独立线程
pub fn spawn(app: AppHandle) {
    // 自排除：不记录 tallymoment 自己
    let self_key = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase()));

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let now = Local::now().timestamp();

        if now % TRAY_REFRESH_SECS == 0 {
            refresh_tray(&app);
        }

        check_checklist(&app, Local::now());

        let shared = app.state::<TrackerShared>();
        let paused = shared.paused.load(Ordering::Relaxed);

        if paused {
            // 暂停：结算未完会话，此后不再计时
            if let Ok(mut guard) = shared.session.lock() {
                if let Some(s) = guard.take() {
                    with_db(&app, |db| close_session(db, &s, now));
                }
            }
            continue;
        }

        let idle = idle_ms();
        let active = idle < IDLE_THRESHOLD_MS;

        if !active {
            // 离开：会话截止到最后一次输入时刻
            if let Ok(mut guard) = shared.session.lock() {
                if let Some(s) = guard.take() {
                    let end = (now - (idle as i64 / 1000)).max(s.seg_start);
                    with_db(&app, |db| close_session(db, &s, end));
                }
            }
            continue;
        }

        let Some((app_key, display, exe, title)) = foreground() else {
            // 拿不到前台窗口（如安全桌面）：挂起当前会话
            if let Ok(mut guard) = shared.session.lock() {
                if let Some(s) = guard.take() {
                    with_db(&app, |db| close_session(db, &s, now));
                }
            }
            continue;
        };

        if self_key.as_ref() == Some(&app_key) {
            // 前台是自己：不计时
            if let Ok(mut guard) = shared.session.lock() {
                if let Some(s) = guard.take() {
                    with_db(&app, |db| close_session(db, &s, now));
                }
            }
            continue;
        }

        let Ok(mut guard) = shared.session.lock() else {
            continue;
        };

        let same_app = guard.as_ref().map(|s| s.app_key == app_key).unwrap_or(false);
        if !same_app {
            if let Some(old) = guard.take() {
                with_db(&app, |db| close_session(db, &old, now));
            }
            match with_db(&app, |db| storage::app_id_for(db, &app_key, &display, &exe)) {
                Some(Ok(app_id)) => {
                    *guard = Some(Session {
                        app_id,
                        app_key,
                        title,
                        seg_start: now,
                    });
                }
                Some(Err(e)) => eprintln!("[tracker] 应用登记失败: {e}"),
                None => eprintln!("[tracker] 数据库忙，本秒未记录"),
            }
        }

        // 检查点：会话超过 5 分钟先落库，再原地续段
        let need_checkpoint = guard
            .as_ref()
            .map(|s| now - s.seg_start >= CHECKPOINT_SECS)
            .unwrap_or(false);
        if need_checkpoint {
            if let Some(s) = guard.as_ref() {
                with_db(&app, |db| close_session(db, s, now));
            }
            if let Some(s) = guard.as_mut() {
                s.seg_start = now;
            }
        }
    });
}

/// 清单调度：当前 HH:MM 命中起止时间且今日未提醒过则弹窗（每秒检查，防重复键保证只发一次）
fn check_checklist(app: &AppHandle, now: chrono::DateTime<chrono::Local>) {
    let Some(Ok(rows)) = with_db(app, |db| storage::checklist_enabled(db)) else {
        return;
    };
    let date = now.format("%Y-%m-%d").to_string();
    let hhmm = now.format("%H:%M").to_string();
    for r in rows {
        let (kind, wanted) = if hhmm == r.start_time {
            ("start", r.remind_start)
        } else if hhmm == r.end_time {
            ("end", r.remind_end)
        } else {
            continue;
        };
        if !wanted {
            continue;
        }
        let key = format!("{}-{}-{}", r.id, date, kind);
        if r.last_fired_key == key {
            continue;
        }
        with_db(app, |db| storage::checklist_mark_fired(db, r.id, &key));
        let message = if kind == "start" {
            format!("该开始了：{}（{} – {}）", r.name, r.start_time, r.end_time)
        } else {
            format!("该收尾了：{}（{} – {}）", r.name, r.start_time, r.end_time)
        };
        reminder::show(app, &r.name, &message);
        break; // 同一秒只弹一条，其余下秒继续
    }
}

fn refresh_tray(app: &AppHandle) {
    let minutes = today_total(app) / 60;
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(format!("拾刻 · 今日已记录 {minutes} 分钟")));
    }
    if let Some(item) = app.try_state::<TrayMenu>().and_then(|m| m.today.get().cloned()) {
        let _ = item.set_text(format!("今日累计 {minutes} 分钟"));
    }
}

/// 今日总秒数 = 入库汇总 + 当前未结算会话的实时秒数
pub fn today_total(app: &AppHandle) -> i64 {
    let mut total = with_db(app, |db| storage::today_seconds(db).unwrap_or(0)).unwrap_or(0);
    if let Some(shared) = app.try_state::<TrackerShared>() {
        if let Ok(guard) = shared.session.lock() {
            if let Some(s) = guard.as_ref() {
                let now = Local::now().timestamp();
                if now > s.seg_start {
                    total += now - s.seg_start;
                }
            }
        }
    }
    total
}
