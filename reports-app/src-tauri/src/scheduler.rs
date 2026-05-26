/// نظام الجدولة التلقائية للتقارير
/// يدعم: تقارير نصية، PDF، Excel — بمعدلات متكررة (ثوانٍ / دقائق / ساعات / أيام)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tauri::Emitter;

// ─── هياكل البيانات ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledReport {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sql_query: String,
    pub report_title: String,
    /// "text" | "pdf" | "excel"
    pub report_type: String,
    /// أسماء الأعمدة المترجمة للعربية (تطابق ترتيب SELECT)
    pub columns: Vec<String>,
    /// معدل التكرار بالثواني (86400 = يومي، 3600 = ساعي، 300 = كل 5 دقائق)
    pub interval_seconds: u64,
    /// unix timestamp للتشغيل القادم
    pub next_run_unix: u64,
    /// unix timestamp لآخر تشغيل (None إن لم يُشغَّل بعد)
    pub last_run_unix: Option<u64>,
    pub created_at_unix: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportNotification {
    pub id: String,
    pub schedule_id: String,
    pub schedule_name: String,
    pub title: String,
    pub generated_at_unix: u64,
    /// "text" | "pdf" | "excel"
    pub report_type: String,
    /// محتوى نصي (لنوع text)
    pub text_content: Option<String>,
    /// مسار الملف المحلي (لـ pdf / excel)
    pub file_path: Option<String>,
    pub is_read: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchedulerState {
    pub schedules: Vec<ScheduledReport>,
    pub notifications: Vec<ReportNotification>,
}

pub type SharedScheduler = Arc<Mutex<SchedulerState>>;

// ─── مولّد معرّف فريد بسيط ──────────────────────────────────────

pub fn new_id() -> String {
    let t = unix_now_millis();
    let r = rand::random::<u32>();
    format!("{:x}{:08x}", t, r)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

// ─── مسارات التخزين ──────────────────────────────────────────────

fn schedules_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("schedules.json")
}

fn notifications_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("notifications.json")
}

// ─── تحميل / حفظ ────────────────────────────────────────────────

pub fn load_state(data_dir: &PathBuf) -> SchedulerState {
    let schedules = std::fs::read_to_string(schedules_path(data_dir))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<ScheduledReport>>(&s).ok())
        .unwrap_or_default();

    let notifications = std::fs::read_to_string(notifications_path(data_dir))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<ReportNotification>>(&s).ok())
        .unwrap_or_default();

    SchedulerState { schedules, notifications }
}

pub fn save_schedules(data_dir: &PathBuf, schedules: &[ScheduledReport]) {
    if let Ok(json) = serde_json::to_string_pretty(schedules) {
        let _ = std::fs::create_dir_all(data_dir);
        let _ = std::fs::write(schedules_path(data_dir), json);
    }
}

pub fn save_notifications(data_dir: &PathBuf, notifications: &[ReportNotification]) {
    if let Ok(json) = serde_json::to_string_pretty(notifications) {
        let _ = std::fs::create_dir_all(data_dir);
        let _ = std::fs::write(notifications_path(data_dir), json);
    }
}

// ─── صياغة تقرير نصي من نتائج SQL ──────────────────────────────

