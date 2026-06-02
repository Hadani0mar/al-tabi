use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{AppState, execute_sql_query};
use crate::telegram::{send_message, send_html, send_pdf, send_excel, SupabaseReport, search_schema};
use crate::excel_generator::generate_report_excel;
use crate::pdf_generator::generate_report_pdf;
use tauri::Emitter;
use tauri_plugin_store::StoreExt;

/// النموذج الثابت — لا يُقرأ من الإعدادات
/// النموذج الافتراضي — مدفوع على OpenRouter (أدوات + SQL + عربية)
pub const DEFAULT_AI_MODEL: &str = "minimax/minimax-m3";

/// احتياطي مدفوع عند تعذّر النموذج أو rate limit
pub const OPENROUTER_PAID_MODEL_FALLBACKS: &[&str] = &[
    "minimax/minimax-m3",
];
const RATE_LIMIT_RETRIES_PER_MODEL: u8 = 2;
const RATE_LIMIT_BASE_DELAY_MS: u64 = 2000;
/// حد افتراضي منخفض لتجنب خطأ OpenRouter 402 (رصيد غير كافٍ لـ max_tokens كبير)
pub const DEFAULT_MAX_TOKENS: u32 = 4096;
/// حد أقصى لحجم مخطط RAG داخل system prompt (تجنب تجاوز حد OpenRouter)
const PROMPT_SCHEMA_CHAR_LIMIT: usize = 2000;
const HISTORY_MSG_CHAR_LIMIT: usize = 800;
const TOOL_RESULT_ROW_LIMIT: usize = 8;
#[allow(dead_code)]
const RAG_MATCH_COUNT: u32 = 5;
#[allow(dead_code)]
const RAG_DOC_CHAR_LIMIT: usize = 1200;

// ── كاش ملفات AGENT (تعاليم + أنماط) حسب نوع ERP — التحميل في erp_profile ──

/// يستخرج قسم نمط واحد من ملف AGENT حسب slug (مثل `عدد-المنتجات`)
pub fn extract_pattern_section_by_slug(
    slug: &str,
    erp: crate::erp_profile::ErpKind,
) -> Option<String> {
    let content = crate::erp_profile::load_agent_patterns(erp);
    let target = slug.trim();
    for section in content.split("\n## PATTERN:").skip(1) {
        let header = section.lines().next()?.trim();
        if header == target {
            return Some(format!("## PATTERN:{section}"));
        }
    }
    None
}

