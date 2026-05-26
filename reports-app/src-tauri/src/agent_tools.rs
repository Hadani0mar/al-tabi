//! أدوات الوكيل المتقدمة — SQL، أنماط، مفضلة، تصدير آخر نتيجة

use crate::excel_generator::generate_report_excel;
use crate::pdf_generator::generate_report_pdf;
use crate::telegram::{send_excel, send_pdf};
use crate::{execute_sql_query, AppState};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

// ─── حالة الجلسة (آخر نتيجة) ───────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct StoredQueryResult {
    pub sql: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub saved_at_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryPlanStep {
    pub step_id: u32,
    pub title: String,
    pub purpose: String,
    pub recommended_tool: String,
    pub pattern_keywords: Option<String>,
    pub sql_query: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryPlan {
    pub question: String,
    pub product_filter: Option<String>,
    pub plan_type: String,
    pub mermaid: String,
    pub steps: Vec<QueryPlanStep>,
    pub notes: Vec<String>,
}

#[derive(Default)]
pub struct AgentSessionState {
    pub last_result: Option<StoredQueryResult>,
    pub last_plan: Option<QueryPlan>,
    /// آخر ملف PDF/Excel حُفظ فعلياً على القرص (مسار Windows كامل)
    pub last_file_path: Option<String>,
    /// منتج مستخرج من @mention في آخر رسالة المستخدم
    pub last_product_filter: Option<String>,
}

/// اسم ملف تصدير ASCII آمن (بدون أحرف عربية — Excel على Windows)
pub fn safe_export_filename(title: &str, ext: &str) -> String {
    let base: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '_'
            } else {
                '_'
            }
        })
        .collect();
    let trimmed: String = base
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if trimmed.is_empty() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("report_{}.{}", ts, ext)
    } else {
        format!(
            "{}.{}",
            trimmed.chars().take(40).collect::<String>(),
            ext
        )
    }
}

pub async fn record_exported_file(app_state: &Arc<AppState>, path: &std::path::Path) {
    let mut session = app_state.agent_session.lock().await;
    session.last_file_path = Some(path.display().to_string());
}

static PRODUCT_SCHEMA_CACHE: OnceLock<String> = OnceLock::new();
const PRODUCT_SCHEMA_EMBED: &str = include_str!("../../PRODUCT_SCHEMA.md");

static DATABASE_VIEWS_CACHE: OnceLock<String> = OnceLock::new();
const DATABASE_VIEWS_EMBED: &str = include_str!("../../DATABASE_VIEWS.md");

pub fn load_database_views() -> &'static str {
    DATABASE_VIEWS_CACHE.get_or_init(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();
        let candidates = [
            exe_dir.join("DATABASE_VIEWS.md"),
            exe_dir.join("../DATABASE_VIEWS.md"),
            exe_dir.join("../../DATABASE_VIEWS.md"),
            std::path::PathBuf::from(
                r"C:\Users\DELL\Desktop\al-tabi\reports-app\DATABASE_VIEWS.md",
            ),
        ];
        for path in &candidates {
            if let Ok(c) = std::fs::read_to_string(path) {
                if c.contains("SALE_ITEMS_INVOICE_VIEW") {
                    return c;
                }
            }
        }
        if DATABASE_VIEWS_EMBED.contains("SALE_ITEMS_INVOICE_VIEW") {
            return DATABASE_VIEWS_EMBED.to_string();
        }
        "# DATABASE_VIEWS not found\n".to_string()
    })
}

pub fn load_product_schema() -> &'static str {
    PRODUCT_SCHEMA_CACHE.get_or_init(|| {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();
        let candidates = [
            exe_dir.join("PRODUCT_SCHEMA.md"),
            exe_dir.join("../PRODUCT_SCHEMA.md"),
            exe_dir.join("../../PRODUCT_SCHEMA.md"),
            exe_dir.join("../../../PRODUCT_SCHEMA.md"),
            std::path::PathBuf::from(
                r"C:\Users\DELL\Desktop\al-tabi\reports-app\PRODUCT_SCHEMA.md",
            ),
        ];
        for path in &candidates {
            if let Ok(c) = std::fs::read_to_string(path) {
                if c.contains("ITEMS") {
                    return c;
                }
            }
        }
        if PRODUCT_SCHEMA_EMBED.contains("ITEMS") {
            return PRODUCT_SCHEMA_EMBED.to_string();
        }
        "# PRODUCT_SCHEMA not found\n".to_string()
    })
}

/// يزيل @ والأقواس الفارغة من @mention (مثل `TRADOL ()` → `TRADOL 10ML (TUNISIA)`)
pub fn sanitize_product_filter(raw: &str) -> String {
    let mut s = raw.trim().trim_start_matches('@').trim().to_string();
    loop {
        let trimmed = s.trim();
        if trimmed.ends_with("()") {
            s = trimmed[..trimmed.len().saturating_sub(2)]
                .trim()
                .to_string();
            continue;
        }
        if trimmed.ends_with("( )") {
            s = trimmed[..trimmed.len().saturating_sub(3)]
                .trim()
                .to_string();
            continue;
        }
        if let Some(idx) = trimmed.rfind('(') {
            if trimmed.ends_with(')') {
                let inner = trimmed[idx + 1..trimmed.len() - 1].trim();
                if inner.is_empty() {
                    s = trimmed[..idx].trim().to_string();
                    continue;
                }
            }
        }
        break;
    }
    s
}

/// يستخرج اسم/كود المنتج من @mention في نص المستخدم
pub fn extract_product_filter_from_text(text: &str) -> Option<String> {
    let at_idx = text.find('@')?;
    let rest = &text[at_idx + 1..];
    let end = rest
        .find('\n')
        .unwrap_or(rest.len());
    let chunk = rest[..end].trim();
    if chunk.is_empty() {
        return None;
    }
    let cleaned = sanitize_product_filter(chunk);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

pub fn resolve_product_filter(explicit: Option<&str>, fallback: Option<&str>) -> Option<String> {
    explicit
        .map(sanitize_product_filter)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            fallback
                .map(sanitize_product_filter)
                .filter(|s| !s.is_empty())
        })
}

fn product_tokens(filter: &str) -> Vec<String> {
    sanitize_product_filter(filter)
        .split_whitespace()
        .filter(|t| t.len() >= 2)
        .map(|t| t.replace('\'', "''"))
        .collect()
}

fn token_and_likes(column: &str, tokens: &[String]) -> String {
    if tokens.is_empty() {
        return format!("{} LIKE N'%%'", column);
    }
    tokens
        .iter()
        .map(|t| format!("{} LIKE N'%{}%'", column, t))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn product_match_or(model_col: &str, name_col: &str, tokens: &[String]) -> String {
    format!(
        "({}) OR ({})",
        token_and_likes(model_col, tokens),
        token_and_likes(name_col, tokens)
    )
}

/// يستبدل N'%PRODUCT%' ببحث AND لكل كلمة — يتجاوز فرق المسافات (مثل ARTAMIN ثلاث مسافات)
pub fn apply_product_filter(sql: &str, filter: &str) -> String {
    let tokens = product_tokens(filter);
    if tokens.is_empty() {
        return sql.to_string();
    }
    let mut out = sql.to_string();
    for (model, name) in [("I.ITEM_MODEL", "I.ITEM_NAME"), ("ITEM_MODEL", "ITEM_NAME")] {
        for wrapper in [true, false] {
            let bare = format!(
                "{} LIKE N'%PRODUCT%' OR {} LIKE N'%PRODUCT%'",
                model, name
            );
            let wrapped = format!("({bare})");
            let replacement = if wrapper {
                format!("({})", product_match_or(model, name, &tokens))
            } else {
                product_match_or(model, name, &tokens)
            };
            if wrapper {
                out = out.replace(&wrapped, &replacement);
            }
            out = out.replace(&bare, &product_match_or(model, name, &tokens));
        }
    }
    for col in ["I.ITEM_MODEL", "I.ITEM_NAME", "ITEM_MODEL", "ITEM_NAME"] {
        let old = format!("{} LIKE N'%PRODUCT%'", col);
        if out.contains(&old) {
            out = out.replace(&old, &token_and_likes(col, &tokens));
        }
    }
    out
}

/// يبحث في رسائل المستخدم السابقة عن @mention (لـ «كل شي» بعد سؤال منتج)
pub fn extract_product_filter_from_history(history: &[serde_json::Value]) -> Option<String> {
    for msg in history.iter().rev() {
        if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            if let Some(pf) = extract_product_filter_from_text(content) {
                return Some(pf);
            }
        }
    }
    None
}

pub fn app_data_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("com.dell.reports-app");
    }
    std::env::temp_dir().join("com.dell.reports-app")
}

