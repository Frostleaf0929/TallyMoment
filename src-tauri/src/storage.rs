use chrono::{Local, TimeZone, Timelike};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub name: String,
    pub display_name: String,
    pub seconds: i64,
    /// 区间内后台运行秒数（track_background 应用）
    pub bg_seconds: i64,
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
pub struct Task {
    pub id: i64,
    pub content: String,
    pub priority: i32,
    pub due_ts: Option<i64>,
    pub done: bool,
    pub done_ts: Option<i64>,
    pub created_ts: i64,
    /// 固定事项的重复方式：空 = 一次性；daily / weekly / monthly / yearly
    pub repeat_mode: String,
    /// 由哪个固定事项（模板）生成的；模板自身为 None
    pub template_id: Option<i64>,
    /// 开始做的时间（每日追踪用）
    pub start_ts: Option<i64>,
    /// 是否有今天是它的一次"实例"（固定事项列表算出来的，不落库）
    pub is_template: bool,
    /// 到期提醒方式：''=默认卡片 / card / fullscreen / none
    pub remind_style: String,
    /// 进行中间隔提醒分钟数（0 = 不提醒）
    pub remind_interval_min: i64,
    /// 相关应用（心流归属用）：exe 名列表，JSON 存储
    pub related_apps: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderRule {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub mode: String,
    pub interval_minutes: Option<i64>,
    pub daily_times: Vec<String>,
    pub sticky: bool,
    pub card_duration_sec: i32,
    pub accent_color: Option<String>,
    pub enabled: bool,
    /// card | fullscreen
    pub style: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoStats {
    pub today_done: i64,
    pub week_done: i64,
    pub week_rate: i32,
    pub ontime_rate: i32,
    pub avg_minutes: i64,
    /// 完成用时分布：<15分 / <1时 / <4时 / <1天 / ≥1天
    pub buckets: Vec<i64>,
}

/// 数据库文件位置：绿色优先（exe 旁 Data/），不可写时回退 %APPDATA%\TallyMoment\Data
/// 把当前库完整快照到数据目录 backups/tm-backup-<时间戳>.db，返回路径
/// （VACUUM INTO 生成单文件快照，WAL 内容已合并进去）
pub fn backup_database(conn: &Connection) -> Result<String, String> {
    let db_path = resolve_db_path()?;
    let dir = db_path.parent().ok_or("数据目录异常")?.join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
    let path = dir.join(format!("tm-backup-{}.db", Local::now().format("%Y%m%d-%H%M%S")));
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])
        .map_err(|e| format!("备份失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

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

/// 单个应用信息（详细页应用卡）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub display_name: String,
    pub exe_path: Option<String>,
    pub ignored: bool,
    /// 是否跟踪后台运行时长（BongoCat 这类挂后台也算时间的应用）
    pub track_background: bool,
}

pub fn app_info(conn: &Connection, name: &str) -> Result<AppInfo, String> {
    conn.query_row(
        "SELECT name, COALESCE(display_name, name), exe_path, ignored, track_background FROM apps WHERE name = ?1",
        [name],
        |r| {
            Ok(AppInfo {
                name: r.get(0)?,
                display_name: r.get(1)?,
                exe_path: r.get(2)?,
                ignored: r.get::<_, i64>(3)? != 0,
                track_background: r.get::<_, i64>(4)? != 0,
            })
        },
    )
    .map_err(|_| format!("应用 {name} 不存在"))
}

/// 忽略/恢复：忽略后 tracker 不再计时，报表里也不再出现（历史数据保留）
/// 后台跟踪开关
pub fn app_set_track_background(conn: &Connection, name: &str, on: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE apps SET track_background = ?2 WHERE name = ?1",
        rusqlite::params![name, if on { 1 } else { 0 }],
    )
    .map_err(|e| format!("更新后台跟踪设置失败: {e}"))?;
    Ok(())
}

/// 开了后台跟踪的应用（枚举线程用）
pub fn track_background_apps(conn: &Connection) -> Result<Vec<(i64, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name FROM apps WHERE track_background = 1 AND ignored = 0")
        .map_err(|e| format!("查询后台跟踪应用失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| format!("查询后台跟踪应用失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取后台跟踪应用失败: {e}"))?);
    }
    Ok(out)
}

/// 后台秒数累加（小时表 + 日表，均 UPSERT；前台 seconds 不动）
pub fn bg_add_seconds(
    conn: &Connection,
    date: &str,
    hour: i32,
    app_id: i64,
    secs: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO hourly_stats(date, hour, app_id, seconds, bg_seconds)
         VALUES(?1, ?2, ?3, 0, ?4)
         ON CONFLICT(date, hour, app_id) DO UPDATE SET bg_seconds = bg_seconds + ?4",
        rusqlite::params![date, hour, app_id, secs],
    )
    .map_err(|e| format!("写后台小时统计失败: {e}"))?;
    conn.execute(
        "INSERT INTO daily_stats(date, app_id, seconds, bg_seconds)
         VALUES(?1, ?2, 0, ?3)
         ON CONFLICT(date, app_id) DO UPDATE SET bg_seconds = bg_seconds + ?3",
        rusqlite::params![date, app_id, secs],
    )
    .map_err(|e| format!("写后台日统计失败: {e}"))?;
    Ok(())
}

pub fn app_set_ignored(conn: &Connection, name: &str, ignored: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE apps SET ignored = ?2 WHERE name = ?1",
        rusqlite::params![name, ignored as i64],
    )
    .map_err(|e| format!("更新忽略状态失败: {e}"))?;
    Ok(())
}

pub fn app_is_ignored(conn: &Connection, app_id: i64) -> bool {
    conn.query_row("SELECT ignored FROM apps WHERE id = ?1", [app_id], |r| {
        r.get::<_, i64>(0)
    })
    .map(|v| v != 0)
    .unwrap_or(false)
}

/// 用 exe 版本资源的 FileDescription 回填显示名（Tai 式友好名：花笺、哔哩哔哩……）
/// 只处理 显示名为空 / 等于进程名 / 等于去 .exe 进程名 的行；返回回填条数
pub fn backfill_friendly_names(conn: &Connection) -> Result<usize, String> {
    let rows: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, COALESCE(exe_path, '') FROM apps
                 WHERE display_name IS NULL OR display_name = ''
                    OR display_name = name OR display_name = replace(name, '.exe', '')",
            )
            .map_err(|e| format!("查询待回填应用失败: {e}"))?;
        let rs = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| format!("查询待回填应用失败: {e}"))?;
        rs.filter_map(|x| x.ok()).collect()
    };
    let mut n = 0usize;
    for (id, exe) in rows {
        if exe.is_empty() || !std::path::Path::new(&exe).exists() {
            continue;
        }
        let Some(fd) = crate::app_icon::file_description(&exe) else {
            continue;
        };
        conn.execute(
            "UPDATE apps SET display_name = ?2 WHERE id = ?1",
            rusqlite::params![id, fd],
        )
        .map_err(|e| format!("回填显示名失败: {e}"))?;
        n += 1;
    }
    Ok(n)
}

/// 打开连接并建表（WAL + 忙等待，两张汇总表都有唯一约束防重复——吸收 Tai 的坑）
pub fn open(path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("打开数据库失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {e}"))?;
    // 崩溃安全优先：每次提交都真正落盘（落库频率低，性能影响可忽略）
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(|e| format!("设置 synchronous 失败: {e}"))?;
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
        CREATE TABLE IF NOT EXISTS tasks (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            content     TEXT NOT NULL,
            priority    INTEGER NOT NULL DEFAULT 1,
            due_ts      INTEGER,
            done        INTEGER NOT NULL DEFAULT 0,
            done_ts     INTEGER,
            created_ts  INTEGER NOT NULL,
            reminded_key TEXT
        );

        CREATE TABLE IF NOT EXISTS reminder_rules (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            title            TEXT NOT NULL,
            body             TEXT NOT NULL DEFAULT '',
            mode             TEXT NOT NULL,
            interval_minutes INTEGER,
            daily_times      TEXT NOT NULL DEFAULT '[]',
            sticky           INTEGER NOT NULL DEFAULT 0,
            card_duration_sec INTEGER NOT NULL DEFAULT 10,
            accent_color     TEXT,
            enabled          INTEGER NOT NULL DEFAULT 1,
            last_fired_key   TEXT
        );
        CREATE TABLE IF NOT EXISTS daily_notes (
            date       TEXT PRIMARY KEY,
            content    TEXT NOT NULL DEFAULT '',
            images     TEXT NOT NULL DEFAULT '[]',
            updated_ts INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS input_stats (
            date        TEXT NOT NULL,
            hour        INTEGER NOT NULL,
            key_count   INTEGER NOT NULL,
            click_count INTEGER NOT NULL,
            UNIQUE(date, hour)
        );",
    )
    .map_err(|e| format!("建表失败: {e}"))?;
    // B3 迁移：固定事项与每日追踪。单独执行且**逐个容错**——
    // 建表批处理遇到第一个错误会整体中断，老库重开时"列已存在"会直接导致启动失败。
    for sql in [
        "ALTER TABLE tasks ADD COLUMN repeat_mode TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE tasks ADD COLUMN template_id INTEGER",
        "ALTER TABLE tasks ADD COLUMN start_ts INTEGER",
        "ALTER TABLE reminder_rules ADD COLUMN style TEXT NOT NULL DEFAULT 'card'",
        "ALTER TABLE apps ADD COLUMN ignored INTEGER NOT NULL DEFAULT 0",
        // 06 模块重构：任务级提醒方式（默认卡片/全屏/关闭）与进行中间隔提醒（手动确认回补）
        "ALTER TABLE tasks ADD COLUMN remind_style TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE tasks ADD COLUMN remind_interval_min INTEGER NOT NULL DEFAULT 0",
        // 12 后台时长：应用级开关 + 统计表后台秒数列（独立于前台 seconds）
        "ALTER TABLE apps ADD COLUMN track_background INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE hourly_stats ADD COLUMN bg_seconds INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE daily_stats ADD COLUMN bg_seconds INTEGER NOT NULL DEFAULT 0",
        // 心流任务绑定：任务的相关应用集合（JSON 数组，exe 名）
        "ALTER TABLE tasks ADD COLUMN related_apps TEXT NOT NULL DEFAULT '[]'",
        // 软删除：删除只从活动列表隐藏，完成记录/统计保留
        "ALTER TABLE tasks ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0",
    ] {
        let _ = conn.execute(sql, []);
    }
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
        // 没有更好的名字（空 / 就是进程名去后缀）时，读 exe 版本资源的 FileDescription
        // （任务栏悬停名：花笺、哔哩哔哩……Tai 式友好名在首次记录时就拿对）
        let mut display = display_name.to_string();
        let stem = std::path::Path::new(name)
            .file_stem()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        if (display.is_empty() || display == stem) && !exe_path.is_empty() {
            if let Some(fd) = crate::app_icon::file_description(exe_path) {
                display = fd;
            }
        }
        conn.execute(
            "INSERT OR IGNORE INTO apps(name, display_name, exe_path) VALUES(?1, ?2, ?3)",
            [name, display.as_str(), exe_path],
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
             FROM daily_stats s JOIN apps a ON a.id = s.app_id AND a.ignored = 0
             WHERE s.date = ?1 ORDER BY s.seconds DESC",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([date], |row| {
            Ok(AppUsage {
                name: row.get(0)?,
                display_name: row.get(1)?,
                seconds: row.get(2)?,
                bg_seconds: 0,
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
             FROM hourly_stats s JOIN apps a ON a.id = s.app_id AND a.ignored = 0
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
             FROM segments s JOIN apps a ON a.id = s.app_id AND a.ignored = 0
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

// ---------- 待办任务 ----------

/// 解析任务行：`- [ ] 内容` / `* [x] 内容` / `+ [ ] 内容`（兼容 Obsidian 与 Notion 的写法）
fn strip_task_marker(line: &str) -> Option<(bool, String)> {
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))?;
    let (mark, body) = if let Some(r) = rest.strip_prefix("[ ]") {
        (false, r)
    } else if let Some(r) = rest.strip_prefix("[x]").or_else(|| rest.strip_prefix("[X]")) {
        (true, r)
    } else {
        // 也支持 `-[ ]`（无空格）这种不规范但常见的写法
        if let Some(r) = rest.strip_prefix("[ ]") {
            (false, r)
        } else {
            return None;
        }
    };
    Some((mark, body.to_string()))
}

fn valid_hhmm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[..2].iter().all(|c| c.is_ascii_digit())
        && b[3..].iter().all(|c| c.is_ascii_digit())
}

fn task_row(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let related_apps: Option<String> = row.get(13)?;
    Ok(Task {
        id: row.get(0)?,
        content: row.get(1)?,
        priority: row.get(2)?,
        due_ts: row.get(3)?,
        done: row.get::<_, i64>(4)? != 0,
        done_ts: row.get(5)?,
        created_ts: row.get(6)?,
        repeat_mode: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        template_id: row.get(8)?,
        start_ts: row.get(9)?,
        // 列表查询会额外传入一列"今天是否该出现"（见 task_list）
        is_template: row.get::<_, Option<i64>>(10)?.unwrap_or(0) != 0,
        remind_style: row.get::<_, Option<String>>(11)?.unwrap_or_default(),
        remind_interval_min: row.get::<_, Option<i64>>(12)?.unwrap_or(0),
        related_apps: related_apps
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default(),
    })
}

pub fn task_list(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, content, priority, due_ts, done, done_ts, created_ts,
                    repeat_mode, template_id, start_ts,
                    CASE WHEN repeat_mode = '' THEN 0 ELSE 1 END,
                    remind_style, remind_interval_min, related_apps
             FROM tasks
             WHERE deleted = 0
             ORDER BY done ASC,
                      CASE WHEN due_ts IS NULL THEN 1 ELSE 0 END ASC, due_ts ASC,
                      priority DESC, created_ts DESC",
        )
        .map_err(|e| format!("查询任务失败: {e}"))?;
    let rows = stmt
        .query_map([], task_row)
        .map_err(|e| format!("查询任务失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取任务失败: {e}"))?);
    }
    Ok(out)
}

pub fn task_add(conn: &Connection, content: &str, priority: i32, due_ts: Option<i64>) -> Result<(), String> {
    task_add_repeat(conn, content, priority, due_ts, "", None)
}

/// 新增任务 / 固定事项（repeat 为空 = 一次性；template_id 为模板 id，实例用）
pub fn task_add_repeat(
    conn: &Connection,
    content: &str,
    priority: i32,
    due_ts: Option<i64>,
    repeat_mode: &str,
    template_id: Option<i64>,
) -> Result<(), String> {
    let content = content.trim();
    if content.is_empty() || content.chars().count() > 200 {
        return Err("任务内容需为 1~200 个字符".into());
    }
    if !(0..=2).contains(&priority) {
        return Err("优先级非法".into());
    }
    let mode = match repeat_mode {
        "" | "daily" | "weekly" | "monthly" | "yearly" => repeat_mode,
        _ => return Err("重复方式非法".into()),
    };
    conn.execute(
        "INSERT INTO tasks(content, priority, due_ts, created_ts, repeat_mode, template_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![content, priority, due_ts, Local::now().timestamp(), mode, template_id],
    )
    .map_err(|e| format!("新增任务失败: {e}"))?;
    Ok(())
}

