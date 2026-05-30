//! مزامنة منتجات الصيدلية إلى Supabase (مشاركة عامة — بدون كميات)

use crate::erp_adapters;
use crate::erp_profile::ErpKind;
use crate::{execute_sql_query, BusinessProfile, SqlConnection};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

use std::collections::HashSet;

use crate::supabase_config::{SUPABASE_ANON_KEY, SUPABASE_URL};

const STORE_KEY: &str = "pharmacy_share_settings";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PharmacyShareSettings {
    pub sync_key: String,
    pub sharing_enabled: bool,
    pub show_prices: bool,
    pub last_sync_at: Option<String>,
    pub last_product_count: u32,
    pub last_business_name: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PharmacyShareSyncResult {
    pub product_count: u32,
    pub business_name: String,
    pub city: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
struct ShareProductRow {
    code: Option<String>,
    name: String,
    price: Option<f64>,
    quantity: i64,
    nearest_expiry: Option<String>,
}

fn http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap()
}

async fn supabase_rpc(name: &str, body: Value) -> Result<Value, String> {
    let url = format!("{}/rest/v1/rpc/{}", SUPABASE_URL, name);
    let res = http_client()
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("فشل الاتصال بـ Supabase: {}", e))?;

    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        eprintln!("[pharmacy_share] Supabase {name} HTTP {status}: {text}");
        return Err(format!("Supabase {name}: {text}"));
    }
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).map_err(|e| format!("Supabase {name} parse: {e}"))
}

pub fn marketing_shareable_products_sql() -> &'static str {
    r#"SET NOCOUNT ON;
;WITH Stock AS (
  SELECT
    sub.ITEM_ID,
    CAST(SUM(sub.QTY) AS int) AS total_qty,
    CONVERT(varchar(10), MIN(CAST(sub.CATEOGRY3 AS date)), 23) AS nearest_expiry
  FROM dbo.ITEMS_SUB sub
  WHERE sub.QTY > 0
    AND sub.CATEOGRY3 IS NOT NULL
    AND CAST(sub.CATEOGRY3 AS date) >= CAST(GETDATE() AS date)
  GROUP BY sub.ITEM_ID
  HAVING SUM(sub.QTY) > 0
),
PricePick AS (
  SELECT BC.ITEM_ID,
    MAX(ISNULL(BC.PRICE1, 0))       AS SellPrice,
    MAX(ISNULL(BC.PUBLIC_PRICE, 0)) AS PublicPrice
  FROM dbo.BARCODE BC
  GROUP BY BC.ITEM_ID
)
SELECT
  NULLIF(LTRIM(RTRIM(CAST(I.ITEM_MODEL AS nvarchar(50)))), N'') AS product_code,
  LEFT(LTRIM(RTRIM(I.ITEM_NAME)), 500) AS product_name,
  CAST(
    CASE
      WHEN ISNULL(PP.SellPrice, 0)   > 0 THEN PP.SellPrice
      WHEN ISNULL(PP.PublicPrice, 0) > 0 THEN PP.PublicPrice
      ELSE NULL
    END AS decimal(18, 2)
  ) AS price,
  S.total_qty AS quantity,
  S.nearest_expiry
FROM dbo.ITEMS I
INNER JOIN Stock S ON S.ITEM_ID = I.ITEM_ID
LEFT JOIN PricePick PP ON PP.ITEM_ID = I.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND LTRIM(RTRIM(I.ITEM_NAME)) <> N''
ORDER BY I.ITEM_NAME"#
}

pub fn infinity_shareable_products_sql() -> &'static str {
    r#"SET NOCOUNT ON;
