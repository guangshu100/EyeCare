// 用药提醒调度器
// 负责：每 30 秒扫描一次，更新 due 状态（升级提醒级别），并触发 reminder::send_reminder

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Timelike;
use log::info;
use tauri::{AppHandle, Manager};

use crate::config::{DoseStatus, MedicationLog};
use crate::AppState;

pub static MEDICATION_TIMER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 启动后台调度线程
pub fn start_medication_timer(app_handle: AppHandle) {
    if MEDICATION_TIMER_RUNNING.load(Ordering::SeqCst) {
        return;
    }
    MEDICATION_TIMER_RUNNING.store(true, Ordering::SeqCst);
    info!("Medication reminder timer started");

    std::thread::spawn(move || {
        loop {
            // 获取配置快照
            let (enabled, escalation_minutes) = {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Ok(config) = state.config.lock() {
                        (config.medication.enabled, config.medication.escalation_minutes.max(1))
                    } else {
                        (false, 30)
                    }
                } else {
                    (false, 30)
                }
            };

            if !enabled {
                std::thread::sleep(Duration::from_secs(10));
                continue;
            }

            // 触发扫描
            let due = update_due_state_with_app(&app_handle, escalation_minutes);

            // 白名单检测：用户在白名单应用中跳过所有用药提醒
            // （仍更新 severity 用于状态显示，但不发通知）
            let in_whitelist = crate::whitelist::is_in_whitelist();
            if in_whitelist {
                log::debug!("Skipping medication reminder: user in whitelist app");
            }

            // 发送 due 的提醒
            for log in due {
                if in_whitelist {
                    continue; // 白名单中：静默
                }
                if let Err(e) = crate::medication::reminder::send_reminder(&app_handle, &log) {
                    log::error!("Failed to send medication reminder: {}", e);
                }
            }

            // 库存预警
            if let Err(e) = crate::medication::reminder::check_stock_alerts(&app_handle) {
                log::error!("Stock alert check failed: {}", e);
            }

            std::thread::sleep(Duration::from_secs(30));
        }
    });
}

/// 公共 API：更新今日 logs 中所有 Pending 条目的 due 级别
/// 返回需要触发通知的 logs（仅在级别上升时返回）
pub fn update_due_state(app: &AppHandle) -> Vec<MedicationLog> {
    update_due_state_with_app(app, 30)
}

fn update_due_state_with_app(app: &AppHandle, _escalation_minutes: u32) -> Vec<MedicationLog> {
    let now = chrono::Local::now();
    let now_secs = now.hour() as u32 * 3600 + now.minute() as u32 * 60 + now.second() as u32;
    let now_str = format!(
        "{:02}:{:02}:{:02}",
        now.hour(),
        now.minute(),
        now.second()
    );

    let mut to_notify: Vec<MedicationLog> = Vec::new();

    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return to_notify,
    };
    let mut config = match state.config.lock() {
        Ok(c) => c,
        Err(_) => return to_notify,
    };

    for log in config.medication.today_logs.iter_mut() {
        if log.status != DoseStatus::Pending {
            continue;
        }
        let parts: Vec<&str> = log.scheduled_time.split(':').collect();
        if parts.len() != 2 {
            continue;
        }
        let h: u32 = parts[0].parse().unwrap_or(0);
        let m: u32 = parts[1].parse().unwrap_or(0);
        let target = h * 3600 + m * 60;
        if target > now_secs {
            // 还没到时间
            continue;
        }
        let elapsed = now_secs - target;

        // 升级 severity：0=未触发 1=准时 2=5min 3=15min 4=30min
        let new_sev = if elapsed < 5 * 60 {
            1
        } else if elapsed < 15 * 60 {
            2
        } else if elapsed < 30 * 60 {
            3
        } else {
            4
        };

        // 距离上次发出通知已经过 >= 5 分钟（避免疯狂刷新）
        let should_notify = if log.severity == 0 {
            true
        } else if new_sev > log.severity {
            // 升级时通知
            true
        } else {
            // 同级别：每 5 分钟再通知一次
            log.actual_time.is_none()
                && false
        };

        if log.severity == 0 || new_sev > log.severity {
            log.severity = new_sev;
        }
        log.actual_time = Some(now_str.clone());

        if should_notify {
            to_notify.push(log.clone());
        }
    }

    // 持久化（仅 severity 改变）
    let _ = config.save_silent();
    to_notify
}

/// 重置今日状态（每日 0 点由 lib.rs 调度）
pub fn reset_daily_if_needed(app: &AppHandle) {
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return,
    };
    let mut config = match state.config.lock() {
        Ok(c) => c,
        Err(_) => return,
    };
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    // 通过 last_date 检测 - 在 AppConfig 中已有此字段
    if config.last_date != today {
        config.medication.reset_daily();
    }
}
