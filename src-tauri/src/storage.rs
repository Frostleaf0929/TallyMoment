use chrono::{Local, TimeZone, Timelike};
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub name: String,
    pub display_name: String,
    pub seconds: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HourSlice {
    pub hour: i32,
    pub app_name: String,
    pub seconds: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegSlice {
    pub app_name: String,
    pub start_ts: i64,
    pub end_ts: i64,
    pub title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub id: i64,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub remind_start: bool,
    pub remind_end: bool,
    pub enabled: bool,
}

/// 数据库文件位置：绿色优先（exe 旁 Data/），不可写时回退 %APPDATA%\TallyMoment\Data
pub fn resolve_db_path() -> Result<PathBuf, String> {
    let primary = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("Data")))
        .ok_or_else(|| "无法定位程序目录".to_string())?;
    if ensure_dir_writable(&primary) {
        return Ok(primary.join("tallymoment.db"));
    }
    let fallback = std::env::var("APPDATA")
        .map(|d| PathBuf::from(d).join("TallyMoment").join("Data"))
        .map_err(|_| "无法读取 APPDATA".to_string())?;
    if ensure_dir_writable(&fallback) {
        return Ok(fallback.join("tallymoment.db"));
    }
    Err("数据目录不可写：exe 旁与 %APPDATA% 均失败".to_string())
}