pub fn format_text_report(title: &str, columns: &[String], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return format!("📊 **{}**\n\nلا توجد نتائج.", title);
    }

    let mut out = format!("📊 **{}**\n\n", title);

    // حساب عرض الأعمدة
    let widths: Vec<usize> = columns.iter().enumerate().map(|(i, h)| {
        let max_data = rows.iter()
            .filter_map(|r| r.get(i))
            .map(|v| v.chars().count())
            .max()
            .unwrap_or(0);
        h.chars().count().max(max_data).max(4)
    }).collect();

    // رأس الجدول
    let header: String = columns.iter().zip(&widths)
        .map(|(h, &w)| format!("{:width$}", h, width = w))
        .collect::<Vec<_>>()
        .join(" | ");
    let sep: String = widths.iter().map(|&w| "-".repeat(w)).collect::<Vec<_>>().join("-+-");

    out.push_str(&format!("| {} |\n", header));
    out.push_str(&format!("|-{}-|\n", sep));

    // صفوف البيانات (أقصى 50 صفاً في التنبيه)
    for row in rows.iter().take(50) {
        let line: String = row.iter().zip(&widths)
            .map(|(v, &w)| {
                let s = v.chars().take(w).collect::<String>();
                format!("{:width$}", s, width = w)
            })
            .collect::<Vec<_>>()
            .join(" | ");
        out.push_str(&format!("| {} |\n", line));
    }

    if rows.len() > 50 {
        out.push_str(&format!("\n_(عرض 50 من {} سجل)_", rows.len()));
    }

    // ملخص إجماليات أرقام
    let totals: Vec<Option<f64>> = (0..columns.len()).map(|c| {
        let nums: Vec<f64> = rows.iter()
            .filter_map(|r| r.get(c)?.trim().replace(',', "").parse::<f64>().ok())
            .collect();
        if nums.len() > 1 { Some(nums.iter().sum()) } else { None }
    }).collect();

    let has_totals = totals.iter().any(|t| t.is_some());
    if has_totals {
        out.push_str("\n\n**الإجماليات:**\n");
        for (col, total) in columns.iter().zip(&totals) {
            if let Some(t) = total {
                out.push_str(&format!("- **{}**: {:.2}\n", col, t));
            }
        }
    }

    out
}

// ─── مهمة الجدولة في الخلفية ────────────────────────────────────

pub async fn run_scheduler(
    shared: SharedScheduler,
    sql_conn: Arc<Mutex<Option<crate::SqlConnection>>>,
    data_dir: PathBuf,
    app_handle: tauri::AppHandle,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        interval.tick().await;
        let now = unix_now();

        // جمع التقارير المستحقة
        let due: Vec<ScheduledReport> = {
            let state = shared.lock().await;
            state.schedules.iter()
                .filter(|s| s.is_active && s.next_run_unix <= now)
                .cloned()
                .collect()
        };

        if due.is_empty() { continue; }

        // نسخة من إعدادات الاتصال
        let conn_opt = {
            let lock = sql_conn.lock().await;
            lock.clone()
        };

        let conn = match conn_opt {
            Some(c) => c,
            None => continue, // لا يوجد اتصال نشط
        };

        for sched in due {
            let result = execute_scheduled_sql(&conn, &sched.sql_query).await;

            let notification = match result {
                Ok((raw_cols, rows)) => {
                    // استخدم الأعمدة المترجمة إن وُجدت، وإلا استخدم أسماء SQL
                    let cols = if sched.columns.len() == raw_cols.len() {
                        sched.columns.clone()
                    } else {
                        raw_cols
                    };

                    match sched.report_type.as_str() {
                        "pdf" => {
                            match crate::pdf_generator::generate_report_pdf(
                                &sched.report_title, &cols, &rows
                            ) {
                                Ok(bytes) => {
                                    let path = save_report_file(&data_dir, &sched.id, "pdf", &bytes);
                                    ReportNotification {
                                        id: new_id(),
                                        schedule_id: sched.id.clone(),
                                        schedule_name: sched.name.clone(),
                                        title: sched.report_title.clone(),
                                        generated_at_unix: now,
                                        report_type: "pdf".to_string(),
                                        text_content: None,
                                        file_path: path,
                                        is_read: false,
                                    }
                                }
                                Err(e) => error_notification(&sched, now, &e),
                            }
                        }
                        "excel" => {
                            match crate::excel_generator::generate_report_excel(
                                &sched.report_title, &cols, &rows
                            ) {
                                Ok(bytes) => {
                                    let path = save_report_file(&data_dir, &sched.id, "xlsx", &bytes);
                                    ReportNotification {
                                        id: new_id(),
                                        schedule_id: sched.id.clone(),
                                        schedule_name: sched.name.clone(),
                                        title: sched.report_title.clone(),
                                        generated_at_unix: now,
                                        report_type: "excel".to_string(),
                                        text_content: None,
                                        file_path: path,
                                        is_read: false,
                                    }
                                }
                                Err(e) => error_notification(&sched, now, &e),
                            }
                        }
                        _ => {
                            // text (default)
                            let text = format_text_report(&sched.report_title, &cols, &rows);
                            ReportNotification {
                                id: new_id(),
                                schedule_id: sched.id.clone(),
                                schedule_name: sched.name.clone(),
                                title: sched.report_title.clone(),
                                generated_at_unix: now,
                                report_type: "text".to_string(),
                                text_content: Some(text),
                                file_path: None,
                                is_read: false,
                            }
                        }
                    }
                }
                Err(e) => error_notification(&sched, now, &e),
            };

            // تحديث الجدول وإضافة التنبيه
            {
                let mut state = shared.lock().await;

                // حدّث next_run للجدول
                if let Some(s) = state.schedules.iter_mut().find(|s| s.id == sched.id) {
                    s.last_run_unix = Some(now);
                    s.next_run_unix = now + sched.interval_seconds;
                }

                // أضف التنبيه (احتفظ بآخر 100 تنبيه فقط)
                state.notifications.insert(0, notification.clone());
                state.notifications.truncate(100);

                save_schedules(&data_dir, &state.schedules);
                save_notifications(&data_dir, &state.notifications);
            }

            // أرسل حدث Tauri للواجهة
            let _ = app_handle.emit("report-notification", &notification);
        }
    }
}

