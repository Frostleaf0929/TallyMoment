//! 桌宠设置与 Mver 模型包管理（批次 5，对标新版 BongoCat 的设置子集）
use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

use crate::storage;

pub const BASE_W: f64 = 372.0;
pub const BASE_H: f64 = 226.0;
pub const BUILTIN_ID: &str = "builtin";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PetSettings {
    pub scale: f64,
    pub opacity: f64,
    pub always_on_top: bool,
    pub pass_through: bool,
    pub mirror: bool,
    pub pos_x: f64,
    pub pos_y: f64,
    /// "builtin" 或模型目录名
    pub active_model: String,
    /// 自定义模型的完整目录（builtin 时为 None）
    pub active_model_dir: Option<String>,
    /// keyboard | standard
    pub mode: String,
}

fn f_setting(conn: &Connection, key: &str, default: f64) -> f64 {
    storage::get_setting(conn, key)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

fn b_setting(conn: &Connection, key: &str, default: bool) -> bool {
    storage::get_setting(conn, key)
        .map(|v| v == "1")
        .unwrap_or(default)
}

fn valid_model_dir(models: &Path, name: &str) -> bool {
    name != BUILTIN_ID && models.join(name).join("config.json").is_file()
}

pub fn load(conn: &Connection) -> PetSettings {
    let active_model = storage::get_setting(conn, "pet.active_model").unwrap_or_else(|| BUILTIN_ID.into());
    let models = models_dir(conn);
    let (active_model_dir, mode) = if valid_model_dir(&models, &active_model) {
        let dir = models.join(&active_model);
        let m = detect_mode(&dir).unwrap_or_else(|| "keyboard".into());
        (Some(dir.to_string_lossy().to_string()), m)
    } else {
        (None, "keyboard".into())
    };
    let uses_custom = active_model_dir.is_some();
    PetSettings {
        scale: f_setting(conn, "pet.scale", 100.0).clamp(50.0, 200.0),
        opacity: f_setting(conn, "pet.opacity", 100.0).clamp(30.0, 100.0),
        always_on_top: b_setting(conn, "pet.always_on_top", true),
        pass_through: b_setting(conn, "pet.pass_through", false),
        mirror: b_setting(conn, "pet.mirror", false),
        pos_x: f_setting(conn, "pet.pos_x", -1.0),
        pos_y: f_setting(conn, "pet.pos_y", -1.0),
        active_model_dir,
        mode,
        active_model: if uses_custom {
            active_model
        } else {
            BUILTIN_ID.into()
        },
    }
}

pub fn save(conn: &Connection, s: &PetSettings) -> Result<(), String> {
    for (k, v) in [
        ("pet.scale", s.scale.to_string()),
        ("pet.opacity", s.opacity.to_string()),
        ("pet.always_on_top", (s.always_on_top as i32).to_string()),
        ("pet.pass_through", (s.pass_through as i32).to_string()),
        ("pet.mirror", (s.mirror as i32).to_string()),
        ("pet.pos_x", s.pos_x.to_string()),
        ("pet.pos_y", s.pos_y.to_string()),
        ("pet.active_model", s.active_model.clone()),
    ] {
        storage::set_setting(conn, k, &v)?;
    }
    Ok(())
}

/// 把设置应用到桌宠窗口（尺寸/置顶/穿透/位置；透明度由前端 CSS 应用）
pub fn apply_window(app: &AppHandle, s: &PetSettings) {
    if let Some(win) = app.get_webview_window("pet") {
        let k = s.scale / 100.0;
        let _ = win.set_size(tauri::LogicalSize::new(BASE_W * k, BASE_H * k));
        let _ = win.set_always_on_top(s.always_on_top);
        let _ = win.set_ignore_cursor_events(s.pass_through);
        if s.pos_x >= 0.0 && s.pos_y >= 0.0 {
            let _ = win.set_position(tauri::LogicalPosition::new(s.pos_x, s.pos_y));
        } else if let Ok(Some(m)) = app.primary_monitor() {
            let sc = m.scale_factor();
            let size = m.size();
            let x = size.width as f64 / sc - BASE_W * k - 392.0;
            let y = size.height as f64 / sc - BASE_H * k - 306.0;
            let _ = win.set_position(tauri::LogicalPosition::new(x, y));
        }
    }
    let _ = app.emit_to("pet", "pet-settings-changed", s.clone());
}

// ---------- 模型管理 ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// keyboard | standard
    pub mode: String,
    pub builtin: bool,
    pub active: bool,
}

pub fn models_dir(_conn: &Connection) -> PathBuf {
    storage::resolve_db_path()
        .map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("models")
}

/// 识别模型目录模式（keyboard 双爪 / standard 单手）
fn detect_mode(dir: &Path) -> Option<String> {
    let img = dir.join("img");
    if img.join("keyboard").join("lefthand").is_dir() && img.join("keyboard").join("righthand").is_dir() {
        return Some("keyboard".into());
    }
    if img.join("standard").join("hand").is_dir() {
        return Some("standard".into());
    }
    None
}

