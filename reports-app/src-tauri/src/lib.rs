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
pub mod agent_memory;
pub mod scheduler;
pub mod pos_sale;
pub mod erp_profile;
pub mod erp_adapters;
pub mod pattern_catalog;
pub mod pharmacy_share;

pub struct AppState {
    pub conn: Arc<Mutex<Option<SqlConnection>>>,
    pub bot_cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// إيقاف طلبات الوكيل المحلي — مفتاح = request_id من الواجهة
    pub ai_cancels: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    /// طلب واحد للوكيل في كل مرة — يمنع تداخل الجلسات عند إرسال مزدوج
    pub ai_request_lock: Arc<tokio::sync::Mutex<()>>,
    pub agent_session: Arc<Mutex<agent_tools::AgentSessionState>>,
    pub scheduler: scheduler::SharedScheduler,
    /// نوع ERP المكتشف من الاتصال النشط (Marketing2026 | InfinityRetailDB)
    pub erp_kind: Arc<Mutex<Option<erp_profile::ErpKind>>>,
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
            erp_kind: Arc::new(Mutex::new(None)),
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
    pub erp_kind: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BusinessProfile {
    pub company_name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub activity_code: Option<String>,
    pub activity_name: Option<String>,
    pub phone: Option<String>,
    pub mobile: Option<String>,
    pub fax: Option<String>,
    pub branch: Option<String>,
    /// marketing2026 | infinity_retail_db
    pub erp_kind: Option<String>,
    pub erp_label: Option<String>,
}

/// بيانات النشاط التجاري من dbo.SITTEINGS (صف الإعدادات العامة)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CancelledInvoiceRow {
    pub invoice_id: i32,
    pub invoice_kind: String,
    pub invoice_kind_label: String,
    pub invoice_time: String,
    pub updated_at: Option<String>,
    pub party_name: String,
    pub employee_name: String,
    pub note: String,
}

fn parse_iso_date_param(value: &str) -> Result<String, String> {
    let t = value.trim();
    if t.len() != 10
        || t.as_bytes().get(4) != Some(&b'-')
        || t.as_bytes().get(7) != Some(&b'-')
        || !t.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return Err("صيغة التاريخ غير صالحة. استخدم YYYY-MM-DD.".to_string());
    }
    Ok(t.to_string())
}

#[tauri::command]
async fn list_cancelled_invoices(
    target_date: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CancelledInvoiceRow>, String> {
    let day = parse_iso_date_param(&target_date)?;
    let conn_opt = state.conn.lock().await.clone();
    let conn = conn_opt.ok_or("غير متصل بقاعدة البيانات. سجّل الدخول أولاً.")?;

    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;
    let sql = erp_adapters::cancelled_invoices_sql(erp, &day);

    let result = execute_sql_query(conn, sql).await?;
    if result.row_count == 0 {
        return Ok(vec![]);
    }

    let idx = |cols: &[String], name: &str| -> Option<usize> {
        cols.iter().position(|c| c.eq_ignore_ascii_case(name))
    };

    let id_i = idx(&result.columns, "invoice_id").ok_or("عمود invoice_id مفقود")?;
    let kind_i = idx(&result.columns, "invoice_kind").unwrap_or(0);
    let kind_label_i = idx(&result.columns, "invoice_kind_label").unwrap_or(0);
    let time_i = idx(&result.columns, "invoice_time").unwrap_or(0);
    let updated_i = idx(&result.columns, "updated_at");
    let party_i = idx(&result.columns, "party_name").unwrap_or(0);
    let emp_i = idx(&result.columns, "employee_name").unwrap_or(0);
    let note_i = idx(&result.columns, "note").unwrap_or(0);

    let cell = |row: &[String], i: usize| -> String {
        row.get(i).cloned().unwrap_or_default().trim().to_string()
    };

    let rows = result
        .rows
        .into_iter()
        .filter_map(|row| {
            let id_str = cell(&row, id_i);
            let invoice_id: i32 = id_str.parse().ok()?;
            Some(CancelledInvoiceRow {
                invoice_id,
                invoice_kind: cell(&row, kind_i),
                invoice_kind_label: cell(&row, kind_label_i),
                invoice_time: cell(&row, time_i),
                updated_at: updated_i
                    .and_then(|i| {
                        let v = cell(&row, i);
                        if v.is_empty() || v == "—" { None } else { Some(v) }
                    }),
                party_name: cell(&row, party_i),
                employee_name: cell(&row, emp_i),
                note: cell(&row, note_i),
            })
        })
        .collect();

    Ok(rows)
}

#[tauri::command]
async fn get_business_profile(
    state: tauri::State<'_, AppState>,
) -> Result<BusinessProfile, String> {
    let conn_opt = state.conn.lock().await.clone();
    let conn = conn_opt.ok_or("غير متصل بقاعدة البيانات. سجّل الدخول أولاً.")?;

    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;

    let query = erp_adapters::fetch_business_profile(&conn, erp);

    tokio::time::timeout(std::time::Duration::from_secs(8), query)
        .await
        .map_err(|_| "انتهت مهلة قراءة بيانات النشاط التجاري (8 ثوانٍ).".to_string())?
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Serialize)]
pub struct TelegramSettingsLocal {
    pub bot_token: String,
    pub chat_id: String,
}

