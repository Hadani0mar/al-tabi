use crate::pdf_generator::{generate_pos_receipt_pdf, PosReceiptLine, PosReceiptMeta};
use crate::{AppState, SqlConnection};
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
    let config = crate::prepare_config(conn);
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
    print_receipt_to_default_printer(meta, lines, filename)?;
    Ok(path.display().to_string())
}

#[cfg(target_os = "windows")]
fn print_receipt_to_default_printer(meta: &PosReceiptMeta, lines: &[PosReceiptLine], filename: &str) -> Result<(), String> {
    let html_filename = filename.trim_end_matches(".pdf").to_string() + ".html";
    let html_path = std::env::temp_dir().join(html_filename);
    std::fs::write(&html_path, receipt_print_html(meta, lines))
        .map_err(|e| format!("فشل تجهيز ملف الطباعة: {}", e))?;

    let browser = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ]
    .iter()
    .find(|p| std::path::Path::new(p).exists())
    .ok_or("تعذر العثور على Microsoft Edge أو Google Chrome للطباعة الصامتة.".to_string())?;

    let url = file_url(&html_path)?;
    let profile_dir = std::env::temp_dir().join(format!(
        "reports_app_print_profile_{}",
        filename
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
    ));
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("فشل تجهيز ملف الطباعة المؤقت: {}", e))?;

    let user_data_arg = format!("--user-data-dir={}", profile_dir.display());
    let app_arg = format!("--app={}", url);

    std::process::Command::new(browser)
        .arg("--kiosk-printing")
        .arg("--disable-print-preview")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg(user_data_arg)
        .arg(app_arg)
        .spawn()
        .map_err(|e| format!("فشل إرسال الإيصال للطابعة: {}", e))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn print_receipt_to_default_printer(_meta: &PosReceiptMeta, _lines: &[PosReceiptLine], _filename: &str) -> Result<(), String> {
    Err("الطباعة المباشرة مدعومة حالياً على Windows فقط.".to_string())
}