/// يبحث عن النمط الأنسب بمطابقة الكلمات المفتاحية ويُعيد أقصى قسمين مطابقَين
pub fn search_query_patterns_local(keywords: &str, erp: crate::erp_profile::ErpKind) -> String {
    let content = crate::erp_profile::load_agent_patterns(erp);
    let file_label = erp.agent_file_label();

    // قسّم الملف إلى أقسام عند ## PATTERN:
    let sections: Vec<&str> = content.split("\n## PATTERN:").collect();
    if sections.len() <= 1 {
        return format!(
            "لم يُعثر على ملف {} أو هو فارغ.\n\n{}",
            file_label,
            content.lines().take(3).collect::<Vec<_>>().join("\n")
        );
    }

    let kw_lower = keywords.to_lowercase();
    let kw_words: Vec<&str> = kw_lower.split_whitespace().collect();
    let customer_intent = kw_lower.contains("عملاء")
        || kw_lower.contains("زبون")
        || kw_lower.contains("زبائن")
        || kw_lower.contains("customer");
    let employee_intent = kw_lower.contains("موظف") || kw_lower.contains("employee");
    let products_sold_today_intent = (kw_lower.contains("منتج")
        || kw_lower.contains("منتجات")
        || kw_lower.contains("صنف")
        || kw_lower.contains("أصناف")
        || kw_lower.contains("product"))
        && (kw_lower.contains("بيع")
            || kw_lower.contains("مبيع")
            || kw_lower.contains("sold")
            || kw_lower.contains("sales"))
        && (kw_lower.contains("اليوم")
            || kw_lower.contains("today")
            || kw_lower.contains("آخر"));

    // احسب درجة تطابق كل قسم
    let mut scored: Vec<(usize, &str)> = sections.iter().skip(1).map(|section| {
        let section_lower = section.to_lowercase();
        let header_lines: String = section_lower.lines().take(3).collect::<Vec<_>>().join(" ");
        let mut score = kw_words.iter()
            .map(|w| {
                if header_lines.contains(w) { 3 } else if section_lower.contains(w) { 1 } else { 0 }
            })
            .sum::<usize>();

        if customer_intent {
            if header_lines.contains("عملاء")
                || header_lines.contains("زبون")
                || header_lines.contains("customer")
            {
                score += 8;
            }
            if section_lower.contains("منتج") && !header_lines.contains("عملاء") {
                score = score.saturating_sub(5);
            }
        }
        if employee_intent && header_lines.contains("موظف") {
            score += 5;
        }
        if products_sold_today_intent {
            if header_lines.contains("آخر-منتجات-بيعت-اليوم")
                || header_lines.contains("منتجات بيعت اليوم")
            {
                score += 12;
            }
            if employee_intent && header_lines.contains("موظف") && !header_lines.contains("منتج") {
                score = score.saturating_sub(4);
            }
        }
        if employee_intent && header_lines.contains("عملاء") {
            score = score.saturating_sub(3);
        }

        (score, *section)
    }).collect();

    // رتّب تنازلياً ثم خذ أفضل قسمين
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let top: Vec<&str> = scored.iter()
        .filter(|(s, _)| *s > 0)
        .take(2)
        .map(|(_, sec)| *sec)
        .collect();

    if top.is_empty() {
        format!("لم يُعثر على نمط مطابق للكلمات: «{}»\nالأنماط المتاحة: {}", keywords,
            sections.iter().skip(1)
                .filter_map(|s| s.lines().next())
                .collect::<Vec<_>>().join(", "))
    } else {
        top.iter()
            .map(|sec| format!("## PATTERN:{}", sec))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

/// تاريخ مضغوط للحقن في system prompt — يُغني عن tool call لـ get_current_datetime
pub fn compact_date_for_prompt() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let local_secs = secs + 2 * 3600;
    let days_since_epoch = local_secs / 86400;
    let mut remaining = days_since_epoch;
    let mut year: u64 = 1970;
    loop {
        let dy = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 366 } else { 365 };
        if remaining < dy { break; }
        remaining -= dy;
        year += 1;
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let dpm = [31u64, if is_leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month: u64 = 1;
    for &d in &dpm { if remaining < d { break; } remaining -= d; month += 1; }
    let day = remaining + 1;
    let dow = (days_since_epoch + 4) % 7;
    let weekday_ar = ["الأحد","الاثنين","الثلاثاء","الأربعاء","الخميس","الجمعة","السبت"][dow as usize % 7];
    format!("{weekday_ar} {day}/{month}/{year} | الشهر:{month} | السنة:{year}")
}

/// يُعيد التاريخ والوقت الحالي كاملاً بصيغة عربية ورقمية
fn get_current_datetime_info() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // نحصل على الوقت UTC ثم نضيف فارق التوقيت الليبي (UTC+2)
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // UTC+2 للتوقيت الليبي
    let local_secs = secs + 2 * 3600;

    let s   = local_secs % 60;
    let m   = (local_secs / 60) % 60;
    let h   = (local_secs / 3600) % 24;
    let days_since_epoch = local_secs / 86400;

    // حساب التاريخ الميلادي من الأيام منذ 1970-01-01
    let mut remaining = days_since_epoch;
    let mut year: u64 = 1970;
    loop {
        let days_in_year = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        year += 1;
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_per_month = [31u64,if is_leap {29} else {28},31,30,31,30,31,31,30,31,30,31];
    let mut month: u64 = 1;
    for &d in &days_per_month {
        if remaining < d { break; }
        remaining -= d;
        month += 1;
    }
    let day = remaining + 1;

    // يوم الأسبوع (0=الأحد)
    let dow = (days_since_epoch + 4) % 7; // 1970-01-01 كان الخميس (4)
    let weekday_ar = match dow {
        0 => "الأحد",
        1 => "الاثنين",
        2 => "الثلاثاء",
        3 => "الأربعاء",
        4 => "الخميس",
        5 => "الجمعة",
        6 => "السبت",
        _ => "؟",
    };
    let month_ar = match month {
        1 => "يناير", 2 => "فبراير", 3 => "مارس", 4 => "أبريل",
        5 => "مايو",  6 => "يونيو",  7 => "يوليو", 8 => "أغسطس",
        9 => "سبتمبر", 10 => "أكتوبر", 11 => "نوفمبر", 12 => "ديسمبر",
        _ => "؟",
    };
    let period_ar = if h < 12 { "صباحاً" } else { "مساءً" };
    let h12 = if h == 0 { 12 } else if h > 12 { h - 12 } else { h };

    format!(
        "التاريخ والوقت الحالي (توقيت ليبيا UTC+2):\n\
         - التاريخ: {weekday_ar} {day}/{month}/{year}\n\
         - اليوم: {weekday_ar}\n\
         - الشهر: {month} ({month_ar})\n\
         - السنة: {year}\n\
         - الوقت: {:02}:{:02}:{:02} {period_ar}\n\
         - الوقت 24h: {:02}:{:02}:{:02}\n\
         - اليوم من الشهر: {day}\n\
         - رقم الشهر: {month}\n\
         - SQL للشهر الحالي: MONTH(GETDATE())={month} AND YEAR(GETDATE())={year}",
        h12, m, s, h, m, s
    )
}

#[allow(dead_code)]
fn merge_ddl_schemas(existing: &str, new_ddl: &str) -> String {
    let mut merged_tables = Vec::new();
    let mut merged_ddls = Vec::new();

    let mut process_block = |block: &str| {
        for chunk in block.split("\n\n") {
            let chunk_trim = chunk.trim();
            if chunk_trim.is_empty() {
                continue;
            }
            if chunk_trim.starts_with("⚠️") || chunk_trim.starts_with("✅") || chunk_trim.contains("تعليمات مهمة") || chunk_trim.contains("الآن استخدم") {
                continue;
            }

            let mut table_name = String::new();
            if let Some(idx) = chunk_trim.find("dbo.") {
                let start = idx;
                let mut end = start + 4;
                let bytes = chunk_trim.as_bytes();
                while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'[' || bytes[end] == b']') {
                    end += 1;
                }
                if end > start {
                    table_name = chunk_trim[start..end].to_uppercase();
                }
            }

            if !table_name.is_empty() {
                if !merged_tables.contains(&table_name) {
                    merged_tables.push(table_name);
                    merged_ddls.push(chunk_trim.to_string());
                }
            } else {
                let snippet = if chunk_trim.len() > 50 { &chunk_trim[..50] } else { chunk_trim };
                let already_exists = merged_ddls.iter().any(|d| d.contains(snippet));
                if !already_exists {
                    merged_ddls.push(chunk_trim.to_string());
                }
            }
        }
    };

    process_block(existing);
    process_block(new_ddl);

    merged_ddls.join("\n\n")
}

fn truncate_prompt_text(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let head: String = text.chars().take(max_chars).collect();
    format!(
        "{}\n...[تم اختصار {} حرفاً لتقليل حجم الطلب]",
        head,
        count - max_chars
    )
}

fn prepare_schema_for_system_prompt(rag_schema: &str) -> String {
    truncate_prompt_text(rag_schema.trim(), PROMPT_SCHEMA_CHAR_LIMIT)
}

fn read_u64_path(value: &Value, path: &str) -> Option<u64> {
    value.pointer(path).and_then(|v| v.as_u64())
}

fn openrouter_usage_payload(request_id: &str, res_json: &Value) -> Value {
    let generation = res_json.get("openrouter_generation").unwrap_or(&Value::Null);
    let usage = res_json.get("usage").unwrap_or(&Value::Null);
    let model = res_json
        .get("model")
        .and_then(|v| v.as_str())
        .or_else(|| generation.get("model").and_then(|v| v.as_str()))
        .unwrap_or(DEFAULT_AI_MODEL);

    let prompt_tokens = read_u64_path(generation, "/native_tokens_prompt")
        .or_else(|| read_u64_path(generation, "/tokens_prompt"))
        .or_else(|| usage.get("prompt_tokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let completion_tokens = read_u64_path(generation, "/native_tokens_completion")
        .or_else(|| read_u64_path(generation, "/tokens_completion"))
        .or_else(|| usage.get("completion_tokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt_tokens + completion_tokens);
    let usage_source = if generation.is_object() {
        "openrouter_generation"
    } else {
        "chat_completion_usage"
    };

    json!({
        "requestId": request_id,
        "model": model,
        "promptTokens": prompt_tokens,
        "completionTokens": completion_tokens,
        "totalTokens": total_tokens,
        "usageSource": usage_source,
        "generationId": res_json.get("id").and_then(|v| v.as_str()),
        "cost": generation.get("total_cost").or_else(|| generation.get("usage")),
    })
}

fn emit_openrouter_usage(app_handle: &tauri::AppHandle, request_id: &str, res_json: &Value) {
    let payload = openrouter_usage_payload(request_id, res_json);

    let _ = app_handle.emit(
        "ai-usage",
        payload,
    );
}

fn trim_message_content(content: &str, max_chars: usize) -> String {
    truncate_prompt_text(content, max_chars)
}

fn trim_history_for_api(history: &mut [Value]) {
    for msg in history.iter_mut() {
        if msg.get("role").and_then(|r| r.as_str()) == Some("system") {
            continue;
        }
        if let Some(s) = msg.get("content").and_then(|c| c.as_str()) {
            if s.chars().count() > HISTORY_MSG_CHAR_LIMIT {
                let trimmed = trim_message_content(s, HISTORY_MSG_CHAR_LIMIT);
                msg["content"] = json!(trimmed);
            }
        }
    }
}

/// يضغط نتيجة tool قبل حفظها في التاريخ — يحتفظ بأول TOOL_RESULT_ROW_LIMIT صف فقط.
/// النتيجة الكاملة محفوظة في AppState.agent_session.last_result للتصدير.
fn compress_tool_result(tool_content: &str) -> String {
    if tool_content.len() <= 800 {
        return tool_content.to_string();
    }
    let Ok(mut v) = serde_json::from_str::<Value>(tool_content) else {
        // ليس JSON — اقتطع مباشرةً
        let s: String = tool_content.chars().take(800).collect();
        return format!("{}…", s);
    };
    // أخطاء → أعد كما هي (قصيرة عادةً)
    if v.get("error").is_some() {
        return tool_content.to_string();
    }
    // اقتطع الصفوف مع الإبقاء على row_count الحقيقي
    if let Some(rows) = v.get("rows").and_then(|r| r.as_array()) {
        let total = rows.len();
        if total > TOOL_RESULT_ROW_LIMIT {
            let kept: Vec<_> = rows.iter().take(TOOL_RESULT_ROW_LIMIT).cloned().collect();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("rows".into(), serde_json::Value::Array(kept));
                obj.insert("row_count".into(), json!(total));
                obj.insert("rows_shown".into(), json!(TOOL_RESULT_ROW_LIMIT));
                obj.remove("sql"); // لا داعي لإرسال SQL للنموذج ثانيةً
            }
        }
    }
    let s = v.to_string();
    if s.chars().count() > 900 {
        let cap: String = s.chars().take(900).collect();
        format!("{}…", cap)
    } else {
        s
    }
}

/// Some models return `"tool_calls": []` with final text — treat as no tools.
fn message_tool_calls(message: &Value) -> Option<&Vec<Value>> {
    message
        .get("tool_calls")
        .and_then(|tc| tc.as_array())
        .filter(|tc| !tc.is_empty())
}

/// بصمة دلالية للاستعلام: الجداول الرئيسية (FROM/JOIN) + كلمات الفلترة الأساسية.
/// تكشف التكرار حتى لو غيّر النموذج subquery إلى LEFT JOIN أو رتّب الأعمدة.
fn sql_semantic_fingerprint(sql: &str) -> String {
    let lower = sql.to_lowercase();

    let mut tables: Vec<String> = Vec::new();
    for marker in &["from ", "join "] {
        let mut rest = lower.as_str();
        while let Some(idx) = rest.find(marker) {
            let after = &rest[idx + marker.len()..];
            let mut end = 0usize;
            for (i, c) in after.char_indices() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']' {
                    end = i + c.len_utf8();
                } else {
                    break;
                }
            }
            if end > 0 {
                let tbl = after[..end]
                    .trim_start_matches("dbo.")
                    .trim_matches(|c: char| c == '[' || c == ']')
                    .to_string();
                if !tbl.is_empty() && !tables.contains(&tbl) {
                    tables.push(tbl);
                }
            }
            rest = &after[end..];
        }
    }
    tables.sort();

    let mut filter_keys: Vec<String> = Vec::new();
    let interesting_columns = [
        "cust_emp", "cust_vendor", "cust_custom", "cust_id", "users_id",
        "item_id", "store_id", "s_date", "b_date", "g_date", "g_statues",
        "expences_id", "wait", "item_invisible", "qty", "min_level",
        "cateogry3", "s_statues",
    ];
    for col in interesting_columns {
        if lower.contains(col) {
            filter_keys.push(col.to_string());
        }
    }
    filter_keys.sort();

    format!("T:{}|F:{}", tables.join(","), filter_keys.join(","))
}

const SLIM_DOMAIN_CRITICAL_FACTS: &str = r#"<DOMAIN_CRITICAL_FACTS>
### جداول أساسية (أعمدة حرجة)
- **ITEMS**: ITEM_ID, ITEM_MODEL, ITEM_NAME, LAST_COST, AVER_COST, MIN_LEVEL, MAX_LEVEL
- **ITEMS_SUB**: ITEM_ID, STORE_ID, QTY, CATEOGRY1=Batch, CATEOGRY3=Expiry ⭐
- **SALE_INVOICE**: S_ID, S_DATE ⭐, CUST_ID, USERS_ID, CUST_NAME
- **SALE_ITEMS**: S_ITEM_ID, S_ID→SALE_INVOICE (⚠️ لا S_DATE), ITEM_ID, QTY, PRICE
- **BUY_INVOICE**: B_ID, B_DATE, CUST_ID; **BUY_ITEMS**: B_ID, ITEM_ID, QTY, PRICE
- **CUSTOMERS**: CUST_ID, CUST_NAME, CUST_VENDOR, CUST_CUSTOM, CUST_EMP, BL_DEBIT, BL_CREDIT
- **USERS**: USERS_ID, FULL_NAME | **STORES**: STORE_ID, STORE_NAME
- **GIVE**: G_ID, G_DATE, G_VALUE, G_STATUES, EXPENCES_ID, CUST_ID, USERS_ID
  - EXPENCES_ID=1 رواتب | >0 و<>1 مصروف | =0 دفع مورد (ليس مصروفاً)
- **TAKE**: تحصيلات | **R_S_***/**B_R_***: مردودات مبيعات/مشتريات
- **BALANCE_C**: ⚠️ فارغ — لا تستخدم للديون
- **View مبيعات**: dbo.SALE_ITEMS_INVOICE_VIEW (S_DATE, FULL_NAME, QTY, PRICE, ITEM_NAME, CUST_NAME)

### قواعد سريعة
- الكمية = QTY (ليس Quantity) | CATEOGRY3 = تاريخ الصلاحية
- بحث منتج: LIKE N'%name%' | @AsOfDate = MAX(S_DATE) — لا hardcode
- COMMISSIONER غير مستخدم | SALARIES فارغ — الرواتب من GIVE WHERE EXPENCES_ID=1

### أنماط run_query_pattern (keywords)
| السؤال | keywords |
| مبيعات اليوم/آخر يوم/موظف | مبيعات آخر يوم موظف / مبيعات يومية موظف |
| **آخر منتجات/أصناف بيعت اليوم** | **آخر منتجات بيعت اليوم** |
| أفضل عملاء / أكثر زبائن مبيعاً | أفضل عملاء مبيعات |
| ديون لي وعلي | متابعة الديون |
| ديون الزباين فقط + آخر إيصال قبض + الإجمالي | ديون الزباين |
| ديون الموردين فقط | ديون الموردين |
| مصاريف/رواتب شهرية | ملخص مالي شهري |
| نواقص + مورد + سعر | نواقص نشطة مورد |
| متابعة نواقص | متابعة النواقص |
| مقارنة أسعار موردين (+ product_filter) | مقارنة أسعار موردين |
| طلبية شراء | طلبية شراء ذكية |
| صلاحية/منتهية | تقرير الصلاحية |
| تفاصيل منتج | تفاصيل منتج وحدات أسعار |
| حركة صنف | حركة صنف تفصيلية |
| مردودات / إرجاع | مردودات مبيعات / مردودات مشتريات |
| مقبوضات / تحصيل | مقبوضات تحصيلات |
| مدفوعات مورد | مدفوعات موردين سندات |
| تحويل مخزن | تحويلات مخازن |
| تالف / متلف | أصناف تالفة |
| راكد / بطيء | أصناف راكة |
| مقارنة أشهر | مقارنة مبيعات شهرية |
| سجل زبون | سجل مبيعات عميل |
| فواتير شراء | فواتير شراء حديثة |
| بحث منتج | بحث منتج سريع |
| مخزون حسب مخزن | جرد مخزون حسب المخزن |
| أكثر مبيعاً | أعلى منتجات مبيعاً |
| مبيعات مخزن | مبيعات حسب المخزن |
</DOMAIN_CRITICAL_FACTS>"#;

const SLIM_DOMAIN_CRITICAL_FACTS_INFINITY: &str = r#"<DOMAIN_CRITICAL_FACTS>
### ERP: InfinityRetailDB (Inventory, SALES, Purchase, MyCompany)
- **Inventory.Data_Products**: ProductCode, ProductName, SalesDecription, StockOnHand, Min/MaxStockLevel, IsInActive
- **Inventory.Data_ProductInventories**: StockOnHand, ExpiryDate, BranchID_FK
- **Inventory.Data_View_ProductUOMBarcodes**: ProductBarcode, UomPrice1–4, UOMName, UomLastCost
- **SALES.Data_SalesInvoices**: SalesInvoiceDate ⭐, CustomerID_FK, CreatedByUserName, InvoiceNumber
- **SALES.Data_SalesInvoiceItems / Data_View_SalesInvoiceItems**: ProductName, QYT, UnitPrice, UOMName
- **SALES.Data_Customers**: CustomerName, CustomerOutstanding
- **Purchase.Data_Suppliers / Data_PurchaseInvoices / Data_PurchaseInvoiceItems**: SupplierName, InvoiceDate, UnitCost, QYT
- **MyCompany.Config_Branchs**: BranchName
- ⚠️ لا dbo.ITEMS ولا SALE_INVOICE — جداول Marketing غير موجودة هنا

### قواعد سريعة
- بحث منتج: ProductName/ProductCode LIKE N'%name%' | @AsOfDate = MAX(SalesInvoiceDate)
- إيراد = SUM(QYT * UnitPrice) | الصلاحية = Data_ProductInventories.ExpiryDate
- المورد = Purchase.Data_Suppliers عبر آخر فاتورة شراء

### أنماط run_query_pattern (keywords)
| السؤال | keywords |
| منتجات بيعت اليوم | آخر منتجات بيعت اليوم |
| مبيعات آخر يوم / موظف | مبيعات آخر يوم موظف / مبيعات يومية موظف |
| نواقص نشطة + مورد + سعر | نواقص نشطة مورد |
| متابعة نواقص | متابعة النواقص |
| مقارنة أسعار موردين (+ product_filter) | مقارنة أسعار موردين |
| طلبية شراء | طلبية شراء ذكية |
| صلاحية | تقرير الصلاحية |
| تفاصيل منتج | تفاصيل منتج وحدات أسعار |
| دراسة منتج | دراسة منتج شاملة |
| حركة صنف | حركة صنف تفصيلية |
| آخر سعر شراء | آخر سعر شراء مورد |
| مبيعات بالوحدة | مبيعات منتج حسب الوحدة |
| ربحية / أفضل مبيعات | تحليل المبيعات والربحية |
| أفضل عملاء | أفضل عملاء مبيعات |
| جرد فرع | جرد المخزون حسب الفرع |
| ديون زبائن | متابعة الديون |
| ديون موردين | ديون الموردين |
| ملخص شهري | ملخص مالي شهري |
| مردودات / إرجاع | مردودات مبيعات / مردودات مشتريات |
| تحصيلات | تحصيلات عملاء |
| سندات دفع | سندات دفع مالية |
| تحويل مخزن | تحويلات مخزون |
| تالف / متلف | أصناف تالفة متلفة |
| تسوية جرد | تسوية جرد مخزون |
| راكد / بطيء | أصناف راكة |
| مقارنة أشهر | مقارنة مبيعات شهرية |
| سجل زبون | سجل مبيعات عميل |
| فواتير شراء | فواتير شراء حديثة |
| أكثر مبيعاً | أعلى منتجات مبيعاً |
| مبيعات فرع | مبيعات حسب الفرع |
| مندوب | مبيعات المندوب |
</DOMAIN_CRITICAL_FACTS>"#;

fn slim_domain_facts(erp: crate::erp_profile::ErpKind) -> &'static str {
    match erp {
        crate::erp_profile::ErpKind::InfinityRetailDb => SLIM_DOMAIN_CRITICAL_FACTS_INFINITY,
        _ => SLIM_DOMAIN_CRITICAL_FACTS,
    }
}

fn build_fast_system_prompt(
    _schema_extra: &str,
    product_filter: Option<&str>,
    erp: crate::erp_profile::ErpKind,
) -> String {
    let date_str = compact_date_for_prompt();
    crate::pattern_catalog::build_executor_system_prompt(erp, product_filter, &date_str)
}

fn message_text_content(message: &Value) -> Option<String> {
    message
        .get("content")
        .and_then(|c| {
            if let Some(s) = c.as_str() {
                return Some(s.to_string());
            }
            if let Some(parts) = c.as_array() {
                let joined: String = parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !joined.is_empty() {
                    return Some(joined);
                }
            }
            None
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            message
                .get("reasoning")
                .or_else(|| message.get("reasoning_content"))
                .and_then(|c| c.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

fn trim_chat_history_vec(history: &mut Vec<Value>, max_len: usize) {
    while history.len() > max_len {
        if history.len() > 1 {
            history.remove(1);
        } else {
            break;
        }
    }
}

async fn execute_raw_sql_for_agent(
    sql: &str,
    app_state: &Arc<AppState>,
    delivery: &crate::agent_tools::ExportDelivery,
) -> Value {
    let sql_upper = sql.to_uppercase();
    if sql_upper.contains("INSERT ") || sql_upper.contains("UPDATE ") || sql_upper.contains("DELETE ") 
       || sql_upper.contains("DROP ") || sql_upper.contains("ALTER ") || sql_upper.contains("TRUNCATE ")
       || sql_upper.contains("EXEC ") || sql_upper.contains("GRANT ") || sql_upper.contains("REVOKE ") {
        return json!({ "error": "غير مصرح لك بتنفيذ استعلامات تعديل أو حذف. يُسمح فقط باستعلامات القراءة (SELECT)." });
    }

    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    if let Some(block_msg) = crate::agent_tools::antipattern_block_message(sql, erp) {
        return json!({
            "error": block_msg,
            "hint": "استخدم run_query_pattern(keywords=\"مبيعات يومية موظف\") أو get_database_views() ثم أعد كتابة SQL."
        });
    }

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات المحلية" });
    }
    let conn = conn_opt.unwrap();

    match execute_sql_query(conn, sql.to_string()).await {
        Ok(result) => {
            crate::agent_tools::package_query_result(
                app_state,
                delivery,
                sql,
                &result.columns,
                &result.rows,
                "استعلام SQL",
            )
            .await
        }
        Err(e) => json!({ "error": format!("خطأ في الاستعلام: {}", e) }),
    }
}

fn validate_read_only_select_sql(sql: &str) -> Result<(), String> {
    crate::agent_tools::validate_read_only_sql(sql)
}

async fn explore_local_schema_for_agent(table_hint: &str, app_state: &Arc<AppState>) -> Value {
    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات المحلية" });
    }
    let conn = conn_opt.unwrap();

    let sql = if table_hint.trim().is_empty() {
        // List all tables
        "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_TYPE = 'BASE TABLE'".to_string()
    } else {
        // List columns for a specific table or hint
        let safe_hint = table_hint.replace("'", "''");
        format!("SELECT TABLE_NAME, COLUMN_NAME, DATA_TYPE FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_NAME LIKE '%{}%'", safe_hint)
    };

    match execute_sql_query(conn, sql).await {
        Ok(result) => {
            if result.rows.len() > 150 {
                json!({
                    "error": format!("تم العثور على {} نتيجة وهذا يتجاوز الحد المسموح. يرجى أن تكون أكثر تحديداً في اسم الجدول (table_hint).", result.rows.len())
                })
            } else {
                json!({
                    "columns": result.columns,
                    "rows": result.rows,
                    "row_count": result.row_count
                })
            }
        },
        Err(e) => json!({ "error": format!("خطأ في الاستعلام عن المخطط المحلي: {}", e) })
    }
}

/// Streaming API call — يبعث chunks عبر Tauri events ويرجع النص الكامل كـ Value.
/// يُستخدم فقط لـ iterations التلخيص (tool_choice: none).
async fn call_api_streaming(
    api_key: &str,
    req_body: &Value,
    app_handle: &tauri::AppHandle,
    request_id: &str,
) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap();

    let mut body = req_body.clone();
    body["stream"] = json!(true);
    // streaming لا يحتاج max_tokens عالياً — النموذج يوقف عند الانتهاء
    if body.get("max_tokens").is_none() {
        body["max_tokens"] = json!(DEFAULT_MAX_TOKENS);
    }

    let clean_key = api_key.trim();
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or(DEFAULT_AI_MODEL).to_string();

    let mut response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", clean_key))
        .header("HTTP-Referer", "http://localhost:1420")
        .header("X-Title", "Reports App")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let err = response.text().await.unwrap_or_default();
        // للرسائل القصيرة نرجع الخطأ كاملاً
        let short_err = if err.len() > 300 { &err[..300] } else { &err };
        return Err(format!("HTTP {}: {}", status, short_err));
    }

    let mut buf = String::new();
    let mut full_text = String::new();

    // نقرأ الـ SSE stream chunk بـ chunk
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        buf.push_str(&String::from_utf8_lossy(&chunk));
        // نعالج كل السطور الكاملة في الـ buffer
        loop {
            let Some(nl) = buf.find('\n') else { break };
            let line = buf[..nl].trim_end_matches('\r').to_string();
            buf.drain(..=nl);

            let data = match line.strip_prefix("data: ") {
                Some(d) if d != "[DONE]" && !d.is_empty() => d.to_string(),
                _ => continue,
            };

            let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };

            if let Some(t) = v.pointer("/choices/0/delta/content").and_then(|v| v.as_str()) {
                if !t.is_empty() {
                    full_text.push_str(t);
                    let _ = app_handle.emit("ai-stream-chunk", json!({
                        "requestId": request_id,
                        "delta": t
                    }));
                }
            }
        }
    }

    // أبلغ الواجهة بانتهاء الـ stream
    let _ = app_handle.emit("ai-stream-done", json!({ "requestId": request_id }));

    // أرجع استجابة بنفس صيغة call_groq_api لتسهيل التكامل
    Ok(json!({
        "choices": [{
            "message": { "role": "assistant", "content": full_text },
            "finish_reason": "stop"
        }],
        "model": model,
        "usage": null
    }))
}

async fn call_groq_api(api_key: &str, _ai_model: &str, req_body: &Value) -> Result<Value, String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();
    let url = "https://openrouter.ai/api/v1/chat/completions";
    let models = OPENROUTER_PAID_MODEL_FALLBACKS;
    let mut current_body = req_body.clone();
    let mut max_tokens = DEFAULT_MAX_TOKENS;
    current_body["max_tokens"] = json!(max_tokens);
    let clean_key = api_key.trim();
    eprintln!("[OpenRouter] key len after trim: {} starts_with: '{}'", clean_key.len(), &clean_key[..clean_key.len().min(10)]);
    let mut last_error = String::new();

    for model in models {
        current_body["model"] = json!(model);
        let mut credit_retries = 0u8;
        let mut rate_retries = 0u8;
        eprintln!("[OpenRouter] trying model: {}", model);

        loop {
            current_body["max_tokens"] = json!(max_tokens);

            let res = client
                .post(url)
                .header("Authorization", format!("Bearer {}", clean_key))
                .header("HTTP-Referer", "http://localhost:1420")
                .header("X-Title", "Reports App")
                .json(&current_body)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if !res.status().is_success() {
                let status = res.status();
                let retry_after_secs = parse_retry_after_secs(&res);
                let err_text = res.text().await.unwrap_or_default();
                last_error = err_text.clone();
                if status.as_u16() == 429
                    || err_text.contains("rate_limit")
                    || err_text.contains("Rate limit")
                    || err_text.contains("rate-limited")
                {
                    if rate_retries < RATE_LIMIT_RETRIES_PER_MODEL {
                        rate_retries += 1;
                        let delay_ms = retry_after_secs
                            .map(|s| s.saturating_mul(1000).max(RATE_LIMIT_BASE_DELAY_MS))
                            .unwrap_or(RATE_LIMIT_BASE_DELAY_MS * (1u64 << (rate_retries - 1)));
                        eprintln!(
                            "[OpenRouter] rate limit on {} — retry {}/{} after {}ms",
                            model, rate_retries, RATE_LIMIT_RETRIES_PER_MODEL, delay_ms
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        continue;
                    }
                    eprintln!("[OpenRouter] rate limit on {} — next model", model);
                    break;
                }
                if status.as_u16() == 404
                    || err_text.contains("No endpoints found")
                    || err_text.contains("not a valid model")
                {
                    eprintln!("[OpenRouter] model unavailable: {} — next model", model);
                    break;
                }
                if status.as_u16() == 402
                    && err_text.contains("Prompt tokens limit exceeded")
                {
                    return Err(
                        "حجم المحادثة كبير جداً. ابدأ محادثة جديدة أو أضف رصيد OpenRouter."
                            .to_string(),
                    );
                }
                if status.as_u16() == 402
                    && err_text.contains("can only afford")
                    && credit_retries < 3
                {
                    credit_retries += 1;
                    if let Some(affordable) = parse_openrouter_affordable_tokens(&err_text) {
                        max_tokens = affordable.saturating_sub(128).max(512).min(max_tokens);
                    } else {
                        max_tokens = (max_tokens / 2).max(512);
                    }
                    eprintln!(
                        "[OpenRouter] 402 credits — retry with max_tokens={}",
                        max_tokens
                    );
                    continue;
                }
                if status.as_u16() == 402 {
                    return Err(
                        "رصيد OpenRouter غير كافٍ. أضف رصيداً على openrouter.ai/settings/credits"
                            .to_string(),
                    );
                }
                eprintln!("[OpenRouter] error on {} — trying fallback", model);
                break;
            }

            eprintln!("[OpenRouter] success with model: {}", model);
            let mut json_res: Value = res.json().await.map_err(|e| e.to_string())?;
            if let Some(generation_id) = json_res.get("id").and_then(|v| v.as_str()) {
                if let Some(generation) =
                    fetch_openrouter_generation_usage(&client, clean_key, generation_id).await
                {
                    json_res["openrouter_generation"] = generation;
                }
            }
            return Ok(json_res);
        }
    }

    Err(format!(
        "تعذّر الاتصال بـ OpenRouter. آخر خطأ: {}",
        if last_error.len() > 200 {
            &last_error[..200]
        } else {
            &last_error
        }
    ))
}

async fn fetch_openrouter_generation_usage(
    client: &reqwest::Client,
    api_key: &str,
    generation_id: &str,
) -> Option<Value> {
    if generation_id.trim().is_empty() {
        return None;
    }

    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(250 * attempt)).await;
        }
        let res = client
            .get("https://openrouter.ai/api/v1/generation")
            .header("Authorization", format!("Bearer {}", api_key))
            .query(&[("id", generation_id)])
            .send()
            .await;

        let Ok(res) = res else {
            continue;
        };
        if !res.status().is_success() {
            continue;
        }
        let parsed: Value = match res.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(data) = parsed.get("data") {
            eprintln!("[OpenRouter] generation usage loaded id={}", generation_id);
            return Some(data.clone());
        }
    }

    eprintln!("[OpenRouter] generation usage unavailable id={}", generation_id);
    None
}

fn parse_retry_after_secs(res: &reqwest::Response) -> Option<u64> {
    res.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

fn parse_openrouter_affordable_tokens(err_text: &str) -> Option<u32> {
    let marker = "can only afford ";
    let start = err_text.find(marker)? + marker.len();
    let tail = &err_text[start..];
    let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse::<u32>().ok()
}

/// Detects short greetings/social messages that don't need RAG or LLM.
/// Returns a canned response if matched, None otherwise.
fn try_handle_greeting(text: &str, erp: crate::erp_profile::ErpKind) -> Option<String> {
    let t = text.trim().to_lowercase();
    if t.chars().count() > 25 { return None; }

    let greetings = [
        "اهلا", "أهلا", "هلا", "هاي", "مرحبا", "مرحباً", "السلام",
        "صباح الخير", "مساء الخير", "كيفك", "كيف الحال", "شلونك",
        "hi", "hello", "hey", "salam",
    ];
    let thanks = ["شكرا", "شكراً", "تسلم", "thanks", "thank you"];
    let bye = ["باي", "وداعا", "bye", "مع السلامة"];

    if greetings.iter().any(|g| t.contains(g)) {
        let erp_name = erp.display_name_ar();
        return Some(format!(
            "👋 أهلاً بك! أنا مساعد قاعدة بيانات {erp_name}.\n\
            اسألني عن:\n📦 المخزون والمنتجات\n💰 فواتير الشراء والبيع\n\
            📅 تواريخ الصلاحية\n👥 العملاء والموردين"
        ));
    }
    if thanks.iter().any(|w| t.contains(w)) {
        return Some("على الرحب والسعة 🌹".to_string());
    }
    if bye.iter().any(|w| t.contains(w)) {
        return Some("إلى اللقاء 👋".to_string());
    }
    None
}

#[allow(dead_code)]
const VERIFIED_DB_ENTITY_KNOWLEDGE: &str = r#"<VERIFIED_DATABASE_ENTITY_MAP>
Source of truth:
- DDL file reviewed: C:\Users\DELL\Desktop\al-tabi\Full_Marketing_Database_DDL.sql
- Live checks were verified with sqlcmd against SQL Server database Marketing2026.
- The DDL file contains 164 table definitions. The live database currently exposes 168 dbo base tables.

Live database facts verified with sqlcmd:
- dbo.SALE_INVOICE date range: query at runtime — `SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE` (do NOT hardcode; was ~2026-04-07 in old backup, live DB may be newer e.g. 2026-05-21).
- dbo.BUY_INVOICE date range: 2025-07-22 02:14:34.210 to 2026-04-05 23:11:51.773, 1473 invoices.
- dbo.ITEMS contains 17616 product rows.
- dbo.ITEMS_SUB contains 4110 stock rows; all 4110 currently have QTY > 0.
- dbo.ITEMS_SUB has 4024 rows with CATEOGRY3 not null; expiry range: 2025-03-01 to 2032-06-30.
- dbo.COMMISSIONER has only COMM_ID=0 / COMM_NAME='N/A'. BUY_INVOICE and SALE_INVOICE both have 0 non-zero COMM_ID rows.
- dbo.CUSTOMERS currently has 116 vendors, 90 customers, and 203 total customer/supplier rows.
- dbo.Invoice_Items currently has 0 rows and is a temporary scratch invoice editor table, not an analytical source.

Business entity map:
- Product / item / medicine / صنف / دواء / منتج -> dbo.ITEMS.
  Key columns: ITEM_ID, ITEM_MODEL, ITEM_NAME, LAST_COST, AVER_COST, MIN_LEVEL, MAX_LEVEL, ITEM_INVISIBLE, PLACE.
- Current stock / inventory / المخزون / الرصيد المخزني -> dbo.ITEMS_SUB.
  Key columns: ITEM_ID, STORE_ID, QTY, CATEOGRY1, CATEOGRY2, CATEOGRY3.
  Join to dbo.ITEMS by ITEM_ID and to dbo.STORES by STORE_ID.
- Store / warehouse / مخزن -> dbo.STORES.
  Key columns: STORE_ID, STORE_NAME.
- Supplier / vendor / مورد / شركة موردة -> dbo.CUSTOMERS where CUST_VENDOR = 1.
  Key columns: CUST_ID, CUST_NAME, CUST_NO, ACC_ID, CUST_VENDOR, CUST_CUSTOM, CUST_EMP, CUST_MAX_DEBIT.
- Customer / client / زبون / عميل -> dbo.CUSTOMERS where CUST_CUSTOM = 1.
- Internal employee / user / مدخل الفاتورة / مستخدم -> dbo.USERS.
  Key columns: USERS_ID, FULL_NAME, USER_NAMES.
- Live customer/vendor balance / debt / رصيد / دين -> prefer invoice math: dbo.TAKE (collections), dbo.GIVE (payments), SALE/BUY invoices, R_S/B_R returns, dbo.BALANCE_EDIT adjustments. dbo.BALANCE_C exists but is currently empty in the live DB.
- Shortage monitoring / متابعة النواقص -> dbo.ITEMS + dbo.ITEMS_SUB + MIN_LEVEL/MAX_LEVEL + recent SALE_ITEMS.
- Active shortages with supplier + last buy price / نواقص نشطة بمورد -> `run_query_pattern("نواقص نشطة مورد")`: ITEMS + ITEMS_SUB + BUY_ITEMS/BUY_INVOICE/CUSTOMERS + recent net sales. Supplier from last BUY_INVOICE.CUST_ID, NOT GIVE.
- Supplier price comparison for one product / مقارنة أسعار الموردين -> `run_query_pattern("مقارنة أسعار موردين", product_filter="...")`: BUY_ITEMS/BUY_INVOICE/CUSTOMERS grouped by supplier. Requires product name/code. NOT GIVE.

Purchase flow:
- Purchase invoice header / فاتورة شراء -> dbo.BUY_INVOICE.
  Key columns: B_ID, B_DATE, CUST_ID, USERS_ID, B_DISCOUNT, B_SPENT, S_REF_NO.
  Supplier is dbo.CUSTOMERS via BUY_INVOICE.CUST_ID.
  Entered-by employee is dbo.USERS via BUY_INVOICE.USERS_ID.
- Purchase line / بند شراء -> dbo.BUY_ITEMS.
  Key columns: B_ITEM_ID, B_ID, ITEM_ID, STORE_ID, QTY, UNIT_ID, UNIT_QTY, CATEOGRY1, CATEOGRY2, CATEOGRY3, PRICE, BARCODE.
  Last purchase price queries usually start from BUY_ITEMS, join BUY_INVOICE for date/supplier, then ITEMS for product name/code.
- Purchase return header/items -> dbo.B_R_INVOICE and dbo.B_R_ITEMS.
  Header key/date: B_R_ID, B_R_DATE. Item foreign key: B_R_ITEMS.B_R_ID.

Sales flow:
- Sales invoice header / فاتورة بيع -> dbo.SALE_INVOICE.
  Key columns: S_ID, S_DATE, CUST_ID, CUST_NAME, USERS_ID, S_DISCOUNT, S_TAX1, S_TAX2, S_SHIPMENT, WAIT.
- Sales line / بند بيع -> dbo.SALE_ITEMS.
  Key columns: S_ITEM_ID, S_ID, ITEM_ID, STORE_ID, QTY, UNIT_ID, UNIT_QTY, CATEOGRY1, CATEOGRY2, CATEOGRY3, PRICE, LAST_COST, AVER_COST, PUBLIC_PRICE, BARCODE, S_TIME.
  Important: SALE_ITEMS does NOT contain S_DATE. For sale dates, always join SALE_ITEMS.S_ID to SALE_INVOICE.S_ID.
  Revenue per line = QTY * PRICE. Never SUM(PRICE) alone. Never aggregate PRICE in a subquery then join invoice header.
- Sales employee / موظف المبيعات -> SALE_INVOICE.USERS_ID joined to dbo.USERS.USERS_ID; display USERS.FULL_NAME.
  COMMISSIONER is unused (COMM_ID=0 only). Do not use COMMISSIONER for employee sales reports.
- Preferred sales reporting VIEW: dbo.SALE_ITEMS_INVOICE_VIEW (has S_DATE, FULL_NAME, QTY, PRICE, ITEM_NAME, CUST_NAME pre-joined).
  Alternative header VIEW: dbo.SALE_INVOICE_VIEW (no line amounts).
  Call get_database_views() before writing employee/daily sales SQL.
- Sales return header/items -> dbo.R_S_INVOICE and dbo.R_S_ITEMS.
  Header key/date: S_R_ID, S_R_DATE. Item foreign key: R_S_ITEMS.S_R_ID.

Expiry and batch:
- In item/stock transaction tables, CATEOGRY1 means batch/lot number.
- In item/stock transaction tables, CATEOGRY3 is the expiry date as datetime.
- The table dbo.CATEOGRY3 is a category lookup table and is not the expiry source.
- For stock expiry reports, use dbo.ITEMS_SUB.CATEOGRY3 and require ITEMS_SUB.QTY > 0.
- Never use dbo.Invoice_Items.Expiry for stock expiry analytics; it is varchar(30) and belongs to the empty scratch table.

Other operational entities:
- Spoiled/damaged products -> dbo.SPOIL_INVOICE and dbo.SPOIL_ITEMS.
  Header key/date: SP_ID, SP_DATE. Item foreign key: SPOIL_ITEMS.SP_ID.
- Warehouse transfers -> dbo.TRANSFER_INVOICE and dbo.TRANSFER_ITEMS.
  Header key/date: TR_ID, TR_DATE. Transfer items include STORE_F_ID and STORE_T_ID, both referencing dbo.STORES.STORE_ID.
- Units -> dbo.UNITS.
  Key columns: UNIT_ID, UNIT_DISC, UNIT_QTY.
- Global company/settings row -> dbo.SITTEINGS.
  Important spelling: SITTEINGS. It has general settings such as A_NAME, PHONE, MOBILE, FAX and many flags. Do not join it to invoices or items.

Reporting table caution:
- Tables ending with SEARCH_TRANS_TABLE, REPORT, CHART, *_DELETED, SERIAL_NO, and temporary/editor tables may be useful only for very specific questions.
- Prefer normalized operational tables above unless the user explicitly asks for an existing report/search table.

Query discipline:
- Use SQL Server T-SQL only: TOP, GETDATE(), DATEADD, CONVERT(date,...), ISNULL.
- Never use LIMIT, NOW(), ILIKE, or PostgreSQL syntax against Marketing2026.
- Start from the business entity map before calling search_schema.
- If a requested field is not in this map or in <schema>, call explore_local_schema before writing SQL.
</VERIFIED_DATABASE_ENTITY_MAP>"#;

/// هذه الثوابت نُقلت إلى QUERY_PATTERNS.md — محفوظة هنا كمرجع احتياطي فقط
#[allow(dead_code)]
const SMART_PURCHASE_ORDER_TEMPLATE: &str = r#"
**Q: "طلبية شراء ذكية" / "ماذا أشتري" / Smart purchase order (sales velocity + stock coverage days):**
Triggers (Arabic): طلبية شراء، شراء ذكي، ماذا أشتري، أيام تغطية الرصيد، كم يكفي المخزون، سرعة البيع، نفاد، أولويات الشراء، تحليل نواقص مع كمية مقترحة.
⚠️ Prefer this template over the simple "نواقص أهم المنتجات" template when the user wants suggested purchase qty or "how many days stock will last".

**Parameters (DECLARE at top — adapt if user specifies):**
- `@DaysRecent int = 60` — sales velocity window (NEVER default 30; latest sale is ~48 days before today)
- `@DaysTotal int = 180` — wider comparison window (optional CTE)
- `@CoverageDays int = 30` — target days of stock after purchase (user may ask 15 / 45 / 60)
- `@AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE)` — anchor to last sale, NOT GETDATE() alone

**Formulas:**
- Stock = `SUM(ITEMS_SUB.QTY)` per `ITEM_ID`
- Net sales = `SALE_ITEMS` joined to `SALE_INVOICE` on `S_DATE` MINUS `R_S_ITEMS` returns on `R_S_INVOICE.S_R_DATE` (negative QTY)
- `ActiveSaleDays` = `COUNT(DISTINCT CAST(invoice_date AS date))` where net qty > 0
- `DailyRate` = `SoldQty / NULLIF(ActiveSaleDays, 0)` (fallback: `SoldQty / @DaysRecent`)
- `DaysCoverage` = `StockQty / DailyRate` (NULL if no sales)
- `SuggestedBuyQty` = `CASE WHEN (DailyRate * @CoverageDays - StockQty) < 0 THEN 0 ELSE (DailyRate * @CoverageDays - StockQty) END`
- Priority: stock=0 AND sales>0 → `نفاد — شراء عاجل`; DaysCoverage<7 → `حرج`; DaysCoverage<@CoverageDays → `يُنصح بالشراء`; else `كافٍ`
- Last supplier: latest `BUY_ITEMS` row per `ITEM_ID` via `MAX(B_ITEM_ID)` → `BUY_INVOICE` → `CUSTOMERS.CUST_NAME`
- Filter: `ITEM_INVISIBLE=0`, `SoldQty>0`, and (`Stock<=MIN_LEVEL` OR `Stock=0` OR `DaysCoverage<@CoverageDays`)
- ORDER BY: zero stock first, then `DaysCoverage ASC`, then `SoldQty DESC`
- Output columns (Arabic aliases OK): الكود، اسم المنتج، رصيد المخزون، مبيعات آخر نافذة، أيام بيع فعلية، معدل يومي، أيام تغطية الرصيد، آخر بيع، كمية الشراء المقترحة، الأولوية، آخر سعر شراء، آخر مورد
- Append `د.ل` to all price columns in the Arabic reply.

```
DECLARE @DaysRecent int=60, @CoverageDays int=30;
DECLARE @AsOfDate date=(SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @RecentFrom date=DATEADD(day,-@DaysRecent,@AsOfDate);
;WITH Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT ITEM_ID, SUM(QTY) SoldQty,
         COUNT(DISTINCT CAST(S_DATE AS date)) ActiveSaleDays, MAX(S_DATE) LastSaleDate
  FROM (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE FROM dbo.SALE_ITEMS SI
    JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE FROM dbo.R_S_ITEMS RSI
    JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
  ) X GROUP BY ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE LastBuyPrice, B.B_DATE LastBuyDate, CU.CUST_NAME LastSupplier
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.B_ITEM_ID IN (
    SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
    JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID=B2.B_ID GROUP BY BI2.ITEM_ID
  )
)
SELECT TOP 50 I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ISNULL(S.StockQty,0) AS Stock, SR.SoldQty, SR.ActiveSaleDays,
  SR.SoldQty/NULLIF(SR.ActiveSaleDays,0) AS DailyRate,
  ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(SR.ActiveSaleDays,0),0) AS DaysCoverage,
  CASE WHEN ISNULL(S.StockQty,0)<=0 AND SR.SoldQty>0 THEN N'نفاد'
       WHEN ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(SR.ActiveSaleDays,0),0)<7 THEN N'حرج'
       WHEN ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(SR.ActiveSaleDays,0),0)<@CoverageDays THEN N'شراء'
       ELSE N'كافٍ' END AS Priority,
  CASE WHEN (SR.SoldQty/NULLIF(SR.ActiveSaleDays,0))*@CoverageDays-ISNULL(S.StockQty,0)<0 THEN 0
       ELSE (SR.SoldQty/NULLIF(SR.ActiveSaleDays,0))*@CoverageDays-ISNULL(S.StockQty,0) END AS SuggestedBuy,
  LB.LastBuyPrice, LB.LastSupplier
FROM dbo.ITEMS I
JOIN SalesRecent SR ON I.ITEM_ID=SR.ITEM_ID
LEFT JOIN Stock S ON I.ITEM_ID=S.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID=LB.ITEM_ID
WHERE I.ITEM_INVISIBLE=0 AND SR.SoldQty>0
  AND (ISNULL(S.StockQty,0)<=I.MIN_LEVEL OR ISNULL(S.StockQty,0)=0
       OR ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(SR.ActiveSaleDays,0),0)<@CoverageDays)
ORDER BY CASE WHEN ISNULL(S.StockQty,0)<=0 THEN 0 ELSE 1 END,
  DaysCoverage ASC, SR.SoldQty DESC;
```
Full tested SQL file: `reports-app/smart_purchase_order.sql`
"#;

#[allow(dead_code)]
const SHORTAGE_TRACKING_TEMPLATE: &str = r#"
**Q: "متابعة النواقص" / "الأصناف النافدة" / Shortage monitoring dashboard:**
Triggers (Arabic): متابعة النواقص، قائمة النواقص، أصناف نافدة، تحت الحد الأدنى، فجوة المخزون، مراقبة المخزون.
⚠️ Use for **monitoring** (status + gap vs MIN_LEVEL). If user wants **supplier + last buy price** → use `run_query_pattern("نواقص نشطة مورد")` instead (file `active_shortage_tracking.sql`). For suggested purchase qty use "طلبية شراء ذكية".

**Parameters:**
- `@DaysRecent int = 60` — context sales in window
- `@AsOfDate = MAX(S_DATE)` from `SALE_INVOICE`

**Rules:**
- Stock = `SUM(ITEMS_SUB.QTY)` per `ITEM_ID`
- Net sales in window = `SALE_ITEMS` + `SALE_INVOICE.S_DATE` minus `R_S_ITEMS` returns
- `فجوة النقص` = `MIN_LEVEL - Stock` when `MIN_LEVEL > 0`
- Status: stock=0 + sales>0 → `نفاد`; stock=0 → `نفاد بدون مبيعات حديثة`; stock<=MIN_LEVEL → `تحت الحد الأدنى`; stock < MIN_LEVEL*1.25 + sales>0 → `قريب من النفاد`
- Filter: stock=0 OR stock<=MIN_LEVEL OR (stock < MIN_LEVEL*1.25 AND recent sales>0)
- ORDER BY: نفاد first, then recent sales DESC, then gap DESC

```
DECLARE @DaysRecent int=60;
DECLARE @AsOfDate date=(SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @RecentFrom date=DATEADD(day,-@DaysRecent,@AsOfDate);
;WITH Stock AS (SELECT ITEM_ID, SUM(ISNULL(QTY,0)) StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID),
SalesRecent AS (
  SELECT ITEM_ID, SUM(QTY) SoldQty, MAX(S_DATE) LastSaleDate FROM (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE FROM dbo.SALE_ITEMS SI
    JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE FROM dbo.R_S_ITEMS RSI
    JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
  ) X GROUP BY ITEM_ID
)
SELECT TOP 100 I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ISNULL(S.StockQty,0) AS Stock, I.MIN_LEVEL, I.MAX_LEVEL,
  CASE WHEN I.MIN_LEVEL>0 THEN I.MIN_LEVEL-ISNULL(S.StockQty,0) ELSE 0 END AS ShortageGap,
  ISNULL(SR.SoldQty,0) AS RecentSales, SR.LastSaleDate,
  CASE WHEN ISNULL(S.StockQty,0)<=0 AND ISNULL(SR.SoldQty,0)>0 THEN N'نفاد'
       WHEN ISNULL(S.StockQty,0)<=0 THEN N'نفاد بدون مبيعات'
       WHEN I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<=I.MIN_LEVEL THEN N'تحت الحد الأدنى'
       WHEN I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<I.MIN_LEVEL*1.25 AND ISNULL(SR.SoldQty,0)>0 THEN N'قريب من النفاد'
       ELSE N'مراقبة' END AS ShortageStatus
FROM dbo.ITEMS I
LEFT JOIN Stock S ON I.ITEM_ID=S.ITEM_ID
LEFT JOIN SalesRecent SR ON I.ITEM_ID=SR.ITEM_ID
WHERE I.ITEM_INVISIBLE=0
  AND (ISNULL(S.StockQty,0)<=0 OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<=I.MIN_LEVEL)
       OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<I.MIN_LEVEL*1.25 AND ISNULL(SR.SoldQty,0)>0))
ORDER BY CASE WHEN ISNULL(S.StockQty,0)<=0 THEN 0 ELSE 1 END, ISNULL(SR.SoldQty,0) DESC;
```
Full tested SQL file: `reports-app/shortage_tracking.sql`
"#;

#[allow(dead_code)]
const DEBTS_TRACKING_TEMPLATE: &str = r#"
**Q: "متابعة الديون" / "اللي لي واللي علي" / Debts receivable & payable:**
Triggers (Arabic): ديون، متابعة الديون، رصيد الزبائن، ديون الموردين، اللي لي، اللي علي، مقبوضات، مدفوعات.
⚠️ `dbo.BALANCE_C` is **empty** in this live DB — compute balances from invoices + payments, NOT from BALANCE_C alone.

**Tables:**
- Sales → `SALE_INVOICE` + `SALE_ITEMS` (line value = QTY*PRICE)
- Sales returns → `R_S_INVOICE` + `R_S_ITEMS`
- Collections from customers → `dbo.TAKE` (`T_VALUE`, `CUST_ID`, `T_DATE`) — linked to sales receipts
- Purchases → `BUY_INVOICE` + `BUY_ITEMS`
- Purchase returns → `B_R_INVOICE` + `B_R_ITEMS`
- Payments to suppliers → `dbo.GIVE` (`G_VALUE`, `CUST_ID`, `G_DATE`)
- Opening/adjustment → `dbo.BALANCE_EDIT` (`BL_DEBIT`, `BL_CREDIT` per `CUST_ID`)

**Formulas:**
- **لي (زبون مدين)** `CUST_CUSTOM=1`: `Remaining = Sales - SaleReturns - TAKE + (SUM(BL_DEBIT)-SUM(BL_CREDIT))`
- **علي (مورد دائن)** `CUST_VENDOR=1`: `Remaining = Buys - BuyReturns - GIVE + (SUM(BL_DEBIT)-SUM(BL_CREDIT))`
- Filter `Remaining >= @MinBalance` (default 1)
- Append `د.ل` to all money columns in Arabic reply
- ORDER BY `نوع الدين`, then `Remaining DESC`

```
DECLARE @MinBalance float=1;
;WITH BalanceAdj AS (SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AdjBalance FROM dbo.BALANCE_EDIT GROUP BY CUST_ID),
SaleTot AS (SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) SalesValue, MAX(SI.S_DATE) LastSaleDate
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID),
SaleReturnTot AS (SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) ReturnValue FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID),
TakeTot AS (SELECT CUST_ID, SUM(T_VALUE) PaidValue, MAX(T_DATE) LastTakeDate FROM dbo.TAKE GROUP BY CUST_ID),
BuyTot AS (SELECT B.CUST_ID, SUM(BI.QTY*BI.PRICE) BuyValue, MAX(B.B_DATE) LastBuyDate
  FROM dbo.BUY_INVOICE B JOIN dbo.BUY_ITEMS BI ON B.B_ID=BI.B_ID GROUP BY B.CUST_ID),
BuyReturnTot AS (SELECT BR.CUST_ID, SUM(BRI.QTY*BRI.PRICE) ReturnValue FROM dbo.B_R_INVOICE BR JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID=BRI.B_R_ID GROUP BY BR.CUST_ID),
GiveTot AS (SELECT CUST_ID, SUM(G_VALUE) PaidValue, MAX(G_DATE) LastGiveDate FROM dbo.GIVE GROUP BY CUST_ID),
Receivables AS (
  SELECT N'لي — زبون مدين' AS DebtType, C.CUST_NO, C.CUST_NAME,
    ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS Remaining,
    ISNULL(ST.SalesValue,0) AS TotalMovement, ISNULL(TT.PaidValue,0) AS TotalSettled, C.CUST_MAX_DEBIT,
    ST.LastSaleDate, TT.LastTakeDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN SaleTot ST ON C.CUST_ID=ST.CUST_ID LEFT JOIN SaleReturnTot SRT ON C.CUST_ID=SRT.CUST_ID
  LEFT JOIN TakeTot TT ON C.CUST_ID=TT.CUST_ID LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
  WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0
    AND ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0)>=@MinBalance
),
Payables AS (
  SELECT N'علي — مورد دائن' AS DebtType, C.CUST_NO, C.CUST_NAME,
    ISNULL(BT.BuyValue,0)-ISNULL(BRT.ReturnValue,0)-ISNULL(GT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS Remaining,
    ISNULL(BT.BuyValue,0) AS TotalMovement, ISNULL(GT.PaidValue,0) AS TotalSettled, C.CUST_MAX_DEBIT,
    BT.LastBuyDate, GT.LastGiveDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN BuyTot BT ON C.CUST_ID=BT.CUST_ID LEFT JOIN BuyReturnTot BRT ON C.CUST_ID=BRT.CUST_ID
  LEFT JOIN GiveTot GT ON C.CUST_ID=GT.CUST_ID LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
  WHERE C.CUST_VENDOR=1 AND C.CUST_INVISIBLE=0
    AND ISNULL(BT.BuyValue,0)-ISNULL(BRT.ReturnValue,0)-ISNULL(GT.PaidValue,0)+ISNULL(BA.AdjBalance,0)>=@MinBalance
)
SELECT TOP 50 DebtType, CUST_NO, CUST_NAME, CAST(Remaining AS decimal(18,2)) AS Remaining,
  CAST(TotalMovement AS decimal(18,2)) AS TotalMovement, CAST(TotalSettled AS decimal(18,2)) AS TotalSettled,
  CUST_MAX_DEBIT, LastSaleDate, LastTakeDate
FROM (SELECT DebtType,CUST_NO,CUST_NAME,Remaining,TotalMovement,TotalSettled,CUST_MAX_DEBIT,
        LastSaleDate AS LastSaleDate, LastTakeDate AS LastTakeDate FROM Receivables
      UNION ALL
      SELECT DebtType,CUST_NO,CUST_NAME,Remaining,TotalMovement,TotalSettled,CUST_MAX_DEBIT,
        LastBuyDate, LastGiveDate FROM Payables) D
ORDER BY DebtType, Remaining DESC;
```
Full tested SQL file: `reports-app/debts_tracking.sql`
"#;

const MONTHLY_EXPENSES_TEMPLATE: &str = r#"
**Q: "مصاريف شهرية" / "إيصالات رواتب" / "مصاريف خاصة" / Monthly expenses:**
Triggers: مصاريف شهرية, رواتب شهرية, إيصالات رواتب, مصاريف تشغيلية, مصاريف خاصة, ما دفعناه رواتب, مصروفات الشهر.

**CRITICAL — verified on live DB:**
- `dbo.SALARIES` = **0 rows** — NEVER query for paid salaries.
- **Paid salary receipts** = `dbo.GIVE` WHERE `EXPENCES_ID = 1` (`EXPENCES.EXPENSE_DISC` = N'مصاريف رواتب'). Employee name often in `G_DISC` (e.g. N'مرتب عمر شهر 4').
- **Operational/private expenses** = `GIVE` WHERE `EXPENCES_ID > 0 AND EXPENCES_ID <> 1` AND `G_STATUES = 1`.
- **NOT expenses:** `GIVE` WHERE `EXPENCES_ID = 0` → supplier purchase payments (فاتورة مشتريات).

**Action:** `run_query_pattern("ملخص مالي شهري")` — executes 4 SQL blocks (debts + salary receipts + operational detail + summary).

**May 2026 sample totals:** رواتب 2690 د.ل (2 receipts), تشغيلية 291 د.ل (كهرباء + أخرى).

Full tested SQL: `reports-app/monthly_expenses_tracking.sql`
"#;

const ACTIVE_SHORTAGE_TRACKING_TEMPLATE: &str = r#"
**Q: "نواقص نشطة" / "منتجات ناقصة تباع" / Active shortages with supplier & last buy price:**
Triggers: نواقص نشطة, منتجات ناقصة تباع, أصناف ناقصة نشطة, نواقص بمورد, آخر سعر شراء للنواقص, shortage active selling, active shortages supplier.

**CRITICAL — verified on live DB (~150 rows):**
- **Stock** = `SUM(ITEMS_SUB.QTY)` per `ITEM_ID`.
- **نشطة (actively selling)** = net sales > 0 in last `@DaysRecent` days (default 60) ending at `@AsOfDate = MAX(S_DATE)`.
- **ناقصة** = stock ≤ 0 OR stock ≤ MIN_LEVEL OR stock < MIN_LEVEL×1.25.
- **Last buy price** = last `BUY_ITEMS.PRICE` for the item; fallback `ITEMS.LAST_COST`.
- **Supplier** = `CUSTOMERS.CUST_NAME` from last `BUY_INVOICE.CUST_ID`. ⚠️ NOT from `GIVE` (EXPENCES_ID=0 = supplier payments, not product supplier lookup).

**Output columns:** اسم المنتج، الكمية، آخر سعر شراء، المورد، مبيعات النافذة، الحد الأدنى، حالة النقص.

**Action:** `run_query_pattern("نواقص نشطة مورد")` — single SQL block, TOP 150.

**Distinction:** `متابعة النواقص` = monitoring dashboard without supplier/price. `نواقص نشطة مورد` = shortage list for reorder with supplier + last cost.

Full tested SQL: `reports-app/active_shortage_tracking.sql`
"#;

const SUPPLIER_PRICE_COMPARISON_TEMPLATE: &str = r#"
**Q: "مقارنة أسعار الموردين" / "أرخص مورد لمنتج" / Supplier price comparison for one product:**
Triggers: مقارنة أسعار, مقارنة أسعار الموردين, أسعار الموردين لمنتج, أرخص مورد, فرق أسعار الشراء, supplier price comparison.

**CRITICAL — verified on live DB (TRAMADOL NORMON example):**
- **Requires product name/code** — user `@mention` in chat OR pass `product_filter` to `run_query_pattern`.
- Picks best-matching `ITEM_ID` (most purchase lines if multiple name matches).
- **Supplier** = last `BUY_INVOICE.CUST_ID` → `CUSTOMERS.CUST_NAME`. NOT `GIVE`.
- **Price** = `BUY_ITEMS.PRICE` per purchase line; aggregates per supplier: last/min/max/avg/count.
- Default window: last **36 months** (`@MonthsBack=36`).
- Ordered by **cheapest last price first** (`ترتيب السعر`).

**Output columns:** اسم المنتج، الكود، المورد، آخر سعر شراء، آخر تاريخ، أقل/أعلى/متوسط، عدد مرات الشراء.

**Action:** `run_query_pattern("مقارنة أسعار موردين", product_filter="TRAMADOL")` — user can write `@TRAMADOL NORMON INJ` in message.

**Distinction:** `آخر-سعر-شراء-مورد` = last buy price list for many products. `مقارنة أسعار موردين` = per-supplier comparison for ONE product.

Full tested SQL: `reports-app/supplier_price_comparison.sql`
"#;

fn build_infinity_telegram_system_prompt(schema_extra: &str, memory_block: &str) -> String {
    format!(
        "{}\n\n\
<role>\n\
محلل SQL Server لـ **InfinityRetailDB** (صيدلية/تجزئة). حوّل الأسئلة العربية إلى T-SQL على schemas: \
Inventory, SALES, Purchase, MyCompany — نفّذ عبر الأدوات وأجب بالعربية (Telegram HTML).\n\
ملف الأنماط: AGENT_InfinityRetailDB.md\n\
</role>\n\n\
<tone_and_dialect>\n\
- **لهجة ليبية خفيفة وقصيرة جداً (لتوفير التوكنز):**\n\
  - ابدأ بترحيب ليبي خفيف ومختصر للغاية (مثل: 'مرحبتين.' أو 'أهلاً بيك. تفضل:').\n\
  - ممنوع نهائياً التملق، التحيات الطويلة، الكلام الفارغ، أو صيغ التبجيل والمبالغة (مثل 'يا فندم'، 'يسعدني خدمتكم'، 'بدقة متناهية').\n\
  - اعرض نتائج البيانات فوراً واختصر قدر الإمكان لتقليل استهلاك التوكنز.\n\
  - اقترح التصدير أو الخطوة التالية باختصار شديد ودون إطالة (مثل: 'تبيه إكسل أو PDF؟').\n\
</tone_and_dialect>\n\n\
<critical_rules>\n\
1. **ممنوع** جداول Marketing2026: dbo.ITEMS, SALE_INVOICE, BUY_ITEMS, CUSTOMERS, ITEMS_SUB.\n\
2. **run_query_pattern / search_query_patterns أولاً** — نواقص، طلبية شراء، مبيعات موظف، صلاحية، مقارنة موردين، دراسة منتج.\n\
3. **إيراد** = SUM(QYT * UnitPrice). **التاريخ** = SalesInvoiceDate (JOIN Data_SalesInvoices أو Data_View_SalesInvoiceItems).\n\
4. **get_product_schema / get_database_views** يُحمّلان INFINITY_* docs تلقائياً.\n\
5. **plan_complex_query** → **execute_query_plan** لدراسة منتج متعددة الخطوات.\n\
6. @mention منتج → product_filter في run_query_pattern.\n\
7. **search_schema** مفلتر على Infinity DDL — إن فشل استخدم search_query_patterns.\n\
8. العملة: د.ل | HTML فقط: <b>, <i>, <code>.\n\
9. احسب الإجماليات الفرعية والعامة للنتائج واعرضها بوضوح باختصار في نهاية ردك.\n\
</critical_rules>\n\n\
<workflow>\n\
1. حدّد النمط من جدول DOMAIN_CRITICAL_FACTS.\n\
2. run_query_pattern(keywords=...) أو plan_complex_query + execute_query_plan.\n\
3. validate_sql قبل SQL مخصص؛ export_last_result / PDF / Excel عند الطلب.\n\
</workflow>\n\n\
<anti_patterns>\n\
❌ dbo.ITEMS / SALE_ITEMS على Infinity\n\
❌ SUM(UnitPrice) بدون QYT\n\
❌ LIMIT / NOW() — استخدم TOP و GETDATE()\n\
❌ فلترة تاريخ من Data_SalesInvoiceItems بدون JOIN Data_SalesInvoices\n\
</anti_patterns>\n\n\
<schema>\n{schema_extra}\n</schema>\n\
{memory_block}",
        slim_domain_facts(crate::erp_profile::ErpKind::InfinityRetailDb),
        schema_extra = schema_extra,
        memory_block = memory_block,
    )
}

pub async fn handle_with_groq(
    client: &Client,
    token: &str,
    chat_id: i64,
    user_text: &str,
    groq_key: &str,
    ai_model: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
    chat_history: &mut Vec<Value>,
    openai_key: &str,
) {
    // Greeting gate: skip RAG and LLM for simple social messages
    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    if let Some(reply) = try_handle_greeting(user_text, erp) {
        let _ = send_html(client, token, chat_id, reply).await;
        return;
    }

    let _ = send_message(client, token, chat_id, "⏳ جاري التحليل...".to_string()).await;

    let new_schema_info = search_schema(user_text, openai_key, erp).await;
    let schema_info = prepare_schema_for_system_prompt(&new_schema_info);
    let memory_block =
        crate::agent_memory::recall_memory_prompt_block(user_text, openai_key, erp, crate::supabase_config::DEFAULT_APP_ACCESS_TOKEN).await;

    let system_instruction = if erp == crate::erp_profile::ErpKind::InfinityRetailDb {
        build_infinity_telegram_system_prompt(&schema_info, &memory_block)
    } else {
        format!(
        "<DOMAIN_CRITICAL_FACTS>\n\
**READ THIS FIRST.** This is your pre-loaded knowledge of the Marketing2026 database. \
Use this as your primary reference. Only call `search_schema` if the answer needs a table NOT listed here.\n\n\
### 📚 Core Tables (memorize these — they cover 90% of all queries)\n\n\
**`dbo.ITEMS`** — Product master (محتوى الكتالوج).\n\
```\n\
ITEM_ID int PK, ITEM_MODEL varchar(50)=code, ITEM_NAME varchar(800),\n\
LAST_COST float=آخر سعر شراء, AVER_COST float=متوسط التكلفة,\n\
MIN_LEVEL float, MAX_LEVEL float, ITEM_INVISIBLE bit=محذوف,\n\
ITEM_UPDATE_DATE datetime, USERS_ID, PLACE varchar(30)\n\
```\n\
Use `WHERE ITEM_NAME LIKE '%word%' OR ITEM_MODEL LIKE '%word%'` for product searches (never `=`).\n\n\
**`dbo.ITEMS_SUB`** — Current stock per store + batch + expiry. THE source of truth for stock.\n\
```\n\
ITEM_SUB_ID PK, ITEM_ID→ITEMS, STORE_ID→STORES, QTY float=الكمية,\n\
CATEOGRY1 varchar(10)=Batch رقم الدفعة,\n\
CATEOGRY2 varchar(10)=Sub-batch,\n\
CATEOGRY3 datetime=EXPIRY DATE تاريخ الصلاحية ⭐ (has INDEX)\n\
```\n\n\
**`dbo.BUY_INVOICE`** — Purchase invoices (فواتير الشراء = ما اشتريته من الموردين).\n\
```\n\
B_ID int PK, B_DATE datetime=تاريخ الفاتورة,\n\
CUST_ID→CUSTOMERS=المورد, USERS_ID→USERS=من أدخل الفاتورة,\n\
COMM_ID smallint (always 0 — IGNORE),\n\
B_DISCOUNT, B_SPENT, S_REF_NO=رقم مرجعي للفاتورة\n\
```\n\n\
**`dbo.BUY_ITEMS`** — Purchase line items (بنود فواتير الشراء).\n\
```\n\
B_ITEM_ID PK, B_ID→BUY_INVOICE, ITEM_ID→ITEMS, STORE_ID→STORES,\n\
QTY float=الكمية, PRICE float=سعر الشراء,\n\
CATEOGRY1=Batch, CATEOGRY3 datetime=ExpiryDate,\n\
BARCODE varchar(30), UNIT_ID, CURRENCY_ID, RATE float\n\
```\n\n\
**`dbo.SALE_INVOICE`** — Sales invoices (فواتير المبيعات).\n\
```\n\
S_ID int PK, S_DATE datetime=تاريخ البيع ⭐,\n\
CUST_ID→CUSTOMERS=الزبون, CUST_NAME varchar(100)=اسم الزبون مباشرة,\n\
USERS_ID=من أدخل الفاتورة, S_DISCOUNT, S_TAX1, S_TAX2, S_SHIPMENT,\n\
S_NOTE, S_STATUES tinyint, WAIT bit=معلَّقة\n\
```\n\n\
**`dbo.SALE_ITEMS`** — Sales line items (بنود فواتير المبيعات).\n\
```\n\
S_ITEM_ID PK, S_ID→SALE_INVOICE ⭐ (use this to join for date),\n\
ITEM_ID→ITEMS, STORE_ID, QTY, PRICE=سعر البيع,\n\
LAST_COST, AVER_COST, PUBLIC_PRICE,\n\
S_TIME datetime=وقت البيع (per-item, optional),\n\
CATEOGRY3=ExpiryDate, BARCODE\n\
```\n\
⚠️ **SALE_ITEMS does NOT have S_DATE.** For date filters, JOIN to SALE_INVOICE via S_ID.\n\n\
**`dbo.CUSTOMERS`** — Suppliers + customers (table unifié). \"مندوب\" in chat usually = supplier (مورد).\n\
```\n\
CUST_ID PK, CUST_NAME varchar(100), CUST_NO varchar(20),\n\
CUST_VENDOR bit=1 لو مورد, CUST_CUSTOM bit=1 لو زبون,\n\
CUST_EMP bit=1 لو موظف, CUST_MAX_DEBIT float=حد الدين,\n\
BL_DEBIT float, BL_CREDIT float=رصيد ابتدائي,\n\
ACC_ID→ACCOUNTS (⚠️ BALANCE_C فارغ — احسب الديون من TAKE/GIVE + الفواتير)\n\
```\n\n\
**`dbo.USERS`** — Internal employees (موظفو الشركة).\n\
```\n\
USERS_ID PK, FULL_NAME varchar(50), USER_NAMES varchar(20)\n\
```\n\n\
**`dbo.STORES`** — Stores/warehouses (المخازن).\n\
```\n\
STORE_ID PK, STORE_NAME varchar(50)\n\
```\n\n\
**`dbo.BALANCE_C`** — ⚠️ **فارغ في هذه القاعدة** (0 صف). لا تستخدمه لحساب ديون الزبائن/الموردين.\n\
```\n\
ACC_ID, ACC_DEBIT, ACC_CREDIT, BALANCE — غير مُحدَّث هنا\n\
```\n\
**البديل الإلزامي للديون:** `run_query_pattern(\"متابعة الديون\")` (تقرير شامل: لي + علي + تفاصيل) أو صيغة: مبيعات−مردودات−TAKE+BALANCE_EDIT (لي) / مشتريات−مردودات−GIVE+BALANCE_EDIT (علي).\n\
**ديون الموردين فقط (مبسّط — عمودان: اسم المورد + الدين):** `run_query_pattern(\"ديون الموردين\")` — استخدمه عندما يطلب المستخدم «ديون الموردين فقط» أو «اسم المورد والدين» أو لا يحتاج تواريخ/مقبوضات. لا تضف أي عمود آخر مهما طلب لاحقاً — هذا النمط مغلق على عمودين.\n\n\
**`dbo.R_S_INVOICE`** / **`dbo.R_S_ITEMS`** — Sales return invoices & items (مردودات المبيعات).\n\
- `R_S_INVOICE`: `S_R_ID` PK, `S_R_DATE` datetime, `CUST_ID`→CUSTOMERS, `CUST_NAME`, `USERS_ID`→USERS, `S_R_NOTE`.\n\
- `R_S_ITEMS`: `S_R_ITEM_ID` PK, `S_R_ID`→R_S_INVOICE, `ITEM_ID`→ITEMS, `STORE_ID`→STORES, `QTY` float, `PRICE` float=Returned Price, `CATEOGRY3` datetime=Expiry.\n\n\
**`dbo.B_R_INVOICE`** / **`dbo.B_R_ITEMS`** — Purchase return invoices & items (مردودات المشتريات للموردين).\n\
- `B_R_INVOICE`: `B_R_ID` PK, `B_R_DATE` datetime, `CUST_ID`→CUSTOMERS, `USERS_ID`→USERS, `B_R_NOTE`.\n\
- `B_R_ITEMS`: `B_R_ITEM_ID` PK, `B_R_ID`→B_R_INVOICE, `ITEM_ID`→ITEMS, `STORE_ID`→STORES, `QTY` float, `PRICE` float=Return Price, `CATEOGRY3` datetime=Expiry.\n\n\
**`dbo.SPOIL_INVOICE`** / **`dbo.SPOIL_ITEMS`** — Spoiled/Damaged products (الأدوية التالفة/المتلفة).\n\
- `SPOIL_INVOICE`: `SP_ID` PK, `SP_DATE` datetime, `SP_NOTE` varchar, `USERS_ID`.\n\
- `SPOIL_ITEMS`: `SP_ITEM_ID` PK, `SP_ID`→SPOIL_INVOICE, `ITEM_ID`→ITEMS, `QTY` float, `STORE_ID`→STORES, `CATEOGRY3` datetime=Expiry, `PRICE`, `LAST_COST`, `AVER_COST`.\n\n\
**`dbo.TRANSFER_INVOICE`** / **`dbo.TRANSFER_ITEMS`** — Warehouse transfer logs (تحويلات الأصناف بين المخازن).\n\
- `TRANSFER_INVOICE`: `TR_ID` PK, `TR_DATE` datetime, `TR_NOTE` varchar, `USERS_ID`.\n\
- `TRANSFER_ITEMS`: `TR_ITEM_ID` PK, `TR_ID`→TRANSFER_INVOICE, `ITEM_ID`→ITEMS, `QTY`, `STORE_F_ID`→STORES(from), `STORE_T_ID`→STORES(to), `CATEOGRY3` datetime=Expiry.\n\n\
**`dbo.UNITS`** — Product units (الوحدات: علبة، شريط...).\n\
- `UNIT_ID` PK, `UNIT_DISC` varchar=Name of unit, `UNIT_QTY` float.\n\n\
**`dbo.SITTEINGS`** — Global settings (الإعدادات العامة — note spelling with double 'T' and 'EI').\n\
- `A_NAME`=Company Name, `PHONE`, `MOBILE`, `FAX`. WARNING: Has NO ID column, contains exactly ONE row. NEVER try to JOIN it on items/invoices.\n\n\
### 🚨 Critical naming conventions\n\
- **`CATEOGRY3 datetime` = EXPIRY DATE.** Despite the name, it's NOT a category. It appears in: `ITEMS_SUB`, `BUY_ITEMS`, `SALE_ITEMS`, `JARED_ITEMS_B`, `MANF_F_ITEMS`, `C_BUY_ITEMS`, and all `*_DELETED` variants. The category lookup table (with categories proper) is `dbo.CATEOGRY3` (separate).\n\
- **Quantity column is `QTY`** (never Quantity, ITEM_QTY, AVAILABLE_QTY, Total_Quantity).\n\
- **`COMMISSIONER` table is UNUSED.** All `COMM_ID = 0`. \"مندوب\" → `CUSTOMERS WHERE CUST_VENDOR = 1`, not COMMISSIONER.\n\
- **`Invoice_Items` (mixed case)** is a SCRATCH table for in-progress edits, with `Expiry varchar(30)`. NEVER use it for stock expiry queries. Always use `ITEMS_SUB.CATEOGRY3` for that.\n\
- **`IS` is a SQL Server reserved word.** Don't use it as a table alias. Use `S`, `SUB`, `ITM`, etc.\n\
- **Product search: use `LIKE '%X%'` not `= 'X'`.** Names have variable spacing, case, etc.\n\n\
### 🎯 Reusable query templates\n\
For SQL templates use `run_query_pattern` or `search_query_patterns` — do NOT embed long SQL in answers.\n\
Key patterns: **آخر منتجات بيعت اليوم**, مبيعات يومية موظف, متابعة الديون, ملخص مالي شهري, نواقص نشطة مورد, مقارنة أسعار موردين, رواتب, مصروفات, طلبية شراء ذكية.\n\
**Debts:** NEVER `BALANCE_C` (empty). لي = Sales−Returns−TAKE+BALANCE_EDIT.\n\
**Monthly expenses (مصاريف شهرية):** Paid salary receipts = `dbo.GIVE` WHERE `EXPENCES_ID=1`. Operational/private = `GIVE` WHERE `EXPENCES_ID>0 AND EXPENCES_ID<>1` AND `G_STATUES=1`. ⚠️ `EXPENCES_ID=0` = supplier payments NOT expenses. ⚠️ `SALARIES` is **empty**.\n\
**Pattern:** `run_query_pattern(\"ملخص مالي شهري\")` → 4 parts: debts + salary receipts + operational + summary. SQL file: `monthly_expenses_tracking.sql`.\n\
{}\n\
{}\n\
{}\n\
**Date anchor:** `@AsOfDate = MAX(S_DATE)` from SALE_INVOICE — **query at runtime**, never assume 2026-04-07.\n\n\
### 🔍 أنماط الاستعلامات المعقدة\n\
للاستعلامات المعقدة (طلبية شراء ذكية، متابعة الديون، متابعة النواقص، نواقص نشطة مورد، مقارنة أسعار موردين، تقرير الصلاحية، الجرد الفعلي، الرواتب، التصنيع، الربحية، حركة صنف): \
**استدعِ أداة `search_query_patterns` أولاً** بكلمات مفتاحية مناسبة، ثم طبّق القالب الذي تُعيده. \
هذه الأداة تُعيد SQL كاملاً مختبراً جاهزاً للتعديل والتنفيذ — لا تكتب هذه الاستعلامات من الصفر.\n\n\
**Date ranges:** NEVER hardcode last sale date. Always run `SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE` or use `@LastSaleDay` in patterns. Old docs said 2026-04-07 — **ignore**; live DB may be 2026-05-21 or later.\n\
**Last products sold today / آخر منتجات بيعت اليوم:** `run_query_pattern(\"آخر منتجات بيعت اليوم\")` — بنود SALE_ITEMS_INVOICE_VIEW ليوم `@SaleDay=GETDATE()`، الأحدث أولاً. **ليس** نمط موظفين. إن فارغ → `@SaleDay=MAX(S_DATE)`.\n\
**Last sale day by employee:** `run_query_pattern(\"مبيعات آخر يوم موظف\")` — file `last_sale_day_by_employee.sql`.\n\
**Active shortages (supplier + last buy price):** `run_query_pattern(\"نواقص نشطة مورد\")` — file `active_shortage_tracking.sql`. Use when user asks for اسم المنتج، الكمية، آخر سعر شراء، المورد for items that are short AND still selling.\n\
**Supplier price comparison (one product):** `run_query_pattern(\"مقارنة أسعار موردين\", product_filter=\"...\")` — file `supplier_price_comparison.sql`. Use when user asks to compare buy prices across suppliers for ONE product (@mention or name fragment). NOT the same as نواقص نشطة مورد.\n\
</DOMAIN_CRITICAL_FACTS>\n\n\
<role>\n\
You are a senior SQL Server analyst for a pharmaceutical distribution company. \
Your job: convert Arabic questions into precise T-SQL queries against the Marketing2026 database, \
execute them via the `execute_raw_sql` tool, and report results back in Arabic.\n\
</role>\n\n\
<schema>\n\
The schema below was retrieved from a vector store (RAG). \
Every table name, every column name in your SQL **must** appear verbatim here OR in <DOMAIN_CRITICAL_FACTS>. \
If you can't find a needed column, call `search_schema` ONCE more with different keywords or use `explore_local_schema` — do NOT guess.\n\n\
{}\n\
</schema>\n\n\
{}\n\n\
<critical_rules>\n\
1. **NEVER invent column or table names.** Every column/table in your SQL must appear in <DOMAIN_CRITICAL_FACTS> or <schema>. \
If unsure, call `explore_local_schema` first.\n\n\
2. **SQL Server syntax only.** Use `TOP N` (never `LIMIT`). Use `GETDATE()` for current date. \
Column names are typically UPPERCASE_SNAKE_CASE — copy them verbatim.\n\n\
3. **One query at a time.** Never call `execute_raw_sql` in parallel for the same logical question. \
Wait for results before deciding the next step. If 0 rows, READ <DOMAIN_CRITICAL_FACTS> to check if you're querying the right table.\n\n\
4. **Domain knowledge — primary tables:**\n\
   - Products catalogue → `dbo.ITEMS` (ITEM_ID, ITEM_MODEL, ITEM_NAME, LAST_COST, AVER_COST)\n\
   - **Stock per store** → `dbo.ITEMS_SUB` (ITEM_ID, STORE_ID, QTY, CATEOGRY1=Batch, CATEOGRY3=ExpiryDate)\n\
   - Purchase invoices → `dbo.BUY_INVOICE` (B_ID, B_DATE, CUST_ID, USERS_ID)\n\
   - Purchase line items → `dbo.BUY_ITEMS` (B_ITEM_ID, B_ID, ITEM_ID, QTY, PRICE, BARCODE, CATEOGRY3=ExpiryDate)\n\
   - Sales line items → `dbo.SALE_ITEMS` (S_ITEM_ID, S_ID, ITEM_ID, QTY, PRICE, CATEOGRY3=ExpiryDate)\n\
   - Customers/suppliers → `dbo.CUSTOMERS` (CUST_ID, CUST_NAME, CUST_VENDOR, CUST_CUSTOMER)\n\
   - Users (employees) → `dbo.USERS` (USERS_ID, FULL_NAME)\n\n\
5. **Output format.** Reply in Arabic, formatted with Telegram HTML tags only: \
`<b>bold</b>`, `<i>italic</i>`, `<code>inline code</code>`. \
Do NOT use Markdown (`**`, `_`). Add 📊 📦 emojis where helpful. The official currency is `د.ل` (Libyan Dinar) — always append `د.ل` to any financial amounts, prices, or revenues in your final Arabic response (e.g., `150.00 د.ل`).\n\n\
5b. **Column name translation (MANDATORY).** When generating PDF or Excel reports, ALWAYS translate every column name from its database form into clear Arabic business terminology before passing it to any report tool. Database column names like ITEM_NAME → اسم المنتج, CUST_NAME → اسم العميل, QTY → الكمية, PRICE → السعر, LAST_COST → آخر تكلفة, AVER_COST → متوسط التكلفة, S_DATE → تاريخ البيع, B_DATE → تاريخ الشراء, FULL_NAME → اسم الموظف, G_VALUE → المبلغ, T_VALUE → المبلغ المحصَّل, ITEM_MODEL → الكود, STORE_NAME → المخزن, CUST_VENDOR → مورد, etc. NEVER pass raw DB column names to reports.\n\n\
5c. **Professional advisor behavior.** You are not just a query engine — you are a business intelligence advisor. When asked for advice, recommendations, or analysis: provide concrete, specific, actionable insights with data to back them up. Proactively suggest related queries the user might find useful. If you notice something important in the data (e.g., critically low stock, high debt concentration, expired products), mention it even if not asked.\n\n\
6. **PDF Reports.** Create PDF files only when the user explicitly asks for PDF, export, print, file, or report document. For a predefined saved report, call `generate_pdf`. For a new custom report that you design from the database, call `create_pdf_report` with a clear title and one read-only T-SQL SELECT query. If you already have tabular rows in memory and the user asks to export those exact rows, call `generate_custom_pdf`. Never create a PDF for a normal answer unless requested.\n\
7. **Excel Reports.** Create Excel (.xlsx) files only when the user explicitly asks for Excel, اكسل, إكسل, xlsx, spreadsheet, or جدول بيانات. For a predefined saved report, call `generate_excel`. For a new custom report from SQL, call `create_excel_report` with title + one read-only SELECT. If you already have tabular rows in memory, call `generate_custom_excel`. Never create Excel for a normal answer unless requested.\n\
8. **Advanced SQL tools.** `get_database_views` for Views + join rules (مبيعات موظف: SUM(QTY*PRICE), SALE_ITEMS_INVOICE_VIEW). `get_product_schema` for products. **مبيعات يومية/موظف:** `run_query_pattern(\"مبيعات يومية موظف\")` FIRST. **Complex:** `plan_complex_query` → `execute_query_plan`. `validate_sql`, `explain_sql`, `get_table_sample`, `compare_periods`, `suggest_indexes`, favorites, `export_last_result`.\n\n\
9. **Save to favorites (MANDATORY).** When the user says: «احفظ»، «خزّن»، «ضعه في المفضلة»، «أضفه للمحفوظات»، «save», «remember» — you MUST call `save_favorite_query` in the SAME turn with the EXACT SQL from the last successful `execute_raw_sql` / `run_query_pattern`. Generate a concise Arabic `name` (≤50 chars) and brief `description`. **Never** reply «تم الحفظ» without calling the tool. **Never** ask permission — just save.\n\
</critical_rules>\n\n\
<workflow>\n\
For each user question, follow these steps:\n\
1. **Read the question carefully.** Identify what data is requested and which domain entity (product, invoice, customer, expiry...).\n\
2. **Date/time sensitive?** If the request involves today, current month, current year, 'الشهر الحالي', 'اليوم', 'هذه السنة', salary period, or attendance — call `get_current_datetime` FIRST to get the exact date and use it in your SQL filters.\n\
3. **Is this a complex query?** **آخر منتجات/أصناف بيعت اليوم** → `run_query_pattern(\"آخر منتجات بيعت اليوم\")`. مبيعات يومية/موظف → `run_query_pattern(\"مبيعات يومية موظف\")`. **ديون + مصاريف + رواتب شهرية** → `run_query_pattern(\"ملخص مالي شهري\")`. **مقارنة أسعار الموردين لمنتج** → `run_query_pattern(\"مقارنة أسعار موردين\", product_filter=\"...\")`. **نواقص نشطة + مورد + آخر سعر شراء** → `run_query_pattern(\"نواقص نشطة مورد\")`. **متابعة نواقص بدون مورد** → `run_query_pattern(\"متابعة النواقص\")`. دراسة منتج → `plan_complex_query` + `execute_query_plan`. طلبية شراء/ديون/صلاحية/جرد/رواتب/تصنيع → `run_query_pattern`. Skip steps 4-5.\n\
3b. **Before execute_raw_sql:** call `validate_sql` if unsure. Unknown table shape? → `get_table_sample`. User wants period comparison? → `compare_periods`.\n\
4. **Scan <schema>** for matching tables. If no obvious match, call `search_schema` ONCE with refined keywords.\n\
5. **Pick ONE table** that best fits. Identify the exact column names you need (copy them from <schema>).\n\
5. **Write a single SELECT** — start with `SELECT TOP 10 *` if you're unsure of columns, then narrow down.\n\
6. **Call `execute_raw_sql` once.** Wait for the result.\n\
7. **If error or empty:** Read the error. Fix ONE thing at a time. Do not spam parallel queries.\n\
8. **Format the answer** in Arabic with Telegram HTML and send.\n\
</workflow>\n\n\
<examples>\n\
<example>\n\
User: \"كم منتج عندي في المخزن؟\"\n\
You think: \"كمية المخزون = stock quantity. From <critical_rules> rule 5, that's dbo.ITEMS_SUB with column QTY.\"\n\
Your SQL: `SELECT SUM(QTY) AS TotalQty FROM dbo.ITEMS_SUB`\n\
</example>\n\n\
<example>\n\
User: \"اعرض 10 منتجات منتهية الصلاحية\"\n\
You think: \"Expired stock means CATEOGRY3 (datetime) < today in dbo.ITEMS_SUB. Join to dbo.ITEMS for the product name.\"\n\
Your SQL: `SELECT TOP 10 I.ITEM_NAME, S.CATEOGRY3 AS Expiry, S.CATEOGRY1 AS Batch, S.QTY FROM dbo.ITEMS_SUB S LEFT JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID WHERE S.CATEOGRY3 IS NOT NULL AND S.CATEOGRY3 < GETDATE() AND S.QTY > 0 ORDER BY S.CATEOGRY3 ASC`\n\
</example>\n\n\
<example>\n\
User: \"اعرض آخر فاتورة شراء\"\n\
You think: \"Latest BUY_INVOICE by B_DATE, joined to CUSTOMERS for supplier name.\"\n\
Your SQL: `SELECT TOP 1 B.B_ID, B.B_DATE, CU.CUST_NAME AS Supplier FROM dbo.BUY_INVOICE B LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID ORDER BY B.B_DATE DESC`\n\
</example>\n\n\
<example>\n\
User: \"اعطني طلبية شراء ذكية لمدة 30 يوم تغطية\"\n\
You think: \"Smart purchase order from <DOMAIN_CRITICAL_FACTS> SMART_PURCHASE_ORDER template. @CoverageDays=30, @DaysRecent=60, @AsOfDate=MAX(S_DATE). Stock=ITEMS_SUB, net sales minus R_S returns.\"\n\
Your SQL: copy the DECLARE + CTE template from the \"طلبية شراء ذكية\" section; set `@CoverageDays=30`.\n\
</example>\n\n\
<example>\n\
User: \"اعرض متابعة النواقص\"\n\
You think: \"Shortage monitoring template — ITEMS_SUB stock vs MIN_LEVEL, recent net sales, status نفاد/تحت الحد الأدنى. NOT the smart purchase template.\"\n\
Your SQL: copy the \"متابعة النواقص\" DECLARE + CTE from <DOMAIN_CRITICAL_FACTS>.\n\
</example>\n\n\
<example>\n\
User: \"اعطني النواقص النشطة مع المورد وآخر سعر شراء\"\n\
You think: \"Active shortages with supplier — must filter items still selling + short stock. Supplier from last BUY_INVOICE, NOT GIVE.\"\n\
Your action: `run_query_pattern(\"نواقص نشطة مورد\")` — file `active_shortage_tracking.sql`.\n\
</example>\n\n\
<example>\n\
User: \"قارن أسعار الموردين لـ TRAMADOL NORMON\"\n\
You think: \"Single-product supplier price comparison — BUY_ITEMS grouped by CUST_ID, NOT GIVE. Need product_filter.\"\n\
Your action: `run_query_pattern(\"مقارنة أسعار موردين\", product_filter=\"TRAMADOL NORMON\")` — file `supplier_price_comparison.sql`.\n\
</example>\n\n\
<example>\n\
User: \"ما هي الديون التي لي والتي علي؟\"\n\
You think: \"Debts template — TAKE/GIVE + invoice totals. BALANCE_C is empty. لي=CUST_CUSTOM, علي=CUST_VENDOR.\"\n\
Your SQL: copy the \"متابعة الديون\" UNION query from <DOMAIN_CRITICAL_FACTS>.\n\
</example>\n\
</examples>\n\n\
<anti_examples>\n\
❌ NEVER: `SELECT SUM(Quantity) FROM dbo.ITEMS` — column is QTY, not Quantity, and ITEMS doesn't track stock anyway.\n\
❌ NEVER: parallel-calling execute_raw_sql for 5 tables guessing column names.\n\
❌ NEVER: `SELECT * FROM products WHERE expiry_date < NOW()` — wrong table name, wrong column name, wrong function.\n\
❌ NEVER: using `GETDATE()` alone as the sales window end when computing velocity — data ends at MAX(S_DATE).\n\
❌ NEVER: `FROM dbo.SALARIES` for paid salary receipts — table is empty; use `GIVE WHERE EXPENCES_ID=1`.\n\
❌ NEVER: treating `GIVE WHERE EXPENCES_ID=0` as operational expense — those are supplier purchase payments.\n\
❌ NEVER: claiming \"لا توجد بيانات\" before running an actual query.\n\
❌ NEVER: replying \"تم الحفظ\" without actually calling `save_favorite_query` — that's a lie.\n\
❌ NEVER: asking \"هل تريد حفظ هذا الاستعلام؟\" — when the user said \"احفظ\", just save it immediately.\n\
</anti_examples>\n\n\
<save_examples>\n\
<example>\n\
Context: User just received results from a sales-by-employee query.\n\
User: \"احفظ هذا الاستعلام\"\n\
Your action: `save_favorite_query(name=\"مبيعات اليوم حسب الموظف\", sql_query=<EXACT last SQL>, description=\"إجمالي مبيعات كل موظف في آخر يوم تسجيل فواتير.\")`\n\
Reply AFTER tool succeeds: «✅ حُفظ في المحفوظات باسم «مبيعات اليوم حسب الموظف» — يمكنك تشغيله من تبويب المحفوظات.»\n\
</example>\n\n\
<example>\n\
User: \"ضعه في المفضلة باسم تقرير الديون اليومي\"\n\
Your action: `save_favorite_query(name=\"تقرير الديون اليومي\", sql_query=<EXACT last SQL>, description=\"...\")` — use the user-provided name verbatim.\n\
</example>\n\
</save_examples>",
        MONTHLY_EXPENSES_TEMPLATE,
        ACTIVE_SHORTAGE_TRACKING_TEMPLATE,
        SUPPLIER_PRICE_COMPARISON_TEMPLATE,
        schema_info,
        memory_block
        )
    };

    let tools_json = json!([
        {
            "type": "function",
            "function": {
                "name": "search_schema",
                "description": "Semantic vector search over the Marketing2026 DDL knowledge base in Supabase. Returns up to 15 complete table definitions matching the keywords. Use this when the <schema> in your system prompt doesn't contain the table you need. CALL ONCE PER QUESTION — repeated calls with similar keywords return the same tables.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keywords": { "type": "string", "description": "Arabic + English keywords describing the entity/concept (e.g. 'منتهية الصلاحية expiry', 'متابعة النواقص ITEMS_SUB MIN_LEVEL', 'متابعة الديون TAKE GIVE CUSTOMERS', 'طلبية شراء ذكية SALE_ITEMS'). Mix Arabic and English for best results." }
                    },
                    "required": ["keywords"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "explore_local_schema",
                "description": "Lists actual column names and data types for a table from MSSQL INFORMATION_SCHEMA. Use this when you have a table name but are unsure about exact column names (especially case-sensitivity). Faster and more reliable than guessing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_hint": { "type": "string", "description": "Partial or full table name to filter INFORMATION_SCHEMA.COLUMNS (e.g. 'ITEMS', 'BUY_INVOICE'). Leave empty to list all tables." }
                    },
                    "required": ["table_hint"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_query_patterns",
                "description": "Searches QUERY_PATTERNS.md for SQL templates (up to 2 matches). Use FIRST for: دراسة منتج شاملة, وحدات وأسعار, طلبية شراء ذكية, ديون, نواقص, صلاحية, جرد, رواتب, تصنيع, ربحية, حركة صنف. For multi-step product study prefer plan_complex_query + execute_query_plan.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keywords": { "type": "string", "description": "Arabic or English keywords to search for the pattern (e.g. 'طلبية شراء ذكية', 'debts TAKE GIVE', 'expiry صلاحية', 'جرد JARED', 'رواتب salaries', 'رواتب بعد الخصم')." }
                    },
                    "required": ["keywords"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_current_datetime",
                "description": "Returns the current date, time, day of week, month, and year in Arabic (Libya timezone UTC+2). Call this whenever the user's request involves: today's date, current month/year, time-sensitive filtering, 'الشهر الحالي', 'اليوم', 'الآن', 'هذا الشهر', 'هذه السنة', salary month, attendance period. Also useful before any query that uses GETDATE() or date comparisons.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "execute_raw_sql",
                "description": "Executes a single read-only SELECT query against the local MSSQL database. Returns columns + rows. If result has more than 25 rows, server auto-creates a PDF and returns a 20-row preview plus file_path. Use TOP N (not LIMIT). Read-only enforced.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql_query": { "type": "string", "description": "A T-SQL SELECT statement. Always use TOP N to limit results. Use bracket notation [Name] for reserved words." }
                    },
                    "required": ["sql_query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_pdf",
                "description": "Generates a beautiful Arabic PDF file for a predefined system report by report_id and sends it. Use this when the user asks for the PDF version of a predefined report (like 'تحليل النواقص' or 'آخر سعر شراء').",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string", "description": "The UUID of the report." },
                        "search_term": { "type": "string", "description": "Optional search term / filter." },
                        "target_date": { "type": "string", "description": "Optional target date (e.g. '2026-04-01')." }
                    },
                    "required": ["report_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_custom_pdf",
                "description": "Generates and exports a beautifully formatted Arabic PDF for any custom query results or raw tabular data. Supports full Arabic text shaping and visual order. Use this when the user asks to export their current custom SQL query results or custom data as a PDF document.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "A descriptive title for the PDF report (e.g., 'تقرير مبيعات شهر أبريل 2026')." },
                        "columns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "The list of column names (must match the order of data in the rows)."
                        },
                        "rows": {
                            "type": "array",
                            "items": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "description": "The 2D array of rows (each row contains string values representing the columns)."
                        }
                    },
                    "required": ["title", "columns", "rows"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_pdf_report",
                "description": "Creates a PDF report from a single read-only T-SQL SELECT query and sends it to the Telegram chat. Use this ONLY when the user explicitly asks for a PDF, export, print-ready file, or report file. Do not call it for normal questions. The model may freely choose the report title and SQL design, but must use verified table/column names and SQL Server syntax.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Clear Arabic or English title for the PDF report." },
                        "sql_query": { "type": "string", "description": "A single read-only SQL Server SELECT query. Use TOP to keep the report bounded. Never use INSERT, UPDATE, DELETE, DROP, ALTER, EXEC, or PostgreSQL syntax." }
                    },
                    "required": ["title", "sql_query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_excel",
                "description": "Generates an Excel (.xlsx) file for a predefined system report by report_id and sends it to Telegram. Use when the user asks for Excel/xlsx export of a saved report.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string", "description": "The UUID of the report." },
                        "search_term": { "type": "string", "description": "Optional search term / filter." },
                        "target_date": { "type": "string", "description": "Optional target date (e.g. '2026-04-01')." }
                    },
                    "required": ["report_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_custom_excel",
                "description": "Generates and sends an Excel (.xlsx) file from custom tabular data already in the conversation. Use when the user asks to export current query results as Excel.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Report title for the Excel sheet." },
                        "columns": { "type": "array", "items": { "type": "string" }, "description": "Column headers." },
                        "rows": {
                            "type": "array",
                            "items": { "type": "array", "items": { "type": "string" } },
                            "description": "2D row data matching columns order."
                        }
                    },
                    "required": ["title", "columns", "rows"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_excel_report",
                "description": "Creates an Excel (.xlsx) report from a single read-only T-SQL SELECT query and sends it to Telegram. Use ONLY when the user explicitly asks for Excel, اكسل, xlsx, or spreadsheet export.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Clear title for the Excel report / worksheet." },
                        "sql_query": { "type": "string", "description": "A single read-only SQL Server SELECT query. Use TOP to bound rows." }
                    },
                    "required": ["title", "sql_query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "schedule_report",
                "description": "Schedules a recurring automated report to run at a set interval and appear in the Notifications section. Use whenever the user says: يومياً، كل ساعة، كل X دقائق، كل X ثواني، جدول تقرير، تنبيه دوري، تقرير تلقائي. The report will be generated automatically and shown as a notification (text/PDF/Excel) each time it fires.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Short Arabic name for this schedule (e.g. 'تقرير المبيعات اليومي')." },
                        "description": { "type": "string", "description": "One-line description of what this report does." },
                        "sql_query": { "type": "string", "description": "A single read-only T-SQL SELECT query to run on schedule." },
                        "report_title": { "type": "string", "description": "Title shown on the generated report." },
                        "report_type": { "type": "string", "description": "Output format: 'text' (default), 'pdf', or 'excel'." },
                        "columns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arabic column names matching the SELECT order (e.g. ['اسم المنتج', 'الكمية', 'السعر']). MANDATORY — always translate DB column names."
                        },
                        "interval_seconds": { "type": "integer", "description": "Repeat interval in seconds. Examples: 86400=يومي, 3600=ساعي, 300=كل 5 دقائق, 60=دقيقة, 10=كل 10 ثواني." },
                        "first_run_offset_seconds": { "type": "integer", "description": "Seconds from now until the first run (default 0 = run immediately at next tick)." }
                    },
                    "required": ["name", "sql_query", "report_title", "report_type", "columns", "interval_seconds"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_scheduled_reports",
                "description": "Returns a list of all currently scheduled recurring reports with their names, intervals, next run time, and status.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_scheduled_report",
                "description": "Cancels and removes a scheduled report by its ID. Use when the user asks to stop, cancel, or delete a scheduled report.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "schedule_id": { "type": "string", "description": "The ID of the scheduled report to delete." }
                    },
                    "required": ["schedule_id"]
                }
            }
        }
    ]);
    let mut tools: Vec<Value> = serde_json::from_value(tools_json).unwrap_or_default();
    tools.extend(crate::agent_tools::tool_definitions());

    // Initialize history with system prompt if empty
    if chat_history.is_empty() {
        chat_history.push(json!({
            "role": "system",
            "content": system_instruction
        }));
    } else {
        // Update the system prompt in case reports changed
        if let Some(first) = chat_history.first_mut() {
            if first.get("role").and_then(|r| r.as_str()) == Some("system") {
                *first = json!({
                    "role": "system",
                    "content": system_instruction
                });
            }
        }
    }

    // 2. Add user message to history
    chat_history.push(json!({
        "role": "user",
        "content": user_text
    }));

    // Keep history length manageable (system + last 8 messages)
    trim_chat_history_vec(chat_history, 9);

    let mut current_history = chat_history.clone();

    // Per-conversation guards
    let mut recent_sql: Vec<String> = Vec::new();           // last 3 executed SQLs
    let mut sql_fingerprints: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut schema_cache: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut sql_call_count: usize = 0;                       // total execute_raw_sql calls this turn
    const MAX_SQL_PER_TURN: usize = 5;
    const MAX_SAME_FINGERPRINT: usize = 2;
    const FORCE_FINALIZE_AFTER_SQL: usize = 4;

    // 3. Agent Loop (multi-step function calling)
    for iter_num in 0..20 {
        println!("[Telegram Agent] Iteration {} started", iter_num);
        trim_history_for_api(&mut current_history);

        let force_finalize = sql_call_count >= FORCE_FINALIZE_AFTER_SQL;
        if force_finalize {
            let nudge = json!({
                "role": "user",
                "content": "لقد جمعت بيانات كافية من الاستعلامات السابقة. توقف عن تنفيذ أي استعلام جديد، \
                وقدّم الآن الإجابة النهائية بالعربية بناءً على النتائج التي حصلت عليها بالفعل. \
                إن لم تكن البيانات كافية، اشرح ما وجدته وما ينقصه — دون استدعاء أي أداة."
            });
            current_history.push(nudge.clone());
            chat_history.push(nudge);
        }
        let req_body = if force_finalize {
            json!({
                "model": DEFAULT_AI_MODEL,
                "messages": current_history,
                "tool_choice": "none"
            })
        } else {
            json!({
                "model": DEFAULT_AI_MODEL,
                "messages": current_history,
                "tools": tools,
                "tool_choice": "auto"
            })
        };

        match call_groq_api(groq_key, ai_model, &req_body).await {
            Ok(res_json) => {
                if let Some(choices) = res_json.get("choices").and_then(|c| c.as_array()) {
                    if let Some(choice) = choices.get(0) {
                        let empty_json = json!({});
                        let message = choice.get("message").unwrap_or(&empty_json);

                        // Add model's response to history
                        current_history.push(message.clone());
                        chat_history.push(message.clone());

                        // Check if the model decided to call a function
                        if let Some(tool_calls) = message_tool_calls(message) {
                            // ─── Anti-parallel guard: if multiple execute_raw_sql calls, keep first only ───
                            let mut filtered_calls: Vec<&Value> = Vec::new();
                            let mut sql_seen = false;
                            for tc in tool_calls {
                                let tname = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                                if tname == "execute_raw_sql" || tname == "create_pdf_report" || tname == "create_excel_report" {
                                    if sql_seen {
                                        // skip subsequent SQL calls in same turn — respond with a hint instead
                                        let tid = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                        let resp = json!({
                                            "role": "tool",
                                            "tool_call_id": tid,
                                            "content": "{\"error\": \"تجاوز قاعدة 'استعلام واحد في كل دور'. تم تجاهل هذا الاستدعاء. انتظر نتيجة الاستعلام الأول قبل المحاولة مرة أخرى.\"}"
                                        });
                                        current_history.push(resp.clone());
                                        chat_history.push(resp);
                                        continue;
                                    }
                                    sql_seen = true;
                                }
                                filtered_calls.push(tc);
                            }

                            for tool_call in filtered_calls {
                                let func = tool_call.get("function").unwrap_or(&empty_json);
                                let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                let args_str = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                                let tool_call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("");

                                println!("[Telegram Agent] Tool called: {} with args: {}", name, args_str);
                                if name == "execute_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");
                                    
                                    let func_response_data = execute_db_report(rep_id, search_term, target_date, app_state, reports_cache).await;
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "generate_pdf" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");
                                    
                                    let func_response_data = generate_and_send_pdf_for_agent(
                                        client, token, chat_id, rep_id, search_term, target_date, app_state, reports_cache
                                    ).await;
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "generate_custom_pdf" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("");
                                    let empty_arr = vec![];
                                    let columns_val = args.get("columns").and_then(|c| c.as_array()).unwrap_or(&empty_arr);
                                    let rows_val = args.get("rows").and_then(|r| r.as_array()).unwrap_or(&empty_arr);

                                    let columns: Vec<String> = columns_val.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                    let mut rows: Vec<Vec<String>> = Vec::new();
                                    for r_val in rows_val {
                                        if let Some(arr) = r_val.as_array() {
                                            let r_str: Vec<String> = arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                            rows.push(r_str);
                                        }
                                    }

                                    let func_response_data = generate_custom_pdf_for_agent(
                                        client, token, chat_id, title, &columns, &rows
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "create_pdf_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("PDF Report");
                                    let sql_query = args.get("sql_query").and_then(|q| q.as_str()).unwrap_or("");

                                    let func_response_data = generate_pdf_report_from_sql_for_agent(
                                        client, token, chat_id, title, sql_query, app_state
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "generate_excel" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");

                                    let func_response_data = generate_and_send_excel_for_agent(
                                        client, token, chat_id, rep_id, search_term, target_date, app_state, reports_cache
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "generate_custom_excel" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("");
                                    let empty_arr = vec![];
                                    let columns_val = args.get("columns").and_then(|c| c.as_array()).unwrap_or(&empty_arr);
                                    let rows_val = args.get("rows").and_then(|r| r.as_array()).unwrap_or(&empty_arr);
                                    let columns: Vec<String> = columns_val.iter().map(json_value_to_cell).collect();
                                    let mut rows: Vec<Vec<String>> = Vec::new();
                                    for r_val in rows_val {
                                        if let Some(arr) = r_val.as_array() {
                                            rows.push(arr.iter().map(json_value_to_cell).collect());
                                        }
                                    }
                                    let func_response_data = generate_custom_excel_for_agent(
                                        client, token, chat_id, title, &columns, &rows
                                    ).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "create_excel_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("Excel Report");
                                    let sql_query = args.get("sql_query").and_then(|q| q.as_str()).unwrap_or("");

                                    let func_response_data = generate_excel_report_from_sql_for_agent(
                                        client, token, chat_id, title, sql_query, app_state
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "search_schema" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let keywords = args.get("keywords").and_then(|k| k.as_str()).unwrap_or("");
                                    
                                    let _ = send_message(client, token, chat_id, format!("⏳ جاري البحث في المخطط عن '{}'...", keywords)).await;
                                    let schema_data = search_schema(keywords, openai_key, erp).await;
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": schema_data
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "execute_raw_sql" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let sql_query = args.get("sql_query").and_then(|q| q.as_str()).unwrap_or("");
                                    let sql_norm = sql_query.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                                    let fingerprint = sql_semantic_fingerprint(sql_query);

                                    // Per-turn cap
                                    sql_call_count += 1;
                                    let fp_count = sql_fingerprints.entry(fingerprint.clone()).or_insert(0);
                                    *fp_count += 1;
                                    let fp_hits = *fp_count;

                                    let resp_content = if sql_call_count > MAX_SQL_PER_TURN {
                                        "{\"error\": \"تجاوزت الحد الأقصى للاستعلامات في هذا الدور. توقف وقدّم إجابة نهائية بالعربية بناءً على النتائج التي حصلت عليها بالفعل، أو اشرح ما ينقصك.\"}".to_string()
                                    } else if recent_sql.iter().any(|s| s == &sql_norm) {
                                        "{\"error\": \"هذا الاستعلام تم تنفيذه للتو بنفس الصيغة. النتيجة لن تتغير. استخدم النتيجة السابقة وقدّم الإجابة النهائية الآن.\"}".to_string()
                                    } else if fp_hits > MAX_SAME_FINGERPRINT {
                                        "{\"error\": \"حاولت الاستعلام عن نفس الجداول/الشروط عدة مرات بصياغات مختلفة. توقف عن إعادة الصياغة وقدّم إجابة نهائية بالعربية بما لديك، أو اطلب توضيحاً محدداً — ولا تكرر هذا الاستعلام.\"}".to_string()
                                    } else {
                                        let _ = send_message(client, token, chat_id, "⏳ جاري تنفيذ استعلام SQL...".to_string()).await;
                                        let result = execute_raw_sql_for_agent(
                                            sql_query,
                                            app_state,
                                            &crate::agent_tools::ExportDelivery::Telegram {
                                                client: client.clone(),
                                                token: token.to_string(),
                                                chat_id,
                                            },
                                        )
                                        .await;
                                        recent_sql.push(sql_norm);
                                        if recent_sql.len() > 3 { recent_sql.remove(0); }
                                        result.to_string()
                                    };

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": resp_content
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "explore_local_schema" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let table_hint = args.get("table_hint").and_then(|h| h.as_str()).unwrap_or("");
                                    let cache_key = table_hint.to_lowercase();

                                    // Cache hit: avoid re-querying INFORMATION_SCHEMA in same turn
                                    let resp_content = if let Some(cached) = schema_cache.get(&cache_key) {
                                        format!("{{\"cached\": true, \"data\": {}}}", cached)
                                    } else {
                                        let _ = send_message(client, token, chat_id, "⏳ جاري استكشاف قاعدة البيانات المحلية...".to_string()).await;
                                        let func_response_data = explore_local_schema_for_agent(table_hint, app_state).await;
                                        let s = func_response_data.to_string();
                                        schema_cache.insert(cache_key, s.clone());
                                        s
                                    };

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": resp_content
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "search_query_patterns" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let keywords = args.get("keywords").and_then(|k| k.as_str()).unwrap_or("");
                                    let result = search_query_patterns_local(keywords, erp);
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "get_current_datetime" {
                                    let result = get_current_datetime_info();
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "schedule_report" {
                                    let result = handle_schedule_report_tool(args_str, app_state).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "list_scheduled_reports" {
                                    let result = handle_list_scheduled_reports_tool(app_state).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if name == "delete_scheduled_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                                    let schedule_id = args.get("schedule_id").and_then(|v| v.as_str()).unwrap_or("");
                                    let result = handle_delete_scheduled_report_tool(schedule_id, app_state).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                    chat_history.push(tool_resp_msg);
                                } else if crate::agent_tools::is_extended_tool(name) {
                                    let delivery = crate::agent_tools::ExportDelivery::Telegram {
                                        client: client.clone(),
                                        token: token.to_string(),
                                        chat_id,
                                    };
                                    if let Some(func_response_data) =
                                        crate::agent_tools::dispatch_extended_tool(
                                            name, args_str, app_state, delivery,
                                        )
                                        .await
                                    {
                                        let tool_resp_msg = json!({
                                            "role": "tool",
                                            "tool_call_id": tool_call_id,
                                            "content": func_response_data.to_string()
                                        });
                                        current_history.push(tool_resp_msg.clone());
                                        chat_history.push(tool_resp_msg);
                                    }
                                }
                            }
                        } else {
                            // If no function was called, it's a final text response! Extract text and send to Telegram.
                            if let Some(content) = message_text_content(message) {
                                crate::agent_memory::spawn_persist_turn_memories(
                                    user_text.to_string(),
                                    content.clone(),
                                    openai_key.to_string(),
                                    groq_key.to_string(),
                                    crate::agent_memory::build_turn_context_from_history(&current_history),
                                    erp,
                                    crate::supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string(),
                                );
                                let _ = send_html(client, token, chat_id, content).await;
                                return;
                            }
                            let _ = send_message(client, token, chat_id, "رد فارغ من الذكاء الاصطناعي".to_string()).await;
                            return;
                        }
                    } else {
                         let _ = send_message(client, token, chat_id, "لا توجد استجابة صالحة من الذكاء الاصطناعي".to_string()).await;
                         return;
                    }
                } else {
                    let _ = send_message(client, token, chat_id, "فشل في قراءة الرد (JSON Error)".to_string()).await;
                    return;
                }
            }
            Err(e) => {
                let _ = send_message(client, token, chat_id, format!("خطأ في الاتصال بـ OpenRouter: {}", e)).await;
                return;
            }
        }
    }
    
    let _ = send_message(client, token, chat_id, "عذراً، استغرق تحليل البيانات وقتاً أطول من المتوقع. يرجى المحاولة بسؤال أكثر تحديداً.".to_string()).await;
}

// ─── Execute Report locally for the Agent ─────────────────────────────────
async fn execute_db_report(
    report_id: &str,
    param: &str,
    target_date: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
) -> Value {
    let report = match reports_cache.iter().find(|r| r.id == report_id) {
        Some(r) => r,
        None => return json!({ "error": "التقرير غير موجود" }),
    };

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات" });
    }
    let conn = conn_opt.unwrap();

    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    let final_sql = crate::erp_adapters::finalize_supabase_report_sql(
        erp,
        &report.sql_query,
        param,
        target_date,
    );

    match execute_sql_query(conn, final_sql).await {
        Ok(result) => {
            if result.rows.is_empty() {
                json!({ "result": "لا توجد أي بيانات مسجلة لهذا الطلب." })
            } else {
                // To save tokens, only return the first 10 rows to Groq
                let sample_rows: Vec<&Vec<String>> = result.rows.iter().take(10).collect();
                json!({
                    "columns": result.columns,
                    "rows": sample_rows,
                    "total_found": result.rows.len()
                })
            }
        }
        Err(e) => json!({ "error": format!("فشل في تنفيذ الاستعلام: {}", e) })
    }
}

// ─── Generate PDF locally for the Agent ──────────────────────────────────
async fn generate_and_send_pdf_for_agent(
    client: &Client,
    token: &str,
    chat_id: i64,
    report_id: &str,
    param: &str,
    target_date: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
) -> Value {
    let _ = send_message(client, token, chat_id, "⏳ جاري تجهيز التقرير كملف PDF...".to_string()).await;

    let report = match reports_cache.iter().find(|r| r.id == report_id) {
        Some(r) => r,
        None => return json!({ "error": "التقرير غير موجود" }),
    };

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات" });
    }
    let conn = conn_opt.unwrap();

    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    let final_sql = crate::erp_adapters::finalize_supabase_report_sql(
        erp,
        &report.sql_query,
        param,
        target_date,
    );

    match execute_sql_query(conn, final_sql).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "لا توجد أي بيانات مسجلة لإصدار التقرير كملف PDF." });
            }

            match generate_report_pdf(&report.name_ar, &result.columns, &result.rows) {
                Ok(pdf_bytes) => {
                    let filename = format!("{}.pdf", report.name_ar.chars().take(30).collect::<String>().replace(' ', "_"));
                    let caption = format!("📊 *{}*\n✅ {} نتيجة", report.name_ar, result.rows.len());
                    
                    if let Err(e) = send_pdf(client, token, chat_id, &filename, pdf_bytes, &caption).await {
                        json!({ "error": format!("فشل في إرسال ملف PDF إلى تيليجرام: {}", e) })
                    } else {
                        json!({ "result": "تم إنشاء وإرسال ملف PDF للمستخدم بنجاح." })
                    }
                }
                Err(e) => {
                    json!({ "error": format!("فشل في إنشاء ملف PDF: {}", e) })
                }
            }
        }
        Err(e) => json!({ "error": format!("فشل في تنفيذ الاستعلام: {}", e) })
    }
}

async fn generate_custom_pdf_for_agent(
    client: &Client,
    token: &str,
    chat_id: i64,
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Value {
    let _ = send_message(client, token, chat_id, "⏳ جاري تجهيز تقرير PDF مخصص...".to_string()).await;
    match generate_report_pdf(title, columns, rows) {
        Ok(pdf_bytes) => {
            let filename = format!("{}.pdf", title.chars().take(30).collect::<String>().replace(' ', "_"));
            let caption = format!("📊 *{}*\n✅ {} نتيجة", title, rows.len());
            if let Err(e) = send_pdf(client, token, chat_id, &filename, pdf_bytes, &caption).await {
                json!({ "error": format!("فشل في إرسال ملف PDF إلى تيليجرام: {}", e) })
            } else {
                json!({ "result": "تم إنشاء وإرسال ملف PDF للمستخدم بنجاح." })
            }
        }
        Err(e) => json!({ "error": format!("فشل في إنشاء ملف PDF: {}", e) })
    }
}

// ─── Local Agent Loop ──────────────────────────────────────────────────
async fn generate_pdf_report_from_sql_for_agent(
    client: &Client,
    token: &str,
    chat_id: i64,
    title: &str,
    sql_query: &str,
    app_state: &Arc<AppState>,
) -> Value {
    if let Err(e) = validate_read_only_select_sql(sql_query) {
        return json!({ "error": e });
    }

    let _ = send_message(client, token, chat_id, "Preparing the PDF report...".to_string()).await;

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "Not connected to the local database." });
    }
    let conn = conn_opt.unwrap();

    match execute_sql_query(conn, sql_query.to_string()).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "The query returned no rows, so no PDF report was created." });
            }

            match generate_report_pdf(title, &result.columns, &result.rows) {
                Ok(pdf_bytes) => {
                    let filename = format!("{}.pdf", title.chars().take(30).collect::<String>().replace(' ', "_"));
                    let caption = format!("PDF report: {}\nRows: {}", title, result.rows.len());
                    if let Err(e) = send_pdf(client, token, chat_id, &filename, pdf_bytes, &caption).await {
                        json!({ "error": format!("Failed to send PDF to Telegram: {}", e) })
                    } else {
                        json!({ "result": format!("PDF report was generated and sent successfully. Rows: {}", result.rows.len()) })
                    }
                }
                Err(e) => json!({ "error": format!("Failed to generate PDF: {}", e) })
            }
        }
        Err(e) => json!({ "error": format!("Failed to execute the PDF report query: {}", e) })
    }
}

// ─── Generate Excel for the Agent (Telegram) ─────────────────────────────
async fn generate_and_send_excel_for_agent(
    client: &Client,
    token: &str,
    chat_id: i64,
    report_id: &str,
    param: &str,
    target_date: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
) -> Value {
    let _ = send_message(client, token, chat_id, "⏳ جاري تجهيز التقرير كملف Excel...".to_string()).await;

    let report = match reports_cache.iter().find(|r| r.id == report_id) {
        Some(r) => r,
        None => return json!({ "error": "التقرير غير موجود" }),
    };

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات" });
    }
    let conn = conn_opt.unwrap();

    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    let final_sql = crate::erp_adapters::finalize_supabase_report_sql(
        erp,
        &report.sql_query,
        param,
        target_date,
    );

    match execute_sql_query(conn, final_sql).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "لا توجد أي بيانات مسجلة لإصدار التقرير كملف Excel." });
            }

            match generate_report_excel(&report.name_ar, &result.columns, &result.rows) {
                Ok(xlsx_bytes) => {
                    let filename = format!("{}.xlsx", report.name_ar.chars().take(30).collect::<String>().replace(' ', "_"));
                    let caption = format!("📊 *{}*\n✅ {} نتيجة", report.name_ar, result.rows.len());

                    if let Err(e) = send_excel(client, token, chat_id, &filename, xlsx_bytes, &caption).await {
                        json!({ "error": format!("فشل في إرسال ملف Excel إلى تيليجرام: {}", e) })
                    } else {
                        json!({ "result": "تم إنشاء وإرسال ملف Excel للمستخدم بنجاح." })
                    }
                }
                Err(e) => json!({ "error": format!("فشل في إنشاء ملف Excel: {}", e) })
            }
        }
        Err(e) => json!({ "error": format!("فشل في تنفيذ الاستعلام: {}", e) })
    }
}

async fn generate_custom_excel_for_agent(
    client: &Client,
    token: &str,
    chat_id: i64,
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Value {
    let _ = send_message(client, token, chat_id, "⏳ جاري تجهيز تقرير Excel مخصص...".to_string()).await;
    match generate_report_excel(title, columns, rows) {
        Ok(xlsx_bytes) => {
            let filename = format!("{}.xlsx", title.chars().take(30).collect::<String>().replace(' ', "_"));
            let caption = format!("📊 *{}*\n✅ {} نتيجة", title, rows.len());
            if let Err(e) = send_excel(client, token, chat_id, &filename, xlsx_bytes, &caption).await {
                json!({ "error": format!("فشل في إرسال ملف Excel إلى تيليجرام: {}", e) })
            } else {
                json!({ "result": "تم إنشاء وإرسال ملف Excel للمستخدم بنجاح." })
            }
        }
        Err(e) => json!({ "error": format!("فشل في إنشاء ملف Excel: {}", e) })
    }
}

async fn generate_excel_report_from_sql_for_agent(
    client: &Client,
    token: &str,
    chat_id: i64,
    title: &str,
    sql_query: &str,
    app_state: &Arc<AppState>,
) -> Value {
    if let Err(e) = validate_read_only_select_sql(sql_query) {
        return json!({ "error": e });
    }

    let _ = send_message(client, token, chat_id, "Preparing the Excel report...".to_string()).await;

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "Not connected to the local database." });
    }
    let conn = conn_opt.unwrap();

    match execute_sql_query(conn, crate::agent_tools::prepare_sql_batch(sql_query)).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "The query returned no rows, so no Excel report was created." });
            }

            match generate_report_excel(title, &result.columns, &result.rows) {
                Ok(xlsx_bytes) => {
                    let filename = format!("{}.xlsx", title.chars().take(30).collect::<String>().replace(' ', "_"));
                    let caption = format!("Excel report: {}\nRows: {}", title, result.rows.len());
                    if let Err(e) = send_excel(client, token, chat_id, &filename, xlsx_bytes, &caption).await {
                        json!({ "error": format!("Failed to send Excel to Telegram: {}", e) })
                    } else {
                        json!({ "result": format!("Excel report was generated and sent successfully. Rows: {}", result.rows.len()) })
                    }
                }
                Err(e) => json!({ "error": format!("Failed to generate Excel: {}", e) })
            }
        }
        Err(e) => json!({ "error": format!("Failed to execute the Excel report query: {}", e) })
    }
}

fn ai_cancelled(cancel_rx: &mut Option<tokio::sync::oneshot::Receiver<()>>) -> bool {
    if let Some(rx) = cancel_rx {
        match rx.try_recv() {
            Ok(()) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => true,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => false,
        }
    } else {
        false
    }
}

fn response_defers_tool_execution(content: &str) -> bool {
    const MARKERS: &[&str] = &[
        "سأقوم", "سأنفذ", "سأستخدم", "سأبحث", "سأستعرض", "سأبدأ", "سوف ",
        "الآن س", "في الخطوة", "سأقوم ب",
    ];
    MARKERS.iter().any(|m| content.contains(m))
}

fn response_claims_query_results(content: &str) -> bool {
    content.contains("تم تنفيذ")
        || content.contains("|---|")
        || content.contains("| # |")
        || (content.contains("د.ل") && content.contains('|'))
        || content.contains("📊")
}

pub fn replace_file_path_tags(content: &str, path: &str) -> String {
    let tag = "[FILE_PATH:";
    let mut out = String::new();
    let mut rest = content;
    while let Some(idx) = rest.find(tag) {
        out.push_str(&rest[..idx]);
        if let Some(end) = rest[idx..].find(']') {
            out.push_str(&format!("[FILE_PATH:{}]", path));
            rest = &rest[idx + end + 1..];
        } else {
            out.push_str(&rest[idx..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

async fn sanitize_response_file_paths(content: &str, app_state: &Arc<AppState>) -> String {
    sanitize_response_file_paths_ex(content, app_state, false).await
}

/// نسخة موسّعة — force=true تضيف المسار دائماً بعد عمليات التصدير
async fn sanitize_response_file_paths_ex(
    content: &str,
    app_state: &Arc<AppState>,
    force: bool,
) -> String {
    let valid_path = {
        let session = app_state.agent_session.lock().await;
        session.last_file_path.clone()
    };
    let Some(vp) = valid_path.filter(|p| std::path::Path::new(p).exists()) else {
        return content.to_string();
    };
    if content.contains("[FILE_PATH:") {
        return replace_file_path_tags(content, &vp);
    }
    // كلمات مفتاحية تدل على ملف تصدير
    let export_keywords = [
        "xlsx", "Excel", "اكسل", "إكسل", "PDF", "pdf",
        "تصدير", "تم التصدير", "تم إنشاء", "تم حفظ", "الملف",
        "ملف التقرير", "جاهز", "حُفظ",
    ];
    let has_keyword = export_keywords.iter().any(|k| content.contains(k));
    if force || has_keyword {
        format!("{}\n\n[FILE_PATH:{}]", content.trim_end(), vp)
    } else {
        content.to_string()
    }
}

pub async fn handle_with_groq_local(
    user_text: &str,
    mut chat_history: Vec<Value>,
    groq_key: &str,
    ai_model: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
    app_handle: tauri::AppHandle,
    request_id: &str,
    openai_key: &str,
    mut cancel_rx: Option<tokio::sync::oneshot::Receiver<()>>,
    advanced_mode: bool,
) -> Result<String, String> {
    if groq_key.trim().is_empty() {
        return Err(
            "مفتاح OpenRouter غير متوفر. تحقق من اتصال الإنترنت أو راجع إعدادات المطوّر."
                .to_string(),
        );
    }

    let erp = crate::erp_profile::current_erp_kind(app_state).await;

    let access_token = match crate::supabase_config::read_stored_access_token(&app_handle, crate::decrypt_value_internal) {
        Ok(Some(token)) => token,
        _ => crate::supabase_config::DEFAULT_APP_ACCESS_TOKEN.to_string(),
    };

    if let Some(reply) = try_handle_greeting(user_text, erp) {
        return Ok(reply);
    }

    let agent_file = erp.agent_file_label();

    let mut schema_info = if advanced_mode {
        format!(
            "ERP النشط: {erp_name} | ملف الأنماط: {agent_file}\n\
            {domain}\n\n\
            لا يُحقَن DDL إضافي تلقائياً — DOMAIN_CRITICAL_FACTS أعلاه كافٍ لمعظم الأسئلة. \
            استخدم search_query_patterns ثم run_query_pattern قبل أي استكشاف. \
            ناد search_schema فقط إذا احتجت جدولاً غير مذكور في DOMAIN_CRITICAL_FACTS.",
            erp_name = erp.display_name_ar(),
            agent_file = agent_file,
            domain = crate::erp_profile::domain_critical_facts(erp),
        )
    } else {
        format!(
            "ERP النشط: {erp_name} | ملف الأنماط: {agent_file}\n\
            الوضع السريع: نفّذ run_query_pattern مباشرة — لا استكشاف schema ولا SQL حر.",
            erp_name = erp.display_name_ar(),
            agent_file = agent_file,
        )
    };

    let session_pf = {
        let s = app_state.agent_session.lock().await;
        s.last_product_filter.clone()
    };
    let active_pf = crate::agent_tools::extract_product_hint_from_text(user_text).or(session_pf);

    let product_note = if let Some(ref pf) = active_pf {
        schema_info.push_str(&format!(
            "\n\n### منتج نشط\n**product_filter=\"{pf}\"** — مرّره لـ run_query_pattern.",
            pf = pf,
        ));
        format!(
            "\n\n<product_filter>\nproduct_filter=\"{pf}\" — استخدمه في run_query_pattern.\n</product_filter>",
            pf = pf
        )
    } else {
        String::new()
    };

    if advanced_mode {
        if let Some(ref pf) = active_pf {
            let fallback = if erp == crate::erp_profile::ErpKind::InfinityRetailDb {
                "إن لم توجد مشتريات، اعرض صف Data_Products (ProductName، ProductCode، UomLastCost)."
            } else {
                "إن لم توجد مشتريات، اعرض صف ITEMS (اسم، كود، LAST_COST، AVER_COST)."
            };
            schema_info.push_str(&format!(
                "\n\n### منتج نشط في المحادثة\n\
                **product_filter=\"{pf}\"** — مرّره لـ run_query_pattern / plan_complex_query. \
                لـ «كل شي» أو «تفاصيل كاملة» بدون @: نفّذ plan_complex_query + execute_query_plan فوراً. \
                {fallback}",
                pf = pf,
                fallback = fallback,
            ));
        }
    }

    let memory_block = if advanced_mode {
        crate::agent_memory::recall_memory_prompt_block(user_text, openai_key, erp, &access_token).await
    } else {
        String::new()
    };

    let system_instruction = if advanced_mode {
        if erp == crate::erp_profile::ErpKind::InfinityRetailDb {
            format!(
                "{domain}\n\n\
<role>\n\
مساعد تقارير InfinityRetailDB (Desktop). ملف الأنماط: `{agent_file}`.\n\
نفّذ search_query_patterns → run_query_pattern قبل SQL حر. لا تستخدم dbo.ITEMS أو SALE_INVOICE.\n\
</role>\n\n\
<tone_and_dialect>\n\
- **لهجة ليبية خفيفة وقصيرة جداً (لتوفير التوكنز):**\n\
  - ابدأ بترحيب ليبي خفيف ومختصر للغاية (مثل: 'مرحبتين.' أو 'أهلاً بيك. تفضل:').\n\
  - ممنوع نهائياً التملق، التحيات الطويلة، الكلام الفارغ، أو صيغ التبجيل والمبالغة (مثل 'يا فندم'، 'يسعدني خدمتكم'، 'بدقة متناهية').\n\
  - اعرض نتائج البيانات فوراً واختصر قدر الإمكان لتقليل استهلاك التوكنز.\n\
  - اقترح التصدير أو الخطوة التالية باختصار شديد ودون إطالة (مثل: 'تبيه إكسل أو PDF؟').\n\
</tone_and_dialect>\n\n\
<critical_rules>\n\
1. **run_query_pattern أولاً** للأنماط في AGENT_InfinityRetailDB.md.\n\
2. **get_product_schema / get_database_views** → INFINITY_* docs للاتصال النشط.\n\
3. **plan_complex_query** → **execute_query_plan** لدراسة منتج (Purchase.* للمشتريات).\n\
4. إيراد = SUM(QYT*UnitPrice) | تاريخ = MAX(SalesInvoiceDate).\n\
5. @mention → product_filter | validate_sql قبل execute_raw_sql.\n\
6. export_last_result / save_favorite_query عند الطلب.\n\
7. احسب الإجماليات الفرعية والعامة للنتائج واعرضها بوضوح باختصار في نهاية ردك.\n\
</critical_rules>\n\n\
<schema>\n{schema_info}\n{product_note}\n</schema>\n\
{memory_block}",
                domain = slim_domain_facts(erp),
                agent_file = erp.agent_file_label(),
                product_note = product_note,
            )
        } else {
        format!(
        "<DOMAIN_CRITICAL_FACTS>\n\
**READ THIS FIRST.** This is your pre-loaded knowledge of the Marketing2026 database. \
Use this as your primary reference. Only call `search_schema` if the answer needs a table NOT listed here.\n\n\
### 📚 Core Tables (memorize these — they cover 90% of all queries)\n\n\
**`dbo.ITEMS`** — Product master (محتوى الكتالوج).\n\
```\n\
ITEM_ID int PK, ITEM_MODEL varchar(50)=code, ITEM_NAME varchar(800),\n\
LAST_COST float=آخر سعر شراء, AVER_COST float=متوسط التكلفة,\n\
MIN_LEVEL float, MAX_LEVEL float, ITEM_INVISIBLE bit=محذوف,\n\
ITEM_UPDATE_DATE datetime, USERS_ID, PLACE varchar(30)\n\
```\n\
Use `WHERE ITEM_NAME LIKE '%word%' OR ITEM_MODEL LIKE '%word%'` for product searches (never `=`).\n\n\
**`dbo.ITEMS_SUB`** — Current stock per store + batch + expiry. THE source of truth for stock.\n\
```\n\
ITEM_SUB_ID PK, ITEM_ID→ITEMS, STORE_ID→STORES, QTY float=الكمية,\n\
CATEOGRY1 varchar(10)=Batch رقم الدفعة,\n\
CATEOGRY2 varchar(10)=Sub-batch,\n\
CATEOGRY3 datetime=EXPIRY DATE تاريخ الصلاحية ⭐ (has INDEX)\n\
```\n\n\
**`dbo.BUY_INVOICE`** — Purchase invoices (فواتير الشراء = ما اشتريته من الموردين).\n\
```\n\
B_ID int PK, B_DATE datetime=تاريخ الفاتورة,\n\
CUST_ID→CUSTOMERS=المورد, USERS_ID→USERS=من أدخل الفاتورة,\n\
COMM_ID smallint (always 0 — IGNORE),\n\
B_DISCOUNT, B_SPENT, S_REF_NO=رقم مرجعي للفاتورة\n\
```\n\n\
**`dbo.BUY_ITEMS`** — Purchase line items (بنود فواتير الشراء).\n\
```\n\
B_ITEM_ID PK, B_ID→BUY_INVOICE, ITEM_ID→ITEMS, STORE_ID→STORES,\n\
QTY float=الكمية, PRICE float=سعر الشراء,\n\
CATEOGRY1=Batch, CATEOGRY3 datetime=ExpiryDate,\n\
BARCODE varchar(30), UNIT_ID, CURRENCY_ID, RATE float\n\
```\n\n\
**`dbo.SALE_INVOICE`** — Sales invoices (فواتير المبيعات).\n\
```\n\
S_ID int PK, S_DATE datetime=تاريخ البيع ⭐,\n\
CUST_ID→CUSTOMERS=الزبون, CUST_NAME varchar(100)=اسم الزبون مباشرة,\n\
USERS_ID=من أدخل الفاتورة, S_DISCOUNT, S_TAX1, S_TAX2, S_SHIPMENT,\n\
S_NOTE, S_STATUES tinyint, WAIT bit=معلَّقة\n\
```\n\n\
**`dbo.SALE_ITEMS`** — Sales line items (بنود فواتير المبيعات).\n\
```\n\
S_ITEM_ID PK, S_ID→SALE_INVOICE ⭐ (use this to join for date),\n\
ITEM_ID→ITEMS, STORE_ID, QTY, PRICE=سعر البيع,\n\
LAST_COST, AVER_COST, PUBLIC_PRICE,\n\
S_TIME datetime=وقت البيع (per-item, optional),\n\
CATEOGRY3=ExpiryDate, BARCODE\n\
```\n\
⚠️ **SALE_ITEMS does NOT have S_DATE.** For date filters, JOIN to SALE_INVOICE via S_ID.\n\n\
**`dbo.CUSTOMERS`** — Suppliers + customers (table unifié). \"مندوب\" in chat usually = supplier (مورد).\n\
```\n\
CUST_ID PK, CUST_NAME varchar(100), CUST_NO varchar(20),\n\
CUST_VENDOR bit=1 لو مورد, CUST_CUSTOM bit=1 لو زبون,\n\
CUST_EMP bit=1 لو موظف, CUST_MAX_DEBIT float=حد الدين,\n\
BL_DEBIT float, BL_CREDIT float=رصيد ابتدائي,\n\
ACC_ID→ACCOUNTS (⚠️ BALANCE_C فارغ — احسب الديون من TAKE/GIVE + الفواتير)\n\
```\n\n\
**`dbo.USERS`** — Internal employees (موظفو الشركة).\n\
```\n\
USERS_ID PK, FULL_NAME varchar(50), USER_NAMES varchar(20)\n\
```\n\n\
**`dbo.STORES`** — Stores/warehouses (المخازن).\n\
```\n\
STORE_ID PK, STORE_NAME varchar(50)\n\
```\n\n\
**`dbo.BALANCE_C`** — ⚠️ **فارغ في هذه القاعدة** (0 صف). لا تستخدمه لحساب ديون الزبائن/الموردين.\n\
```\n\
ACC_ID, ACC_DEBIT, ACC_CREDIT, BALANCE — غير مُحدَّث هنا\n\
```\n\
**البديل الإلزامي للديون:** `run_query_pattern(\"متابعة الديون\")` (تقرير شامل: لي + علي + تفاصيل) أو صيغة: مبيعات−مردودات−TAKE+BALANCE_EDIT (لي) / مشتريات−مردودات−GIVE+BALANCE_EDIT (علي).\n\
**ديون الموردين فقط (مبسّط — عمودان: اسم المورد + الدين):** `run_query_pattern(\"ديون الموردين\")` — استخدمه عندما يطلب المستخدم «ديون الموردين فقط» أو «اسم المورد والدين» أو لا يحتاج تواريخ/مقبوضات. لا تضف أي عمود آخر مهما طلب لاحقاً — هذا النمط مغلق على عمودين.\n\n\
**`dbo.R_S_INVOICE`** / **`dbo.R_S_ITEMS`** — Sales return invoices & items (مردودات المبيعات).\n\
- `R_S_INVOICE`: `S_R_ID` PK, `S_R_DATE` datetime, `CUST_ID`→CUSTOMERS, `CUST_NAME`, `USERS_ID`→USERS, `S_R_NOTE`.\n\
- `R_S_ITEMS`: `S_R_ITEM_ID` PK, `S_R_ID`→R_S_INVOICE, `ITEM_ID`→ITEMS, `STORE_ID`→STORES, `QTY` float, `PRICE` float=Returned Price, `CATEOGRY3` datetime=Expiry.\n\n\
**`dbo.B_R_INVOICE`** / **`dbo.B_R_ITEMS`** — Purchase return invoices & items (مردودات المشتريات للموردين).\n\
- `B_R_INVOICE`: `B_R_ID` PK, `B_R_DATE` datetime, `CUST_ID`→CUSTOMERS, `USERS_ID`→USERS, `B_R_NOTE`.\n\
- `B_R_ITEMS`: `B_R_ITEM_ID` PK, `B_R_ID`→B_R_INVOICE, `ITEM_ID`→ITEMS, `STORE_ID`→STORES, `QTY` float, `PRICE` float=Return Price, `CATEOGRY3` datetime=Expiry.\n\n\
**`dbo.SPOIL_INVOICE`** / **`dbo.SPOIL_ITEMS`** — Spoiled/Damaged products (الأدوية التالفة/المتلفة).\n\
- `SPOIL_INVOICE`: `SP_ID` PK, `SP_DATE` datetime, `SP_NOTE` varchar, `USERS_ID`.\n\
- `SPOIL_ITEMS`: `SP_ITEM_ID` PK, `SP_ID`→SPOIL_INVOICE, `ITEM_ID`→ITEMS, `QTY` float, `STORE_ID`→STORES, `CATEOGRY3` datetime=Expiry, `PRICE`, `LAST_COST`, `AVER_COST`.\n\n\
**`dbo.TRANSFER_INVOICE`** / **`dbo.TRANSFER_ITEMS`** — Warehouse transfer logs (تحويلات الأصناف بين المخازن).\n\
- `TRANSFER_INVOICE`: `TR_ID` PK, `TR_DATE` datetime, `TR_NOTE` varchar, `USERS_ID`.\n\
- `TRANSFER_ITEMS`: `TR_ITEM_ID` PK, `TR_ID`→TRANSFER_INVOICE, `ITEM_ID`→ITEMS, `QTY`, `STORE_F_ID`→STORES(from), `STORE_T_ID`→STORES(to), `CATEOGRY3` datetime=Expiry.\n\n\
**`dbo.UNITS`** — Product units (الوحدات: علبة، شريط...).\n\
- `UNIT_ID` PK, `UNIT_DISC` varchar=Name of unit, `UNIT_QTY` float.\n\n\
**`dbo.SITTEINGS`** — Global settings (الإعدادات العامة — note spelling with double 'T' and 'EI').\n\
- `A_NAME`=Company Name, `PHONE`, `MOBILE`, `FAX`. WARNING: Has NO ID column, contains exactly ONE row. NEVER try to JOIN it on items/invoices.\n\n\
### 🚨 Critical naming conventions\n\
- **`CATEOGRY3 datetime` = EXPIRY DATE.** Despite the name, it's NOT a category. It appears in: `ITEMS_SUB`, `BUY_ITEMS`, `SALE_ITEMS`, `JARED_ITEMS_B`, `MANF_F_ITEMS`, `C_BUY_ITEMS`, and all `*_DELETED` variants. The category lookup table (with categories proper) is `dbo.CATEOGRY3` (separate).\n\
- **Quantity column is `QTY`** (never Quantity, ITEM_QTY, AVAILABLE_QTY, Total_Quantity).\n\
- **`COMMISSIONER` table is UNUSED.** All `COMM_ID = 0`. \"مندوب\" → `CUSTOMERS WHERE CUST_VENDOR = 1`, not COMMISSIONER.\n\
- **`Invoice_Items` (mixed case)** is a SCRATCH table for in-progress edits, with `Expiry varchar(30)`. NEVER use it for stock expiry queries. Always use `ITEMS_SUB.CATEOGRY3` for that.\n\
- **`IS` is a SQL Server reserved word.** Don't use it as a table alias. Use `S`, `SUB`, `ITM`, etc.\n\
- **Product search: use `LIKE '%X%'` not `= 'X'`.** Names have variable spacing, case, etc.\n\n\
### 🎯 Reusable query templates\n\
For SQL templates use `run_query_pattern` or `search_query_patterns` — do NOT embed long SQL in answers.\n\
Key patterns: **آخر منتجات بيعت اليوم**, مبيعات يومية موظف, متابعة الديون, ملخص مالي شهري, نواقص نشطة مورد, مقارنة أسعار موردين, رواتب, مصروفات, طلبية شراء ذكية.\n\
**Debts:** NEVER `BALANCE_C` (empty). لي = Sales−Returns−TAKE+BALANCE_EDIT.\n\
**Monthly expenses (مصاريف شهرية):** Paid salary receipts = `dbo.GIVE` WHERE `EXPENCES_ID=1`. Operational/private = `GIVE` WHERE `EXPENCES_ID>0 AND EXPENCES_ID<>1` AND `G_STATUES=1`. ⚠️ `EXPENCES_ID=0` = supplier payments NOT expenses. ⚠️ `SALARIES` is **empty**.\n\
**Pattern:** `run_query_pattern(\"ملخص مالي شهري\")` → 4 parts: debts + salary receipts + operational + summary. SQL file: `monthly_expenses_tracking.sql`.\n\
{}\n\
{}\n\
{}\n\
**Date anchor:** `@AsOfDate = MAX(S_DATE)` from SALE_INVOICE — **query at runtime**, never assume 2026-04-07.\n\n\
### 🔍 أنماط الاستعلامات المعقدة\n\
للاستعلامات المعقدة (طلبية شراء ذكية، متابعة الديون، متابعة النواقص، نواقص نشطة مورد، مقارنة أسعار موردين، تقرير الصلاحية، الجرد الفعلي، الرواتب، التصنيع، الربحية، حركة صنف): \
**استدعِ أداة `search_query_patterns` أولاً** بكلمات مفتاحية مناسبة، ثم طبّق القالب الذي تُعيده. \
هذه الأداة تُعيد SQL كاملاً مختبراً جاهزاً للتعديل والتنفيذ — لا تكتب هذه الاستعلامات من الصفر.\n\n\
**Date ranges:** NEVER hardcode last sale date. Always run `SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE` or use `@LastSaleDay` in patterns. Old docs said 2026-04-07 — **ignore**; live DB may be 2026-05-21 or later.\n\
**Last products sold today / آخر منتجات بيعت اليوم:** `run_query_pattern(\"آخر منتجات بيعت اليوم\")` — بنود SALE_ITEMS_INVOICE_VIEW ليوم `@SaleDay=GETDATE()`، الأحدث أولاً. **ليس** نمط موظفين. إن فارغ → `@SaleDay=MAX(S_DATE)`.\n\
**Last sale day by employee:** `run_query_pattern(\"مبيعات آخر يوم موظف\")` — file `last_sale_day_by_employee.sql`.\n\
**Active shortages (supplier + last buy price):** `run_query_pattern(\"نواقص نشطة مورد\")` — file `active_shortage_tracking.sql`. Use when user asks for اسم المنتج، الكمية، آخر سعر شراء، المورد for items that are short AND still selling.\n\
**Supplier price comparison (one product):** `run_query_pattern(\"مقارنة أسعار موردين\", product_filter=\"...\")` — file `supplier_price_comparison.sql`. Use when user asks to compare buy prices across suppliers for ONE product (@mention or name fragment). NOT the same as نواقص نشطة مورد.\n\
</DOMAIN_CRITICAL_FACTS>\n\n\
<role>\n\
You are a senior SQL Server analyst for a pharmaceutical distribution company. \
Your job: convert Arabic questions into precise T-SQL queries against the Marketing2026 database, \
execute them via the `execute_raw_sql` tool, and report results back in Arabic.\n\
</role>\n\n\
<schema>\n\
{}\n\
</schema>\n\n\
{}\n\n\
<critical_rules>\n\
1. **EXECUTE FIRST — never defer.** Call a tool (run_query_pattern / execute_raw_sql / create_excel_report) in the SAME turn. \
NEVER reply with \"سأنفذ/سأبحث/سأستخدم\" without an immediate tool call.\n\n\
2. **No clarifying questions.** Do NOT ask the user for confirmation, dates, or names. Use defaults: @AsOfDate=MAX(S_DATE), TOP 20, current month via get_current_datetime. \
For product details (@mention or name): run plan_complex_query + execute_query_plan OR run_query_pattern immediately — never ask \"هل تريد 1/2/3/4\".\n\n\
2b. **@mention products:** User @mentions may include empty `( )` when code is blank — server strips them into `product_filter`. \
Never paste raw `@NAME ()` into SQL. Use `LIKE N'%NAME%'` with cleaned name. \
**موردي/موردين + @product** → `run_query_pattern(\"مقارنة أسعار موردين\", product_filter=...)`.\n\n\
3. **NEVER invent column or table names.** Every column/table in your SQL must appear in <DOMAIN_CRITICAL_FACTS>. \
Prefer run_query_pattern over explore_local_schema/search_schema.\n\n\
4. **SQL Server syntax only.** Use `TOP N` (never `LIMIT`). Column names UPPERCASE_SNAKE_CASE — copy verbatim.\n\n\
5. **One query at a time.** Never call `execute_raw_sql` in parallel for the same logical question.\n\n\
6. **Domain knowledge — primary tables:**\n\
   - Products catalogue → `dbo.ITEMS` (ITEM_ID, ITEM_MODEL, ITEM_NAME, LAST_COST, AVER_COST)\n\
   - **Stock per store** → `dbo.ITEMS_SUB` (ITEM_ID, STORE_ID, QTY, CATEOGRY1=Batch, CATEOGRY3=ExpiryDate)\n\
   - Purchase invoices → `dbo.BUY_INVOICE` (B_ID, B_DATE, CUST_ID, USERS_ID)\n\
   - Purchase line items → `dbo.BUY_ITEMS` (B_ITEM_ID, B_ID, ITEM_ID, QTY, PRICE, BARCODE, CATEOGRY3=ExpiryDate)\n\
   - Sales line items → `dbo.SALE_ITEMS` (S_ITEM_ID, S_ID, ITEM_ID, QTY, PRICE, CATEOGRY3=ExpiryDate)\n\
   - Customers/suppliers → `dbo.CUSTOMERS` (CUST_ID, CUST_NAME, CUST_VENDOR, CUST_CUSTOMER)\n\
   - Users (employees) → `dbo.USERS` (USERS_ID, FULL_NAME)\n\n\
7. **لهجة ليبية خفيفة وقصيرة جداً (لتوفير التوكنز):**\n\
   - ابدأ الرد بترحيب ليبي خفيف ومختصر للغاية (مثل: 'مرحبتين بيك.' أو 'أهلاً بيك أخي. تفضل النتائج:').\n\
   - ممنوع نهائياً التملق، التحيات الطويلة، الكلام الفارغ، أو صيغ التبجيل والمبالغة (مثل 'يا فندم'، 'يسعدني خدمتكم'، 'بدقة متناهية').\n\
   - اعرض نتائج البيانات والأرقام فوراً واختصر قدر الإمكان لتقليل استهلاك التوكنز.\n\
   - اقترح التصدير أو الخطوة التالية باختصار شديد ودون إطالة (مثل: 'تبيه إكسل أو PDF؟' أو 'نزيدك تقرير الديون؟').\n\
8. **الأسئلة العامة والاستشارية:** أجب باختصار بلهجة ليبية ودية وموجزة دون أدوات قاعدة البيانات.\n\
9. احسب الإجماليات الفرعية والعامة للنتائج واعرضها بوضوح باختصار في نهاية ردك.\n\
10. **فهم النية وتجنب الجمود:** إذا طلب المستخدم مبيعات اليوم الحالي ولم تكن هناك فواتير مسجلة اليوم، لا تقل «لا توجد مبيعات» صامتة، بل ابحث عن آخر تاريخ مبيعات نشط `MAX(S_DATE)` وعالجه بلطف: «بناءً على آخر البيانات المسجلة بتاريخ [التاريخ]، تفضل تقرير المبيعات...».\n\
**FILE_PATH:** copy the EXACT path from the tool result — NEVER invent paths like `C:\\Users\\User\\...`. \
Append `[FILE_PATH:actual_path]` only when a file was saved.\n\n\
8. **Excel Reports.** When user asks for Excel/اكسل/xlsx: call create_excel_report or export_last_result — do NOT describe the file without creating it.\n\
9. **Large results (>25 rows).** Server auto-creates PDF — summarize preview rows and include [FILE_PATH] from tool result. Do NOT dump all rows in chat.\n\
10. **Advanced tools.** run_query_pattern FIRST for: ديون، مصاريف، مبيعات موظف، مقارنة أسعار موردين (مع product_filter)، نواقص نشطة مورد، متابعة النواقص، طلبية شراء. \
Avoid search_schema/get_table_sample unless run_query_pattern failed.\n\n\
11. **Save to favorites (MANDATORY behavior).** When the user asks to save, store, remember, or favorite a query — using ANY of these triggers: \
«احفظ»، «خزّن»، «خزن»، «خزنه»، «احفظه»، «ضعه في المفضلة»، «ضيف للمفضلة»، «أضفه للمحفوظات»، «أريد تخزينه»، «احفظ هذا الاستعلام»، «save», «remember», «store this» \
— you MUST call `save_favorite_query` IMMEDIATELY in the same turn, using the EXACT SQL from the last successful `execute_raw_sql` or `run_query_pattern` call in this conversation. \
Generate a short descriptive Arabic `name` (max 50 chars) from the user's intent (e.g. «مبيعات اليوم حسب الموظف»، «نواقص المخزون النشطة»). \
Add a brief `description` (one sentence) explaining what it does. \
**Never** reply «تم الحفظ» without actually calling the tool. **Never** ask «هل تريد حفظه؟» — just save it. \
If no successful SELECT was executed yet in this conversation, run it first then save in the next turn.\n\
</critical_rules>\n\n\
<workflow>\n\
1. Identify the request type.\n\
2. **Known pattern?** → `run_query_pattern` immediately (**آخر منتجات بيعت اليوم** → \"آخر منتجات بيعت اليوم\", آخر يوم مبيعات → \"مبيعات آخر يوم موظف\", ديون/مصاريف → \"ملخص مالي شهري\", مبيعات موظف → \"مبيعات يومية موظف\", مقارنة أسعار موردين → \"مقارنة أسعار موردين\" + product_filter, نواقص+مورد+سعر → \"نواقص نشطة مورد\", متابعة نواقص → \"متابعة النواقص\", ديون → \"متابعة الديون\").\n\
3. **Need today's date?** → `get_current_datetime` then execute SQL.\n\
4. **Custom query?** → ONE `execute_raw_sql` using DOMAIN_CRITICAL_FACTS columns.\n\
5. **Export?** → `export_last_result` or `create_excel_report` after data is ready.\n\
6. **Answer in Arabic** with numbers from tool results — never guess.\n\
</workflow>\n\n\
<examples>\n\
<example>\n\
User: \"كم منتج عندي في المخزن؟\"\n\
You think: \"كمية المخزون = stock quantity. From rule 5, that's dbo.ITEMS_SUB with column QTY.\"\n\
Your SQL: `SELECT SUM(QTY) AS TotalQty FROM dbo.ITEMS_SUB`\n\
</example>\n\n\
<example>\n\
User: \"اعرض 10 منتجات منتهية الصلاحية\"\n\
You think: \"Expired stock means CATEOGRY3 (datetime) < today in dbo.ITEMS_SUB. Join to dbo.ITEMS for the product name.\"\n\
Your SQL: `SELECT TOP 10 I.ITEM_NAME, S.CATEOGRY3 AS Expiry, S.CATEOGRY1 AS Batch, S.QTY FROM dbo.ITEMS_SUB S LEFT JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID WHERE S.CATEOGRY3 IS NOT NULL AND S.CATEOGRY3 < GETDATE() AND S.QTY > 0 ORDER BY S.CATEOGRY3 ASC`\n\
</example>\n\n\
<example>\n\
User: \"اعرض آخر فاتورة شراء\"\n\
You think: \"Latest BUY_INVOICE by B_DATE, joined to CUSTOMERS for supplier name.\"\n\
Your SQL: `SELECT TOP 1 B.B_ID, B.B_DATE, CU.CUST_NAME AS Supplier FROM dbo.BUY_INVOICE B LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID ORDER BY B.B_DATE DESC`\n\
</example>\n\n\
<example>\n\
User: \"اعطني طلبية شراء ذكية لمدة 30 يوم تغطية\"\n\
You think: \"Smart purchase order from <DOMAIN_CRITICAL_FACTS> SMART_PURCHASE_ORDER template. @CoverageDays=30, @DaysRecent=60, @AsOfDate=MAX(S_DATE). Stock=ITEMS_SUB, net sales minus R_S returns.\"\n\
Your SQL: copy the DECLARE + CTE template from the \"طلبية شراء ذكية\" section; set `@CoverageDays=30`.\n\
</example>\n\n\
<example>\n\
User: \"اعرض متابعة النواقص\"\n\
You think: \"Shortage monitoring template — ITEMS_SUB stock vs MIN_LEVEL, recent net sales, status نفاد/تحت الحد الأدنى. NOT the smart purchase template.\"\n\
Your SQL: copy the \"متابعة النواقص\" DECLARE + CTE from <DOMAIN_CRITICAL_FACTS>.\n\
</example>\n\n\
<example>\n\
User: \"اعطني النواقص النشطة مع المورد وآخر سعر شراء\"\n\
You think: \"Active shortages with supplier — must filter items still selling + short stock. Supplier from last BUY_INVOICE, NOT GIVE.\"\n\
Your action: `run_query_pattern(\"نواقص نشطة مورد\")` — file `active_shortage_tracking.sql`.\n\
</example>\n\n\
<example>\n\
User: \"قارن أسعار الموردين لـ TRAMADOL NORMON\"\n\
You think: \"Single-product supplier price comparison — BUY_ITEMS grouped by CUST_ID, NOT GIVE. Need product_filter.\"\n\
Your action: `run_query_pattern(\"مقارنة أسعار موردين\", product_filter=\"TRAMADOL NORMON\")` — file `supplier_price_comparison.sql`.\n\
</example>\n\n\
<example>\n\
User: \"ما هي الديون التي لي والتي علي؟\"\n\
You think: \"Debts template — TAKE/GIVE + invoice totals. BALANCE_C is empty. لي=CUST_CUSTOM, علي=CUST_VENDOR.\"\n\
Your SQL: copy the \"متابعة الديون\" UNION query from <DOMAIN_CRITICAL_FACTS>.\n\
</example>\n\
</examples>\n\n\
<anti_examples>\n\
❌ NEVER: `SELECT SUM(Quantity) FROM dbo.ITEMS` — column is QTY, not Quantity, and ITEMS doesn't track stock anyway.\n\
❌ NEVER: parallel-calling execute_raw_sql for 5 tables guessing column names.\n\
❌ NEVER: `SELECT * FROM products WHERE expiry_date < NOW()` — wrong table name, wrong column name, wrong function.\n\
❌ NEVER: using `GETDATE()` alone as the sales window end when computing velocity — data ends at MAX(S_DATE).\n\
❌ NEVER: `FROM dbo.SALARIES` for paid salary receipts — table is empty; use `GIVE WHERE EXPENCES_ID=1`.\n\
❌ NEVER: treating `GIVE WHERE EXPENCES_ID=0` as operational expense — those are supplier purchase payments.\n\
❌ NEVER: claiming \"لا توجد بيانات\" before running an actual query.\n\
❌ NEVER: replying \"تم الحفظ\" without actually calling `save_favorite_query` — that's a lie.\n\
❌ NEVER: asking \"هل تريد حفظ هذا الاستعلام؟\" — when the user said \"احفظ\", just save it immediately.\n\
</anti_examples>\n\n\
<save_examples>\n\
<example>\n\
Context: User just received results from a sales-by-employee query.\n\
User: \"احفظ هذا الاستعلام\"\n\
Your action: `save_favorite_query(name=\"مبيعات اليوم حسب الموظف\", sql_query=<EXACT last SQL>, description=\"إجمالي مبيعات كل موظف في آخر يوم تسجيل فواتير.\")`\n\
Reply AFTER tool succeeds: «✅ حُفظ في المحفوظات باسم «مبيعات اليوم حسب الموظف» — يمكنك تشغيله من تبويب المحفوظات.»\n\
</example>\n\n\
<example>\n\
User: \"ضعه في المفضلة باسم تقرير الديون اليومي\"\n\
Your action: `save_favorite_query(name=\"تقرير الديون اليومي\", sql_query=<EXACT last SQL>, description=\"...\")` — use the user-provided name verbatim.\n\
</example>\n\
</save_examples>",
        MONTHLY_EXPENSES_TEMPLATE,
        ACTIVE_SHORTAGE_TRACKING_TEMPLATE,
        SUPPLIER_PRICE_COMPARISON_TEMPLATE,
        schema_info,
        memory_block
        )
        }
    } else {
        build_fast_system_prompt(&schema_info, active_pf.as_deref(), erp)
    };

    let tools_json = json!([
        {
            "type": "function",
            "function": {
                "name": "search_schema",
                "description": "Semantic vector search over the Marketing2026 DDL knowledge base in Supabase. Returns up to 15 complete table definitions matching the keywords. Use this when the <schema> in your system prompt doesn't contain the table you need. CALL ONCE PER QUESTION — repeated calls with similar keywords return the same tables.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keywords": { "type": "string", "description": "Arabic + English keywords describing the entity/concept (e.g. 'منتهية الصلاحية expiry', 'متابعة النواقص ITEMS_SUB MIN_LEVEL', 'متابعة الديون TAKE GIVE CUSTOMERS', 'طلبية شراء ذكية SALE_ITEMS'). Mix Arabic and English for best results." }
                    },
                    "required": ["keywords"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "explore_local_schema",
                "description": "Lists actual column names and data types for a table from MSSQL INFORMATION_SCHEMA. Use this when you have a table name but are unsure about exact column names (especially case-sensitivity). Faster and more reliable than guessing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "table_hint": { "type": "string", "description": "Partial or full table name to filter INFORMATION_SCHEMA.COLUMNS (e.g. 'ITEMS', 'BUY_INVOICE'). Leave empty to list all tables." }
                    },
                    "required": ["table_hint"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_query_patterns",
                "description": "Searches QUERY_PATTERNS.md for SQL templates (up to 2 matches). Use FIRST for: دراسة منتج شاملة, وحدات وأسعار, طلبية شراء ذكية, ديون, نواقص, صلاحية, جرد, رواتب, تصنيع, ربحية, حركة صنف. For multi-step product study prefer plan_complex_query + execute_query_plan.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keywords": { "type": "string", "description": "Arabic or English keywords to search for the pattern (e.g. 'طلبية شراء ذكية', 'debts TAKE GIVE', 'expiry صلاحية', 'جرد JARED', 'رواتب salaries', 'رواتب بعد الخصم')." }
                    },
                    "required": ["keywords"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_current_datetime",
                "description": "Returns the current date, time, day of week, month, and year in Arabic (Libya timezone UTC+2). Call this whenever the user's request involves: today's date, current month/year, time-sensitive filtering, 'الشهر الحالي', 'اليوم', 'الآن', 'هذا الشهر', 'هذه السنة', salary month, attendance period. Also useful before any query that uses GETDATE() or date comparisons.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "execute_raw_sql",
                "description": "Executes a single read-only SELECT query against the local MSSQL database. Returns columns + rows. If result has more than 25 rows, server auto-creates a PDF and returns a 20-row preview plus file_path. Use TOP N (not LIMIT). Read-only enforced.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql_query": { "type": "string", "description": "A T-SQL SELECT statement. Always use TOP N to limit results. Use bracket notation [Name] for reserved words." }
                    },
                    "required": ["sql_query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_pdf",
                "description": "Generates a beautiful Arabic PDF file for a predefined system report by report_id and saves it locally. Use this when the user asks for the PDF version of a predefined report.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string", "description": "The UUID of the report." },
                        "search_term": { "type": "string", "description": "Optional search term / filter." },
                        "target_date": { "type": "string", "description": "Optional target date." }
                    },
                    "required": ["report_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_custom_pdf",
                "description": "Generates and exports a beautifully formatted Arabic PDF for any custom query results or raw tabular data. Supports full Arabic text shaping and visual order. Use this when the user asks to export their current custom SQL query results or custom data as a PDF document.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "A descriptive title for the PDF report (e.g., 'تقرير مبيعات شهر أبريل 2026')." },
                        "columns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "The list of column names (must match the order of data in the rows)."
                        },
                        "rows": {
                            "type": "array",
                            "items": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "description": "The 2D array of rows (each row contains string values representing the columns)."
                        }
                    },
                    "required": ["title", "columns", "rows"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_pdf_report",
                "description": "Creates a local PDF report from a single read-only T-SQL SELECT query and returns a FILE_PATH. Use this ONLY when the user explicitly asks for a PDF, export, print-ready file, or report file. Do not call it for normal questions. The model may freely choose the report title and SQL design, but must use verified table/column names and SQL Server syntax.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Clear Arabic or English title for the PDF report." },
                        "sql_query": { "type": "string", "description": "A single read-only SQL Server SELECT query. Use TOP to keep the report bounded. Never use INSERT, UPDATE, DELETE, DROP, ALTER, EXEC, or PostgreSQL syntax." }
                    },
                    "required": ["title", "sql_query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "send_pdf_to_telegram",
                "description": "Generates a beautiful Arabic PDF file for a predefined system report by report_id and sends it directly to the user's configured Telegram chat. Use this when the desktop user specifically asks to send a report PDF to their Telegram.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string", "description": "The UUID of the report." },
                        "search_term": { "type": "string", "description": "Optional search term / filter." },
                        "target_date": { "type": "string", "description": "Optional target date." }
                    },
                    "required": ["report_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_excel",
                "description": "Generates a local Excel (.xlsx) file for a predefined system report by report_id and returns a FILE_PATH. Use when the user asks for Excel/xlsx export of a saved report.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string", "description": "The UUID of the report." },
                        "search_term": { "type": "string", "description": "Optional search term / filter." },
                        "target_date": { "type": "string", "description": "Optional target date." }
                    },
                    "required": ["report_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "generate_custom_excel",
                "description": "Generates a local Excel (.xlsx) file from custom tabular data and returns a FILE_PATH. Use when the user asks to export current query results as Excel.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Report title for the Excel sheet." },
                        "columns": { "type": "array", "items": { "type": "string" }, "description": "Column headers." },
                        "rows": {
                            "type": "array",
                            "items": { "type": "array", "items": { "type": "string" } },
                            "description": "2D row data matching columns order."
                        }
                    },
                    "required": ["title", "columns", "rows"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_excel_report",
                "description": "Creates a local Excel (.xlsx) report from a single read-only T-SQL SELECT query and returns a FILE_PATH. Use ONLY when the user explicitly asks for Excel, اكسل, xlsx, or spreadsheet export.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Clear title for the Excel report / worksheet." },
                        "sql_query": { "type": "string", "description": "A single read-only SQL Server SELECT query. Use TOP to bound rows." }
                    },
                    "required": ["title", "sql_query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "send_excel_to_telegram",
                "description": "Generates an Excel (.xlsx) file for a predefined system report by report_id and sends it to the user's configured Telegram chat.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "report_id": { "type": "string", "description": "The UUID of the report." },
                        "search_term": { "type": "string", "description": "Optional search term / filter." },
                        "target_date": { "type": "string", "description": "Optional target date." }
                    },
                    "required": ["report_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "schedule_report",
                "description": "Schedules a recurring automated report to run at a set interval and appear in the Notifications section. Use whenever the user says: يومياً، كل ساعة، كل X دقائق، كل X ثواني، جدول تقرير، تنبيه دوري، تقرير تلقائي. The report will be generated automatically and shown as a notification (text/PDF/Excel) each time it fires.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Short Arabic name for this schedule (e.g. 'تقرير المبيعات اليومي')." },
                        "description": { "type": "string", "description": "One-line description of what this report does." },
                        "sql_query": { "type": "string", "description": "A single read-only T-SQL SELECT query to run on schedule." },
                        "report_title": { "type": "string", "description": "Title shown on the generated report." },
                        "report_type": { "type": "string", "description": "Output format: 'text' (default), 'pdf', or 'excel'." },
                        "columns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arabic column names matching the SELECT order. MANDATORY — always translate DB column names to Arabic."
                        },
                        "interval_seconds": { "type": "integer", "description": "Repeat interval in seconds. Examples: 86400=يومي, 3600=ساعي, 300=كل 5 دقائق, 60=دقيقة, 10=كل 10 ثواني." },
                        "first_run_offset_seconds": { "type": "integer", "description": "Seconds from now until first run (default 0)." }
                    },
                    "required": ["name", "sql_query", "report_title", "report_type", "columns", "interval_seconds"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_scheduled_reports",
                "description": "Returns all scheduled recurring reports with their names, intervals, next run time, and active status.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_scheduled_report",
                "description": "Cancels and removes a scheduled report by its ID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "schedule_id": { "type": "string", "description": "The ID of the scheduled report to delete." }
                    },
                    "required": ["schedule_id"]
                }
            }
        }
    ]);
    let tools: Vec<Value> = if !advanced_mode {
        crate::pattern_catalog::executor_tool_definitions()
    } else {
        let mut tools: Vec<Value> = serde_json::from_value(tools_json).unwrap_or_default();
        tools.extend(crate::agent_tools::tool_definitions_for_mode(true));
        tools
    };
    if advanced_mode {
        eprintln!("[Desktop Agent] advanced mode: {} tools", tools.len());
    } else {
        eprintln!(
            "[Desktop Agent] executor mode: {} tools (ERP patterns only)",
            tools.len()
        );
    }
    eprintln!(
        "[Desktop Agent] system prompt chars: {}",
        system_instruction.chars().count()
    );

    // Check if system prompt exists in history
    if chat_history.is_empty() || chat_history[0].get("role").and_then(|r| r.as_str()) != Some("system") {
        chat_history.insert(0, json!({
            "role": "system",
            "content": system_instruction
        }));
    } else {
        chat_history[0] = json!({
            "role": "system",
            "content": system_instruction
        });
    }

    trim_chat_history_vec(&mut chat_history, 9);

    let mut current_history = chat_history.clone();
    current_history.push(json!({
        "role": "user",
        "content": user_text
    }));

    // Per-conversation guards (Desktop)
    let mut recent_sql: Vec<String> = Vec::new();
    let mut sql_fingerprints: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut schema_cache: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut sql_call_count: usize = 0;
    const MAX_SQL_PER_TURN: usize = 5;
    const MAX_SAME_FINGERPRINT: usize = 2;
    const FORCE_FINALIZE_AFTER_SQL: usize = 4;
    let mut schema_explore_count: usize = 0;
    const MAX_SCHEMA_EXPLORE_PER_TURN: usize = 3;
    let mut nudge_count: u8 = 0;
    let mut pattern_call_count: usize = 0;
    let mut pattern_keywords_seen: Vec<String> = Vec::new();
    const MAX_PATTERN_PER_TURN: usize = 5;
    let mut empty_response_retries: u8 = 0;
    let mut pattern_executed = false;
    let mut meta_tool_only = false;
    let mut fast_path_summarize_only = false;

    if !advanced_mode {
        match crate::agent_memory::fetch_agent_tool_recipes(&access_token, erp, user_text, 1).await {
            Ok(recipes) => {
                if let Some(recipe) = recipes.first() {
                    eprintln!(
                        "[agent_memory] recipe candidate id={} tool={} score={:.3}",
                        recipe.id, recipe.tool_name, recipe.score
                    );
                    if recipe.score >= 0.30 && recipe.tool_name == "run_query_pattern" {
                        let requires_product_filter = recipe
                            .slots
                            .get("requires_product_filter")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        if requires_product_filter && active_pf.as_deref().unwrap_or("").trim().is_empty() {
                            eprintln!(
                                "[agent_memory] recipe skipped id={} reason=missing_product_filter",
                                recipe.id
                            );
                        } else {
                            let mut args = recipe.tool_args_template.clone();
                            if requires_product_filter {
                                if let Some(pf) = active_pf.as_deref() {
                                    args["product_filter"] = json!(pf);
                                }
                            }
                            let args_str = args.to_string();
                            let fast_tool_call_id = format!("fast_path_{}", request_id);
                            let pattern_id = args
                                .get("pattern_id")
                                .and_then(|v| v.as_str())
                                .or(recipe.pattern_id.as_deref())
                                .unwrap_or("")
                                .to_string();

                            eprintln!(
                                "[agent_memory] fast recipe hit id={} tool={} pattern={} score={:.3}",
                                recipe.id, recipe.tool_name, pattern_id, recipe.score
                            );

                            current_history.push(json!({
                                "role": "assistant",
                                "content": Value::Null,
                                "tool_calls": [{
                                    "id": fast_tool_call_id,
                                    "type": "function",
                                    "function": {
                                        "name": recipe.tool_name,
                                        "arguments": args_str
                                    }
                                }]
                            }));

                            let tool_content = match crate::agent_tools::dispatch_extended_tool(
                                &recipe.tool_name,
                                &args_str,
                                app_state,
                                crate::agent_tools::ExportDelivery::Local,
                            )
                            .await
                            {
                                Some(value) => value.to_string(),
                                None => "{\"error\":\"fast_path_tool_failed\"}".to_string(),
                            };
                            let success = !tool_content.contains("\"error\"");
                            if success {
                                pattern_executed = true;
                                fast_path_summarize_only = true;
                                pattern_call_count = 1;
                                if !pattern_id.is_empty() {
                                    pattern_keywords_seen.push(pattern_id.clone());
                                }
                            }

                            if let Err(e) = crate::agent_memory::record_agent_tool_result(
                                &access_token,
                                Some(recipe.id),
                                request_id,
                                erp,
                                user_text,
                                &recipe.tool_name,
                                if pattern_id.is_empty() { None } else { Some(pattern_id.as_str()) },
                                success,
                                if success { None } else { Some(tool_content.as_str()) },
                            )
                            .await
                            {
                                eprintln!("[agent_memory] record fast recipe: {}", e);
                            }

                            current_history.push(json!({
                                "role": "tool",
                                "tool_call_id": fast_tool_call_id,
                                "content": compress_tool_result(&tool_content)
                            }));
                            current_history.push(json!({
                                "role": "user",
                                "content": "لخّص نتيجة الأداة السابقة مباشرة بالعربية. لا تستدعِ أي أداة أخرى إلا إذا كانت النتيجة تحتوي خطأ واضحاً."
                            }));
                        }
                    } else {
                        eprintln!(
                            "[agent_memory] recipe skipped tool={} score={:.3}",
                            recipe.tool_name, recipe.score
                        );
                    }
                } else {
                    eprintln!("[agent_memory] no fast recipe matched");
                }
            }
            Err(e) => eprintln!("[agent_memory] fast recipe search: {}", e),
        }
    }

    // 3. Agent Loop (multi-step function calling)
    for iter_num in 0..20 {
        if ai_cancelled(&mut cancel_rx) {
            return Err("تم إيقاف الطلب من المستخدم.".to_string());
        }
        println!("[Desktop Agent] Iteration {} started", iter_num);
        trim_history_for_api(&mut current_history);

        // Force-finalize: بعد عدد كافٍ من الاستعلامات، اسحب الأدوات وأجبر إجابة نصية
        // لمنع الدوامة (إعادة صياغة نفس الاستعلام بلا نهاية).
        let force_finalize = sql_call_count >= FORCE_FINALIZE_AFTER_SQL;
        if force_finalize {
            current_history.push(json!({
                "role": "user",
                "content": "لقد جمعت بيانات كافية من الاستعلامات السابقة. توقف عن تنفيذ أي استعلام جديد، \
                وقدّم الآن الإجابة النهائية بالعربية بناءً على النتائج التي حصلت عليها بالفعل. \
                إن لم تكن البيانات كافية، اشرح للمستخدم ما وجدته وما ينقصه — دون استدعاء أي أداة."
            }));
        }
        let summarize_fast_path = fast_path_summarize_only && iter_num == 0;
        // بعد تنفيذ pattern ناجح، iteration التالية هي تلخيص فقط — لا أدوات
        let summarize_after_pattern = pattern_executed && iter_num >= 1;
        let text_only_mode = force_finalize || summarize_fast_path || summarize_after_pattern;

        // ── Slim Summarize History ──────────────────────────────────────────────
        // في وضع التلخيص بعد pattern، نبني تاريخاً مضغوطاً بدل إرسال السياق كاملاً:
        //   [system] + [user_original] + [tool_result كـ user message]
        // هذا يقطع ~40-50% من توكنز iter 1 (نتخلص من tool_call message + nudge + old history)
        // slim_messages يُطبَّق على fast path أيضاً (أكثر الحالات شيوعاً)
        let slim_messages: Option<serde_json::Value> = if (summarize_after_pattern || summarize_fast_path) && !advanced_mode {
            // استخرج آخر نتيجة tool من التاريخ (role=tool)
            let tool_result_content = current_history.iter().rev()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"))
                .and_then(|m| m.get("content").and_then(|c| c.as_str()))
                .unwrap_or("")
                .to_string();
            // استخرج رسالة المستخدم الأصلية (أول user بعد system)
            let user_original = current_history.iter()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .and_then(|m| m.get("content").and_then(|c| c.as_str()))
                .unwrap_or(user_text)
                .to_string();
            if !tool_result_content.is_empty() {
                let system_msg = current_history.first().cloned().unwrap_or(json!({}));
                Some(json!([
                    system_msg,
                    { "role": "user", "content": user_original },
                    {
                        "role": "user",
                        "content": format!(
                            "نتيجة الاستعلام:\n{}\n\nلخّص النتيجة للمستخدم بالعربية باختصار مع الإجماليات. العملة: د.ل.",
                            tool_result_content
                        )
                    }
                ]))
            } else {
                None
            }
        } else {
            None
        };

        let full_history_json = json!(current_history);
        let messages_to_send = slim_messages.as_ref().unwrap_or(&full_history_json);

        let req_body = if text_only_mode {
            json!({
                "model": DEFAULT_AI_MODEL,
                "messages": messages_to_send,
                "tool_choice": "none"
            })
        } else {
            json!({
                "model": DEFAULT_AI_MODEL,
                "messages": messages_to_send,
                "tools": tools,
                "tool_choice": "auto"
            })
        };

        // في وضع التلخيص (text-only)، نستخدم streaming لعرض النص فوراً
        let api_result = if text_only_mode && !advanced_mode {
            match call_api_streaming(groq_key, &req_body, &app_handle, request_id).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    eprintln!("[Streaming] failed ({}) — fallback to blocking", e);
                    call_groq_api(groq_key, ai_model, &req_body).await
                }
            }
        } else {
            call_groq_api(groq_key, ai_model, &req_body).await
        };

        match api_result {
            Ok(res_json) => {
                if let Some(choices) = res_json.get("choices").and_then(|c| c.as_array()) {
                    if let Some(choice) = choices.get(0) {
                        let empty_json = json!({});
                        let message = choice.get("message").unwrap_or(&empty_json);

                        emit_openrouter_usage(&app_handle, request_id, &res_json);
                        current_history.push(message.clone());

                        if let Some(tool_calls) = message_tool_calls(message) {
                            // Anti-parallel guard for execute_raw_sql
                            let mut filtered_calls: Vec<&Value> = Vec::new();
                            let mut sql_seen = false;
                            for tc in tool_calls {
                                if ai_cancelled(&mut cancel_rx) {
                                    return Err("تم إيقاف الطلب من المستخدم.".to_string());
                                }
                                let tname = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                                if tname == "execute_raw_sql" || tname == "create_pdf_report" || tname == "create_excel_report" {
                                    if sql_seen {
                                        let tid = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                        let resp = json!({
                                            "role": "tool",
                                            "tool_call_id": tid,
                                            "content": "{\"error\": \"تجاوز قاعدة 'استعلام واحد في كل دور'. تم تجاهل هذا الاستدعاء. انتظر نتيجة الاستعلام الأول قبل المحاولة مرة أخرى.\"}"
                                        });
                                        current_history.push(resp);
                                        continue;
                                    }
                                    sql_seen = true;
                                }
                                filtered_calls.push(tc);
                            }

                            for tool_call in filtered_calls {
                                if ai_cancelled(&mut cancel_rx) {
                                    return Err("تم إيقاف الطلب من المستخدم.".to_string());
                                }
                                let func = tool_call.get("function").unwrap_or(&empty_json);
                                let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                let args_str = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                                let tool_call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("");

                                println!("[Desktop Agent] Tool called: {} with args: {}", name, args_str);
                                
                                let friendly_tool_name = match name {
                                    "execute_raw_sql" => "سأستخدم أداة الاستعلام (SQL) للبحث في قاعدة البيانات...",
                                    "search_schema" => "سأبحث في هيكل البيانات للتعرف على الجداول المناسبة...",
                                    "explore_local_schema" => "سأستكشف جداول قاعدة البيانات للبحث عن الأعمدة المطلوبة...",
                                    "search_query_patterns" => "سأبحث في أنماط الاستعلام الجاهزة والمختبرة...",
                                    "get_current_datetime" => "سأتحقق من التاريخ والوقت الحاليين...",
                                    "schedule_report" => "سأقوم بجدولة هذا التقرير ليعمل تلقائياً...",
                                    "list_scheduled_reports" => "سأستعرض قائمة التقارير المجدولة...",
                                    "delete_scheduled_report" => "سأقوم بحذف التقرير المجدول...",
                                    "validate_sql" => "سأقوم بفحص صحة استعلام SQL قبل تنفيذه...",
                                    "explain_sql" => "سأشرح لك ما يفعله هذا الاستعلام...",
                                    "get_table_sample" => "سأخذ عينة سريعة من بيانات الجدول...",
                                    "run_query_pattern" => "سأنفذ نمط الاستعلام الجاهز المخصص...",
                                    "get_product_schema" => "سأراجع هيكل المنتجات لفهم العلاقة بين الفروع والمخازن...",
                                    "get_database_views" => "سأراجع Views وقواعد ربط المبيعات والموظفين...",
                                    "plan_complex_query" => "سأنشئ خطة استعلام مفصلة خطوة بخطوة...",
                                    "execute_query_plan" => "سأنفذ الخطة التي قمت بإنشائها...",
                                    "compare_periods" => "سأقارن بين الفترتين لاستخراج النتائج...",
                                    "suggest_indexes" => "سأقترح فهارس لتحسين أداء الاستعلام...",
                                    "save_favorite_query" => "سأستخدم أداة الحفظ لتخزين الاستعلام لتتمكن من استخدامه لاحقاً...",
                                    "list_favorite_queries" => "سأستعرض الاستعلامات التي قمت بحفظها...",
                                    "export_last_result" => "سأقوم بتصدير آخر نتيجة ظهرت...",
                                    "execute_report" => "سأقوم بتشغيل التقرير لاستخراج البيانات المطلوبة...",
                                    "generate_pdf" | "generate_custom_pdf" | "create_pdf_report" => "سأنشئ ملف PDF ليحتوي على البيانات...",
                                    "generate_excel" | "generate_custom_excel" | "create_excel_report" => "سأنشئ ملف Excel ليحتوي على البيانات...",
                                    "send_pdf_to_telegram" | "send_excel_to_telegram" => "سأرسل الملف مباشرة عبر تليجرام...",
                                    _ => "سأقوم باستخدام أداة ذكية لمساعدتك..."
                                };
                                let _ = app_handle.emit("tool-usage", json!({ "tool": friendly_tool_name }));
                                if name == "execute_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");
                                    
                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data = execute_db_report(rep_id, search_term, target_date, app_state, reports_cache).await;
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "generate_pdf" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");
                                    
                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data = generate_and_save_pdf_local(
                                        rep_id, search_term, target_date, app_state, reports_cache
                                    ).await;
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "generate_custom_pdf" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("");
                                    let empty_arr2 = vec![];
                                    let columns_val = args.get("columns").and_then(|c| c.as_array()).unwrap_or(&empty_arr2);
                                    let rows_val = args.get("rows").and_then(|r| r.as_array()).unwrap_or(&empty_arr2);

                                    let columns: Vec<String> = columns_val.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                    let mut rows: Vec<Vec<String>> = Vec::new();
                                    for r_val in rows_val {
                                        if let Some(arr) = r_val.as_array() {
                                            let r_str: Vec<String> = arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
                                            rows.push(r_str);
                                        }
                                    }

                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data = generate_custom_pdf_local(title, &columns, &rows).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "create_pdf_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("PDF Report");
                                    let sql_query = args.get("sql_query").and_then(|q| q.as_str()).unwrap_or("");

                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data = generate_pdf_report_from_sql_local(
                                        title, sql_query, app_state
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "generate_excel" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");

                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data = generate_and_save_excel_local(
                                        rep_id, search_term, target_date, app_state, reports_cache
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "generate_custom_excel" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("");
                                    let empty_arr2 = vec![];
                                    let columns_val = args.get("columns").and_then(|c| c.as_array()).unwrap_or(&empty_arr2);
                                    let rows_val = args.get("rows").and_then(|r| r.as_array()).unwrap_or(&empty_arr2);

                                    let columns: Vec<String> = columns_val.iter().map(json_value_to_cell).collect();
                                    let mut rows: Vec<Vec<String>> = Vec::new();
                                    for r_val in rows_val {
                                        if let Some(arr) = r_val.as_array() {
                                            let r_str: Vec<String> = arr.iter().map(json_value_to_cell).collect();
                                            rows.push(r_str);
                                        }
                                    }

                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data =
                                        generate_custom_excel_local(title, &columns, &rows, app_state).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "create_excel_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("Excel Report");
                                    let sql_query = args.get("sql_query").and_then(|q| q.as_str()).unwrap_or("");

                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let func_response_data = generate_excel_report_from_sql_local(
                                        title, sql_query, app_state
                                    ).await;

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "send_pdf_to_telegram" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");
                                    
                                    // let _ = app_handle.emit("tool-usage"... removed
                                    
                                    let mut func_response_data = json!({ "error": "لم يتم إعداد تيليجرام. يرجى إضافة توكن البوت ومعرف المحادثة في الإعدادات." });
                                    
                                    if let Ok(store) = app_handle.store("settings.json") {
                                        let token = store.get("telegram_bot_token").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
                                        let chat_id_str = store.get("telegram_chat_id").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
                                        let chat_id = chat_id_str.parse::<i64>().unwrap_or(0);
                                        
                                        if !token.is_empty() && chat_id != 0 {
                                            let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();
                                            func_response_data = generate_and_send_pdf_for_agent(
                                                &client, &token, chat_id, rep_id, search_term, target_date, app_state, reports_cache
                                            ).await;
                                        }
                                    }
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "send_excel_to_telegram" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let rep_id = args.get("report_id").and_then(|id| id.as_str()).unwrap_or("");
                                    let search_term = args.get("search_term").and_then(|t| t.as_str()).unwrap_or("");
                                    let target_date = args.get("target_date").and_then(|t| t.as_str()).unwrap_or("");

                                    // let _ = app_handle.emit("tool-usage"... removed

                                    let mut func_response_data = json!({ "error": "لم يتم إعداد تيليجرام. يرجى إضافة توكن البوت ومعرف المحادثة في الإعدادات." });

                                    if let Ok(store) = app_handle.store("settings.json") {
                                        let token = store.get("telegram_bot_token").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
                                        let chat_id_str = store.get("telegram_chat_id").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
                                        let chat_id = chat_id_str.parse::<i64>().unwrap_or(0);

                                        if !token.is_empty() && chat_id != 0 {
                                            let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();
                                            func_response_data = generate_and_send_excel_for_agent(
                                                &client, &token, chat_id, rep_id, search_term, target_date, app_state, reports_cache
                                            ).await;
                                        }
                                    }

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "search_schema" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let keywords = args.get("keywords").and_then(|k| k.as_str()).unwrap_or("");
                                    
                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let schema_data = search_schema(keywords, openai_key, erp).await;
                                    
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": schema_data
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "execute_raw_sql" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let sql_query = args.get("sql_query").and_then(|q| q.as_str()).unwrap_or("");
                                    let sql_norm = sql_query.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
                                    let fingerprint = sql_semantic_fingerprint(sql_query);

                                    sql_call_count += 1;
                                    let fp_count = sql_fingerprints.entry(fingerprint.clone()).or_insert(0);
                                    *fp_count += 1;
                                    let fp_hits = *fp_count;

                                    let resp_content = if !advanced_mode {
                                        "{\"error\":\"الوضع السريع لا يسمح بـ execute_raw_sql. استخدم run_query_pattern(pattern_id=...) فقط.\"}".to_string()
                                    } else if sql_call_count > MAX_SQL_PER_TURN {
                                        "{\"error\": \"تجاوزت الحد الأقصى للاستعلامات في هذا الدور. توقف عن المحاولة وقدّم إجابة نهائية بالعربية بناءً على النتائج التي حصلت عليها بالفعل، أو اشرح للمستخدم ما ينقصك.\"}".to_string()
                                    } else if recent_sql.iter().any(|s| s == &sql_norm) {
                                        "{\"error\": \"هذا الاستعلام تم تنفيذه للتو بنفس الصيغة. النتيجة لن تتغير. استخدم النتيجة السابقة وقدّم الإجابة النهائية الآن.\"}".to_string()
                                    } else if fp_hits > MAX_SAME_FINGERPRINT {
                                        "{\"error\": \"حاولت الاستعلام عن نفس الجداول/الشروط عدة مرات بصياغات مختلفة دون نتيجة مفيدة. توقف عن إعادة الصياغة وقدّم إجابة نهائية بالعربية بما لديك، أو اطلب توضيحاً محدداً من المستخدم — ولا تكرر هذا الاستعلام.\"}".to_string()
                                    } else {
                                    // let _ = app_handle.emit("tool-usage"... removed
                                        let result = execute_raw_sql_for_agent(
                                            sql_query,
                                            app_state,
                                            &crate::agent_tools::ExportDelivery::Local,
                                        )
                                        .await;
                                        recent_sql.push(sql_norm);
                                        if recent_sql.len() > 3 { recent_sql.remove(0); }
                                        result.to_string()
                                    };

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": resp_content
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "explore_local_schema" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let table_hint = args.get("table_hint").and_then(|h| h.as_str()).unwrap_or("");
                                    let cache_key = table_hint.to_lowercase();

                                    schema_explore_count += 1;
                                    let func_response_data: Value = if schema_explore_count > MAX_SCHEMA_EXPLORE_PER_TURN {
                                        json!({
                                            "error": "تجاوزت 4 استكشافات للمخطط في هذا الدور. استخدم run_query_pattern أو DOMAIN_CRITICAL_FACTS ثم نفّذ استعلاماً واحداً."
                                        })
                                    } else if let Some(cached) = schema_cache.get(&cache_key) {
                                        json!({ "cached": true, "data": cached })
                                    } else {
                                        let result = explore_local_schema_for_agent(table_hint, app_state).await;
                                        let s = result.to_string();
                                        schema_cache.insert(cache_key, s.clone());
                                        result
                                    };

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": func_response_data.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "search_query_patterns" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                    let keywords = args.get("keywords").and_then(|k| k.as_str()).unwrap_or("");
                                    let result = search_query_patterns_local(keywords, erp);
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "list_available_patterns" {
                                    let result =
                                        crate::pattern_catalog::handle_list_available_patterns(erp);
                                    meta_tool_only = true;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result.to_string()
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "get_current_datetime" {
                                    let result = get_current_datetime_info();
                                    meta_tool_only = true;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "schedule_report" {
                                    // let _ = app_handle.emit("tool-usage"... removed
                                    let result = handle_schedule_report_tool(args_str, app_state).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "list_scheduled_reports" {
                                    let result = handle_list_scheduled_reports_tool(app_state).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if name == "delete_scheduled_report" {
                                    let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                                    let schedule_id = args.get("schedule_id").and_then(|v| v.as_str()).unwrap_or("");
                                    let result = handle_delete_scheduled_report_tool(schedule_id, app_state).await;
                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": result
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                } else if crate::agent_tools::is_extended_tool(name) {
                                    let tool_content = if name == "run_query_pattern" {
                                        let args: Value =
                                            serde_json::from_str(args_str).unwrap_or(empty_json.clone());
                                        let pid = args
                                            .get("pattern_id")
                                            .and_then(|k| k.as_str())
                                            .unwrap_or("");
                                        let kw = args
                                            .get("keywords")
                                            .and_then(|k| k.as_str())
                                            .unwrap_or("");
                                        let dedupe_key = if !pid.trim().is_empty() {
                                            pid.trim().to_lowercase()
                                        } else {
                                            kw.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
                                        };
                                        if pattern_call_count >= MAX_PATTERN_PER_TURN {
                                            json!({
                                                "error": format!("تجاوزت {} محاولات لـ run_query_pattern. لخّص آخر نتيجة — لا تقل إن الأنماط «معطّلة».", MAX_PATTERN_PER_TURN)
                                            })
                                            .to_string()
                                        } else if pattern_keywords_seen.iter().any(|k| k == &dedupe_key) {
                                            json!({
                                                "error": "جرّبت هذا النمط للتو. لخّص آخر نتيجة للمستخدم الآن.",
                                                "tried": dedupe_key
                                            })
                                            .to_string()
                                        } else {
                                            pattern_call_count += 1;
                                            pattern_keywords_seen.push(dedupe_key);
                                            if let Some(func_response_data) =
                                                crate::agent_tools::dispatch_extended_tool(
                                                    name,
                                                    args_str,
                                                    app_state,
                                                    crate::agent_tools::ExportDelivery::Local,
                                                )
                                                .await
                                            {
                                                let s = func_response_data.to_string();
                                                if !s.contains("\"error\"") {
                                                    pattern_executed = true;
                                                }
                                                s
                                            } else {
                                                "{\"error\": \"فشل تنفيذ النمط.\"}".to_string()
                                            }
                                        }
                                    } else if let Some(func_response_data) =
                                        crate::agent_tools::dispatch_extended_tool(
                                            name,
                                            args_str,
                                            app_state,
                                            crate::agent_tools::ExportDelivery::Local,
                                        )
                                        .await
                                    {
                                        if name == "export_last_result" {
                                            meta_tool_only = true;
                                        }
                                        func_response_data.to_string()
                                    } else {
                                        "{\"error\": \"فشل تنفيذ الأداة.\"}".to_string()
                                    };

                                    let tool_resp_msg = json!({
                                        "role": "tool",
                                        "tool_call_id": tool_call_id,
                                        "content": compress_tool_result(&tool_content)
                                    });
                                    current_history.push(tool_resp_msg.clone());
                                }
                            }
                        } else {
                            if let Some(content) = message_text_content(message) {
                                if nudge_count < 3
                                    && response_defers_tool_execution(&content)
                                    && iter_num < 6
                                {
                                    nudge_count += 1;
                                    current_history.push(json!({
                                        "role": "user",
                                        "content": "نفّذ الآن — استدعِ run_query_pattern(pattern_id=...) فوراً. ممنوع الرد بنص فقط دون tool call."
                                    }));
                                    continue;
                                }
                                // meta_tool_only = export — أضف المسار دائماً حتى لو لم يذكره النموذج
                                let final_content =
                                    sanitize_response_file_paths_ex(&content, app_state, meta_tool_only).await;
                                if !advanced_mode {
                                    if !pattern_executed
                                        && !meta_tool_only
                                        && (response_claims_query_results(&content)
                                            || nudge_count < 4)
                                    {
                                        if response_claims_query_results(&content) {
                                            nudge_count += 1;
                                            current_history.push(json!({
                                                "role": "user",
                                                "content": "⚠️ ممنوع اختراع أرقام. استدعِ run_query_pattern(pattern_id=...) ثم لخّص نتيجة الأداة فقط."
                                            }));
                                            continue;
                                        }
                                        if nudge_count < 4 {
                                            nudge_count += 1;
                                            current_history.push(json!({
                                                "role": "user",
                                                "content": "استدعِ run_query_pattern(pattern_id=...) أو list_available_patterns — لا ترد نصاً فقط."
                                            }));
                                            continue;
                                        }
                                        return Err(
                                            "لم يُنفَّذ أي استعلام. جرّب list_available_patterns ثم pattern_id مناسب.".to_string(),
                                        );
                                    }
                                    return Ok(final_content);
                                }
                                crate::agent_memory::spawn_persist_turn_memories(
                                    user_text.to_string(),
                                    final_content.clone(),
                                    openai_key.to_string(),
                                    groq_key.to_string(),
                                    crate::agent_memory::build_turn_context_from_history(&current_history),
                                    erp,
                                    access_token.clone(),
                                );
                                return Ok(final_content);
                            }
                            if empty_response_retries < 2 {
                                empty_response_retries += 1;
                                current_history.push(json!({
                                    "role": "user",
                                    "content": "ردّك فارغ. قدّم إجابة نهائية بالعربية الآن بناءً على نتائج run_query_pattern السابقة في هذه المحادثة. \
                                    إن لم ينجح أي نمط، اشرح ذلك واقترح: «أفضل عملاء مبيعات»، «مبيعات آخر يوم موظف»، «متابعة الديون»، «ملخص مالي شهري»."
                                }));
                                continue;
                            }
                            return Err("رد فارغ من الذكاء الاصطناعي — جرّب صياغة السؤال باستخدام أحد الأنماط المدعومة.".to_string());
                        }
                    } else {
                         return Err("لا توجد استجابة صالحة من الذكاء الاصطناعي".to_string());
                    }
                } else {
                    return Err("فشل في قراءة الرد (JSON Error)".to_string());
                }
            }
            Err(e) => {
                return Err(format!("خطأ في الاتصال بـ OpenRouter: {}", e));
            }
        }
    }
    
    Err("عذراً، استغرق تحليل البيانات وقتاً أطول من المتوقع. يرجى المحاولة بسؤال أكثر تحديداً.".to_string())
}

async fn generate_and_save_pdf_local(
    report_id: &str,
    param: &str,
    target_date: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
) -> Value {
    use std::io::Write;
    let report = match reports_cache.iter().find(|r| r.id == report_id) {
        Some(r) => r,
        None => return json!({ "error": "التقرير غير موجود" }),
    };

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات" });
    }
    let conn = conn_opt.unwrap();

    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    let final_sql = crate::erp_adapters::finalize_supabase_report_sql(
        erp,
        &report.sql_query,
        param,
        target_date,
    );

    match crate::execute_sql_query(conn, final_sql).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "لا توجد أي بيانات مسجلة لإصدار التقرير كملف PDF." });
            }

            match crate::pdf_generator::generate_report_pdf(&report.name_ar, &result.columns, &result.rows) {
                Ok(pdf_bytes) => {
                    let filename = format!("{}.pdf", report.name_ar.chars().take(30).collect::<String>().replace(' ', "_"));
                    let path = std::env::temp_dir().join(&filename);
                    if let Ok(mut file) = std::fs::File::create(&path) {
                        let _ = file.write_all(&pdf_bytes);
                        json!({ "result": format!("تم حفظ ملف PDF بنجاح. أخبر المستخدم وأضف: [FILE_PATH:{}]", path.display()) })
                    } else {
                        json!({ "error": "فشل في حفظ ملف PDF على الجهاز." })
                    }
                }
                Err(e) => {
                    json!({ "error": format!("فشل في إنشاء ملف PDF: {}", e) })
                }
            }
        }
        Err(e) => json!({ "error": format!("فشل في تنفيذ الاستعلام: {}", e) })
    }
}

async fn generate_custom_pdf_local(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Value {
    use std::io::Write;
    match crate::pdf_generator::generate_report_pdf(title, columns, rows) {
        Ok(pdf_bytes) => {
            let filename = format!("{}.pdf", title.chars().take(30).collect::<String>().replace(' ', "_"));
            let path = std::env::temp_dir().join(&filename);
            if let Ok(mut file) = std::fs::File::create(&path) {
                let _ = file.write_all(&pdf_bytes);
                json!({ "result": format!("تم حفظ ملف PDF بنجاح. أخبر المستخدم وأضف: [FILE_PATH:{}]", path.display()) })
            } else {
                json!({ "error": "فشل في حفظ ملف PDF على الجهاز." })
            }
        }
        Err(e) => json!({ "error": format!("فشل في إنشاء ملف PDF: {}", e) })
    }
}

async fn generate_pdf_report_from_sql_local(
    title: &str,
    sql_query: &str,
    app_state: &Arc<AppState>,
) -> Value {
    use std::io::Write;

    if let Err(e) = validate_read_only_select_sql(sql_query) {
        return json!({ "error": e });
    }

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "Not connected to the local database." });
    }
    let conn = conn_opt.unwrap();

    match crate::execute_sql_query(conn, crate::agent_tools::prepare_sql_batch(sql_query)).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "The query returned no rows, so no PDF report was created." });
            }

            match crate::pdf_generator::generate_report_pdf(title, &result.columns, &result.rows) {
                Ok(pdf_bytes) => {
                    let filename = format!("{}.pdf", title.chars().take(30).collect::<String>().replace(' ', "_"));
                    let path = std::env::temp_dir().join(&filename);
                    if let Ok(mut file) = std::fs::File::create(&path) {
                        let _ = file.write_all(&pdf_bytes);
                        json!({
                            "result": format!("PDF report was saved successfully. Rows: {}. Tell the user and append: [FILE_PATH:{}]", result.rows.len(), path.display())
                        })
                    } else {
                        json!({ "error": "Failed to save PDF file locally." })
                    }
                }
                Err(e) => json!({ "error": format!("Failed to generate PDF: {}", e) })
            }
        }
        Err(e) => json!({ "error": format!("Failed to execute the PDF report query: {}", e) })
    }
}

async fn generate_and_save_excel_local(
    report_id: &str,
    param: &str,
    target_date: &str,
    app_state: &Arc<AppState>,
    reports_cache: &[SupabaseReport],
) -> Value {
    use std::io::Write;
    let report = match reports_cache.iter().find(|r| r.id == report_id) {
        Some(r) => r,
        None => return json!({ "error": "التقرير غير موجود" }),
    };

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات" });
    }
    let conn = conn_opt.unwrap();

    let erp = crate::erp_profile::current_erp_kind(app_state).await;
    let final_sql = crate::erp_adapters::finalize_supabase_report_sql(
        erp,
        &report.sql_query,
        param,
        target_date,
    );

    match crate::execute_sql_query(conn, final_sql).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "لا توجد أي بيانات مسجلة لإصدار التقرير كملف Excel." });
            }

            match generate_report_excel(&report.name_ar, &result.columns, &result.rows) {
                Ok(xlsx_bytes) => {
                    let filename = crate::agent_tools::safe_export_filename(&report.name_ar, "xlsx");
                    let path = std::env::temp_dir().join(&filename);
                    if let Ok(mut file) = std::fs::File::create(&path) {
                        let _ = file.write_all(&xlsx_bytes);
                        crate::agent_tools::record_exported_file(app_state, &path).await;
                        json!({ "result": format!("تم حفظ ملف Excel بنجاح. أخبر المستخدم وأضف: [FILE_PATH:{}]", path.display()) })
                    } else {
                        json!({ "error": "فشل في حفظ ملف Excel على الجهاز." })
                    }
                }
                Err(e) => json!({ "error": format!("فشل في إنشاء ملف Excel: {}", e) })
            }
        }
        Err(e) => json!({ "error": format!("فشل في تنفيذ الاستعلام: {}", e) })
    }
}

async fn generate_custom_excel_local(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
    app_state: &Arc<AppState>,
) -> Value {
    save_excel_bytes_local(
        app_state,
        title,
        generate_report_excel(title, columns, rows),
    )
    .await
}

fn json_value_to_cell(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => v.to_string(),
    }
}

fn excel_filename_from_title(title: &str) -> String {
    crate::agent_tools::safe_export_filename(title, "xlsx")
}

async fn save_excel_bytes_local(
    app_state: &Arc<AppState>,
    title: &str,
    generated: Result<Vec<u8>, String>,
) -> Value {
    use std::io::Write;
    match generated {
        Ok(xlsx_bytes) => {
            let filename = excel_filename_from_title(title);
            let path = std::env::temp_dir().join(&filename);
            match std::fs::File::create(&path) {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(&xlsx_bytes) {
                        eprintln!("[Excel Report] write failed: {}", e);
                        return json!({ "error": format!("فشل في كتابة ملف Excel: {}", e) });
                    }
                    eprintln!(
                        "[Excel Report] saved {} bytes -> {}",
                        xlsx_bytes.len(),
                        path.display()
                    );
                    crate::agent_tools::record_exported_file(app_state, &path).await;
                    json!({
                        "result": format!(
                            "تم حفظ ملف Excel بنجاح. أخبر المستخدم وأضف: [FILE_PATH:{}]",
                            path.display()
                        )
                    })
                }
                Err(e) => {
                    eprintln!("[Excel Report] create failed: {}", e);
                    json!({ "error": format!("فشل في إنشاء ملف Excel على الجهاز: {}", e) })
                }
            }
        }
        Err(e) => {
            eprintln!("[Excel Report] generate failed: {}", e);
            json!({ "error": format!("فشل في إنشاء ملف Excel: {}", e) })
        }
    }
}

async fn generate_excel_report_from_sql_local(
    title: &str,
    sql_query: &str,
    app_state: &Arc<AppState>,
) -> Value {
    if sql_query.trim().is_empty() {
        let stored = {
            let session = app_state.agent_session.lock().await;
            session.last_result.clone()
        };
        if let Some(data) = stored {
            if !data.rows.is_empty() {
                eprintln!(
                    "[Excel Report] empty sql_query — using last_result ({} rows)",
                    data.rows.len()
                );
                return save_excel_bytes_local(
                    app_state,
                    title,
                    generate_report_excel(title, &data.columns, &data.rows),
                )
                .await;
            }
        }
        return json!({ "error": "لم يُحدَّد استعلام SQL ولا توجد نتيجة سابقة للتصدير." });
    }

    if let Err(e) = validate_read_only_select_sql(sql_query) {
        eprintln!("[Excel Report] validation failed: {}", e);
        return json!({
            "error": e,
            "hint": "استخدم SELECT/WITH فقط، أو generate_custom_excel / export_last_result إذا البيانات جاهزة في الذاكرة."
        });
    }

    let conn_opt = app_state.conn.lock().await.clone();
    if conn_opt.is_none() {
        return json!({ "error": "غير متصل بقاعدة البيانات المحلية." });
    }
    let conn = conn_opt.unwrap();
    let sql_batch = crate::agent_tools::prepare_sql_batch(sql_query);

    match crate::execute_sql_query(conn, sql_batch).await {
        Ok(result) => {
            if result.rows.is_empty() {
                return json!({ "result": "الاستعلام لم يُرجع صفوفاً — لم يُنشأ ملف Excel." });
            }
            eprintln!(
                "[Excel Report] query ok: {} rows, {} cols",
                result.rows.len(),
                result.columns.len()
            );
            crate::agent_tools::set_last_result(
                app_state,
                sql_query,
                &result.columns,
                &result.rows,
            )
            .await;
            save_excel_bytes_local(
                app_state,
                title,
                generate_report_excel(title, &result.columns, &result.rows),
            )
            .await
        }
        Err(e) => {
            eprintln!("[Excel Report] SQL failed: {}", e);
            json!({
                "error": format!("فشل تنفيذ استعلام Excel: {}", e),
                "hint": "جرّب أسماء أعمدة بين [أقواس] بدلاً من \"علامات تنصيص\"، أو export_last_result(format=\"excel\")."
            })
        }
    }
}

// ─── أدوات الجدولة ────────────────────────────────────────────────────────────

async fn handle_schedule_report_tool(args_str: &str, app_state: &Arc<AppState>) -> String {
    use crate::scheduler::{ScheduledReport, new_id};
    use std::time::{SystemTime, UNIX_EPOCH};

    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("{{\"error\": \"تعذّر تحليل المعاملات: {}\"}}", e),
    };

    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("تقرير مجدوَل").to_string();
    let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let sql_query = args.get("sql_query").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let report_title = args.get("report_title").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
    let report_type = args.get("report_type").and_then(|v| v.as_str()).unwrap_or("text").to_string();
    let columns: Vec<String> = args.get("columns").and_then(|v| v.as_array()).map(|arr| {
        arr.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect()
    }).unwrap_or_default();
    let interval_seconds = args.get("interval_seconds").and_then(|v| v.as_u64()).unwrap_or(3600);
    let offset = args.get("first_run_offset_seconds").and_then(|v| v.as_u64()).unwrap_or(0);

    if sql_query.is_empty() {
        return "{\"error\": \"يجب تحديد sql_query للجدول.\"}".to_string();
    }

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let interval_desc = crate::scheduler::describe_interval(interval_seconds);
    let id = new_id();
    let msg = format!(
        "تم جدولة التقرير «{}» بنجاح. سيعمل {} وسيظهر في قسم الإشعارات.",
        name, interval_desc
    );

    let report = ScheduledReport {
        id: id.clone(),
        name: name.clone(),
        description,
        sql_query,
        report_title,
        report_type,
        columns,
        interval_seconds,
        next_run_unix: now_secs + offset,
        last_run_unix: None,
        created_at_unix: now_secs,
        is_active: true,
    };

    let mut state = app_state.scheduler.lock().await;
    state.schedules.push(report);

    serde_json::json!({
        "success": true,
        "id": id,
        "message": msg
    }).to_string()
}

async fn handle_list_scheduled_reports_tool(app_state: &Arc<AppState>) -> String {
    let state = app_state.scheduler.lock().await;
    if state.schedules.is_empty() {
        return "{\"schedules\": [], \"message\": \"لا توجد تقارير مجدوَلة حالياً.\"}".to_string();
    }

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let list: Vec<serde_json::Value> = state.schedules.iter().map(|s| {
        let secs_until = if s.next_run_unix > now { s.next_run_unix - now } else { 0 };
        serde_json::json!({
            "id": s.id,
            "name": s.name,
            "interval": crate::scheduler::describe_interval(s.interval_seconds),
            "report_type": s.report_type,
            "is_active": s.is_active,
            "next_run_in_seconds": secs_until,
            "last_run_unix": s.last_run_unix
        })
    }).collect();

    let count = list.len();
    serde_json::json!({
        "schedules": list,
        "count": count
    }).to_string()
}

async fn handle_delete_scheduled_report_tool(schedule_id: &str, app_state: &Arc<AppState>) -> String {
    let mut state = app_state.scheduler.lock().await;
    let before = state.schedules.len();
    state.schedules.retain(|s| s.id != schedule_id);
    let after = state.schedules.len();

    if before == after {
        format!("{{\"error\": \"لم يُعثر على جدول بالمعرّف: {}\"}}", schedule_id)
    } else {
        "{\"success\": true, \"message\": \"تم حذف الجدول بنجاح.\"}".to_string()
    }
}

#[tauri::command]
pub async fn generate_ai_suggestions(
    groq_key: String,
    ai_model: String,
) -> Result<Vec<String>, String> {
    if groq_key.is_empty() {
        return Ok(vec![
            "اكتب لي استعلام SQL لمعرفة المبيعات".to_string(),
            "ما هي المنتجات التي تقارب على الانتهاء؟".to_string(),
            "أريد تقريراً للديون المستحقة".to_string(),
            "كيف أقارن بين أسعار الموردين؟".to_string(),
        ]);
    }

    let sys_prompt = "أنت وكيل ذكي للبيانات. اقترح 4 أسئلة عملية ومختلفة (مثل المبيعات، النواقص، الأرباح، الديون) يمكن للمستخدم أن يطرحها عليك بخصوص نظام المبيعات والمخازن. أعد الإجابة حصراً بصيغة مصفوفة JSON تحتوي على 4 نصوص (بدون أي علامات توضيحية أخرى).";
    
    let current_history = vec![
        serde_json::json!({
            "role": "system",
            "content": sys_prompt
        })
    ];

    let req_body = serde_json::json!({
        "model": DEFAULT_AI_MODEL,
        "messages": current_history,
        "temperature": 0.7
    });

    match call_groq_api(&groq_key, &ai_model, &req_body).await {
        Ok(res_json) => {
            if let Some(choices) = res_json.get("choices").and_then(|c| c.as_array()) {
                if let Some(choice) = choices.get(0) {
                    if let Some(content) = choice.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
                        let cleaned = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
                        if let Ok(suggestions) = serde_json::from_str::<Vec<String>>(cleaned) {
                            if !suggestions.is_empty() {
                                return Ok(suggestions.into_iter().take(4).collect());
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("[Desktop Agent] Failed to generate suggestions: {}", e);
        }
    }

    // Fallback
    Ok(vec![
        "اكتب لي استعلام SQL لمعرفة المبيعات".to_string(),
        "ما هي المنتجات التي تقارب على الانتهاء؟".to_string(),
        "أريد تقريراً للديون المستحقة".to_string(),
        "كيف أقارن بين أسعار الموردين؟".to_string(),
    ])
}
