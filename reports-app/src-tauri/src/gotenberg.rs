//! عميل Gotenberg لتحويل HTML إلى PDF عبر Chromium.

use base64::Engine;

const GOTENBERG_URL: &str = "http://187.127.111.243:32768";
const GOTENBERG_USER: &str = "admin";
const GOTENBERG_PASS: &str = "Flashdb@3200";

fn basic_auth_header() -> String {
    let raw = format!("{}:{}", GOTENBERG_USER, GOTENBERG_PASS);
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
    format!("Basic {}", encoded)
}

/// يرسل HTML إلى Gotenberg ويعيد بايتات PDF.
pub async fn html_to_pdf(html: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("فشل بناء HTTP client: {e}"))?;

    let url = format!("{}/forms/chromium/convert/html", GOTENBERG_URL);

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
        .header("Authorization", basic_auth_header())
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
