// 用药提醒模块入口
pub mod scheduler;
pub mod reminder;

use chrono::{Datelike, Timelike};
use log::{error, info};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::{
    AppConfig, DoseStatus, MealRelation, Medication, MedicationForm, MedicationLog,
    MedicationSchedule, ScheduleTime,
};
use crate::AppState;

/// 生成一个简单的 UUID（无需引入额外依赖）
pub fn gen_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let rand: u32 = (nanos as u32).wrapping_mul(2654435761);
    format!("med-{:x}-{:x}", nanos, rand)
}

/// 获取今日某药品的固定时间点展开结果
pub fn expand_today_times(med: &Medication) -> Vec<(u32, u32, String)> {
    let mut out = Vec::new();
    if med.interval_hours > 0 {
        // 按 N 小时间隔生成（简单实现：从 8:00 开始，间隔 N 小时）
        let mut h: u32 = 8;
        while h < 24 {
            out.push((h, 0, format!("每{}小时", med.interval_hours)));
            h += med.interval_hours;
        }
    } else {
        for t in &med.schedule.times {
            out.push((t.hour, t.minute, t.label.clone()));
        }
    }
    out
}

/// 判断今天是否需要服用
pub fn is_scheduled_today(med: &Medication) -> bool {
    if !med.enabled {
        return false;
    }
    if med.schedule.days.is_empty() {
        return true;
    }
    // 1=Monday..7=Sunday  (chrono: Mon=0..Sun=6)
    let weekday = chrono::Local::now().weekday().number_from_monday();
    med.schedule.days.contains(&weekday)
}

/// 构建今日 logs（按时间顺序，未触发时为 Pending）
pub fn build_today_logs(config: &mut AppConfig) {
    if !config.medication.enabled {
        return;
    }
    // 清空今日 Pending 状态（保留已确认的）
    config.medication.today_logs.retain(|l| l.status != DoseStatus::Pending);

    let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    for med in &config.medication.medications {
        if !is_scheduled_today(med) {
            continue;
        }
        for (h, m, label) in expand_today_times(med) {
            let scheduled_time = format!("{:02}:{:02}", h, m);
            // 跳过已存在的
            let exists = config.medication.today_logs.iter().any(|l| {
                l.medication_id == med.id && l.scheduled_time == scheduled_time
            });
            if exists {
                continue;
            }
            let log = MedicationLog {
                id: gen_id(),
                medication_id: med.id.clone(),
                medication_name: med.name.clone(),
                scheduled_time,
                actual_time: None,
                status: DoseStatus::Pending,
                skipped_reason: None,
                notes: if label.is_empty() { None } else { Some(label) },
                severity: 0,
            };
            config.medication.today_logs.push(log);
        }
    }

    // 排序
    config
        .medication
        .today_logs
        .sort_by(|a, b| a.scheduled_time.cmp(&b.scheduled_time));

    let _ = today_str; // 保留以备扩展
}

/// 记录用户已服药
pub fn confirm_dose(app: &AppHandle, log_id: &str) -> Result<MedicationLog, String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let med_name;
    let med_id;

    {
        let log = config
            .medication
            .today_logs
            .iter_mut()
            .find(|l| l.id == log_id)
            .ok_or_else(|| "找不到该服药记录".to_string())?;
        log.status = DoseStatus::Taken;
        log.actual_time = Some(chrono::Local::now().format("%H:%M:%S").to_string());
        med_name = log.medication_name.clone();
        med_id = log.medication_id.clone();
    }

    // 扣减库存
    if let Some(med) = config.medication.find_medication_mut(&med_id) {
        if let Some(stock) = med.stock_remaining {
            let qty = med.quantity_per_dose.max(0.0);
            med.stock_remaining = Some((stock - qty).max(0.0));
        }
    }

    // 宠物心情提升（按时服药）
    config.pet.mood = (config.pet.mood + 3).min(100);

    let result = config
        .medication
        .today_logs
        .iter()
        .find(|l| l.id == log_id)
        .cloned()
        .ok_or_else(|| "记录丢失".to_string())?;

    config.save().map_err(|e| e.to_string())?;
    info!("Medication confirmed: {} ({})", med_name, log_id);

    // 关闭对应的全屏提醒窗口
    crate::medication::reminder::close_reminder_window(app, log_id);
    // 广播事件以便主窗口与窗口间同步
    let _ = app.emit("medication-dosed", &result);

    Ok(result)
}

