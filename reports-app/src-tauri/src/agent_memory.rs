//! ذاكرة الوكيل — محلي (SQLite) + مشترك (Supabase db_facts)
//! المشترك: أعمدة، جداول، علاقات، وتسميات schema فقط — لا بيانات تشغيلية متغيرة.

use crate::erp_profile::ErpKind;
use crate::supabase_config::{SUPABASE_ANON_KEY, SUPABASE_URL};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const EMBEDDING_DIM: usize = 1536;
const LOCAL_MATCH_K: usize = 3;
const SHARED_MATCH_K: i32 = 4;
const SHARED_DEDUP_SIMILARITY: f32 = 0.82;
const LOCAL_DEDUP_SIMILARITY: f32 = 0.88;
const RECALL_MIN_SIMILARITY: f32 = 0.38;

const SCHEMA_TOOLS: &[&str] = &[
    "search_schema",
    "explore_local_schema",
    "get_table_sample",
    "get_database_views",
    "get_product_schema",
    "execute_raw_sql",
    "run_query_pattern",
    "validate_sql",
    "explain_sql",
];

#[derive(Debug, Clone, Default)]
pub struct TurnMemoryContext {
    pub schema_tools_used: bool,
    pub snippets: Vec<String>,
}

impl TurnMemoryContext {
    pub fn note_tool(&mut self, tool_name: &str, response: &str) {
        if !SCHEMA_TOOLS.contains(&tool_name) {
            return;
        }
        self.schema_tools_used = true;
        let snippet = response.chars().take(800).collect::<String>();
        if !snippet.trim().is_empty() {
            self.snippets.push(format!("[{tool_name}] {snippet}"));
        }
    }
}

