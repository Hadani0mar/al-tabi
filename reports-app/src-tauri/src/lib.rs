use rust_decimal::Decimal;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

pub mod telegram;
pub mod supabase_config;
pub mod pdf_generator;
pub mod excel_generator;
pub mod ai_agent;
pub mod agent_tools;
pub mod scheduler;

pub struct AppState {
    pub conn: Arc<Mutex<Option<SqlConnection>>>,
    pub bot_cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// إيقاف طلبات الوكيل المحلي — مفتاح = request_id من الواجهة
    pub ai_cancels: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    /// طلب واحد للوكيل في كل مرة — يمنع تداخل الجلسات عند إرسال مزدوج
    pub ai_request_lock: Arc<tokio::sync::Mutex<()>>,
    pub agent_session: Arc<Mutex<agent_tools::AgentSessionState>>,
    pub scheduler: scheduler::SharedScheduler,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
            bot_cancel: Arc::new(Mutex::new(None)),
            ai_cancels: Arc::new(Mutex::new(HashMap::new())),
            ai_request_lock: Arc::new(tokio::sync::Mutex::new(())),
            agent_session: Arc::new(Mutex::new(agent_tools::AgentSessionState::default())),
            scheduler: Arc::new(Mutex::new(scheduler::SchedulerState::default())),
        }
    }
}

// ─── نماذج البيانات ───────────────────────────────────────────
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SqlConnection {
    pub server: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub use_windows_auth: bool,
}

#[derive(Debug, Serialize)]
pub struct ConnectionResult {
    pub success: bool,
    pub message: String,
    pub server_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
}

// ─── مفتاح التشفير ────────────────────────────────────────────
const ENCRYPTION_KEY: &[u8; 32] = b"ReportsApp-SecureKey-2026-v1.0!!";

// ─── تشفير ────────────────────────────────────────────────────
#[tauri::command]
fn encrypt_value(value: String) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(ENCRYPTION_KEY).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, value.as_bytes()).map_err(|e| e.to_string())?;
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(combined))
}

// ─── فك تشفير ─────────────────────────────────────────────────
#[tauri::command]
fn decrypt_value(encrypted: String) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(ENCRYPTION_KEY).map_err(|e| e.to_string())?;
    let combined = BASE64.decode(encrypted).map_err(|e| e.to_string())?;
    if combined.len() < 12 {
        return Err("بيانات مشفرة غير صالحة".to_string());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "فشل فك التشفير".to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

// ─── اختبار الاتصال ───────────────────────────────────────────
#[tauri::command]
async fn test_sql_connection(conn: SqlConnection) -> Result<ConnectionResult, String> {
    Ok(try_connect(conn).await)
}

async fn try_connect(conn: SqlConnection) -> ConnectionResult {
    let mut config = build_config(&conn);
    config.trust_cert();

    match TcpStream::connect(format!("{}:{}", conn.server, conn.port)).await {
        Ok(tcp) => match Client::connect(config, tcp.compat_write()).await {
            Ok(mut client) => {
                let version = match client.query("SELECT @@VERSION AS version", &[]).await {
                    Ok(stream) => match stream.into_row().await {
                        Ok(Some(row)) => row
                            .get::<&str, _>(0)
                            .map(|s| s.lines().next().unwrap_or(s).to_string()),
                        _ => None,
                    },
                    Err(_) => None,
                };

                ConnectionResult {
                    success: true,
                    message: format!("✅ تم الاتصال بنجاح بقاعدة البيانات «{}»", conn.database),
                    server_version: version,
                }
            }
            Err(e) => ConnectionResult {
                success: false,
                message: format!("❌ فشل الاتصال: {}", e),
                server_version: None,
            },
        },
        Err(e) => ConnectionResult {
            success: false,
            message: format!("❌ تعذّر الوصول للسيرفر: {}", e),
            server_version: None,
        },
    }
}

// ─── تنفيذ استعلام SQL عام ────────────────────────────────────
// sql_query: الاستعلام الكامل الجاهز للتنفيذ (بدون placeholders)
#[tauri::command]
async fn execute_sql_query(
    conn: SqlConnection,
    sql_query: String,
) -> Result<QueryResult, String> {
    let mut config = build_config(&conn);
    config.trust_cert();

    let tcp = TcpStream::connect(format!("{}:{}", conn.server, conn.port))
        .await
        .map_err(|e| format!("تعذّر الوصول للسيرفر: {}", e))?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("فشل الاتصال: {}", e))?;

    let rows = client
        .simple_query(&sql_query)
        .await
        .map_err(|e| format!("خطأ في تنفيذ الاستعلام: {}", e))?
        .into_first_result()
        .await
        .map_err(|e| format!("خطأ في قراءة النتائج: {}", e))?;

    if rows.is_empty() {
        return Ok(QueryResult { columns: vec![], rows: vec![], row_count: 0 });
    }

    let columns: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();

    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| row_cell_to_string(row, i))
                .collect()
        })
        .collect();

    let row_count = data.len();
    Ok(QueryResult { columns, rows: data, row_count })
}

