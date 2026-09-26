use chrono::{Local, TimeZone, Timelike};
use rusqlite::Connection;
use std::path::PathBuf;

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

#[allow(dead_code)]
pub fn today_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[allow(dead_code)]
pub fn today_app_seconds(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let today = today_date();
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(a.display_name, a.name), s.seconds
             FROM daily_stats s JOIN apps a ON a.id = s.app_id
             WHERE s.date = ?1 ORDER BY s.seconds DESC",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([&today], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取失败: {e}"))?);
    }
    Ok(out)
}

/// 本地今天的 0 点时间戳（用于判断当前会话是否跨天）
#[allow(dead_code)]
pub fn today_start_ts() -> i64 {
    let now = Local::now();
    let midnight = now
        .with_hour(0)
        .and_then(|t| t.with_minute(0))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0));
    midnight.map(|t| t.timestamp()).unwrap_or(now.timestamp())
}
