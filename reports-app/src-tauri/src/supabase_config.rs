use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const SUPABASE_URL: &str = "https://nsgmhijtaaenpqxxgjds.supabase.co";
pub const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im5zZ21oaWp0YWFlbnBxeHhnamRzIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxODU1NTMsImV4cCI6MjA5NDc2MTU1M30.bva5PiwsoBiLR7u2upQV7q2spl6GhAg-JqrQ8nnUC8E";

const ACCESS_TOKEN_STORE_KEY: &str = "app_access_token";
/// مفتاح ربط التطبيق بـ Supabase (يُخزَّن hash في app_access)
pub const DEFAULT_APP_ACCESS_TOKEN: &str = "tPg1lWttWj71HBpxPYHDIgBSkJMEjHlEG9CpZbM4N_k";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSecretsSettings {
    pub openrouter_api_key: String,
    pub openai_api_key: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
}

impl AppSecretsSettings {
    pub fn to_remote_json(&self) -> Value {
        json!({
            "openrouter_api_key": self.openrouter_api_key.trim(),
            "openai_api_key": self.openai_api_key.trim(),
        })
    }

    pub fn to_json(&self) -> Value {
        self.to_remote_json()
    }

    pub fn from_json(value: &Value) -> Self {
        let s = |key: &str| {
            value
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        Self {
            openrouter_api_key: s("openrouter_api_key"),
            openai_api_key: s("openai_api_key"),
            telegram_bot_token: s("telegram_bot_token"),
            telegram_chat_id: s("telegram_chat_id"),
        }
    }

    pub fn has_remote_payload(&self) -> bool {
        !self.openrouter_api_key.trim().is_empty()
            || !self.openai_api_key.trim().is_empty()
    }
}

fn http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap()
}

fn supabase_headers() -> [(&'static str, String); 2] {
    [
        ("apikey", SUPABASE_ANON_KEY.to_string()),
        ("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY)),
    ]
}

pub fn generate_access_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn fetch_secrets_from_supabase(access_token: &str) -> Result<AppSecretsSettings, String> {
    let client = http_client();
    let url = format!("{}/rest/v1/rpc/get_app_secrets", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&json!({ "p_access_token": access_token.trim() }))
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Supabase get_app_secrets: {}", body));
    }

    let value: Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(AppSecretsSettings::from_json(&value))
}

pub async fn save_secrets_to_supabase(
    access_token: &str,
    settings: &AppSecretsSettings,
) -> Result<(), String> {
    if !settings.has_remote_payload() {
        return Err("لا توجد مفاتيح للحفظ.".to_string());
    }

    let client = http_client();
    let url = format!("{}/rest/v1/rpc/save_app_secrets", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&json!({
            "p_access_token": access_token.trim(),
            "p_secrets": settings.to_remote_json(),
        }))
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Supabase save_app_secrets: {}", body));
    }
    Ok(())
}

pub fn read_stored_access_token(
    app: &AppHandle,
    decrypt: fn(String) -> Result<String, String>,
) -> Result<Option<String>, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let Some(enc) = store.get(ACCESS_TOKEN_STORE_KEY) else {
        return Ok(None);
    };
    let enc_str = enc.as_str().unwrap_or("").to_string();
    if enc_str.is_empty() {
        return Ok(None);
    }
    decrypt(enc_str).map(Some)
}

pub fn store_access_token(
    app: &AppHandle,
    token: &str,
    encrypt: fn(String) -> Result<String, String>,
) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let encrypted = encrypt(token.to_string())?;
    store.set(ACCESS_TOKEN_STORE_KEY, encrypted);
    store.save().map_err(|e| e.to_string())
}

pub fn load_local_telegram_settings(
    app: &AppHandle,
    decrypt: fn(String) -> Result<String, String>,
) -> Result<(String, String), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut token = String::new();
    let mut chat_id = String::new();
    if let Some(v) = store.get("telegram_bot_token") {
        let enc = v.as_str().unwrap_or("").to_string();
        if !enc.is_empty() {
            token = decrypt(enc).unwrap_or_default();
        }
    }
    if let Some(v) = store.get("telegram_chat_id") {
        let enc = v.as_str().unwrap_or("").to_string();
        if !enc.is_empty() {
            chat_id = decrypt(enc).unwrap_or_default();
        }
    }
    Ok((token, chat_id))
}

