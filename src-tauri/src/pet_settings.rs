//! 桌宠设置与 Mver 模型包管理（批次 5 建立，批次 6 按新版 BongoCat 的导入器思路重做）
//!
//! 相对批次 5 的变化：
//! - 识别更宽：接受 `config.json` 或 `bongocat.skin.json`；包根可向下探 3 层；模式识别补 gamepad
//! - 导入更稳：先解压/复制到模型库内的 `.import-*.tmp` → 复检 → 原子改名提交；失败清理临时目录
//! - 去重更聪明：内容指纹（FNV-1a 64）相同 → 提示「已导入过」；同名不同内容 → 自动加 `-2` 后缀
//! - 名字可改：显示名覆盖存 settings（只改显示名，绝不动磁盘目录名）
//! - 错误全中文，ZIP 有大小/条目/深度/路径安全上限
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

use crate::storage;

pub const BASE_W: f64 = 372.0;
pub const BASE_H: f64 = 226.0;
pub const BUILTIN_ID: &str = "builtin";
/// 内置模型的实际素材来源（原版 BongoCat 分层图；兔子洞皮肤只有 Live2D，不在其中）
pub const BUILTIN_NAME: &str = "原版 BongoCat（内置）";
const LABELS_KEY: &str = "pet.model_labels";

/// Mver 包的两种配置文件名
const SKIN_JSON_NAMES: [&str; 2] = ["config.json", "bongocat.skin.json"];
/// ZIP 安全上限（对标新版 BongoCat）
const MAX_ENTRIES: usize = 20_000;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRY_DEPTH: usize = 24;
/// 素材读取上限
const MAX_ASSET_FILES: usize = 400;
const MAX_ASSET_BYTES: u64 = 48 * 1024 * 1024;

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
    /// keyboard | gamepad | standard
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

fn has_skin_json(dir: &Path) -> bool {
    SKIN_JSON_NAMES.iter().any(|n| dir.join(n).is_file())
}

pub fn valid_model_dir(models: &Path, name: &str) -> bool {
    name != BUILTIN_ID && has_skin_json(&models.join(name))
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

// ---------- 显示名覆盖（只改名字，不改磁盘目录） ----------

pub fn model_labels(conn: &Connection) -> BTreeMap<String, String> {
    storage::get_setting(conn, LABELS_KEY)
        .and_then(|txt| serde_json::from_str::<BTreeMap<String, String>>(&txt).ok())
        .unwrap_or_default()
}

/// 设置显示名；传空或与默认名相同则清除覆盖（对标新版 BongoCat）
pub fn set_model_label(
    conn: &Connection,
    models: &Path,
    id: &str,
    label: &str,
) -> Result<(), String> {
    if id != BUILTIN_ID && !models.join(id).is_dir() {
        return Err("模型不存在".into());
    }
    let mut labels = model_labels(conn);
    let trimmed = label.trim();
    let default_name = if id == BUILTIN_ID {
        BUILTIN_NAME
    } else {
        id
    };
    if trimmed.is_empty() || trimmed == default_name {
        labels.remove(id);
    } else {
        if trimmed.chars().count() > 40 {
            return Err("显示名最长 40 个字符".into());
        }
        labels.insert(id.to_string(), trimmed.to_string());
    }
    let txt = serde_json::to_string(&labels).map_err(|e| format!("保存显示名失败: {e}"))?;
    storage::set_setting(conn, LABELS_KEY, &txt)
}

// ---------- 模型管理 ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    /// 显示名（可被覆盖）
    pub name: String,
    /// 默认名（用于「还原名字」与判断是否被改过）
    pub default_name: String,
    /// keyboard | gamepad | standard
    pub mode: String,
    pub builtin: bool,
    pub active: bool,
    pub renamed: bool,
    /// 含 Live2D 素材（.model3.json）：需要 Live2D 运行时才能渲染
    pub live2d: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub id: String,
    pub name: String,
    pub mode: String,
    /// 含 Live2D 素材（静态图只是兜底）
    pub live2d: bool,
    /// 内容指纹命中：本来就已经导入过，未重复导入
    pub deduped: bool,
    /// 因同名不同内容而自动加的后缀（例如 -2）
    pub suffixed: bool,
}

pub fn models_dir(_conn: &Connection) -> PathBuf {
    storage::resolve_db_path()
        .map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("models")
}

