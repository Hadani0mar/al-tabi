//! عميل Gotenberg لتحويل HTML إلى PDF عبر Chromium.

use base64::Engine;
use std::path::PathBuf;

fn read_dotenv_value(key: &str) -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::current_dir() {
        candidates.push(dir.join(".env"));
        candidates.push(dir.join("..").join(".env"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(".env"));
            candidates.push(dir.join("..").join(".env"));
            candidates.push(dir.join("..").join("..").join(".env"));
            candidates.push(dir.join("..").join("..").join("..").join(".env"));
        }
    }

    for path in candidates {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((k, v)) = trimmed.split_once('=') else {
                continue;
            };
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    None
}

fn gotenberg_env(key: &str) -> Result<String, String> {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| read_dotenv_value(key))
        .ok_or_else(|| format!("متغير البيئة {} غير موجود.", key))
}

fn basic_auth_header() -> Result<String, String> {
    let user = gotenberg_env("GOTENBERG_USERNAME")?;
    let pass = gotenberg_env("GOTENBERG_PASSWORD")?;
    let raw = format!("{}:{}", user, pass);
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
    Ok(format!("Basic {}", encoded))
}

/// يرسل HTML إلى Gotenberg ويعيد بايتات PDF.
pub async fn html_to_pdf(html: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("فشل بناء HTTP client: {e}"))?;

    let base_url = gotenberg_env("GOTENBERG_URL")?.trim_end_matches('/').to_string();
    let url = format!("{}/forms/chromium/convert/html", base_url);

    let part = reqwest::multipart::Part::bytes(html.as_bytes().to_vec())
        .file_name("index.html")
        .mime_str("text/html; charset=utf-8")
        .map_err(|e| format!("mime error: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .part("files", part)
        .text("paperWidth", "11.69")   // A4 landscape width
        .text("paperHeight", "8.27")   // A4 landscape height
        .text("marginTop", "0.4")
        .text("marginBottom", "0.4")
        .text("marginLeft", "0.4")
        .text("marginRight", "0.4")
        .text("printBackground", "true")
        .text("preferCssPageSize", "false");

    let resp = client
        .post(&url)
        .header("Authorization", basic_auth_header()?)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("خطأ في الاتصال بـ Gotenberg: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Gotenberg HTTP {status}: {body}"));
    }

    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("فشل قراءة استجابة PDF: {e}"))
}
