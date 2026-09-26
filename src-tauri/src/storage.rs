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