fn favorites_path() -> PathBuf {
    app_data_dir().join("agent_favorites.json")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteQuery {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sql: String,
    pub created_at_unix: u64,
}

/// فوق هذا العدد: حفظ كامل + إنشاء PDF تلقائياً + معاينة مختصرة للمحادثة
pub const AUTO_PDF_ROW_THRESHOLD: usize = 25;
pub const CHAT_PREVIEW_MAX_ROWS: usize = 20;
pub const MAX_QUERY_ROWS: usize = 5000;

pub async fn set_last_result(
    state: &Arc<AppState>,
    sql: &str,
    columns: &[String],
    rows: &[Vec<String>],
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut s = state.agent_session.lock().await;
    s.last_result = Some(StoredQueryResult {
        sql: sql.to_string(),
        columns: columns.to_vec(),
        rows: rows.to_vec(),
        saved_at_unix: now,
    });
}

async fn save_pdf_export(
    app_state: &Arc<AppState>,
    delivery: &ExportDelivery,
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Value {
    match generate_report_pdf(title, columns, rows) {
        Ok(bytes) => match delivery {
            ExportDelivery::Local => {
                let filename = safe_export_filename(title, "pdf");
                let path = std::env::temp_dir().join(&filename);
                if std::fs::write(&path, &bytes).is_ok() {
                    record_exported_file(app_state, &path).await;
                    json!({
                        "auto_pdf": true,
                        "file_path": path.display().to_string(),
                    })
                } else {
                    json!({ "auto_pdf": false, "pdf_error": "فشل حفظ PDF محلياً." })
                }
            }
            ExportDelivery::Telegram {
                client,
                token,
                chat_id,
            } => {
                let filename = safe_export_filename(title, "pdf");
                let caption = format!("📊 {} ({} صف)", title, rows.len());
                if send_pdf(client, token, *chat_id, &filename, bytes, &caption)
                    .await
                    .is_ok()
                {
                    json!({
                        "auto_pdf": true,
                        "message": format!(
                            "تم إرسال PDF تلقائياً إلى Telegram ({} صف).",
                            rows.len()
                        ),
                    })
                } else {
                    json!({ "auto_pdf": false, "pdf_error": "فشل إرسال PDF إلى Telegram." })
                }
            }
        },
        Err(e) => json!({ "auto_pdf": false, "pdf_error": e }),
    }
}

/// يحفظ النتيجة كاملة في الجلسة؛ إن كانت كبيرة يُصدّر PDF تلقائياً ويُرجع معاينة مختصرة.
pub async fn package_query_result(
    app_state: &Arc<AppState>,
    delivery: &ExportDelivery,
    sql: &str,
    columns: &[String],
    rows: &[Vec<String>],
    title: &str,
) -> Value {
    if rows.len() > MAX_QUERY_ROWS {
        return json!({
            "error": format!(
                "الاستعلام أعاد {} صفاً — ضيّق النطاق أو أضف TOP/WHERE.",
                rows.len()
            ),
            "row_count": rows.len()
        });
    }

    set_last_result(app_state, sql, columns, rows).await;
    let row_count = rows.len();

    if row_count > AUTO_PDF_ROW_THRESHOLD {
        let preview: Vec<Vec<String>> = rows
            .iter()
            .take(CHAT_PREVIEW_MAX_ROWS)
            .cloned()
            .collect();
        let pdf_meta = save_pdf_export(app_state, delivery, title, columns, rows).await;
        let file_path = pdf_meta
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut out = json!({
            "columns": columns,
            "rows": preview,
            "row_count": row_count,
            "preview_only": true,
            "preview_rows_shown": preview.len(),
            "message": format!(
                "النتيجة {} صفاً — تم إنشاء PDF تلقائياً. اعرض ملخصاً لأول {} صف وأخبر المستخدم أن التقرير الكامل في PDF{}.",
                row_count,
                preview.len(),
                if file_path.is_empty() {
                    ".".to_string()
                } else {
                    format!(": [FILE_PATH:{}]", file_path)
                }
            ),
        });
        if let (Some(base), Some(extra)) = (out.as_object_mut(), pdf_meta.as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
        return out;
    }

    json!({
        "columns": columns,
        "rows": rows,
        "row_count": row_count,
    })
}

/// يزيل تعليقات/DECLARE/فاصلة من البداية قبل التحقق
pub fn normalize_sql_for_readonly(sql: &str) -> String {
    let mut s = sql.trim().trim_start_matches('\u{FEFF}').to_string();
    while s.starts_with(';') {
        s = s[1..].trim_start().to_string();
    }
    while s.starts_with("--") {
        if let Some(pos) = s.find('\n') {
            s = s[pos + 1..].trim_start().to_string();
        } else {
            s.clear();
            break;
        }
    }
    let upper = s.to_uppercase();
    if upper.starts_with("DECLARE ") {
        if let Some(idx) = upper.rfind("; SELECT ") {
            s = s[idx + 2..].trim_start().to_string();
        } else if let Some(idx) = upper.rfind("\nSELECT ") {
            s = s[idx + 1..].trim_start().to_string();
        } else if let Some(idx) = upper.rfind("; WITH ") {
            s = s[idx + 2..].trim_start().to_string();
        } else if let Some(idx) = upper.rfind("\nWITH ") {
            s = s[idx + 1..].trim_start().to_string();
        }
    }
    s
}

fn contains_blocked_sql_keyword(sql_upper: &str) -> bool {
    const BLOCKED: &[&str] = &[
        "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "TRUNCATE", "EXEC", "EXECUTE",
        "GRANT", "REVOKE", "MERGE", "CREATE", "BACKUP", "RESTORE",
    ];
    for kw in BLOCKED {
        let mut search_from = 0;
        while let Some(rel) = sql_upper[search_from..].find(kw) {
            let idx = search_from + rel;
            let before_ok =
                idx == 0 || !sql_upper.as_bytes()[idx - 1].is_ascii_alphanumeric();
            let after_idx = idx + kw.len();
            let after_ok = after_idx >= sql_upper.len()
                || !sql_upper.as_bytes()[after_idx].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
            search_from = idx + 1;
        }
    }
    false
}

pub fn validate_read_only_sql(sql: &str) -> Result<(), String> {
    let normalized = normalize_sql_for_readonly(sql);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err("الاستعلام فارغ.".to_string());
    }
    let select_count = normalized
        .match_indices("SELECT")
        .filter(|(idx, _)| {
            let before_ok = *idx == 0 || !normalized.as_bytes()[idx - 1].is_ascii_alphanumeric();
            let after_idx = idx + 6;
            let after_ok = after_idx >= normalized.len()
                || !normalized.as_bytes()[after_idx].is_ascii_alphanumeric();
            before_ok && after_ok
        })
        .count();
    if select_count > 1 {
        return Err(
            "استعلام واحد فقط في كل مرة — لا تدمج عدة SELECT. استخدم plan_complex_query + execute_query_plan لدراسة منتج شاملة.".to_string(),
        );
    }
    let sql_upper = trimmed.to_uppercase();
    if !(sql_upper.starts_with("SELECT ") || sql_upper.starts_with("WITH ")) {
        return Err(
            "يجب أن يبدأ الاستعلام بـ SELECT أو WITH (يمكن DECLARE قبله).".to_string(),
        );
    }
    if contains_blocked_sql_keyword(&sql_upper) {
        return Err("استعلامات التعديل أو DDL غير مسموحة.".to_string());
    }
    Ok(())
}

/// يُفعّل QUOTED_IDENTIFIER لدعم أسماء أعمدة عربية بين \"...\"
pub fn prepare_sql_batch(sql: &str) -> String {
    format!(
        "SET NOCOUNT ON; SET QUOTED_IDENTIFIER ON;\n{}",
        sql.trim()
    )
}

fn is_safe_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.chars().next().unwrap_or('0').is_ascii_digit()
}

// ─── تعريفات الأدوات (JSON) ─────────────────────────────────────────────────

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "validate_sql",
                "description": "Validates a read-only T-SQL query BEFORE execute_raw_sql. Returns ok/issues (syntax class, blocked keywords, missing TOP). Call when unsure or after writing complex SQL.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql_query": { "type": "string", "description": "The T-SQL SELECT/WITH to validate." }
                    },
                    "required": ["sql_query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "explain_sql",
                "description": "Explains a T-SQL query in Arabic for the user: tables, joins, filters, aggregates. Use after writing SQL or when user asks what a query does.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql_query": { "type": "string", "description": "The SQL to explain." }
                    },
                    "required": ["sql_query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_table_sample",
                "description": "Returns TOP N sample rows from a dbo table to see real column values. table_name must be a simple name like ITEMS or SALE_INVOICE (no SQL injection).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_name": { "type": "string", "description": "Table name without schema or with dbo. prefix." },
                        "row_limit": { "type": "integer", "description": "Rows to return (default 5, max 20)." }
                    },
                    "required": ["table_name"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "save_favorite_query",
                "description": "Saves a successful SELECT query to favorites for reuse. Call after execute_raw_sql returned good results.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Short Arabic name." },
                        "sql_query": { "type": "string", "description": "The SELECT to save." },
                        "description": { "type": "string", "description": "Optional one-line description." }
                    },
                    "required": ["name", "sql_query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_favorite_queries",
                "description": "Lists saved favorite SQL queries. Use before re-running a saved query.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_query_pattern",
                "description": "Finds a QUERY_PATTERNS template by keywords, executes it, returns rows. Results >25 rows auto-export as PDF (preview + file_path). For مقارنة أسعار موردين pass product_filter (product name/code). Prefer over writing complex SQL from scratch.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keywords": { "type": "string", "description": "Pattern search keywords (Arabic/English)." },
                        "days_recent": { "type": "integer", "description": "Override sales window days (default 60)." },
                        "coverage_days": { "type": "integer", "description": "For purchase patterns: target coverage days (default 30)." },
                        "product_filter": { "type": "string", "description": "Product code or name fragment — replaces %PRODUCT% in pattern SQL." }
                    },
                    "required": ["keywords"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_database_views",
                "description": "Returns DATABASE_VIEWS.md: SQL Server views (SALE_ITEMS_INVOICE_VIEW, etc.), join rules, revenue formulas SUM(QTY*PRICE), employee=USERS.FULL_NAME via SALE_INVOICE.USERS_ID. Call BEFORE writing sales/employee SQL or when aggregation errors occur.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": { "type": "string", "description": "Optional: 'sales', 'employee', 'views', or empty for full doc." }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_product_schema",
                "description": "Returns PRODUCT_SCHEMA.md: ITEMS, ITEMS_SUB, UNITS, BARCODE, unit prices, stock formulas. Call before complex product analysis or when unsure which columns hold description/units/prices.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "section": { "type": "string", "description": "Optional: 'units', 'prices', 'stock', or empty for full doc." }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "plan_complex_query",
                "description": "Designs a multi-step query plan (Mermaid diagram + numbered steps with SQL or pattern keywords) for complex requests: product study, smart purchase, debts, etc. ALWAYS call this BEFORE execute_query_plan when the user asks for deep analysis (دراسة منتج، مخزون وسرعة بيع، طلبية مقترحة). Stores plan in session.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "question": { "type": "string", "description": "User question in Arabic." },
                        "product_filter": { "type": "string", "description": "Product code/name from @mention or user text." },
                        "days_recent": { "type": "integer", "description": "Sales window days (default 60)." },
                        "coverage_days": { "type": "integer", "description": "Target stock coverage days (default 30)." }
                    },
                    "required": ["question"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "execute_query_plan",
                "description": "Executes a query plan step-by-step. Pass steps from plan_complex_query (each needs sql_query) OR omit steps to run the last stored plan. Returns per-step columns/rows/errors. Use after plan_complex_query for دراسة منتج and similar.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "steps": {
                            "type": "array",
                            "description": "Optional. Array of {step_id, title, sql_query}. If empty, uses last plan from plan_complex_query.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "step_id": { "type": "integer" },
                                    "title": { "type": "string" },
                                    "sql_query": { "type": "string" }
                                },
                                "required": ["step_id", "title", "sql_query"]
                            }
                        },
                        "stop_on_error": { "type": "boolean", "description": "Stop after first failed step (default true)." }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "compare_periods",
                "description": "Compares sales or purchases between two date ranges side by side (totals + invoice counts).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "metric": { "type": "string", "description": "'sales' or 'purchases'." },
                        "period1_start": { "type": "string", "description": "YYYY-MM-DD" },
                        "period1_end": { "type": "string", "description": "YYYY-MM-DD" },
                        "period2_start": { "type": "string", "description": "YYYY-MM-DD" },
                        "period2_end": { "type": "string", "description": "YYYY-MM-DD" }
                    },
                    "required": ["metric", "period1_start", "period1_end", "period2_start", "period2_end"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "suggest_indexes",
                "description": "Suggests SQL Server indexes based on tables/columns in a query (heuristic, Marketing2026-aware).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql_query": { "type": "string", "description": "The query to analyze." }
                    },
                    "required": ["sql_query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "export_last_result",
                "description": "Exports the last successful execute_raw_sql / run_query_pattern result as PDF or Excel. Requires a prior query in this session.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Report title in Arabic." },
                        "format": { "type": "string", "description": "'pdf' or 'excel'." }
                    },
                    "required": ["title", "format"]
                }
            }
        }),
    ]
}

