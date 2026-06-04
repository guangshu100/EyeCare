// 提醒发送 + 库存预警

use log::{info, warn};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use crate::config::{MedicationLog, MedicationForm, MealRelation, DoseStatus};
use crate::AppState;

/// 发送用药提醒
pub fn send_reminder(app: &AppHandle, log: &MedicationLog) -> Result<(), String> {
    let med_detail = {
        let state = app.state::<AppState>();
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config
            .medication
            .find_medication(&log.medication_id)
            .map(|m| {
                (
                    m.dosage.clone(),
                    m.unit.clone(),
                    m.quantity_per_dose,
                    m.form.clone(),
                    m.schedule.relation.clone(),
                    m.notes.clone(),
                    m.color.clone(),
                    m.icon.clone(),
                )
            })
    };

    // 个性化系统通知文案
    let (title, body) = build_personalized_message(log, med_detail.as_ref());

    info!(
        "Medication reminder sev={} : {} - {}",
        log.severity, title, body
    );

    // 系统通知（保留以兜底）
    if let Err(e) = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
    {
        warn!("System notification failed: {}", e);
    }

    // 持久化的全屏提醒窗口
    if let Err(e) = show_reminder_window(app, log, &title, &body, med_detail.as_ref()) {
        warn!("Failed to show reminder window: {}", e);
    }

    // 通知前端刷新
    let _ = app.emit("medication-reminder", log);

    // 同步更新托盘 tooltip
    update_tray_tooltip(app);

    Ok(())
}

type MedDetail = (
    String,         // dosage
    String,         // unit
    f32,            // quantity_per_dose
    MedicationForm, // form
    MealRelation,   // relation
    String,         // notes
    String,         // color
    String,         // icon
);

fn build_personalized_message(log: &MedicationLog, detail: Option<&MedDetail>) -> (String, String) {
    let rel_text = detail
        .map(|d| relation_label(&d.4))
        .unwrap_or_default();
    let dosage_str = detail
        .map(|d| {
            if d.0.is_empty() {
                d.1.clone()
            } else {
                format!("{} × {}", d.0, d.1)
            }
        })
        .unwrap_or_default();
    let notes = detail.map(|d| d.5.trim().to_string()).unwrap_or_default();

    let title = match log.severity {
        0 | 1 => "⏰ 该吃药啦".to_string(),
        2 => "💊 别忘了哦".to_string(),
        3 => "⚠️ 已经等了好一会儿".to_string(),
        _ => "🔴 超时未服，请尽快".to_string(),
    };

    let mut body = format!(
        "{}（{}）· 计划时间 {}",
        log.medication_name,
        if dosage_str.is_empty() {
            "请按医嘱服用".to_string()
        } else {
            dosage_str
        },
        log.scheduled_time
    );
    if !rel_text.is_empty() {
        body.push_str(&format!(" · {}", rel_text));
    }
    if !notes.is_empty() && notes.len() <= 30 {
        body.push_str(&format!("\n📝 {}", notes));
    }

    (title, body)
}

fn relation_label(r: &MealRelation) -> String {
    match r {
        MealRelation::BeforeMeal => "饭前服用".to_string(),
        MealRelation::AfterMeal => "饭后服用".to_string(),
        MealRelation::WithMeal => "随餐服用".to_string(),
        MealRelation::EmptyStomach => "空腹服用".to_string(),
        MealRelation::BeforeSleep => "睡前服用".to_string(),
        MealRelation::AnyTime => "".to_string(),
    }
}

/// 显示全屏提醒窗口（持久化、可交互）
fn show_reminder_window(
    app: &AppHandle,
    log: &MedicationLog,
    title: &str,
    body: &str,
    detail: Option<&MedDetail>,
) -> Result<(), String> {
    let label = format!("med-reminder-{}", log.id);

    // 关闭已存在的同 log_id 窗口
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.close();
    }

    // 构建 URL with query params
    let icon = detail.map(|d| d.7.as_str()).unwrap_or("💊");
    let color = detail.map(|d| d.6.as_str()).unwrap_or("#4CAF50");
    let mut url = format!(
        "medication-reminder.html?log_id={}&name={}&title={}&body={}&time={}&severity={}&icon={}&color={}",
        urlencoding(&log.id),
        urlencoding(&log.medication_name),
        urlencoding(title),
        urlencoding(body),
        urlencoding(&log.scheduled_time),
        log.severity,
        urlencoding(icon),
        urlencoding(color),
    );
    if let Some(d) = detail {
        url.push_str(&format!(
            "&dosage={}&unit={}&relation={}&notes={}",
            urlencoding(&d.0),
            urlencoding(&d.1),
            urlencoding(&relation_label(&d.4)),
            urlencoding(&d.5),
        ));
    }

    WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("💊 用药提醒")
        .inner_size(420.0, 320.0)
        .always_on_top(true)
        .decorations(false)
        .skip_taskbar(false)
        .resizable(false)
        .center()
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 简单的 URL 编码（避免引入额外依赖）
fn urlencoding(s: &str) -> String {
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

/// 检查库存预警
pub fn check_stock_alerts(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|e| e.to_string())?;

    let mut alerts: Vec<String> = Vec::new();
    for med in &config.medication.medications {
        if let Some(stock) = med.stock_remaining {
            if stock <= med.stock_alert_threshold {
                alerts.push(format!(
                    "{} 库存仅剩 {:.1}{}（阈值 {:.1}）",
                    med.name, stock, med.unit, med.stock_alert_threshold
                ));
            }
        }
    }

    if alerts.is_empty() {
        config.medication.last_stock_alert = None;
        let _ = config.save_silent();
        return Ok(());
    }

    let summary = alerts.join("；");
    if config.medication.last_stock_alert.as_deref() != Some(summary.as_str()) {
        config.medication.last_stock_alert = Some(summary.clone());
        let _ = config.save_silent();

        info!("Stock alerts: {}", summary);
        let _ = app
            .notification()
            .builder()
            .title("📦 药品库存预警")
            .body(&summary)
            .show();
        let _ = app.emit("medication-stock-alert", &summary);
    }
    Ok(())
}

fn update_tray_tooltip(app: &AppHandle) {
    let eye_health = eye_health_text(app);
    let next: Option<(u64, String, String)> = {
        let state = app.state::<AppState>();
        let cfg_lock = state.config.lock();
        match cfg_lock {
            Ok(config) => crate::medication::next_medication_seconds(&config),
            Err(_) => None,
        }
    };

    let tooltip = match next {
        Some((_secs, name, time)) if _secs < 30 * 60 => {
            format!("EyeCare | 眼睛生命值: {}% | 下次用药: {} {}", eye_health, name, time)
        }
        _ => format!("EyeCare | 眼睛生命值: {}%", eye_health),
    };

    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(tooltip.as_str()));
    }
}

fn eye_health_text(_app: &AppHandle) -> u32 {
    crate::idle::get_eye_health()
}

/// 关闭指定 log 的提醒窗口（用户点击确认/跳过/稍后时调用）
pub fn close_reminder_window(app: &AppHandle, log_id: &str) {
    let label = format!("med-reminder-{}", log_id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.close();
    }
}

/// 抑制未使用警告
#[allow(dead_code)]
fn _status_used(s: DoseStatus) -> DoseStatus {
    s
}