fn model_list_from(models: &Path, active: &str) -> Vec<ModelInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(models) else {
        return out;
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| detect_mode(&e.path()).is_some())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        let dir = models.join(&name);
        let mode = detect_mode(&dir).unwrap_or_default();
        let is_active = name == active;
        out.push(ModelInfo {
            id: name.clone(),
            name,
            mode,
            builtin: false,
            active: is_active,
        });
    }
    out
}

/// 模型列表（内置兔子洞永远在最前）
pub fn list_models(conn: &Connection) -> Result<Vec<ModelInfo>, String> {
    let s = load(conn);
    let models = models_dir(conn);
    let _ = std::fs::create_dir_all(&models);
    let mut out = vec![ModelInfo {
        id: BUILTIN_ID.into(),
        name: "兔子洞（内置）".into(),
        mode: "keyboard".into(),
        builtin: true,
        active: s.active_model == BUILTIN_ID,
    }];
    out.extend(model_list_from(&models, &s.active_model));
    Ok(out)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("创建目录失败: {e}"))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("读取目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录失败: {e}"))?;
        let ty = entry.file_type().map_err(|e| format!("读取失败: {e}"))?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| format!("复制文件失败: {e}"))?;
        }
    }
    Ok(())
}

fn now_millis() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 在解压结果里找模型根：staging 本身或其下一层子目录
fn find_root_down(staging: &Path) -> Option<PathBuf> {
    if staging.join("config.json").is_file() && staging.join("img").is_dir() {
        return Some(staging.to_path_buf());
    }
    let Ok(entries) = std::fs::read_dir(staging) else {
        return None;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() && p.join("config.json").is_file() && p.join("img").is_dir() {
            return Some(p);
        }
    }
    None
}

/// 导入模型（文件夹或 ZIP）。模型 id 取自源名称；同名已存在则拒绝。
pub fn import_model_in(models: &Path, src_path: &str) -> Result<ModelInfo, String> {
    let src = PathBuf::from(src_path);
    let _ = std::fs::create_dir_all(models);

    // id：文件夹导入取源文件夹名；ZIP 导入取解压后顶层文件夹名
    let folder_id = if src.is_dir() {
        src.file_name().map(|f| f.to_string_lossy().to_string())
    } else {
        None
    };

    let staging = std::env::temp_dir().join(format!(
        "tm-import-{}-{}",
        std::process::id(),
        now_millis()
    ));
    let _ = std::fs::remove_dir_all(&staging);
    if src.is_file() {
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if ext != "zip" {
            return Err("请选择模型文件夹或 .zip 压缩包".into());
        }
        let file = std::fs::File::open(&src).map_err(|e| format!("打开 zip 失败: {e}"))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {e}"))?;
        for i in 0..archive.len() {
            let mut f = archive
                .by_index(i)
                .map_err(|e| format!("读取 zip 条目失败: {e}"))?;
            let Some(rel) = f.enclosed_name() else { continue };
            let out = staging.join(&rel);
            if f.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| format!("创建目录失败: {e}"))?;
            } else {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
                }
                let mut out_f = std::fs::File::create(&out).map_err(|e| format!("写文件失败: {e}"))?;
                std::io::copy(&mut f, &mut out_f).map_err(|e| format!("解压失败: {e}"))?;
            }
        }
    } else if src.is_dir() {
        copy_dir_recursive(&src, &staging)?;
    } else {
        return Err("路径不存在".into());
    }

    // 找模型根：staging 本身或其下一层（zip 常带顶层文件夹）
    let root = find_root_down(&staging);
    let root = match root {
        Some(r) => r,
        None => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err("不是有效的 Mver 模型包（需要 config.json + img/ 目录）".into());
        }
    };
    let Some(mode) = detect_mode(&root) else {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("模型缺少可用的分层素材（keyboard 双爪或 standard 单手）".into());
    };

    let id = folder_id.unwrap_or_else(|| {
        root.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("model-{}", now_millis()))
    });
    let target = models.join(&id);
    if target.exists() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("同名模型「{id}」已存在"));
    }
    copy_dir_recursive(&root, &target)?;
    let _ = std::fs::remove_dir_all(&staging);

    Ok(ModelInfo {
        active: false,
        builtin: false,
        id: id.clone(),
        name: id,
        mode,
    })
}

pub fn delete_model_in(models: &Path, conn: &Connection, id: &str) -> Result<(), String> {
    if id == BUILTIN_ID {
        return Err("内置模型不可删除".into());
    }
    let dir = models.join(id);
    if dir.join("config.json").is_file() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除模型失败: {e}"))?;
    }
    if storage::get_setting(conn, "pet.active_model").as_deref() == Some(id) {
        storage::set_setting(conn, "pet.active_model", BUILTIN_ID)?;
    }
    Ok(())
}