pub const EXTENDED_TOOL_NAMES: &[&str] = &[
    "validate_sql",
    "explain_sql",
    "get_table_sample",
    "save_favorite_query",
    "list_favorite_queries",
    "run_query_pattern",
    "compare_periods",
    "suggest_indexes",
    "export_last_result",
    "get_product_schema",
    "get_database_views",
    "plan_complex_query",
    "execute_query_plan",
];

pub fn is_extended_tool(name: &str) -> bool {
    EXTENDED_TOOL_NAMES.contains(&name)
}

/// أنماط SQL ممنوعة (تسبب Msg 130 أو نتائج خاطئة) — تُستخدم قبل التنفيذ
pub fn sql_antipattern_issues(sql: &str) -> Vec<(bool, String)> {
    let upper = sql.to_uppercase();
    let mut out: Vec<(bool, String)> = Vec::new();

    if upper.contains("SUM(") && upper.contains("SELECT SUM") {
        out.push((
            true,
            "SQL Server Msg 130: لا يجوز SUM(...) فوق subquery فيها SUM. \
             الصحيح: JOIN SALE_ITEMS + SALE_INVOICE ثم SUM(QTY*PRICE)، \
             أو run_query_pattern(keywords=\"مبيعات يومية موظف\").".to_string(),
        ));
    }

    if upper.contains("SALE_ITEMS")
        && !upper.contains("SALE_INVOICE")
        && !upper.contains("SALE_ITEMS_INVOICE_VIEW")
        && (upper.contains("S_DATE") || upper.contains("CONVERT(DATE"))
    {
        out.push((
            true,
            "SALE_ITEMS لا يحتوي S_DATE — يجب JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID \
             أو استخدم dbo.SALE_ITEMS_INVOICE_VIEW.".to_string(),
        ));
    }

    if (upper.contains("SALE_ITEMS") || upper.contains("SALE_ITEMS_INVOICE_VIEW"))
        && upper.contains("SUM(")
        && upper.contains("PRICE")
        && !upper.contains("QTY *")
        && !upper.contains("QTY*")
        && !upper.contains("LINEVALUE")
    {
        out.push((
            true,
            "إيراد المبيعات = SUM(QTY * PRICE) على مستوى البند — لا SUM(PRICE) وحده.".to_string(),
        ));
    }

    if upper.contains("BALANCE_C") && (upper.contains("CUST_CUSTOM") || upper.contains("دين") || upper.contains("DEBT") || upper.contains("BALANCE <") || upper.contains("BALANCE >")) {
        out.push((
            true,
            "BALANCE_C فارغ في Marketing2026 — لا تستخدمه للديون. استخدم run_query_pattern('متابعة الديون'): مبيعات−مردودات−TAKE+BALANCE_EDIT.".to_string(),
        ));
    }

    if upper.contains("FROM DBO.SALARIES") && !upper.contains("UNION") && !upper.contains("EMP_SALARY") && !upper.contains("CUSTOMERS") {
        out.push((
            false,
            "جدول SALARIES قد يكون فارغاً — استخدم run_query_pattern('رواتب') الذي ي fallback إلى CUSTOMERS.EMP_SALARY و GIVE مصاريف رواتب.".to_string(),
        ));
    }

    if upper.contains("FROM DBO.SALE_INVOICE")
        && upper.contains("GROUP BY")
        && upper.contains("SUM(")
        && !upper.contains("SALE_ITEMS")
        && !upper.contains("SALE_ITEMS_INVOICE_VIEW")
    {
        out.push((
            true,
            "لا تجمع إيرادات من SALE_INVOICE وحدها — البنود في SALE_ITEMS (QTY×PRICE).".to_string(),
        ));
    }

    out
}

pub fn antipattern_block_message(sql: &str) -> Option<String> {
    let blocking: Vec<String> = sql_antipattern_issues(sql)
        .into_iter()
        .filter(|(b, _)| *b)
        .map(|(_, m)| m)
        .collect();
    if blocking.is_empty() {
        None
    } else {
        Some(blocking.join(" | "))
    }
}