// ─── تنفيذ SQL من الجدول ────────────────────────────────────────

async fn execute_scheduled_sql(
    conn: &crate::SqlConnection,
    sql: &str,
) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    use tiberius::{Client, AuthMethod, Config};
    use tokio::net::TcpStream;
    use tokio_util::compat::TokioAsyncWriteCompatExt;

    let mut config = Config::new();
    config.host(&conn.server);
    config.port(conn.port);
    config.database(&conn.database);
    if conn.use_windows_auth {
        config.authentication(AuthMethod::Integrated);
    } else {
        config.authentication(AuthMethod::sql_server(&conn.username, &conn.password));
    }
    config.trust_cert();

    let tcp = TcpStream::connect(format!("{}:{}", conn.server, conn.port))
        .await
        .map_err(|e| e.to_string())?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| e.to_string())?;

    let rows = client
        .simple_query(sql)
        .await
        .map_err(|e| e.to_string())?
        .into_first_result()
        .await
        .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Ok((vec![], vec![]));
    }

    let columns: Vec<String> = rows[0].columns().iter()
        .map(|c| c.name().to_string())
        .collect();

    let data: Vec<Vec<String>> = rows.iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| crate::row_cell_to_string(row, i))
                .collect()
        })
        .collect();

    Ok((columns, data))
}

// ─── حفظ ملف تقرير ────────────────────────────────────────────

fn save_report_file(data_dir: &PathBuf, id: &str, ext: &str, bytes: &[u8]) -> Option<String> {
    let reports_dir = data_dir.join("reports");
    let _ = std::fs::create_dir_all(&reports_dir);
    let filename = format!("report_{}_{}.{}", id, unix_now(), ext);
    let path = reports_dir.join(&filename);
    std::fs::write(&path, bytes).ok()?;
    path.to_str().map(str::to_string)
}

// ─── تنبيه خطأ ───────────────────────────────────────────────────

fn error_notification(sched: &ScheduledReport, now: u64, err: &str) -> ReportNotification {
    ReportNotification {
        id: new_id(),
        schedule_id: sched.id.clone(),
        schedule_name: sched.name.clone(),
        title: format!("⚠️ فشل: {}", sched.report_title),
        generated_at_unix: now,
        report_type: "text".to_string(),
        text_content: Some(format!("خطأ في تنفيذ التقرير المجدوَل:\n{}", err)),
        file_path: None,
        is_read: false,
    }
}

// ─── صياغة وصف الفترة الزمنية ────────────────────────────────────

pub fn describe_interval(seconds: u64) -> String {
    if seconds >= 86400 * 7 {
        format!("كل {} أيام", seconds / 86400)
    } else if seconds >= 86400 {
        let d = seconds / 86400;
        if d == 1 { "يومياً".to_string() } else { format!("كل {} أيام", d) }
    } else if seconds >= 3600 {
        let h = seconds / 3600;
        if h == 1 { "كل ساعة".to_string() } else { format!("كل {} ساعات", h) }
    } else if seconds >= 60 {
        let m = seconds / 60;
        if m == 1 { "كل دقيقة".to_string() } else { format!("كل {} دقائق", m) }
    } else {
        format!("كل {} ثانية", seconds)
    }
}
