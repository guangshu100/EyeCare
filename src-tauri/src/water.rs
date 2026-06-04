use chrono::Timelike;
use log::{info, warn};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::WaterConfig;

static WATER_TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static LAST_REMINDER_TIME: Mutex<Option<Instant>> = Mutex::new(None);
static NEXT_REMINDER_SECONDS: AtomicU64 = AtomicU64::new(0);
/// 当前生效的间隔（秒），用于"修改间隔时保留已等待时间"逻辑
static LAST_INTERVAL: AtomicU64 = AtomicU64::new(1800);

/// 排班模式追踪状态
#[derive(Default)]
struct ScheduleState {
    /// 上次所处的排班时段索引（None=未在排班内）
    last_slot_index: Option<usize>,
    /// 进入当前排班时段的时刻（用于首条提醒延迟）
    slot_entered_at: Option<Instant>,
    /// 该时段是否已发出首次提醒
    first_reminded: bool,
    /// 上次模式（interval / schedule），用于检测切换
    last_mode: Option<&'static str>,
    /// 上次"空档期兜底"提醒时刻
    last_gap_remind: Option<Instant>,
}

static SCHEDULE_STATE: Mutex<ScheduleState> = Mutex::new(ScheduleState {
    last_slot_index: None,
    slot_entered_at: None,
    first_reminded: false,
    last_mode: None,
    last_gap_remind: None,
});

// ==================== 上班模式排班定义 ====================

/// 上班时段喝水排班（9:00-17:30 六个时段）
pub struct WaterScheduleSlot {
    pub start_minutes: u32,  // 距午夜分钟数
    pub end_minutes: u32,
    pub amount_ml: u32,
    pub label: &'static str,
    pub message: &'static str,
    pub icon: &'static str,
    pub time_display: &'static str,
}

pub const WATER_SCHEDULE: [WaterScheduleSlot; 6] = [
    WaterScheduleSlot {
        start_minutes: 540,   // 9:00
        end_minutes: 570,     // 9:30
        amount_ml: 250,
        label: "到岗补水",
        message: "早晨身体缺水，先来杯温水唤醒新陈代谢吧！",
        icon: "🌅",
        time_display: "9:00-9:30",
    },
    WaterScheduleSlot {
        start_minutes: 630,   // 10:30
        end_minutes: 660,     // 11:00
        amount_ml: 200,
        label: "工作间隙",
        message: "工作近两小时了，起来活动活动，顺手喝杯水～",
        icon: "💻",
        time_display: "10:30-11:00",
    },
    WaterScheduleSlot {
        start_minutes: 690,   // 11:30
        end_minutes: 720,     // 12:00
        amount_ml: 150,
        label: "午餐前",
        message: "午餐前喝点水，避免用餐时过量饮水哦",
        icon: "🍽️",
        time_display: "11:30-12:00",
    },
    WaterScheduleSlot {
        start_minutes: 810,   // 13:30
        end_minutes: 840,     // 14:00
        amount_ml: 200,
        label: "午休后",
        message: "午睡醒来喝杯温水，快速恢复精神！",
        icon: "😴",
        time_display: "13:30-14:00",
    },
    WaterScheduleSlot {
        start_minutes: 900,   // 15:00
        end_minutes: 930,     // 15:30
        amount_ml: 200,
        label: "下午茶时间",
        message: "下午犯困了？喝杯水搭配拉伸，提提神！",
        icon: "🍵",
        time_display: "15:00-15:30",
    },
    WaterScheduleSlot {
        start_minutes: 1020,  // 17:00
        end_minutes: 1050,    // 17:30
        amount_ml: 150,
        label: "下班前",
        message: "下班前补杯水，路上注意补充水分哦",
        icon: "🚶",
        time_display: "17:00-17:30",
    },
];