// ─── معالجات الأدوات ────────────────────────────────────────────────────────

pub async fn handle_validate_sql(sql: &str) -> Value {
    let mut issues: Vec<String> = Vec::new();
    let mut ok = true;

    if let Err(e) = validate_read_only_sql(sql) {
        issues.push(e);
        ok = false;
    }

    for (blocking, msg) in sql_antipattern_issues(sql) {
        issues.push(msg);
        if blocking {
            ok = false;
        }
    }

    let upper = sql.to_uppercase();
    if !upper.contains("TOP ") && !upper.contains("TOP\t") {
        issues.push("يُفضّل إضافة TOP N لتقليل الصفوف (مثلاً TOP 100).".to_string());
    }
    if (sql.matches('(').count()) != (sql.matches(')').count()) {
        issues.push("عدد الأقواس ( ) غير متوازن.".to_string());
        ok = false;
    }
    if upper.contains("LIMIT ") {
        issues.push("استخدم TOP وليس LIMIT (SQL Server).".to_string());
        ok = false;
    }
    if upper.contains("NOW()") {
        issues.push("استخدم GETDATE() بدلاً من NOW().".to_string());
        ok = false;
    }

    json!({
        "valid": ok,
        "issues": issues,
        "message": if ok { "الاستعلام يبدو صالحاً للتنفيذ (قراءة فقط)." } else { "يوجد مشاكل يجب إصلاحها قبل التنفيذ." }
    })
}

pub fn handle_explain_sql(sql: &str) -> Value {
    let upper = sql.to_uppercase();
    let mut parts: Vec<String> = Vec::new();

    parts.push("شرح الاستعلام بالعربية:".to_string());

    if upper.contains("WITH ") {
        parts.push("• يستخدم CTE (WITH) لخطوات وسيطة قبل النتيجة النهائية.".to_string());
    }
    if upper.contains("JOIN ") {
        parts.push("• يربط عدة جداول (JOIN) — تحقق أن مفتاح الربط S_ID أو ITEM_ID صحيح.".to_string());
    }
    if upper.contains("SALE_ITEMS") && upper.contains("SALE_INVOICE") {
        parts.push("• مبيعات: SALE_ITEMS للبنود + SALE_INVOICE للتاريخ (SALE_ITEMS لا يحتوي S_DATE).".to_string());
    }
    if upper.contains("ITEMS_SUB") {
        parts.push("• مخزون: ITEMS_SUB.QTY هو مصدر الكمية؛ CATEOGRY3 = تاريخ الصلاحية.".to_string());
    }
    if upper.contains("GROUP BY") {
        parts.push("• يجمّع النتائج (GROUP BY) — غالباً مع SUM/COUNT.".to_string());
    }
    if upper.contains("WHERE ") {
        parts.push("• يفلتر الصفوف بشرط WHERE — راجع التواريخ وLIKE للبحث النصي.".to_string());
    }
    if upper.contains("ORDER BY") {
        parts.push("• يرتب النتائج (ORDER BY).".to_string());
    }
    if upper.contains("UNION") {
        parts.push("• يدمج نتيجتين (UNION) — مثلاً ديون لي وعلي.".to_string());
    }

    for table in [
        "ITEMS", "ITEMS_SUB", "SALE_INVOICE", "SALE_ITEMS", "BUY_INVOICE", "BUY_ITEMS",
        "CUSTOMERS", "TAKE", "GIVE", "BALANCE_EDIT",
    ] {
        if upper.contains(table) {
            parts.push(format!("• يستخدم جدول {}.", table));
        }
    }

    json!({
        "explanation_ar": parts.join("\n"),
        "sql_preview": if sql.len() > 500 { format!("{}...", &sql[..500]) } else { sql.to_string() }
    })
}

pub async fn handle_get_table_sample(
    table_name: &str,
    row_limit: u32,
    app_state: &Arc<AppState>,
) -> Value {
    let clean = table_name.trim().replace("dbo.", "").replace("DBO.", "");
    if !is_safe_identifier(&clean) {
        return json!({ "error": "اسم جدول غير صالح. استخدم اسماً بسيطاً مثل ITEMS." });
    }
    let limit = row_limit.clamp(1, 20);
    let sql = format!("SELECT TOP {} * FROM dbo.[{}]", limit, clean);

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات." });
    }

    match execute_sql_query(conn_opt.unwrap(), sql.clone()).await {
        Ok(result) => {
            set_last_result(app_state, &sql, &result.columns, &result.rows).await;
            json!({
                "table": clean,
                "columns": result.columns,
                "rows": result.rows,
                "row_count": result.row_count
            })
        }
        Err(e) => json!({ "error": format!("{}", e) }),
    }
}

fn load_favorites() -> Vec<FavoriteQuery> {
    let path = favorites_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_favorites(list: &[FavoriteQuery]) -> Result<(), String> {
    let dir = app_data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let data = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    std::fs::write(favorites_path(), data).map_err(|e| e.to_string())
}

pub fn handle_save_favorite(name: &str, sql: &str, description: &str) -> Value {
    if let Err(e) = validate_read_only_sql(sql) {
        return json!({ "error": e });
    }
    let mut list = load_favorites();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let fav = FavoriteQuery {
        id: format!("fav_{}", now),
        name: name.to_string(),
        description: description.to_string(),
        sql: sql.to_string(),
        created_at_unix: now,
    };
    list.insert(0, fav.clone());
    if list.len() > 50 {
        list.truncate(50);
    }
    match save_favorites(&list) {
        Ok(()) => json!({ "success": true, "id": fav.id, "message": format!("تم حفظ «{}» في المفضلة.", name) }),
        Err(e) => json!({ "error": e }),
    }
}

pub fn handle_list_favorites() -> Value {
    let list = load_favorites();
    json!({
        "favorites": list.iter().map(|f| json!({
            "id": f.id,
            "name": f.name,
            "description": f.description,
            "sql_preview": if f.sql.len() > 120 { format!("{}...", &f.sql[..120]) } else { f.sql.clone() },
            "created_at_unix": f.created_at_unix
        })).collect::<Vec<_>>(),
        "count": list.len()
    })
}

/// يستخرج أول كتلة ```sql من نمط QUERY_PATTERNS
fn extract_sql_from_pattern_section(section: &str) -> Option<String> {
    extract_all_sql_from_pattern_section(section).into_iter().next()
}

/// يستخرج كل كتل ```sql من قسم النمط (مثل ملخص-مالي-شهري = 3 استعلامات)
fn extract_all_sql_from_pattern_section(section: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut in_block = false;
    let mut lines: Vec<String> = Vec::new();
    for line in section.lines() {
        if line.trim().starts_with("```sql") {
            in_block = true;
            continue;
        }
        if in_block && line.trim() == "```" {
            if !lines.is_empty() {
                blocks.push(lines.join("\n"));
            }
            lines.clear();
            in_block = false;
            continue;
        }
        if in_block {
            lines.push(line.to_string());
        }
    }
    blocks
}

fn apply_pattern_sql_params(sql: &str, days_recent: u32, coverage_days: u32) -> String {
    let mut out = sql.to_string();
    out = out.replace("DATEADD(day,-60,", &format!("DATEADD(day,-{},", days_recent));
    out = out.replace("DATEADD(day, -60,", &format!("DATEADD(day, -{},", days_recent));
    out = out.replace("*30 -", &format!("*{} -", coverage_days));
    out = out.replace("* 30 -", &format!("* {} -", coverage_days));
    out
}

fn sql_from_pattern_keywords(keywords: &str) -> Option<String> {
    let pattern_text = crate::ai_agent::search_query_patterns_local(keywords);
    if pattern_text.contains("لم يُعثر على نمط") {
        return None;
    }
    let section = pattern_text.split("\n\n---\n\n").next().unwrap_or(&pattern_text);
    extract_sql_from_pattern_section(section)
}

pub fn handle_get_database_views(section: Option<&str>) -> Value {
    let full = load_database_views();
    let content = match section.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        Some(s) if s.contains("employee") || s.contains("موظف") => full
            .lines()
            .skip_while(|l| !l.contains("موظف") && !l.contains("USERS"))
            .take(80)
            .collect::<Vec<_>>()
            .join("\n"),
        Some(s) if s.contains("sales") || s.contains("مبيع") || s.contains("view") => full
            .lines()
            .skip_while(|l| !l.contains("SALE_ITEMS_INVOICE_VIEW") && !l.contains("Views"))
            .take(100)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => full.to_string(),
    };
    json!({
        "views_guide": content,
        "message": "استخدم run_query_pattern('مبيعات يومية موظف') أو SALE_ITEMS_INVOICE_VIEW مع SUM(QTY*PRICE)."
    })
}