// ─── بحث عن أسماء المنتجات (للـ autocomplete) ───────────────
#[tauri::command]
async fn search_products(
    conn: SqlConnection,
    query: String,
) -> Result<Vec<String>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let escaped = query.replace('\'', "''");
    let sql = format!(
        "SELECT DISTINCT TOP 20 ITEM_NAME \
         FROM dbo.BUY_ITEMS_INVOICE_VIEW \
         WHERE ITEM_NAME LIKE N'%{}%' \
         ORDER BY ITEM_NAME",
        escaped
    );

    let mut config = build_config(&conn);
    config.trust_cert();

    let tcp = TcpStream::connect(format!("{}:{}", conn.server, conn.port))
        .await
        .map_err(|e| format!("تعذّر الوصول: {}", e))?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("فشل الاتصال: {}", e))?;

    let rows = client
        .simple_query(&sql)
        .await
        .map_err(|e| format!("خطأ في البحث: {}", e))?
        .into_first_result()
        .await
        .map_err(|e| format!("خطأ في القراءة: {}", e))?;

    let names = rows
        .iter()
        .filter_map(|r| r.try_get::<&str, _>(0).ok().flatten().map(str::to_string))
        .collect();

    Ok(names)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProductMention {
    pub name: String,
    pub code: String,
}

/// بحث منتجات للإكمال التلقائي (@) في شات الوكيل — يستخدم الاتصال النشط في AppState
#[tauri::command]
async fn search_product_mentions(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ProductMention>, String> {
    let conn_opt = state.conn.lock().await.clone();
    let conn = conn_opt.ok_or("غير متصل بقاعدة البيانات. سجّل الدخول أولاً.")?;

    let q = query.trim();
    let escaped = q.replace('\'', "''");

    let sql = if q.is_empty() {
        "SELECT TOP 12 I.ITEM_NAME, I.ITEM_MODEL \
         FROM dbo.ITEMS I \
         WHERE I.ITEM_INVISIBLE = 0 \
         ORDER BY I.ITEM_UPDATE_DATE DESC".to_string()
    } else {
        format!(
            "SELECT TOP 15 I.ITEM_NAME, I.ITEM_MODEL \
             FROM dbo.ITEMS I \
             WHERE I.ITEM_INVISIBLE = 0 \
               AND (I.ITEM_NAME LIKE N'%{}%' OR I.ITEM_MODEL LIKE N'%{}%') \
             ORDER BY \
               CASE WHEN I.ITEM_MODEL LIKE N'{}%' THEN 0 \
                    WHEN I.ITEM_NAME LIKE N'{}%' THEN 1 \
                    ELSE 2 END, \
               I.ITEM_NAME",
            escaped, escaped, escaped, escaped
        )
    };

    let mut config = build_config(&conn);
    config.trust_cert();

    let tcp = TcpStream::connect(format!("{}:{}", conn.server, conn.port))
        .await
        .map_err(|e| format!("تعذّر الوصول: {}", e))?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("فشل الاتصال: {}", e))?;

    let rows = client
        .simple_query(&sql)
        .await
        .map_err(|e| format!("خطأ في البحث: {}", e))?
        .into_first_result()
        .await
        .map_err(|e| format!("خطأ في القراءة: {}", e))?;

    let mut out = Vec::new();
    for row in rows {
        let name = row
            .try_get::<&str, _>(0)
            .ok()
            .flatten()
            .unwrap_or("")
            .to_string();
        let code = row
            .try_get::<&str, _>(1)
            .ok()
            .flatten()
            .unwrap_or("")
            .to_string();
        if !name.is_empty() || !code.is_empty() {
            out.push(ProductMention { name, code });
        }
    }

    Ok(out)
}

