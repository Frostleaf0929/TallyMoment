//! 读取 exe 的应用图标（Win32：SHGetFileInfoW 取 HICON + GDI 取像素）
//! 用途：设置里的「应用图标」显示模式与「根据图标取色」配色模式
use std::collections::HashMap;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject, BITMAP,
    BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

pub struct IconData {
    pub w: i32,
    pub h: i32,
    /// BGRA（每像素 4 字节，左上为原点）
    pub bgra: Vec<u8>,
    /// 主色（#rrggbb）
    pub color: String,
}

static CACHE: Mutex<Option<HashMap<String, std::sync::Arc<IconData>>>> = Mutex::new(None);

/// 取图标（带进程内缓存，key 为 exe 路径）
pub fn extract_cached(path: &str) -> Result<std::sync::Arc<IconData>, String> {
    {
        let guard = CACHE.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(map) = guard.as_ref() {
            if let Some(hit) = map.get(path) {
                return Ok(hit.clone());
            }
        }
    }
    let data = std::sync::Arc::new(extract(path)?);
    let mut guard = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(path.to_string(), data.clone());
    Ok(data)
}

fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// 读 exe 版本资源的指定字符串字段（FileDescription / ProductName / ...）。
/// 读不到 / 没有该资源返回 None。
#[cfg(windows)]
fn version_string(path: &str, key: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    let w = wide(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(w.as_ptr()), Some(0), size, buf.as_mut_ptr() as *mut _).ok()?;
        let mut ptr: *mut u16 = std::ptr::null_mut();
        let mut len = 0u32;
        // 翻译表给出 语言/代码页，再按它拼出 FileDescription 的查询路径
        if !VerQueryValueW(
            buf.as_ptr() as *const _,
            PCWSTR(wide("\\VarFileInfo\\Translation").as_ptr()),
            &mut ptr as *mut *mut u16 as *mut *mut core::ffi::c_void,
            &mut len,
        )
        .as_bool()
            || ptr.is_null()
            || len < 2
        {
            return None;
        }
        let (lang, codepage) = (*ptr, *ptr.add(1));
        let q = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\{key}");
        if !VerQueryValueW(
            buf.as_ptr() as *const _,
            PCWSTR(wide(&q).as_ptr()),
            &mut ptr as *mut *mut u16 as *mut *mut core::ffi::c_void,
            &mut len,
        )
        .as_bool()
            || ptr.is_null()
            || len == 0
        {
            return None;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len as usize));
        let t = s.trim_end_matches('\0').trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    }
}

/// 任务栏悬停名 / Tai 式"花笺"友好名的来源
#[cfg(windows)]
pub fn file_description(path: &str) -> Option<String> {
    version_string(path, "FileDescription")
}

/// exe 产品名（构建期由打包配置写入，**不含版本号**）
#[cfg(windows)]
pub fn product_name(path: &str) -> Option<String> {
    version_string(path, "ProductName")
}

