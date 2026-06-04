use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_store::StoreExt;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::TokioAsyncWriteCompatExt;

pub mod agent_memory;
pub mod agent_tools;
pub mod ai_agent;
pub mod erp_adapters;
pub mod erp_profile;
pub mod excel_generator;
pub mod gotenberg;
pub mod pattern_catalog;
pub mod pdf_generator;
pub mod pharmacy_share;
pub mod pos_sale;
pub mod scheduler;
pub mod supabase_config;
pub mod telegram;

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
    /// تعطيل TLS — لـ SQL Server القديم الذي لا يدعم خوارزميات TLS الحديثة
    #[serde(default)]
    pub disable_encryption: bool,
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PrintableReportSection {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PrintableReportPayload {
    pub report_id: Option<u32>,
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub analysis: Option<String>,
    #[serde(default)]
    pub sections: Vec<PrintableReportSection>,
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
                updated_at: updated_i.and_then(|i| {
                    let v = cell(&row, i);
                    if v.is_empty() || v == "—" {
                        None
                    } else {
                        Some(v)
                    }
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
fn load_telegram_settings_local(app: tauri::AppHandle) -> Result<TelegramSettingsLocal, String> {
    let (bot_token, chat_id) =
        supabase_config::load_local_telegram_settings(&app, decrypt_value_internal)?;
    Ok(TelegramSettingsLocal { bot_token, chat_id })
}

// ─── مفتاح التشفير ────────────────────────────────────────────
const ENCRYPTION_KEY: &[u8; 32] = b"ReportsApp-SecureKey-2026-v1.0!!";

// ─── تشفير ────────────────────────────────────────────────────
pub(crate) fn encrypt_value_internal(value: String) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(ENCRYPTION_KEY).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(combined))
}

#[tauri::command]
fn encrypt_value(value: String) -> Result<String, String> {
    encrypt_value_internal(value)
}

// ─── فك تشفير ─────────────────────────────────────────────────
pub(crate) fn decrypt_value_internal(encrypted: String) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(ENCRYPTION_KEY).map_err(|e| e.to_string())?;
    let combined = BASE64.decode(encrypted).map_err(|e| e.to_string())?;
    if combined.len() < 12 {
        return Err("بيانات مشفرة غير صالحة".to_string());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "فشل فك التشفير".to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

#[tauri::command]
fn decrypt_value(encrypted: String) -> Result<String, String> {
    decrypt_value_internal(encrypted)
}

// ─── اختبار الاتصال ───────────────────────────────────────────
#[tauri::command]
async fn test_sql_connection(conn: SqlConnection) -> Result<ConnectionResult, String> {
    Ok(try_connect(conn).await)
}

async fn try_connect(conn: SqlConnection) -> ConnectionResult {
    let config = prepare_config(&conn);

    match TcpStream::connect(format!("{}:{}", conn.server, conn.port)).await {
        Ok(tcp) => match Client::connect(config, tcp.compat_write()).await {
            Ok(_client) => {
                let version = match execute_sql_query(
                    conn.clone(),
                    "SELECT @@VERSION AS version".to_string(),
                )
                .await
                {
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
async fn execute_sql_query(conn: SqlConnection, sql_query: String) -> Result<QueryResult, String> {
    let config = prepare_config(&conn);

    let tcp = TcpStream::connect(format!("{}:{}", conn.server, conn.port))
        .await
        .map_err(|e| format!("تعذّر الوصول للسيرفر: {}", e))?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("فشل الاتصال: {}", e))?;

    let result_sets = client
        .simple_query(&sql_query)
        .await
        .map_err(|e| format!("خطأ في تنفيذ الاستعلام: {}", e))?
        .into_results()
        .await
        .map_err(|e| format!("خطأ في قراءة النتائج: {}", e))?;

    let rows = result_sets
        .into_iter()
        .rev()
        .find(|rows| !rows.is_empty())
        .unwrap_or_default();

    Ok(query_result_from_rows(rows))
}

fn query_result_from_rows(rows: Vec<tiberius::Row>) -> QueryResult {
    if rows.is_empty() {
        return QueryResult {
            columns: vec![],
            rows: vec![],
            row_count: 0,
        };
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
    QueryResult {
        columns,
        rows: data,
        row_count,
    }
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

    let config = prepare_config(&conn);

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
        _ => {
            if q.is_empty() {
                "SELECT TOP 12 I.ITEM_NAME, I.ITEM_MODEL \
             FROM dbo.ITEMS I \
             WHERE I.ITEM_INVISIBLE = 0 \
             ORDER BY I.ITEM_UPDATE_DATE DESC"
                    .to_string()
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
            }
        }
    };

    let config = prepare_config(&conn);

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
    supabase_config::resolve_app_secrets(&app, decrypt_value_internal, encrypt_value_internal).await
}

#[tauri::command]
async fn save_app_secrets_settings(
    app: tauri::AppHandle,
    settings: supabase_config::AppSecretsSettings,
) -> Result<(), String> {
    if !settings.has_remote_payload() {
        return Err("أدخل مفتاح OpenRouter أو OpenAI.".to_string());
    }

    let access_token = supabase_config::read_stored_access_token(&app, decrypt_value_internal)?
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
    supabase_config::save_local_telegram_settings(
        &app,
        &bot_token,
        &chat_id,
        encrypt_value_internal,
    )?;
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
        supabase_config::resolve_app_secrets(&app, decrypt_value_internal, encrypt_value_internal)
            .await?;

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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap();
    let token = token.trim();
    let chat_id = chat_id.trim();

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let res = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": "تم اختبار الاتصال بنجاح من نظام المتمكن! ✅"
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
    chat_session_id: Option<String>,
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

    let secrets = supabase_config::resolve_app_secrets(
        &app_handle,
        decrypt_value_internal,
        encrypt_value_internal,
    )
    .await?;
    let groq_key = secrets.openrouter_api_key;
    let dec_openai_key = secrets.openai_api_key;

    if groq_key.trim().is_empty() {
        eprintln!("[ask_local_ai] ERROR: OpenRouter key is empty");
        return Err(
            "مفتاح OpenRouter غير متوفر. تحقق من اتصال الإنترنت أو راجع إعدادات المطوّر.".to_string(),
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
            &request_id,
            chat_session_id.as_deref(),
            &dec_openai_key,
            Some(cancel_rx),
            advanced_mode,
        ),
    )
    .await;

    app_state
        .inner()
        .ai_cancels
        .lock()
        .await
        .remove(&request_id);

    let result = match result {
        Ok(inner) => inner,
        Err(_) => {
            Err("انتهت مهلة تحليل السؤال (5 دقائق). جرّب سؤالاً أبسط أو أعد المحاولة.".to_string())
        }
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

fn escape_report_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn normalize_report_digits(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '٠' => '0',
            '١' => '1',
            '٢' => '2',
            '٣' => '3',
            '٤' => '4',
            '٥' => '5',
            '٦' => '6',
            '٧' => '7',
            '٨' => '8',
            '٩' => '9',
            _ => c,
        })
        .collect()
}

fn parse_report_number(value: &str) -> Option<f64> {
    let mut s = normalize_report_digits(value)
        .replace(',', "")
        .replace('٬', "")
        .replace('،', "")
        .replace('−', "-")
        .trim()
        .to_string();
    if s.is_empty() {
        return None;
    }
    let trailing_minus = s.ends_with('-');
    if trailing_minus {
        s.pop();
    }
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    if cleaned.is_empty() || cleaned == "-" || cleaned == "." {
        return None;
    }
    cleaned
        .parse::<f64>()
        .ok()
        .map(|n| if trailing_minus { -n.abs() } else { n })
}

fn format_report_number(value: f64) -> String {
    if (value.fract()).abs() < 0.005 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    }
}

fn find_report_column(columns: &[String], needles: &[&str]) -> Option<usize> {
    columns.iter().position(|column| {
        let normalized = column.trim().to_lowercase();
        needles
            .iter()
            .any(|needle| normalized.contains(&needle.to_lowercase()))
    })
}

fn sum_report_column(rows: &[Vec<String>], index: usize) -> f64 {
    rows.iter()
        .filter_map(|row| row.get(index))
        .filter_map(|value| parse_report_number(value))
        .sum()
}

fn report_cell_value(rows: &[Vec<String>], index: usize) -> Option<String> {
    rows.iter()
        .rev()
        .filter_map(|row| row.get(index))
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn report_id_from_title(title: &str) -> Option<String> {
    let normalized = normalize_report_digits(title);
    let mut current = String::new();
    let mut numbers = Vec::new();
    for ch in normalized.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            numbers.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        numbers.push(current);
    }
    numbers.into_iter().find(|n| n.len() >= 3)
}

fn metric_card(label: &str, value: &str) -> String {
    format!(
        r#"<div class="metric"><span>{}</span><strong>{}</strong></div>"#,
        escape_report_html(label),
        escape_report_html(value)
    )
}

fn metrics_from_balance_columns(
    columns: &[String],
    rows: &[Vec<String>],
) -> Option<Vec<(String, String)>> {
    let debit_idx = find_report_column(columns, &["المدين", "debit"]);
    let credit_idx = find_report_column(columns, &["الدائن", "credit"]);
    let balance_idx = find_report_column(columns, &["الرصيد", "balance"]);
    let balance_idx = balance_idx?;

    let mut metrics = Vec::new();
    if let Some(idx) = debit_idx {
        if let Some(value) = report_cell_value(rows, idx) {
            metrics.push(("المدين".to_string(), value));
        }
    }
    if let Some(idx) = credit_idx {
        if let Some(value) = report_cell_value(rows, idx) {
            metrics.push(("الدائن".to_string(), value));
        }
    }
    if let Some(value) = report_cell_value(rows, balance_idx) {
        metrics.push(("الرصيد".to_string(), value));
    }

    if metrics.is_empty() {
        None
    } else {
        Some(metrics)
    }
}

fn employee_sales_metrics(
    columns: &[String],
    rows: &[Vec<String>],
) -> Option<Vec<(String, String)>> {
    let employee_idx = find_report_column(columns, &["الموظف", "employee", "user_names"])?;
    let invoice_value_idx = find_report_column(
        columns,
        &["قيمة الفاتورة", "invoice value", "invoice_total"],
    )?;
    let employee_total_idx = find_report_column(columns, &["الإجمالي", "اجمالي", "total"]);

    let total_sales = sum_report_column(rows, invoice_value_idx);
    if total_sales.abs() <= 0.0 {
        return None;
    }
    let invoice_count = rows
        .iter()
        .filter(|row| {
            row.get(invoice_value_idx)
                .and_then(|value| parse_report_number(value))
                .map(|value| value.abs() > 0.0)
                .unwrap_or(false)
        })
        .count();

    let mut top_employee: Option<(String, f64)> = None;
    if let Some(total_idx) = employee_total_idx {
        for row in rows {
            let employee = row
                .get(employee_idx)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty());
            let total = row
                .get(total_idx)
                .and_then(|value| parse_report_number(value));
            if let (Some(employee), Some(total)) = (employee, total) {
                if top_employee
                    .as_ref()
                    .map(|(_, current)| total > *current)
                    .unwrap_or(true)
                {
                    top_employee = Some((employee.to_string(), total));
                }
            }
        }
    }

    let mut metrics = vec![
        (
            "إجمالي مبيعات الكل".to_string(),
            format_report_number(total_sales),
        ),
        ("عدد الفواتير".to_string(), invoice_count.to_string()),
    ];
    if let Some((employee, total)) = top_employee {
        metrics.push((
            "أعلى موظف".to_string(),
            format!("{} ({})", employee, format_report_number(total)),
        ));
    }
    Some(metrics)
}

fn employee_sales_insights(columns: &[String], rows: &[Vec<String>]) -> Vec<String> {
    let Some(employee_idx) = find_report_column(columns, &["الموظف", "employee", "user_names"])
    else {
        return Vec::new();
    };
    let Some(invoice_value_idx) = find_report_column(
        columns,
        &["قيمة الفاتورة", "invoice value", "invoice_total"],
    ) else {
        return Vec::new();
    };
    let employee_total_idx = find_report_column(columns, &["الإجمالي", "اجمالي", "total"]);
    let total_sales = sum_report_column(rows, invoice_value_idx);
    if total_sales.abs() <= 0.0 {
        return Vec::new();
    }
    let invoice_count = rows
        .iter()
        .filter(|row| {
            row.get(invoice_value_idx)
                .and_then(|value| parse_report_number(value))
                .map(|value| value.abs() > 0.0)
                .unwrap_or(false)
        })
        .count();
    let mut employee_totals: Vec<(String, f64)> = Vec::new();
    if let Some(total_idx) = employee_total_idx {
        for row in rows {
            let employee = row
                .get(employee_idx)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty());
            let total = row
                .get(total_idx)
                .and_then(|value| parse_report_number(value));
            if let (Some(employee), Some(total)) = (employee, total) {
                employee_totals.push((employee.to_string(), total));
            }
        }
    }
    employee_totals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut lines = vec![format!(
        "إجمالي مبيعات آخر يوم بلغ {} عبر {} فاتورة؛ هذا هو رقم اليوم الذي تُقاس عليه حركة الموظفين.",
        format_report_number(total_sales),
        invoice_count
    )];
    if !employee_totals.is_empty() {
        let contributions = employee_totals
            .iter()
            .take(6)
            .map(|(employee, total)| {
                let share = if total_sales.abs() > 0.0 {
                    total / total_sales * 100.0
                } else {
                    0.0
                };
                format!(
                    "{} ساهم بـ {} ({}%)",
                    employee,
                    format_report_number(*total),
                    format_report_number(share)
                )
            })
            .collect::<Vec<_>>()
            .join("، ");
        lines.push(format!(
            "توزيع مساهمة الموظفين في المبيعات: {}.",
            contributions
        ));
    }
    if let Some((employee, total)) = employee_totals.first() {
        let share = if total_sales.abs() > 0.0 {
            *total / total_sales * 100.0
        } else {
            0.0
        };
        lines.push(format!(
            "أعلى مساهمة كانت من {} بمبيعات {}، تعادل تقريباً {}% من إجمالي اليوم.",
            employee,
            format_report_number(*total),
            format_report_number(share)
        ));
        if share >= 45.0 {
            lines.push(
                "مهنياً، يوجد تركّز واضح في الإيراد لدى موظف واحد؛ يفضّل مقارنة ذلك بعدد الفواتير والوردية قبل تقييم الأداء النهائي.".to_string(),
            );
        } else {
            lines.push(
                "مهنياً، توزيع الإيراد بين الموظفين يبدو أكثر توازناً؛ تبقى المقارنة الأدق مرتبطة بعدد ساعات العمل والوردية.".to_string(),
            );
        }
    }
    lines
}

fn build_report_summary_cards(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
    generated_at: &str,
    extra_sources: &[(&[String], &[Vec<String>])],
) -> String {
    let mut metrics = metrics_from_balance_columns(columns, rows)
        .or_else(|| employee_sales_metrics(columns, rows))
        .or_else(|| {
            extra_sources
                .iter()
                .find_map(|(cols, source_rows)| metrics_from_balance_columns(cols, source_rows))
        })
        .unwrap_or_default();

    if metrics.is_empty() {
        if let Some(amount_idx) = find_report_column(
            columns,
            &["الصافي", "الإجمالي", "القيمة", "المبلغ", "total", "amount"],
        ) {
            let total = sum_report_column(rows, amount_idx);
            if total.abs() > 0.0 {
                metrics.push(("إجمالي القيمة".to_string(), format_report_number(total)));
            }
        }
        if let Some(qty_idx) = find_report_column(columns, &["الكمية", "qty", "quantity"]) {
            let total_qty = sum_report_column(rows, qty_idx);
            if total_qty.abs() > 0.0 {
                metrics.push(("إجمالي الكمية".to_string(), format_report_number(total_qty)));
            }
        }
        if let Some(days_idx) = find_report_column(columns, &["أيام", "days"]) {
            let values = rows
                .iter()
                .filter_map(|row| row.get(days_idx))
                .filter_map(|value| parse_report_number(value))
                .collect::<Vec<_>>();
            if !values.is_empty() {
                let expired = values.iter().filter(|value| **value <= 0.0).count();
                let min_days = values
                    .iter()
                    .fold(f64::INFINITY, |acc, value| acc.min(*value));
                metrics.push(("منتهية أو حرجة".to_string(), expired.to_string()));
                if min_days.is_finite() {
                    metrics.push((
                        "أقرب صلاحية".to_string(),
                        format!("{} يوم", format_report_number(min_days)),
                    ));
                }
            }
        }
    }

    if metrics.is_empty() {
        metrics.push(("تاريخ الإنشاء".to_string(), generated_at.to_string()));
        metrics.push(("حالة التقرير".to_string(), "جاهز للمراجعة".to_string()));
    }

    if metrics.len() > 3 {
        metrics.truncate(3);
    }
    while metrics.len() < 3 {
        if let Some(report_id) = report_id_from_title(title) {
            if !metrics.iter().any(|(label, _)| label == "رقم التقرير") {
                metrics.push(("رقم التقرير".to_string(), report_id));
                continue;
            }
        }
        if !metrics.iter().any(|(label, _)| label == "تاريخ الإنشاء") {
            metrics.push(("تاريخ الإنشاء".to_string(), generated_at.to_string()));
        } else {
            metrics.push(("جاهزية التقرير".to_string(), "مكتمل".to_string()));
        }
    }

    let cards = metrics
        .iter()
        .map(|(label, value)| metric_card(label, value))
        .collect::<Vec<_>>()
        .join("\n  ");

    format!(
        r#"<section class="summary">
  {}
</section>"#,
        cards
    )
}

fn build_almutamakken_insights(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
    model_analysis: Option<&str>,
) -> String {
    if rows.is_empty() || columns.is_empty() {
        return String::new();
    }

    let mut insights: Vec<String> = Vec::new();
    if let Some(analysis) = model_analysis.map(str::trim).filter(|v| !v.is_empty()) {
        insights.extend(
            analysis
                .lines()
                .map(|line| {
                    line.trim()
                        .trim_start_matches(['-', '•', '*', ' '])
                        .trim()
                        .to_string()
                })
                .filter(|line| {
                    !line.is_empty()
                        && !line.contains("ملخص تحليلي")
                        && !line.contains("التقرير الكامل")
                })
                .take(4),
        );
    }
    let visible_title = if title.trim().is_empty() {
        "التقرير"
    } else {
        title.trim()
    };
    if insights.is_empty() {
        insights.push(format!(
            "قراءة مهنية مختصرة لـ {} مع إبراز المؤشرات الأهم للمراجعة.",
            visible_title
        ));
    }

    for line in employee_sales_insights(columns, rows) {
        if !insights.iter().any(|existing| existing == &line) {
            insights.push(line);
        }
    }

    let debit_idx = find_report_column(columns, &["المدين", "debit"]);
    let credit_idx = find_report_column(columns, &["الدائن", "credit"]);
    let balance_idx = find_report_column(columns, &["الرصيد", "balance"]);
    if let Some(balance_idx) = balance_idx {
        let balance = rows
            .last()
            .and_then(|row| row.get(balance_idx))
            .and_then(|value| parse_report_number(value))
            .unwrap_or_else(|| sum_report_column(rows, balance_idx));
        let mut line = format!(
            "الرصيد الظاهر في التقرير: {}.",
            format_report_number(balance)
        );
        if let Some(debit_idx) = debit_idx {
            let debit = sum_report_column(rows, debit_idx);
            line.push_str(&format!(" إجمالي المدين: {}.", format_report_number(debit)));
        }
        if let Some(credit_idx) = credit_idx {
            let credit = sum_report_column(rows, credit_idx);
            line.push_str(&format!(
                " إجمالي الدائن/المدفوع: {}.",
                format_report_number(credit)
            ));
        }
        insights.push(line);
    }

    if let Some(amount_idx) =
        find_report_column(columns, &["إجمالي", "المبلغ", "القيمة", "total", "amount"])
    {
        let total = sum_report_column(rows, amount_idx);
        if total.abs() > 0.0 {
            insights.push(format!(
                "إجمالي القيمة المالية في هذا التقرير: {}.",
                format_report_number(total)
            ));
        }
    }

    if let Some(days_idx) = find_report_column(columns, &["أيام", "days"]) {
        let values = rows
            .iter()
            .filter_map(|row| row.get(days_idx))
            .filter_map(|value| parse_report_number(value))
            .collect::<Vec<_>>();
        if !values.is_empty() {
            let expired = values.iter().filter(|value| **value <= 0.0).count();
            let min_days = values
                .iter()
                .fold(f64::INFINITY, |acc, value| acc.min(*value));
            if expired > 0 {
                insights.push(format!(
                    "توجد {} أصناف منتهية أو بلا أيام متبقية؛ الأولوية لمعالجتها فوراً.",
                    expired
                ));
            } else if min_days.is_finite() {
                insights.push(format!(
                    "أقرب بند يحتاج متابعة بعد {} يوم.",
                    format_report_number(min_days)
                ));
            }
        }
    }

    if insights.len() > 4 {
        insights.truncate(4);
    }

    let items = insights
        .iter()
        .map(|item| format!("<li>{}</li>", escape_report_html(item)))
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<section class="insights">
  <h2>آراء المتمكن</h2>
  <ul>{}</ul>
</section>"#,
        items
    )
}

pub(crate) async fn resolve_report_business_name(app_state: &AppState) -> String {
    let conn_opt = app_state.conn.lock().await.clone();
    let Some(conn) = conn_opt else {
        return "نظام المتمكن".to_string();
    };
    let erp = erp_profile::resolve_erp_kind(app_state, &conn).await;
    match tokio::time::timeout(
        std::time::Duration::from_secs(4),
        erp_adapters::fetch_business_profile(&conn, erp),
    )
    .await
    {
        Ok(Ok(profile)) => profile
            .company_name
            .filter(|v| !v.trim().is_empty())
            .or(profile.activity_name.filter(|v| !v.trim().is_empty()))
            .unwrap_or_else(|| "نظام المتمكن".to_string()),
        _ => "نظام المتمكن".to_string(),
    }
}

pub(crate) fn html_report_document(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
    business_name: &str,
) -> String {
    html_report_document_with_analysis(title, columns, rows, business_name, None)
}

pub(crate) fn html_report_document_with_analysis(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
    business_name: &str,
    model_analysis: Option<&str>,
) -> String {
    html_report_document_with_sources(title, columns, rows, business_name, &[], model_analysis)
}

fn html_report_document_with_sources(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
    business_name: &str,
    extra_sources: &[(&[String], &[Vec<String>])],
    model_analysis: Option<&str>,
) -> String {
    let generated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let headers = columns
        .iter()
        .map(|c| format!("<th><span>{}</span></th>", escape_report_html(c)))
        .collect::<Vec<_>>()
        .join("");
    let body = rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_class = if row_index % 2 == 0 { "even" } else { "odd" };
            let row_number = row_index + 1;
            let cells = std::iter::once(format!("<td class=\"row-index\">{}</td>", row_number))
                .chain(columns.iter().enumerate().map(|(i, _)| {
                    format!(
                        "<td>{}</td>",
                        escape_report_html(row.get(i).map(String::as_str).unwrap_or(""))
                    )
                }))
                .collect::<Vec<_>>()
                .join("");
            format!("<tr class=\"{}\">{}</tr>", row_class, cells)
        })
        .collect::<Vec<_>>()
        .join("");
    let header_count = format!("<th class=\"row-index-head\">#</th>{}", headers);
    let visible_title = if title.trim().is_empty() {
        "تقرير"
    } else {
        title.trim()
    };
    let summary_cards =
        build_report_summary_cards(visible_title, columns, rows, &generated_at, extra_sources);
    let report_number_line = report_id_from_title(visible_title)
        .map(|id| format!("رقم التقرير: {}", id))
        .unwrap_or_else(|| "تقرير منسق للطباعة والمراجعة".to_string());
    let empty_state = if rows.is_empty() {
        "<div class=\"empty\">لا توجد بيانات للعرض.</div>".to_string()
    } else {
        String::new()
    };
    let insights = build_almutamakken_insights(visible_title, columns, rows, model_analysis);

    format!(
        r#"<!doctype html>
<html lang="ar" dir="rtl">
<head>
<meta charset="utf-8">
<style>
@page {{ size: A4 landscape; margin: 9mm; }}
* {{ box-sizing: border-box; }}
html {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
body {{
  margin: 0;
  font-family: Arial, Tahoma, "Segoe UI", sans-serif;
  direction: rtl;
  color: #172033;
  background: #f6f7fb;
}}
.page {{
  background: #ffffff;
  border: 1px solid #d9dfeb;
}}
.hero {{
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 18px;
  padding: 18px 22px;
  background: linear-gradient(135deg, #10243f 0%, #1d4f72 58%, #2b7a78 100%);
  color: #fff;
}}
.brand {{
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  font-weight: 700;
  opacity: .95;
}}
.mark {{
  width: 28px;
  height: 28px;
  border-radius: 8px;
  display: inline-grid;
  place-items: center;
  background: rgba(255,255,255,.18);
  border: 1px solid rgba(255,255,255,.28);
}}
h1 {{
  margin: 12px 0 0;
  font-size: 25px;
  line-height: 1.35;
  letter-spacing: 0;
}}
.subtitle {{
  margin-top: 6px;
  font-size: 12px;
  color: rgba(255,255,255,.78);
}}
.badge {{
  white-space: nowrap;
  border: 1px solid rgba(255,255,255,.32);
  background: rgba(255,255,255,.14);
  border-radius: 999px;
  padding: 7px 12px;
  font-size: 12px;
  font-weight: 700;
}}
.summary {{
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
  padding: 14px 22px 4px;
}}
.metric {{
  border: 1px solid #dde5f1;
  background: #f8fafc;
  border-radius: 10px;
  padding: 10px 12px;
}}
.metric span {{
  display: block;
  color: #607086;
  font-size: 11px;
  margin-bottom: 5px;
}}
.metric strong {{
  color: #10243f;
  font-size: 15px;
}}
.table-wrap {{
  padding: 14px 22px 18px;
}}
table {{
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: auto;
  border: 1px solid #cfd8e7;
  border-radius: 10px;
  overflow: hidden;
  page-break-inside: auto;
}}
thead {{ display: table-header-group; }}
tfoot {{ display: table-footer-group; }}
tr {{ page-break-inside: avoid; break-inside: avoid; }}
thead th {{
  background: #eaf0f7;
  color: #10243f;
  font-weight: 800;
  text-align: center;
  border-bottom: 1px solid #c5d0df;
  padding: 8px 7px;
  font-size: 11.5px;
  vertical-align: middle;
}}
tbody td {{
  color: #182338;
  border-bottom: 1px solid #e2e8f0;
  border-left: 1px solid #e2e8f0;
  padding: 7px 7px;
  font-size: 10.7px;
  line-height: 1.45;
  vertical-align: top;
  overflow-wrap: anywhere;
}}
tbody tr.even td {{ background: #ffffff; }}
tbody tr.odd td {{ background: #f8fbff; }}
tbody tr:last-child td {{ border-bottom: 0; }}
.row-index, .row-index-head {{
  width: 34px;
  min-width: 34px;
  max-width: 34px;
  text-align: center;
  color: #617087;
  font-weight: 700;
}}
.empty {{
  margin: 18px 22px;
  border: 1px dashed #b8c4d6;
  border-radius: 10px;
  padding: 18px;
  text-align: center;
  color: #607086;
}}
.insights {{
  margin: 0 22px 18px;
  border: 1px solid #c8d8e8;
  border-right: 5px solid #2b7a78;
  border-radius: 12px;
  background: #f7fbfc;
  padding: 12px 14px;
  page-break-inside: avoid;
}}
.insights h2 {{
  margin: 0 0 8px;
  color: #10243f;
  font-size: 15px;
  line-height: 1.35;
}}
.insights ul {{
  margin: 0;
  padding: 0 18px 0 0;
}}
.insights li {{
  margin: 5px 0;
  color: #26364d;
  font-size: 11.5px;
  line-height: 1.65;
}}
.footer {{
  display: flex;
  justify-content: space-between;
  gap: 12px;
  border-top: 1px solid #e2e8f0;
  padding: 10px 22px 14px;
  color: #64748b;
  font-size: 10px;
}}
</style>
</head>
<body>
<main class="page">
  <section class="hero">
    <div>
      <div class="brand"><span class="mark">R</span><span>نظام المتمكن</span></div>
      <h1>{}</h1>
      <div class="subtitle">{}</div>
    </div>
    <div class="badge">{}</div>
  </section>
  {}
  {}
  <section class="table-wrap">
    <table>
      <thead><tr>{}</tr></thead>
      <tbody>{}</tbody>
    </table>
  </section>
  {}
  <footer class="footer">
    <span>نظام المتمكن</span>
    <span>تم الإنشاء: {}</span>
  </footer>
</main>
</body>
</html>"#,
        escape_report_html(visible_title),
        escape_report_html(&report_number_line),
        escape_report_html(if business_name.trim().is_empty() {
            "نظام المتمكن"
        } else {
            business_name.trim()
        }),
        summary_cards,
        empty_state,
        header_count,
        body,
        insights,
        escape_report_html(&generated_at)
    )
}

fn render_report_table_html(columns: &[String], rows: &[Vec<String>]) -> String {
    let headers = columns
        .iter()
        .map(|c| format!("<th><span>{}</span></th>", escape_report_html(c)))
        .collect::<Vec<_>>()
        .join("");
    let body = rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_class = if row_index % 2 == 0 { "even" } else { "odd" };
            let row_number = row_index + 1;
            let cells = std::iter::once(format!("<td class=\"row-index\">{}</td>", row_number))
                .chain(columns.iter().enumerate().map(|(i, _)| {
                    format!(
                        "<td>{}</td>",
                        escape_report_html(row.get(i).map(String::as_str).unwrap_or(""))
                    )
                }))
                .collect::<Vec<_>>()
                .join("");
            format!("<tr class=\"{}\">{}</tr>", row_class, cells)
        })
        .collect::<Vec<_>>()
        .join("");
    let header_count = format!("<th class=\"row-index-head\">#</th>{}", headers);
    format!(
        r#"<table>
      <thead><tr>{}</tr></thead>
      <tbody>{}</tbody>
    </table>"#,
        header_count, body
    )
}

fn html_report_document_with_sections(
    title: &str,
    sections: &[PrintableReportSection],
    business_name: &str,
    model_analysis: Option<&str>,
) -> String {
    let generated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let visible_title = if title.trim().is_empty() {
        "تقرير"
    } else {
        title.trim()
    };
    let primary = sections.first();
    let empty_columns: Vec<String> = Vec::new();
    let empty_rows: Vec<Vec<String>> = Vec::new();
    let primary_columns = primary
        .map(|s| s.columns.as_slice())
        .unwrap_or(&empty_columns);
    let primary_rows = primary.map(|s| s.rows.as_slice()).unwrap_or(&empty_rows);
    let extra_sources = sections
        .iter()
        .skip(1)
        .map(|section| (section.columns.as_slice(), section.rows.as_slice()))
        .collect::<Vec<_>>();
    let summary_cards = build_report_summary_cards(
        visible_title,
        primary_columns,
        primary_rows,
        &generated_at,
        &extra_sources,
    );
    let report_number_line = report_id_from_title(visible_title)
        .map(|id| format!("رقم التقرير: {}", id))
        .unwrap_or_else(|| "تقرير منسق للطباعة والمراجعة".to_string());
    let table_sections = if sections.is_empty() {
        "<div class=\"empty\">لا توجد بيانات للعرض.</div>".to_string()
    } else {
        sections
            .iter()
            .enumerate()
            .map(|(idx, section)| {
                let title = if section.title.trim().is_empty() {
                    format!("القسم {}", idx + 1)
                } else {
                    section.title.trim().to_string()
                };
                format!(
                    r#"<section class="table-wrap report-section">
    <h2>{}</h2>
    {}
  </section>"#,
                    escape_report_html(&title),
                    render_report_table_html(&section.columns, &section.rows)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let insights =
        build_almutamakken_insights(visible_title, primary_columns, primary_rows, model_analysis);

    format!(
        r#"<!doctype html>
<html lang="ar" dir="rtl">
<head>
<meta charset="utf-8">
<style>
@page {{ size: A4 landscape; margin: 9mm; }}
* {{ box-sizing: border-box; }}
html {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
body {{
  margin: 0;
  font-family: Arial, Tahoma, "Segoe UI", sans-serif;
  direction: rtl;
  color: #172033;
  background: #f6f7fb;
}}
.page {{ background: #ffffff; border: 1px solid #d9dfeb; }}
.hero {{
  display: flex; justify-content: space-between; align-items: flex-start; gap: 18px;
  padding: 18px 22px; background: linear-gradient(135deg, #10243f 0%, #1d4f72 58%, #2b7a78 100%); color: #fff;
}}
.brand {{ display: flex; align-items: center; gap: 10px; font-size: 13px; font-weight: 700; opacity: .95; }}
.mark {{ width: 28px; height: 28px; border-radius: 8px; display: inline-grid; place-items: center; background: rgba(255,255,255,.18); border: 1px solid rgba(255,255,255,.28); }}
h1 {{ margin: 12px 0 0; font-size: 25px; line-height: 1.35; letter-spacing: 0; }}
.subtitle {{ margin-top: 6px; font-size: 12px; color: rgba(255,255,255,.78); }}
.badge {{ white-space: nowrap; border: 1px solid rgba(255,255,255,.32); background: rgba(255,255,255,.14); border-radius: 999px; padding: 7px 12px; font-size: 12px; font-weight: 700; }}
.summary {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; padding: 14px 22px 4px; }}
.metric {{ border: 1px solid #dde5f1; background: #f8fafc; border-radius: 10px; padding: 10px 12px; }}
.metric span {{ display: block; color: #607086; font-size: 11px; margin-bottom: 5px; }}
.metric strong {{ color: #10243f; font-size: 15px; }}
.table-wrap {{ padding: 14px 22px 18px; }}
.report-section {{ break-inside: avoid; page-break-inside: avoid; }}
.report-section h2 {{ margin: 0 0 10px; font-size: 15px; color: #10243f; }}
table {{ width: 100%; border-collapse: separate; border-spacing: 0; table-layout: auto; border: 1px solid #cfd8e7; border-radius: 10px; overflow: hidden; page-break-inside: auto; }}
thead {{ display: table-header-group; }}
tr {{ page-break-inside: avoid; break-inside: avoid; }}
thead th {{ background: #eaf0f7; color: #10243f; font-weight: 800; text-align: center; border-bottom: 1px solid #c5d0df; padding: 8px 7px; font-size: 11.5px; vertical-align: middle; }}
tbody td {{ color: #182338; border-bottom: 1px solid #e2e8f0; border-left: 1px solid #e2e8f0; padding: 7px 7px; font-size: 10.7px; line-height: 1.45; vertical-align: top; overflow-wrap: anywhere; }}
tbody tr.even td {{ background: #ffffff; }}
tbody tr.odd td {{ background: #f8fbff; }}
tbody tr:last-child td {{ border-bottom: 0; }}
.row-index, .row-index-head {{ width: 34px; min-width: 34px; max-width: 34px; text-align: center; color: #617087; font-weight: 700; }}
.empty {{ margin: 18px 22px; border: 1px dashed #b8c4d6; border-radius: 10px; padding: 18px; text-align: center; color: #607086; }}
.insights {{ margin: 0 22px 18px; border: 1px solid #c8d8e8; border-right: 5px solid #2b7a78; border-radius: 12px; background: #f7fbfc; padding: 12px 14px; page-break-inside: avoid; }}
.insights h2 {{ margin: 0 0 8px; color: #10243f; font-size: 15px; line-height: 1.35; }}
.insights ul {{ margin: 0; padding: 0 18px 0 0; }}
.insights li {{ margin: 5px 0; color: #26364d; font-size: 11.5px; line-height: 1.65; }}
.footer {{ display: flex; justify-content: space-between; gap: 12px; border-top: 1px solid #e2e8f0; padding: 10px 22px 14px; color: #64748b; font-size: 10px; }}
</style>
</head>
<body>
<main class="page">
  <section class="hero">
    <div>
      <div class="brand"><span class="mark">R</span><span>نظام المتمكن</span></div>
      <h1>{}</h1>
      <div class="subtitle">{}</div>
    </div>
    <div class="badge">{}</div>
  </section>
  {}
  {}
  {}
  <footer class="footer">
    <span>نظام المتمكن</span>
    <span>تم الإنشاء: {}</span>
  </footer>
</main>
</body>
</html>"#,
        escape_report_html(visible_title),
        escape_report_html(&report_number_line),
        escape_report_html(if business_name.trim().is_empty() {
            "نظام المتمكن"
        } else {
            business_name.trim()
        }),
        summary_cards,
        table_sections,
        insights,
        escape_report_html(&generated_at)
    )
}

fn safe_report_filename(title: &str, prefix: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let safe_title = title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(40)
        .collect::<String>();
    if safe_title.is_empty() {
        format!("{}_{}.pdf", prefix, stamp)
    } else {
        format!("{}_{}_{}.pdf", prefix, safe_title, stamp)
    }
}

fn html_ai_response_document(title: &str, content_html: &str) -> String {
    let generated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let visible_title = if title.trim().is_empty() {
        "تقرير"
    } else {
        title.trim()
    };
    format!(
        r#"<!doctype html>
<html lang="ar" dir="rtl">
<head>
<meta charset="utf-8">
<style>
@page {{ size: A4 landscape; margin: 9mm; }}
* {{ box-sizing: border-box; }}
html {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
body {{
  margin: 0;
  font-family: Arial, Tahoma, "Segoe UI", sans-serif;
  direction: rtl;
  color: #172033;
  background: #f6f7fb;
}}
.page {{
  min-height: 100vh;
  background: #fff;
  border: 1px solid #d9dfeb;
}}
.hero {{
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 18px;
  padding: 18px 22px;
  background: linear-gradient(135deg, #10243f 0%, #1d4f72 58%, #2b7a78 100%);
  color: #fff;
}}
.brand {{
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  font-weight: 700;
}}
.mark {{
  width: 28px;
  height: 28px;
  border-radius: 8px;
  display: inline-grid;
  place-items: center;
  background: rgba(255,255,255,.18);
  border: 1px solid rgba(255,255,255,.28);
}}
h1 {{
  margin: 12px 0 0;
  font-size: 25px;
  line-height: 1.35;
  letter-spacing: 0;
}}
.subtitle {{
  margin-top: 6px;
  font-size: 12px;
  color: rgba(255,255,255,.78);
}}
.badge {{
  white-space: nowrap;
  border: 1px solid rgba(255,255,255,.32);
  background: rgba(255,255,255,.14);
  border-radius: 999px;
  padding: 7px 12px;
  font-size: 12px;
  font-weight: 700;
}}
.content {{
  padding: 18px 22px 20px;
  font-size: 12.5px;
  line-height: 1.75;
}}
.content > *:first-child {{ margin-top: 0; }}
.content p {{ margin: 8px 0; }}
.content h1, .content h2, .content h3 {{
  color: #10243f;
  margin: 14px 0 8px;
  line-height: 1.35;
}}
.content h1 {{ font-size: 20px; }}
.content h2 {{ font-size: 17px; }}
.content h3 {{ font-size: 15px; }}
.content ul, .content ol {{ margin: 8px 20px; padding: 0; }}
.content li {{ margin: 4px 0; }}
.content strong {{ color: #10243f; }}
.content code {{
  background: #eef3f8;
  border: 1px solid #d9e3ef;
  border-radius: 6px;
  padding: 1px 5px;
  font-family: Consolas, monospace;
  font-size: 11px;
}}
.content pre {{
  background: #111827;
  color: #f8fafc;
  border-radius: 10px;
  padding: 12px;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  direction: ltr;
  text-align: left;
}}
.content [data-printable-table] {{
  margin: 12px 0 14px;
  border: 0 !important;
  box-shadow: none !important;
}}
.content [data-print-control] {{ display: none !important; }}
.content table {{
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: auto;
  border: 1px solid #cfd8e7;
  border-radius: 10px;
  overflow: hidden;
  margin: 10px 0 12px;
}}
.content thead th {{
  background: #eaf0f7;
  color: #10243f;
  font-weight: 800;
  text-align: center;
  border-bottom: 1px solid #c5d0df;
  padding: 8px 7px;
  font-size: 11.5px;
  vertical-align: middle;
}}
.content tbody td {{
  color: #182338;
  border-bottom: 1px solid #e2e8f0;
  border-left: 1px solid #e2e8f0;
  padding: 7px 7px;
  font-size: 10.7px;
  line-height: 1.45;
  vertical-align: top;
  overflow-wrap: anywhere;
}}
.content tbody tr:nth-child(odd) td {{ background: #fff; }}
.content tbody tr:nth-child(even) td {{ background: #f8fbff; }}
.footer {{
  display: flex;
  justify-content: space-between;
  gap: 12px;
  border-top: 1px solid #e2e8f0;
  padding: 10px 22px 14px;
  color: #64748b;
  font-size: 10px;
}}
</style>
</head>
<body>
<main class="page">
  <section class="hero">
    <div>
      <div class="brand"><span class="mark">R</span><span>نظام المتمكن</span></div>
      <h1>{}</h1>
      <div class="subtitle">محتوى رد الوكيل الذكي مع الجداول والملاحظات</div>
    </div>
    <div class="badge">نظام المتمكن</div>
  </section>
  <section class="content">{}</section>
  <footer class="footer">
    <span>نظام المتمكن</span>
    <span>تم الإنشاء: {}</span>
  </footer>
</main>
</body>
</html>"#,
        escape_report_html(visible_title),
        content_html,
        escape_report_html(&generated_at)
    )
}

#[tauri::command]
async fn print_html_report_with_gotenberg(
    title: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    if columns.is_empty() {
        return Err("لا توجد أعمدة للطباعة.".to_string());
    }
    let business_name = resolve_report_business_name(&state).await;
    let html = html_report_document(&title, &columns, &rows, &business_name);
    let pdf_bytes = gotenberg::html_to_pdf(&html).await?;
    let filename = safe_report_filename(&title, "html_report");
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, pdf_bytes).map_err(|e| format!("فشل حفظ التقرير: {}", e))?;
    Ok(path.display().to_string())
}

#[tauri::command]
async fn print_ai_response_with_gotenberg(
    title: String,
    content_html: String,
) -> Result<String, String> {
    if content_html.trim().is_empty() {
        return Err("لا يوجد محتوى للطباعة.".to_string());
    }
    let html = html_ai_response_document(&title, &content_html);
    let pdf_bytes = gotenberg::html_to_pdf(&html).await?;
    let filename = safe_report_filename(&title, "ai_report");
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, pdf_bytes).map_err(|e| format!("فشل حفظ التقرير: {}", e))?;
    Ok(path.display().to_string())
}

#[tauri::command]
async fn print_ai_response_bundle_with_gotenberg(
    title: String,
    content_html: String,
    mut reports: Vec<PrintableReportPayload>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let recent_results = {
        let session = state.agent_session.lock().await;
        session.recent_reports.clone()
    };
    for result in recent_results.into_iter().rev().take(6).rev() {
        let already_has_full_result = reports.iter().any(|report| {
            report.columns == result.columns && report.rows.len() >= result.rows.len()
        });
        if !already_has_full_result && !result.columns.is_empty() && !result.rows.is_empty() {
            reports.push(PrintableReportPayload {
                report_id: Some(result.report_id),
                title: if title.trim().is_empty() {
                    "البيانات الكاملة".to_string()
                } else {
                    format!("{} - البيانات الكاملة", title.trim())
                },
                columns: result.columns,
                rows: result.rows,
                analysis: result.analysis,
                sections: Vec::new(),
            });
        }
    }

    if reports.is_empty() {
        let _ = content_html;
        return Err("لا يوجد محتوى للطباعة.".to_string());
    }
    let selected = reports
        .iter()
        .max_by_key(|report| {
            report
                .sections
                .iter()
                .map(|section| section.rows.len())
                .sum::<usize>()
                .max(report.rows.len())
        })
        .ok_or_else(|| "لا يوجد تقرير كامل للطباعة.".to_string())?;
    let report_title = if let Some(id) = selected.report_id {
        format!("تقرير رقم {}", id)
    } else if selected.title.trim().is_empty() {
        title.clone()
    } else {
        selected.title.clone()
    };
    let business_name = resolve_report_business_name(&state).await;
    if !selected.sections.is_empty() {
        let mut sections = selected.sections.clone();
        if sections.is_empty() && !selected.columns.is_empty() {
            sections.push(PrintableReportSection {
                title: selected.title.clone(),
                columns: selected.columns.clone(),
                rows: selected.rows.clone(),
            });
        }
        let html = html_report_document_with_sections(
            &report_title,
            &sections,
            &business_name,
            selected.analysis.as_deref(),
        );
        let pdf_bytes = gotenberg::html_to_pdf(&html).await?;
        let filename = safe_report_filename(&report_title, "report");
        let path = std::env::temp_dir().join(filename);
        std::fs::write(&path, pdf_bytes).map_err(|e| format!("فشل حفظ التقرير: {}", e))?;
        return Ok(path.display().to_string());
    }
    let extra_sources = reports
        .iter()
        .filter(|report| {
            !(report.columns == selected.columns && report.rows.len() == selected.rows.len())
        })
        .map(|report| (report.columns.as_slice(), report.rows.as_slice()))
        .collect::<Vec<_>>();
    let html = html_report_document_with_sources(
        &report_title,
        &selected.columns,
        &selected.rows,
        &business_name,
        &extra_sources,
        selected.analysis.as_deref(),
    );
    let pdf_bytes = gotenberg::html_to_pdf(&html).await?;
    let filename = safe_report_filename(&report_title, "report");
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, pdf_bytes).map_err(|e| format!("فشل حفظ التقرير: {}", e))?;
    Ok(path.display().to_string())
}

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

pub(crate) fn prepare_config(conn: &SqlConnection) -> Config {
    let mut config = build_config(conn);
    if conn.disable_encryption {
        config.encryption(EncryptionLevel::NotSupported);
    } else {
        config.trust_cert();
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
        return if v {
            "نعم".to_string()
        } else {
            "لا".to_string()
        };
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
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
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
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
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
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
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
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
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
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let mut state = app_state.scheduler.lock().await;
    state.notifications.clear();
    scheduler::save_notifications(&data_dir, &state.notifications);
    Ok(())
}

fn read_app_access_token(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(
        supabase_config::read_stored_access_token(app, decrypt_value_internal)?
            .unwrap_or_else(|| supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string()),
    )
}

async fn call_supabase_rpc_value(
    rpc_name: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    let url = format!("{}/rest/v1/rpc/{}", supabase_config::SUPABASE_URL, rpc_name);
    let res = client
        .post(&url)
        .header("apikey", supabase_config::SUPABASE_ANON_KEY)
        .header(
            "Authorization",
            format!("Bearer {}", supabase_config::SUPABASE_ANON_KEY),
        )
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Supabase {} error: {}", rpc_name, body));
    }
    res.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Supabase {} parse error: {}", rpc_name, e))
}

fn chat_message_row_to_ui_message(row: serde_json::Value) -> Option<serde_json::Value> {
    let role = row.get("role").and_then(|v| v.as_str())?;
    if !matches!(role, "user" | "assistant" | "system") {
        return None;
    }
    let mut message = serde_json::json!({
        "role": role,
        "content": row.get("content").and_then(|v| v.as_str()).unwrap_or(""),
    });
    let total_tokens = row
        .get("total_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if role == "assistant" && total_tokens > 0 {
        message["aiUsage"] = serde_json::json!({
            "model": crate::ai_agent::DEFAULT_AI_MODEL,
            "promptTokens": row.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            "completionTokens": row.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
            "totalTokens": total_tokens,
            "usageSource": row.get("usage_source").and_then(|v| v.as_str()).unwrap_or("supabase"),
        });
    }
    if let Some(reports) = row
        .get("metadata")
        .and_then(|m| m.get("reports"))
        .filter(|v| v.is_array())
    {
        message["reports"] = reports.clone();
    }
    Some(message)
}

#[tauri::command]
async fn sync_chat_to_supabase(
    chat_id: String,
    title: String,
    messages: serde_json::Value,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let _ = messages;
    let access_token = read_app_access_token(&app)?;
    call_supabase_rpc_value(
        "upsert_chat_session",
        serde_json::json!({
            "p_access_token": access_token.trim(),
            "p_session_id": chat_id.trim(),
            "p_title": title.trim(),
            "p_summary": serde_json::Value::Null,
        }),
    )
    .await
    .map(|_| ())
}

#[tauri::command]
async fn append_chat_message_to_supabase(
    chat_id: String,
    role: String,
    content: String,
    turn_index: Option<i32>,
    tool_used: Option<String>,
    pattern_id: Option<String>,
    success: Option<bool>,
    error_text: Option<String>,
    row_count: Option<i32>,
    report_number: Option<i64>,
    prompt_tokens: Option<i32>,
    completion_tokens: Option<i32>,
    total_tokens: Option<i32>,
    usage_source: Option<String>,
    metadata: Option<serde_json::Value>,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let access_token = read_app_access_token(&app)?;
    call_supabase_rpc_value(
        "append_chat_message",
        serde_json::json!({
            "p_access_token": access_token.trim(),
            "p_session_id": chat_id.trim(),
            "p_role": role.trim(),
            "p_content": content,
            "p_turn_index": turn_index,
            "p_tool_used": tool_used,
            "p_pattern_id": pattern_id,
            "p_sql_text": serde_json::Value::Null,
            "p_success": success,
            "p_error_text": error_text,
            "p_row_count": row_count,
            "p_report_number": report_number,
            "p_prompt_tokens": prompt_tokens,
            "p_completion_tokens": completion_tokens,
            "p_total_tokens": total_tokens,
            "p_usage_source": usage_source,
            "p_metadata": metadata.unwrap_or_else(|| serde_json::json!({})),
        }),
    )
    .await
}

#[tauri::command]
async fn fetch_chats_from_supabase(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let access_token = read_app_access_token(&app)?;
    let _ = call_supabase_rpc_value(
        "migrate_user_chats_to_messages",
        serde_json::json!({ "p_access_token": access_token.trim() }),
    )
    .await;

    let sessions = call_supabase_rpc_value(
        "get_chat_sessions",
        serde_json::json!({ "p_access_token": access_token.trim() }),
    )
    .await?;
    let Some(session_rows) = sessions.as_array() else {
        return Ok(serde_json::json!([]));
    };

    let mut out = Vec::new();
    for session in session_rows {
        let chat_id = session
            .get("chat_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if chat_id.trim().is_empty() {
            continue;
        }
        let messages_value = call_supabase_rpc_value(
            "get_session_messages",
            serde_json::json!({
                "p_access_token": access_token.trim(),
                "p_session_id": chat_id,
                "p_limit": 250,
            }),
        )
        .await
        .unwrap_or_else(|_| serde_json::json!([]));
        let messages = messages_value
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(chat_message_row_to_ui_message)
            .collect::<Vec<_>>();

        out.push(serde_json::json!({
            "chat_id": chat_id,
            "title": session.get("title").and_then(|v| v.as_str()).unwrap_or("محادثة"),
            "summary": session.get("summary").and_then(|v| v.as_str()).unwrap_or(""),
            "messages": messages,
            "updated_at": session.get("updated_at").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }

    Ok(serde_json::Value::Array(out))
}

#[tauri::command]
async fn delete_chat_from_supabase(chat_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let access_token = read_app_access_token(&app)?;
    call_supabase_rpc_value(
        "delete_chat_session",
        serde_json::json!({
            "p_access_token": access_token.trim(),
            "p_session_id": chat_id.trim(),
        }),
    )
    .await?;
    let _ = call_supabase_rpc_value(
        "delete_user_chat",
        serde_json::json!({
            "p_access_token": access_token.trim(),
            "p_chat_id": chat_id.trim(),
        }),
    )
    .await;
    Ok(())
}

#[tauri::command]
#[allow(dead_code)]
async fn sync_chat_to_supabase_legacy(
    chat_id: String,
    title: String,
    messages: serde_json::Value,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let access_token = supabase_config::read_stored_access_token(&app, decrypt_value_internal)?
        .unwrap_or_else(|| supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let url = format!(
        "{}/rest/v1/rpc/upsert_user_chat",
        supabase_config::SUPABASE_URL
    );
    let payload = serde_json::json!({
        "p_access_token": access_token.trim(),
        "p_chat_id": chat_id.trim(),
        "p_title": title.trim(),
        "p_messages": messages,
    });

    let res = client
        .post(&url)
        .header("apikey", supabase_config::SUPABASE_ANON_KEY)
        .header(
            "Authorization",
            format!("Bearer {}", supabase_config::SUPABASE_ANON_KEY),
        )
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Supabase upsert_user_chat error: {}", body));
    }

    Ok(())
}

#[tauri::command]
#[allow(dead_code)]
async fn fetch_chats_from_supabase_legacy(
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let access_token = supabase_config::read_stored_access_token(&app, decrypt_value_internal)?
        .unwrap_or_else(|| supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let url = format!(
        "{}/rest/v1/rpc/get_user_chats",
        supabase_config::SUPABASE_URL
    );
    let payload = serde_json::json!({
        "p_access_token": access_token.trim(),
    });

    let res = client
        .post(&url)
        .header("apikey", supabase_config::SUPABASE_ANON_KEY)
        .header(
            "Authorization",
            format!("Bearer {}", supabase_config::SUPABASE_ANON_KEY),
        )
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Supabase get_user_chats error: {}", body));
    }

    let value: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(value)
}

#[tauri::command]
#[allow(dead_code)]
async fn delete_chat_from_supabase_legacy(
    chat_id: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let access_token = supabase_config::read_stored_access_token(&app, decrypt_value_internal)?
        .unwrap_or_else(|| supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let url = format!(
        "{}/rest/v1/rpc/delete_user_chat",
        supabase_config::SUPABASE_URL
    );
    let payload = serde_json::json!({
        "p_access_token": access_token.trim(),
        "p_chat_id": chat_id.trim(),
    });

    let res = client
        .post(&url)
        .header("apikey", supabase_config::SUPABASE_ANON_KEY)
        .header(
            "Authorization",
            format!("Bearer {}", supabase_config::SUPABASE_ANON_KEY),
        )
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Supabase delete_user_chat error: {}", body));
    }

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
            let data_dir = app
                .handle()
                .path()
                .app_data_dir()
                .expect("تعذّر الحصول على مجلد بيانات التطبيق");
            let initial_state = scheduler::load_state(&data_dir);
            let shared_scheduler: scheduler::SharedScheduler = Arc::new(Mutex::new(initial_state));

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
                )
                .await;
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
            print_html_report_with_gotenberg,
            print_ai_response_with_gotenberg,
            print_ai_response_bundle_with_gotenberg,
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
            sync_chat_to_supabase,
            append_chat_message_to_supabase,
            fetch_chats_from_supabase,
            delete_chat_from_supabase,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod report_render_tests {
    use super::*;

    #[test]
    fn employee_sales_cards_show_total_all_sales() {
        let columns = vec![
            "الموظف".to_string(),
            "رقم الفاتورة".to_string(),
            "قيمة الفاتورة".to_string(),
            "الإجمالي".to_string(),
        ];
        let rows = vec![
            vec![
                "عائشة".into(),
                "10".into(),
                "973.50".into(),
                "973.50".into(),
            ],
            vec!["ليلى".into(), "11".into(), "846.00".into(), "846.00".into()],
            vec![
                "موسى".into(),
                "12".into(),
                "1707.50".into(),
                "1707.50".into(),
            ],
        ];
        let html = build_report_summary_cards("تقرير رقم 1000", &columns, &rows, "2026-06-04", &[]);
        assert!(html.contains("إجمالي مبيعات الكل"));
        assert!(html.contains("3527"));
        assert!(html.contains("أعلى موظف"));
    }

    #[test]
    fn employee_sales_insights_include_accounting_flavor() {
        let columns = vec![
            "الموظف".to_string(),
            "رقم الفاتورة".to_string(),
            "قيمة الفاتورة".to_string(),
            "الإجمالي".to_string(),
        ];
        let rows = vec![
            vec![
                "عائشة".into(),
                "10".into(),
                "973.50".into(),
                "973.50".into(),
            ],
            vec!["ليلى".into(), "11".into(), "846.00".into(), "846.00".into()],
            vec![
                "موسى".into(),
                "12".into(),
                "1707.50".into(),
                "1707.50".into(),
            ],
        ];
        let insights = employee_sales_insights(&columns, &rows);
        assert!(insights
            .iter()
            .any(|line| line.contains("إجمالي مبيعات آخر يوم")));
        assert!(insights.iter().any(|line| line.contains("موسى")));
        assert!(insights
            .iter()
            .any(|line| line.contains("عائشة ساهم") && line.contains("ليلى ساهم")));
    }
}