/// 识别模型目录模式（keyboard 双爪 / gamepad 手柄 / standard 单手）
pub fn detect_mode(dir: &Path) -> Option<String> {
    let img = dir.join("img");
    if img.join("keyboard").join("lefthand").is_dir()
        && img.join("keyboard").join("righthand").is_dir()
    {
        return Some("keyboard".into());
    }
    if img.join("gamepad").join("lefthand").is_dir()
        && img.join("gamepad").join("righthand").is_dir()
    {
        return Some("gamepad".into());
    }
    if img.join("standard").join("hand").is_dir() {
        return Some("standard".into());
    }
    None
}

/// 是否含 Live2D 素材（.model3.json）：含的话静态图只是兜底，需要 Live2D 运行时才是"正确的样子"
pub fn has_live2d(dir: &Path) -> bool {
    let img = dir.join("img");
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    if collect_files(dir, &img, &mut files).is_err() {
        return false;
    }
    files.iter().any(|(rel, _)| rel.ends_with(".model3.json"))
}

fn display_name(id: &str, labels: &BTreeMap<String, String>) -> String {
    labels
        .get(id)
        .cloned()
        .unwrap_or_else(|| if id == BUILTIN_ID { BUILTIN_NAME.into() } else { id.into() })
}

fn model_list_from(models: &Path, active: &str, labels: &BTreeMap<String, String>) -> Vec<ModelInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(models) else {
        return out;
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| detect_mode(&e.path()).is_some())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    for name in names {
        let dir = models.join(&name);
        let mode = detect_mode(&dir).unwrap_or_default();
        let renamed = labels.contains_key(&name);
        let live2d = has_live2d(&dir);
        out.push(ModelInfo {
            name: display_name(&name, labels),
            default_name: name.clone(),
            id: name.clone(),
            mode,
            builtin: false,
            active: name == active,
            renamed,
            live2d,
        });
    }
    out
}

/// 模型列表（内置永远在最前）
pub fn list_models(conn: &Connection) -> Result<Vec<ModelInfo>, String> {
    let s = load(conn);
    let models = models_dir(conn);
    let _ = std::fs::create_dir_all(&models);
    let labels = model_labels(conn);
    let mut out = vec![ModelInfo {
        id: BUILTIN_ID.into(),
        name: display_name(BUILTIN_ID, &labels),
        default_name: BUILTIN_NAME.into(),
        mode: "keyboard".into(),
        builtin: true,
        active: s.active_model == BUILTIN_ID,
        renamed: labels.contains_key(BUILTIN_ID),
        live2d: false,
    }];
    out.extend(model_list_from(&models, &s.active_model, &labels));
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

/// 在解压结果里找模型根：自身或向下最多 3 层（ZIP 常带一层顶层文件夹）
fn find_root_down(staging: &Path, depth: usize) -> Option<PathBuf> {
    if has_skin_json(staging) && staging.join("img").is_dir() && detect_mode(staging).is_some() {
        return Some(staging.to_path_buf());
    }
    if depth >= 3 {
        return None;
    }
    let entries = std::fs::read_dir(staging).ok()?;
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| !p.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true))
        .collect();
    dirs.sort();
    for d in dirs {
        if let Some(found) = find_root_down(&d, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// 内容指纹（FNV-1a 64，用途是「同一模型重复导入」识别，不是安全哈希）
fn fingerprint_dir(root: &Path) -> Result<u64, String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mix = |b: u8, h: &mut u64| {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for (rel, abs) in files {
        for b in rel.as_bytes() {
            mix(*b, &mut h);
        }
        mix(b'|', &mut h);
        let data = std::fs::read(&abs).map_err(|e| format!("读取模型文件失败: {e}"))?;
        for chunk in data.chunks(64) {
            for b in chunk {
                mix(*b, &mut h);
            }
        }
    }
    Ok(h)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("读取模型目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取模型目录失败: {e}"))?;
        let path = entry.path();
        let ty = entry.file_type().map_err(|e| format!("读取失败: {e}"))?;
        if ty.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            out.push((rel, path));
        }
    }
    Ok(())
}