pub fn handle_get_product_schema(section: Option<&str>) -> Value {
    let full = load_product_schema();
    let content = match section.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        Some(s) if s.contains("unit") || s.contains("وحدة") => {
            full.lines()
                .skip_while(|l| !l.contains("UNITS") && !l.contains("BARCODE"))
                .take(80)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Some(s) if s.contains("price") || s.contains("سعر") => {
            full.lines()
                .skip_while(|l| !l.contains("PRICE") && !l.contains("BARCODE"))
                .take(80)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Some(s) if s.contains("stock") || s.contains("مخزون") => {
            full.lines()
                .skip_while(|l| !l.contains("ITEMS_SUB") && !l.contains("مخزون"))
                .take(60)
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => full.to_string(),
    };
    json!({
        "schema": content,
        "message": "مرجع PRODUCT_SCHEMA — استخدم run_query_pattern أو execute_query_plan للتنفيذ."
    })
}

fn build_complex_plan(
    question: &str,
    product_filter: Option<&str>,
    days_recent: u32,
    coverage_days: u32,
) -> QueryPlan {
    let q = question.to_lowercase();
    let pf = product_filter
        .and_then(|s| {
            let c = sanitize_product_filter(s);
            if c.is_empty() {
                None
            } else {
                Some(c)
            }
        });

    let supplier_intent = (q.contains("مقارنة")
        && (q.contains("سعر")
            || q.contains("مورد")
            || q.contains("اسعار")
            || q.contains("أسعار")))
        || q.contains("أرخص مورد")
        || q.contains("supplier price")
        || ((q.contains("مورد") || q.contains("موردين") || q.contains("موردي"))
            && pf.is_some()
            && !q.contains("نواقص"));

    let (plan_type, mermaid, steps, notes) = if supplier_intent {
        let steps = vec![QueryPlanStep {
            step_id: 1,
            title: "مقارنة أسعار الموردين".to_string(),
            purpose: "BUY_ITEMS + BUY_INVOICE — آخر/أقل/أعلى/متوسط سعر لكل مورد لصنف واحد".to_string(),
            recommended_tool: "run_query_pattern".to_string(),
            pattern_keywords: Some("مقارنة أسعار موردين".to_string()),
            sql_query: sql_from_pattern_keywords("مقارنة أسعار موردين"),
        }];
        (
            "supplier_price_comparison".to_string(),
            "flowchart TD\n  Q[مقارنة/موردي منتج] --> P[run_query_pattern + product_filter] --> R[جدول مورد+أسعار]"
                .to_string(),
            steps,
            vec![
                "يجب تمرير product_filter (اسم/كود المنتج)".to_string(),
                "المورد = BUY_INVOICE.CUST_ID → CUSTOMERS، NOT GIVE".to_string(),
            ],
        )
    } else if pf.is_some()
        && (q.contains("دراس")
            || q.contains("منتج")
            || q.contains("صنف")
            || q.contains("مخزون")
            || q.contains("سرعة")
            || q.contains("طلبية")
            || q.contains("يصمد")
            || q.contains("product")
            || q.contains("stock"))
    {
        let product = pf.as_deref().unwrap_or("");
        let mut steps = Vec::new();
        let mut n = 1u32;

        if let Some(mut sql) = sql_from_pattern_keywords("دراسة منتج شاملة") {
            sql = apply_product_filter(&sql, product);
            sql = sql.replace("DATEADD(day,-60,", &format!("DATEADD(day,-{},", days_recent));
            sql = sql.replace("*30 -", &format!("*{} -", coverage_days));
            steps.push(QueryPlanStep {
                step_id: n,
                title: "ملخص المخزون والبيع وطلبية الشراء".to_string(),
                purpose: "مخزون حالي، معدل يومي، أيام تغطية، كمية شراء مقترحة، آخر مورد".to_string(),
                recommended_tool: "execute_query_plan".to_string(),
                pattern_keywords: Some("دراسة منتج شاملة".to_string()),
                sql_query: Some(sql),
            });
            n += 1;
        }

        if let Some(sql) = sql_from_pattern_keywords("تفاصيل منتج وحدات أسعار")
            .map(|s| apply_product_filter(&s, product))
        {
            steps.push(QueryPlanStep {
                step_id: n,
                title: "وحدات البيع وأسعار PRICE1–5".to_string(),
                purpose: "BARCODE + UNITS — كل وحدة وأسعارها".to_string(),
                recommended_tool: "execute_query_plan".to_string(),
                pattern_keywords: Some("تفاصيل منتج وحدات أسعار".to_string()),
                sql_query: Some(sql),
            });
            n += 1;
        }

        if let Some(sql) = sql_from_pattern_keywords("مبيعات منتج حسب الوحدة")
            .map(|s| apply_product_filter(&s, product))
        {
            steps.push(QueryPlanStep {
                step_id: n,
                title: "توزيع المبيعات على الوحدات".to_string(),
                purpose: "أي وحدة تُباع أكثر في آخر 90 يوم".to_string(),
                recommended_tool: "execute_query_plan".to_string(),
                pattern_keywords: Some("مبيعات منتج حسب الوحدة".to_string()),
                sql_query: Some(sql),
            });
            n += 1;
        }

        let purchases_sql = apply_product_filter(
            r";WITH ItemPick AS (
  SELECT TOP 1 ITEM_ID FROM dbo.ITEMS
  WHERE ITEM_INVISIBLE=0 AND (ITEM_MODEL LIKE N'%PRODUCT%' OR ITEM_NAME LIKE N'%PRODUCT%')
  ORDER BY CASE WHEN ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END
)
SELECT TOP 15 CAST(B.B_DATE AS date) AS BuyDate, CU.CUST_NAME AS Supplier,
  BI.QTY, U.UNIT_DISC AS UnitName, BI.PRICE, BI.QTY*BI.PRICE AS LineValue
FROM dbo.BUY_ITEMS BI
JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
JOIN ItemPick IP ON BI.ITEM_ID=IP.ITEM_ID
LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
LEFT JOIN dbo.UNITS U ON BI.UNIT_ID=U.UNIT_ID
ORDER BY B.B_DATE DESC",
            product,
        );
        steps.push(QueryPlanStep {
            step_id: n,
            title: "آخر مشتريات للصنف".to_string(),
            purpose: "تاريخ، مورد، كمية، سعر".to_string(),
            recommended_tool: "execute_query_plan".to_string(),
            pattern_keywords: None,
            sql_query: Some(purchases_sql),
        });

        let mermaid = format!(
            "flowchart TD\n  A[سؤال: {}] --> B[خطوة 1: ملخص مخزون/بيع]\n  B --> C[خطوة 2: وحدات وأسعار]\n  C --> D[خطوة 3: مبيعات بالوحدة]\n  D --> E[خطوة 4: آخر مشتريات]\n  E --> F[تحليل وتوصية للمستخدم]",
            question.chars().take(40).collect::<String>()
        );
        let notes = vec![
            format!("نافذة مبيعات: {} يوم | تغطية مستهدفة: {} يوم", days_recent, coverage_days),
            "بعد التنفيذ: لخّص DaysCoverage و SuggestedBuyQty و Priority للمستخدم.".to_string(),
        ];
        ("product_study".to_string(), mermaid, steps, notes)
    } else if q.contains("موظف") || q.contains("مبيعات يوم") || q.contains("employee") || q.contains("daily sales") {
        let steps = vec![QueryPlanStep {
            step_id: 1,
            title: "مبيعات يومية لكل موظف".to_string(),
            purpose: "SALE_ITEMS_INVOICE_VIEW أو CTE — SUM(QTY*PRICE) — لا subquery على PRICE".to_string(),
            recommended_tool: "run_query_pattern".to_string(),
            pattern_keywords: Some("مبيعات يومية موظف".to_string()),
            sql_query: sql_from_pattern_keywords("مبيعات يومية موظف"),
        }];
        (
            "employee_daily_sales".to_string(),
            "flowchart TD\n  Q[مبيعات يومية/موظف] --> V[get_database_views]\n  V --> P[run_query_pattern]\n  P --> R[جدول يوم+موظف+إيراد]"
                .to_string(),
            steps,
            vec![
                "الموظف = USERS.FULL_NAME عبر SALE_INVOICE.USERS_ID".to_string(),
                "استخدم get_database_views إذا فشل التجميع".to_string(),
            ],
        )
    } else if q.contains("ديون") || q.contains("ذمة") || q.contains("مدين") {
        let steps = vec![QueryPlanStep {
            step_id: 1,
            title: "متابعة الديون".to_string(),
            purpose: "أرصدة الزبائن والموردين".to_string(),
            recommended_tool: "run_query_pattern".to_string(),
            pattern_keywords: Some("متابعة الديون".to_string()),
            sql_query: sql_from_pattern_keywords("متابعة الديون"),
        }];
        (
            "debts".to_string(),
            "flowchart TD\n  Q[سؤال ديون] --> P[run_query_pattern: متابعة الديون] --> R[تقرير للمستخدم]"
                .to_string(),
            steps,
            vec![],
        )
    } else if q.contains("طلبية") || q.contains("شراء ذك") || q.contains("نواقص") {
        let kw = if q.contains("نواقص") {
            if q.contains("مورد")
                || q.contains("سعر")
                || q.contains("شراء")
                || q.contains("نشط")
                || q.contains("تباع")
            {
                "نواقص نشطة مورد"
            } else {
                "متابعة النواقص"
            }
        } else {
            "طلبية شراء ذكية"
        };
        let steps = vec![QueryPlanStep {
            step_id: 1,
            title: kw.to_string(),
            purpose: "قائمة أصناف مع كميات مقترحة".to_string(),
            recommended_tool: "run_query_pattern".to_string(),
            pattern_keywords: Some(kw.to_string()),
            sql_query: sql_from_pattern_keywords(kw),
        }];
        (
            "purchase".to_string(),
            "flowchart TD\n  Q[طلبية/نواقص] --> P[نمط QUERY_PATTERNS] --> R[جدول أولويات]"
                .to_string(),
            steps,
            vec![],
        )
    } else {
        let steps = vec![
            QueryPlanStep {
                step_id: 1,
                title: "فهم المخطط".to_string(),
                purpose: "قراءة أعمدة المنتجات إن لزم".to_string(),
                recommended_tool: "get_product_schema".to_string(),
                pattern_keywords: None,
                sql_query: None,
            },
            QueryPlanStep {
                step_id: 2,
                title: "بحث نمط جاهز".to_string(),
                purpose: "search_query_patterns ثم validate_sql".to_string(),
                recommended_tool: "search_query_patterns".to_string(),
                pattern_keywords: Some(question.chars().take(80).collect()),
                sql_query: None,
            },
            QueryPlanStep {
                step_id: 3,
                title: "تنفيذ SQL".to_string(),
                purpose: "execute_raw_sql أو run_query_pattern".to_string(),
                recommended_tool: "execute_raw_sql".to_string(),
                pattern_keywords: None,
                sql_query: None,
            },
        ];
        (
            "generic".to_string(),
            "flowchart TD\n  Q[سؤال معقد] --> S[schema/نمط] --> V[validate] --> X[تنفيذ] --> A[إجابة]"
                .to_string(),
            steps,
            vec!["املأ sql_query في execute_query_plan بعد search_query_patterns.".to_string()],
        )
    };

    QueryPlan {
        question: question.to_string(),
        product_filter: pf,
        plan_type,
        mermaid,
        steps,
        notes,
    }
}

pub async fn handle_plan_complex_query(
    question: &str,
    product_filter: Option<&str>,
    days_recent: Option<u32>,
    coverage_days: Option<u32>,
    app_state: &Arc<AppState>,
) -> Value {
    let dr = days_recent.unwrap_or(60);
    let cov = coverage_days.unwrap_or(30);
    let plan = build_complex_plan(question, product_filter, dr, cov);

    {
        let mut s = app_state.agent_session.lock().await;
        s.last_plan = Some(plan.clone());
    }

    json!({
        "plan_type": plan.plan_type,
        "product_filter": plan.product_filter,
        "mermaid": plan.mermaid,
        "notes": plan.notes,
        "steps": plan.steps.iter().map(|st| json!({
            "step_id": st.step_id,
            "title": st.title,
            "purpose": st.purpose,
            "recommended_tool": st.recommended_tool,
            "pattern_keywords": st.pattern_keywords,
            "has_sql": st.sql_query.is_some(),
            "sql_preview": st.sql_query.as_ref().map(|s| if s.len() > 200 { format!("{}...", &s[..200]) } else { s.clone() })
        })).collect::<Vec<_>>(),
        "message": "تم رسم الخطة. للتنفيذ: استدعِ execute_query_plan (يمكن تمرير steps من الخطة أو تركها فارغة لاستخدام آخر خطة محفوظة).",
        "next_tool": "execute_query_plan"
    })
}

pub async fn handle_execute_query_plan(
    steps_arg: Option<Vec<Value>>,
    stop_on_error: bool,
    app_state: &Arc<AppState>,
    delivery: &ExportDelivery,
) -> Value {
    let steps_from_session: Vec<QueryPlanStep> = {
        let s = app_state.agent_session.lock().await;
        s.last_plan.as_ref().map(|p| p.steps.clone()).unwrap_or_default()
    };

    let exec_steps: Vec<(u32, String, String)> = if let Some(arr) = steps_arg {
        if arr.is_empty() {
            steps_from_session
                .iter()
                .filter_map(|st| {
                    st.sql_query
                        .clone()
                        .map(|sql| (st.step_id, st.title.clone(), sql))
                })
                .collect()
        } else {
            arr.iter()
                .filter_map(|v| {
                    let id = v.get("step_id").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                    let title = v
                        .get("title")
                        .and_then(|x| x.as_str())
                        .unwrap_or("خطوة")
                        .to_string();
                    let sql = v.get("sql_query").and_then(|x| x.as_str())?.to_string();
                    Some((id, title, sql))
                })
                .collect()
        }
    } else {
        steps_from_session
            .iter()
            .filter_map(|st| {
                st.sql_query
                    .clone()
                    .map(|sql| (st.step_id, st.title.clone(), sql))
            })
            .collect()
    };

    if exec_steps.is_empty() {
        return json!({
            "error": "لا توجد خطوات قابلة للتنفيذ. نفّذ plan_complex_query أولاً أو مرّر steps[].sql_query."
        });
    }

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات." });
    }
    let conn = conn_opt.unwrap();

    let mut step_results: Vec<Value> = Vec::new();
    let mut last_columns: Vec<String> = Vec::new();
    let mut last_rows: Vec<Vec<String>> = Vec::new();

    for (step_id, step_title, sql) in exec_steps {
        if let Err(e) = validate_read_only_sql(&sql) {
            step_results.push(json!({
                "step_id": step_id, "title": step_title, "ok": false, "error": e
            }));
            if stop_on_error {
                break;
            }
            continue;
        }
        match execute_sql_query(conn.clone(), sql.clone()).await {
            Ok(result) => {
                let packaged = package_query_result(
                    app_state,
                    delivery,
                    &sql,
                    &result.columns,
                    &result.rows,
                    &step_title,
                )
                .await;
                if packaged.get("error").is_some() {
                    step_results.push(json!({
                        "step_id": step_id, "title": step_title, "ok": false,
                        "error": packaged.get("error"),
                        "row_count": result.row_count
                    }));
                    if stop_on_error {
                        break;
                    }
                    continue;
                }
                last_columns = result.columns.clone();
                last_rows = result.rows.clone();
                step_results.push(json!({
                    "step_id": step_id,
                    "title": step_title,
                    "ok": true,
                    "row_count": result.row_count,
                    "columns": packaged.get("columns").cloned().unwrap_or(json!(result.columns)),
                    "rows": packaged.get("rows").cloned().unwrap_or(json!(result.rows)),
                    "preview_only": packaged.get("preview_only"),
                    "auto_pdf": packaged.get("auto_pdf"),
                    "file_path": packaged.get("file_path"),
                }));
            }
            Err(e) => {
                step_results.push(json!({
                    "step_id": step_id, "title": step_title, "ok": false, "error": format!("{}", e)
                }));
                if stop_on_error {
                    break;
                }
            }
        }
    }

    if !last_rows.is_empty() {
        set_last_result(
            app_state,
            "-- execute_query_plan",
            &last_columns,
            &last_rows,
        )
        .await;
    }

    json!({
        "step_results": step_results,
        "steps_run": step_results.len(),
        "message": "تم تنفيذ الخطة. ادمج النتائج في إجابة تحليلية (مخزون، سرعة بيع، أيام تغطية، طلبية مقترحة)."
    })
}