/// يستخرج مخرجات أدوات الـ schema من تاريخ المحادثة الحالي.
pub fn build_turn_context_from_history(history: &[Value]) -> TurnMemoryContext {
    let mut ctx = TurnMemoryContext::default();
    for (i, msg) in history.iter().enumerate() {
        if msg.get("role").and_then(|r| r.as_str()) != Some("assistant") {
            continue;
        }
        let Some(tool_calls) = msg.get("tool_calls").and_then(|t| t.as_array()) else {
            continue;
        };
        for tc in tool_calls {
            let name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            if !SCHEMA_TOOLS.contains(&name) {
                continue;
            }
            let Some(tid) = tc.get("id").and_then(|x| x.as_str()) else {
                continue;
            };
            for follow in history.iter().skip(i + 1) {
                if follow.get("role").and_then(|r| r.as_str()) != Some("tool") {
                    continue;
                }
                if follow.get("tool_call_id").and_then(|x| x.as_str()) != Some(tid) {
                    continue;
                }
                if let Some(c) = follow.get("content").and_then(|x| x.as_str()) {
                    ctx.note_tool(name, c);
                }
                break;
            }
        }
    }
    ctx
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractedFact {
    scope: String,
    kind: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct SharedFactRow {
    content: Option<String>,
    category: Option<String>,
    similarity: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentToolRecipe {
    pub id: i64,
    pub pattern_id: Option<String>,
    pub tool_name: String,
    pub tool_args_template: Value,
    pub slots: Value,
    pub score: f64,
}

fn content_fingerprint(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap()
}

pub async fn create_embedding(text: &str, openai_key: &str) -> Result<Vec<f32>, String> {
    let key = openai_key.trim();
    if key.is_empty() {
        return Err("missing_openai_key".to_string());
    }
    let client = http_client();
    let req = json!({
        "model": "text-embedding-3-small",
        "input": text.chars().take(8000).collect::<String>(),
    });
    let res = client
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", key))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("embedding request failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("embedding HTTP error: {}", body));
    }
    let parsed: OpenAiEmbeddingResponse = res
        .json()
        .await
        .map_err(|e| format!("embedding parse error: {}", e))?;
    let embedding = parsed
        .data
        .first()
        .map(|d| d.embedding.clone())
        .ok_or_else(|| "empty embedding response".to_string())?;
    if embedding.len() != EMBEDDING_DIM {
        return Err(format!(
            "unexpected embedding dim {} (expected {})",
            embedding.len(),
            EMBEDDING_DIM
        ));
    }
    Ok(embedding)
}

/// هل النص يصف schema ثابت (جدول/عمود/علاقة) وليس بيانات تشغيل متغيرة؟
fn is_valid_shared_schema_fact(content: &str) -> bool {
    let lower = content.to_lowercase();

    const BLOCK: &[&str] = &[
        "sitteings",
        "sitteings",
        "نشاط تجاري",
        "business activity",
        "company name",
        "phone",
        "mobile",
        "fax",
        "address",
        "المستخدم",
        "user requested",
        "فارغ",
        "empty",
        "missing",
        "حاليا",
        "currently",
        "إعدادات النظام",
        "system settings",
        "administrator",
        "المسؤول",
        "thank you",
        "credit limit",
        "tax1",
        "tax2",
        "تواصل مع",
        "contact admin",
        "update settings",
        "تحديث بيانات",
        "طلب معلومات",
        "requested information",
        "d.ل",
        "amount",
        "رصيد",
        "balance of customer",
        "عدد الصفوف",
        "row count",
    ];
    if BLOCK.iter().any(|b| lower.contains(b)) {
        return false;
    }

    const SCHEMA_MARKERS: &[&str] = &[
        "dbo.",
        "table",
        "column",
        "جدول",
        "عمود",
        "join",
        "foreign",
        "references",
        "→",
        "يرتبط",
        "relation",
        "pk",
        "fk",
        "primary key",
        "information_schema",
        "_id",
        "_date",
        "sale_items",
        "sale_invoice",
        "buy_invoice",
        "buy_items",
        "items_sub",
        "customers",
        "stores",
    ];
    if !SCHEMA_MARKERS.iter().any(|m| lower.contains(m)) {
        return false;
    }

    // يجب ذكر اسم جدول/عمود بصيغة تقنية
    let has_identifier = content
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|tok| {
            let t = tok.trim();
            (t.len() >= 4 && t.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
                || t.starts_with("dbo.")
        });
    has_identifier
}

fn is_valid_local_preference(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("pref")
        || lower.contains("يفضل")
        || lower.contains("prefer")
        || lower.contains("دائما")
        || lower.contains("always use")
        || lower.contains("نافذة")
        || lower.contains("days_recent")
        || lower.contains("يوم")
}

async fn search_user_memories(
    access_token: &str,
    embedding: &[f32],
    limit: i32,
) -> Result<Vec<(String, String, f32)>, String> {
    let client = http_client();
    let rpc = json!({
        "p_access_token": access_token.trim(),
        "query_embedding": embedding,
        "match_threshold": RECALL_MIN_SIMILARITY,
        "match_count": limit + 2,
    });
    let url = format!("{}/rest/v1/rpc/match_user_memories", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&rpc)
        .send()
        .await
        .map_err(|e| format!("match_user_memories failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        if body.contains("match_user_memories") || body.contains("does not exist") {
            return Ok(Vec::new());
        }
        return Err(format!("match_user_memories HTTP: {}", body));
    }
    let rows: Vec<SharedFactRow> = res.json().await.unwrap_or_default();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let content = r.content?;
            let cat = r.category.unwrap_or_else(|| "preference".to_string());
            let sim = r.similarity.unwrap_or(0.0);
            Some((cat, content, sim))
        })
        .collect())
}

pub async fn fetch_agent_tool_recipes(
    access_token: &str,
    erp: ErpKind,
    user_message: &str,
    limit: i32,
) -> Result<Vec<AgentToolRecipe>, String> {
    if access_token.trim().is_empty() || user_message.trim().is_empty() {
        return Ok(Vec::new());
    }
    let client = http_client();
    let rpc = json!({
        "p_token": access_token.trim(),
        "p_erp_kind": erp.display_name_ar(),
        "p_user_message": user_message,
        "p_limit": limit.max(1),
    });
    let url = format!("{}/rest/v1/rpc/get_agent_tool_recipes", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&rpc)
        .send()
        .await
        .map_err(|e| format!("get_agent_tool_recipes failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        if body.contains("get_agent_tool_recipes") || body.contains("does not exist") {
            return Ok(Vec::new());
        }
        return Err(format!("get_agent_tool_recipes HTTP: {}", body));
    }
    Ok(res.json::<Vec<AgentToolRecipe>>().await.unwrap_or_default())
}

