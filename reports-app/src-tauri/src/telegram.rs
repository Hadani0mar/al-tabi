use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};
use crate::supabase_config::{self, SUPABASE_ANON_KEY, SUPABASE_URL};
use crate::AppState;

#[derive(Deserialize, Clone, Debug)]
pub struct SupabaseReport {
    pub id: String,
    pub name_ar: String,
    pub sql_query: String,
    pub has_parameters: bool,
}

#[derive(Deserialize)]
struct TelegramUpdate {
    update_id: u64,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    text: Option<String>,
    chat: Chat,
}

#[derive(Deserialize)]
struct Chat {
    id: i64,
}

#[derive(Deserialize)]
struct TelegramResponse {
    ok: bool,
    result: Option<Vec<TelegramUpdate>>,
}

// ─── Fetch Reports from Supabase ─────────────────────────────────
pub async fn fetch_reports() -> Vec<SupabaseReport> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap();
    let url = supabase_config::supabase_rest_url("reports?is_active=eq.true&select=id,name_ar,sql_query,has_parameters");
    let api_key = SUPABASE_ANON_KEY;

    match client.get(url)
        .header("apikey", api_key)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        Ok(res) => {
            if let Ok(reports) = res.json::<Vec<SupabaseReport>>().await {
                return reports;
            }
        }
        Err(e) => println!("Error fetching reports: {}", e),
    }
    vec![]
}

#[derive(serde::Deserialize)]
struct DocumentRecord {
    content: Option<String>,
}

#[derive(serde::Serialize)]
struct OpenAiEmbeddingRequest {
    model: String,
    input: String,
}

#[derive(serde::Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(serde::Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(serde::Serialize)]
struct MatchDocumentsRequest {
    query_embedding: Vec<f32>,
    match_threshold: f32,
    match_count: i32,
}

fn enrich_query_with_english(query: &str) -> String {
    let mut enriched = query.to_string();
    let query_lower = query.to_lowercase();
    
    // expiry / expiration / date
    if query_lower.contains("صلاحية") || query_lower.contains("منتهي") || query_lower.contains("تاريخ انتهاء") || query_lower.contains("اكسباير") || query_lower.contains("انتهاء") {
        enriched.push_str(" expiry expire expiration date batch Invoice_Items");
    }
    
    // products / items
    if query_lower.contains("منتج") || query_lower.contains("أدوية") || query_lower.contains("دواء") || query_lower.contains("أصناف") || query_lower.contains("صنف") {
        enriched.push_str(" product item medicine items ITEMS");
    }
    
    // invoice / billing / sales / buy
    if query_lower.contains("فاتورة") || query_lower.contains("فواتير") || query_lower.contains("بيع") || query_lower.contains("شراء") {
        enriched.push_str(" invoice bill sale purchase sale_items buy_items buy_invoice");
    }
    
    // customer / supplier
    if query_lower.contains("عميل") || query_lower.contains("عملاء") || query_lower.contains("زبون") || query_lower.contains("مورد") || query_lower.contains("موردين") {
        enriched.push_str(" customer supplier account CUSTOMERS");
    }
    
    // store / stock / qty
    if query_lower.contains("مخزن") || query_lower.contains("مخازن") || query_lower.contains("جرد") || query_lower.contains("رصيد") || query_lower.contains("كمية") {
        enriched.push_str(" store stock qty quantity jared inventory STORES");
    }
    
    // finance / cash / safe
    if query_lower.contains("خزينة") || query_lower.contains("صندوق") || query_lower.contains("مالية") || query_lower.contains("حسابات") || query_lower.contains("أرباح") || query_lower.contains("خسائر") {
        enriched.push_str(" finance cash safe profit loss account");
    }

    // debt / credit / balance — BALANCE_C is empty; steer RAG toward invoice math
    if query_lower.contains("دين") || query_lower.contains("ديون") || query_lower.contains("رصيد") || query_lower.contains("مديون") || query_lower.contains("علي") || query_lower.contains("استحقاق") {
        enriched.push_str(" debt receivable TAKE GIVE BALANCE_EDIT SALE_INVOICE BUY_INVOICE CUSTOMERS CUST_CUSTOM CUST_VENDOR");
    }

    // sales rep / commissioner (note: actually unused, but include for completeness)
    if query_lower.contains("مندوب") || query_lower.contains("مندوبين") {
        enriched.push_str(" supplier vendor مورد CUSTOMERS CUST_VENDOR COMMISSIONER");
    }

    enriched
}