/// 跳过本次服药
pub fn skip_dose(
    app: &AppHandle,
    log_id: &str,
    reason: Option<String>,
) -> Result<MedicationLog, String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    {
        let log = config
            .medication
            .today_logs
            .iter_mut()
            .find(|l| l.id == log_id)
            .ok_or_else(|| "找不到该服药记录".to_string())?;
        log.status = DoseStatus::Skipped;
        log.skipped_reason = reason.clone();
    }

    // 宠物心情略降
    config.pet.mood = config.pet.mood.saturating_sub(5);

    let result = config
        .medication
        .today_logs
        .iter()
        .find(|l| l.id == log_id)
        .cloned()
        .ok_or_else(|| "记录丢失".to_string())?;

    config.save().map_err(|e| e.to_string())?;
    info!("Medication skipped: {} ({:?})", result.medication_name, reason);

    crate::medication::reminder::close_reminder_window(app, log_id);
    let _ = app.emit("medication-dosed", &result);

    Ok(result)
}

/// 稍后提醒（分钟）
pub fn snooze_dose(app: &AppHandle, log_id: &str, minutes: u32) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    {
        let log = config
            .medication
            .today_logs
            .iter_mut()
            .find(|l| l.id == log_id)
            .ok_or_else(|| "找不到该服药记录".to_string())?;
        // 推迟时间
        let parts: Vec<&str> = log.scheduled_time.split(':').collect();
        if parts.len() == 2 {
            let h: u32 = parts[0].parse().unwrap_or(0);
            let m: u32 = parts[1].parse().unwrap_or(0);
            let total = h * 60 + m + minutes;
            let new_h = (total / 60) % 24;
            let new_m = total % 60;
            log.scheduled_time = format!("{:02}:{:02}", new_h, new_m);
            log.status = DoseStatus::Pending;
            log.severity = 0;
        }
    }
    config.save().map_err(|e| e.to_string())?;
    info!("Medication snoozed: {} (+{}min)", log_id, minutes);

    crate::medication::reminder::close_reminder_window(app, log_id);
    let _ = app.emit("medication-snoozed", log_id);

    Ok(())
}

/// 添加药品
pub fn add_medication(app: &AppHandle, mut med: Medication) -> Result<Medication, String> {
    if med.name.trim().is_empty() {
        return Err("药品名称不能为空".to_string());
    }
    if med.id.is_empty() {
        med.id = gen_id();
    } else {
        // 若 id 已存在（前端回填）但同 id 药品已存在，则视为更新
        let id_exists = {
            let state = app.state::<AppState>();
            let cfg_lock = state.config.lock();
            match cfg_lock {
                Ok(cfg) => cfg.medication.medications.iter().any(|m| m.id == med.id),
                Err(_) => false,
            }
        };
        if id_exists {
            return update_medication(app, med);
        }
    }
    if med.created_at.is_empty() {
        med.created_at = chrono::Local::now().to_rfc3339();
    }
    if med.color.is_empty() {
        med.color = "#4CAF50".to_string();
    }
    if med.icon.is_empty() {
        med.icon = "💊".to_string();
    }
    med.unit = if med.unit.is_empty() {
        "片".to_string()
    } else {
        med.unit.clone()
    };

    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.medication.medications.push(med.clone());
    // 重建今日计划
    build_today_logs(&mut config);
    config.save().map_err(|e| e.to_string())?;
    info!("Medication added: {} ({})", med.name, med.id);
    Ok(med)
}