pub async fn handle_run_query_pattern(
    keywords: &str,
    days_recent: Option<u32>,
    coverage_days: Option<u32>,
    product_filter: Option<&str>,
    app_state: &Arc<AppState>,
    delivery: &ExportDelivery,
) -> Value {
    let pattern_text = crate::ai_agent::search_query_patterns_local(keywords);
    if pattern_text.contains("لم يُعثر على نمط") {
        return json!({ "error": pattern_text });
    }

    let section = pattern_text
        .split("\n\n---\n\n")
        .next()
        .unwrap_or(&pattern_text);

    let sql_blocks = extract_all_sql_from_pattern_section(section);
    if sql_blocks.is_empty() {
        return json!({
            "error": "وُجد النمط لكن لم تُستخرج كتلة SQL. استخدم search_query_patterns ثم انسخ SQL يدوياً.",
            "pattern_excerpt": pattern_text.lines().take(15).collect::<Vec<_>>().join("\n")
        });
    }

    let dr = days_recent.filter(|d| *d > 0).unwrap_or(60);
    let cov = coverage_days.filter(|d| *d > 0).unwrap_or(30);

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات." });
    }
    let conn = conn_opt.unwrap();

    let mut parts: Vec<Value> = Vec::new();
    let mut total_rows = 0usize;
    let mut last_sql = String::new();
    let mut last_columns: Vec<String> = Vec::new();
    let mut last_rows: Vec<Vec<String>> = Vec::new();

    for (idx, block) in sql_blocks.iter().enumerate() {
        let mut sql = apply_pattern_sql_params(block, dr, cov);
        if let Some(pf) = product_filter.filter(|s| !s.trim().is_empty()) {
            sql = apply_product_filter(&sql, pf);
        }
        if let Err(e) = validate_read_only_sql(&sql) {
            return json!({ "error": e, "sql_attempted": sql, "part_index": idx + 1 });
        }
        match execute_sql_query(conn.clone(), sql.clone()).await {
            Ok(result) => {
                let part_title = if sql_blocks.len() > 1 {
                    format!("{} - جزء {}", keywords, idx + 1)
                } else {
                    keywords.to_string()
                };
                let packaged = package_query_result(
                    app_state,
                    delivery,
                    &sql,
                    &result.columns,
                    &result.rows,
                    &part_title,
                )
                .await;
                if packaged.get("error").is_some() {
                    return json!({
                        "error": packaged.get("error").cloned().unwrap_or(json!("فشل التنفيذ")),
                        "row_count": result.row_count,
                        "part_index": idx + 1
                    });
                }
                total_rows += result.row_count;
                last_sql = sql;
                last_columns = result.columns.clone();
                last_rows = result.rows.clone();
                let mut part_obj = packaged.as_object().cloned().unwrap_or_default();
                part_obj.insert("part".to_string(), json!(idx + 1));
                parts.push(Value::Object(part_obj));
            }
            Err(e) => {
                return json!({
                    "error": format!("فشل الجزء {}: {}", idx + 1, e),
                    "part_index": idx + 1,
                    "sql_attempted": sql
                });
            }
        }
    }

    set_last_result(app_state, &last_sql, &last_columns, &last_rows).await;

    if parts.len() == 1 {
        let mut p = parts[0].clone();
        if let Some(obj) = p.as_object_mut() {
            obj.insert("pattern_keywords".to_string(), json!(keywords));
            if !obj.contains_key("message") {
                obj.insert(
                    "message".to_string(),
                    json!("تم تنفيذ نمط QUERY_PATTERNS بنجاح — لخّص النتائج للمستخدم مباشرة."),
                );
            }
        }
        return p;
    }

    json!({
        "pattern_keywords": keywords,
        "parts": parts,
        "part_count": parts.len(),
        "row_count": total_rows,
        "message": format!(
            "تم تنفيذ {} استعلامات من النمط — لخّص كل جزء (ديون، مصاريف، رواتب…) للمستخدم.",
            parts.len()
        )
    })
}