-- المخزون في Data_Products.StockOnHand، الصلاحية في Data_ProductInventories.ExpiryDate
;WITH ValidExpiry AS (
  SELECT
    i.ProductID_FK,
    CONVERT(varchar(10), MIN(CAST(i.ExpiryDate AS date)), 23) AS nearest_expiry
  FROM Inventory.Data_ProductInventories i
  WHERE i.ExpiryDate IS NOT NULL
    AND CAST(i.ExpiryDate AS date) >= CAST(GETDATE() AS date)
  GROUP BY i.ProductID_FK
)
SELECT
  NULLIF(LTRIM(RTRIM(p.ProductCode)), N'') AS product_code,
  LEFT(LTRIM(RTRIM(p.ProductName)), 500) AS product_name,
  CAST(
    NULLIF(
      (SELECT MAX(ISNULL(b.UomPrice1, 0))
       FROM Inventory.Data_View_ProductUOMBarcodes b
       WHERE b.ProductID_FK = p.ProductID_PK),
      0
    ) AS decimal(18, 2)
  ) AS price,
  CAST(p.StockOnHand AS int) AS quantity,
  ve.nearest_expiry
FROM Inventory.Data_Products p
INNER JOIN ValidExpiry ve ON ve.ProductID_FK = p.ProductID_PK
WHERE p.IsInActive = 0
  AND LTRIM(RTRIM(p.ProductName)) <> N''
  AND ISNULL(p.StockOnHand, 0) > 0
ORDER BY p.ProductName"#
}

fn parse_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse().ok()
}

pub async fn fetch_shareable_products(
    conn: &SqlConnection,
    erp: ErpKind,
) -> Result<Vec<ShareProductRow>, String> {
    let sql = match erp {
        ErpKind::InfinityRetailDb => infinity_shareable_products_sql(),
        ErpKind::Marketing2026 | ErpKind::Unknown => marketing_shareable_products_sql(),
    };
    let result = execute_sql_query(conn.clone(), sql.to_string()).await?;
    let mut out = Vec::new();
    for row in &result.rows {
        let code = row
            .get(0)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let name = row.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let price = row.get(2).and_then(|s| parse_f64(s));
        let quantity = row
            .get(3)
            .and_then(|s| {
                let t = s.trim();
                t.parse::<i64>().ok().or_else(|| t.parse::<f64>().ok().map(|f| f as i64))
            })
            .unwrap_or(0);
        let nearest_expiry = row
            .get(4)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        out.push(ShareProductRow { code, name, price, quantity, nearest_expiry });
    }
    Ok(out)
}

fn dedupe_share_products(products: Vec<ShareProductRow>) -> Vec<ShareProductRow> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(products.len());
    for p in products {
        let key = p
            .code
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| format!("c:{s}"))
            .unwrap_or_else(|| format!("n:{}", p.name.trim().to_lowercase()));
        if seen.insert(key) && p.quantity > 0 {
            out.push(p);
        }
    }
    out
}

fn pharmacy_contact_phone(profile: &BusinessProfile) -> Option<String> {
    profile
        .phone
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| profile.mobile.as_ref().filter(|s| !s.trim().is_empty()))
        .cloned()
}

fn pharmacy_display_name(profile: &BusinessProfile) -> String {
    profile
        .company_name
        .clone()
        .or_else(|| profile.activity_name.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "صيدلية".to_string())
}

fn pharmacy_city(profile: &BusinessProfile) -> String {
    profile
        .city
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "طرابلس".to_string())
}

pub async fn push_pharmacy_profile(
    sync_key: &str,
    profile: &BusinessProfile,
    show_prices: bool,
    is_active: bool,
) -> Result<(), String> {
    let key = sync_key.trim();
    if key.len() < 8 {
        return Err("مفتاح المزامنة (sync_key) غير صالح.".to_string());
    }
    let resp = supabase_rpc(
        "update_pharmacy_profile",
        json!({
            "p_sync_key": key,
            "p_name": pharmacy_display_name(profile),
            "p_city": pharmacy_city(profile),
            "p_address": profile.address.as_deref().unwrap_or(""),
            "p_phone": pharmacy_contact_phone(profile).unwrap_or_default(),
            "p_show_prices": show_prices,
            "p_is_active": is_active,
        }),
    )
    .await?;
    if resp.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let err = resp
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("فشل تحديث بيانات النشاط");
        return Err(err.to_string());
    }
    Ok(())
}

