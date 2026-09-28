//! 每日日志（B 阶段）：Markdown 正文 + 图片附件
//! 数据位置：`<数据目录>/notes/<日期>/`（图片）+ 数据库 `daily_notes` 表（正文与附件清单）
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
    if date.len() != 10 || date.contains('/') || date.contains(char::from(92u8)) {
        return Err("日期需要是 YYYY-MM-DD".into());
    }
    let d = notes_root()?.join(date);
    std::fs::create_dir_all(&d).map_err(|e| format!("创建日期目录失败: {e}"))?;
    Ok(d)
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
    Ok(Some(DailyNote {
        date: date.into(),
        images: serde_json::from_str(&images).unwrap_or_default(),
        content,
        updated_ts,
    }))
}

pub fn save(conn: &Connection, date: &str, content: &str, images: &[String]) -> Result<(), String> {
    let imgs = serde_json::to_string(images).map_err(|e| format!("序列化附件失败: {e}"))?;
    let ts = chrono::Local::now().timestamp();
    conn.execute(
        "INSERT INTO daily_notes(date, content, images, updated_ts) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(date) DO UPDATE SET content = ?2, images = ?3, updated_ts = ?4",
        rusqlite::params![date, content, imgs, ts],
    )
    .map_err(|e| format!("保存日志失败: {e}"))?;
    Ok(())
}

/// 最近 N 天的日志（新→旧）
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
    let size = std::fs::metadata(p)
        .map_err(|e| format!("读取图片失败: {e}"))?
        .len();
    if size > 24 * 1024 * 1024 {
        return Err("图片过大（超过 24MB）".into());
    }
    let dir = note_dir(date)?;
    let name = format!("{}.{}", chrono::Local::now().timestamp_millis(), ext);
    std::fs::copy(p, dir.join(&name)).map_err(|e| format!("保存图片失败: {e}"))?;
    Ok(name)
}

/// 读取附件（base64，前端转 data URL 显示）
pub fn image_data(date: &str, name: &str) -> Result<(String, String), String> {
    if name.contains('/') || name.contains(char::from(92u8)) || name.contains("..") {
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

/// 一次读出某天的全部附件（mime, base64, 文件名）——避免前端逐张 IPC 造成卡顿
pub fn images_of(date: &str, names: &[String]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for name in names {
        if let Ok((mime, b64)) = image_data(date, name) {
            out.push((name.clone(), mime, b64));
        }
    }
    out
}

/// 导出全部日志为 Markdown：每篇一个 `<日期>.md`，图片复制到同目录 `images/`
pub fn export_md(conn: &Connection, dir: &Path) -> Result<usize, String> {
    let notes = recent(conn, 100_000)?;
    if notes.is_empty() {
        return Err("还没有日志可导出".into());
    }
    let img_dir = dir.join("images");
    std::fs::create_dir_all(&img_dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let root = notes_root()?;
    let mut n = 0usize;
    for note in notes {
        let mut body = format!("# {}\n\n", note.date);
        body.push_str(&note.content);
        if !note.content.ends_with('\n') {
            body.push('\n');
        }
        for img in &note.images {
            let src = root.join(&note.date).join(img);
            if src.is_file() {
                let _ = std::fs::copy(&src, img_dir.join(img));
                body.push_str(&format!("\n![](images/{img})\n"));
            }
        }
        std::fs::write(dir.join(format!("{}.md", note.date)), body)
            .map_err(|e| format!("写入失败: {e}"))?;
        n += 1;
    }
    Ok(n)
}

/// 导入单个 Markdown 为日志：日期取文件名（YYYY-MM-DD）或首个 `# 日期` 标题，
/// `![](相对路径)` 引用的图片一并复制进数据目录
pub fn import_md(conn: &Connection, path: &str) -> Result<(usize, usize), String> {
    let p = PathBuf::from(path);
    let size = std::fs::metadata(&p)
        .map_err(|e| format!("读取文件失败: {e}"))?
        .len();
    if size > 4 * 1024 * 1024 {
        return Err("文件过大（超过 4MB）".into());
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("读取失败（需要 UTF-8）: {e}"))?;
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let date = if chrono::NaiveDate::parse_from_str(&stem, "%Y-%m-%d").is_ok() {
        stem.clone()
    } else {
        text.lines()
            .find_map(|l| l.trim().strip_prefix("# ").map(|s| s.trim().to_string()))
            .filter(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok())
            .ok_or("无法确定日期（文件名或首个标题需要是 YYYY-MM-DD）")?
    };

    let base = p.parent().unwrap_or(Path::new("."));
    let mut images: Vec<String> = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let Some(end) = after.find(')') else { break };
            let rel = after[..end].trim();
            if !rel.starts_with("http") {
                let src = base.join(rel);
                if src.is_file() {
                    if let Ok(name) = add_image(&date, &src.to_string_lossy()) {
                        images.push(name);
                    }
                }
            }
            rest = &after[end..];
        }
    }

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
        let dir = std::env::temp_dir().join(format!("tm-notes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{tag}.db"));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn note_roundtrip_with_image_and_md() {
        let conn = storage::open(&temp_db("n1")).unwrap();
        let date = "2000-01-02";

        // 造一张"图片"并附加到日志
        let src = std::env::temp_dir().join("tm-note-img.png");
        std::fs::write(&src, b"\x89PNG\r\n\x1a\nfake").unwrap();
        let name = add_image(date, &src.to_string_lossy()).expect("附件应能保存");

        save(&conn, date, "# 今天\n\n写了点东西", &[name.clone()]).unwrap();
        let got = get(&conn, date).unwrap().expect("应能读回日志");
        assert!(got.content.contains("写了点东西"));
        assert_eq!(got.images.len(), 1);

        let (mime, b64) = image_data(date, &name).unwrap();
        assert_eq!(mime, "image/png");
        assert!(!b64.is_empty());
        // 附件名不能穿目录
        assert!(image_data(date, "../x.png").is_err());
        assert_eq!(recent(&conn, 10).unwrap().len(), 1);

        // 导出 -> 同日期再导入（应覆盖）
        let out = std::env::temp_dir().join("tm-note-out");
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        assert_eq!(export_md(&conn, &out).unwrap(), 1);
        let md = out.join(format!("{date}.md"));
        assert!(md.is_file(), "应导出 {date}.md");
        let (added, replaced) = import_md(&conn, &md.to_string_lossy()).unwrap();
        assert_eq!(added, 1);
        assert_eq!(replaced, 1);
        // 导入时把图片也搬了进来
        assert!(!get(&conn, date).unwrap().unwrap().images.is_empty());

        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_dir_all(&out);
    }
}