pub fn task_update(
    conn: &Connection,
    id: i64,
    content: &str,
    priority: i32,
    due_ts: Option<i64>,
    remind_style: Option<&str>,
    remind_interval_min: Option<i64>,
) -> Result<(), String> {
    let content = content.trim();
    if content.is_empty() || content.chars().count() > 200 {
        return Err("任务内容需为 1~200 个字符".into());
    }
    if !(0..=2).contains(&priority) {
        return Err("优先级非法".into());
    }
    if let Some(style) = remind_style {
        if !matches!(style, "" | "card" | "fullscreen" | "none") {
            return Err("提醒方式非法".into());
        }
    }
    if let Some(m) = remind_interval_min {
        if !(0..=1440).contains(&m) {
            return Err("间隔提醒需在 0~1440 分钟".into());
        }
    }
    conn.execute(
        "UPDATE tasks SET content = ?2, priority = ?3, due_ts = ?4,
            remind_style = COALESCE(?5, remind_style),
            remind_interval_min = COALESCE(?6, remind_interval_min)
         WHERE id = ?1",
        rusqlite::params![id, content, priority, due_ts, remind_style, remind_interval_min],
    )
    .map_err(|e| format!("更新任务失败: {e}"))?;
    Ok(())
}

/// 记录"开始做"的时间（每日追踪）
pub fn task_set_started(conn: &Connection, id: i64, ts: Option<i64>) -> Result<(), String> {
    conn.execute("UPDATE tasks SET start_ts = ?2 WHERE id = ?1", rusqlite::params![id, ts])
        .map_err(|e| format!("记录开始时间失败: {e}"))?;
    Ok(())
}

/// 固定事项：为"今天该出现"的模板生成实例（幂等，可反复调用）
pub fn materialize_repeats(conn: &Connection, today: &str) -> Result<usize, String> {
    use chrono::{Datelike, NaiveDate};
    let Some(today_date) = NaiveDate::parse_from_str(today, "%Y-%m-%d").ok() else {
        return Ok(0);
    };
    let templates: Vec<(i64, String, i32, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, content, priority, repeat_mode FROM tasks WHERE repeat_mode != '' AND deleted = 0")
            .map_err(|e| format!("查询固定事项失败: {e}"))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| format!("查询固定事项失败: {e}"))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    let mut made = 0usize;
    for (id, content, priority, mode) in templates {
        // 判断今天是否命中该重复规则（按模板创建日推定周期锚点）
        let anchor: String = conn
            .query_row(
                "SELECT COALESCE(substr(datetime(created_ts, 'unixepoch', 'localtime'), 1, 10), ?2)
                 FROM tasks WHERE id = ?1",
                rusqlite::params![id, today],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| today.to_string());
        let hit = match mode.as_str() {
            "daily" => true,
            "weekly" => {
                // 同一星期几
                NaiveDate::parse_from_str(&anchor, "%Y-%m-%d")
                    .map(|a| a.weekday() == today_date.weekday())
                    .unwrap_or(false)
            }
            "monthly" => NaiveDate::parse_from_str(&anchor, "%Y-%m-%d")
                .map(|a| a.day() == today_date.day())
                .unwrap_or(false),
            "yearly" => NaiveDate::parse_from_str(&anchor, "%Y-%m-%d")
                .map(|a| a.month() == today_date.month() && a.day() == today_date.day())
                .unwrap_or(false),
            _ => false,
        };
        if !hit || anchor == today {
            continue; // 模板自己就是今天建的，不额外生成
        }
        // 今天已经有实例了吗
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE template_id = ?1
                 AND substr(datetime(due_ts, 'unixepoch', 'localtime'), 1, 10) = ?2",
                rusqlite::params![id, today],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        let due = NaiveDate::parse_from_str(today, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(9, 0, 0))
            .map(|d| d.and_utc().timestamp());
        task_add_repeat(conn, &content, priority, due, "", Some(id))?;
        made += 1;
    }
    Ok(made)
}

/// 习惯追踪：固定事项（重复任务模板）× 近 N 天的完成格
/// 每天一格：当天有实例 = 该日被安排；实例 done = 完成；spent_min = 开始/创建 → 完成的分钟数
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HabitCell {
    pub date: String,
    pub done: bool,
    pub spent_min: Option<i64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HabitRow {
    pub task_id: i64,
    pub content: String,
    pub repeat_mode: String,
    pub cells: Vec<HabitCell>,
}

pub fn habit_grid(conn: &Connection, days: i32) -> Result<Vec<HabitRow>, String> {
    let days = days.clamp(7, 365);
    let cutoff = (Local::now() - chrono::Duration::days((days - 1) as i64))
        .format("%Y-%m-%d")
        .to_string();

    let templates: Vec<(i64, String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, content, repeat_mode FROM tasks WHERE repeat_mode != '' AND template_id IS NULL AND deleted = 0 ORDER BY id")
            .map_err(|e| format!("查询固定事项失败: {e}"))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| format!("查询固定事项失败: {e}"))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let mut out: Vec<HabitRow> = Vec::new();
    for (tid, content, mode) in templates {
        let mut stmt = conn
            .prepare(
                "SELECT done,
                        COALESCE(substr(datetime(due_ts, 'unixepoch', 'localtime'), 1, 10),
                                 substr(datetime(created_ts, 'unixepoch', 'localtime'), 1, 10)) AS d,
                        CASE WHEN done = 1 AND done_ts IS NOT NULL THEN
                             CAST((done_ts - COALESCE(start_ts, created_ts)) / 60 AS INTEGER) END
                 FROM tasks WHERE template_id = ?1 OR id = ?1",
            )
            .map_err(|e| format!("查询实例失败: {e}"))?;
        let rows = stmt
            .query_map([tid], |r| {
                Ok((
                    r.get::<_, i64>(0)? != 0,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|e| format!("查询实例失败: {e}"))?;
        // 同一天多个实例：任一完成即算完成；用时取完成实例里最短的
        let mut by_date: std::collections::BTreeMap<String, HabitCell> = std::collections::BTreeMap::new();
        for r in rows.filter_map(|x| x.ok()) {
            let (done, date, spent) = r;
            if date.is_empty() || date.as_str() < cutoff.as_str() {
                continue;
            }
            let e = by_date.entry(date.clone()).or_insert(HabitCell {
                date,
                done: false,
                spent_min: None,
            });
            e.done = e.done || done;
            if let Some(m) = spent {
                e.spent_min = Some(e.spent_min.map_or(m, |p| p.min(m)));
            }
        }
        out.push(HabitRow {
            task_id: tid,
            content,
            repeat_mode: mode,
            cells: by_date.into_values().collect(),
        });
    }
    Ok(out)
}

/// 习惯格打卡：该日有实例则切换完成状态；没有则补一个实例并直接标记完成（补卡）
pub fn habit_toggle(conn: &Connection, template_id: i64, date: &str) -> Result<bool, String> {
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT id, done FROM tasks
             WHERE (template_id = ?1 OR id = ?1)
               AND COALESCE(substr(datetime(due_ts, 'unixepoch', 'localtime'), 1, 10),
                            substr(datetime(created_ts, 'unixepoch', 'localtime'), 1, 10)) = ?2
             ORDER BY (template_id IS NULL) LIMIT 1",
            rusqlite::params![template_id, date],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match row {
        Some((id, done)) => {
            task_set_done(conn, id, done == 0)?;
            Ok(done == 0)
        }
        None => {
            let (content, priority): (String, i32) = conn
                .query_row(
                    "SELECT content, priority FROM tasks WHERE id = ?1",
                    [template_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|_| "固定事项不存在".to_string())?;
            let due = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(9, 0, 0))
                .map(|d| d.and_utc().timestamp());
            let now = Local::now().timestamp();
            conn.execute(
                "INSERT INTO tasks(content, priority, due_ts, created_ts, done, done_ts, repeat_mode, template_id)
                 VALUES(?1, ?2, ?3, ?4, 1, ?4, '', ?5)",
                rusqlite::params![content, priority, due, now, template_id],
            )
            .map_err(|e| format!("补卡失败: {e}"))?;
            Ok(true)
        }
    }
}

pub fn task_set_done(conn: &Connection, id: i64, done: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE tasks SET done = ?2, done_ts = ?3 WHERE id = ?1",
        rusqlite::params![id, done as i64, if done { Some(Local::now().timestamp()) } else { None }],
    )
    .map_err(|e| format!("更新任务失败: {e}"))?;
    Ok(())
}

/// 删除任务：**软删除**——只从活动列表隐藏，完成记录与统计保留
/// （过去的任务删除只代表某个阶段结束了，不应从完成记录里消失）
pub fn task_delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE tasks SET deleted = 1 WHERE id = ?1", [id])
        .map_err(|e| format!("删除任务失败: {e}"))?;
    Ok(())
}

/// 任务的相关应用集合（心流归属用）：整体覆写（exe 名统一小写去空白）
pub fn task_set_related_apps(conn: &Connection, id: i64, apps: &[String]) -> Result<(), String> {
    let apps: Vec<String> = apps
        .iter()
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
        .collect();
    conn.execute(
        "UPDATE tasks SET related_apps = ?2 WHERE id = ?1",
        rusqlite::params![id, serde_json::to_string(&apps).unwrap_or_else(|_| "[]".into())],
    )
    .map_err(|e| format!("保存相关应用失败: {e}"))?;
    Ok(())
}

/// 归并式追加（心流确认学习闭环：把这段的应用并入任务集合，已存在的不重复），返回合并结果
pub fn task_link_apps(conn: &Connection, id: i64, apps: &[String]) -> Result<Vec<String>, String> {
    let cur: Vec<String> = conn
        .query_row("SELECT related_apps FROM tasks WHERE id = ?1", [id], |r| {
            r.get::<_, Option<String>>(0)
        })
        .map_err(|e| format!("读取任务失败: {e}"))?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    let mut merged = cur;
    for a in apps {
        let a = a.trim().to_lowercase();
        if !a.is_empty() && !merged.contains(&a) {
            merged.push(a);
        }
    }
    task_set_related_apps(conn, id, &merged)?;
    Ok(merged)
}

/// 从任务的追踪时间窗（start_ts → done_ts/现在）推荐高频应用（前 8）
pub fn task_suggest_apps(conn: &Connection, id: i64) -> Result<Vec<(String, i64)>, String> {
    let (start_ts, done_ts): (Option<i64>, Option<i64>) = conn
        .query_row("SELECT start_ts, done_ts FROM tasks WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| format!("读取任务失败: {e}"))?;
    let Some(start) = start_ts else {
        return Ok(Vec::new());
    };
    let end = done_ts.unwrap_or_else(|| chrono::Local::now().timestamp());
    if end <= start {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT a.name, SUM(s.end_ts - s.start_ts) AS secs
             FROM segments s JOIN apps a ON a.id = s.app_id
             WHERE s.start_ts >= ?1 AND s.end_ts <= ?2 AND COALESCE(a.ignored, 0) = 0
             GROUP BY a.name ORDER BY secs DESC LIMIT 8",
        )
        .map_err(|e| format!("查询推荐应用失败: {e}"))?;
    let rows = stmt.query_map(rusqlite::params![start, end], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    });
    let mut out = Vec::new();
    for r in rows.map_err(|e| format!("查询推荐应用失败: {e}"))? {
        out.push(r.map_err(|e| format!("查询推荐应用失败: {e}"))?);
    }
    Ok(out)
}

pub fn task_set_reminded(conn: &Connection, id: i64, key: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE tasks SET reminded_key = ?2 WHERE id = ?1",
        rusqlite::params![id, key],
    )
    .map_err(|e| format!("更新任务提醒状态失败: {e}"))?;
    Ok(())
}

/// 任务延后：截止时间顺延 seconds，并允许再次提醒
pub fn task_push_due(conn: &Connection, id: i64, seconds: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE tasks SET due_ts = due_ts + ?2, reminded_key = '' WHERE id = ?1 AND due_ts IS NOT NULL",
        rusqlite::params![id, seconds],
    )
    .map_err(|e| format!("延后任务失败: {e}"))?;
    Ok(())}

/// 到期未提醒的任务（调度用）
pub struct DueTask {
    pub id: i64,
    pub content: String,
    pub due_ts: i64,
    /// '' = 默认卡片 / card / fullscreen / none
    pub remind_style: String,
}

pub fn due_tasks(conn: &Connection, now: i64) -> Result<Vec<DueTask>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, content, due_ts, remind_style FROM tasks
             WHERE done = 0 AND deleted = 0 AND due_ts IS NOT NULL AND due_ts <= ?1
               AND COALESCE(reminded_key, '') <> CAST(due_ts AS TEXT)",
        )
        .map_err(|e| format!("查询到期任务失败: {e}"))?;
    let rows = stmt
        .query_map([now], |row| {
            Ok(DueTask {
                id: row.get(0)?,
                content: row.get(1)?,
                due_ts: row.get(2)?,
                remind_style: row.get(3)?,
            })
        })
        .map_err(|e| format!("查询到期任务失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取到期任务失败: {e}"))?);
    }
    Ok(out)
}

/// 进行中且开了间隔提醒的任务（调度用；reminded_key 存 "i:<上次触发ts>"）
pub struct IntervalTask {
    pub id: i64,
    pub content: String,
    pub start_ts: i64,
    pub interval_min: i64,
    pub last_key: String,
}

pub fn interval_tasks(conn: &Connection) -> Result<Vec<IntervalTask>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, content, start_ts, remind_interval_min, COALESCE(reminded_key, '') FROM tasks
             WHERE deleted = 0 AND done = 0 AND start_ts IS NOT NULL AND remind_interval_min > 0",
        )
        .map_err(|e| format!("查询间隔提醒任务失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(IntervalTask {
                id: row.get(0)?,
                content: row.get(1)?,
                start_ts: row.get(2)?,
                interval_min: row.get(3)?,
                last_key: row.get(4)?,
            })
        })
        .map_err(|e| format!("查询间隔提醒任务失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取间隔提醒任务失败: {e}"))?);
    }
    Ok(out)
}

/// 完成率统计
pub fn todo_stats(conn: &Connection) -> Result<TodoStats, String> {
    let now = Local::now();
    let today0 = now
        .with_hour(0).and_then(|t| t.with_minute(0))
        .and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0))
        .ok_or("时间计算失败")?;
    let today0 = today0.timestamp();
    let week0 = today0 - 6 * 86_400;

    let today_done: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks WHERE done = 1 AND done_ts >= ?1", [today0], |r| r.get(0))
        .map_err(|e| format!("统计失败: {e}"))?;
    let week_done: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks WHERE done = 1 AND done_ts >= ?1", [week0], |r| r.get(0))
        .map_err(|e| format!("统计失败: {e}"))?;
    let week_missed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE done = 0 AND due_ts IS NOT NULL AND due_ts >= ?1 AND due_ts <= ?2",
            rusqlite::params![week0, now.timestamp()],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计失败: {e}"))?;
    let week_rate = if week_done + week_missed > 0 {
        (week_done * 100 / (week_done + week_missed)) as i32
    } else {
        -1
    };
    let (ontime_done, ontime_total): (i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(CASE WHEN done_ts <= due_ts THEN 1 ELSE 0 END), 0), COUNT(*)
             FROM tasks WHERE done = 1 AND due_ts IS NOT NULL AND done_ts IS NOT NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("统计失败: {e}"))?;
    let ontime_rate = if ontime_total > 0 {
        (ontime_done * 100 / ontime_total) as i32
    } else {
        -1
    };
    let avg_minutes: i64 = conn
        .query_row(
            "SELECT COALESCE(AVG(done_ts - COALESCE(start_ts, created_ts)), 0) / 60 FROM tasks
             WHERE done = 1 AND done_ts IS NOT NULL AND done_ts >= COALESCE(start_ts, created_ts)",
            [],
            |r| r.get::<_, f64>(0).map(|v| v as i64),
        )
        .map_err(|e| format!("统计失败: {e}"))?;

    // 完成用时分布：<15分 / <1时 / <4时 / <1天 / ≥1天
    let mut buckets = vec![0i64; 5];
    let mut stmt = conn
        .prepare("SELECT done_ts - created_ts FROM tasks WHERE done = 1 AND done_ts IS NOT NULL AND done_ts >= created_ts")
        .map_err(|e| format!("统计失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("统计失败: {e}"))?;
    for r in rows {
        let secs: i64 = r.map_err(|e| format!("统计失败: {e}"))?;
        let idx = if secs < 900 { 0 } else if secs < 3600 { 1 } else if secs < 14400 { 2 } else if secs < 86400 { 3 } else { 4 };
        buckets[idx] += 1;
    }

    Ok(TodoStats {
        today_done,
        week_done,
        week_rate,
        ontime_rate,
        avg_minutes,
        buckets,
    })
}

// ---------- 提醒规则（对标 Catrace timer） ----------

fn rule_row(row: &rusqlite::Row) -> rusqlite::Result<ReminderRule> {
    let daily: String = row.get(6)?;
    Ok(ReminderRule {
        id: row.get(0)?,
        title: row.get(1)?,
        body: row.get(2)?,
        mode: row.get(3)?,
        interval_minutes: row.get(4)?,
        daily_times: serde_json::from_str(&daily).unwrap_or_default(),
        sticky: row.get::<_, i64>(5)? != 0,
        card_duration_sec: row.get(7)?,
        accent_color: row.get(8)?,
        enabled: row.get::<_, i64>(9)? != 0,
        style: row.get::<_, Option<String>>(10)?.unwrap_or_else(|| "card".into()),
    })
}

const RULE_COLS: &str =
    "id, title, body, mode, interval_minutes, sticky, daily_times, card_duration_sec, accent_color, enabled,
     COALESCE(style, 'card')";

pub fn rule_list(conn: &Connection) -> Result<Vec<ReminderRule>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT {RULE_COLS} FROM reminder_rules ORDER BY enabled DESC, id"))
        .map_err(|e| format!("查询提醒规则失败: {e}"))?;
    let rows = stmt
        .query_map([], rule_row)
        .map_err(|e| format!("查询提醒规则失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取提醒规则失败: {e}"))?);
    }
    Ok(out)
}