fn safe_date(d: &str) -> Result<String, String> {
    let t = d.trim();
    if t.len() == 10 && t.chars().filter(|c| *c == '-').count() == 2 {
        if t.chars().all(|c| c.is_ascii_digit() || c == '-') {
            return Ok(t.to_string());
        }
    }
    Err(format!("تاريخ غير صالح: {}", d))
}

pub async fn handle_compare_periods(
    metric: &str,
    p1s: &str,
    p1e: &str,
    p2s: &str,
    p2e: &str,
    app_state: &Arc<AppState>,
) -> Value {
    let d1s = match safe_date(p1s) { Ok(v) => v, Err(e) => return json!({ "error": e }) };
    let d1e = match safe_date(p1e) { Ok(v) => v, Err(e) => return json!({ "error": e }) };
    let d2s = match safe_date(p2s) { Ok(v) => v, Err(e) => return json!({ "error": e }) };
    let d2e = match safe_date(p2e) { Ok(v) => v, Err(e) => return json!({ "error": e }) };

    let sql = match metric.to_lowercase().as_str() {
        "sales" | "مبيعات" => format!(
            r";WITH P1 AS (
  SELECT N'الفترة 1' AS Period, COUNT(DISTINCT INV.S_ID) AS InvoiceCount,
    CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS TotalValue
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  WHERE CAST(INV.S_DATE AS date) BETWEEN '{d1s}' AND '{d1e}'
), P2 AS (
  SELECT N'الفترة 2' AS Period, COUNT(DISTINCT INV.S_ID) AS InvoiceCount,
    CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS TotalValue
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  WHERE CAST(INV.S_DATE AS date) BETWEEN '{d2s}' AND '{d2e}'
)
SELECT * FROM P1 UNION ALL SELECT * FROM P2",
            d1s = d1s,
            d1e = d1e,
            d2s = d2s,
            d2e = d2e
        ),
        "purchases" | "مشتريات" | "شراء" => format!(
            r";WITH P1 AS (
  SELECT N'الفترة 1' AS Period, COUNT(DISTINCT B.B_ID) AS InvoiceCount,
    CAST(SUM(BI.QTY * BI.PRICE) AS decimal(18,2)) AS TotalValue
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
  WHERE CAST(B.B_DATE AS date) BETWEEN '{d1s}' AND '{d1e}'
), P2 AS (
  SELECT N'الفترة 2' AS Period, COUNT(DISTINCT B.B_ID) AS InvoiceCount,
    CAST(SUM(BI.QTY * BI.PRICE) AS decimal(18,2)) AS TotalValue
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
  WHERE CAST(B.B_DATE AS date) BETWEEN '{d2s}' AND '{d2e}'
)
SELECT * FROM P1 UNION ALL SELECT * FROM P2",
            d1s = d1s,
            d1e = d1e,
            d2s = d2s,
            d2e = d2e
        ),
        _ => {
            return json!({ "error": "metric يجب أن يكون sales أو purchases." });
        }
    };

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات." });
    }

    match execute_sql_query(conn_opt.unwrap(), sql.clone()).await {
        Ok(result) => {
            set_last_result(app_state, &sql, &result.columns, &result.rows).await;
            json!({
                "metric": metric,
                "columns": result.columns,
                "rows": result.rows,
                "row_count": result.row_count
            })
        }
        Err(e) => json!({ "error": format!("{}", e) }),
    }
}

pub fn handle_suggest_indexes(sql: &str) -> Value {
    let upper = sql.to_uppercase();
    let mut suggestions: Vec<String> = Vec::new();

    if upper.contains("ITEMS_SUB") && upper.contains("CATEOGRY3") {
        suggestions.push(
            "ITEMS_SUB.CATEOGRY3 — فهرس موجود غالباً؛ فلتر الصلاحية يستفيد منه.".to_string(),
        );
    }
    if upper.contains("SALE_ITEMS") && upper.contains("SALE_INVOICE") && upper.contains("S_DATE") {
        suggestions.push(
            "SALE_INVOICE(S_DATE) + SALE_ITEMS(S_ID) — اربط دائماً على S_ID للفلترة بالتاريخ.".to_string(),
        );
    }
    if upper.contains("ITEM_NAME") || upper.contains("ITEM_MODEL") {
        suggestions.push(
            "ITEMS(ITEM_NAME, ITEM_MODEL) — للبحث LIKE استخدم N'%x%'؛ فهرس نصي اختياري.".to_string(),
        );
    }
    if upper.contains("CUST_ID") && upper.contains("TAKE") {
        suggestions.push("TAKE(CUST_ID, T_DATE) — لتسريع مقبوضات الزبائن.".to_string());
    }
    if upper.contains("CUST_ID") && upper.contains("GIVE") {
        suggestions.push("GIVE(CUST_ID, G_DATE) — لتسريع مدفوعات الموردين.".to_string());
    }
    if upper.contains("BUY_ITEMS") && upper.contains("ITEM_ID") {
        suggestions.push("BUY_ITEMS(ITEM_ID, B_ID) — لآخر سعر شراء per item.".to_string());
    }

    if suggestions.is_empty() {
        suggestions.push(
            "لم تُكتشف قواعد محددة — راجع Execution Plan في SSMS للاستعلام البطيء.".to_string(),
        );
    }

    json!({
        "suggestions": suggestions,
        "note": "اقتراحات heuristic لـ Marketing2026 — ليست بديلاً عن تحليل SSMS."
    })
}

pub enum ExportDelivery {
    Local,
    Telegram {
        client: Client,
        token: String,
        chat_id: i64,
    },
}