pub async fn search_schema(keywords: &str, openai_key: &str) -> String {
    println!("search_schema called with keywords: {}, openai_key length: {}", keywords, openai_key.len());
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();
    let api_key = SUPABASE_ANON_KEY;
    
    if !openai_key.trim().is_empty() {
        println!("Using Vector RAG with OpenAI...");
        let enriched = enrich_query_with_english(keywords);
        println!("Enriched query for embedding: {}", enriched);
        
        let req = OpenAiEmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: enriched,
        };
        
        let emb_res = client.post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", openai_key.trim()))
            .json(&req)
            .send()
            .await;
            
        if let Ok(res) = emb_res {
            if let Ok(json) = res.json::<OpenAiEmbeddingResponse>().await {
                if let Some(data) = json.data.first() {
                    println!("Got OpenAI embedding with length {}", data.embedding.len());
                    let rpc_req = MatchDocumentsRequest {
                        query_embedding: data.embedding.clone(),
                        match_threshold: 0.05,
                        match_count: 3,
                    };
                    
                    let supabase_url = format!("{}/rest/v1/rpc/match_documents", SUPABASE_URL);
                    if let Ok(rpc_res) = client.post(supabase_url)
                        .header("apikey", api_key)
                        .header("Authorization", format!("Bearer {}", api_key))
                        .json(&rpc_req)
                        .send()
                        .await 
                    {
                        if let Ok(docs) = rpc_res.json::<Vec<DocumentRecord>>().await {
                            println!("Supabase returned {} documents from RAG", docs.len());
                            let mut result = String::new();
                            for doc in docs {
                                if let Some(c) = doc.content {
                                    // Use a much larger truncation limit to avoid cutting off table DDL definitions (definitions are typically ~1000-1500 chars)
                                    let truncated = if c.len() > 800 { &c[..800] } else { &c };
                                    result.push_str(truncated);
                                    result.push_str("\n\n");
                                }
                            }
                            if !result.is_empty() {
                                println!("Returning RAG result!");
                                let wrapped = format!(
                                    "DDL tables (use exact names, TOP not LIMIT):\n\n{}\n",
                                    result
                                );
                                return wrapped;
                            }
                        } else {
                            println!("Failed to parse Supabase RAG response");
                        }
                    } else {
                        println!("Supabase RAG POST failed");
                    }
                } else {
                    println!("OpenAI returned no embedding data");
                }
            } else {
                println!("Failed to parse OpenAI response");
            }
        } else {
            println!("OpenAI POST failed");
        }
    }

    println!("Falling back to ILIKE text matching...");
    // Enrich with English so ILIKE can match the English DDL content
    let enriched_for_ilike = enrich_query_with_english(keywords);
    let terms: Vec<&str> = enriched_for_ilike.split(|c: char| c == ',' || c == ' ')
        .filter(|s: &&str| !s.trim().is_empty() && s.trim().chars().count() > 2)
        .collect();
    let mut or_conditions = Vec::new();
    for term in &terms {
        or_conditions.push(format!("content.ilike.*{}*", term));
    }
    
    let query_param = if or_conditions.is_empty() {
        "content=not.is.null".to_string()
    } else {
        format!("or=({})", or_conditions.join(","))
    };
    
    let url = format!(
        "{}/rest/v1/documents?select=content&{}&limit=5",
        SUPABASE_URL, query_param
    );
    
    match client.get(&url)
        .header("apikey", api_key)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        Ok(res) => {
            if let Ok(docs) = res.json::<Vec<DocumentRecord>>().await {
                let mut result = String::new();
                for doc in docs {
                    if let Some(c) = doc.content {
                        result.push_str(&c);
                        result.push_str("\n\n");
                    }
                }
                if result.is_empty() {
                    return "لم يتم العثور على جداول مطابقة لهذه الكلمات المفتاحية.".to_string();
                }
                return result;
            }
        }
        Err(e) => return format!("Error fetching schema: {}", e),
    }
    "لم يتم العثور على بيانات".to_string()
}