// ─── تنفيذ تقرير مقارنة أسعار الموردين ───────────────────────
#[tauri::command]
async fn execute_search_report(
    conn: SqlConnection,
    sql_template: String,
    search_term: String,
) -> Result<QueryResult, String> {
    if search_term.is_empty() {
        return Err("يرجى إدخال اسم أو كود المنتج".to_string());
    }

    let mut final_sql = sql_template.replace("{{DAYS_RECENT}}", "60");
    final_sql = final_sql.replace("{{DAYS_TOTAL}}",  "180");

    let condition_str = search_term.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let e = t.replace('\'', "''");
            format!("(I.ITEM_NAME LIKE N'%{}%' OR B.BARCODE LIKE N'%{}%')", e, e)
        })
        .collect::<Vec<_>>()
        .join(" OR ");

    let condition = if condition_str.is_empty() { "1=1".to_string() } else { condition_str };
    let escaped = search_term.replace('\'', "''");
    final_sql = final_sql.replace("{{SEARCH_CONDITION}}", &condition);
    final_sql = final_sql.replace("{{SEARCH_TERM}}", &escaped);
    final_sql = final_sql.replace("{{PRODUCTS_LIST}}", &format!("N'{}'", escaped));

    execute_sql_query(conn, final_sql).await
}

#[tauri::command]
async fn set_active_connection(
    conn: SqlConnection,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    *state.conn.lock().await = Some(conn);
    Ok(())
}

#[tauri::command]
async fn load_app_secrets_settings(
    app: tauri::AppHandle,
) -> Result<supabase_config::AppSecretsSettings, String> {
    supabase_config::resolve_app_secrets(&app, decrypt_value, encrypt_value).await
}

#[tauri::command]
async fn save_app_secrets_settings(
    app: tauri::AppHandle,
    settings: supabase_config::AppSecretsSettings,
) -> Result<(), String> {
    if !settings.has_remote_payload() {
        return Err("أدخل مفتاح OpenRouter أو OpenAI.".to_string());
    }

    let access_token = supabase_config::read_stored_access_token(&app, decrypt_value)?
        .unwrap_or_else(|| supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string());

    let remote = supabase_config::AppSecretsSettings {
        openrouter_api_key: settings.openrouter_api_key,
        openai_api_key: settings.openai_api_key,
        ..Default::default()
    };

    supabase_config::save_secrets_to_supabase(&access_token, &remote).await?;
    supabase_config::clear_legacy_ai_keys(&app)?;
    Ok(())
}

#[tauri::command]
async fn save_telegram_settings_local(
    app: tauri::AppHandle,
    bot_token: String,
    chat_id: String,
    enable_queries: bool,
) -> Result<(), String> {
    supabase_config::save_local_telegram_settings(&app, &bot_token, &chat_id, encrypt_value)?;
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("telegram_enable_queries", enable_queries);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn update_telegram_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;

    let enable_queries = match store.get("telegram_enable_queries") {
        Some(v) => v.as_bool().unwrap_or(false),
        None => false,
    };

    let secrets =
        supabase_config::resolve_app_secrets(&app, decrypt_value, encrypt_value).await?;

    let ai_model = crate::ai_agent::DEFAULT_AI_MODEL.to_string();

    let mut cancel_lock = state.bot_cancel.lock().await;
    if let Some(cancel_tx) = cancel_lock.take() {
        let _ = cancel_tx.send(());
    }

    if enable_queries
        && !secrets.telegram_bot_token.is_empty()
        && !secrets.telegram_chat_id.is_empty()
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *cancel_lock = Some(tx);

        let state_clone = Arc::new(AppState {
            conn: state.conn.clone(),
            bot_cancel: Arc::new(Mutex::new(None)),
            ai_cancels: Arc::new(Mutex::new(HashMap::new())),
            ai_request_lock: state.ai_request_lock.clone(),
            agent_session: state.agent_session.clone(),
            scheduler: state.scheduler.clone(),
        });

        let dec_token = secrets.telegram_bot_token;
        let dec_chat_id = secrets.telegram_chat_id;
        let dec_groq_key = secrets.openrouter_api_key;
        let dec_openai_key = secrets.openai_api_key;

        tauri::async_runtime::spawn(async move {
            telegram::start_polling(
                dec_token,
                dec_chat_id,
                dec_groq_key,
                true,
                state_clone,
                rx,
                ai_model,
                dec_openai_key,
            )
            .await;
        });
    }

    Ok(())
}

#[tauri::command]
async fn test_telegram_bot(token: String, chat_id: String) -> Result<String, String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();
    let token = token.trim();
    let chat_id = chat_id.trim();
    
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let res = client.post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": "تم اختبار الاتصال بنجاح من نظام التقارير! ✅"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() {
        Ok("تم الإرسال بنجاح! راجع تليجرام".to_string())
    } else {
        let text = res.text().await.unwrap_or_default();
        Err(format!("فشل الإرسال: {}", text))
    }
}