pub async fn handle_export_last_result(
    title: &str,
    format: &str,
    app_state: &Arc<AppState>,
    delivery: ExportDelivery,
) -> Value {
    let stored = {
        let session = app_state.agent_session.lock().await;
        session.last_result.clone()
    };

    let Some(data) = stored else {
        return json!({ "error": "لا توجد نتيجة سابقة. نفّذ execute_raw_sql أو run_query_pattern أولاً." });
    };

    if data.rows.is_empty() {
        return json!({ "error": "آخر نتيجة فارغة — لا يمكن التصدير." });
    }

    let fmt = format.to_lowercase();
    match fmt.as_str() {
        "pdf" => {
            match generate_report_pdf(title, &data.columns, &data.rows) {
                Ok(bytes) => match delivery {
                    ExportDelivery::Local => {
                        let filename = safe_export_filename(title, "pdf");
                        let path = std::env::temp_dir().join(&filename);
                        if std::fs::write(&path, &bytes).is_ok() {
                            record_exported_file(app_state, &path).await;
                            json!({
                                "result": format!("تم حفظ PDF. أخبر المستخدم وأضف: [FILE_PATH:{}]", path.display())
                            })
                        } else {
                            json!({ "error": "فشل حفظ PDF محلياً." })
                        }
                    }
                    ExportDelivery::Telegram { client, token, chat_id } => {
                        let filename = format!("{}.pdf", title.chars().take(25).collect::<String>().replace(' ', "_"));
                        let caption = format!("📊 {}", title);
                        if let Err(e) = send_pdf(&client, &token, chat_id, &filename, bytes, &caption).await {
                            json!({ "error": format!("{}", e) })
                        } else {
                            json!({ "result": "تم إرسال PDF من آخر نتيجة إلى Telegram." })
                        }
                    }
                },
                Err(e) => json!({ "error": e }),
            }
        }
        "excel" | "xlsx" => {
            match generate_report_excel(title, &data.columns, &data.rows) {
                Ok(bytes) => match delivery {
                    ExportDelivery::Local => {
                        let filename = safe_export_filename(title, "xlsx");
                        let path = std::env::temp_dir().join(&filename);
                        if std::fs::write(&path, &bytes).is_ok() {
                            record_exported_file(app_state, &path).await;
                            json!({
                                "result": format!("تم حفظ Excel. أخبر المستخدم وأضف: [FILE_PATH:{}]", path.display())
                            })
                        } else {
                            json!({ "error": "فشل حفظ Excel محلياً." })
                        }
                    }
                    ExportDelivery::Telegram { client, token, chat_id } => {
                        let filename = format!("{}.xlsx", title.chars().take(25).collect::<String>().replace(' ', "_"));
                        let caption = format!("📊 {}", title);
                        if let Err(e) = send_excel(&client, &token, chat_id, &filename, bytes, &caption).await {
                            json!({ "error": format!("{}", e) })
                        } else {
                            json!({ "result": "تم إرسال Excel من آخر نتيجة إلى Telegram." })
                        }
                    }
                },
                Err(e) => json!({ "error": e }),
            }
        }
        _ => json!({ "error": "format يجب أن يكون pdf أو excel." }),
    }
}

#[cfg(test)]
mod product_filter_tests {
    use super::*;

    #[test]
    fn sanitize_strips_empty_parens_from_mention() {
        assert_eq!(
            sanitize_product_filter("@TRADOL 10ML (TUNISIA) ()"),
            "TRADOL 10ML (TUNISIA)"
        );
    }

    #[test]
    fn sanitize_strips_at_prefix() {
        assert_eq!(sanitize_product_filter("@PARACETAMOL"), "PARACETAMOL");
    }

    #[test]
    fn extract_from_mention_in_message() {
        let pf = extract_product_filter_from_text("اعرض موردي @TRADOL 10ML (TUNISIA) ()")
            .unwrap();
        assert_eq!(pf, "TRADOL 10ML (TUNISIA)");
    }

    #[test]
    fn apply_product_filter_keeps_wildcards() {
        let sql = "WHERE (I.ITEM_MODEL LIKE N'%PRODUCT%' OR I.ITEM_NAME LIKE N'%PRODUCT%')";
        let out = apply_product_filter(sql, "TRADOL 10ML (TUNISIA) ()");
        assert!(out.contains("TRADOL"));
        assert!(out.contains("10ML"));
        assert!(!out.contains("%PRODUCT%"));
    }

    #[test]
    fn tokenized_match_finds_artamin_with_extra_spaces_in_db() {
        let sql = "WHERE (I.ITEM_MODEL LIKE N'%PRODUCT%' OR I.ITEM_NAME LIKE N'%PRODUCT%')";
        let out = apply_product_filter(sql, "ARTAMIN 250MG-USP AUSTRIA");
        assert!(out.contains("ARTAMIN"));
        assert!(out.contains("250MG-USP"));
        assert!(out.contains("AUSTRIA"));
        assert!(out.contains(" AND "));
    }
}

pub async fn dispatch_extended_tool(
    name: &str,
    args_str: &str,
    app_state: &Arc<AppState>,
    delivery: ExportDelivery,
) -> Option<Value> {
    if !is_extended_tool(name) {
        return None;
    }

    let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
    let result = match name {
        "validate_sql" => {
            let sql = args.get("sql_query").and_then(|v| v.as_str()).unwrap_or("");
            handle_validate_sql(sql).await
        }
        "explain_sql" => {
            let sql = args.get("sql_query").and_then(|v| v.as_str()).unwrap_or("");
            handle_explain_sql(sql)
        }
        "get_table_sample" => {
            let table = args.get("table_name").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("row_limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
            handle_get_table_sample(table, limit, app_state).await
        }
        "save_favorite_query" => {
            let n = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let sql = args.get("sql_query").and_then(|v| v.as_str()).unwrap_or("");
            let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
            handle_save_favorite(n, sql, desc)
        }
        "list_favorite_queries" => handle_list_favorites(),
        "run_query_pattern" => {
            let kw = args.get("keywords").and_then(|v| v.as_str()).unwrap_or("");
            let dr = args.get("days_recent").and_then(|v| v.as_u64()).map(|v| v as u32);
            let cov = args.get("coverage_days").and_then(|v| v.as_u64()).map(|v| v as u32);
            let explicit_pf = args.get("product_filter").and_then(|v| v.as_str());
            let session_pf = {
                let s = app_state.agent_session.lock().await;
                s.last_product_filter.clone()
            };
            let pf = resolve_product_filter(explicit_pf, session_pf.as_deref());
            handle_run_query_pattern(kw, dr, cov, pf.as_deref(), app_state, &delivery).await
        }
        "get_product_schema" => {
            let sec = args.get("section").and_then(|v| v.as_str());
            handle_get_product_schema(sec)
        }
        "get_database_views" => {
            let sec = args.get("section").and_then(|v| v.as_str());
            handle_get_database_views(sec)
        }
        "plan_complex_query" => {
            let q = args.get("question").and_then(|v| v.as_str()).unwrap_or("");
            let explicit_pf = args.get("product_filter").and_then(|v| v.as_str());
            let session_pf = {
                let s = app_state.agent_session.lock().await;
                s.last_product_filter.clone()
            };
            let pf = resolve_product_filter(explicit_pf, session_pf.as_deref());
            let dr = args.get("days_recent").and_then(|v| v.as_u64()).map(|v| v as u32);
            let cov = args.get("coverage_days").and_then(|v| v.as_u64()).map(|v| v as u32);
            handle_plan_complex_query(q, pf.as_deref(), dr, cov, app_state).await
        }
        "execute_query_plan" => {
            let steps = args.get("steps").and_then(|v| v.as_array()).cloned();
            let stop = args
                .get("stop_on_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            handle_execute_query_plan(steps, stop, app_state, &delivery).await
        }
        "compare_periods" => {
            handle_compare_periods(
                args.get("metric").and_then(|v| v.as_str()).unwrap_or("sales"),
                args.get("period1_start").and_then(|v| v.as_str()).unwrap_or(""),
                args.get("period1_end").and_then(|v| v.as_str()).unwrap_or(""),
                args.get("period2_start").and_then(|v| v.as_str()).unwrap_or(""),
                args.get("period2_end").and_then(|v| v.as_str()).unwrap_or(""),
                app_state,
            )
            .await
        }
        "suggest_indexes" => {
            let sql = args.get("sql_query").and_then(|v| v.as_str()).unwrap_or("");
            handle_suggest_indexes(sql)
        }
        "export_last_result" => {
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("تقرير");
            let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("excel");
            handle_export_last_result(title, format, app_state, delivery).await
        }
        _ => return None,
    };

    Some(result)
}