fn ensure_dir_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-test");
    match std::fs::write(&probe, b"ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 打开连接并建表（WAL + 忙等待，两张汇总表都有唯一约束防重复——吸收 Tai 的坑）
pub fn open(path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("打开数据库失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {e}"))?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .map_err(|e| format!("设置 busy_timeout 失败: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("设置 foreign_keys 失败: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS apps (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            name         TEXT NOT NULL UNIQUE,
            display_name TEXT,
            exe_path     TEXT
        );
        CREATE TABLE IF NOT EXISTS segments (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            app_id   INTEGER NOT NULL REFERENCES apps(id),
            start_ts INTEGER NOT NULL,
            end_ts   INTEGER NOT NULL,
            title    TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_segments_start ON segments(start_ts);
        CREATE TABLE IF NOT EXISTS hourly_stats (
            date    TEXT NOT NULL,
            hour    INTEGER NOT NULL,
            app_id  INTEGER NOT NULL REFERENCES apps(id),
            seconds INTEGER NOT NULL,
            UNIQUE(date, hour, app_id)
        );
        CREATE TABLE IF NOT EXISTS daily_stats (
            date    TEXT NOT NULL,
            app_id  INTEGER NOT NULL REFERENCES apps(id),
            seconds INTEGER NOT NULL,
            UNIQUE(date, app_id)
        );
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS checklist (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            name          TEXT NOT NULL,
            start_time    TEXT NOT NULL,
            end_time      TEXT NOT NULL,
            remind_start  INTEGER NOT NULL DEFAULT 1,
            remind_end    INTEGER NOT NULL DEFAULT 1,
            enabled       INTEGER NOT NULL DEFAULT 1,
            last_fired_key TEXT
        );",
    )
    .map_err(|e| format!("建表失败: {e}"))?;
    Ok(conn)
}

/// 按 name 找应用，不存在则创建，返回 app_id
pub fn app_id_for(
    conn: &Connection,
    name: &str,
    display_name: &str,
    exe_path: &str,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT id FROM apps WHERE name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .or_else(|_| {
        conn.execute(
            "INSERT OR IGNORE INTO apps(name, display_name, exe_path) VALUES(?1, ?2, ?3)",
            [name, display_name, exe_path],
        )
        .map_err(|e| format!("插入应用失败: {e}"))?;
        conn.query_row(
            "SELECT id FROM apps WHERE name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("查询应用失败: {e}"))
    })
}

/// 把 [start_ts, end_ts) 的秒数按本地小时/日期切分后累加进汇总表
pub fn add_seconds(
    conn: &Connection,
    app_id: i64,
    start_ts: i64,
    end_ts: i64,
) -> Result<(), String> {
    let mut cur = start_ts;
    while cur < end_ts {
        let dt = Local
            .timestamp_opt(cur, 0)
            .single()
            .ok_or_else(|| format!("非法时间戳 {cur}"))?;
        let next_hour = dt
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .and_then(|t| t.checked_add_signed(chrono::Duration::hours(1)))
            .ok_or_else(|| format!("时间计算溢出 {cur}"))?;
        let slice_end = next_hour.timestamp().min(end_ts);
        let seconds = (slice_end - cur) as i32;
        let date = dt.format("%Y-%m-%d").to_string();
        let hour = dt.hour() as i32;
        conn.execute(
            "INSERT INTO hourly_stats(date, hour, app_id, seconds) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(date, hour, app_id) DO UPDATE SET seconds = seconds + ?4",
            rusqlite::params![date, hour, app_id, seconds],
        )
        .map_err(|e| format!("更新小时汇总失败: {e}"))?;
        conn.execute(
            "INSERT INTO daily_stats(date, app_id, seconds) VALUES(?1, ?2, ?3)
             ON CONFLICT(date, app_id) DO UPDATE SET seconds = seconds + ?3",
            rusqlite::params![date, app_id, seconds],
        )
        .map_err(|e| format!("更新每日汇总失败: {e}"))?;
        cur = slice_end;
    }
    Ok(())
}

/// 写入一条已关闭的前台区间原始记录
pub fn write_segment(
    conn: &Connection,
    app_id: i64,
    start_ts: i64,
    end_ts: i64,
    title: &str,
) -> Result<(), String> {
    if end_ts <= start_ts {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO segments(app_id, start_ts, end_ts, title) VALUES(?1, ?2, ?3, ?4)",
        rusqlite::params![app_id, start_ts, end_ts, title],
    )
    .map_err(|e| format!("写入区间失败: {e}"))?;
    Ok(())
}

/// 今天（本地日期）已入库的总秒数
pub fn today_seconds(conn: &Connection) -> Result<i64, String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    conn.query_row(
        "SELECT COALESCE(SUM(seconds), 0) FROM daily_stats WHERE date = ?1",
        [&today],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|e| format!("查询今日汇总失败: {e}"))
}

pub fn today_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// 今日应用排行（按秒降序）
pub fn today_app_usage(conn: &Connection, date: &str) -> Result<Vec<AppUsage>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.name, COALESCE(a.display_name, a.name), s.seconds
             FROM daily_stats s JOIN apps a ON a.id = s.app_id
             WHERE s.date = ?1 ORDER BY s.seconds DESC",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([date], |row| {
            Ok(AppUsage {
                name: row.get(0)?,
                display_name: row.get(1)?,
                seconds: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取失败: {e}"))?);
    }
    Ok(out)
}

/// 今日小时分布（hour 0-23，按应用细分）
pub fn today_hourly(conn: &Connection, date: &str) -> Result<Vec<HourSlice>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.hour, a.name, s.seconds
             FROM hourly_stats s JOIN apps a ON a.id = s.app_id
             WHERE s.date = ?1 ORDER BY s.hour, s.seconds DESC",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([date], |row| {
            Ok(HourSlice {
                hour: row.get(0)?,
                app_name: row.get(1)?,
                seconds: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取失败: {e}"))?);
    }
    Ok(out)
}

/// 今日区间明细（时间线用），按开始时间升序
pub fn today_segments(conn: &Connection, day_start: i64, day_end: i64) -> Result<Vec<SegSlice>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.name, s.start_ts, s.end_ts, s.title
             FROM segments s JOIN apps a ON a.id = s.app_id
             WHERE s.start_ts >= ?1 AND s.start_ts < ?2 ORDER BY s.start_ts",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![day_start, day_end], |row| {
            Ok(SegSlice {
                app_name: row.get(0)?,
                start_ts: row.get(1)?,
                end_ts: row.get(2)?,
                title: row.get(3)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取失败: {e}"))?);
    }
    Ok(out)
}

/// 本地今天的 0 点时间戳（区间查询边界）
pub fn today_start_ts() -> i64 {
    let now = Local::now();
    let midnight = now
        .with_hour(0)
        .and_then(|t| t.with_minute(0))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0));
    midnight.map(|t| t.timestamp()).unwrap_or(now.timestamp())
}