/// 排班时段信息（供前端展示）
#[derive(Clone, serde::Serialize)]
pub struct ScheduleSlotInfo {
    pub index: u32,
    pub time_range: String,
    pub amount_ml: u32,
    pub label: String,
    pub icon: String,
    pub completed: bool,
    pub is_current: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct WaterStatus {
    pub enabled: bool,
    pub total_ml: u32,
    pub drink_count: u32,
    pub daily_goal_ml: u32,
    pub progress_percent: u32,
    pub next_reminder_seconds: u64,
    pub is_in_active_hours: bool,
    /// 上班排班模式
    pub schedule_enabled: bool,
    /// 当前时段索引（0-5）
    pub current_slot_index: Option<u32>,
    /// 距下一时段分钟数
    pub next_slot_minutes: Option<u64>,
    /// 下一时段名称
    pub next_slot_label: Option<String>,
    /// 当前时段建议喝水量
    pub current_slot_amount: Option<u32>,
    /// 当前时段提醒文案
    pub current_slot_message: Option<String>,
    /// 已完成时段数
    pub schedule_completed_count: u32,
    /// 排班总时段数
    pub schedule_total_slots: u32,
    /// 排班详情列表
    pub schedule_slots: Vec<ScheduleSlotInfo>,
}

/// 获取当前分钟数（距午夜）
fn current_minutes_of_day() -> u32 {
    let now = chrono::Local::now();
    now.hour() as u32 * 60 + now.minute() as u32
}

/// 查找当前所在排班时段
fn find_current_slot(minutes: u32) -> Option<usize> {
    WATER_SCHEDULE.iter().position(|slot| {
        minutes >= slot.start_minutes && minutes <= slot.end_minutes
    })
}

/// 查找下一个排班时段
fn find_next_slot(minutes: u32) -> Option<usize> {
    WATER_SCHEDULE.iter().position(|slot| {
        minutes < slot.start_minutes
    })
}

pub fn get_water_status(config: &WaterConfig) -> WaterStatus {
    let next = NEXT_REMINDER_SECONDS.load(Ordering::SeqCst);
    let now = chrono::Local::now();
    let current_hour = now.hour() as u32;
    let is_in_active_hours = current_hour >= config.start_hour && current_hour < config.end_hour;

    if !config.schedule_enabled {
        // 间隔模式（原有逻辑）
        return WaterStatus {
            enabled: config.enabled,
            total_ml: config.stats.total_ml,
            drink_count: config.stats.drink_count,
            daily_goal_ml: config.daily_goal_ml,
            progress_percent: config.progress_percent(),
            next_reminder_seconds: if is_in_active_hours { next } else { 0 },
            is_in_active_hours,
            schedule_enabled: false,
            current_slot_index: None,
            next_slot_minutes: None,
            next_slot_label: None,
            current_slot_amount: None,
            current_slot_message: None,
            schedule_completed_count: 0,
            schedule_total_slots: 6,
            schedule_slots: Vec::new(),
        };
    }

    // 排班模式
    let minutes = current_minutes_of_day();
    let current_idx = find_current_slot(minutes);
    let next_idx = find_next_slot(minutes);
    let completed_count = config.stats.schedule_completed.iter().filter(|&&c| c).count() as u32;

    let (current_slot_index, next_slot_minutes, next_slot_label, current_slot_amount, current_slot_message) =
        if let Some(idx) = current_idx {
            let slot = &WATER_SCHEDULE[idx];
            let completed = config.stats.schedule_completed.get(idx).copied().unwrap_or(false);
            if completed {
                // 当前时段已完成，找下一个
                let remaining = if let Some(ni) = next_idx {
                    (WATER_SCHEDULE[ni].start_minutes - minutes) as u64 * 60
                } else {
                    0
                };
                let label = next_idx.map(|ni| WATER_SCHEDULE[ni].label.to_string());
                (Some(idx as u32), Some(remaining / 60), label, None, None)
            } else {
                // 当前时段未完成
                (Some(idx as u32), None, None, Some(slot.amount_ml), Some(slot.message.to_string()))
            }
        } else if let Some(ni) = next_idx {
            let slot = &WATER_SCHEDULE[ni];
            let remaining = (slot.start_minutes - minutes) as u64;
            (None, Some(remaining), Some(slot.label.to_string()), None, None)
        } else {
            (None, None, None, None, None)
        };

    // 构建排班列表
    let schedule_slots: Vec<ScheduleSlotInfo> = WATER_SCHEDULE.iter().enumerate().map(|(i, slot)| {
        let completed = config.stats.schedule_completed.get(i).copied().unwrap_or(false);
        ScheduleSlotInfo {
            index: i as u32,
            time_range: slot.time_display.to_string(),
            amount_ml: slot.amount_ml,
            label: slot.label.to_string(),
            icon: slot.icon.to_string(),
            completed,
            is_current: current_idx == Some(i),
        }
    }).collect();

    WaterStatus {
        enabled: config.enabled,
        total_ml: config.stats.total_ml,
        drink_count: config.stats.drink_count,
        daily_goal_ml: config.daily_goal_ml,
        progress_percent: config.progress_percent(),
        next_reminder_seconds: if is_in_active_hours { next } else { 0 },
        is_in_active_hours,
        schedule_enabled: true,
        current_slot_index,
        next_slot_minutes,
        next_slot_label,
        current_slot_amount,
        current_slot_message,
        schedule_completed_count: completed_count,
        schedule_total_slots: 6,
        schedule_slots,
    }
}

pub fn start_water_timer(app_handle: AppHandle) {
    if WATER_TIMER_RUNNING.load(Ordering::SeqCst) {
        return;
    }

    WATER_TIMER_RUNNING.store(true, Ordering::SeqCst);
    info!("Water reminder timer started");

    std::thread::spawn(move || {
        loop {
            let (enabled, schedule_enabled, interval_secs, start_hour, end_hour, stats_snapshot) = {
                if let Some(state) = app_handle.try_state::<crate::AppState>() {
                    if let Ok(config) = state.config.lock() {
                        (
                            config.water.enabled,
                            config.water.schedule_enabled,
                            config.water.interval_minutes as u64 * 60,
                            config.water.start_hour,
                            config.water.end_hour,
                            config.water.stats.schedule_completed.clone(),
                        )
                    } else {
                        (false, false, 1800, 8, 22, Vec::new())
                    }
                } else {
                    (false, false, 1800, 8, 22, Vec::new())
                }
            };

            if !enabled {
                NEXT_REMINDER_SECONDS.store(0, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(10));
                continue;
            }

            let now = chrono::Local::now();
            let current_hour = now.hour() as u32;
            let is_in_active_hours = current_hour >= start_hour && current_hour < end_hour;

            if !is_in_active_hours {
                NEXT_REMINDER_SECONDS.store(0, Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(60));
                continue;
            }

            if schedule_enabled {
                handle_schedule_mode(&app_handle, &now, &stats_snapshot);
            } else {
                handle_interval_mode(&app_handle, interval_secs);
            }

            // 5 秒轮询：减少最大 5 秒的提醒漂移
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

/// 间隔模式：按时段个性化文案
fn handle_interval_mode(app_handle: &AppHandle, interval_secs: u64) {
    // 白名单检测：用户在白名单应用中跳过所有喝水提醒
    if crate::whitelist::is_in_whitelist() {
        return;
    }

    // 检测模式切换：从排班 → 间隔模式时重置
    let mut state = SCHEDULE_STATE.lock().unwrap();
    if state.last_mode != Some("interval") {
        state.last_mode = Some("interval");
        state.last_slot_index = None;
        state.slot_entered_at = None;
        state.first_reminded = false;
        state.last_gap_remind = None;
        if let Ok(mut lt) = LAST_REMINDER_TIME.lock() {
            *lt = Some(Instant::now());
        }
        info!("Water interval mode: state reset (mode change detected)");
    }
    drop(state);

    let should_remind = {
        if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
            if let Some(last) = *last_time {
                last.elapsed().as_secs() >= interval_secs
            } else {
                true
            }
        } else {
            false
        }
    };

    if should_remind {
        let (title, body) = build_time_aware_interval_message();
        send_water_notification(app_handle, &title, &body);
        if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
            *last_time = Some(Instant::now());
        }
        LAST_INTERVAL.store(interval_secs, Ordering::SeqCst);
        NEXT_REMINDER_SECONDS.store(interval_secs, Ordering::SeqCst);
    } else {
        let elapsed = if let Ok(last_time) = LAST_REMINDER_TIME.lock() {
            last_time.map(|t| t.elapsed().as_secs()).unwrap_or(0)
        } else {
            0
        };
        let remaining = interval_secs.saturating_sub(elapsed);
        // 同步当前生效间隔（用户中途改配置时也能被追踪到）
        LAST_INTERVAL.store(interval_secs, Ordering::SeqCst);
        NEXT_REMINDER_SECONDS.store(remaining, Ordering::SeqCst);
    }
}

/// 间隔模式按时段生成文案
fn build_time_aware_interval_message() -> (String, String) {
    let hour = chrono::Local::now().hour();
    let (title, msgs) = match hour {
        5..=8 => (
            "🌅 早安，喝杯水唤醒身体",
            &[
                "清晨第一杯水，唤醒沉睡的身体～",
                "空腹喝杯温水，促进新陈代谢",
                "新的一天从补水开始！",
            ][..],
        ),
        9..=11 => (
            "☕ 上午工作别忘了补水",
            &[
                "工作间隙来杯水，效率更高哦",
                "大脑 80% 是水分，记得补充",
                "一小时没喝水了吧？快来一杯",
            ][..],
        ),
        12..=13 => (
            "🍱 午餐前后要小口喝水",
            &[
                "饭前半小时喝点水，饭后更舒适",
                "细嚼慢咽配小口喝水，消化更好",
                "别等渴了才喝哦",
            ][..],
        ),
        14..=17 => (
            "☕ 下午补水提神",
            &[
                "下午是补水黄金时段，状态拉满",
                "小口慢饮比一次性灌水更健康",
                "起来接杯水吧，顺便活动一下",
            ][..],
        ),
        18..=20 => (
            "🌆 晚餐前后记得补水",
            &[
                "下班路上记得补充水分",
                "晚饭前一杯水，有助控制饮食",
                "今天喝水达标了吗？",
            ][..],
        ),
        21..=23 => (
            "🌙 晚间小口补水",
            &[
                "晚间少量多次，避免夜间频繁起床",
                "温热的水最舒服",
                "今天辛苦啦，记得补足水分",
            ][..],
        ),
        _ => (
            "💧 喝水时间到",
            &[
                "该喝水了，保持水分很重要～",
                "夜间也要记得少量补水",
                "小口温水最舒服",
            ][..],
        ),
    };

    let body = msgs[chrono::Local::now().minute() as usize % msgs.len()].to_string();
    (title.to_string(), body)
}

/// 排班模式：按办公时段智能提醒
/// - 首条提醒延迟 2 分钟（避免刚进入就发）
/// - 时段未完成时每 15 分钟重试
/// - 时段空档期（9:30-10:30 等）每 30 分钟兜底提醒
fn handle_schedule_mode(app_handle: &AppHandle, now: &chrono::DateTime<chrono::Local>, schedule_completed: &[bool]) {
    let minutes = now.hour() as u32 * 60 + now.minute() as u32;

    // 白名单检测：用户在白名单应用中跳过所有喝水提醒
    if crate::whitelist::is_in_whitelist() {
        return;
    }

    // 检测模式/跨天切换：若从其他模式切来，重置所有计时器
    let mut state = SCHEDULE_STATE.lock().unwrap();
    let mode_changed = state.last_mode != Some("schedule");
    if mode_changed {
        state.last_slot_index = None;
        state.slot_entered_at = None;
        state.first_reminded = false;
        state.last_gap_remind = None;
        if let Ok(mut lt) = LAST_REMINDER_TIME.lock() {
            *lt = Some(Instant::now()); // 重置为现在，避免切换瞬间立即触发
        }
        info!("Water schedule mode: state reset (mode change detected)");
    }
    state.last_mode = Some("schedule");
    drop(state);

    // 查找当前时段
    if let Some(idx) = find_current_slot(minutes) {
        let slot = &WATER_SCHEDULE[idx];
        let completed = schedule_completed.get(idx).copied().unwrap_or(false);

        // 检测时段切换
        let mut state = SCHEDULE_STATE.lock().unwrap();
        let slot_changed = state.last_slot_index != Some(idx);
        if slot_changed {
            state.slot_entered_at = Some(Instant::now());
            state.first_reminded = false;
            state.last_gap_remind = None;
            state.last_slot_index = Some(idx);
        }

        if completed {
            // 当前时段已完成，计算到下一时段的倒计时
            let remaining_secs = if let Some(next_idx) = find_next_slot(minutes) {
                (WATER_SCHEDULE[next_idx].start_minutes - minutes) as u64 * 60
            } else {
                60
            };
            NEXT_REMINDER_SECONDS.store(remaining_secs, Ordering::SeqCst);
        } else {
            // 未完成：首条延迟 2 分钟，重复间隔 15 分钟
            const FIRST_DELAY: u64 = 120;
            const REPEAT_INTERVAL: u64 = 900;

            let entry_at = state.slot_entered_at.unwrap_or_else(Instant::now);
            let elapsed_in_slot = entry_at.elapsed().as_secs();
            drop(state);

            let should_remind = if mode_changed || elapsed_in_slot < FIRST_DELAY {
                // 模式刚切来 或 还在 2 分钟缓冲期内 → 不发
                false
            } else {
                if let Ok(last_time) = LAST_REMINDER_TIME.lock() {
                    last_time
                        .map(|t| t.elapsed().as_secs() >= REPEAT_INTERVAL)
                        .unwrap_or(true)
                } else {
                    false
                }
            };

            if should_remind {
                let title = format!("{} {} 喝水提醒", slot.icon, slot.label);
                let body = format!("建议饮水 {}ml — {}", slot.amount_ml, slot.message);
                info!("[water] Scheduler firing reminder in slot {} ({})", idx, slot.label);
                send_water_notification(app_handle, &title, &body);
                if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
                    *last_time = Some(Instant::now());
                }
                if let Ok(mut st) = SCHEDULE_STATE.lock() {
                    st.first_reminded = true;
                }
            }

            // 剩余时间：到时段结束 + 缓冲
            let remaining = (slot.end_minutes - minutes + 1) as u64 * 60;
            NEXT_REMINDER_SECONDS.store(remaining, Ordering::SeqCst);
        }
    } else {
        // 不在任何排班时段内 → 空档期兜底：每 30 分钟轻度提醒一次
        const GAP_INTERVAL: u64 = 1800;
        let mut state = SCHEDULE_STATE.lock().unwrap();
        // 离开时段，重置 slot 状态
        state.last_slot_index = None;
        state.slot_entered_at = None;
        state.first_reminded = false;

        let should_gap_remind = if mode_changed {
            false
        } else if let Some(last) = state.last_gap_remind {
            last.elapsed().as_secs() >= GAP_INTERVAL
        } else {
            true
        };

        if should_gap_remind {
            let (title, body) = build_gap_reminder_message(minutes);
            send_water_notification(app_handle, &title, &body);
            state.last_gap_remind = Some(Instant::now());
            if let Ok(mut lt) = LAST_REMINDER_TIME.lock() {
                *lt = Some(Instant::now());
            }
        }

        let remaining_secs = if let Some(next_idx) = find_next_slot(minutes) {
            (WATER_SCHEDULE[next_idx].start_minutes - minutes) as u64 * 60
        } else {
            3600
        };
        NEXT_REMINDER_SECONDS.store(remaining_secs, Ordering::SeqCst);
    }
}

/// 空档期兜底提醒文案（按时段）
fn build_gap_reminder_message(minutes: u32) -> (String, String) {
    let hour = minutes / 60;
    let (title, body) = match hour {
        9..=11 => ("💧 排班间隙补水", "还没到下一时段，先来杯小水润润喉～"),
        12..=13 => ("💧 午休间隙", "午休后记得补一杯水，状态恢复更快"),
        14..=17 => ("💧 下午排班间隙", "距离下一时段还有些时间，先喝口水保持节奏"),
        18..=22 => ("💧 晚间补水", "排班时段已过，但今日目标还没达标哦"),
        _ => ("💧 喝水时间到", "保持水分摄入，别让身体脱水"),
    };
    (title.to_string(), body.to_string())
}

fn send_water_notification(app_handle: &AppHandle, title: &str, body: &str) {
    info!("Sending water reminder: {} - {}", title, body);

    // 1) 跨平台系统通知（Windows / macOS / Linux）
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = app_handle
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
        {
            warn!("System notification failed: {}", e);
        }
    }

    // 2) 立即通知前端（modal 主通道）— 必须先 emit，build() 可能在 dev 模式挂起
    if let Err(e) = app_handle.emit("water-reminder", body) {
        warn!("emit water-reminder failed: {}", e);
    } else {
        info!("[water-reminder] ✓ emit water-reminder to frontend");
    }

    // 3) 独立 always_on_top 提醒窗口（次要通道）— 异步执行，不阻塞主流程
    let app_clone = app_handle.clone();
    let title_clone = title.to_string();
    let body_clone = body.to_string();
    std::thread::spawn(move || {
        show_water_reminder_window(&app_clone, &title_clone, &body_clone);
    });
}

/// 弹出独立的喝水提醒窗口（always_on_top，可跨应用看到）
fn show_water_reminder_window(app_handle: &AppHandle, title: &str, body: &str) {
    info!("[water-reminder] === START ===");
    info!("[water-reminder] title={}, body={}", title, body);

    // 记录当前所有窗口（用于诊断）
    let all_windows: Vec<String> = app_handle.webview_windows().keys().cloned().collect();
    info!("[water-reminder] All existing windows BEFORE: {:?}", all_windows);

    // 关闭已存在的旧喝水提醒窗口（避免堆积）
    let mut closed = 0;
    for (win_label, win) in app_handle.webview_windows() {
        if win_label.starts_with("water-reminder-") {
            info!("[water-reminder] Closing existing: {}", win_label);
            if let Err(e) = win.close() {
                warn!("[water-reminder] Failed to close {}: {}", win_label, e);
            } else {
                closed += 1;
            }
        }
    }
    info!("[water-reminder] Closed {} existing water-reminder windows", closed);

    // 读取当前喝水配置用于显示进度
    let (cup_ml, drink_count, daily_goal_ml) = {
        if let Some(state) = app_handle.try_state::<crate::AppState>() {
            if let Ok(config) = state.config.lock() {
                let v = (
                    config.water.cup_size_ml,
                    config.water.stats.drink_count,
                    config.water.daily_goal_ml,
                );
                info!("[water-reminder] Config: cup={}ml, count={}, goal={}ml", v.0, v.1, v.2);
                v
            } else {
                warn!("[water-reminder] Config lock failed, using defaults");
                (250, 0, 2000)
            }
        } else {
            warn!("[water-reminder] AppState not found, using defaults");
            (250, 0, 2000)
        }
    };

    let daily_cups = if cup_ml > 0 { daily_goal_ml / cup_ml } else { 8 };
    let progress = format!("{}/{}", drink_count, daily_cups);
    info!("[water-reminder] Progress: {}", progress);

    // 使用时间戳保证 label 唯一
    let label = format!("water-reminder-{}", chrono::Local::now().timestamp_millis());
    info!("[water-reminder] New label: {}", label);

    // URL 格式与 medication reminder 一致（无前导 /）
    let url = format!(
        "water-reminder.html?title={}&body={}&amount={}ml&progress={}&cup_ml={}",
        simple_url_encode(title),
        simple_url_encode(body),
        cup_ml,
        simple_url_encode(&progress),
        cup_ml,
    );
    info!("[water-reminder] URL: {}", url);

    // 构建窗口
    info!("[water-reminder] Building window...");
    let build_result = WebviewWindowBuilder::new(app_handle, &label, WebviewUrl::App(url.into()))
        .title("💧 喝水提醒")
        .inner_size(420.0, 300.0)
        .always_on_top(true)
        .decorations(false)
        .skip_taskbar(false)
        .resizable(false)
        .center()
        .build();

    match build_result {
        Ok(win) => {
            info!("[water-reminder] ✓ Window BUILT: label={}", win.label());
            info!("[water-reminder] Window URL: {:?}", win.url());
            info!("[water-reminder] Window is_visible (initial): {}", win.is_visible().unwrap_or(false));
        }
        Err(e) => {
            warn!("[water-reminder] ✗ BUILD FAILED: {}", e);
        }
    }
    info!("[water-reminder] === END ===");
}

/// 简单 URL 编码
fn simple_url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

pub fn record_drink(app_handle: &AppHandle, ml: u32) -> WaterStatus {
    if let Some(state) = app_handle.try_state::<crate::AppState>() {
        if let Ok(mut config) = state.config.lock() {
            config.water.record_drink(ml);

            // 排班模式：标记当前时段为已完成
            if config.water.schedule_enabled {
                let minutes = current_minutes_of_day();
                if let Some(idx) = find_current_slot(minutes) {
                    // 确保 vec 足够长
                    while config.water.stats.schedule_completed.len() <= idx {
                        config.water.stats.schedule_completed.push(false);
                    }
                    config.water.stats.schedule_completed[idx] = true;
                    info!("Schedule slot {} ({}) marked completed", idx, WATER_SCHEDULE[idx].label);
                }
            }

            // 重置提醒计时器
            if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
                *last_time = Some(Instant::now());
            }

            let status = get_water_status(&config.water);
            let _ = config.save();
            info!("Recorded drink: {}ml, total: {}ml", ml, config.water.stats.total_ml);
            return status;
        }
    }
    WaterStatus {
        enabled: false,
        total_ml: 0,
        drink_count: 0,
        daily_goal_ml: 2000,
        progress_percent: 0,
        next_reminder_seconds: 0,
        is_in_active_hours: true,
        schedule_enabled: false,
        current_slot_index: None,
        next_slot_minutes: None,
        next_slot_label: None,
        current_slot_amount: None,
        current_slot_message: None,
        schedule_completed_count: 0,
        schedule_total_slots: 6,
        schedule_slots: Vec::new(),
    }
}