fn file_url(path: &std::path::Path) -> Result<String, String> {
    let raw = path
        .to_str()
        .ok_or("مسار ملف الطباعة يحتوي على أحرف غير مدعومة.".to_string())?
        .replace('\\', "/")
        .replace(' ', "%20")
        .replace('#', "%23");
    Ok(format!("file:///{}", raw))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn money(value: f64) -> String {
    format!("{:.2}", value)
}

fn receipt_print_html(meta: &PosReceiptMeta, lines: &[PosReceiptLine]) -> String {
    let rows = lines
        .iter()
        .map(|line| {
            format!(
                "<tr><td class=\"item\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num total-cell\">{}</td></tr>",
                html_escape(&line.name),
                money(line.qty),
                money(line.price),
                money(line.qty * line.price)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let total = lines.iter().map(|line| line.qty * line.price).sum::<f64>();
    let draft = if meta.is_draft { "<div class=\"draft\">مسودة</div>" } else { "" };

    format!(
        r#"<!doctype html>
<html lang="ar" dir="rtl">
<head>
<meta charset="utf-8">
<title>receipt</title>
<style>
@page {{ size: 72.1mm 297mm; margin: 0; }}
* {{ box-sizing: border-box; -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
html, body {{ margin: 0; padding: 0; background: #fff; color: #000; overflow: hidden; }}
body {{ width: 72.1mm; max-width: 72.1mm; padding: 1.2mm 1.05mm; font-family: Arial, Tahoma, sans-serif; font-size: 11.8px; line-height: 1.28; font-weight: 600; }}
.receipt {{ width: 70mm; max-width: 70mm; margin: 0 auto; overflow: hidden; }}
.center {{ text-align: center; }}
.company {{ font-size: 13.8px; font-weight: 700; margin-bottom: 2px; overflow-wrap: anywhere; }}
.muted {{ font-size: 10.9px; }}
.draft {{ margin: 4px 0; padding: 2px; border: 1px solid #000; text-align: center; font-weight: 700; }}
.sep {{ border-top: 0.3mm dashed #000; margin: 4px 0; }}
.meta {{ display: grid; gap: 2px; margin: 4px 0; overflow-wrap: anywhere; }}
table {{ width: 100%; max-width: 100%; margin: 0; border-collapse: separate; border-spacing: 0; table-layout: fixed; direction: rtl; }}
col.item-col {{ width: 39%; }}
col.qty-col {{ width: 12%; }}
col.price-col {{ width: 22%; }}
col.total-col {{ width: 27%; }}
th, td {{ border-right: 0.25mm solid #000; border-bottom: 0.25mm solid #000; padding: 2.6px 0.8px; vertical-align: middle; min-width: 0; overflow: hidden; }}
tr > :first-child {{ border-right: 0.25mm solid #000; }}
tr > :last-child {{ border-left: 0.25mm solid #000; }}
thead tr:first-child > * {{ border-top: 0.25mm solid #000; }}
th {{ font-size: 9.7px; font-weight: 700; text-align: center; white-space: nowrap; }}
td {{ font-size: 10.8px; font-weight: 700; }}
.item {{ text-align: right; overflow-wrap: anywhere; word-break: break-word; hyphens: auto; }}
.num {{ direction: ltr; unicode-bidi: isolate; text-align: center; white-space: nowrap; font-family: Arial, sans-serif; letter-spacing: 0; font-weight: 600; }}
.total-cell {{ font-weight: 700; }}
.grand-label {{ text-align: right; font-size: 13.8px; font-weight: 700; }}
.grand-total {{ direction: ltr; unicode-bidi: isolate; text-align: center; font-size: 13.8px; font-weight: 700; font-family: Arial, sans-serif; }}
@media screen {{ body {{ padding: 4mm 1.05mm; }} }}
</style>
<script>
function fitReceipt() {{
  const table = document.querySelector('table');
  let size = 8;
  while (table && table.scrollWidth > document.body.clientWidth && size > 6.5) {{
    size -= 0.5;
    document.querySelectorAll('td, th').forEach((el) => el.style.fontSize = size + 'px');
  }}
}}
window.addEventListener('load', () => {{
  fitReceipt();
  setTimeout(() => window.print(), 450);
  setTimeout(() => window.close(), 2500);
}});
</script>
</head>
<body>
<div class="receipt">
  <div class="center company">{company}</div>
  <div class="center muted">{address}</div>
  <div class="center muted">{phone}</div>
  {draft}
  <div class="sep"></div>
  <div class="meta">
    <div>رقم الإيصال: {invoice_no}</div>
    <div>التاريخ: {invoice_time}</div>
    <div>العميل: {customer_name}</div>
  </div>
  <div class="sep"></div>
  <table>
    <colgroup>
      <col class="item-col">
      <col class="qty-col">
      <col class="price-col">
      <col class="total-col">
    </colgroup>
    <thead>
      <tr>
        <th class="item">الصنف</th>
        <th>كم</th>
        <th>سعر</th>
        <th>إج.</th>
      </tr>
    </thead>
    <tbody>{rows}</tbody>
    <tfoot>
      <tr>
        <td class="grand-label" colspan="3">الإجمالي</td>
        <td class="grand-total">{total}</td>
      </tr>
    </tfoot>
  </table>
  <div class="sep"></div>
  <div class="center muted">شكراً لزيارتكم</div>
</div>
</body>
</html>"#,
        company = html_escape(&meta.company_name),
        address = html_escape(&meta.address),
        phone = html_escape(&meta.phone),
        draft = draft,
        invoice_no = html_escape(&meta.invoice_no),
        invoice_time = html_escape(&meta.invoice_time),
        customer_name = html_escape(&meta.customer_name),
        rows = rows,
        total = money(total),
    )
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
