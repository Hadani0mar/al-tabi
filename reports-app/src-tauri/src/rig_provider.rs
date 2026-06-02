//! طبقة استخلاص موحّدة للمزودين عبر rig-core.
//! تدعم OpenRouter, Anthropic, OpenAI, وأي endpoint متوافق مع OpenAI.

use rig::completion::{Chat, Message};
use rig::providers::{anthropic, openai};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── ثوابت نماذج Claude 4 (أحدث من ما في rig-core 0.5) ─────────────────────
pub const CLAUDE_HAIKU_4_5: &str = "claude-haiku-4-5-20251001";
pub const CLAUDE_SONNET_4_6: &str = "claude-sonnet-4-6";
pub const CLAUDE_OPUS_4_8: &str = "claude-opus-4-8";

const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

// ─── المزود ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenRouter,
    Anthropic,
    OpenAi,
    Custom(String), // base_url
}

impl Provider {
    pub fn label(&self) -> &str {
        match self {
            Provider::OpenRouter => "OpenRouter",
            Provider::Anthropic => "Anthropic",
            Provider::OpenAi => "OpenAI",
            Provider::Custom(url) => url,
        }
    }
}

// ─── عميل موحّد ───────────────────────────────────────────────────────────────

/// عميل AI موحّد — يدعم OpenRouter, Anthropic, وأي endpoint متوافق مع OpenAI.
/// يُستخدم لـ chat بسيط بدون tool-calling (مثل التلخيص والإجابات العامة).
pub struct RigClient {
    pub provider: Provider,
    pub api_key: String,
    pub model: String,
}