pub fn save_local_telegram_settings(
    app: &AppHandle,
    token: &str,
    chat_id: &str,
    encrypt: fn(String) -> Result<String, String>,
) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    if token.trim().is_empty() {
        store.set("telegram_bot_token", Value::Null);
    } else {
        store.set("telegram_bot_token", encrypt(token.trim().to_string())?);
    }
    if chat_id.trim().is_empty() {
        store.set("telegram_chat_id", Value::Null);
    } else {
        store.set("telegram_chat_id", encrypt(chat_id.trim().to_string())?);
    }
    store.save().map_err(|e| e.to_string())
}

pub fn load_legacy_secrets_from_store(
    app: &AppHandle,
    decrypt: fn(String) -> Result<String, String>,
) -> Result<AppSecretsSettings, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut settings = AppSecretsSettings::default();

    if let Some(v) = store.get("groq_api_key") {
        let enc = v.as_str().unwrap_or("").to_string();
        if !enc.is_empty() {
            settings.openrouter_api_key = decrypt(enc).unwrap_or_default();
        }
    }
    if let Some(v) = store.get("openai_api_key") {
        let enc = v.as_str().unwrap_or("").to_string();
        if !enc.is_empty() {
            settings.openai_api_key = decrypt(enc).unwrap_or_default();
        }
    }
    Ok(settings)
}

pub fn clear_legacy_ai_keys(app: &AppHandle) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("groq_api_key", Value::Null);
    store.set("openai_api_key", Value::Null);
    store.save().map_err(|e| e.to_string())
}

fn access_token_for_fetch(
    app: &AppHandle,
    decrypt: fn(String) -> Result<String, String>,
) -> Result<String, String> {
    if let Some(token) = read_stored_access_token(app, decrypt)? {
        if !token.is_empty() {
            return Ok(token);
        }
    }
    Ok(DEFAULT_APP_ACCESS_TOKEN.to_string())
}

pub async fn resolve_app_secrets(
    app: &AppHandle,
    decrypt: fn(String) -> Result<String, String>,
    _encrypt: fn(String) -> Result<String, String>,
) -> Result<AppSecretsSettings, String> {
    let (telegram_bot_token, telegram_chat_id) = load_local_telegram_settings(app, decrypt)?;

    // 1. المفتاح المحلي أولاً — أسرع وأكثر موثوقية
    let mut legacy = load_legacy_secrets_from_store(app, decrypt)?;
    if legacy.has_remote_payload() {
        legacy.telegram_bot_token = telegram_bot_token;
        legacy.telegram_chat_id = telegram_chat_id;
        return Ok(legacy);
    }

    // 2. إذا لا يوجد مفتاح محلي → نجرب Supabase كاحتياطي
    let access_token = access_token_for_fetch(app, decrypt)?;
    let remote_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        fetch_secrets_from_supabase(&access_token),
    )
    .await;

    if let Ok(Ok(mut remote)) = remote_result {
        if remote.has_remote_payload() {
            remote.telegram_bot_token = telegram_bot_token;
            remote.telegram_chat_id = telegram_chat_id;
            return Ok(remote);
        }
    } else if remote_result.is_err() {
        eprintln!("[secrets] Supabase fetch timed out — no local OpenRouter key found");
    }

    Ok(AppSecretsSettings {
        telegram_bot_token,
        telegram_chat_id,
        ..Default::default()
    })
}

pub fn supabase_rest_url(path: &str) -> String {
    format!("{}/rest/v1/{}", SUPABASE_URL, path.trim_start_matches('/'))
}

#[allow(dead_code)]
pub fn supabase_headers_vec() -> Vec<(String, String)> {
    supabase_headers()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}