pub async fn record_agent_tool_result(
    access_token: &str,
    recipe_id: Option<i64>,
    request_id: &str,
    erp: ErpKind,
    user_message: &str,
    tool_name: &str,
    pattern_id: Option<&str>,
    success: bool,
    error_message: Option<&str>,
) -> Result<(), String> {
    if access_token.trim().is_empty() {
        return Ok(());
    }
    let client = http_client();
    let rpc = json!({
        "p_token": access_token.trim(),
        "p_recipe_id": recipe_id,
        "p_request_id": request_id,
        "p_erp_kind": erp.display_name_ar(),
        "p_message": user_message,
        "p_tool_name": tool_name,
        "p_pattern_id": pattern_id,
        "p_success": success,
        "p_prompt_tokens": Value::Null,
        "p_completion_tokens": Value::Null,
        "p_error_message": error_message,
    });
    let url = format!("{}/rest/v1/rpc/record_agent_tool_result", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&rpc)
        .send()
        .await
        .map_err(|e| format!("record_agent_tool_result failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        if body.contains("record_agent_tool_result") || body.contains("does not exist") {
            return Ok(());
        }
        return Err(format!("record_agent_tool_result HTTP: {}", body));
    }
    Ok(())
}

async fn is_near_duplicate_user(access_token: &str, embedding: &[f32]) -> Result<bool, String> {
    let hits = search_user_memories(access_token, embedding, 3).await?;
    Ok(hits
        .iter()
        .any(|(_, _, sim)| *sim >= LOCAL_DEDUP_SIMILARITY))
}

async fn store_user_memory(
    access_token: &str,
    category: &str,
    content: &str,
    embedding: &[f32],
) -> Result<bool, String> {
    if is_near_duplicate_user(access_token, embedding).await? {
        return Ok(false);
    }
    let fingerprint = content_fingerprint(content);
    let client = http_client();
    let rpc = json!({
        "p_access_token": access_token.trim(),
        "p_content": content,
        "p_category": category,
        "p_fingerprint": fingerprint,
        "p_embedding": embedding,
    });
    let url = format!("{}/rest/v1/rpc/upsert_user_memory", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&rpc)
        .send()
        .await
        .map_err(|e| format!("upsert_user_memory failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        if body.contains("upsert_user_memory") || body.contains("does not exist") {
            return Ok(false);
        }
        return Err(format!("upsert_user_memory HTTP: {}", body));
    }
    Ok(true)
}

fn dedupe_similar_contents(items: Vec<(String, String)>, limit: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (kind, content) in items {
        let lower = content.to_lowercase();
        if out.iter().any(|(_, existing)| {
            let el = existing.to_lowercase();
            lower == el
                || (lower.len() > 20 && el.contains(&lower[..lower.len().min(40)]))
                || (el.len() > 20 && lower.contains(&el[..el.len().min(40)]))
        }) {
            continue;
        }
        out.push((kind, content));
        if out.len() >= limit {
            break;
        }
    }
    out
}


async fn search_shared_db_facts(
    embedding: &[f32],
    limit: i32,
) -> Result<Vec<(String, String, f32)>, String> {
    let client = http_client();
    let rpc = json!({
        "query_embedding": embedding,
        "match_threshold": RECALL_MIN_SIMILARITY,
        "match_count": limit + 2,
    });
    let url = format!("{}/rest/v1/rpc/match_db_facts", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&rpc)
        .send()
        .await
        .map_err(|e| format!("match_db_facts failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        if body.contains("match_db_facts") || body.contains("does not exist") {
            return Ok(Vec::new());
        }
        return Err(format!("match_db_facts HTTP: {}", body));
    }
    let rows: Vec<SharedFactRow> = res.json().await.unwrap_or_default();
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let content = r.content?;
            let cat = r.category.unwrap_or_else(|| "db_schema".to_string());
            let sim = r.similarity.unwrap_or(0.0);
            Some((cat, content, sim))
        })
        .collect())
}

async fn is_near_duplicate_shared(embedding: &[f32]) -> Result<bool, String> {
    let hits = search_shared_db_facts(embedding, 3).await?;
    Ok(hits
        .iter()
        .any(|(_, _, sim)| *sim >= SHARED_DEDUP_SIMILARITY))
}

async fn upsert_shared_db_fact(
    category: &str,
    content: &str,
    embedding: &[f32],
) -> Result<bool, String> {
    if is_near_duplicate_shared(embedding).await? {
        return Ok(false);
    }
    let client = http_client();
    let fingerprint = content_fingerprint(content);
    let rpc = json!({
        "p_content": content,
        "p_category": category,
        "p_fingerprint": fingerprint,
        "p_embedding": embedding,
    });
    let url = format!("{}/rest/v1/rpc/upsert_db_fact", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&rpc)
        .send()
        .await
        .map_err(|e| format!("upsert_db_fact failed: {}", e))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        if body.contains("upsert_db_fact") || body.contains("does not exist") {
            return Ok(false);
        }
        return Err(format!("upsert_db_fact HTTP: {}", body));
    }
    Ok(true)
}