pub async fn clear_remote_products(sync_key: &str) -> Result<u32, String> {
    let resp = supabase_rpc(
        "clear_pharmacy_products",
        json!({ "p_sync_key": sync_key.trim() }),
    )
    .await?;
    if resp.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let err = resp
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("فشل حذف المنتجات");
        return Err(err.to_string());
    }
    Ok(resp
        .get("deleted")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32)
}

pub async fn push_products(
    sync_key: &str,
    products: &[ShareProductRow],
    show_prices: bool,
) -> Result<u32, String> {
    let payload: Vec<Value> = products
        .iter()
        .map(|p| {
            json!({
                "product_code": p.code,
                "product_name": p.name,
                "product_name_en": null,
                "price": if show_prices { p.price } else { None::<f64> },
                "quantity": p.quantity,
                "nearest_expiry": p.nearest_expiry,
                "is_available": true,
            })
        })
        .collect();

    let resp = supabase_rpc(
        "upsert_pharmacy_products",
        json!({
            "p_sync_key": sync_key.trim(),
            "p_products": payload,
        }),
    )
    .await?;

    if resp.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let err = resp
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("فشل رفع المنتجات");
        return Err(err.to_string());
    }
    Ok(resp
        .get("synced")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32)
}

fn emit_progress(app: &AppHandle, percent: u8, detail: &str) {
    let _ = app.emit(
        "pharmacy-sync-progress",
        json!({ "percent": percent, "detail": detail }),
    );
}

pub async fn sync_pharmacy_share(
    app: &AppHandle,
    conn: &SqlConnection,
    erp: ErpKind,
    settings: &PharmacyShareSettings,
) -> Result<PharmacyShareSyncResult, String> {
    let key = settings.sync_key.trim();
    if key.len() < 8 {
        return Err("أدخل مفتاح المزامنة (sync_key) من Supabase.".to_string());
    }

    emit_progress(app, 5, "جاري قراءة بيانات الصيدلية...");
    let profile = erp_adapters::fetch_business_profile(conn, erp).await?;
    let business_name = pharmacy_display_name(&profile);

    emit_progress(app, 20, "جاري تحديث بيانات الصيدلية على الموقع...");
    push_pharmacy_profile(key, &profile, settings.show_prices, true).await?;

    emit_progress(app, 35, "جاري جلب المنتجات من قاعدة البيانات...");
    let raw = fetch_shareable_products(conn, erp).await?;
    let raw_len = raw.len();
    let products = dedupe_share_products(raw);

    emit_progress(
        app,
        60,
        &format!("جاري رفع {} منتجاً إلى Supabase...", products.len()),
    );
    let count = push_products(key, &products, settings.show_prices).await?;

    emit_progress(app, 100, &format!("اكتملت المزامنة — {} منتج", count));
    eprintln!("[pharmacy_share] sync ok raw={raw_len} synced={count}");

    Ok(PharmacyShareSyncResult {
        product_count: count,
        business_name,
        city: Some(pharmacy_city(&profile)),
        message: format!("تمت مشاركة {count} منتجاً متوفراً."),
    })
}

pub async fn stop_pharmacy_sharing(sync_key: &str) -> Result<String, String> {
    let deleted = clear_remote_products(sync_key).await?;
    Ok(format!("تم إيقاف المشاركة وحذف {deleted} منتجاً من الموقع."))
}

pub fn load_settings(app: &AppHandle) -> Result<PharmacyShareSettings, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    match store.get(STORE_KEY) {
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| e.to_string()),
        None => Ok(PharmacyShareSettings::default()),
    }
}

pub fn save_settings(app: &AppHandle, settings: &PharmacyShareSettings) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set(
        STORE_KEY,
        serde_json::to_value(settings).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())
}