/// 更新药品
pub fn update_medication(app: &AppHandle, med: Medication) -> Result<Medication, String> {
    if med.id.is_empty() {
        return Err("药品ID为空，无法更新（请使用添加功能）".to_string());
    }
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let exists = config.medication.medications.iter().any(|m| m.id == med.id);
    if !exists {
        let existing: Vec<String> = config
            .medication
            .medications
            .iter()
            .map(|m| format!("{}({})", m.name, &m.id[..m.id.len().min(8)]))
            .collect();
        return Err(format!(
            "药品不存在（id={:.8}，当前药品：{}）",
            med.id,
            if existing.is_empty() {
                "无".to_string()
            } else {
                existing.join(", ")
            }
        ));
    }

    // 保留不可变字段（创建时间、原始 schedule.days/start_date）
    if let Some(slot) = config.medication.medications.iter_mut().find(|m| m.id == med.id) {
        let original_created_at = slot.created_at.clone();
        let original_start_date = slot.schedule.start_date.clone();
        let original_days = slot.schedule.days.clone();
        let original_enabled = slot.enabled;
        let original_stock = slot.stock_remaining;
        let mut updated = med.clone();
        if !original_created_at.is_empty() && updated.created_at.is_empty() {
            updated.created_at = original_created_at;
        }
        if updated.schedule.start_date.is_empty() && !original_start_date.is_empty() {
            updated.schedule.start_date = original_start_date;
        }
        if updated.schedule.days.is_empty() && !original_days.is_empty() {
            updated.schedule.days = original_days;
        }
        if original_stock.is_some() && updated.stock_remaining.is_none() {
            updated.stock_remaining = original_stock;
        }
        // 保留原 enabled 状态（编辑表单未显式提供 enabled）
        updated.enabled = original_enabled;
        *slot = updated;
    }

    // 重置今日 logs 中相关 Pending 条目
    config.medication.today_logs.retain(|l| {
        l.medication_id != med.id || l.status != DoseStatus::Pending
    });
    build_today_logs(&mut config);
    config.save().map_err(|e| e.to_string())?;
    info!("Medication updated: {} ({})", med.name, &med.id[..med.id.len().min(8)]);
    Ok(med)
}

/// 删除药品
pub fn delete_medication(app: &AppHandle, med_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let before = config.medication.medications.len();
    config
        .medication
        .medications
        .retain(|m| m.id != med_id);
    if config.medication.medications.len() == before {
        return Err("药品不存在".to_string());
    }
    config.medication.today_logs.retain(|l| l.medication_id != med_id);
    config.save().map_err(|e| e.to_string())?;
    info!("Medication deleted: {}", med_id);
    Ok(())
}

/// 获取下一次用药时间（秒数 + 药品名）
pub fn next_medication_seconds(config: &AppConfig) -> Option<(u64, String, String)> {
    if !config.medication.enabled {
        return None;
    }
    let now = chrono::Local::now();
    let now_secs = now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    let mut best: Option<(u64, String, String)> = None;
    for log in &config.medication.today_logs {
        if log.status != DoseStatus::Pending {
            continue;
        }
        let parts: Vec<&str> = log.scheduled_time.split(':').collect();
        if parts.len() != 2 {
            continue;
        }
        let h: u64 = parts[0].parse().unwrap_or(0);
        let m: u64 = parts[1].parse().unwrap_or(0);
        let target = h * 3600 + m * 60;
        let diff = if target >= now_secs {
            target - now_secs
        } else {
            // 已过时间，归到明天
            target + 24 * 3600 - now_secs
        };
        let is_better = match &best {
            Some((d, _, _)) => diff < *d,
            None => true,
        };
        if is_better {
            best = Some((diff, log.medication_name.clone(), log.scheduled_time.clone()));
        }
    }
    best
}

/// 触发 reminder 流程（被调度器调用）
pub fn trigger_reminder(app: &AppHandle) {
    use scheduler::{update_due_state, MEDICATION_TIMER_RUNNING};
    use std::sync::atomic::Ordering;

    if !MEDICATION_TIMER_RUNNING.load(Ordering::SeqCst) {
        return;
    }

    // 第一步：构建今日 logs（仅在缺失时）
    build_today_if_empty(app);

    // 第二步：更新 due 状态
    let due_logs = update_due_state(app);

    // 第三步：发送通知
    for log in due_logs {
        if let Err(e) = reminder::send_reminder(app, &log) {
            error!("Failed to send medication reminder: {}", e);
        }
    }

    // 第四步：检查库存预警
    if let Err(e) = reminder::check_stock_alerts(app) {
        error!("Failed to check stock alerts: {}", e);
    }
}

