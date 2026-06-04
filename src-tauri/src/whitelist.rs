//! 跨模块共享的白名单检测
//!
//! 用途：用户在白名单应用（如 Teams、Zoom、全屏演示）时不发任何提醒
//! 状态：`IN_WHITELIST_APP` 是缓存的最新结果（由 idle 调度器每 30 秒更新一次）
//! 调用方：idle / water / medication 三种提醒都应先检查

use std::sync::atomic::{AtomicBool, Ordering};
use log::info;
use tauri::{AppHandle, Manager};

/// 当前是否在白名单应用中（true = 在白名单中，应暂停所有提醒）
pub static IN_WHITELIST_APP: AtomicBool = AtomicBool::new(false);

/// 查询当前是否在白名单中（不主动检测，只读取缓存）
pub fn is_in_whitelist() -> bool {
    IN_WHITELIST_APP.load(Ordering::SeqCst)
}

/// 主动检测 + 更新缓存，返回最新结果
/// 建议在 idle 调度器中每 30 秒调用一次以保持缓存新鲜
pub fn check(app_handle: &AppHandle) -> bool {
    let result = detect_whitelist_apps(app_handle);
    IN_WHITELIST_APP.store(result, Ordering::SeqCst);
    result
}

/// 实际检测逻辑：检查前台窗口标题是否包含白名单应用名
#[cfg(target_os = "windows")]
fn detect_whitelist_apps(app_handle: &AppHandle) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        // 从配置中读取白名单
        if let Some(state) = app_handle.try_state::<crate::AppState>() {
            if let Ok(config) = state.config.lock() {
                let whitelist = &config.whitelist_apps;
                if whitelist.is_empty() {
                    return false;
                }

                // 读取前台窗口标题
                let mut title_buf = [0u16; 260];
                let len = GetWindowTextW(hwnd, &mut title_buf);
                if len > 0 {
                    let title = String::from_utf16_lossy(&title_buf[..len as usize]);
                    let title_lower = title.to_lowercase();

                    for app in whitelist {
                        let app_lower = app.to_lowercase();
                        if title_lower.contains(&app_lower) {
                            info!("Whitelist match: foreground window title '{}' contains '{}'", title, app);
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn detect_whitelist_apps(_app_handle: &AppHandle) -> bool {
    // macOS / Linux 暂未实现
    false
}
