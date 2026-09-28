//! 【未完成·下一轮继续】每日日志后端（B1b）
//! ⚠️ 本文件当前**未被 lib.rs 引用**（`mod notes;` 尚未加入），因此不参与编译，
//! 也不会影响任何现有功能。下一轮我会用分块写入的方式补完，并在 lib.rs 注册模块与命令。
//!
//! 计划接口：get / save / recent / add_image / image_data / export_md / import_md
//! 数据布局：`<数据目录>/notes/<日期>/`（图片）+ `daily_notes` 表（date, content, images, updated_ts）

use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::storage;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DailyNote {
    pub date: String,
    pub content: String,
    /// 附件文件名（相对 notes/<date>/）
    pub images: Vec<String>,
    pub updated_ts: i64,
}

pub fn notes_root() -> Result<PathBuf, String> {
    let dir = storage::resolve_db_path()?
        .parent()
        .ok_or("数据目录异常")?
        .to_path_buf()
        .join("notes");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
    Ok(dir)
}

fn note_dir(date: &str) -> Result<PathBuf, String> {
    let d = notes_root()?.join(date);
    std::fs::create_dir_all(&d).map_err(|e| format!("创建日期目录失败: {e}"))?;
    Ok(d)
}

fn now_ts() -> i64 {
    chrono::Local::now().timestamp()
}

pub fn get(conn: &Connection, date: &str) -> Result<Option<DailyNote>, String> {
    let row = conn
        .query_row(
            "SELECT content, images, updated_ts FROM daily_notes WHERE date = ?1",
            [date],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .ok();
    let Some((content, images, updated_ts)) = row else {
        return Ok(None);
    };
    let images: Vec<String> = serde_json::from_str(&images).unwrap_or_default();
    Ok(Some(DailyNote {
        date: date.into(),
        content,
        images,
        updated_ts,
    }))
}

pub fn save(conn: &Connection, date: &str, content: &str, images: &[String]) -> Result<(), String> {
    let imgs = serde_json::to_string(images).map_err(|e| format!("序列化附件失败: {e}"))?;
    conn.execute(
        "INSERT INTO daily_notes(date, content, images, updated_ts) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(date) DO UPDATE SET content = ?2, images = ?3, updated_ts = ?4",
        rusqlite::params![date, content, imgs, now_ts()],
    )
    .map_err(|e| format!("保存日志失败: {e}"))?;
    Ok(())
}

/// 最近 N 天有内容的日志（新→旧）
pub fn recent(conn: &Connection, limit: i64) -> Result<Vec<DailyNote>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date, content, images, updated_ts FROM daily_notes
             ORDER BY date DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询日志失败: {e}"))?;
    let rows = stmt
        .query_map([limit], |r| {
            let images: String = r.get(2)?;
            Ok(DailyNote {
                date: r.get(0)?,
                content: r.get(1)?,
                images: serde_json::from_str(&images).unwrap_or_default(),
                updated_ts: r.get(3)?,
            })
        })
        .map_err(|e| format!("查询日志失败: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("读取日志失败: {e}"))?);
    }
    Ok(out)
}

/// 把一张图片复制进当天目录，返回文件名
pub fn add_image(date: &str, src: &str) -> Result<String, String> {
    let p = Path::new(src);
    if !p.is_file() {
        return Err("图片不存在".into());
    }
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !["png", "jpg", "jpeg", "webp", "gif", "bmp"].contains(&ext.as_str()) {
        return Err("只支持图片格式：png / jpg / webp / gif / bmp".into());
    }
    let size = std::fs::metadata(p).map_err(|e| format!("读取图片失败: {e}"))?.len();
    if size > 24 * 1024 * 1024 {
        return Err("图片过大（超过 24MB）".into());
    }
    let dir = note_dir(date)?;
    let name = format!("{}.{}", chrono::Local::now().timestamp_millis(), ext);
    std::fs::copy(p, dir.join(&name)).map_err(|e| format!("保存图片失败: {e}"))?;
    Ok(name)
}

/// 读取附件（base64，前端转 Blob/data URL 显示）
pub fn image_data(date: &str, name: &str) -> Result<(String, String), String> {
    if name.contains('/') || name.contains('\') || name.contains("..") {
        return Err("附件名不合法".into());
    }
    let p = notes_root()?.join(date).join(name);
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
    let data = std::fs::read(&p).map_err(|e| format!("读取附件失败: {e}"))?;
    Ok((mime.into(), crate::pet_settings::base64_encode(&data)))
}

/// 导出全部日志为 Markdown（每篇一个 .md，图片放到同目录 images/）
pub fn export_md(conn: &Connection, dir: &Path) -> Result<usize, String> {
    let notes = recent(conn, 100_000)?;
    if notes.is_empty() {
        return Err("还没有日志可导出".into());
    }
    let img_dir = dir.join("images");
    std::fs::create_dir_all(&img_dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let mut n = 0usize;
    for note in notes {
        let mut body = format!("# {}\n\n", note.date);
        body.push_str(&note.content);
        if !note.content.ends_with('\n') {
            body.push('\n');
        }
        for img in &note.images {
            if let Ok((_, _)) = image_data(&note.date, img) {
                let src = notes_root()?.join(&note.date).join(img);
                let _ = std::fs::copy(&src, img_dir.join(img));
                body.push_str(&format!("\n![]({}/{})\n", "images", img));
            }
        }
        let path = dir.join(format!("{}.md", note.date));
        std::fs::write(&path, body).map_err(|e| format!("写入失败: {e}"))?;
        n += 1;
    }
    Ok(n)
}

/// 导入 Markdown：文件名（YYYY-MM-DD）作为日期；`![](images/x.png)` 的图片一并搬进来
pub fn import_md(conn: &Connection, path: &str) -> Result<(usize, usize), String> {
    let p = PathBuf::from(path);
    let text = std::fs::read_to_string(&p).map_err(|e| format!("读取失败（需要 UTF-8）: {e}"))?;
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    // 日期优先取文件名，其次取正文首个 # 标题
    let date = if chrono::NaiveDate::parse_from_str(&stem, "%Y-%m-%d").is_ok() {
        stem.clone()
    } else {
        text.lines()
            .find_map(|l| l.trim().strip_prefix("# ").map(|s| s.trim().to_string()))
            .filter(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok())
            .ok_or("无法确定日期（文件名或首个 # 标题需要是 YYYY-MM-DD）")?
    };

    let mut images: Vec<String> = Vec::new();
    let base = p.parent().unwrap_or(Path::new("."));
    for (i, line) in text.lines().enumerate() {
        let _ = i;
        // 支持 ![](path) 与 ![alt](path)
        let mut rest = line;
        while let Some(idx) = rest.find("](") {
            let after = &rest[idx + 2..];
            if let Some(end) = after.find(')') {
                let rel = after[..end].trim();
                if !rel.starts_with("http") {
                    let src = base.join(rel);
                    if src.is_file() {
                        match add_image(&date, &src.to_string_lossy()) {
                            Ok(name) => images.push(name),
                            Err(_) => {}
                        }
                    }
                }
                rest = &after[end..];
            } else {
                break;
            }
        }
    }

    // 正文：去掉首行的 `# 日期` 标题
    let mut content = String::new();
    for line in text.lines() {
        if line.trim() == format!("# {date}") {
            continue;
        }
        content.push_str(line);
        content.push('\n');
    }
    let existed = get(conn, &date)?.is_some();
    save(conn, &date, content.trim_end(), &images)?;
    Ok((1, if existed { 1 } else { 0 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tm-notes-{}", std::proces
