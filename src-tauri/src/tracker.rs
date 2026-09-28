use active_win_pos_rs::get_active_window;
use chrono::Local;
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::{input_hook, reminder, storage};
use chrono::Timelike;

/// 空闲判定阈值：超过 60 秒无键鼠输入视为离开，停止计时
const IDLE_THRESHOLD_MS: u32 = 60_000;
/// 每 1 分钟做一次落库检查点，异常退出最多丢 1 分钟数据
const CHECKPOINT_SECS: i64 = 60;
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
    // 切换防抖：新应用需连续 2 秒在前台才确认切换（过滤 Alt-Tab 掠过的瞬态）
    let mut pending: Option<(String, String, String, String, i64)> = None;

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let now = Local::now().timestamp();

        // 键鼠增量每 5 秒落库（强杀/崩溃最多丢 5 秒）
        if now % 5 == 0 {
            let (dk, dc) = input_hook::flush_delta();
            if dk > 0 || dc > 0 {
                let dt = Local::now();
                let date = dt.format("%Y-%m-%d").to_string();
                let hour = dt.hour() as i32;
                with_db(&app, |db| storage::add_input_stats(db, &date, hour, dk, dc));
            }
        }

        if now % TRAY_REFRESH_SECS == 0 {
            refresh_tray(&app);
        }

        check_reminders(&app, Local::now());

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
        if same_app {
            // 回到原应用：撤销未确认的候选
            pending = None;
        } else if let Some((pk, pd, pe, pt, pts)) = pending.as_ref() {
            if *pk == app_key {
                // 连续第 2 秒仍在新应用 → 确认切换
                let (k2, d2, e2, t2, ts0) = (pk.clone(), pd.clone(), pe.clone(), pt.clone(), *pts);
                if let Some(old) = guard.take() {
                    with_db(&app, |db| close_session(db, &old, ts0));
                }
                match with_db(&app, |db| storage::app_id_for(db, &k2, &d2, &e2)) {
                    Some(Ok(app_id)) => {
                        *guard = Some(Session {
                            app_id,
                            app_key: k2,
                            title: t2,
                            seg_start: ts0,
                        });
                    }
                    Some(Err(e)) => eprintln!("[tracker] 应用登记失败: {e}"),
                    None => eprintln!("[tracker] 数据库忙，本秒未记录"),
                }
                pending = None;
            } else {
                // 又跳到别的应用：换候选重新计时
                pending = Some((app_key.clone(), display.clone(), exe.clone(), title.clone(), now));
            }
        } else {
            // 首次出现的新应用候选
            pending = Some((app_key.clone(), display.clone(), exe.clone(), title.clone(), now));
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

/// 规则稍后提醒登记（rule_id -> 生效时间戳）
static SNOOZE: OnceLock<Mutex<HashMap<i64, i64>>> = OnceLock::new();

pub fn snooze_rule(id: i64, until: i64) {
    let map = SNOOZE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut m) = map.lock() {
        m.insert(id, until);
    }
}

fn snoozed_until(id: i64) -> Option<i64> {
    let map = SNOOZE.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock().ok().and_then(|m| m.get(&id).copied())
}

/// 提醒调度：规则（间隔/定点）+ 到期任务；每秒检查，键去重
fn check_reminders(app: &AppHandle, now: chrono::DateTime<chrono::Local>) {
    let ts = now.timestamp();
    let date = now.format("%Y-%m-%d").to_string();
    let hhmm = now.format("%H:%M").to_string();

    let Some(Ok(rows)) = with_db(app, |db| storage::rule_enabled(db)) else {
        return;
    };
    for r in rows {
        let fire_key: Option<String> = match r.mode.as_str() {
            "interval" => {
                let last = r.last_fired_key.parse::<i64>().unwrap_or(0);
                let interval = r.interval_minutes.unwrap_or(0) * 60;
                let snooze_ok = snoozed_until(r.id).map(|s| ts >= s).unwrap_or(true);
                if interval > 0 && ts >= last + interval && snooze_ok {
                    Some(ts.to_string())
                } else {
                    None
                }
            }
            "daily" => r
                .daily_times
                .iter()
                .find(|t| **t == hhmm)
                .map(|t| format!("{date}-{t}")),
            _ => None,
        };
        let Some(key) = fire_key else { continue };
        if r.last_fired_key == key {
            continue;
        }
        with_db(app, |db| storage::rule_mark_fired(db, r.id, &key));
        let mut payload = reminder::Payload::new("rule", r.id, &r.title, &r.body);
        payload.sticky = r.sticky;
        payload.duration_ms = r.card_duration_sec as u64 * 1000;
        payload.accent = r.accent_color.clone();
        payload.style = r.style.clone();
        payload.actions = vec![
            reminder::ActionDef::new("ack", "知道了"),
            reminder::ActionDef::new("snooze5", "5 分钟后"),
        ];
        reminder::show(app, payload);
        break; // 同一秒只弹一张，其余下秒继续
    }

    // 到期任务提醒
    let Some(Ok(due)) = with_db(app, |db| storage::due_tasks(db, ts)) else {
        return;
    };
    for t in due {
        let key = t.due_ts.to_string();
        let mut payload = reminder::Payload::new("task", t.id, "任务到期", &t.content);
        payload.duration_ms = 30_000;
        payload.actions = vec![
            reminder::ActionDef::new("done", "完成"),
            reminder::ActionDef::new("snooze10", "10 分钟后"),
            reminder::ActionDef::new("dismiss", "忽略"),
        ];
        reminder::show(app, payload);
        with_db(app, |db| storage::task_set_reminded(db, t.id, &key));
        break;
    }
}

fn refresh_tray(app: &AppHandle) {
    let paused = app
        .state::<TrackerShared>()
        .paused
        .load(Ordering::Relaxed);
    let minutes = today_total(app) / 60;
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tip = if paused {
            "拾刻 · 已暂停记录".to_string()
        } else {
            format!("拾刻 · 今日已记录 {minutes} 分钟")
        };
        let _ = tray.set_tooltip(Some(tip));
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