fn validate_rule(
    title: &str,
    mode: &str,
    interval_minutes: Option<i64>,
    daily_times: &[String],
    card_duration_sec: i32,
) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 50 {
        return Err("标题需为 1~50 个字符".into());
    }
    if mode != "interval" && mode != "daily" {
        return Err("触发方式非法".into());
    }
    if mode == "interval" {
        let m = interval_minutes.ok_or("间隔模式需要填写分钟数")?;
        if !(1..=1440).contains(&m) {
            return Err("间隔需在 1~1440 分钟".into());
        }
    }
    if mode == "daily" {
        if daily_times.is_empty() {
            return Err("定点模式至少需要一个时间点".into());
        }
        if daily_times.len() > 8 {
            return Err("时间点最多 8 个".into());
        }
        for t in daily_times {
            if !valid_hhmm(t) {
                return Err(format!("时间点 {t} 格式需为 HH:MM"));
            }
        }
    }
    if !(3..=600).contains(&card_duration_sec) {
        return Err("停留时长需在 3~600 秒".into());
    }
    Ok(())
}

/// 新增规则；interval 模式把上次触发基线设为创建时刻（首次提醒在下一个间隔后）
#[allow(clippy::too_many_arguments)]
pub fn rule_add(
    conn: &Connection,
    title: &str,
    body: &str,
    mode: &str,
    interval_minutes: Option<i64>,
    daily_times: Vec<String>,
    sticky: bool,
    card_duration_sec: i32,
    accent_color: Option<String>,
) -> Result<(), String> {
    rule_add_style(
        conn,
        title,
        body,
        mode,
        interval_minutes,
        daily_times,
        sticky,
        card_duration_sec,
        accent_color,
        "card",
    )
}

/// 新增提醒规则（style: card | fullscreen）
#[allow(clippy::too_many_arguments)]
pub fn rule_add_style(
    conn: &Connection,
    title: &str,
    body: &str,
    mode: &str,
    interval_minutes: Option<i64>,
    daily_times: Vec<String>,
    sticky: bool,
    card_duration_sec: i32,
    accent_color: Option<String>,
    style: &str,
) -> Result<(), String> {
    if style != "card" && style != "fullscreen" {
        return Err("提醒方式需为 card 或 fullscreen".into());
    }
    validate_rule(title, mode, interval_minutes, &daily_times, card_duration_sec)?;
    conn.execute(
        "INSERT INTO reminder_rules(title, body, mode, interval_minutes, daily_times, sticky, card_duration_sec, accent_color, last_fired_key, style)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            title.trim(),
            body.trim(),
            mode,
            interval_minutes,
            serde_json::to_string(&daily_times).unwrap_or_else(|_| "[]".into()),
            sticky as i64,
            card_duration_sec,
            accent_color,
            Local::now().timestamp().to_string(),
            style
        ],
    )
    .map_err(|e| format!("新增提醒规则失败: {e}"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn rule_update(
    conn: &Connection,
    id: i64,
    title: &str,
    body: &str,
    mode: &str,
    interval_minutes: Option<i64>,
    daily_times: Vec<String>,
    sticky: bool,
    card_duration_sec: i32,
    accent_color: Option<String>,
    style: Option<String>,
) -> Result<(), String> {
    let style = style.unwrap_or_else(|| "card".into());
    if style != "card" && style != "fullscreen" {
        return Err("提醒方式需为 card 或 fullscreen".into());
    }
    validate_rule(title, mode, interval_minutes, &daily_times, card_duration_sec)?;
    conn.execute(
        "UPDATE reminder_rules SET title = ?2, body = ?3, mode = ?4, interval_minutes = ?5,
         daily_times = ?6, sticky = ?7, card_duration_sec = ?8, accent_color = ?9,
         style = ?10 WHERE id = ?1",
        rusqlite::params![
            id,
            title.trim(),
            body.trim(),
            mode,
            interval_minutes,
            serde_json::to_string(&daily_times).unwrap_or_else(|_| "[]".into()),
            sticky as i64,
            card_duration_sec,
            accent_color,
            style
        ],
    )
    .map_err(|e| format!("更新提醒规则失败: {e}"))?;
    Ok(())
}

pub fn rule_set_enabled(conn: &Connection, id: i64, enabled: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE reminder_rules SET enabled = ?2 WHERE id = ?1",
        rusqlite::params![id, enabled as i64],
    )
    .map_err(|e| format!("更新提醒规则失败: {e}"))?;
    Ok(())
}

pub fn rule_delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM reminder_rules WHERE id = ?1", [id])
        .map_err(|e| format!("删除提醒规则失败: {e}"))?;
    Ok(())
}

/// 调度用：启用的规则（last_fired_key：interval 存上次触发时间戳，daily 存 {日期}-{HH:MM}）
pub struct RuleDue {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub mode: String,
    pub interval_minutes: Option<i64>,
    pub daily_times: Vec<String>,
    pub sticky: bool,
    pub card_duration_sec: i32,
    pub accent_color: Option<String>,
    pub last_fired_key: String,
    /// card | fullscreen
    pub style: String,
}

pub fn rule_enabled(conn: &Connection) -> Result<Vec<RuleDue>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {RULE_COLS}, COALESCE(last_fired_key, '') FROM reminder_rules
             WHERE enabled = 1 ORDER BY id"
        ))
        .map_err(|e| format!("查询提醒规则失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let r = rule_row(row)?;
            let last = row.get::<_, String>(11)?;
            Ok(RuleDue {
                id: r.id,
                title: r.title,
                body: r.body,
                mode: r.mode,
                interval_minutes: r.interval_minutes,
                daily_times: r.daily_times,
                sticky: r.sticky,
                card_duration_sec: r.card_duration_sec,
                accent_color: r.accent_color,
                last_fired_key: last,
                style: r.style,
            })
        })
        .map_err(|e| format!("查询提醒规则失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取提醒规则失败: {e}"))?);
    }
    Ok(out)
}

pub fn rule_mark_fired(conn: &Connection, id: i64, key: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE reminder_rules SET last_fired_key = ?2 WHERE id = ?1",
        rusqlite::params![id, key],
    )
    .map_err(|e| format!("更新提醒状态失败: {e}"))?;
    Ok(())
}

// ---------- 导入 / 导出 ----------