// ---------- 清单 ----------

fn valid_hhmm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[..2].iter().all(|c| c.is_ascii_digit())
        && b[3..].iter().all(|c| c.is_ascii_digit())
}

fn checklist_row(row: &rusqlite::Row) -> rusqlite::Result<ChecklistItem> {
    Ok(ChecklistItem {
        id: row.get(0)?,
        name: row.get(1)?,
        start_time: row.get(2)?,
        end_time: row.get(3)?,
        remind_start: row.get::<_, i64>(4)? != 0,
        remind_end: row.get::<_, i64>(5)? != 0,
        enabled: row.get::<_, i64>(6)? != 0,
    })
}

const CHECKLIST_COLS: &str =
    "id, name, start_time, end_time, remind_start, remind_end, enabled";

pub fn checklist_list(conn: &Connection) -> Result<Vec<ChecklistItem>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {CHECKLIST_COLS} FROM checklist ORDER BY start_time, id"
        ))
        .map_err(|e| format!("查询清单失败: {e}"))?;
    let rows = stmt
        .query_map([], checklist_row)
        .map_err(|e| format!("查询清单失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取清单失败: {e}"))?);
    }
    Ok(out)
}

/// 新增清单项（不支持跨天，要求 start < end）
pub fn checklist_add(
    conn: &Connection,
    name: &str,
    start_time: &str,
    end_time: &str,
    remind_start: bool,
    remind_end: bool,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return Err("名称需为 1~50 个字符".into());
    }
    if !valid_hhmm(start_time) || !valid_hhmm(end_time) {
        return Err("时间格式需为 HH:MM".into());
    }
    if start_time >= end_time {
        return Err("开始时间需早于结束时间（暂不支持跨天）".into());
    }
    conn.execute(
        "INSERT INTO checklist(name, start_time, end_time, remind_start, remind_end) VALUES(?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            name,
            start_time,
            end_time,
            remind_start as i64,
            remind_end as i64
        ],
    )
    .map_err(|e| format!("新增清单失败: {e}"))?;
    Ok(())
}

pub fn checklist_set_enabled(conn: &Connection, id: i64, enabled: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE checklist SET enabled = ?2 WHERE id = ?1",
        rusqlite::params![id, enabled as i64],
    )
    .map_err(|e| format!("更新清单失败: {e}"))?;
    Ok(())
}

pub fn checklist_delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM checklist WHERE id = ?1", [id])
        .map_err(|e| format!("删除清单失败: {e}"))?;
    Ok(())
}

/// 调度用：启用的清单项（含防重复键）
pub struct ChecklistDue {
    pub id: i64,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub remind_start: bool,
    pub remind_end: bool,
    pub last_fired_key: String,
}

pub fn checklist_enabled(conn: &Connection) -> Result<Vec<ChecklistDue>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, start_time, end_time, remind_start, remind_end, COALESCE(last_fired_key, '')
             FROM checklist WHERE enabled = 1",
        )
        .map_err(|e| format!("查询清单失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ChecklistDue {
                id: row.get(0)?,
                name: row.get(1)?,
                start_time: row.get(2)?,
                end_time: row.get(3)?,
                remind_start: row.get::<_, i64>(4)? != 0,
                remind_end: row.get::<_, i64>(5)? != 0,
                last_fired_key: row.get(6)?,
            })
        })
        .map_err(|e| format!("查询清单失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取清单失败: {e}"))?);
    }
    Ok(out)
}

pub fn checklist_mark_fired(conn: &Connection, id: i64, key: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE checklist SET last_fired_key = ?2 WHERE id = ?1",
        rusqlite::params![id, key],
    )
    .map_err(|e| format!("更新提醒状态失败: {e}"))?;
    Ok(())
}