pub fn reset_reminder_timer() {
    if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
        *last_time = Some(Instant::now());
    }
    info!("Water reminder timer reset");
}

pub fn update_next_reminder(interval_secs: u64) {
    let old_interval = LAST_INTERVAL.load(Ordering::SeqCst);
    let new_interval = interval_secs;
    let now = Instant::now();

    // 保留已等待时间：取 "原计划剩余时间" 与 "新间隔" 中的较小者
    let effective_remaining = if let Ok(mut last_time) = LAST_REMINDER_TIME.lock() {
        match *last_time {
            Some(last) => {
                let elapsed = last.elapsed().as_secs();
                let old_remaining = old_interval.saturating_sub(elapsed);
                // 下一触发 = max(0, min(old_remaining, new_interval)) 秒后
                let eff = old_remaining.min(new_interval);
                // 反推 last_time：使得在 new_interval 秒后，elapsed 达到 new_interval
                // 设 last = now - (new_interval - eff)，则 t 秒后 elapsed = t + new_interval - eff
                // 触发条件 elapsed >= new_interval → t >= eff ✓
                let adjust_secs = new_interval.saturating_sub(eff);
                *last_time = Some(now.checked_sub(Duration::from_secs(adjust_secs)).unwrap_or(now));
                eff
            }
            None => {
                // 从未提醒过，按新间隔等待
                *last_time = Some(now);
                new_interval
            }
        }
    } else {
        new_interval
    };

    LAST_INTERVAL.store(new_interval, Ordering::SeqCst);
    NEXT_REMINDER_SECONDS.store(effective_remaining, Ordering::SeqCst);

    info!(
        "Water interval changed: {}s -> {}s, effective remaining: {}s (preserved waited time)",
        old_interval, new_interval, effective_remaining
    );
}

/// 主动触发一次喝水提醒（不依赖调度器，用于诊断和测试弹窗）
pub fn test_water_reminder(app_handle: &AppHandle) {
    info!("Manual water reminder triggered");
    let title = "💧 测试喝水提醒";
    let body = "这是一条测试提醒，用于确认弹窗/通知是否正常工作。";
    send_water_notification(app_handle, title, body);
}