// ---------- Tai 对齐导出 / 数据删除 ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSummary {
    pub segments: usize,
    pub hourly: usize,
    pub daily: usize,
    pub input: usize,
    pub apps: usize,
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// 生成 Tai 能直接打开的 data.db（App/DailyLog/HoursLog 三表；Tai 启动自检会自动补齐其余表）
fn export_tai_db(conn: &Connection, db_path: &std::path::Path) -> Result<(), String> {
    let tai = Connection::open(db_path).map_err(|e| format!("创建 Tai 库失败: {e}"))?;
    tai.execute_batch(
        r#"CREATE TABLE "App" ([ID] INTEGER PRIMARY KEY, [Name] nvarchar NULL DEFAULT '', [Alias] nvarchar NULL DEFAULT '', [Description] nvarchar NULL DEFAULT '', [File] nvarchar NULL DEFAULT '', [CategoryID] int NULL DEFAULT 0, [IconFile] nvarchar NULL DEFAULT '', [TotalTime] int NULL DEFAULT 0);
CREATE TABLE "DailyLog" ([ID] INTEGER PRIMARY KEY, [Date] datetime NULL, [Time] int NULL DEFAULT 0, [AppModelID] int NULL DEFAULT 0);
CREATE TABLE "HoursLog" ([ID] INTEGER PRIMARY KEY, [DataTime] datetime NULL, [Time] int NULL DEFAULT 0, [AppModelID] int NULL DEFAULT 0);"#,
    )
    .map_err(|e| format!("建 Tai 表失败: {e}"))?;

    let mut apps: Vec<(i64, String, String, String, i64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.name, COALESCE(a.display_name, a.name), COALESCE(a.exe_path, ''),
                        COALESCE((SELECT SUM(seconds) FROM daily_stats d WHERE d.app_id = a.id), 0)
                 FROM apps a ORDER BY a.id",
            )
            .map_err(|e| format!("读取应用失败: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| format!("读取应用失败: {e}"))?;
        for r in rows {
            apps.push(r.map_err(|e| format!("读取应用失败: {e}"))?);
        }
    }
    for (id, name, display, exe, total) in &apps {
        tai.execute(
            "INSERT INTO \"App\" (ID, Name, Alias, Description, File, TotalTime) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, name, display, display, exe, total],
        )
        .map_err(|e| format!("写入 Tai App 失败: {e}"))?;
    }

    let mut stmt = conn
        .prepare("SELECT date, app_id, seconds FROM daily_stats ORDER BY date, app_id")
        .map_err(|e| format!("读取日汇总失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| format!("读取日汇总失败: {e}"))?;
    let mut daily_rows = Vec::new();
    for r in rows {
        daily_rows.push(r.map_err(|e| format!("读取日汇总失败: {e}"))?);
    }
    for (i, (date, app_id, secs)) in daily_rows.iter().enumerate() {
        tai.execute(
            "INSERT INTO \"DailyLog\" (ID, Date, Time, AppModelID) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![
                (i + 1) as i64,
                format!("{date} 00:00:00"),
                secs,
                app_id
            ],
        )
        .map_err(|e| format!("写入 Tai DailyLog 失败: {e}"))?;
    }

    let mut stmt = conn
        .prepare("SELECT date, hour, app_id, seconds FROM hourly_stats ORDER BY date, hour, app_id")
        .map_err(|e| format!("读取小时汇总失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("读取小时汇总失败: {e}"))?;
    let mut hourly_rows = Vec::new();
    for r in rows {
        hourly_rows.push(r.map_err(|e| format!("读取小时汇总失败: {e}"))?);
    }
    for (i, (date, hour, app_id, secs)) in hourly_rows.iter().enumerate() {
        tai.execute(
            "INSERT INTO \"HoursLog\" (ID, DataTime, Time, AppModelID) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![
                (i + 1) as i64,
                format!("{date} {hour:02}:00:00"),
                secs,
                app_id
            ],
        )
        .map_err(|e| format!("写入 Tai HoursLog 失败: {e}"))?;
    }
    Ok(())
}

/// Tai 同列 CSV 导出（每日/时段；列：日期,应用,描述,时长,分类；UTF-8 带 BOM，同 Tai）
fn write_tai_csv(
    conn: &Connection,
    path: &std::path::Path,
    granularity: &str,
) -> Result<(), String> {
    let (date_sel, table) = if granularity == "daily" {
        ("s.date", "daily_stats")
    } else {
        ("s.date || ' ' || printf('%02d:00:00', s.hour)", "hourly_stats")
    };
    let sql = format!(
        "SELECT {t} AS t, COALESCE(a.display_name, a.name), COALESCE(a.display_name, a.name), s.seconds
         FROM {tb} s JOIN apps a ON a.id = s.app_id AND a.ignored = 0 ORDER BY t",
        t = date_sel,
        tb = table
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("读取 CSV 数据失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("读取 CSV 数据失败: {e}"))?;
    let mut out = String::from("\u{FEFF}日期,应用,描述,时长,分类\r\n");
    for r in rows {
        let (t, name, desc, secs) = r.map_err(|e| format!("读取 CSV 数据失败: {e}"))?;
        out.push_str(&format!(
            "{},{},{},{},{}\r\n",
            csv_escape(&t),
            csv_escape(&name),
            csv_escape(&desc),
            secs,
            "未分类"
        ));
    }
    std::fs::write(path, out).map_err(|e| format!("写 CSV 失败: {e}"))?;
    Ok(())
}

/// Tai 同款 xlsx 表格导出（每日/时段两个工作表，列同 CSV）
fn write_tai_xlsx(conn: &Connection, path: &std::path::Path) -> Result<(), String> {
    use rust_xlsxwriter::Workbook;

    // 先把两份行数据从 SQLite 取出
    let mut daily_rows: Vec<(String, String, String, i64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT s.date, COALESCE(a.display_name, a.name), COALESCE(a.display_name, a.name), s.seconds
                 FROM daily_stats s JOIN apps a ON a.id = s.app_id AND a.ignored = 0 ORDER BY s.date",
            )
            .map_err(|e| format!("读取日汇总失败: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| format!("读取日汇总失败: {e}"))?;
        for r in rows {
            daily_rows.push(r.map_err(|e| format!("读取日汇总失败: {e}"))?);
        }
    }
    let mut hours_rows: Vec<(String, String, String, i64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT s.date || ' ' || printf('%02d:00:00', s.hour), COALESCE(a.display_name, a.name),
                        COALESCE(a.display_name, a.name), s.seconds
                 FROM hourly_stats s JOIN apps a ON a.id = s.app_id AND a.ignored = 0
                 ORDER BY s.date, s.hour",
            )
            .map_err(|e| format!("读取小时汇总失败: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| format!("读取小时汇总失败: {e}"))?;
        for r in rows {
            hours_rows.push(r.map_err(|e| format!("读取小时汇总失败: {e}"))?);
        }
    }

    let headers = ["日期", "应用", "描述", "时长", "分类"];
    let mut wb = Workbook::new();
    {
        let sheet = wb.add_worksheet();
        sheet
            .set_name("每日")
            .map_err(|e| format!("设置工作表失败: {e}"))?;
        for (c, h) in headers.iter().enumerate() {
            sheet
                .write(0, c as u16, *h)
                .map_err(|e| format!("写表头失败: {e}"))?;
        }
        for (r, (d, n, ds, secs)) in daily_rows.iter().enumerate() {
            let row = (r + 1) as u32;
            sheet.write(row, 0, d).map_err(|e| format!("写入失败: {e}"))?;
            sheet.write(row, 1, n).map_err(|e| format!("写入失败: {e}"))?;
            sheet.write(row, 2, ds).map_err(|e| format!("写入失败: {e}"))?;
            sheet
                .write(row, 3, *secs)
                .map_err(|e| format!("写入失败: {e}"))?;
            sheet
                .write(row, 4, "未分类")
                .map_err(|e| format!("写入失败: {e}"))?;
        }
    }
    {
        let sheet = wb.add_worksheet();
        sheet
            .set_name("时段")
            .map_err(|e| format!("设置工作表失败: {e}"))?;
        for (c, h) in headers.iter().enumerate() {
            sheet
                .write(0, c as u16, *h)
                .map_err(|e| format!("写表头失败: {e}"))?;
        }
        for (r, (t, n, ds, secs)) in hours_rows.iter().enumerate() {
            let row = (r + 1) as u32;
            sheet.write(row, 0, t).map_err(|e| format!("写入失败: {e}"))?;
            sheet.write(row, 1, n).map_err(|e| format!("写入失败: {e}"))?;
            sheet.write(row, 2, ds).map_err(|e| format!("写入失败: {e}"))?;
            sheet
                .write(row, 3, *secs)
                .map_err(|e| format!("写入失败: {e}"))?;
            sheet
                .write(row, 4, "未分类")
                .map_err(|e| format!("写入失败: {e}"))?;
        }
    }
    wb.save(path).map_err(|e| format!("保存 xlsx 失败: {e}"))?;
    Ok(())
}

/// Tai 对齐导出：data.db + 每日/时段两个 CSV，返回生成的文件路径
pub fn export_tai(
    conn: &Connection,
    dir: &std::path::Path,
    base: &str,
) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let db_path = dir.join(format!("{base}.db"));
    export_tai_db(conn, &db_path)?;
    let csv_daily = dir.join(format!("{base}-每日.csv"));
    let csv_hours = dir.join(format!("{base}-时段.csv"));
    let xlsx_path = dir.join(format!("{base}.xlsx"));
    write_tai_csv(conn, &csv_daily, "daily")?;
    write_tai_csv(conn, &csv_hours, "hours")?;
    write_tai_xlsx(conn, &xlsx_path)?;
    Ok(vec![
        db_path.to_string_lossy().to_string(),
        xlsx_path.to_string_lossy().to_string(),
        csv_daily.to_string_lossy().to_string(),
        csv_hours.to_string_lossy().to_string(),
    ])
}

/// 删除时间记录数据：scope = "today" | "all"；app_id 可选（仅删该应用）
pub fn delete_range(
    conn: &Connection,
    scope: &str,
    app_id: Option<i64>,
) -> Result<DeleteSummary, String> {
    let mut sum = DeleteSummary {
        segments: 0,
        hourly: 0,
        daily: 0,
        input: 0,
        apps: 0,
    };
    let app_cond = app_id
        .map(|a| format!(" AND app_id = {a}"))
        .unwrap_or_default();
    let run = |sql: String| -> Result<usize, String> {
        conn.execute(&sql, [])
            .map_err(|e| format!("删除失败: {e}"))
    };
    match scope {
        "today" => {
            let today = today_date();
            sum.segments = run(format!(
                "DELETE FROM segments WHERE start_ts >= {}{app_cond}",
                today_start_ts()
            ))?;
            sum.hourly = run(format!(
                "DELETE FROM hourly_stats WHERE date = '{today}'{app_cond}"
            ))?;
            sum.daily = run(format!(
                "DELETE FROM daily_stats WHERE date = '{today}'{app_cond}"
            ))?;
            sum.input = run(format!("DELETE FROM input_stats WHERE date = '{today}'"))?;
        }
        "all" => {
            sum.segments = run(format!("DELETE FROM segments{app_cond}"))?;
            sum.hourly = run(format!("DELETE FROM hourly_stats WHERE 1=1{app_cond}"))?;
            sum.daily = run(format!("DELETE FROM daily_stats WHERE 1=1{app_cond}"))?;
            sum.input = run("DELETE FROM input_stats".to_string())?;
            sum.apps = run(format!("DELETE FROM apps WHERE 1=1{app_cond}"))?;
        }
        _ => return Err("删除范围非法".into()),
    }
    Ok(sum)
}


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub apps: usize,
    pub daily_rows: usize,
    pub hourly_rows: usize,
    pub segments: usize,
    pub skipped: usize,
    /// replace 模式下清掉的本地时间数据行数（daily+hourly+segments）
    #[serde(default)]
    pub cleared: usize,
    /// replace 前自动备份的文件路径（命令层填）
    #[serde(default)]
    pub backup_path: Option<String>,
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

// ---------- 键鼠统计（M6） ----------

/// 累加键鼠计数到指定小时桶
pub fn add_input_stats(
    conn: &Connection,
    date: &str,
    hour: i32,
    keys: i64,
    clicks: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO input_stats(date, hour, key_count, click_count) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(date, hour) DO UPDATE SET
           key_count = key_count + ?3, click_count = click_count + ?4",
        rusqlite::params![date, hour, keys, clicks],
    )
    .map_err(|e| format!("写入键鼠统计失败: {e}"))?;
    Ok(())
}

/// 某日键鼠总计数
pub fn input_for_date(conn: &Connection, date: &str) -> Result<(i64, i64), String> {
    conn.query_row(
        "SELECT COALESCE(SUM(key_count), 0), COALESCE(SUM(click_count), 0)
         FROM input_stats WHERE date = ?1",
        [date],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(|e| format!("查询键鼠统计失败: {e}"))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
/// 最近 N 天逐日键鼠计数（旧→新）
pub struct InputDay {
    pub date: String,
    pub keys: i64,
    pub clicks: i64,
}

pub fn input_daily(conn: &Connection, days: i32) -> Result<Vec<InputDay>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date, SUM(key_count), SUM(click_count) FROM input_stats
             GROUP BY date ORDER BY date DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询键鼠趋势失败: {e}"))?;
    let rows = stmt
        .query_map([days], |r| {
            Ok(InputDay {
                date: r.get(0)?,
                keys: r.get(1)?,
                clicks: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询键鼠趋势失败: {e}"))?;
    let mut out: Vec<InputDay> = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取键鼠趋势失败: {e}"))?);
    }
    out.reverse();
    Ok(out)
}

/// 最近 N 天各小时累计（洞察页「作息分布」用，聚合到 24 小时）
pub fn recent_hourly(conn: &Connection, days: i32) -> Result<Vec<HourSlice>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT h.hour, a.name, SUM(h.seconds)
             FROM hourly_stats h JOIN apps a ON a.id = h.app_id AND a.ignored = 0
             WHERE h.date IN (
                 SELECT DISTINCT date FROM hourly_stats ORDER BY date DESC LIMIT ?1
             )
             GROUP BY h.hour, h.app_id",
        )
        .map_err(|e| format!("查询作息分布失败: {e}"))?;
    let rows = stmt
        .query_map([days], |r| {
            Ok(HourSlice {
                hour: r.get(0)?,
                app_name: r.get(1)?,
                seconds: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询作息分布失败: {e}"))?;
    let mut out: Vec<HourSlice> = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取作息分布失败: {e}"))?);
    }
    Ok(out)
}

/// 最近 N 天每日总时长（旧→新）
pub fn recent_daily(conn: &Connection, days: i32) -> Result<Vec<(String, i64)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date, SUM(seconds) FROM daily_stats
             GROUP BY date ORDER BY date DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询每日汇总失败: {e}"))?;
    let rows = stmt
        .query_map([days], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| format!("查询每日汇总失败: {e}"))?;
    let mut out: Vec<(String, i64)> = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取每日汇总失败: {e}"))?);
    }
    out.reverse();
    Ok(out)
}

// ---------- 周期报表（历史页：按月 / 按年 / 总计） ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodBucket {
    /// 显示标签（月视图=日、年视图=月、总视图=年）
    pub label: String,
    /// 该桶的首日日期（用于下钻查看某天）
    pub date: String,
    pub seconds: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodReport {
    /// month | year | all
    pub kind: String,
    pub key: String,
    pub title: String,
    pub total_seconds: i64,
    pub app_count: usize,
    pub active_days: i64,
    pub keys: i64,
    pub clicks: i64,
    pub apps: Vec<AppUsage>,
    pub hourly: Vec<HourSlice>,
    pub buckets: Vec<PeriodBucket>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodIndex {
    /// 有数据的月份，新→旧，如 "2026-09"
    pub months: Vec<String>,
    pub years: Vec<String>,
}

/// 可用周期清单（供历史页下拉选择）
pub fn period_index(conn: &Connection) -> Result<PeriodIndex, String> {
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT substr(date, 1, 7) FROM daily_stats ORDER BY 1 DESC",
        )
        .map_err(|e| format!("查询月份清单失败: {e}"))?;
    let months: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("查询月份清单失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    let years = months
        .iter()
        .filter_map(|m| m.split('-').next().map(|s| s.to_string()))
        .fold(Vec::<String>::new(), |mut acc, y| {
            if !acc.contains(&y) {
                acc.push(y);
            }
            acc
        });
    Ok(PeriodIndex { months, years })
}

fn period_prefix(kind: &str, key: &str) -> Result<String, String> {
    match kind {
        "month" => {
            let ok = key.len() == 7 && key.as_bytes()[4] == b'-';
            if !ok {
                return Err("月份格式需为 YYYY-MM".into());
            }
            Ok(format!("{key}-%"))
        }
        "year" => {
            let ok = key.len() == 4 && key.chars().all(|c| c.is_ascii_digit());
            if !ok {
                return Err("年份格式需为 YYYY".into());
            }
            Ok(format!("{key}-%"))
        }
        "all" => Ok("%".into()),
        _ => Err("周期类型需为 month / year / all".into()),
    }
}

/// 任意周期的汇总：区间总时长 + 应用排行 + 24 小时分布 + 趋势桶
pub fn period_report(conn: &Connection, kind: &str, key: &str) -> Result<PeriodReport, String> {
    let prefix = period_prefix(kind, key)?;

    let mut app_stmt = conn
        .prepare(
            "SELECT a.name, COALESCE(a.display_name, a.name), SUM(d.seconds)
             FROM daily_stats d JOIN apps a ON a.id = d.app_id AND a.ignored = 0
             WHERE d.date LIKE ?1
             GROUP BY d.app_id ORDER BY 3 DESC LIMIT 60",
        )
        .map_err(|e| format!("查询周期应用排行失败: {e}"))?;
    let apps: Vec<AppUsage> = app_stmt
        .query_map([&prefix], |r| {
            Ok(AppUsage {
                name: r.get(0)?,
                display_name: r.get(1)?,
                seconds: r.get(2)?,
                bg_seconds: 0,
            })
        })
        .map_err(|e| format!("查询周期应用排行失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut hour_stmt = conn
        .prepare(
            "SELECT h.hour, a.name, SUM(h.seconds)
             FROM hourly_stats h JOIN apps a ON a.id = h.app_id AND a.ignored = 0
             WHERE h.date LIKE ?1
             GROUP BY h.hour, h.app_id",
        )
        .map_err(|e| format!("查询周期时段分布失败: {e}"))?;
    let hourly: Vec<HourSlice> = hour_stmt
        .query_map([&prefix], |r| {
            Ok(HourSlice {
                hour: r.get(0)?,
                app_name: r.get(1)?,
                seconds: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询周期时段分布失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // 趋势桶：月视图按天、年视图按月、总计按年
    let group_expr = match kind {
        "month" => "d.date",
        "year" => "substr(d.date, 1, 7)",
        _ => "substr(d.date, 1, 4)",
    };
    let bucket_sql = format!(
        "SELECT {group_expr} AS k, MIN(d.date), SUM(d.seconds)
         FROM daily_stats d WHERE d.date LIKE ?1 GROUP BY k ORDER BY k"
    );
    let mut bucket_stmt = conn
        .prepare(&bucket_sql)
        .map_err(|e| format!("查询周期趋势失败: {e}"))?;
    let buckets: Vec<PeriodBucket> = bucket_stmt
        .query_map([&prefix], |r| {
            let k: String = r.get(0)?;
            let date: String = r.get(1)?;
            Ok(PeriodBucket {
                label: if kind == "month" {
                    k.get(8..).unwrap_or(&k).trim_start_matches('0').to_string()
                } else if kind == "year" {
                    k.get(5..).unwrap_or(&k).to_string()
                } else {
                    k.clone()
                },
                date,
                seconds: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询周期趋势失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let (keys, clicks) = conn
        .query_row(
            "SELECT COALESCE(SUM(key_count), 0), COALESCE(SUM(click_count), 0)
             FROM input_stats WHERE date LIKE ?1",
            [&prefix],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap_or((0, 0));
    let active_days: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT date) FROM daily_stats WHERE date LIKE ?1",
            [&prefix],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let total_seconds = apps.iter().map(|a| a.seconds).sum();
    let title = match kind {
        "month" => {
            let (y, m) = key.split_once('-').unwrap_or((key, ""));
            format!("{y} 年 {} 月", m.trim_start_matches('0'))
        }
        "year" => format!("{key} 年"),
        _ => "全部时间".into(),
    };
    Ok(PeriodReport {
        kind: kind.into(),
        key: key.into(),
        title,
        total_seconds,
        app_count: apps.len(),
        active_days,
        keys,
        clicks,
        apps,
        hourly,
        buckets,
    })
}

// ---------- 按应用查看（历史页「按应用」视图，对标 Tai 的应用详情） ----------

/// 全部时间的应用清单（按时长排序，供侧栏选择）
pub fn app_list(conn: &Connection, limit: i64) -> Result<Vec<AppUsage>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.name, COALESCE(a.display_name, a.name), SUM(d.seconds)
             FROM daily_stats d JOIN apps a ON a.id = d.app_id AND a.ignored = 0
             GROUP BY d.app_id ORDER BY 3 DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询应用清单失败: {e}"))?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(AppUsage {
                name: r.get(0)?,
                display_name: r.get(1)?,
                seconds: r.get(2)?,
                bg_seconds: 0,
            })
        })
        .map_err(|e| format!("查询应用清单失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取应用清单失败: {e}"))?);
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPeriodReport {
    pub name: String,
    pub display_name: String,
    /// day | month | year | all
    pub kind: String,
    pub key: String,
    pub title: String,
    pub total_seconds: i64,
    pub active_days: i64,
    pub buckets: Vec<PeriodBucket>,
}

/// 某个应用在指定周期的趋势：日=按小时、月=按天、年=按月、总=按年
pub fn app_period_report(
    conn: &Connection,
    app: &str,
    kind: &str,
    key: &str,
) -> Result<AppPeriodReport, String> {
    let display_name: String = conn
        .query_row(
            "SELECT COALESCE(display_name, name) FROM apps WHERE name = ?1",
            [app],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| app.to_string());

    // 日视图走 hourly_stats（一天的 24 个小时桶）
    if kind == "day" {
        let day = if key.len() == 10 { key.to_string() } else { today_date() };
        let mut stmt = conn
            .prepare(
                "SELECT h.hour, SUM(h.seconds)
                 FROM hourly_stats h JOIN apps a ON a.id = h.app_id AND a.ignored = 0
                 WHERE h.date = ?1 AND a.name = ?2 GROUP BY h.hour",
            )
            .map_err(|e| format!("查询应用日分布失败: {e}"))?;
        let rows = stmt
            .query_map([day.as_str(), app], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i64>(1)?)))
            .map_err(|e| format!("查询应用日分布失败: {e}"))?;
        let mut hours = [0i64; 24];
        for r in rows {
            let (h, s) = r.map_err(|e| format!("读取应用日分布失败: {e}"))?;
            if (0..24).contains(&h) {
                hours[h as usize] = s;
            }
        }
        let buckets: Vec<PeriodBucket> = (0..24)
            .map(|h| PeriodBucket {
                label: format!("{h}"),
                date: day.clone(),
                seconds: hours[h],
            })
            .collect();
        let total = hours.iter().sum();
        let active_days = if total > 0 { 1 } else { 0 };
        return Ok(AppPeriodReport {
            name: app.into(),
            display_name,
            kind: kind.into(),
            key: day.clone(),
            title: format!("{} · {day}", app.trim_end_matches(".exe")),
            total_seconds: total,
            active_days,
            buckets,
        });
    }

    let prefix = period_prefix(kind, key)?;
    let group_expr = match kind {
        "month" => "d.date",
        "year" => "substr(d.date, 1, 7)",
        _ => "substr(d.date, 1, 4)",
    };
    let sql = format!(
        "SELECT {group_expr} AS k, MIN(d.date), SUM(d.seconds)
         FROM daily_stats d JOIN apps a ON a.id = d.app_id AND a.ignored = 0
         WHERE d.date LIKE ?1 AND a.name = ?2
         GROUP BY k ORDER BY k"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询应用趋势失败: {e}"))?;
    let buckets: Vec<PeriodBucket> = stmt
        .query_map([prefix.as_str(), app], |r| {
            let k: String = r.get(0)?;
            let date: String = r.get(1)?;
            Ok(PeriodBucket {
                label: if kind == "month" {
                    k.get(8..).unwrap_or(&k).trim_start_matches('0').to_string()
                } else if kind == "year" {
                    k.get(5..).unwrap_or(&k).to_string()
                } else {
                    k.clone()
                },
                date,
                seconds: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询应用趋势失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    let total_seconds = buckets.iter().map(|b| b.seconds).sum();
    let active_days = buckets.iter().filter(|b| b.seconds > 0).count() as i64;
    let title = match kind {
        "month" => {
            let (y, m) = key.split_once('-').unwrap_or((key, ""));
            format!("{} · {y} 年 {} 月", app.trim_end_matches(".exe"), m.trim_start_matches('0'))
        }
        "year" => format!("{} · {key} 年", app.trim_end_matches(".exe")),
        _ => format!("{} · 全部时间", app.trim_end_matches(".exe")),
    };
    Ok(AppPeriodReport {
        name: app.into(),
        display_name,
        kind: kind.into(),
        key: key.into(),
        title,
        total_seconds,
        active_days,
        buckets,
    })
}

/// 界面偏好（排行条数等）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub apps_top_n: i64,
}

pub fn prefs_get(conn: &Connection) -> Prefs {
    Prefs {
        apps_top_n: get_setting(conn, "ui.apps_top_n")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(10)
            .clamp(5, 20),
    }
}

// ---------- 任意日期区间报表（详细页：按天/按周/按月/按年，可按应用下钻） ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeReport {
    pub from: String,
    pub to: String,
    pub title: String,
    /// 区间天数
    pub days: i64,
    pub total_seconds: i64,
    pub app_count: usize,
    pub active_days: i64,
    pub avg_per_day: i64,
    pub keys: i64,
    pub clicks: i64,
    pub apps: Vec<AppUsage>,
    pub hourly: Vec<HourSlice>,
    pub buckets: Vec<PeriodBucket>,
    /// 仅当按应用下钻时有值
    pub app: Option<String>,
    pub app_display: Option<String>,
}

/// 区间报表：应用排行 / 24 小时分布 / 趋势桶；`app` 有值时只统计该应用
pub fn range_report(
    conn: &Connection,
    from: &str,
    to: &str,
    app: Option<&str>,
) -> Result<RangeReport, String> {
    use chrono::NaiveDate;
    let d_from = NaiveDate::parse_from_str(from, "%Y-%m-%d").map_err(|_| "起始日期格式需为 YYYY-MM-DD")?;
    let d_to = NaiveDate::parse_from_str(to, "%Y-%m-%d").map_err(|_| "结束日期格式需为 YYYY-MM-DD")?;
    if d_to < d_from {
        return Err("结束日期早于起始日期".into());
    }
    let days = (d_to - d_from).num_days() + 1;
    // 上限放宽到 200 年：详细页的"总共"从 2000-01-01 起算，旧上限（10 年）会把"总共"判为非法
    if days > 73_000 {
        return Err("区间过大（最多 200 年）".into());
    }
    let app_filter = app.filter(|a| !a.is_empty());

    let apps_sql = format!(
        "SELECT a.name, COALESCE(a.display_name, a.name), SUM(d.seconds), SUM(d.bg_seconds)
         FROM daily_stats d JOIN apps a ON a.id = d.app_id AND a.ignored = 0
         WHERE d.date >= ?1 AND d.date <= ?2 {app_cond}
         GROUP BY d.app_id ORDER BY 3 DESC LIMIT 300",
        app_cond = if app_filter.is_some() { "AND a.name = ?3" } else { "" }
    );
    let mut stmt = conn.prepare(&apps_sql).map_err(|e| format!("查询区间应用失败: {e}"))?;
    let map_row = |r: &rusqlite::Row| {
        Ok(AppUsage {
            name: r.get(0)?,
            display_name: r.get(1)?,
            seconds: r.get(2)?,
            bg_seconds: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
        })
    };
    let apps: Vec<AppUsage> = match app_filter {
        Some(a) => stmt
            .query_map(rusqlite::params![from, to, a], map_row)
            .map_err(|e| format!("查询区间应用失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect(),
        None => stmt
            .query_map(rusqlite::params![from, to], map_row)
            .map_err(|e| format!("查询区间应用失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect(),
    };

    let hourly_sql = format!(
        "SELECT h.hour, a.name, SUM(h.seconds)
         FROM hourly_stats h JOIN apps a ON a.id = h.app_id AND a.ignored = 0
         WHERE h.date >= ?1 AND h.date <= ?2 {app_cond}
         GROUP BY h.hour, h.app_id",
        app_cond = if app_filter.is_some() { "AND a.name = ?3" } else { "" }
    );
    let mut hstmt = conn.prepare(&hourly_sql).map_err(|e| format!("查询区间时段失败: {e}"))?;
    let h_row = |r: &rusqlite::Row| {
        Ok(HourSlice {
            hour: r.get(0)?,
            app_name: r.get(1)?,
            seconds: r.get(2)?,
        })
    };
    let hourly: Vec<HourSlice> = match app_filter {
        Some(a) => hstmt
            .query_map(rusqlite::params![from, to, a], h_row)
            .map_err(|e| format!("查询区间时段失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect(),
        None => hstmt
            .query_map(rusqlite::params![from, to], h_row)
            .map_err(|e| format!("查询区间时段失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect(),
    };

    // 桶粒度随区间长度自动选：≤62 天按天、≤730 天按月、更长按年
    let group_expr = if days <= 62 {
        "d.date"
    } else if days <= 730 {
        "substr(d.date, 1, 7)"
    } else {
        "substr(d.date, 1, 4)"
    };
    let bucket_sql = format!(
        "SELECT {group_expr} AS k, MIN(d.date), SUM(d.seconds)
         FROM daily_stats d JOIN apps a ON a.id = d.app_id AND a.ignored = 0
         WHERE d.date >= ?1 AND d.date <= ?2 {app_cond}
         GROUP BY k ORDER BY k",
        app_cond = if app_filter.is_some() { "AND a.name = ?3" } else { "" }
    );
    let mut bstmt = conn.prepare(&bucket_sql).map_err(|e| format!("查询区间趋势失败: {e}"))?;
    let b_row = |r: &rusqlite::Row| {
        let k: String = r.get(0)?;
        let date: String = r.get(1)?;
        Ok(PeriodBucket {
            label: if days <= 62 {
                k.get(5..).unwrap_or(&k).replace('-', "/")
            } else if days <= 730 {
                k.get(5..).unwrap_or(&k).to_string()
            } else {
                k.clone()
            },
            date,
            seconds: r.get(2)?,
        })
    };
    let buckets: Vec<PeriodBucket> = match app_filter {
        Some(a) => bstmt
            .query_map(rusqlite::params![from, to, a], b_row)
            .map_err(|e| format!("查询区间趋势失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect(),
        None => bstmt
            .query_map(rusqlite::params![from, to], b_row)
            .map_err(|e| format!("查询区间趋势失败: {e}"))?
            .filter_map(|r| r.ok())
            .collect(),
    };

    let (keys, clicks) = conn
        .query_row(
            "SELECT COALESCE(SUM(key_count),0), COALESCE(SUM(click_count),0)
             FROM input_stats WHERE date >= ?1 AND date <= ?2",
            [from, to],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap_or((0, 0));
    let active_days: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT date) FROM daily_stats WHERE date >= ?1 AND date <= ?2",
            [from, to],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let total_seconds = apps.iter().map(|a| a.seconds).sum();
    let app_display = apps.first().map(|a| a.display_name.clone());
    let title = if from == to {
        from.to_string()
    } else {
        format!("{from} ~ {to}")
    };
    Ok(RangeReport {
        from: from.into(),
        to: to.into(),
        title,
        days,
        total_seconds,
        app_count: apps.len(),
        active_days,
        avg_per_day: if active_days > 0 { total_seconds / active_days } else { 0 },
        keys,
        clicks,
        apps,
        hourly,
        buckets,
        app: app_filter.map(|a| a.to_string()),
        app_display,
    })
}

// ---------- 待办 Markdown 导入导出（对齐 Obsidian / Notion 的 checkbox 语法） ----------

/// 导出为 Markdown：`- [ ] 内容 [优先级:高] [到期:YYYY-MM-DD HH:MM] [完成:…]`
pub fn tasks_export_md(conn: &Connection, dir: &std::path::Path, file_name: &str) -> Result<String, String> {
    let tasks = task_list(conn)?;
    let mut done: Vec<&Task> = Vec::new();
    let mut undone: Vec<&Task> = Vec::new();
    for t in &tasks {
        if t.done {
            done.push(t);
        } else {
            undone.push(t);
        }
    }
    let fmt_due = |ts: Option<i64>| -> String {
        match ts {
            Some(v) => chrono::Local
                .timestamp_opt(v, 0)
                .single()
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
            None => String::new(),
        }
    };
    let prio = |p: i32| match p {
        2 => "高",
        0 => "低",
        _ => "中",
    };
    let today = today_date();
    let mut out = String::new();
    out.push_str(&format!("# 拾刻待办 · {today}\n\n"));
    out.push_str("> 说明：本文件可直接用 Obsidian / Notion 打开；勾选状态用 `- [ ] / - [x]` 表示，\n> 行尾的 `[优先级:…] [到期:…]` 是附加信息，导入时会读回。\n\n");

    let section = |title: &str, list: &Vec<&Task>, out: &mut String| {
        if list.is_empty() {
            return;
        }
        out.push_str(&format!("## {title}\n\n"));
        for t in list {
            let mark = if t.done { "x" } else { " " };
            let mut line = format!("- [{mark}] {}", t.content);
            if t.priority != 1 {
                line.push_str(&format!(" [优先级:{}]", prio(t.priority)));
            }
            if let Some(ts) = t.due_ts {
                line.push_str(&format!(" [到期:{}]", fmt_due(Some(ts))));
            }
            if t.done {
                if let Some(ts) = t.done_ts {
                    line.push_str(&format!(" [完成:{}]", fmt_due(Some(ts))));
                }
            }
            out.push_str(&line);
            out.push_str("\n");
        }
        out.push('\n');
    };
    section("未完成", &undone, &mut out);
    section("已完成", &done, &mut out);

    std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let path = dir.join(file_name);
    std::fs::write(&path, out).map_err(|e| format!("写入 Markdown 失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 从 Markdown 导入：识别 `- [ ] 内容` / `- [x] 内容` 行，行尾元数据可读回
pub fn tasks_import_md(conn: &Connection, path: &str) -> Result<ImportSummary, String> {
    let p = std::path::PathBuf::from(path);
    let size = std::fs::metadata(&p).map_err(|e| format!("读取文件失败: {e}"))?.len();
    if size > 4 * 1024 * 1024 {
        return Err("文件过大（超过 4MB）".into());
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("读取失败（需要 UTF-8 文本）: {e}"))?;

    let re_meta = |line: &str, key: &str| -> Option<String> {
        let open = format!("[{key}:");
        let i = line.find(&open)?;
        let rest = &line[i + open.len()..];
        let j = rest.find(']')?;
        Some(rest[..j].trim().to_string())
    };
    let parse_ts = |s: &str| -> Option<i64> {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
            .ok()
            .map(|d| d.and_utc().timestamp() - chrono::Local::now().offset().local_minus_utc() as i64)
            .or_else(|| {
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(9, 0, 0))
                    .map(|d| d.and_utc().timestamp() - chrono::Local::now().offset().local_minus_utc() as i64)
            })
    };

    let existing: Vec<String> = task_list(conn)?.into_iter().map(|t| t.content).collect();
    let mut skipped = 0usize;
    let mut in_front = false;
    let mut front_seen = 0usize;
    for raw in text.lines() {
        let line = raw.trim();
        // 跳过 YAML front-matter（Obsidian / Notion 导出常带），否则会被当成内容
        if line == "---" {
            if front_seen == 0 {
                in_front = true;
                front_seen = 1;
            } else if in_front {
                in_front = false;
            }
            continue;
        }
        if in_front {
            continue;
        }
        // 项目符号支持 - * + 三种；勾选支持 [ ] / [x] / [X]
        let (done, body) = if let Some(r) = strip_task_marker(line) {
            r
        } else {
            continue;
        };
        // 去掉行尾元数据后再取正文
        let mut content = body.to_string();
        for key in ["[优先级:", "[到期:", "[完成:", "[priority::", "[due::", "📅"] {
            if let Some(i) = content.find(key) {
                content = content[..i].to_string();
            }
        }
        let content = content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        if existing.contains(&content) {
            skipped += 1;
            continue;
        }
        let body_ref: &str = &body;
        let priority = match re_meta(body_ref, "优先级").or_else(|| re_meta(body_ref, "priority")).as_deref() {
            Some("高") | Some("high") | Some("High") => 2,
            Some("低") | Some("low") | Some("Low") => 0,
            Some("中") | Some("medium") | Some("Medium") => 1,
            _ => {
                // Obsidian Tasks 插件的表情写法：⏫ 最高、🔼 高、🔽 低
                if body.contains('⏫') || body.contains('🔼') {
                    2
                } else if body.contains('🔽') {
                    0
                } else {
                    1
                }
            }
        };
        let due = re_meta(body_ref, "到期")
            .or_else(|| re_meta(body_ref, "due"))
            .and_then(|s| parse_ts(&s))
            .or_else(|| {
                // 📅 2026-09-30（Tasks 插件常用）
                let i = body.find('📅')?;
                let rest = &body[i + '📅'.len_utf8()..];
                let date: String = rest.trim().chars().take(10).collect();
                parse_ts(&date)
            });
        task_add(conn, &content, priority, due)?;
        if done {
            if let Ok(id) = conn.query_row("SELECT id FROM tasks WHERE content = ?1", [&content], |r| r.get::<_, i64>(0)) {
                let _ = task_set_done(conn, id, true);
            }
        }
    }
    Ok(ImportSummary {
        apps: 0,
        daily_rows: 0,
        hourly_rows: 0,
        segments: 0,
        skipped,
        cleared: 0,
        backup_path: None,
    })
}

// ---------- 提醒规则 JSON 导入导出（结构化字段多，Markdown 表达不了，故用 JSON） ----------

#[derive(serde::Serialize, serde::Deserialize)]
struct RulePack {
    title: String,
    body: String,
    mode: String,
    interval_minutes: Option<i64>,
    daily_times: Vec<String>,
    sticky: bool,
    card_duration_sec: i32,
    accent_color: Option<String>,
}

pub fn rules_export_json(conn: &Connection, dir: &std::path::Path, file_name: &str) -> Result<String, String> {
    let rules = rule_list(conn)?;
    let pack: Vec<RulePack> = rules
        .into_iter()
        .map(|r| RulePack {
            title: r.title,
            body: r.body,
            mode: r.mode,
            interval_minutes: r.interval_minutes,
            daily_times: r.daily_times,
            sticky: r.sticky,
            card_duration_sec: r.card_duration_sec,
            accent_color: r.accent_color,
        })
        .collect();
    let json = serde_json::to_string_pretty(&pack).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let path = dir.join(file_name);
    std::fs::write(&path, json).map_err(|e| format!("写入失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 导入提醒规则：按标题去重，返回(新增, 跳过)
pub fn rules_import_json(conn: &Connection, path: &str) -> Result<(usize, usize), String> {
    let p = std::path::PathBuf::from(path);
    let size = std::fs::metadata(&p).map_err(|e| format!("读取文件失败: {e}"))?.len();
    if size > 2 * 1024 * 1024 {
        return Err("文件过大（超过 2MB）".into());
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("读取失败: {e}"))?;
    let pack: Vec<RulePack> = serde_json::from_str(&text).map_err(|e| format!("解析失败（需要是导出的规则 JSON）: {e}"))?;
    let existing: Vec<String> = rule_list(conn)?.into_iter().map(|r| r.title).collect();
    let (mut added, mut skipped) = (0usize, 0usize);
    for r in pack {
        if existing.contains(&r.title) {
            skipped += 1;
            continue;
        }
        rule_add(
            conn,
            &r.title,
            &r.body,
            &r.mode,
            r.interval_minutes,
            r.daily_times,
            r.sticky,
            r.card_duration_sec,
            r.accent_color,
        )?;
        added += 1;
    }
    Ok((added, skipped))
}

// ---------- 壁纸与主题包（个性化页；墙纸只存本机数据目录） ----------

const WALLPAPER_MAX_BYTES: u64 = 16 * 1024 * 1024;

pub fn wallpaper_dir() -> Result<PathBuf, String> {
    let dir = resolve_db_path()?
        .parent()
        .ok_or("数据目录异常")?
        .to_path_buf()
        .join("wallpapers");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建壁纸目录失败: {e}"))?;
    Ok(dir)
}

/// 把用户选的图片复制进数据目录（只保存一份，替换旧的）
pub fn wallpaper_set(conn: &Connection, src_path: &str) -> Result<(), String> {
    let src = PathBuf::from(src_path);
    if !src.is_file() {
        return Err("所选图片不存在或无法读取".into());
    }
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !["png", "jpg", "jpeg", "webp", "gif", "bmp"].contains(&ext.as_str()) {
        return Err("只支持图片格式：png / jpg / webp / gif / bmp".into());
    }
    let size = std::fs::metadata(&src).map_err(|e| format!("读取图片失败: {e}"))?.len();
    if size > WALLPAPER_MAX_BYTES {
        return Err("图片过大（超过 16MB），请先压缩再试".into());
    }
    let dir = wallpaper_dir()?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("wall.") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let dst = dir.join(format!("wall.{ext}"));
    std::fs::copy(&src, &dst).map_err(|e| format!("保存壁纸失败: {e}"))?;
    set_setting(conn, "ui.wallpaper", &dst.to_string_lossy())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperFile {
    pub mime: String,
    /// base64（前端转 Blob URL，避免 asset 协议的路径/权限问题）
    pub data: String,
}

/// 读取当前壁纸（没有则 None）
pub fn wallpaper_get(conn: &Connection) -> Result<Option<WallpaperFile>, String> {
    let Some(path) = get_setting(conn, "ui.wallpaper") else {
        return Ok(None);
    };
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Ok(None);
    }
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    let data = std::fs::read(&p).map_err(|e| format!("读取壁纸失败: {e}"))?;
    Ok(Some(WallpaperFile {
        mime: mime.into(),
        data: crate::pet_settings::base64_encode(&data),
    }))
}

/// 全屏提醒背景图：组模型（每组最多 10 张；组数不限、组名可改、组间快速切换）。
/// 播放方式：默认按顺序从左到右循环；开启"随机"后洗牌轮换（一轮内不重复，一轮结束重洗）。
const BG_GROUP_MAX: usize = 10;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BgGroup {
    pub name: String,
    pub files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BgOverview {
    pub groups: Vec<BgGroup>,
    pub active: usize,
    pub rotate: bool,
}

fn bg_groups(conn: &Connection) -> Vec<BgGroup> {
    if let Some(v) = get_setting(conn, "ui.reminder_bg_groups") {
        if let Ok(g) = serde_json::from_str::<Vec<BgGroup>>(&v) {
            if !g.is_empty() {
                return g;
            }
        }
    }
    // 迁移：旧版历史列表 / 旧版单张 → 单组"组一"
    let mut files: Vec<String> = get_setting(conn, "ui.reminder_bg_list")
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();
    if let Some(p) = get_setting(conn, "ui.reminder_bg") {
        if !p.is_empty() && PathBuf::from(&p).is_file() && !files.contains(&p) {
            files.push(p);
        }
    }
    let name = if files.is_empty() { "默认组" } else { "组一" };
    vec![BgGroup { name: name.into(), files }]
}

fn bg_save_groups(conn: &Connection, groups: &[BgGroup]) {
    let _ = set_setting(
        conn,
        "ui.reminder_bg_groups",
        &serde_json::to_string(groups).unwrap_or_else(|_| "[]".into()),
    );
}

fn bg_active_index(conn: &Connection, len: usize) -> usize {
    let i = get_setting(conn, "ui.reminder_bg_active")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if len == 0 { 0 } else { i.min(len - 1) }
}

/// 总览：组列表 + 当前组下标 + 播放方式
pub fn reminder_bg_overview(conn: &Connection) -> BgOverview {
    let groups = bg_groups(conn);
    let active = bg_active_index(conn, groups.len());
    BgOverview {
        groups,
        active,
        rotate: reminder_bg_rotate_get(conn),
    }
}

/// 缩略图条目（含路径与显示配置，供前端移除/编辑）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BgThumb {
    pub path: String,
    pub mime: String,
    pub data: String,
    pub fit: String,
    pub align: String,
    pub zoom: i64,
    pub scrim: i64,
}

/// 全屏背景的单次取用（含显示配置）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BgPick {
    pub mime: String,
    pub data: String,
    pub fit: String,
    pub align: String,
    pub zoom: i64,
    pub scrim: i64,
}

/// 当前组的缩略图（base64，与全屏背景同一套读取路径，避开资产协议的环境差异）
pub fn reminder_bg_thumbs(conn: &Connection) -> Result<Vec<BgThumb>, String> {
    let groups = bg_groups(conn);
    let active = bg_active_index(conn, groups.len());
    let mut out = Vec::new();
    for p in groups
        .get(active)
        .map(|g| g.files.clone())
        .unwrap_or_default()
    {
        if !PathBuf::from(&p).is_file() {
            continue;
        }
        let data = std::fs::read(&p).map_err(|e| format!("读取图片失败: {e}"))?;
        let cfg = bg_img_cfg_of(conn, &p);
        out.push(BgThumb {
            path: p.clone(),
            mime: bg_mime(&p),
            data: crate::pet_settings::base64_encode(&data),
            fit: cfg.fit,
            align: cfg.align,
            zoom: cfg.zoom,
            scrim: cfg.scrim,
        });
    }
    Ok(out)
}

/// 新增图片到当前组（队首，超过 10 张挤掉队尾；不再被任何组引用的文件从磁盘清理）
pub fn reminder_bg_set(conn: &Connection, src_path: &str) -> Result<(), String> {
    let src = PathBuf::from(src_path);
    if !src.is_file() {
        return Err("所选图片不存在或无法读取".into());
    }
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !["png", "jpg", "jpeg", "webp", "gif", "bmp"].contains(&ext.as_str()) {
        return Err("只支持图片格式：png / jpg / webp / gif / bmp".into());
    }
    let size = std::fs::metadata(&src).map_err(|e| format!("读取图片失败: {e}"))?.len();
    if size > WALLPAPER_MAX_BYTES {
        return Err("图片过大（超过 16MB）".into());
    }
    let dir = wallpaper_dir()?;
    // 文件名带纳秒时间戳，避免同扩展名互相覆盖
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dst = dir.join(format!("rbg.{ts}.{ext}"));
    std::fs::copy(&src, &dst).map_err(|e| format!("保存图片失败: {e}"))?;

    let mut groups = bg_groups(conn);
    let active = bg_active_index(conn, groups.len());
    let dst_str = dst.to_string_lossy().to_string();
    {
        let g = &mut groups[active];
        g.files.insert(0, dst_str.clone());
        g.files.truncate(BG_GROUP_MAX);
    }
    bg_prune_files(&groups, &dir)?;
    bg_save_groups(conn, &groups);
    set_setting(conn, "ui.reminder_bg", &dst_str)?;
    // 组内容变了：重置播放进度
    let _ = set_setting(conn, "ui.reminder_bg_cursor", "0");
    let _ = set_setting(conn, "ui.reminder_bg_hand", "[]");
    Ok(())
}

/// 从当前组移除一张；若所有组都不再引用该文件则从磁盘删除
pub fn reminder_bg_remove_file(conn: &Connection, path: &str) -> Result<(), String> {
    let mut groups = bg_groups(conn);
    let active = bg_active_index(conn, groups.len());
    if let Some(g) = groups.get_mut(active) {
        g.files.retain(|f| f != path);
    }
    let dir = wallpaper_dir()?;
    bg_prune_files(&groups, &dir)?;
    bg_save_groups(conn, &groups);
    let _ = set_setting(conn, "ui.reminder_bg_cursor", "0");
    let _ = set_setting(conn, "ui.reminder_bg_hand", "[]");
    Ok(())
}

/// 清掉不被任何组引用的 rbg.* 文件
fn bg_prune_files(groups: &[BgGroup], dir: &std::path::Path) -> Result<(), String> {
    let referenced: Vec<&String> = groups.iter().flat_map(|g| g.files.iter()).collect();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("rbg.") {
                let p = e.path().to_string_lossy().to_string();
                if !referenced.contains(&&p) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    Ok(())
}

/// 新建组，返回其下标
pub fn bg_add_group(conn: &Connection, name: &str) -> Result<usize, String> {
    let mut groups = bg_groups(conn);
    let n = groups.len() + 1;
    let name = if name.trim().is_empty() {
        format!("组{n}")
    } else {
        name.trim().to_string()
    };
    groups.push(BgGroup { name, files: Vec::new() });
    bg_save_groups(conn, &groups);
    Ok(groups.len() - 1)
}

/// 重命名组
pub fn bg_rename_group(conn: &Connection, index: usize, name: &str) -> Result<(), String> {
    let mut groups = bg_groups(conn);
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 20 {
        return Err("组名需为 1~20 个字符".into());
    }
    match groups.get_mut(index) {
        Some(g) => g.name = name.into(),
        None => return Err("组不存在".into()),
    }
    bg_save_groups(conn, &groups);
    Ok(())
}

/// 删除组（至少保留一组）；被删组的图片若无其他组引用则从磁盘清理
pub fn bg_remove_group(conn: &Connection, index: usize) -> Result<(), String> {
    let mut groups = bg_groups(conn);
    if groups.len() <= 1 {
        return Err("至少要保留一个组".into());
    }
    if index >= groups.len() {
        return Err("组不存在".into());
    }
    groups.remove(index);
    let _ = bg_prune_files(&groups, &wallpaper_dir()?);
    bg_save_groups(conn, &groups);
    // 活动组下标收敛
    let mut active = bg_active_index(conn, groups.len() + 1);
    if active >= groups.len() {
        active = 0;
    }
    let _ = set_setting(conn, "ui.reminder_bg_active", &active.to_string());
    let _ = set_setting(conn, "ui.reminder_bg_cursor", "0");
    let _ = set_setting(conn, "ui.reminder_bg_hand", "[]");
    Ok(())
}

/// 切换当前组（播放进度随之重置）
pub fn bg_set_active_group(conn: &Connection, index: usize) -> Result<(), String> {
    let groups = bg_groups(conn);
    if index >= groups.len() {
        return Err("组不存在".into());
    }
    bg_save_groups(conn, &groups); // 顺带把迁移后的组结构落库
    let _ = set_setting(conn, "ui.reminder_bg_active", &index.to_string());
    let _ = set_setting(conn, "ui.reminder_bg_cursor", "0");
    let _ = set_setting(conn, "ui.reminder_bg_hand", "[]");
    Ok(())
}

/// 每张背景图的显示配置：fit = cover(铺满裁剪)/contain(完整显示)；
/// align = center/top/bottom/left/right/... 九宫格；scrim = 蒙版浓度 0~90(%)
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BgImgCfg {
    pub fit: String,
    pub align: String,
    /// 缩放百分比：100 = 原始；50~300（旧数据缺省按 100）
    #[serde(default = "default_bg_zoom")]
    pub zoom: i64,
    pub scrim: i64,
}

fn default_bg_zoom() -> i64 {
    100
}

impl Default for BgImgCfg {
    fn default() -> Self {
        // 100 = 原始渐变浓度（与未加可调蒙版前的观感一致）；align 用 "X% Y%" 百分比定位
        Self { fit: "cover".into(), align: "50% 50%".into(), zoom: 100, scrim: 100 }
    }
}

const BG_IMGCFG_KEY: &str = "ui.reminder_bg_imgcfg";

fn bg_imgcfg_all(conn: &Connection) -> std::collections::HashMap<String, BgImgCfg> {
    get_setting(conn, BG_IMGCFG_KEY)
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

/// 旧版九宫格关键词 → 百分比（迁移用）
fn align_keyword_to_pct(a: &str) -> Option<String> {
    let m: &[(&str, &str)] = &[
        ("center", "50% 50%"),
        ("top", "50% 0%"),
        ("bottom", "50% 100%"),
        ("left", "0% 50%"),
        ("right", "100% 50%"),
        ("left top", "0% 0%"),
        ("right top", "100% 0%"),
        ("left bottom", "0% 100%"),
        ("right bottom", "100% 100%"),
    ];
    m.iter().find(|(k, _)| *k == a).map(|(_, v)| v.to_string())
}

fn valid_align(a: &str) -> bool {
    let parts: Vec<&str> = a.split(' ').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            p.ends_with('%')
                && p[..p.len() - 1]
                    .parse::<i64>()
                    .map(|x| (0..=100).contains(&x))
                    .unwrap_or(false)
        })
}

fn bg_img_cfg_of(conn: &Connection, path: &str) -> BgImgCfg {
    let all = bg_imgcfg_all(conn);
    let mut cfg = all.get(path).cloned().unwrap_or_default();
    if !["cover", "contain"].contains(&cfg.fit.as_str()) {
        cfg.fit = "cover".into();
    }
    if !valid_align(&cfg.align) {
        cfg.align = align_keyword_to_pct(&cfg.align).unwrap_or_else(|| "50% 50%".into());
    }
    if !(50..=300).contains(&cfg.zoom) {
        cfg.zoom = 100;
    }
    cfg.scrim = cfg.scrim.clamp(0, 100);
    cfg
}

/// 保存某张背景图的显示配置
pub fn reminder_bg_imgcfg_set(conn: &Connection, path: &str, cfg: &BgImgCfg) -> Result<(), String> {
    let mut all = bg_imgcfg_all(conn);
    let mut c = cfg.clone();
    if !["cover", "contain"].contains(&c.fit.as_str()) {
        return Err("填充方式非法".into());
    }
    if !valid_align(&c.align) {
        return Err("位置非法".into());
    }
    if !(50..=300).contains(&c.zoom) {
        return Err("缩放需在 50~300%".into());
    }
    c.scrim = c.scrim.clamp(0, 100);
    all.insert(path.to_string(), c);
    set_setting(conn, BG_IMGCFG_KEY, &serde_json::to_string(&all).unwrap_or_else(|_| "{}".into()))
}

fn bg_mime(path: &str) -> String {
    let ext = PathBuf::from(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
    .into()
}

/// 轮换方式：false = 顺序（从左到右循环，默认）；true = 随机洗牌（一轮内不重复）
pub fn reminder_bg_rotate_get(conn: &Connection) -> bool {
    get_setting(conn, "ui.reminder_bg_rotate").map(|v| v == "1").unwrap_or(false)
}

pub fn reminder_bg_set_rotate(conn: &Connection, on: bool) {
    let _ = set_setting(conn, "ui.reminder_bg_rotate", if on { "1" } else { "0" });
}

/// 试看：强制下一次全屏提醒使用指定图片（轮换开=塞进洗牌队列队首；关=把游标拨到它）
pub fn reminder_bg_test_pick(conn: &Connection, path: &str) -> Result<(), String> {
    let groups = bg_groups(conn);
    let active = bg_active_index(conn, groups.len());
    let files: Vec<String> = groups
        .get(active)
        .map(|g| g.files.clone())
        .unwrap_or_default();
    if !files.iter().any(|f| f == path) {
        return Err("这张图不在当前组里".into());
    }
    if reminder_bg_rotate_get(conn) {
        let _ = set_setting(conn, "ui.reminder_bg_hand", &serde_json::to_string(&vec![path.to_string()]).unwrap_or_else(|_| "[]".into()));
    } else if let Some(i) = files.iter().position(|f| f == path) {
        let _ = set_setting(conn, "ui.reminder_bg_cursor", &i.to_string());
    }
    Ok(())
}

/// 取本次全屏提醒要用的背景图（当前组）：
/// 顺序 = 游标从左到右循环；随机 = 洗牌队列逐张出队，一轮结束自动重洗（轮内不重复）
pub fn reminder_bg_pick(conn: &Connection) -> Result<Option<BgPick>, String> {
    let groups = bg_groups(conn);
    let active = bg_active_index(conn, groups.len());
    let mut files: Vec<String> = groups
        .get(active)
        .map(|g| g.files.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| PathBuf::from(p).is_file())
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    let path = if reminder_bg_rotate_get(conn) && files.len() > 1 {
        // 随机洗牌：出队；队列空了重洗（时间戳做简易种子）
        let mut hand: Vec<String> = get_setting(conn, "ui.reminder_bg_hand")
            .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| files.contains(p))
            .collect();
        let last = get_setting(conn, "ui.reminder_bg_last");
        if hand.is_empty() {
            let mut seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
                .unwrap_or(12345)
                .max(1);
            for i in (1..files.len()).rev() {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let j = ((seed >> 33) as usize) % (i + 1);
                files.swap(i, j);
            }
            hand = files.clone();
        }
        // 跨轮衔接处避免与上一张相同
        if hand.len() > 1 {
            if let Some(l) = &last {
                if hand[0] == *l {
                    hand.swap(0, 1);
                }
            }
        }
        let picked = hand.remove(0);
        let _ = set_setting(
            conn,
            "ui.reminder_bg_hand",
            &serde_json::to_string(&hand).unwrap_or_else(|_| "[]".into()),
        );
        let _ = set_setting(conn, "ui.reminder_bg_last", &picked);
        picked
    } else {
        // 顺序循环：游标从左到右
        let len = files.len() as u64;
        let cur = get_setting(conn, "ui.reminder_bg_cursor")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let _ = set_setting(conn, "ui.reminder_bg_cursor", &(cur + 1).to_string());
        files[(cur % len) as usize].clone()
    };
    let data = std::fs::read(&path).map_err(|e| format!("读取图片失败: {e}"))?;
    let cfg = bg_img_cfg_of(conn, &path);
    Ok(Some(BgPick {
        mime: bg_mime(&path),
        data: crate::pet_settings::base64_encode(&data),
        fit: cfg.fit,
        align: cfg.align,
        zoom: cfg.zoom,
        scrim: cfg.scrim,
    }))
}

#[allow(dead_code)]
pub fn reminder_bg_clear(conn: &Connection) -> Result<(), String> {
    if let Ok(dir) = wallpaper_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.filter_map(|e| e.ok()) {
                if e.file_name().to_string_lossy().starts_with("rbg.") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    set_setting(conn, "ui.reminder_bg", "")?;
    set_setting(conn, "ui.reminder_bg_list", "[]")?;
    set_setting(conn, "ui.reminder_bg_groups", r#"[{"name":"默认组","files":[]}]"#)?;
    set_setting(conn, "ui.reminder_bg_active", "0")?;
    set_setting(conn, "ui.reminder_bg_cursor", "0")?;
    set_setting(conn, "ui.reminder_bg_hand", "[]")?;
    set_setting(conn, "ui.reminder_bg_last", "")?;
    set_setting(conn, "ui.reminder_bg_rotate", "0")
}

pub fn wallpaper_clear(conn: &Connection) -> Result<(), String> {
    if let Ok(dir) = wallpaper_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("wall.") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    set_setting(conn, "ui.wallpaper", "")
}

/// 主题包导出到用户选定路径
pub fn theme_export(path: &str, json: &str) -> Result<(), String> {
    std::fs::write(path, json).map_err(|e| format!("写入主题包失败: {e}"))
}

/// 主题包导入（只读文本，限制 1MB）
pub fn theme_import(path: &str) -> Result<String, String> {
    let p = PathBuf::from(path);
    let size = std::fs::metadata(&p).map_err(|e| format!("读取主题包失败: {e}"))?.len();
    if size > 1024 * 1024 {
        return Err("主题包文件过大，不是有效的主题包".into());
    }
    std::fs::read_to_string(&p).map_err(|e| format!("读取主题包失败: {e}"))
}

/// 数据目录与文件信息（设置页展示，回答「数据存在哪」）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataInfo {
    pub dir: String,
    pub db_path: String,
    pub db_bytes: u64,
    pub wal_bytes: u64,
    pub fallback: bool,
    pub exists: bool,
}

pub fn data_info() -> Result<DataInfo, String> {
    let db_path = resolve_db_path()?;
    let dir = db_path
        .parent()
        .ok_or("数据目录异常")?
        .to_string_lossy()
        .to_string();
    let size = |p: &std::path::Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let wal = db_path.with_extension("db-wal");
    let appdata = std::env::var("APPDATA")
        .map(|d| PathBuf::from(d).join("TallyMoment").join("Data"))
        .ok();
    let fallback = appdata
        .as_ref()
        .map(|a| db_path.starts_with(a))
        .unwrap_or(false);
    Ok(DataInfo {
        exists: db_path.is_file(),
        db_bytes: size(&db_path),
        wal_bytes: size(&wal),
        db_path: db_path.to_string_lossy().to_string(),
        dir,
        fallback,
    })
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
/// mode：merge = 与本地时长累加（历史行为）；fill = 只补本地没有的日期/小时；
///       replace = 先清空本地时间数据（segments/daily/hourly）再导入。
///       键鼠计数、任务、提醒规则、日志任何模式都不动。
pub fn import_tai_data(src_path: &str, conn: &Connection, mode: &str) -> Result<ImportSummary, String> {
    use std::collections::HashMap;

    let mode = match mode {
        "fill" => "fill",
        "replace" => "replace",
        _ => "merge",
    };

    let src = Connection::open_with_flags(
        src_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("打开 Tai 数据库失败: {e}"))?;

    // Tai 的表名有两种形态：导出/教程里是 App / DailyLog / HoursLog，
    // 真实运行库是 AppModels / DailyLogModels / HoursLogModels（首次接真库才发现）
    let table = |base: &str, src: &Connection| -> Result<String, String> {
        for cand in [base.to_string(), format!("{base}Models")] {
            let n: i64 = src
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                    [&cand],
                    |r| r.get(0),
                )
                .map_err(|e| format!("读取表结构失败: {e}"))?;
            if n > 0 {
                return Ok(cand);
            }
        }
        Err(format!("不是有效的 Tai 数据库（缺少表 {base}）"))
    };
    let app_t = table("App", &src)?;
    let daily_t = table("DailyLog", &src)?;
    let hours_t = table("HoursLog", &src)?;

    let mut summary = ImportSummary {
        apps: 0,
        daily_rows: 0,
        hourly_rows: 0,
        segments: 0,
        skipped: 0,
        cleared: 0,
        backup_path: None,
    };

    // replace：先清空本地时间数据（汇总与明细），键鼠/任务/规则/日志不受影响
    if mode == "replace" {
        for table in ["segments", "daily_stats", "hourly_stats"] {
            let n = conn
                .execute(&format!("DELETE FROM {table}"), [])
                .map_err(|e| format!("清空 {table} 失败: {e}"))?;
            summary.cleared += n as usize;
        }
    }

    // 应用维表：name 优先取 exe 文件名（小写），退化用 Name；显示名 Alias > Name
    let mut app_map: HashMap<i64, i64> = HashMap::new();
    let mut stmt = src
        .prepare(&format!("SELECT ID, Name, Alias, File FROM {app_t}"))
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
        // 本地已有该应用但没显示名（此前拾刻自己记录的）→ 用 Tai 的 Alias/Name 回填，
        // 否则导入后排行里还是 floral-notepaper.exe 这类进程名（Tai 里明明叫"花笺"）
        if !display.is_empty() {
            let _ = conn.execute(
                "UPDATE apps SET display_name = ?2
                 WHERE id = ?1 AND (display_name IS NULL OR display_name = ''
                    OR display_name = name OR display_name = replace(name, '.exe', ''))",
                rusqlite::params![app_id, display],
            );
        }
        app_map.insert(tid, app_id);
        summary.apps += 1;
    }

    // 每日汇总（聚合同日多行）
    let mut daily: HashMap<(String, i64), i64> = HashMap::new();
    let mut stmt = src
        .prepare(&format!("SELECT AppModelID, Date, Time FROM {daily_t}"))
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
        .prepare(&format!("SELECT AppModelID, DataTime, Time FROM {hours_t}"))
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

    match mode {
        "fill" => {
            // 只补本地没有的行：已存在（同日同应用 / 同日同小时同应用）一律跳过，不覆盖不累加
            for ((date, app_id), secs) in &daily {
                let exists: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM daily_stats WHERE date = ?1 AND app_id = ?2",
                    rusqlite::params![date, app_id],
                    |r| r.get(0),
                )
                .map_err(|e| format!("查询日汇总失败: {e}"))?;
                if exists > 0 {
                    summary.skipped += 1;
                    continue;
                }
                upsert_daily(conn, date, *app_id, *secs)?;
                summary.daily_rows += 1;
            }
            for ((date, hour, app_id), secs) in &hourly {
                let exists: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM hourly_stats WHERE date = ?1 AND hour = ?2 AND app_id = ?3",
                    rusqlite::params![date, hour, app_id],
                    |r| r.get(0),
                )
                .map_err(|e| format!("查询小时汇总失败: {e}"))?;
                if exists > 0 {
                    summary.skipped += 1;
                    continue;
                }
                upsert_hourly(conn, date, *hour, *app_id, *secs)?;
                summary.hourly_rows += 1;
            }
        }
        _ => {
            // merge（历史行为：时长累加）与 replace（已清空，直接写入）
            for ((date, app_id), secs) in &daily {
                upsert_daily(conn, date, *app_id, *secs)?;
                summary.daily_rows += 1;
            }
            for ((date, hour, app_id), secs) in &hourly {
                upsert_hourly(conn, date, *hour, *app_id, *secs)?;
                summary.hourly_rows += 1;
            }
        }
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

    let mut tasks = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, content, priority, due_ts, done, done_ts, created_ts,
                    repeat_mode, template_id, start_ts, 0
             FROM tasks ORDER BY id",
        )
        .map_err(|e| format!("导出 tasks 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "content": r.get::<_, String>(1)?,
                "priority": r.get::<_, i32>(2)?,
                "dueTs": r.get::<_, Option<i64>>(3)?,
                "done": r.get::<_, i64>(4)? != 0,
                "doneTs": r.get::<_, Option<i64>>(5)?,
                "createdTs": r.get::<_, i64>(6)?,
            }))
        })
        .map_err(|e| format!("导出 tasks 失败: {e}"))?;
    for r in rows {
        tasks.push(r.map_err(|e| format!("导出 tasks 失败: {e}"))?);
    }

    let mut rules = Vec::new();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {RULE_COLS} FROM reminder_rules ORDER BY id"
        ))
        .map_err(|e| format!("导出 rules 失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let daily: String = r.get(6)?;
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "body": r.get::<_, String>(2)?,
                "mode": r.get::<_, String>(3)?,
                "intervalMinutes": r.get::<_, Option<i64>>(4)?,
                "sticky": r.get::<_, i64>(5)? != 0,
                "dailyTimes": serde_json::from_str::<Vec<String>>(&daily).unwrap_or_default(),
                "cardDurationSec": r.get::<_, i32>(7)?,
                "accentColor": r.get::<_, Option<String>>(8)?,
                "enabled": r.get::<_, i64>(9)? != 0,
            }))
        })
        .map_err(|e| format!("导出 rules 失败: {e}"))?;
    for r in rows {
        rules.push(r.map_err(|e| format!("导出 rules 失败: {e}"))?);
    }

    let payload = json!({
        "app": "tallymoment",
        "version": 1,
        "exportedAt": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "apps": apps,
        "segments": segments,
        "hourly": hourly,
        "daily": daily,
        "tasks": tasks,
        "rules": rules,
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
        ("tasks", "任务"),
        ("reminder_rules", "提醒规则"),
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
        cleared: 0,
        backup_path: None,
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

    let tasks_in = v.get("tasks").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for t in tasks_in {
        let content = t.get("content").and_then(|x| x.as_str()).unwrap_or("");
        if content.is_empty() {
            summary.skipped += 1;
            continue;
        }
        conn.execute(
            "INSERT INTO tasks(content, priority, due_ts, done, done_ts, created_ts) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                content,
                t.get("priority").and_then(|x| x.as_i64()).unwrap_or(1) as i32,
                t.get("dueTs").and_then(|x| x.as_i64()),
                t.get("done").and_then(|x| x.as_bool()).unwrap_or(false) as i64,
                t.get("doneTs").and_then(|x| x.as_i64()),
                t.get("createdTs").and_then(|x| x.as_i64()).unwrap_or(0),
            ],
        )
        .map_err(|e| format!("恢复任务失败: {e}"))?;
    }

    let rules_in = v.get("rules").and_then(|x| x.as_array()).unwrap_or(&empty_arr);
    for c in rules_in {
        let title = c.get("title").and_then(|x| x.as_str()).unwrap_or("");
        if title.is_empty() {
            summary.skipped += 1;
            continue;
        }
        conn.execute(
            "INSERT INTO reminder_rules(title, body, mode, interval_minutes, daily_times, sticky, card_duration_sec, accent_color, enabled)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                title,
                c.get("body").and_then(|x| x.as_str()).unwrap_or(""),
                c.get("mode").and_then(|x| x.as_str()).unwrap_or("daily"),
                c.get("intervalMinutes").and_then(|x| x.as_i64()),
                serde_json::to_string(
                    &c.get("dailyTimes").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>()).unwrap_or_default()
                ).unwrap_or_else(|_| "[]".into()),
                c.get("sticky").and_then(|x| x.as_bool()).unwrap_or(false) as i64,
                c.get("cardDurationSec").and_then(|x| x.as_i64()).unwrap_or(10) as i32,
                c.get("accentColor").and_then(|x| x.as_str()),
                c.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true) as i64,
            ],
        )
        .map_err(|e| format!("恢复提醒规则失败: {e}"))?;
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
        let s = import_tai_data(src_path.to_str().unwrap(), &conn, "merge").unwrap();

        assert_eq!(s.apps, 2);
        assert_eq!(s.daily_rows, 2);
        assert_eq!(s.hourly_rows, 2);

        let h: i64 = conn
            .query_row(
                "SELECT h.seconds FROM hourly_stats h JOIN apps a ON a.id = h.app_id AND a.ignored = 0
                 WHERE a.name = 'chrome.exe' AND h.date = '2025-01-02' AND h.hour = 8",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(h, 1500, "重复小时桶应合并求和");

        let d: i64 = conn
            .query_row(
                "SELECT dd.seconds FROM daily_stats dd JOIN apps a ON a.id = dd.app_id AND a.ignored = 0
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
    fn tai_import_fill_skips_existing_and_replace_resets() {
        let src_path = tmp_db("tai-source2.db");
        let src = Connection::open(&src_path).unwrap();
        src.execute_batch(
            "CREATE TABLE App (ID INTEGER PRIMARY KEY, Name TEXT, Alias TEXT, File TEXT);
             CREATE TABLE DailyLog (ID INTEGER PRIMARY KEY, Date TEXT, Time INTEGER, AppModelID INTEGER);
             CREATE TABLE HoursLog (ID INTEGER PRIMARY KEY, DataTime TEXT, Time INTEGER, AppModelID INTEGER);
             INSERT INTO App VALUES (1, 'Chrome', '', 'C:/Program/chrome.exe');
             INSERT INTO DailyLog VALUES (1, '2025-01-02 00:00:00', 3600, 1);
             INSERT INTO HoursLog VALUES (1, '2025-01-02 08:00:00', 1200, 1);",
        )
        .unwrap();

        let dst = tmp_db("ours-fill.db");
        let conn = open(&dst).unwrap();
        // 本地已有：chrome 当天 500 秒（daily+hourly）+ 一段明细 + 一条键鼠记录
        conn.execute("INSERT INTO apps(name) VALUES('chrome.exe')", []).unwrap();
        let aid: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO daily_stats(date, app_id, seconds) VALUES('2025-01-02', ?1, 500)",
            [aid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hourly_stats(date, hour, app_id, seconds) VALUES('2025-01-02', 8, ?1, 500)",
            [aid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO segments(app_id, start_ts, end_ts, title) VALUES(?1, 100, 200, 't')",
            [aid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO input_stats(date, hour, key_count, click_count) VALUES('2025-01-02', 8, 10, 5)",
            [],
        )
        .unwrap();

        // fill：同日同应用 / 同日同小时已存在 → 全部跳过，本地 500 不变
        let s = import_tai_data(src_path.to_str().unwrap(), &conn, "fill").unwrap();
        assert_eq!(s.daily_rows, 0);
        assert_eq!(s.hourly_rows, 0);
        assert_eq!(s.skipped, 2);
        let d: i64 = conn
            .query_row(
                "SELECT seconds FROM daily_stats WHERE date='2025-01-02' AND app_id=?1",
                [aid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(d, 500, "fill 不应叠加时长");

        // replace：清空时间数据后写导入值；键鼠记录保留
        let s2 = import_tai_data(src_path.to_str().unwrap(), &conn, "replace").unwrap();
        assert!(s2.cleared >= 3, "应清掉 daily+hourly+segments");
        let d2: i64 = conn
            .query_row(
                "SELECT seconds FROM daily_stats WHERE date='2025-01-02' AND app_id=?1",
                [aid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(d2, 3600, "replace 后应为导入值");
        let h2: i64 = conn
            .query_row(
                "SELECT seconds FROM hourly_stats WHERE date='2025-01-02' AND hour=8 AND app_id=?1",
                [aid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(h2, 1200);
        let seg: i64 = conn
            .query_row("SELECT COUNT(*) FROM segments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(seg, 0, "segments 应被清空");
        let input: i64 = conn
            .query_row("SELECT COUNT(*) FROM input_stats", [], |r| r.get(0))
            .unwrap();
        assert_eq!(input, 1, "键鼠数据保留");
    }

    #[test]
    fn habit_grid_merges_template_instances() {
        let path = std::env::temp_dir().join("tm-habit.db");
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).unwrap();
        let now = Local::now().timestamp();
        conn.execute(
            "INSERT INTO tasks(content, priority, created_ts, repeat_mode) VALUES('每日喝水', 1, ?1, 'daily')",
            [now - 3 * 86400],
        )
        .unwrap();
        let tpl: i64 = conn.last_insert_rowid();
        // 昨天的实例：完成，用时 10 分钟；今天的实例：还没做
        conn.execute(
            "INSERT INTO tasks(content, priority, created_ts, repeat_mode, template_id, due_ts, done, done_ts, start_ts)
             VALUES('每日喝水', 1, ?1, 'daily', ?2, ?3, 1, ?4, ?5)",
            rusqlite::params![now - 86400, tpl, now - 86400 + 3600, now - 86400 + 4200, now - 86400 + 600],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks(content, priority, created_ts, repeat_mode, template_id, due_ts)
             VALUES('每日喝水', 1, ?1, 'daily', ?2, ?3)",
            rusqlite::params![now, tpl, now + 3600],
        )
        .unwrap();

        let grid = habit_grid(&conn, 30).unwrap();
        assert_eq!(grid.len(), 1, "只有一个模板");
        let row = &grid[0];
        // 模板行自己（created 3 天前）+ 昨天实例 + 今天实例 = 3 天
        assert_eq!(row.cells.len(), 3);
        let done_cells: Vec<_> = row.cells.iter().filter(|c| c.done).collect();
        assert_eq!(done_cells.len(), 1, "只有昨天完成");
        assert_eq!(done_cells[0].spent_min, Some(60), "done_ts-start_ts=3600 秒，应为 60 分钟");
    }

    #[test]
    fn habit_toggle_flips_and_backfills() {
        let path = std::env::temp_dir().join("tm-habit-toggle.db");
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).unwrap();
        let now = Local::now().timestamp();
        conn.execute(
            "INSERT INTO tasks(content, priority, created_ts, repeat_mode) VALUES('每日锻炼', 1, ?1, 'daily')",
            [now - 5 * 86400],
        )
        .unwrap();
        let tpl: i64 = conn.last_insert_rowid();
        let yesterday = (Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();

        // 当天没有实例 → 补卡并标记完成
        assert!(habit_toggle(&conn, tpl, &yesterday).unwrap());
        let grid = habit_grid(&conn, 30).unwrap();
        let cell = grid[0].cells.iter().find(|c| c.date == yesterday).unwrap();
        assert!(cell.done, "补卡应直接完成");

        // 再点一次 → 取消完成
        assert!(!habit_toggle(&conn, tpl, &yesterday).unwrap());
        let grid = habit_grid(&conn, 30).unwrap();
        let cell = grid[0].cells.iter().find(|c| c.date == yesterday).unwrap();
        assert!(!cell.done, "再点应取消");
    }

    #[test]
    #[ignore = "仅本机：只读用户真实 Tai 库验证三种导入模式；cargo test tai_real -- --ignored"]
    fn tai_real_data_import_modes_smoke() {
        let src = "E:/02_Programs/Utilities/Tai/Data/data.db";
        assert!(std::path::Path::new(src).exists(), "Tai 库不在预期路径");
        let dst = tmp_db("tai-real.db");
        let conn = open(&dst).unwrap();
        let s_fill = import_tai_data(src, &conn, "fill").unwrap();
        let s_fill2 = import_tai_data(src, &conn, "fill").unwrap();
        assert_eq!(s_fill2.daily_rows + s_fill2.hourly_rows, 0, "fill 第二遍应全跳过");
        let s_merge = import_tai_data(src, &conn, "merge").unwrap();
        let s_replace = import_tai_data(src, &conn, "replace").unwrap();
        assert!(s_replace.daily_rows > 0 && s_replace.hourly_rows > 0, "replace 后应有数据");
        eprintln!(
            "fill: daily={} hourly={} skipped={} | merge: daily={} hourly={} | replace: cleared={} daily={} hourly={}",
            s_fill.daily_rows, s_fill.hourly_rows, s_fill.skipped,
            s_merge.daily_rows, s_merge.hourly_rows,
            s_replace.cleared, s_replace.daily_rows, s_replace.hourly_rows
        );
    }

    #[test]
    #[ignore = "仅本机一次性操作：把真实 Tai 库导入拾刻 dev 实库（fill 补充去重，先自动备份）；cargo test tai_into_app -- --ignored --nocapture"]
    fn tai_real_import_into_app_db() {
        let src = "E:/02_Programs/Utilities/Tai/Data/data.db";
        assert!(std::path::Path::new(src).exists(), "Tai 库不在预期路径");
        // 拾刻 dev 实库（exe 旁 Data/tallymoment.db）；执行前确认拾刻程序已退出
        let app_db = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/debug/Data/tallymoment.db");
        assert!(app_db.exists(), "拾刻实库不在 {:?}", app_db);
        let conn = open(&app_db).unwrap();
        // 导入前先备份整库（与界面「替代全部」同一套备份逻辑）
        let backup = backup_database(&conn).unwrap();
        eprintln!("已备份到 {backup}");
        let s = import_tai_data(src, &conn, "fill").unwrap();
        eprintln!(
            "导入完成：应用 {} 个，日汇总 {} 行，时段 {} 行，跳过已有 {} 条",
            s.apps, s.daily_rows, s.hourly_rows, s.skipped
        );
        // 回填 Tai 式友好名（exe 版本资源的 FileDescription）
        let n = backfill_friendly_names(&conn).unwrap();
        eprintln!("回填友好名 {n} 个");
    }

    #[test]
    fn reminder_style_roundtrip() {
        let path = std::env::temp_dir().join("tm-style.db");
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).unwrap();
        let _ = conn.execute("DELETE FROM reminder_rules", []);
        rule_add_style(
            &conn, "全屏休息", "起来走走", "interval", Some(45), vec![], true, 15, None, "fullscreen",
        )
        .unwrap();
        rule_add(&conn, "普通卡片", "喝水", "interval", Some(30), vec![], false, 10, None).unwrap();
        let list = rule_list(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|r| r.style == "fullscreen"));
        assert!(list.iter().any(|r| r.style == "card"));
        // 非法方式被拒
        assert!(rule_add_style(&conn, "x", "", "interval", Some(1), vec![], false, 10, None, "popup").is_err());
        // 调度用的查询也要带出 style
        let due = rule_enabled(&conn).unwrap();
        assert!(due.iter().any(|r| r.style == "fullscreen"));
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn repeats_are_materialized_once_a_day() {
        let path = std::env::temp_dir().join("tm-repeat.db");
        let _ = std::fs::remove_file(&path);
        let conn = open(&path).unwrap();
        // 关键回归：老的/已迁移过的库再次打开必须能成功（ALTER 不能混进建表批处理）
        drop(conn);
        let conn = open(&path).unwrap();
        let _ = conn.execute("DELETE FROM tasks", []);
        let today = "2030-06-15";
        // 模板：创建时间设为 10 天前（以免"今天建的模板"被跳过）
        conn.execute(
            "INSERT INTO tasks(content, priority, due_ts, created_ts, repeat_mode)
             VALUES('每日喝水', 1, NULL, ?1, 'daily')",
            [chrono::Local::now().timestamp() - 10 * 86400],
        )
        .unwrap();
        // 每周模板：用一个肯定不匹配今天的星期（用今天 +1 天建的 → 不同星期几）
        conn.execute(
            "INSERT INTO tasks(content, priority, due_ts, created_ts, repeat_mode)
             VALUES('每周复盘', 1, NULL, ?1, 'weekly')",
            [chrono::Local::now().timestamp() - 86400],
        )
        .unwrap();

        let made = materialize_repeats(&conn, today).unwrap();
        assert!(made >= 1, "每日模板应生成今日实例");
        // 幂等：再跑一次不重复生成
        let again = materialize_repeats(&conn, today).unwrap();
        assert_eq!(again, 0, "同一天不应重复生成实例");
        // 实例带 template_id
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE template_id IS NOT NULL", [], |r| r.get(0))
            .unwrap();
        assert!(n >= 1);
        // 非法重复方式被拒
        assert!(task_add_repeat(&conn, "x", 1, None, "hourly", None).is_err());
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_restore_roundtrip_and_empty_guard() {
        let p1 = tmp_db("exp-src.db");
        let dir = p1.parent().unwrap().to_path_buf();
        let conn = open(&p1).unwrap();
        let app_id = app_id_for(&conn, "test.exe", "Test", "C:/test.exe").unwrap();
        add_seconds(&conn, app_id, 1_700_000_000, 1_700_000_600).unwrap();
        write_segment(&conn, app_id, 1_700_000_000, 1_700_000_600, "测试区间").unwrap();
        task_add(&conn, "测试任务", 1, None).unwrap();
        rule_add(&conn, "测试规则", "", "interval", Some(30), vec![], false, 10, None).unwrap();

        let exported = export_json(&conn, &dir, "export-test.json").unwrap();

        let p2 = tmp_db("restored.db");
        let conn2 = open(&p2).unwrap();
        let s = restore_json(&conn2, &exported).unwrap();
        assert_eq!(s.apps, 1);
        assert_eq!(s.segments, 1);

        for t in ["apps", "segments", "hourly_stats", "daily_stats", "tasks", "reminder_rules"] {
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
        assert!(import_tai_data(src_path.to_str().unwrap(), &conn, "merge").is_err());
    }
}

#[cfg(test)]
mod tai_export_tests {
    use super::*;

    #[test]
    fn tai_export_creates_compatible_db_and_csv() {
        let dir = std::env::temp_dir().join(format!("tm-tai-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ours = dir.join("ours.db");
        let _ = std::fs::remove_file(&ours);
        let conn = open(&ours).unwrap();
        let app_id = app_id_for(&conn, "test.exe", "测试应用", "C:/test.exe").unwrap();
        add_seconds(&conn, app_id, 1_700_000_000, 1_700_000_600).unwrap();
        write_segment(&conn, app_id, 1_700_000_000, 1_700_000_600, "t").unwrap();

        let files = export_tai(&conn, &dir, "Tai数据").unwrap();
        assert_eq!(files.len(), 4);
        let xlsx_path = dir.join("Tai数据.xlsx");
        let meta = std::fs::metadata(&xlsx_path).unwrap();
        assert!(meta.len() > 200, "xlsx 应有实际内容");

        // 打开生成的 Tai 库并校验
        let tai = Connection::open(&dir.join("Tai数据.db")).unwrap();
        let app_count: i64 = tai
            .query_row("SELECT COUNT(*) FROM \"App\"", [], |r| r.get(0))
            .unwrap();
        assert_eq!(app_count, 1);
        let (total, dcount): (i64, i64) = tai
            .query_row(
                "SELECT TotalTime, (SELECT COUNT(*) FROM \"DailyLog\") FROM \"App\" WHERE ID = ?1",
                [app_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(total, 600);
        assert_eq!(dcount, 1);
        let date_text: String = tai
            .query_row("SELECT Date FROM \"DailyLog\" LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert!(date_text.ends_with("00:00:00"), "Date 应为 datetime 文本");
        let hcount: i64 = tai
            .query_row("SELECT COUNT(*) FROM \"HoursLog\"", [], |r| r.get(0))
            .unwrap();
        assert!(hcount >= 1);

        // CSV 校验
        let csv = std::fs::read_to_string(dir.join("Tai数据-每日.csv")).unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("日期,应用,描述,时长,分类"));
        assert!(csv.contains("测试应用"));

        // 删除：全部时间记录
        let sum = delete_range(&conn, "all", None).unwrap();
        assert!(sum.segments >= 1 && sum.daily >= 1 && sum.apps >= 1);
        for t in ["segments", "daily_stats", "apps"] {
            let left: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(left, 0, "{t} 应为空");
        }
    }
}