fn build_today_if_empty(app: &AppHandle) {
    let state = app.state::<AppState>();
    let result = state.config.lock();
    if let Ok(mut config) = result {
        if config.medication.enabled && config.medication.today_logs.is_empty() {
            build_today_logs(&mut config);
            let _ = config.save_silent();
        }
    }
}

/// 公开接口：手动补建今日 logs（前端调用）
pub fn rebuild_today(app: &AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    build_today_logs(&mut config);
    let n = config.medication.today_logs.len();
    config.save().map_err(|e| e.to_string())?;
    Ok(n)
}

// Re-export 常用枚举辅助方法（前端 enum 序数化）
pub fn form_to_str(f: &MedicationForm) -> &'static str {
    match f {
        MedicationForm::Tablet => "Tablet",
        MedicationForm::Capsule => "Capsule",
        MedicationForm::EyeDrop => "EyeDrop",
        MedicationForm::OralLiquid => "OralLiquid",
        MedicationForm::Injection => "Injection",
        MedicationForm::Cream => "Cream",
        MedicationForm::Patch => "Patch",
        MedicationForm::Powder => "Powder",
        MedicationForm::Spray => "Spray",
        MedicationForm::Other => "Other",
    }
}

pub fn str_to_form(s: &str) -> MedicationForm {
    match s {
        "Tablet" => MedicationForm::Tablet,
        "Capsule" => MedicationForm::Capsule,
        "EyeDrop" => MedicationForm::EyeDrop,
        "OralLiquid" => MedicationForm::OralLiquid,
        "Injection" => MedicationForm::Injection,
        "Cream" => MedicationForm::Cream,
        "Patch" => MedicationForm::Patch,
        "Powder" => MedicationForm::Powder,
        "Spray" => MedicationForm::Spray,
        _ => MedicationForm::Other,
    }
}

pub fn relation_to_str(r: &MealRelation) -> &'static str {
    match r {
        MealRelation::BeforeMeal => "BeforeMeal",
        MealRelation::AfterMeal => "AfterMeal",
        MealRelation::WithMeal => "WithMeal",
        MealRelation::EmptyStomach => "EmptyStomach",
        MealRelation::BeforeSleep => "BeforeSleep",
        MealRelation::AnyTime => "AnyTime",
    }
}

pub fn str_to_relation(s: &str) -> MealRelation {
    match s {
        "BeforeMeal" => MealRelation::BeforeMeal,
        "AfterMeal" => MealRelation::AfterMeal,
        "WithMeal" => MealRelation::WithMeal,
        "EmptyStomach" => MealRelation::EmptyStomach,
        "BeforeSleep" => MealRelation::BeforeSleep,
        _ => MealRelation::AnyTime,
    }
}

pub fn status_to_str(s: &DoseStatus) -> &'static str {
    match s {
        DoseStatus::Pending => "Pending",
        DoseStatus::Taken => "Taken",
        DoseStatus::Skipped => "Skipped",
        DoseStatus::Delayed => "Delayed",
        DoseStatus::Missed => "Missed",
    }
}

pub fn str_to_status(s: &str) -> DoseStatus {
    match s {
        "Taken" => DoseStatus::Taken,
        "Skipped" => DoseStatus::Skipped,
        "Delayed" => DoseStatus::Delayed,
        "Missed" => DoseStatus::Missed,
        _ => DoseStatus::Pending,
    }
}

/// 确保药品有合理默认 schedule 字段
pub fn normalize_medication(mut med: Medication) -> Medication {
    if med.color.is_empty() {
        med.color = "#4CAF50".to_string();
    }
    if med.icon.is_empty() {
        med.icon = "💊".to_string();
    }
    if med.unit.is_empty() {
        med.unit = "片".to_string();
    }
    if med.dosage.is_empty() {
        med.dosage = "1".to_string();
    }
    if med.schedule.start_date.is_empty() {
        med.schedule.start_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    }
    med
}

#[allow(dead_code)]
pub fn empty_schedule() -> MedicationSchedule {
    MedicationSchedule {
        times: vec![ScheduleTime {
            hour: 8,
            minute: 0,
            label: "早餐后".to_string(),
        }],
        relation: MealRelation::AfterMeal,
        days: vec![],
        start_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        end_date: None,
    }
}