/// 是否为"拾刻自身"：只看 exe 版本信息里的产品名/描述，**不看文件名**。
/// 文件名会因版本号（便携版发布时改名以区分下载）或用户重命名而变化，产品名不会——
/// 这是"记录自身被拆成多条、每个版本一条"问题的根治依据。
#[cfg(windows)]
pub fn is_self_exe(path: &str) -> bool {
    let hit = |s: &str| {
        let l = s.to_lowercase();
        l.contains("tallymoment") || l.contains("拾刻")
    };
    product_name(path).as_deref().map(hit).unwrap_or(false)
        || file_description(path).as_deref().map(hit).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn product_name(_path: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn is_self_exe(_path: &str) -> bool {
    false
}

#[cfg(not(windows))]
pub fn file_description(_path: &str) -> Option<String> {
    None
}

#[cfg(windows)]
pub fn extract(path: &str) -> Result<IconData, String> {
    let w = wide(path);
    unsafe {
        // ExtractIconExW 直接从 exe/dll 抽图标，不依赖 shell 上下文（后台进程里 SHGetFileInfoW 会返回 0）
        let mut hicon = windows::Win32::UI::WindowsAndMessaging::HICON::default();
        let n = ExtractIconExW(
            PCWSTR(w.as_ptr()),
            0,
            Some(&mut hicon),
            None,
            1,
        );
        if n == 0 || hicon.is_invalid() {
            return Err("该程序没有可读取的图标".into());
        }

        let mut ii = ICONINFO::default();
        if GetIconInfo(hicon, &mut ii).is_err() {
            let _ = DestroyIcon(hicon);
            return Err("解析图标句柄失败".into());
        }

        let hdc = CreateCompatibleDC(None);
        let old = SelectObject(hdc, HGDIOBJ(ii.hbmColor.0));

        let mut bm = BITMAP::default();
        GetObjectW(
            HGDIOBJ(ii.hbmColor.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut std::ffi::c_void),
        );
        let (bw, bh) = (bm.bmWidth, bm.bmHeight);
        if bw <= 0 || bh <= 0 {
            SelectObject(hdc, old);
            let _ = DeleteDC(hdc);
            let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
            let _ = DestroyIcon(hicon);
            return Err("图标尺寸异常".into());
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bw,
                // 负高度 = 自上而下
                biHeight: -bh,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (bw * bh * 4) as usize];
        let lines = GetDIBits(
            hdc,
            ii.hbmColor,
            0,
            bh as u32,
            Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc, old);
        let _ = DeleteDC(hdc);
        let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
        let _ = DestroyIcon(hicon);

        if lines == 0 || buf.is_empty() {
            return Err("读取图标像素失败".into());
        }
        if buf.chunks_exact(4).all(|p| p[3] == 0) {
            for p in buf.chunks_exact_mut(4) {
                p[3] = 255;
            }
        }
        let color = dominant(&buf);
        Ok(IconData {
            w: bw,
            h: bh,
            bgra: buf,
            color,
        })
    }
}

#[cfg(not(windows))]
pub fn extract(_path: &str) -> Result<IconData, String> {
    Err("仅支持 Windows".into())
}

/// 主色：按 3 bit/通道分桶计数，跳过近黑/近白/透明，取最多的桶做平均
fn dominant(bgra: &[u8]) -> String {
    let mut buckets: HashMap<(u8, u8, u8), (u32, u64, u64, u64)> = HashMap::new();
    for p in bgra.chunks_exact(4) {
        let (b, g, r, a) = (p[0], p[1], p[2], p[3]);
        if a < 128 {
            continue;
        }
        let (max, min) = (
            r.max(g).max(b) as i32,
            r.min(g).min(b) as i32,
        );
        if max > 240 && min > 220 {
            continue; // 近白（多为图标底色）
        }
        if max < 28 {
            continue; // 近黑
        }
        let key = (r >> 5, g >> 5, b >> 5);
        let e = buckets.entry(key).or_insert((0, 0, 0, 0));
        e.0 += 1;
        e.1 += r as u64;
        e.2 += g as u64;
        e.3 += b as u64;
    }
    let Some((_, (n, sr, sg, sb))) = buckets.into_iter().max_by_key(|(_, v)| v.0) else {
        return "#7b84ec".into();
    };
    if n == 0 {
        return "#7b84ec".into();
    }
    let (r, g, b) = (
        (sr / n as u64) as f64,
        (sg / n as u64) as f64,
        (sb / n as u64) as f64,
    );
    let (r, g, b) = limit_saturation(r, g, b, 0.45);
    format!(
        "#{:02x}{:02x}{:02x}",
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8
    )
}

/// 限制饱和度：太艳丽（如纯红纯绿）的图标主色会被压到 max_s 以内，
/// 以免色点在浅色界面上刺眼（用户反馈"取色不要过于艳丽"）
fn limit_saturation(r: f64, g: f64, b: f64, max_s: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d <= 0.0 {
        return (r, g, b);
    }
    let s = if l > 127.5 {
        d / (510.0 - max - min)
    } else {
        d / (max + min)
    };
    if s <= max_s {
        return (r, g, b);
    }
    let s_new = max_s;
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    // HSL -> RGB，仅换饱和度
    let c = (1.0 - (2.0 * l / 255.0 - 1.0).abs()) * s_new * 255.0;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = if h < 1.0 / 6.0 {
        (c, x, 0.0)
    } else if h < 2.0 / 6.0 {
        (x, c, 0.0)
    } else if h < 3.0 / 6.0 {
        (0.0, c, x)
    } else if h < 4.0 / 6.0 {
        (0.0, x, c)
    } else if h < 5.0 / 6.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r1 + m, g1 + m, b1 + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_notepad_icon() {
        let p = "C:/Windows/System32/notepad.exe";
        if !std::path::Path::new(p).is_file() {
            return; // 环境没有该文件就跳过
        }
        let d = extract(p).expect("应能读到记事本图标");
        assert!(d.w >= 16 && d.h >= 16, "尺寸异常: {}x{}", d.w, d.h);
        assert_eq!(d.bgra.len(), (d.w * d.h * 4) as usize);
        assert!(d.bgra.iter().any(|b| *b > 0), "像素全空");
        assert!(d.color.starts_with('#') && d.color.len() == 7, "主色格式: {}", d.color);
        // 缓存命中也应返回同样尺寸
        let again = extract_cached(p).expect("缓存读取失败");
        assert_eq!(again.w, d.w);

        // 主色饱和度必须被压到阈值内（纯红 r=255,g=0,b=0 的 S 会从 1.0 降到 0.45）
        let (r, g, b) = limit_saturation(255.0, 0.0, 0.0, 0.45);
        assert!(g > 0.0 && b > 0.0, "限饱和后应不再是纯红: {r},{g},{b}");
        // 低饱和颜色不受影响
        let (r2, g2, b2) = limit_saturation(120.0, 120.0, 120.0, 0.45);
        assert_eq!((r2, g2, b2), (120.0, 120.0, 120.0));
    }
}