fn erp_fact_prefix(erp: ErpKind) -> &'static str {
    match erp {
        ErpKind::InfinityRetailDb => "[erp:infinity_retail_db] ",
        ErpKind::Marketing2026 | ErpKind::Unknown => "[erp:marketing2026] ",
    }
}

fn shared_fact_matches_erp(content: &str, erp: ErpKind) -> bool {
    if content.contains("[erp:infinity_retail_db]") {
        return erp == ErpKind::InfinityRetailDb;
    }
    if content.contains("[erp:marketing2026]") {
        return erp != ErpKind::InfinityRetailDb;
    }
    // Legacy facts without tag — show for Marketing only to avoid dbo.ITEMS hints on Infinity
    erp != ErpKind::InfinityRetailDb
}

pub async fn recall_memory_prompt_block(
    user_query: &str,
    openai_key: &str,
    erp: ErpKind,
    access_token: &str,
) -> String {
    if openai_key.trim().is_empty() {
        return String::new();
    }
    let embedding = match create_embedding(user_query, openai_key).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[agent_memory] recall embedding: {}", e);
            return String::new();
        }
    };

    let local_raw = match search_user_memories(access_token, &embedding, LOCAL_MATCH_K as i32).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[agent_memory] user search: {}", e);
            Vec::new()
        }
    };
    let local: Vec<(String, String)> = dedupe_similar_contents(
        local_raw
            .into_iter()
            .map(|(c, t, _)| (c, t))
            .collect(),
        LOCAL_MATCH_K,
    );

    let shared_raw = match search_shared_db_facts(&embedding, SHARED_MATCH_K).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[agent_memory] shared search: {}", e);
            Vec::new()
        }
    };
    let shared: Vec<(String, String)> = dedupe_similar_contents(
        shared_raw
            .into_iter()
            .filter(|(_, content, _)| shared_fact_matches_erp(content, erp))
            .map(|(c, t, _)| (c, t))
            .collect(),
        SHARED_MATCH_K as usize,
    );

    if local.is_empty() && shared.is_empty() {
        return String::new();
    }

    let mut block = String::from("\n\n<AGENT_MEMORY>\n");
    block.push_str(
        "Schema hints only — verify with tools before relying on them. \
         Row counts and business values differ per installation.\n\n",
    );

    if !shared.is_empty() {
        block.push_str("### Shared schema facts (columns / tables / joins)\n");
        for (cat, content) in &shared {
            block.push_str(&format!("- [{}] {}\n", cat, content));
        }
        block.push('\n');
    }

    if !local.is_empty() {
        block.push_str("### Private preferences (this device / cloud)\n");
        for (kind, content) in &local {
            block.push_str(&format!("- [{}] {}\n", kind, content));
        }
    }

    block.push_str("</AGENT_MEMORY>\n");
    block
}

async fn extract_facts_with_llm(
    user_text: &str,
    assistant_text: &str,
    ctx: &TurnMemoryContext,
    groq_key: &str,
    erp: ErpKind,
) -> Vec<ExtractedFact> {
    let client = http_client();
    let snippets = if ctx.snippets.is_empty() {
        "(none)".to_string()
    } else {
        ctx.snippets.join("\n")
    };
    let erp_name = erp.display_name_ar();
    let prompt = format!(
        r#"You extract ONLY stable SQL Server schema facts for {erp_name} ERP.
Return ONLY a JSON array (no markdown). Max 2 items. Empty [] if none.

Allowed shared facts (scope=shared):
- Table/column names and meanings for {erp_name} ONLY
- Which table holds which kind of data (master vs transactional)
- JOIN keys between tables
- Column that stores dates, batch, expiry, qty, price
- Empty/unused tables verified from schema tools (not from missing business data)

FORBIDDEN in shared (never save):
- Company name, phone, address, user settings
- Row counts, balances, amounts, "empty currently", "user asked"
- Marketing2026 table names when ERP is InfinityRetailDB and vice versa

Local only (scope=local, kind=preference): explicit user reporting preferences (e.g. prefers 60-day window).

Format: {{"scope":"shared"|"local","kind":"db_schema"|"db_join"|"db_column"|"preference","content":"..."}}
content: one sentence, mention schema.table.column, under 180 chars.

Schema tool output this turn:
{snippets}

User: {user}
Assistant: {assistant}"#,
        erp_name = erp_name,
        snippets = snippets.chars().take(2500).collect::<String>(),
        user = user_text.chars().take(800).collect::<String>(),
        assistant = assistant_text.chars().take(1200).collect::<String>(),
    );

    let body = json!({
        "model": crate::ai_agent::DEFAULT_AI_MODEL,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": 400,
        "temperature": 0.0,
    });

    let res = match client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", groq_key.trim()))
        .header("HTTP-Referer", "http://localhost:1420")
        .header("X-Title", "Reports App Memory")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[agent_memory] extract LLM: {}", e);
            return Vec::new();
        }
    };

    if !res.status().is_success() {
        eprintln!(
            "[agent_memory] extract LLM HTTP: {}",
            res.text().await.unwrap_or_default()
        );
        return Vec::new();
    }

    let parsed: Value = res.json().await.unwrap_or(json!({}));
    let content = parsed
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("[]");
    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Vec<ExtractedFact>>(cleaned).unwrap_or_default()
}