// ─── Polling Task ──────────────────────────────────────────────
pub async fn start_polling(
    token: String,
    chat_id: String,
    groq_key: String,
    enable_queries: bool,
    app_state: Arc<AppState>,
    mut rx: oneshot::Receiver<()>,
    ai_model: String,
    openai_key: String,
) {
    if !enable_queries || token.is_empty() || chat_id.is_empty() {
        return;
    }

    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().unwrap();
    let mut offset = 0;
    let expected_chat_id = chat_id.trim().to_string();

    let mut chat_history: HashMap<i64, Vec<serde_json::Value>> = HashMap::new();
    let mut reports_cache = fetch_reports().await;

    loop {
        if rx.try_recv().is_ok() {
            println!("Stopping telegram bot polling");
            break;
        }

        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
            token, offset
        );

        match client.get(&url).send().await {
            Ok(res) => {
                if let Ok(json) = res.json::<TelegramResponse>().await {
                    if json.ok {
                        if let Some(updates) = json.result {
                            for update in updates {
                                offset = update.update_id + 1;
                                if let Some(msg) = update.message {
                                    if msg.chat.id.to_string() == expected_chat_id {
                                        if let Some(text) = msg.text {
                                            handle_message(
                                                &client, &token, msg.chat.id, text, &groq_key, &ai_model,
                                                &app_state, &mut reports_cache,
                                                &mut chat_history, &openai_key
                                            ).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => println!("Telegram poll error: {}", e),
        }

        sleep(Duration::from_secs(1)).await;
    }
}

async fn handle_message(
    client: &Client,
    token: &str,
    chat_id: i64,
    text: String,
    groq_key: &str,
    ai_model: &str,
    app_state: &Arc<AppState>,
    reports_cache: &mut Vec<SupabaseReport>,
    chat_history: &mut HashMap<i64, Vec<serde_json::Value>>,
    openai_key: &str,
) {
    let text_trim = text.trim();

    // شات حر بالكامل: كل رسالة تذهب للوكيل الذكي
    if groq_key.is_empty() {
        let _ = send_message(
            client,
            token,
            chat_id,
            "⚠️ الوكيل الذكي غير مُهيّأ.\nيرجى إضافة مفتاح OpenRouter (Groq) من إعدادات التطبيق.".to_string(),
        ).await;
        return;
    }

    let history_vec = chat_history.entry(chat_id).or_insert_with(Vec::new);
    crate::ai_agent::handle_with_groq(
        client,
        token,
        chat_id,
        text_trim,
        groq_key,
        ai_model,
        app_state,
        reports_cache,
        history_vec,
        openai_key,
    ).await;
}

// ─── إرسال رسالة نصية عادية (إشعارات) ───────────────────────────
pub async fn send_message(
    client: &Client, token: &str, chat_id: i64, text: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    client.post(&url)
        .json(&serde_json::json!({ "chat_id": chat_id, "text": text }))
        .send().await?;
    Ok(())
}

// ─── إرسال رسالة HTML منسّقة ─────────────────────────────────────
pub async fn send_html(
    client: &Client, token: &str, chat_id: i64, html: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    client.post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": html,
            "parse_mode": "HTML"
        }))
        .send().await?;
    Ok(())
}

// ─── إرسال PDF ────────────────────────────────────────────────────
pub async fn send_pdf(
    client: &Client, token: &str, chat_id: i64,
    filename: &str, content: Vec<u8>, caption: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{}/sendDocument", token);
    let part = reqwest::multipart::Part::bytes(content)
        .file_name(filename.to_string())
        .mime_str("application/pdf")?;
    let form = reqwest::multipart::Form::new()
        .text("chat_id",    chat_id.to_string())
        .text("caption",    caption.to_string())
        .text("parse_mode", "Markdown".to_string())
        .part("document",   part);
    client.post(&url).multipart(form).send().await?;
    Ok(())
}

// ─── إرسال Excel ───────────────────────────────────────────────────
pub async fn send_excel(
    client: &Client,
    token: &str,
    chat_id: i64,
    filename: &str,
    content: Vec<u8>,
    caption: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{}/sendDocument", token);
    let part = reqwest::multipart::Part::bytes(content)
        .file_name(filename.to_string())
        .mime_str(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )?;
    let form = reqwest::multipart::Form::new()
        .text("chat_id", chat_id.to_string())
        .text("caption", caption.to_string())
        .text("parse_mode", "Markdown".to_string())
        .part("document", part);
    client.post(&url).multipart(form).send().await?;
    Ok(())
}