impl RigClient {
    /// OpenRouter (يدعم جميع النماذج عبر OpenAI-compatible API)
    pub fn openrouter(api_key: &str, model: &str) -> Self {
        Self {
            provider: Provider::OpenRouter,
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Anthropic مباشرةً (Claude — بدون OpenRouter كوسيط)
    pub fn anthropic(api_key: &str, model: &str) -> Self {
        Self {
            provider: Provider::Anthropic,
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// OpenAI
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self {
            provider: Provider::OpenAi,
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// أي endpoint متوافق مع OpenAI (Ollama, LM Studio, ...)
    pub fn custom(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            provider: Provider::Custom(base_url.to_string()),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    // ─── الدوال الرئيسية ──────────────────────────────────────────────────────

    /// دردشة بسيطة — system prompt + سؤال واحد، ترجع نص الإجابة.
    pub async fn one_shot(&self, system: &str, user_prompt: &str) -> Result<String, String> {
        match &self.provider {
            Provider::Anthropic => {
                let client = anthropic::ClientBuilder::new(&self.api_key).build();
                let agent = client.agent(&self.model).preamble(system).build();
                agent
                    .chat(user_prompt, vec![])
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => {
                let base_url = self.openai_base_url();
                let client = openai::Client::from_url(&self.api_key, &base_url);
                let agent = client.agent(&self.model).preamble(system).build();
                agent
                    .chat(user_prompt, vec![])
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }

    /// دردشة مع تاريخ المحادثة.
    /// messages: قائمة رسائل بصيغة {"role": "user"|"assistant", "content": "..."}
    /// آخر رسالة من نوع user تُستخرج تلقائياً كـ prompt.
    pub async fn chat_with_history(
        &self,
        system: &str,
        messages: &[Value],
    ) -> Result<String, String> {
        let (prompt, history) = extract_prompt_and_history(messages);
        match &self.provider {
            Provider::Anthropic => {
                let client = anthropic::ClientBuilder::new(&self.api_key).build();
                let agent = client.agent(&self.model).preamble(system).build();
                agent
                    .chat(&prompt, history)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => {
                let base_url = self.openai_base_url();
                let client = openai::Client::from_url(&self.api_key, &base_url);
                let agent = client.agent(&self.model).preamble(system).build();
                agent
                    .chat(&prompt, history)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }

    fn openai_base_url(&self) -> String {
        match &self.provider {
            Provider::OpenRouter => OPENROUTER_BASE_URL.to_string(),
            Provider::OpenAi => "https://api.openai.com".to_string(),
            Provider::Custom(url) => url.clone(),
            Provider::Anthropic => unreachable!(),
        }
    }
}

// ─── تحويل رسائل JSON → rig Messages ─────────────────────────────────────────

/// يستخرج آخر رسالة user كـ prompt والبقية كـ history.
/// يتجاهل: system messages، tool_call messages (content=null)، tool result messages.
fn extract_prompt_and_history(messages: &[Value]) -> (String, Vec<Message>) {
    let usable: Vec<&Value> = messages
        .iter()
        .filter(|m| {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let has_content = m.get("content").and_then(|c| c.as_str()).is_some();
            // نأخذ فقط user/assistant مع محتوى نصي
            (role == "user" || role == "assistant") && has_content
        })
        .collect();

    if usable.is_empty() {
        return (String::new(), vec![]);
    }

    // آخر رسالة user كـ prompt
    let prompt = if usable
        .last()
        .and_then(|m| m.get("role").and_then(|r| r.as_str()))
        == Some("user")
    {
        usable
            .last()
            .and_then(|m| m.get("content").and_then(|c| c.as_str()))
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    let history_slice = if prompt.is_empty() {
        &usable[..]
    } else {
        &usable[..usable.len().saturating_sub(1)]
    };

    let history: Vec<Message> = history_slice
        .iter()
        .filter_map(|msg| {
            let role = msg.get("role")?.as_str()?.to_string();
            let content = msg.get("content")?.as_str()?.to_string();
            if content.is_empty() {
                return None;
            }
            Some(Message { role, content })
        })
        .collect();

    (prompt, history)
}

// ─── Tauri command مساعد ─────────────────────────────────────────────────────

/// استجابة rig للواجهة
#[derive(Serialize)]
pub struct RigResponse {
    pub text: String,
    pub provider: String,
    pub model: String,
}

/// أرسل رسالة عبر rig مع اختيار المزود.
/// يُستخدم للاختبار والإجابات البسيطة بدون tool-calling.
#[tauri::command]
pub async fn rig_chat(
    provider: String,
    api_key: String,
    model: String,
    system: String,
    message: String,
) -> Result<RigResponse, String> {
    let client = match provider.as_str() {
        "anthropic" => RigClient::anthropic(&api_key, &model),
        "openrouter" => RigClient::openrouter(&api_key, &model),
        "openai" => RigClient::openai(&api_key, &model),
        url => RigClient::custom(url, &api_key, &model),
    };

    let text = client.one_shot(&system, &message).await?;

    Ok(RigResponse {
        text,
        provider,
        model,
    })
}

/// قائمة المزودين والنماذج المدعومة
#[tauri::command]
pub fn list_rig_providers() -> Value {
    serde_json::json!({
        "providers": [
            {
                "id": "openrouter",
                "label": "OpenRouter",
                "url": "https://openrouter.ai",
                "models": [
                    { "id": "minimax/minimax-m3", "label": "MiniMax M3 (افتراضي)" },
                    { "id": "anthropic/claude-sonnet-4-6", "label": "Claude Sonnet 4.6" },
                    { "id": "anthropic/claude-haiku-4-5", "label": "Claude Haiku 4.5 (سريع)" },
                    { "id": "anthropic/claude-opus-4-8", "label": "Claude Opus 4.8 (أقوى)" },
                    { "id": "google/gemini-2.0-flash-001", "label": "Gemini 2.0 Flash" }
                ]
            },
            {
                "id": "anthropic",
                "label": "Anthropic (مباشر)",
                "url": "https://api.anthropic.com",
                "models": [
                    { "id": CLAUDE_SONNET_4_6, "label": "Claude Sonnet 4.6" },
                    { "id": CLAUDE_HAIKU_4_5, "label": "Claude Haiku 4.5" },
                    { "id": CLAUDE_OPUS_4_8, "label": "Claude Opus 4.8" }
                ]
            }
        ]
    })
}