#[tauri::command]
async fn get_pharmacy_share_settings(
    app: tauri::AppHandle,
) -> Result<pharmacy_share::PharmacyShareSettings, String> {
    pharmacy_share::load_settings(&app)
}

#[tauri::command]
async fn save_pharmacy_share_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    sync_key: String,
    sharing_enabled: bool,
    show_prices: bool,
) -> Result<pharmacy_share::PharmacyShareSettings, String> {
    let conn = state
        .conn
        .lock()
        .await
        .clone()
        .ok_or("غير متصل بقاعدة البيانات.")?;
    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;

    let mut settings = pharmacy_share::PharmacyShareSettings {
        sync_key: sync_key.trim().to_string(),
        sharing_enabled,
        show_prices,
        ..pharmacy_share::load_settings(&app)?
    };

    if !sharing_enabled {
        if settings.sync_key.len() >= 8 {
            if let Err(e) = pharmacy_share::stop_pharmacy_sharing(&settings.sync_key).await {
                settings.last_error = Some(e);
            } else {
                settings.last_error = None;
                settings.last_product_count = 0;
                settings.last_sync_at = None;
                settings.last_business_name = None;
            }
        }
        settings.sharing_enabled = false;
        pharmacy_share::save_settings(&app, &settings)?;
        return Ok(settings);
    }

    if settings.sync_key.len() < 8 {
        return Err("أدخل مفتاح المزامنة (sync_key) من Supabase (8 أحرف على الأقل).".to_string());
    }

    match pharmacy_share::sync_pharmacy_share(&app, &conn, erp, &settings).await {
        Ok(r) => {
            settings.last_product_count = r.product_count;
            settings.last_business_name = Some(r.business_name);
            settings.last_sync_at = Some(chrono::Local::now().to_rfc3339());
            settings.last_error = None;
            settings.sharing_enabled = true;
            pharmacy_share::save_settings(&app, &settings)?;
            Ok(settings)
        }
        Err(e) => {
            settings.last_error = Some(e.clone());
            pharmacy_share::save_settings(&app, &settings)?;
            Err(e)
        }
    }
}

