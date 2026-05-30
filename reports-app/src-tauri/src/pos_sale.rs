use crate::pdf_generator::{generate_pos_receipt_pdf, PosReceiptLine, PosReceiptMeta};
use crate::{build_config, AppState, SqlConnection};
use serde::{Deserialize, Serialize};
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PosProduct {
    pub bar_id: i32,
    pub item_id: i32,
    pub item_name: String,
    pub item_model: String,
    pub barcode: String,
    pub unit_id: i32,
    pub unit_desc: String,
    pub unit_qty: f64,
    pub price: f64,
    pub last_cost: f64,
    pub aver_cost: f64,
    pub public_price: f64,
    pub stock_qty: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PosReceiptLineInput {
    pub name: String,
    pub unit: String,
    pub qty: f64,
    pub price: f64,
}

fn sql_escape(value: &str) -> String {
    value.replace('\'', "''")
}

fn looks_like_barcode(q: &str) -> bool {
    let t = q.trim();
    t.len() >= 6 && t.chars().all(|c| c.is_ascii_digit())
}

fn local_receipt_no() -> String {
    chrono::Local::now().format("POS-%Y%m%d-%H%M%S").to_string()
}

async fn connect_client(conn: &SqlConnection) -> Result<Client<tokio_util::compat::Compat<TcpStream>>, String> {
    let mut config = build_config(conn);
    config.trust_cert();
    let tcp = TcpStream::connect(format!("{}:{}", conn.server, conn.port))
        .await
        .map_err(|e| format!("تعذّر الوصول للسيرفر: {}", e))?;
    Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("فشل الاتصال: {}", e))
}

fn parse_pos_products(rows: Vec<tiberius::Row>) -> Vec<PosProduct> {
    rows.iter()
        .filter_map(|row| {
            let bar_id: i32 = row.try_get::<i32, _>(0).ok().flatten()?;
            let item_id: i32 = row.try_get::<i32, _>(1).ok().flatten()?;
            let item_name = row
                .try_get::<&str, _>(2)
                .ok()
                .flatten()
                .unwrap_or("")
                .trim()
                .to_string();
            let item_model = row
                .try_get::<&str, _>(3)
                .ok()
                .flatten()
                .unwrap_or("")
                .trim()
                .to_string();
            let barcode = row
                .try_get::<&str, _>(4)
                .ok()
                .flatten()
                .unwrap_or("")
                .trim()
                .to_string();
            let unit_id: i32 = row.try_get::<i32, _>(5).ok().flatten().unwrap_or(0);
            let unit_desc = row
                .try_get::<&str, _>(6)
                .ok()
                .flatten()
                .unwrap_or("")
                .trim()
                .to_string();
            let unit_qty: f64 = row.try_get::<f64, _>(7).ok().flatten().unwrap_or(1.0);
            let price: f64 = row.try_get::<f64, _>(8).ok().flatten().unwrap_or(0.0);
            let last_cost: f64 = row.try_get::<f64, _>(9).ok().flatten().unwrap_or(0.0);
            let aver_cost: f64 = row.try_get::<f64, _>(10).ok().flatten().unwrap_or(0.0);
            let public_price: f64 = row.try_get::<f64, _>(11).ok().flatten().unwrap_or(0.0);
            let stock_qty: f64 = row.try_get::<f64, _>(12).ok().flatten().unwrap_or(0.0);
            Some(PosProduct {
                bar_id,
                item_id,
                item_name,
                item_model,
                barcode,
                unit_id,
                unit_desc,
                unit_qty,
                price,
                last_cost,
                aver_cost,
                public_price,
                stock_qty,
            })
        })
        .collect()
}

fn pos_product_sql(where_clause: &str, order_by: &str, top: u32) -> String {
    format!(
        "SET NOCOUNT ON;
SELECT TOP {top}
  B.BAR_ID,
  I.ITEM_ID,
  I.ITEM_NAME,
  ISNULL(CAST(I.ITEM_MODEL AS nvarchar(50)), N'') AS item_model,
  ISNULL(B.BARCODE, N'') AS barcode,
  ISNULL(B.UNIT_ID, 0) AS unit_id,
  ISNULL(U.UNIT_DISC, N'') AS unit_desc,
  ISNULL(NULLIF(B.UNIT_QTY, 0), 1) AS unit_qty,
  COALESCE(NULLIF(B.PRICE1, 0), NULLIF(B.PUBLIC_PRICE, 0), NULLIF(I.AVER_COST, 0), 0) AS price,
  ISNULL(I.LAST_COST, 0) AS last_cost,
  ISNULL(I.AVER_COST, 0) AS aver_cost,
  ISNULL(B.PUBLIC_PRICE, 0) AS public_price,
  ISNULL(S.stock_qty, 0) AS stock_qty
FROM dbo.BARCODE B
INNER JOIN dbo.ITEMS I ON B.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.UNITS U ON B.UNIT_ID = U.UNIT_ID
OUTER APPLY (
  SELECT SUM(sub.QTY) AS stock_qty
  FROM dbo.ITEMS_SUB sub
  WHERE sub.ITEM_ID = I.ITEM_ID AND sub.STORE_ID = 1
) S
WHERE I.ITEM_INVISIBLE = 0
  AND ({where_clause})
ORDER BY {order_by};"
    )
}

