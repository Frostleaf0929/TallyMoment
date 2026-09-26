//! 全局键鼠"信号"钩子：只告诉桌宠"有按键/有点击"，不读取任何键值、坐标或内容。
//! 低级钩子（WH_KEYBOARD_LL/WH_MOUSE_LL）要求安装线程持续泵消息，故独立线程 + GetMessage 循环。
#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::sync::OnceLock;
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
        WM_RBUTTONDOWN, WM_SYSKEYDOWN,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    /// 事件节流：最快 ~60 次/秒，避免极限手速刷爆 IPC
    static LAST_EMIT_MS: AtomicU32 = AtomicU32::new(0);
    /// 键入/点击总计数（只记次数，不记内容）——M6 键鼠统计的种子
    static KEY_COUNT: AtomicU64 = AtomicU64::new(0);
    static CLICK_COUNT: AtomicU64 = AtomicU64::new(0);
    /// 上次落库基线
    static LAST_FLUSH_KEY: AtomicU64 = AtomicU64::new(0);
    static LAST_FLUSH_CLICK: AtomicU64 = AtomicU64::new(0);

    /// (键入次数, 点击次数) 自应用启动以来
    pub fn stats() -> (u64, u64) {
        (
            KEY_COUNT.load(Ordering::Relaxed),
            CLICK_COUNT.load(Ordering::Relaxed),
        )
    }

    /// 落库用：返回自上次调用以来的增量，并推进基线
    pub fn flush_delta() -> (i64, i64) {
        let k = KEY_COUNT.load(Ordering::Relaxed);
        let c = CLICK_COUNT.load(Ordering::Relaxed);
        let dk = k.saturating_sub(LAST_FLUSH_KEY.swap(k, Ordering::Relaxed)) as i64;
        let dc = c.saturating_sub(LAST_FLUSH_CLICK.swap(c, Ordering::Relaxed)) as i64;
        (dk, dc)
    }

    fn now_tick() -> u32 {
        unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
    }

    fn emit(kind: &str, vk: u32) {
        let now = now_tick();
        let last = LAST_EMIT_MS.load(Ordering::Relaxed);
        if now.wrapping_sub(last) < 16 {
            return;
        }
        LAST_EMIT_MS.store(now, Ordering::Relaxed);
        if let Some(app) = APP.get() {
            // 只带键码（用于桌宠逐键选帧），不带内容、不落库
            let _ = app.emit("pet-input", serde_json::json!({ "kind": kind, "vk": vk }));
        }
    }

    unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let kbd = (lparam.0 as *const KBDLLHOOKSTRUCT).read();
                KEY_COUNT.fetch_add(1, Ordering::Relaxed);
                emit("key", kbd.vkCode);
            }
        }
        // 首参数在 modern Windows 上被忽略，传 None 即可
        CallNextHookEx(None, code, wparam, lparam)
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            let btn = match msg {
                WM_LBUTTONDOWN => 1u32,
                WM_RBUTTONDOWN => 2u32,
                WM_MBUTTONDOWN => 4u32,
                _ => 0,
            };
            if btn != 0 {
                let _ = (lparam.0 as *const MSLLHOOKSTRUCT).read();
                CLICK_COUNT.fetch_add(1, Ordering::Relaxed);
                emit("click", btn);
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    pub fn spawn(app: AppHandle) {
        let _ = APP.set(app);
        std::thread::spawn(move || unsafe {
            let _ = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), None, 0);
            let _ = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
        });
    }
}

#[cfg(windows)]
pub use imp::{spawn, stats, flush_delta};

#[cfg(not(windows))]
pub fn spawn(_app: tauri::AppHandle) {}

#[cfg(not(windows))]
pub fn stats() -> (u64, u64) {
    (0, 0)
}

#[cfg(not(windows))]
pub fn flush_delta() -> (i64, i64) {
    (0, 0)
}