#[tauri::command]
async fn ask_local_ai(
    message: String,
    history: Vec<serde_json::Value>,
    _groq_key: String,
    ai_model: String,
    request_id: String,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let _agent_slot = app_state.inner().ai_request_lock.lock().await;

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    {
        let mut cancels = app_state.inner().ai_cancels.lock().await;
        cancels.insert(request_id.clone(), cancel_tx);
    }

    {
        let pf = crate::agent_tools::extract_product_filter_from_text(&message)
            .or_else(|| crate::agent_tools::extract_product_filter_from_history(&history));
        let mut session = app_state.inner().agent_session.lock().await;
        if pf.is_some() {
            session.last_product_filter = pf;
        }
    }

    let reports_cache = crate::telegram::fetch_reports().await;
    let state_arc = Arc::new(AppState {
        conn: app_state.inner().conn.clone(),
        bot_cancel: app_state.inner().bot_cancel.clone(),
        ai_cancels: app_state.inner().ai_cancels.clone(),
        ai_request_lock: app_state.inner().ai_request_lock.clone(),
        agent_session: app_state.inner().agent_session.clone(),
        scheduler: app_state.inner().scheduler.clone(),
    });

    let secrets =
        supabase_config::resolve_app_secrets(&app_handle, decrypt_value, encrypt_value).await?;
    let groq_key = secrets.openrouter_api_key;
    let dec_openai_key = secrets.openai_api_key;

    let result = crate::ai_agent::handle_with_groq_local(
        &message,
        history,
        &groq_key,
        &ai_model,
        &state_arc,
        &reports_cache,
        app_handle.clone(),
        &dec_openai_key,
        Some(cancel_rx),
    )
    .await;

    app_state.inner().ai_cancels.lock().await.remove(&request_id);

    match result {
        Ok(mut text) => {
            let last_path = {
                let session = state_arc.agent_session.lock().await;
                session.last_file_path.clone()
            };
            if let Some(p) = last_path.filter(|p| std::path::Path::new(p).exists()) {
                text = if text.contains("[FILE_PATH:") {
                    crate::ai_agent::replace_file_path_tags(&text, &p)
                } else {
                    format!("{}\n\n[FILE_PATH:{}]", text.trim_end(), p)
                };
            }
            Ok(text)
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn cancel_local_ai(
    request_id: String,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut cancels = app_state.inner().ai_cancels.lock().await;
    if let Some(tx) = cancels.remove(&request_id) {
        let _ = tx.send(());
        Ok(())
    } else {
        Err("لا يوجد طلب نشط بهذا المعرف.".to_string())
    }
}

#[tauri::command]
async fn open_local_file(path: String) -> Result<(), String> {
    let path = path.trim().to_string();
    if !std::path::Path::new(&path).exists() {
        return Err(format!(
            "الملف غير موجود على القرص:\n{}\n\nقد يكون المسار وهمياً من الوكيل — أعد طلب التصدير.",
            path
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let quoted = format!("\"{}\"", path);
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "pdf" {
            // فتح PDF في المتصفح الافتراضي عبر file:// URL
            let url = format!("file:///{}", path.replace('\\', "/"));
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &url])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else if ext == "xlsx" || ext == "xls" {
            // فتح ملفات Excel مع تطبيق Excel مباشرة
            std::process::Command::new("cmd")
                .args(["/C", "start", "", "excel.exe", &quoted])
                .spawn()
                .or_else(|_| {
                    // احتياطي: افتح بالتطبيق الافتراضي للامتداد
                    std::process::Command::new("cmd")
                        .args(["/C", "start", "", &quoted])
                        .spawn()
                })
                .map_err(|e| e.to_string())?;
        } else {
            // باقي الملفات: افتح بالتطبيق الافتراضي
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &quoted])
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ─── دوال مساعدة ──────────────────────────────────────────────

fn build_config(conn: &SqlConnection) -> Config {
    let mut config = Config::new();
    config.host(&conn.server);
    config.port(conn.port);
    config.database(&conn.database);
    if conn.use_windows_auth {
        config.authentication(AuthMethod::Integrated);
    } else {
        config.authentication(AuthMethod::sql_server(&conn.username, &conn.password));
    }
    config
}

fn row_cell_to_string(row: &tiberius::Row, idx: usize) -> String {
    // varchar / nvarchar
    if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
        return v.to_string();
    }
    // decimal / numeric
    if let Ok(Some(v)) = row.try_get::<Decimal, _>(idx) {
        return v.to_string();
    }
    // int
    if let Ok(Some(v)) = row.try_get::<i32, _>(idx) {
        return v.to_string();
    }
    if let Ok(Some(v)) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    // float / real
    if let Ok(Some(v)) = row.try_get::<f64, _>(idx) {
        return format!("{:.2}", v);
    }
    if let Ok(Some(v)) = row.try_get::<f32, _>(idx) {
        return format!("{:.2}", v);
    }
    // bool
    if let Ok(Some(v)) = row.try_get::<bool, _>(idx) {
        return if v { "نعم".to_string() } else { "لا".to_string() };
    }
    // chrono datetime
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveDateTime, _>(idx) {
        return v.format("%d/%m/%Y").to_string();
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveDate, _>(idx) {
        return v.format("%d/%m/%Y").to_string();
    }
    // NULL
    String::new()
}

// ─── أوامر الجدولة ────────────────────────────────────────────

#[tauri::command]
async fn get_scheduled_reports(
    app_state: tauri::State<'_, AppState>,
) -> Result<Vec<scheduler::ScheduledReport>, String> {
    let state = app_state.scheduler.lock().await;
    Ok(state.schedules.clone())
}

#[tauri::command]
async fn add_scheduled_report(
    report: scheduler::ScheduledReport,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<scheduler::ScheduledReport, String> {
    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut state = app_state.scheduler.lock().await;
    state.schedules.push(report.clone());
    scheduler::save_schedules(&data_dir, &state.schedules);
    Ok(report)
}

#[tauri::command]
async fn delete_scheduled_report(
    id: String,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut state = app_state.scheduler.lock().await;
    state.schedules.retain(|s| s.id != id);
    scheduler::save_schedules(&data_dir, &state.schedules);
    Ok(())
}

#[tauri::command]
async fn toggle_scheduled_report(
    id: String,
    active: bool,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut state = app_state.scheduler.lock().await;
    if let Some(s) = state.schedules.iter_mut().find(|s| s.id == id) {
        s.is_active = active;
    }
    scheduler::save_schedules(&data_dir, &state.schedules);
    Ok(())
}

#[tauri::command]
async fn get_notifications(
    app_state: tauri::State<'_, AppState>,
) -> Result<Vec<scheduler::ReportNotification>, String> {
    let state = app_state.scheduler.lock().await;
    Ok(state.notifications.clone())
}

#[tauri::command]
async fn mark_notification_read(
    id: String,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut state = app_state.scheduler.lock().await;
    if let Some(n) = state.notifications.iter_mut().find(|n| n.id == id) {
        n.is_read = true;
    }
    scheduler::save_notifications(&data_dir, &state.notifications);
    Ok(())
}

#[tauri::command]
async fn clear_all_notifications(
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut state = app_state.scheduler.lock().await;
    state.notifications.clear();
    scheduler::save_notifications(&data_dir, &state.notifications);
    Ok(())
}

// ─── نقطة الدخول ──────────────────────────────────────────────
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // تحميل حالة الجدولة من القرص
            let data_dir = app.handle().path().app_data_dir()
                .expect("تعذّر الحصول على مجلد بيانات التطبيق");
            let initial_state = scheduler::load_state(&data_dir);
            let shared_scheduler: scheduler::SharedScheduler =
                Arc::new(Mutex::new(initial_state));

            app.manage(AppState {
                conn: Arc::new(Mutex::new(None)),
                bot_cancel: Arc::new(Mutex::new(None)),
                ai_cancels: Arc::new(Mutex::new(HashMap::new())),
                ai_request_lock: Arc::new(tokio::sync::Mutex::new(())),
                agent_session: Arc::new(Mutex::new(agent_tools::AgentSessionState::default())),
                scheduler: shared_scheduler.clone(),
            });

            // بدء مهمة الجدولة في الخلفية
            let app_handle_sched = app.handle().clone();
            let sched_clone = shared_scheduler.clone();
            tauri::async_runtime::spawn(async move {
                let app_state = app_handle_sched.state::<AppState>();
                scheduler::run_scheduler(
                    sched_clone,
                    app_state.conn.clone(),
                    data_dir,
                    app_handle_sched,
                ).await;
            });

            // تشغيل بوت تليجرام إن وُجدت الإعدادات
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                let _ = update_telegram_settings(app_handle.clone(), state).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            encrypt_value,
            decrypt_value,
            load_app_secrets_settings,
            save_app_secrets_settings,
            save_telegram_settings_local,
            test_sql_connection,
            execute_sql_query,
            execute_search_report,
            search_products,
            search_product_mentions,
            set_active_connection,
            update_telegram_settings,
            test_telegram_bot,
            ask_local_ai,
            ai_agent::generate_ai_suggestions,
            cancel_local_ai,
            open_local_file,
            get_scheduled_reports,
            add_scheduled_report,
            delete_scheduled_report,
            toggle_scheduled_report,
            get_notifications,
            mark_notification_read,
            clear_all_notifications,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