pub async fn search_pos_products_impl(
    conn: SqlConnection,
    query: String,
    erp: crate::erp_profile::ErpKind,
) -> Result<Vec<PosProduct>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let escaped = sql_escape(q);
    let mut client = connect_client(&conn).await?;

    let (barcode_sql, search_sql) = match erp {
        crate::erp_profile::ErpKind::InfinityRetailDb => (
            crate::erp_adapters::infinity_pos_product_sql(
                &format!("b.ProductBarcode = N'{escaped}'"),
                "b.ProductBarcode",
                5,
            ),
            crate::erp_adapters::infinity_pos_product_sql(
                &format!(
                    "p.ProductName LIKE N'%{escaped}%' \
                     OR p.ProductCode LIKE N'%{escaped}%' \
                     OR b.ProductBarcode LIKE N'%{escaped}%'"
                ),
                &format!(
                    "CASE WHEN b.ProductBarcode = N'{escaped}' THEN 0 \
                          WHEN p.ProductCode LIKE N'{escaped}%' THEN 1 \
                          WHEN p.ProductName LIKE N'{escaped}%' THEN 2 \
                          ELSE 3 END, p.ProductName"
                ),
                20,
            ),
        ),
        _ => (
            pos_product_sql(
                &format!("B.BARCODE = N'{escaped}'"),
                "B.BARCODE",
                5,
            ),
            pos_product_sql(
                &format!(
                    "I.ITEM_NAME LIKE N'%{escaped}%' \
                     OR CAST(I.ITEM_MODEL AS nvarchar(50)) LIKE N'%{escaped}%' \
                     OR B.BARCODE LIKE N'%{escaped}%'"
                ),
                &format!(
                    "CASE WHEN B.BARCODE = N'{escaped}' THEN 0 \
                          WHEN CAST(I.ITEM_MODEL AS nvarchar(50)) LIKE N'{escaped}%' THEN 1 \
                          WHEN I.ITEM_NAME LIKE N'{escaped}%' THEN 2 \
                          ELSE 3 END, I.ITEM_NAME"
                ),
                20,
            ),
        ),
    };

    if looks_like_barcode(q) {
        let rows = client
            .simple_query(&barcode_sql)
            .await
            .map_err(|e| format!("خطأ في البحث: {}", e))?
            .into_first_result()
            .await
            .map_err(|e| format!("خطأ في القراءة: {}", e))?;
        let hits = parse_pos_products(rows);
        if !hits.is_empty() {
            return Ok(hits);
        }
    }

    let rows = client
        .simple_query(&search_sql)
        .await
        .map_err(|e| format!("خطأ في البحث: {}", e))?
        .into_first_result()
        .await
        .map_err(|e| format!("خطأ في القراءة: {}", e))?;

    Ok(parse_pos_products(rows))
}

pub async fn print_pos_receipt_impl(
    conn: SqlConnection,
    cust_name: &str,
    note: Option<&str>,
    lines: &[PosReceiptLineInput],
    erp: crate::erp_profile::ErpKind,
) -> Result<String, String> {
    if lines.is_empty() {
        return Err("لا توجد بنود للطباعة.".to_string());
    }

    let business = crate::erp_adapters::fetch_receipt_business(&conn, erp).await?;
    let receipt_lines: Vec<PosReceiptLine> = lines
        .iter()
        .map(|line| PosReceiptLine {
            name: line.name.clone(),
            unit: line.unit.clone(),
            qty: line.qty,
            price: line.price,
        })
        .collect();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let receipt_no = local_receipt_no();
    let note_text = note.unwrap_or("").trim();
    let customer = if note_text.is_empty() {
        cust_name.to_string()
    } else {
        format!("{cust_name} — {note_text}")
    };

    let meta = PosReceiptMeta {
        company_name: if business.company_name.is_empty() {
            "إثبات بيع".to_string()
        } else {
            business.company_name
        },
        address: business.address,
        phone: business.phone,
        invoice_no: receipt_no.clone(),
        invoice_time: now,
        customer_name: customer,
        is_draft: false,
    };

    write_receipt_pdf(
        &meta,
        &receipt_lines,
        &format!("pos_receipt_{}.pdf", receipt_no.replace(':', "")),
    )
}

fn write_receipt_pdf(meta: &PosReceiptMeta, lines: &[PosReceiptLine], filename: &str) -> Result<String, String> {
    let bytes = generate_pos_receipt_pdf(meta, lines)?;
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, &bytes).map_err(|e| format!("فشل حفظ PDF: {}", e))?;
    Ok(path.display().to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn search_pos_products(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PosProduct>, String> {
    let conn = state
        .conn
        .lock()
        .await
        .clone()
        .ok_or("غير متصل بقاعدة البيانات. سجّل الدخول أولاً.")?;
    let erp = crate::erp_profile::resolve_erp_kind(&state, &conn).await;
    search_pos_products_impl(conn, query, erp).await
}

/// طباعة إثبات بيع محلي — لا يكتب أي شيء في ERP
#[tauri::command(rename_all = "camelCase")]
pub async fn print_pos_receipt(
    cust_name: String,
    note: Option<String>,
    lines: Vec<PosReceiptLineInput>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let conn = state
        .conn
        .lock()
        .await
        .clone()
        .ok_or("غير متصل بقاعدة البيانات. سجّل الدخول أولاً.")?;
    let erp = crate::erp_profile::resolve_erp_kind(&state, &conn).await;
    print_pos_receipt_impl(
        conn,
        cust_name.trim(),
        note.as_deref(),
        &lines,
        erp,
    )
    .await
}