// ---------- 导入 / 导出 ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub apps: usize,
    pub daily_rows: usize,
    pub hourly_rows: usize,
    pub segments: usize,
    pub skipped: usize,
}

pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .ok()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        rusqlite::params![key, value],
    )
    .map_err(|e| format!("写入设置失败: {e}"))?;
    Ok(())
}

/// 解析 Tai 的时间字段 -> (日期 "YYYY-MM-DD", 小时)
/// 兼容 TEXT("YYYY-MM-DD HH:MM:SS") 与 .NET ticks（EF6 存储形态因配置而异）
fn parse_tai_datetime(v: rusqlite::types::Value, want_hour: bool) -> Option<(String, i32)> {
    use chrono::{Datelike, TimeZone, Timelike, Utc};
    match v {
        rusqlite::types::Value::Text(t) => {
            let t = t.trim();
            if t.len() < 10 {
                return None;
            }
            let date = t.get(0..10)?.to_string();
            let b = date.as_bytes();
            if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
                return None;
            }
            let hour = if want_hour && t.len() >= 13 {
                t.get(11..13).and_then(|h| h.parse::<i32>().ok()).unwrap_or(0)
            } else {
                0
            };
            Some((date, hour))
        }
        rusqlite::types::Value::Integer(n) => {
            // .NET ticks：100 纳秒起自 0001-01-01；其余按 Unix 秒
            let unix = if n >= 10_000_000_000_000_000 {
                n / 10_000_000 - 62_135_596_800
            } else {
                n
            };
            let dt = Utc.timestamp_opt(unix, 0).single()?;
            Some((
                format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day()),
                if want_hour { dt.hour() as i32 } else { 0 },
            ))
        }
        _ => None,
    }
}

fn upsert_daily(conn: &Connection, date: &str, app_id: i64, secs: i64) -> Result<(), String> {
    conn.execute(
        "INSERT INTO daily_stats(date, app_id, seconds) VALUES(?1, ?2, ?3)
         ON CONFLICT(date, app_id) DO UPDATE SET seconds = seconds + ?3",
        rusqlite::params![date, app_id, secs],
    )
    .map_err(|e| format!("写入日汇总失败: {e}"))?;
    Ok(())
}

fn upsert_hourly(
    conn: &Connection,
    date: &str,
    hour: i32,
    app_id: i64,
    secs: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO hourly_stats(date, hour, app_id, seconds) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(date, hour, app_id) DO UPDATE SET seconds = seconds + ?4",
        rusqlite::params![date, hour, app_id, secs],
    )
    .map_err(|e| format!("写入小时汇总失败: {e}"))?;
    Ok(())
}

