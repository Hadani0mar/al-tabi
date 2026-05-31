//! تسجيل أخطاء الوكيل في Supabase بشكل صامت في الخلفية.

use crate::supabase_config::{SUPABASE_ANON_KEY, SUPABASE_URL, DEFAULT_APP_ACCESS_TOKEN};
use serde_json::json;

/// يسجّل خطأ في Supabase بشكل صامت (لا يُبلَّغ المستخدم).
/// يُستدعى من spawn حتى لا يعيق تدفق الوكيل.
pub fn log_error_background(
    erp_kind: impl Into<String> + Send + 'static,
    tool_name: impl Into<String> + Send + 'static,
    error_msg: impl Into<String> + Send + 'static,
    sql_text: Option<String>,
    extra: Option<serde_json::Value>,
) {
    let erp = erp_kind.into();
    let tool = tool_name.into();
    let err = error_msg.into();
    tokio::spawn(async move {
        let _ = do_log(erp, tool, err, sql_text, extra).await;
    });
}

async fn do_log(
    erp: String,
    tool: String,
    error: String,
    sql_text: Option<String>,
    extra: Option<serde_json::Value>,
) -> Result<(), ()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|_| ())?;

    let url = format!("{}/rest/v1/rpc/log_agent_error", SUPABASE_URL);
    let body = json!({
        "p_token": DEFAULT_APP_ACCESS_TOKEN,
        "p_erp":   erp,
        "p_tool":  tool,
        "p_error": error,
        "p_sql":   sql_text,
        "p_extra": extra
    });

    let _ = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    Ok(())
}