/// 读取某模型 config.json（前端解析按键矩阵）
pub fn model_config_in(models: &Path, id: &str) -> Result<serde_json::Value, String> {
    if id == BUILTIN_ID {
        return Err("内置模型配置由前端内置".into());
    }
    let txt = std::fs::read_to_string(models.join(id).join("config.json"))
        .map_err(|e| format!("读取 config 失败: {e}"))?;
    serde_json::from_str(&txt).map_err(|e| format!("config 解析失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    /// 模型目录是全局 exe 相对路径，测试串行使用
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn make_model(dir: &Path, name: &str) {
        let root = dir.join(name);
        let lh = root.join("img").join("keyboard").join("lefthand");
        let rh = root.join("img").join("keyboard").join("righthand");
        std::fs::create_dir_all(&lh).unwrap();
        std::fs::create_dir_all(&rh).unwrap();
        std::fs::write(
            root.join("config.json"),
            r#"{"keyboard":{"lefthand":[[65]],"righthand":[[76]]}}"#,
        )
        .unwrap();
        std::fs::write(lh.join("0.png"), b"png").unwrap();
        std::fs::write(rh.join("0.png"), b"png").unwrap();
    }

    fn temp_db(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tm-pet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{tag}.db"));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn import_list_delete_model_roundtrip() {
        let _guard = lock();
        let conn = storage::open(&temp_db("s1")).unwrap();
        let models = models_dir(&conn);
        let _ = std::fs::remove_dir_all(&models);
        std::fs::create_dir_all(&models).unwrap();

        // 源模型放在模型库之外（真实场景：用户从下载目录导入）
        let src_base = std::env::temp_dir().join(format!("tm-src-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&src_base);
        make_model(&src_base, "model-a");
        let src = src_base.join("model-a");

        let info = import_model_in(&models, src.to_str().unwrap()).unwrap();
        assert_eq!(info.id, "model-a");
        assert_eq!(info.mode, "keyboard");

        let list = list_models(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|m| m.builtin));
        assert!(list.iter().any(|m| m.id == "model-a" && m.mode == "keyboard"));

        // 同名再导入应拒绝
        assert!(import_model_in(&models, src.to_str().unwrap()).is_err());

        // 启用导入的模型 → 设置与目录联动
        storage::set_setting(&conn, "pet.active_model", "model-a").unwrap();
        let s = load(&conn);
        assert_eq!(s.active_model, "model-a");
        assert!(s.active_model_dir.is_some());
        assert_eq!(s.mode, "keyboard");

        // 删除 → 回退内置
        delete_model_in(&models, &conn, "model-a").unwrap();
        let s2 = load(&conn);
        assert_eq!(s2.active_model, BUILTIN_ID);

        let _ = std::fs::remove_dir_all(&models);
        let _ = std::fs::remove_dir_all(&src_base);
    }

    #[test]
    fn import_zip_model() {
        let _guard = lock();
        let conn = storage::open(&temp_db("s2")).unwrap();
        let models = models_dir(&conn);
        let _ = std::fs::remove_dir_all(&models);
        std::fs::create_dir_all(&models).unwrap();

        // 源 zip 放模型库之外；zip 内含顶层文件夹「我的模型」
        let stage = std::env::temp_dir().join(format!("tm-zsrc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&stage);
        make_model(&stage, "我的模型");
        let zip_path = std::env::temp_dir().join(format!("tm-zsrc-{}-m.zip", std::process::id()));
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            zw.start_file(
                "我的模型/config.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(
                &mut zw,
                br#"{"keyboard":{"lefthand":[[65]],"righthand":[[76]]}}"#,
            )
            .unwrap();
            zw.start_file(
                "我的模型/img/keyboard/lefthand/0.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(&mut zw, b"png").unwrap();
            zw.start_file(
                "我的模型/img/keyboard/righthand/0.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(&mut zw, b"png").unwrap();
            zw.finish().unwrap();
        }

        let info = import_model_in(&models, zip_path.to_str().unwrap()).unwrap();
        assert_eq!(info.id, "我的模型");
        let list = list_models(&conn).unwrap();
        assert!(list.iter().any(|m| m.id == "我的模型"));

        let _ = std::fs::remove_dir_all(&models);
        let _ = std::fs::remove_dir_all(&stage);
        let _ = std::fs::remove_file(&zip_path);
    }

    #[test]
    fn reject_non_model_folder() {
        let _guard = lock();
        let conn = storage::open(&temp_db("s3")).unwrap();
        let models = models_dir(&conn);
        let _ = std::fs::remove_dir_all(&models);
        std::fs::create_dir_all(&models).unwrap();
        let junk = models.join(".junk-src");
        std::fs::create_dir_all(&junk).unwrap();
        std::fs::write(junk.join("readme.txt"), b"not a model").unwrap();
        assert!(import_model_in(&models, junk.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&models);
    }

    #[test]
    fn settings_roundtrip_defaults() {
        let conn = storage::open(&temp_db("s4")).unwrap();
        let s1 = load(&conn);
        assert_eq!(s1.scale, 100.0);
        assert!(s1.always_on_top);
        let mut s2 = s1.clone();
        s2.scale = 150.0;
        s2.mirror = true;
        save(&conn, &s2).unwrap();
        let s3 = load(&conn);
        assert_eq!(s3.scale, 150.0);
        assert!(s3.mirror);
        assert!(!s3.pass_through);
    }
}