#[tauri::command]
async fn sync_pharmacy_products_now(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<pharmacy_share::PharmacyShareSyncResult, String> {
    let mut settings = pharmacy_share::load_settings(&app)?;
    if !settings.sharing_enabled {
        return Err("فعّل مشاركة المنتجات أولاً.".to_string());
    }
    let conn = state
        .conn
        .lock()
        .await
        .clone()
        .ok_or("غير متصل بقاعدة البيانات.")?;
    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;
    let result = pharmacy_share::sync_pharmacy_share(&app, &conn, erp, &settings).await?;
    settings.last_product_count = result.product_count;
    settings.last_business_name = Some(result.business_name.clone());
    settings.last_sync_at = Some(chrono::Local::now().to_rfc3339());
    settings.last_error = None;
    pharmacy_share::save_settings(&app, &settings)?;
    Ok(result)
}

#[tauri::command]
async fn stop_pharmacy_sharing_cmd(
    app: tauri::AppHandle,
) -> Result<pharmacy_share::PharmacyShareSettings, String> {
    let mut settings = pharmacy_share::load_settings(&app)?;
    if settings.sync_key.trim().len() >= 8 {
        if let Err(e) = pharmacy_share::stop_pharmacy_sharing(&settings.sync_key).await {
            settings.last_error = Some(e);
        } else {
            settings.last_error = None;
            settings.last_product_count = 0;
            settings.last_sync_at = None;
            settings.last_business_name = None;
        }
    }
    settings.sharing_enabled = false;
    pharmacy_share::save_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
async fn preview_pharmacy_business_profile(
    state: tauri::State<'_, AppState>,
) -> Result<BusinessProfile, String> {
    let conn = state
        .conn
        .lock()
        .await
        .clone()
        .ok_or("غير متصل بقاعدة البيانات.")?;
    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;
    erp_adapters::fetch_business_profile(&conn, erp).await
}

#[tauri::command]
fn load_telegram_settings_local(
    app: tauri::AppHandle,
) -> Result<TelegramSettingsLocal, String> {
    let (bot_token, chat_id) =
        supabase_config::load_local_telegram_settings(&app, decrypt_value)?;
    Ok(TelegramSettingsLocal { bot_token, chat_id })
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
            Ok(_client) => {
                let version = match execute_sql_query(conn.clone(), "SELECT @@VERSION AS version".to_string()).await {
                    Ok(result) => result.rows.first().and_then(|r| r.first().cloned()),
                    Err(_) => None,
                };
                let erp = erp_profile::detect_erp_kind(&conn).await;
                let erp_label = erp.display_name_ar();

                ConnectionResult {
                    success: true,
                    message: format!(
                        "✅ تم الاتصال بنجاح بقاعدة البيانات «{}» — ERP: {}",
                        conn.database, erp_label
                    ),
                    server_version: version,
                    erp_kind: Some(erp.kind_id().to_string()),
                }
            }
            Err(e) => ConnectionResult {
                success: false,
                message: format!("❌ فشل الاتصال: {}", e),
                server_version: None,
                erp_kind: None,
            },
        },
        Err(e) => ConnectionResult {
            success: false,
            message: format!("❌ تعذّر الوصول للسيرفر: {}", e),
            server_version: None,
            erp_kind: None,
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
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;
    let escaped = query.replace('\'', "''");
    let sql = match erp {
        erp_profile::ErpKind::InfinityRetailDb => {
            erp_adapters::infinity_product_search_sql(&escaped, 20)
        }
        _ => format!(
            "SELECT DISTINCT TOP 20 \
             CASE \
               WHEN NULLIF(LTRIM(RTRIM(CAST(I.ITEM_MODEL AS nvarchar(50)))), '') IS NOT NULL \
                 THEN I.ITEM_NAME + N' (' + CAST(I.ITEM_MODEL AS nvarchar(50)) + N')' \
               ELSE I.ITEM_NAME \
             END AS label \
             FROM dbo.ITEMS I \
             WHERE I.ITEM_INVISIBLE = 0 \
               AND (I.ITEM_NAME LIKE N'%{}%' OR CAST(I.ITEM_MODEL AS nvarchar(50)) LIKE N'%{}%') \
             ORDER BY I.ITEM_NAME",
            escaped, escaped
        ),
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

    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;
    let q = query.trim();
    let escaped = q.replace('\'', "''");

    let sql = match erp {
        erp_profile::ErpKind::InfinityRetailDb => {
            erp_adapters::infinity_product_mentions_sql(&escaped, q.is_empty())
        }
        _ => if q.is_empty() {
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
        },
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
    state: tauri::State<'_, AppState>,
) -> Result<QueryResult, String> {
    if search_term.trim().is_empty() {
        return Err("يرجى إدخال اسم أو كود المنتج".to_string());
    }

    let erp = erp_profile::resolve_erp_kind(&state, &conn).await;
    let final_sql = erp_adapters::resolve_search_report_sql(erp, &sql_template, search_term.trim());

    execute_sql_query(conn, final_sql).await
}

#[tauri::command]
async fn set_active_connection(
    conn: SqlConnection,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let kind = erp_profile::detect_erp_kind(&conn).await;
    *state.erp_kind.lock().await = Some(kind);
    *state.conn.lock().await = Some(conn);
    Ok(kind.display_name_ar().to_string())
}

#[tauri::command]
async fn get_erp_kind(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let kind = erp_profile::current_erp_kind(&state).await;
    Ok(kind.kind_id().to_string())
}

// ─── المفضلة (Favorites) — للاستخدام في صفحة المحفوظات ───────
// لا يُعاد SQL إلى الواجهة (حماية الملكية الفكرية) — التنفيذ يتم بالـ id فقط.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct FavoriteDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at_unix: u64,
}

#[tauri::command]
async fn list_favorite_queries() -> Result<Vec<FavoriteDto>, String> {
    Ok(agent_tools::list_all_favorites_full()
        .into_iter()
        .map(|f| FavoriteDto {
            id: f.id,
            name: f.name,
            description: f.description,
            created_at_unix: f.created_at_unix,
        })
        .collect())
}

#[tauri::command]
async fn delete_favorite_query(id: String) -> Result<bool, String> {
    agent_tools::delete_favorite_by_id(&id)
}

#[tauri::command]
async fn execute_favorite_query(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<QueryResult, String> {
    let fav = agent_tools::get_favorite_sql(&id)
        .ok_or_else(|| "لم يُعثر على الاستعلام المحفوظ.".to_string())?;
    agent_tools::validate_read_only_sql(&fav.sql)?;

    let conn = {
        let guard = state.conn.lock().await;
        guard
            .clone()
            .ok_or_else(|| "لا يوجد اتصال نشط بقاعدة البيانات.".to_string())?
    };

    execute_sql_query(conn, fav.sql).await
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
            erp_kind: state.erp_kind.clone(),
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
    eprintln!(
        "[ask_local_ai] start request_id={} message_len={}",
        request_id,
        message.len()
    );

    let _agent_slot = app_state.inner().ai_request_lock.lock().await;

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    {
        let mut cancels = app_state.inner().ai_cancels.lock().await;
        cancels.insert(request_id.clone(), cancel_tx);
    }

    {
        let pf = crate::agent_tools::extract_product_hint_from_text(&message)
            .or_else(|| crate::agent_tools::extract_product_filter_from_history(&history));
        let mut session = app_state.inner().agent_session.lock().await;
        if pf.is_some() {
            session.last_product_filter = pf;
        }
    }

    let reports_cache = crate::telegram::fetch_reports().await;

    let secrets =
        supabase_config::resolve_app_secrets(&app_handle, decrypt_value, encrypt_value).await?;
    let groq_key = secrets.openrouter_api_key;
    let dec_openai_key = secrets.openai_api_key;

    if groq_key.trim().is_empty() {
        eprintln!("[ask_local_ai] ERROR: OpenRouter key is empty");
        return Err(
            "مفتاح OpenRouter غير متوفر. تحقق من اتصال الإنترنت أو راجع إعدادات المطوّر."
                .to_string(),
        );
    }
    eprintln!(
        "[ask_local_ai] OpenRouter key loaded (len={}) model={}",
        groq_key.len(),
        ai_model
    );

    let state_arc = Arc::new(AppState {
        conn: app_state.inner().conn.clone(),
        bot_cancel: app_state.inner().bot_cancel.clone(),
        ai_cancels: app_state.inner().ai_cancels.clone(),
        ai_request_lock: app_state.inner().ai_request_lock.clone(),
        agent_session: app_state.inner().agent_session.clone(),
        scheduler: app_state.inner().scheduler.clone(),
        erp_kind: app_state.inner().erp_kind.clone(),
    });

    let advanced_mode = app_handle
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("ai_advanced_mode").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    eprintln!("[ask_local_ai] advanced_mode={}", advanced_mode);

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        crate::ai_agent::handle_with_groq_local(
            &message,
            history,
            &groq_key,
            &ai_model,
            &state_arc,
            &reports_cache,
            app_handle.clone(),
            &dec_openai_key,
            Some(cancel_rx),
            advanced_mode,
        ),
    )
    .await;

    app_state.inner().ai_cancels.lock().await.remove(&request_id);

    let result = match result {
        Ok(inner) => inner,
        Err(_) => Err(
            "انتهت مهلة تحليل السؤال (5 دقائق). جرّب سؤالاً أبسط أو أعد المحاولة.".to_string(),
        ),
    };

    match result {
        Ok(mut text) => {
            eprintln!(
                "[ask_local_ai] success request_id={} response_len={}",
                request_id,
                text.len()
            );
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
        Err(e) => {
            eprintln!("[ask_local_ai] error request_id={}: {}", request_id, e);
            Err(e)
        }
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

pub(crate) fn build_config(conn: &SqlConnection) -> Config {
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
                erp_kind: Arc::new(Mutex::new(None)),
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

            // تأكيد أيقونة النافذة (Windows dev قد يعرض الافتراضية بدون rebuild كامل)
            if let Some(icon) = app.default_window_icon().cloned() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_icon(icon);
                }
            }

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
            get_erp_kind,
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
            list_favorite_queries,
            delete_favorite_query,
            execute_favorite_query,
            get_business_profile,
            list_cancelled_invoices,
            pos_sale::search_pos_products,
            pos_sale::print_pos_receipt,
            load_telegram_settings_local,
            get_pharmacy_share_settings,
            save_pharmacy_share_settings,
            sync_pharmacy_products_now,
            stop_pharmacy_sharing_cmd,
            preview_pharmacy_business_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