pub async fn persist_turn_memories(
    user_text: &str,
    assistant_text: &str,
    openai_key: &str,
    groq_key: &str,
    ctx: TurnMemoryContext,
    erp: ErpKind,
    access_token: &str,
) {
    if groq_key.trim().is_empty() || openai_key.trim().is_empty() {
        return;
    }
    if assistant_text.trim().len() < 20 {
        return;
    }
    if !ctx.schema_tools_used {
        eprintln!("[agent_memory] skip persist: no schema tools used this turn");
        return;
    }

    let facts = extract_facts_with_llm(user_text, assistant_text, &ctx, groq_key, erp).await;
    if facts.is_empty() {
        return;
    }

    for fact in facts {
        let content = fact.content.trim();
        if content.len() < 12 {
            continue;
        }
        let scope = fact.scope.to_lowercase();
        let kind = fact.kind.to_lowercase();
        let tagged_content = if scope == "shared" && !content.starts_with("[erp:") {
            format!("{}{}", erp_fact_prefix(erp), content)
        } else {
            content.to_string()
        };

        let embedding = match create_embedding(&tagged_content, openai_key).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[agent_memory] store embedding: {}", e);
                continue;
            }
        };

        if scope == "shared" {
            if !is_valid_shared_schema_fact(&tagged_content) {
                eprintln!("[agent_memory] rejected shared fact: {}", tagged_content);
                continue;
            }
            let category = match kind.as_str() {
                "db_join" => "db_join",
                "db_column" => "db_column",
                "db_schema" => "db_schema",
                _ => "db_schema",
            };
            match upsert_shared_db_fact(category, &tagged_content, &embedding).await {
                Ok(true) => eprintln!("[agent_memory] shared schema stored: {}", tagged_content),
                Ok(false) => eprintln!("[agent_memory] shared duplicate skipped: {}", tagged_content),
                Err(e) => eprintln!("[agent_memory] shared store: {}", e),
            }
        } else if scope == "local" && is_valid_local_preference(&tagged_content) {
            match store_user_memory(access_token, "preference", &tagged_content, &embedding).await {
                Ok(true) => eprintln!("[agent_memory] local preference stored on Supabase: {}", tagged_content),
                Ok(false) => {}
                Err(e) => eprintln!("[agent_memory] local store on Supabase: {}", e),
            }
        }
    }
}

pub fn spawn_persist_turn_memories(
    user_text: String,
    assistant_text: String,
    openai_key: String,
    groq_key: String,
    ctx: TurnMemoryContext,
    erp: ErpKind,
    access_token: String,
) {
    tokio::spawn(async move {
        persist_turn_memories(&user_text, &assistant_text, &openai_key, &groq_key, ctx, erp, &access_token).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_schema_fact() {
        assert!(is_valid_shared_schema_fact(
            "SALE_ITEMS has no S_DATE; join SALE_INVOICE on S_ID for invoice date."
        ));
    }

    #[test]
    fn rejects_business_settings() {
        assert!(!is_valid_shared_schema_fact(
            "Phone and mobile for business activity may be empty in SITTEINGS."
        ));
    }

    #[test]
    fn rejects_user_context() {
        assert!(!is_valid_shared_schema_fact(
            "The user requested their company address from settings."
        ));
    }
}
