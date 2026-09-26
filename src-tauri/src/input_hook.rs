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

    /// (键入次数, 点击次数) 自应用启动以来
    pub fn stats() -> (u64, u64) {
        (
            KEY_COUNT.load(Ordering::Relaxed),
            CLICK_COUNT.load(Ordering::Relaxed),
        )
    }

    fn now_tick() -> u32 {
        unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
    }

    fn emit(kind: &str) {
        let now = now_tick();
        let last = LAST_EMIT_MS.load(Ordering::Relaxed);
        if now.wrapping_sub(last) < 16 {
            return;
        }
        LAST_EMIT_MS.store(now, Ordering::Relaxed);
        if let Some(app) = APP.get() {
            let _ = app.emit("pet-input", kind);
        }
    }

    unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let _ = (lparam.0 as *const KBDLLHOOKSTRUCT).read();
                KEY_COUNT.fetch_add(1, Ordering::Relaxed);
                emit("key");
            }
        }
        // 首参数在 modern Windows 上被忽略，传 None 即可
        CallNextHookEx(None, code, wparam, lparam)
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
                let _ = (lparam.0 as *const MSLLHOOKSTRUCT).read();
                CLICK_COUNT.fetch_add(1, Ordering::Relaxed);
                emit("click");
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
pub use imp::{spawn, stats};

#[cfg(not(windows))]
pub fn spawn(_app: tauri::AppHandle) {}

#[cfg(not(windows))]
pub fn stats() -> (u64, u64) {
    (0, 0)
}