/// 模型 id 清洗（对齐新版 BongoCat：去掉 Windows 非法字符与保留名风险）
pub fn sanitize_id(raw: &str) -> String {
    let mut s: String = raw
        .trim()
        .trim_start_matches('.')
        .chars()
        .map(|c| {
            if c.is_control() || "\\/:*?\"<>|".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    s = s.trim().to_string();
    if s.is_empty() {
        s = format!("model-{}", now_millis());
    }
    let upper = s.to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5",
        "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
        "LPT7", "LPT8", "LPT9"];
    if reserved.contains(&upper.as_str()) {
        s = format!("_{s}");
    }
    s.chars().take(60).collect()
}

/// 同名冲突时依次尝试 base、base-2、base-3…
fn resolve_install_id(models: &Path, base: &str) -> (String, bool) {
    for suffix in 1..10_000usize {
        let id = if suffix == 1 {
            base.to_string()
        } else {
            format!("{base}-{suffix}")
        };
        if !models.join(&id).exists() {
            return (id, suffix > 1);
        }
    }
    (format!("{base}-{}", now_millis()), true)
}

fn safe_zip_entry(name: &str) -> bool {
    let p = Path::new(name);
    !p.is_absolute()
        && !name.contains("..")
        && !name.contains(':')
        && p.components().count() <= MAX_ENTRY_DEPTH
}

/// 导入模型（文件夹或 ZIP）
pub fn import_model_in(models: &Path, src_path: &str) -> Result<ImportOutcome, String> {
    let src = PathBuf::from(src_path);
    let _ = std::fs::create_dir_all(models);

    let folder_id = if src.is_dir() {
        src.file_name().map(|f| f.to_string_lossy().to_string())
    } else {
        None
    };

    // 事务目录放在模型库内，保证改名是原子操作（同一卷）
    let staging = models.join(format!(".import-{}-{}.tmp", std::process::id(), now_millis()));
    cleanup_stale(models, &staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;

    let result = (|| -> Result<ImportOutcome, String> {
        if src.is_file() {
            let ext = src
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext != "zip" {
                return Err("只支持 ZIP 压缩包，请先解压后导入模型文件夹，或重新打包为 ZIP".into());
            }
            extract_zip(&src, &staging)?;
        } else if src.is_dir() {
            copy_dir_recursive(&src, &staging)?;
        } else {
            return Err("所选来源已不存在或无法访问".into());
        }

        let Some(root) = find_root_down(&staging, 0) else {
            return Err("未找到有效的 Mver 模型包（需要 img/ 目录 + config.json 或 bongocat.skin.json）".into());
        };
        let Some(mode) = detect_mode(&root) else {
            return Err("模型缺少可用的分层素材（keyboard 双爪 / gamepad 手柄 / standard 单手）".into());
        };

        // 指纹去重：已经装过同一个模型就不再装一遍
        let fp = fingerprint_dir(&root)?;
        let live2d = has_live2d(&root);
        for existing in existing_fingerprints(models) {
            if existing.1 == fp {
                return Ok(ImportOutcome {
                    name: existing.0.clone(),
                    id: existing.0,
                    mode,
                    live2d,
                    deduped: true,
                    suffixed: false,
                });
            }
        }

        let base = sanitize_id(&folder_id.unwrap_or_else(|| {
            root.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("model-{}", now_millis()))
        }));
        let (id, suffixed) = resolve_install_id(models, &base);
        let target = models.join(&id);
        // root 可能嵌在 staging 里：先移出来，再清理 staging
        if root != staging {
            std::fs::rename(&root, &target).map_err(|e| format!("保存模型失败: {e}"))?;
        } else {
            std::fs::rename(&staging, &target).map_err(|e| format!("保存模型失败: {e}"))?;
        }
        Ok(ImportOutcome {
            name: id.clone(),
            id,
            mode,
            live2d,
            deduped: false,
            suffixed,
        })
    })();

    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// 清理上次异常退出留下的临时目录
fn cleanup_stale(models: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(models) else {
        return;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if p == keep {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with(".import-") && name.ends_with(".tmp") {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

fn existing_fingerprints(models: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(models) else {
        return out;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if !p.is_dir() || name.starts_with('.') || detect_mode(&p).is_none() {
            continue;
        }
        if let Ok(fp) = fingerprint_dir(&p) {
            out.push((name, fp));
        }
    }
    out
}

fn extract_zip(src: &Path, staging: &Path) -> Result<(), String> {
    let file = std::fs::File::open(src).map_err(|e| format!("打开 ZIP 失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("读取 ZIP 失败: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!("ZIP 条目过多（{} 个），已拒绝解压", archive.len()));
    }
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("读取 ZIP 条目失败: {e}"))?;
        let Some(rel) = f.enclosed_name() else {
            return Err("ZIP 内含不安全的路径（绝对路径或 .. 上跳），已拒绝".into());
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !safe_zip_entry(&rel_str) {
            return Err(format!("ZIP 内含不安全的路径「{rel_str}」，已拒绝"));
        }
        total = total.saturating_add(f.size());
        if total > MAX_TOTAL_BYTES {
            return Err("ZIP 解压后体积过大，已拒绝（可能是异常压缩包）".into());
        }
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
    Ok(())
}

pub fn delete_model_in(models: &Path, conn: &Connection, id: &str) -> Result<(), String> {
    if id == BUILTIN_ID {
        return Err("内置模型不可删除".into());
    }
    let dir = models.join(id);
    if dir.join("config.json").is_file() || has_skin_json(&dir) {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("删除模型失败: {e}"))?;
    }
    // 顺带清掉它的显示名覆盖
    let mut labels = model_labels(conn);
    if labels.remove(id).is_some() {
        if let Ok(txt) = serde_json::to_string(&labels) {
            let _ = storage::set_setting(conn, LABELS_KEY, &txt);
        }
    }
    if storage::get_setting(conn, "pet.active_model").as_deref() == Some(id) {
        storage::set_setting(conn, "pet.active_model", BUILTIN_ID)?;
    }
    Ok(())
}

/// 读取某模型 config.json / bongocat.skin.json（前端解析按键矩阵）
pub fn model_config_in(models: &Path, id: &str) -> Result<serde_json::Value, String> {
    if id == BUILTIN_ID {
        return Err("内置模型配置由前端内置".into());
    }
    let dir = models.join(id);
    for name in SKIN_JSON_NAMES {
        let p = dir.join(name);
        if p.is_file() {
            let txt = std::fs::read_to_string(&p).map_err(|e| format!("读取配置失败: {e}"))?;
            return serde_json::from_str(&txt).map_err(|e| format!("配置解析失败: {e}"));
        }
    }
    Err("模型缺少配置文件".into())
}

// ---------- 素材直读（绕开 asset 协议：路径编码 / scope / dev 与打包差异） ----------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetFile {
    /// 相对模型根、以 / 分隔的路径，如 img/keyboard/face/0.png
    pub rel: String,
    /// image/png 等
    pub mime: String,
    /// base64（前端转 Blob URL）
    pub data: String,
}

fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/png",
    }
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref(),
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("gif")
    )
}

/// 一次性读出某模型当前模式的全部图片（前端缓存为 Blob URL）
pub fn model_assets_in(models: &Path, id: &str, mode: &str) -> Result<Vec<AssetFile>, String> {
    if id == BUILTIN_ID {
        return Err("内置模型素材随前端内置，无需读取".into());
    }
    let root = models.join(id);
    if !root.is_dir() {
        return Err("模型不存在".into());
    }
    let mode_dir = match mode {
        "standard" => "standard",
        "gamepad" => "gamepad",
        _ => "keyboard",
    };
    let base = root.join("img").join(mode_dir);
    if !base.is_dir() {
        // 兜底：模型只有别的模式目录时，读整个 img
        let img = root.join("img");
        if !img.is_dir() {
            return Err("模型缺少 img 素材目录".into());
        }
        return read_images(&root, &img);
    }
    read_images(&root, &base)
}

fn read_images(root: &Path, dir: &Path) -> Result<Vec<AssetFile>, String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(root, dir, &mut files)?;
    files.retain(|(_, p)| is_image(p));
    files.sort();
    if files.len() > MAX_ASSET_FILES {
        return Err(format!("素材图片过多（{} 张），已拒绝读取", files.len()));
    }
    let mut out = Vec::new();
    let mut total: u64 = 0;
    for (rel, abs) in files {
        let data = std::fs::read(&abs).map_err(|e| format!("读取素材失败: {e}"))?;
        total = total.saturating_add(data.len() as u64);
        if total > MAX_ASSET_BYTES {
            return Err("素材体积过大，已拒绝读取".into());
        }
        out.push(AssetFile {
            rel,
            mime: mime_of(&abs).into(),
            data: base64_encode(&data),
        });
    }
    Ok(out)
}

pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
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

        let outcome = import_model_in(&models, src.to_str().unwrap()).unwrap();
        assert_eq!(outcome.id, "model-a");
        assert_eq!(outcome.mode, "keyboard");
        assert!(!outcome.deduped);

        let list = list_models(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|m| m.builtin));
        assert!(list.iter().any(|m| m.id == "model-a" && m.mode == "keyboard"));
        // 临时目录已清理
        assert!(!models.join(".import-x.tmp").exists());

        // 同名同内容再导入 → 识别为「已导入过」，不重复安装
        let again = import_model_in(&models, src.to_str().unwrap()).unwrap();
        assert!(again.deduped);
        assert_eq!(again.id, "model-a");
        assert_eq!(list_models(&conn).unwrap().len(), 2);

        // 启用导入的模型 → 设置与目录联动
        storage::set_setting(&conn, "pet.active_model", "model-a").unwrap();
        let s = load(&conn);
        assert_eq!(s.active_model, "model-a");
        assert!(s.active_model_dir.is_some());
        assert_eq!(s.mode, "keyboard");

        // 显示名覆盖：只改名字，目录名不变
        set_model_label(&conn, &models, "model-a", "我的猫").unwrap();
        let labeled = list_models(&conn).unwrap();
        let m = labeled.iter().find(|m| m.id == "model-a").unwrap();
        assert_eq!(m.name, "我的猫");
        assert!(m.renamed);
        assert!(models.join("model-a").is_dir());
        // 传回默认名 = 清除覆盖
        set_model_label(&conn, &models, "model-a", "model-a").unwrap();
        assert!(!list_models(&conn).unwrap().iter().find(|m| m.id == "model-a").unwrap().renamed);

        // 删除 → 回退内置
        delete_model_in(&models, &conn, "model-a").unwrap();
        let s2 = load(&conn);
        assert_eq!(s2.active_model, BUILTIN_ID);

        let _ = std::fs::remove_dir_all(&models);
        let _ = std::fs::remove_dir_all(&src_base);
    }

    #[test]
    fn import_same_name_different_content_gets_suffix() {
        let _guard = lock();
        let conn = storage::open(&temp_db("s5")).unwrap();
        let models = models_dir(&conn);
        let _ = std::fs::remove_dir_all(&models);
        std::fs::create_dir_all(&models).unwrap();

        let src_base = std::env::temp_dir().join(format!("tm-src2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&src_base);
        make_model(&src_base, "same");
        import_model_in(&models, src_base.join("same").to_str().unwrap()).unwrap();
        // 改内容后再导入同名 → 自动加后缀，不报错
        std::fs::write(
            src_base.join("same").join("img/keyboard/lefthand/1.png"),
            b"png2",
        )
        .unwrap();
        let again = import_model_in(&models, src_base.join("same").to_str().unwrap()).unwrap();
        assert!(!again.deduped);
        assert!(again.suffixed);
        assert_eq!(again.id, "same-2");
        assert!(models.join("same-2").is_dir());

        let _ = std::fs::remove_dir_all(&models);
        let _ = std::fs::remove_dir_all(&src_base);
    }

    #[test]
    fn import_zip_model_with_deep_root() {
        let _guard = lock();
        let conn = storage::open(&temp_db("s2")).unwrap();
        let models = models_dir(&conn);
        let _ = std::fs::remove_dir_all(&models);
        std::fs::create_dir_all(&models).unwrap();

        let stage = std::env::temp_dir().join(format!("tm-zsrc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&stage);
        make_model(&stage, "我的模型");
        let zip_path = std::env::temp_dir().join(format!("tm-zsrc-{}-m.zip", std::process::id()));
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            // 三层嵌套：模拟「压缩包 > 顶层文件夹 > 模型文件夹」
            zw.start_file(
                "顶层/我的模型/bongocat.skin.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(
                &mut zw,
                br#"{"keyboard":{"lefthand":[[65]],"righthand":[[76]]}}"#,
            )
            .unwrap();
            zw.start_file(
                "顶层/我的模型/img/keyboard/lefthand/0.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(&mut zw, b"png").unwrap();
            zw.start_file(
                "顶层/我的模型/img/keyboard/righthand/0.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            std::io::Write::write_all(&mut zw, b"png").unwrap();
            zw.finish().unwrap();
        }

        let info = import_model_in(&models, zip_path.to_str().unwrap()).unwrap();
        assert_eq!(info.id, "我的模型");
        assert!(!info.deduped);
        let list = list_models(&conn).unwrap();
        assert!(list.iter().any(|m| m.id == "我的模型"));
        // 读取素材（base64 直读路径）
        let assets = model_assets_in(&models, "我的模型", "keyboard").unwrap();
        assert_eq!(assets.len(), 2);
        assert!(assets.iter().all(|a| !a.data.is_empty()));

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
        // 非 zip 的压缩包要给出明确中文提示
        let rar = models.join("x.rar");
        std::fs::write(&rar, b"rar").unwrap();
        let err = import_model_in(&models, rar.to_str().unwrap()).unwrap_err();
        assert!(err.contains("ZIP"));
        let _ = std::fs::remove_dir_all(&models);
    }

    #[test]
    fn sanitize_and_base64() {
        assert_eq!(sanitize_id("a/b:c*d"), "a-b-c-d");
        assert_eq!(sanitize_id("  .hidden "), "hidden");
        assert_eq!(sanitize_id("con"), "_con");
        assert_eq!(base64_encode(b"abc"), "YWJj");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"a"), "YQ==");
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