/// 从 Tai 的 data.db 导入核心逻辑（App/DailyLog/HoursLog）。
/// Tai 的 HoursLog 有同一小时重复行的问题，这里按 (日期,小时,应用) 先聚合再写入，天然去重。
pub fn import_tai_data(src_path: &str, conn: &Connection) -> Result<ImportSummary, String> {
    use std::collections::HashMap;

    let src = Connection::open_with_flags(
        src_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("打开 Tai 数据库失败: {e}"))?;

    for t in ["App", "DailyLog", "HoursLog"] {
        let n: i64 = src
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                [t],
                |r| r.get(0),
            )
            .map_err(|e| format!("读取表结构失败: {e}"))?;
        if n == 0 {
            return Err(format!("不是有效的 Tai 数据库（缺少表 {t}）"));
        }
    }

    let mut summary = ImportSummary {
        apps: 0,
        daily_rows: 0,
        hourly_rows: 0,
        segments: 0,
        skipped: 0,
    };

    // 应用维表：name 优先取 exe 文件名（小写），退化用 Name；显示名 Alias > Name
    let mut app_map: HashMap<i64, i64> = HashMap::new();
    let mut stmt = src
        .prepare("SELECT ID, Name, Alias, File FROM App")
        .map_err(|e| format!("读取 App 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| format!("读取 App 失败: {e}"))?;
    for row in rows {
        let (tid, name, alias, file) = match row {
            Ok(v) => v,
            Err(_) => {
                summary.skipped += 1;
                continue;
            }
        };
        let name_raw = name.unwrap_or_default();
        let file = file.unwrap_or_default();
        let key = std::path::Path::new(&file)
            .file_name()
            .map(|x| x.to_string_lossy().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| name_raw.to_lowercase());
        if key.is_empty() {
            summary.skipped += 1;
            continue;
        }
        let alias_raw = alias.unwrap_or_default();
        let display = if alias_raw.trim().is_empty() {
            name_raw.clone()
        } else {
            alias_raw
        };
        let app_id = app_id_for(conn, &key, &display, &file)?;
        app_map.insert(tid, app_id);
        summary.apps += 1;
    }

    // 每日汇总（聚合同日多行）
    let mut daily: HashMap<(String, i64), i64> = HashMap::new();
    let mut stmt = src
        .prepare("SELECT AppModelID, Date, Time FROM DailyLog")
        .map_err(|e| format!("读取 DailyLog 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, rusqlite::types::Value>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|e| format!("读取 DailyLog 失败: {e}"))?;
    for row in rows {
        let (aid, date_v, time) = match row {
            Ok(v) => v,
            Err(_) => {
                summary.skipped += 1;
                continue;
            }
        };
        let Some((date, _)) = parse_tai_datetime(date_v, false) else {
            summary.skipped += 1;
            continue;
        };
        let Some(app_id) = app_map.get(&aid) else {
            summary.skipped += 1;
            continue;
        };
        if let Some(t) = time {
            if t > 0 {
                *daily.entry((date, *app_id)).or_insert(0) += t;
            }
        }
    }

    // 每小时汇总（聚合去重）
    let mut hourly: HashMap<(String, i32, i64), i64> = HashMap::new();
    let mut stmt = src
        .prepare("SELECT AppModelID, DataTime, Time FROM HoursLog")
        .map_err(|e| format!("读取 HoursLog 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, rusqlite::types::Value>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|e| format!("读取 HoursLog 失败: {e}"))?;
    for row in rows {
        let (aid, dt_v, time) = match row {
            Ok(v) => v,
            Err(_) => {
                summary.skipped += 1;
                continue;
            }
        };
        let Some((date, hour)) = parse_tai_datetime(dt_v, true) else {
            summary.skipped += 1;
            continue;
        };
        let Some(app_id) = app_map.get(&aid) else {
            summary.skipped += 1;
            continue;
        };
        if let Some(t) = time {
            if t > 0 {
                *hourly.entry((date, hour, *app_id)).or_insert(0) += t;
            }
        }
    }

    for ((date, app_id), secs) in &daily {
        upsert_daily(conn, date, *app_id, *secs)?;
        summary.daily_rows += 1;
    }
    for ((date, hour, app_id), secs) in &hourly {
        upsert_hourly(conn, date, *hour, *app_id, *secs)?;
        summary.hourly_rows += 1;
    }
    Ok(summary)
}

/// 全量导出为 JSON，写到 dir/file_name，返回文件完整路径
pub fn export_json(conn: &Connection, dir: &std::path::Path, file_name: &str) -> Result<String, String> {
    use serde_json::json;

    let mut apps = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id, name, display_name, exe_path FROM apps ORDER BY id")
        .map_err(|e| format!("导出 apps 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "displayName": r.get::<_, Option<String>>(2)?,
                "exePath": r.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(|e| format!("导出 apps 失败: {e}"))?;
    for r in rows {
        apps.push(r.map_err(|e| format!("导出 apps 失败: {e}"))?);
    }

    let mut segments = Vec::new();
    let mut stmt = conn
        .prepare("SELECT app_id, start_ts, end_ts, title FROM segments ORDER BY start_ts")
        .map_err(|e| format!("导出 segments 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "appId": r.get::<_, i64>(0)?,
                "startTs": r.get::<_, i64>(1)?,
                "endTs": r.get::<_, i64>(2)?,
                "title": r.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(|e| format!("导出 segments 失败: {e}"))?;
    for r in rows {
        segments.push(r.map_err(|e| format!("导出 segments 失败: {e}"))?);
    }

    let mut hourly = Vec::new();
    let mut stmt = conn
        .prepare("SELECT date, hour, app_id, seconds FROM hourly_stats ORDER BY date, hour")
        .map_err(|e| format!("导出 hourly 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "date": r.get::<_, String>(0)?,
                "hour": r.get::<_, i32>(1)?,
                "appId": r.get::<_, i64>(2)?,
                "seconds": r.get::<_, i64>(3)?,
            }))
        })
        .map_err(|e| format!("导出 hourly 失败: {e}"))?;
    for r in rows {
        hourly.push(r.map_err(|e| format!("导出 hourly 失败: {e}"))?);
    }

    let mut daily = Vec::new();
    let mut stmt = conn
        .prepare("SELECT date, app_id, seconds FROM daily_stats ORDER BY date")
        .map_err(|e| format!("导出 daily 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "date": r.get::<_, String>(0)?,
                "appId": r.get::<_, i64>(1)?,
                "seconds": r.get::<_, i64>(2)?,
            }))
        })
        .map_err(|e| format!("导出 daily 失败: {e}"))?;
    for r in rows {
        daily.push(r.map_err(|e| format!("导出 daily 失败: {e}"))?);
    }

    let mut checklist = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, start_time, end_time, remind_start, remind_end, enabled, last_fired_key
             FROM checklist ORDER BY id",
        )
        .map_err(|e| format!("导出 checklist 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "startTime": r.get::<_, String>(2)?,
                "endTime": r.get::<_, String>(3)?,
                "remindStart": r.get::<_, i64>(4)? != 0,
                "remindEnd": r.get::<_, i64>(5)? != 0,
                "enabled": r.get::<_, i64>(6)? != 0,
                "lastFiredKey": r.get::<_, Option<String>>(7)?,
            }))
        })
        .map_err(|e| format!("导出 checklist 失败: {e}"))?;
    for r in rows {
        checklist.push(r.map_err(|e| format!("导出 checklist 失败: {e}"))?);
    }

    let payload = json!({
        "app": "tallymoment",
        "version": 1,
        "exportedAt": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "apps": apps,
        "segments": segments,
        "hourly": hourly,
        "daily": daily,
        "checklist": checklist,
    });

    std::fs::create_dir_all(dir).map_err(|e| format!("创建导出目录失败: {e}"))?;
    let path = dir.join(file_name);
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&payload).map_err(|e| format!("序列化失败: {e}"))?,
    )
    .map_err(|e| format!("写文件失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 从 JSON 恢复（仅允许空库，避免数据叠加）
pub fn restore_json(conn: &Connection, path: &str) -> Result<ImportSummary, String> {
    for (table, label) in [
        ("apps", "应用"),
        ("segments", "区间"),
        ("hourly_stats", "小时汇总"),
        ("daily_stats", "日汇总"),
        ("checklist", "清单"),
    ] {
        let n: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .map_err(|e| format!("检查表 {table} 失败: {e}"))?;
        if n > 0 {
            return Err(format!(
                "当前数据库不为空（{label}已有数据），恢复仅支持空库，避免数据叠加错乱"
            ));
        }
    }

    let txt = std::fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&txt).map_err(|e| format!("JSON 解析失败: {e}"))?;
    if v.get("app").and_then(|x| x.as_str()) != Some("tallymoment") {
        return Err("不是拾刻的导出文件".into());
    }

    let mut summary = ImportSummary {
        apps: 0,
        daily_rows: 0,
        hourly_rows: 0,
        segments: 0,
        skipped: 0,
    };

    // 应用：老 id -> 新 id 映射
    let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let empty_arr = Vec::new();
    let apps = v.get("apps").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for a in apps {
        let old = a.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
        let name = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
        if name.is_empty() {
            summary.skipped += 1;
            continue;
        }
        conn.execute(
            "INSERT INTO apps(name, display_name, exe_path) VALUES(?1, ?2, ?3)",
            rusqlite::params![
                name,
                a.get("displayName").and_then(|x| x.as_str()),
                a.get("exePath").and_then(|x| x.as_str())
            ],
        )
        .map_err(|e| format!("恢复应用失败: {e}"))?;
        id_map.insert(old, conn.last_insert_rowid());
        summary.apps += 1;
    }

    let segs = v.get("segments").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for s in segs {
        let old_app = s.get("appId").and_then(|x| x.as_i64()).unwrap_or(0);
        let Some(app_id) = id_map.get(&old_app) else {
            summary.skipped += 1;
            continue;
        };
        conn.execute(
            "INSERT INTO segments(app_id, start_ts, end_ts, title) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![
                app_id,
                s.get("startTs").and_then(|x| x.as_i64()).unwrap_or(0),
                s.get("endTs").and_then(|x| x.as_i64()).unwrap_or(0),
                s.get("title").and_then(|x| x.as_str())
            ],
        )
        .map_err(|e| format!("恢复区间失败: {e}"))?;
        summary.segments += 1;
    }

    let hours = v.get("hourly").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for h in hours {
        let old_app = h.get("appId").and_then(|x| x.as_i64()).unwrap_or(0);
        let Some(app_id) = id_map.get(&old_app) else {
            summary.skipped += 1;
            continue;
        };
        let date = h.get("date").and_then(|x| x.as_str()).unwrap_or("");
        if date.is_empty() {
            summary.skipped += 1;
            continue;
        }
        upsert_hourly(
            conn,
            date,
            h.get("hour").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            *app_id,
            h.get("seconds").and_then(|x| x.as_i64()).unwrap_or(0),
        )?;
        summary.hourly_rows += 1;
    }

    let days = v.get("daily").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for d in days {
        let old_app = d.get("appId").and_then(|x| x.as_i64()).unwrap_or(0);
        let Some(app_id) = id_map.get(&old_app) else {
            summary.skipped += 1;
            continue;
        };
        let date = d.get("date").and_then(|x| x.as_str()).unwrap_or("");
        if date.is_empty() {
            summary.skipped += 1;
            continue;
        }
        upsert_daily(conn, date, *app_id, d.get("seconds").and_then(|x| x.as_i64()).unwrap_or(0))?;
        summary.daily_rows += 1;
    }

    let list = v.get("checklist").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for c in list {
        let name = c.get("name").and_then(|x| x.as_str()).unwrap_or("");
        if name.is_empty() {
            summary.skipped += 1;
            continue;
        }
        conn.execute(
            "INSERT INTO checklist(name, start_time, end_time, remind_start, remind_end, enabled, last_fired_key)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                name,
                c.get("startTime").and_then(|x| x.as_str()).unwrap_or("00:00"),
                c.get("endTime").and_then(|x| x.as_str()).unwrap_or("00:00"),
                c.get("remindStart").and_then(|x| x.as_bool()).unwrap_or(true) as i64,
                c.get("remindEnd").and_then(|x| x.as_bool()).unwrap_or(true) as i64,
                c.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true) as i64,
                c.get("lastFiredKey").and_then(|x| x.as_str())
            ],
        )
        .map_err(|e| format!("恢复清单失败: {e}"))?;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tallymoment-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(dir.join(format!("{name}-wal")));
        let _ = std::fs::remove_file(dir.join(format!("{name}-shm")));
        p
    }

    #[test]
    fn tai_import_maps_apps_and_dedups_hour_buckets() {
        let src_path = tmp_db("tai-source.db");
        let src = Connection::open(&src_path).unwrap();
        src.execute_batch(
            "CREATE TABLE App (ID INTEGER PRIMARY KEY, Name TEXT, Alias TEXT, File TEXT);
             CREATE TABLE DailyLog (ID INTEGER PRIMARY KEY, Date TEXT, Time INTEGER, AppModelID INTEGER);
             CREATE TABLE HoursLog (ID INTEGER PRIMARY KEY, DataTime TEXT, Time INTEGER, AppModelID INTEGER);
             INSERT INTO App VALUES (1, 'Chrome', '谷歌浏览器', 'C:/Program/chrome.exe');
             INSERT INTO App VALUES (2, 'Code', '', '');
             INSERT INTO DailyLog VALUES (1, '2025-01-02 00:00:00', 3600, 1);
             INSERT INTO DailyLog VALUES (2, '2025-01-02 00:00:00', 60, 2);
             -- Tai 的重复小时桶：同一天同一小时两行，导入应合并为 1500
             INSERT INTO HoursLog VALUES (1, '2025-01-02 08:00:00', 1000, 1);
             INSERT INTO HoursLog VALUES (2, '2025-01-02 08:00:00', 500, 1);
             INSERT INTO HoursLog VALUES (3, '2025-01-02 09:00:00', 60, 2);",
        )
        .unwrap();

        let dst = tmp_db("ours.db");
        let conn = open(&dst).unwrap();
        let s = import_tai_data(src_path.to_str().unwrap(), &conn).unwrap();

        assert_eq!(s.apps, 2);
        assert_eq!(s.daily_rows, 2);
        assert_eq!(s.hourly_rows, 2);

        let h: i64 = conn
            .query_row(
                "SELECT h.seconds FROM hourly_stats h JOIN apps a ON a.id = h.app_id
                 WHERE a.name = 'chrome.exe' AND h.date = '2025-01-02' AND h.hour = 8",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(h, 1500, "重复小时桶应合并求和");

        let d: i64 = conn
            .query_row(
                "SELECT dd.seconds FROM daily_stats dd JOIN apps a ON a.id = dd.app_id
                 WHERE a.name = 'code' AND dd.date = '2025-01-02'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(d, 60);

        let display: String = conn
            .query_row("SELECT display_name FROM apps WHERE name = 'chrome.exe'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(display, "谷歌浏览器", "别名应作为显示名");
    }

    #[test]
    fn export_restore_roundtrip_and_empty_guard() {
        let p1 = tmp_db("exp-src.db");
        let dir = p1.parent().unwrap().to_path_buf();
        let conn = open(&p1).unwrap();
        let app_id = app_id_for(&conn, "test.exe", "Test", "C:/test.exe").unwrap();
        add_seconds(&conn, app_id, 1_700_000_000, 1_700_000_600).unwrap();
        write_segment(&conn, app_id, 1_700_000_000, 1_700_000_600, "测试区间").unwrap();
        checklist_add(&conn, "测试清单", "08:00", "09:00", true, true).unwrap();

        let exported = export_json(&conn, &dir, "export-test.json").unwrap();

        let p2 = tmp_db("restored.db");
        let conn2 = open(&p2).unwrap();
        let s = restore_json(&conn2, &exported).unwrap();
        assert_eq!(s.apps, 1);
        assert_eq!(s.segments, 1);

        for t in ["apps", "segments", "hourly_stats", "daily_stats", "checklist"] {
            let a: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap();
            let b: i64 = conn2.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap();
            assert_eq!(a, b, "表 {t} 行数应一致");
        }

        let name: String = conn2
            .query_row("SELECT name FROM apps WHERE name = 'test.exe'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "test.exe");

        // 非空库应拒绝再次恢复
        assert!(restore_json(&conn2, &exported).is_err());
    }

    #[test]
    fn reject_non_tai_database() {
        let src_path = tmp_db("not-tai.db");
        let src = Connection::open(&src_path).unwrap();
        src.execute_batch("CREATE TABLE other (x INTEGER);").unwrap();
        let dst = tmp_db("ours2.db");
        let conn = open(&dst).unwrap();
        assert!(import_tai_data(src_path.to_str().unwrap(), &conn).is_err());
    }
}
