//! محوّلات ERP — استعلامات مشتركة الشكل لـ Marketing2026 و InfinityRetailDB

use crate::erp_profile::{self, ErpKind};
use crate::{execute_sql_query, BusinessProfile, QueryResult, SqlConnection};

const INFINITY_BRANCH_PROFILE: &str = r#"SELECT TOP 1
    NULLIF(LTRIM(RTRIM(b.BranchName)), N'') AS A_NAME,
    NULLIF(LTRIM(RTRIM(b.BranchAddressLine1)), N'') AS A_ADDRESS,
    NULLIF(LTRIM(RTRIM(b.BranchAddressLine3)), N'') AS CITY,
    CAST(b.BranchTypeID_FK AS nvarchar(20)) AS ACTIVITY,
    NULLIF(LTRIM(RTRIM(b.BranchName)), N'') AS ACTIVITYName,
    NULLIF(LTRIM(RTRIM(b.BranchPhone)), N'') AS PHONE,
    NULLIF(LTRIM(RTRIM(b.BranchEmailAddress)), N'') AS MOBILE,
    NULL AS FAX,
    CAST(b.BranchID_PK AS nvarchar(20)) AS TRAN_BARNCH,
    NULLIF(LTRIM(RTRIM(b.BranchEmailAddress)), N'') AS SMS_PHONE
FROM MyCompany.Config_Branchs b
ORDER BY CASE WHEN b.IsCurrentBranch = 1 THEN 0 ELSE 1 END, b.BranchID_PK"#;

const INFINITY_BRANCH_PROFILE_ALT: &str = r#"SELECT TOP 1
    NULLIF(LTRIM(RTRIM(b.BranchName)), N'') AS A_NAME,
    NULLIF(LTRIM(RTRIM(b.BranchAddressLine1)), N'') AS A_ADDRESS,
    NULLIF(LTRIM(RTRIM(b.BranchAddressLine3)), N'') AS CITY,
    CAST(b.BranchTypeID_FK AS nvarchar(20)) AS ACTIVITY,
    NULLIF(LTRIM(RTRIM(b.BranchName)), N'') AS ACTIVITYName,
    NULLIF(LTRIM(RTRIM(b.BranchPhone)), N'') AS PHONE,
    NULLIF(LTRIM(RTRIM(b.BranchEmailAddress)), N'') AS MOBILE,
    NULL AS FAX,
    CAST(b.BranchID_PK AS nvarchar(20)) AS TRAN_BARNCH,
    NULLIF(LTRIM(RTRIM(b.BranchEmailAddress)), N'') AS SMS_PHONE
FROM MyCompany.Config_View_Branchs b
ORDER BY CASE WHEN b.IsCurrentBranch = 1 THEN 0 ELSE 1 END, b.BranchID_PK"#;

const INFINITY_COMPANY_FALLBACK: &str = r#"SELECT TOP 1
    NULLIF(LTRIM(RTRIM(c.CompanyName)), N'') AS A_NAME,
    NULL AS A_ADDRESS,
    NULL AS CITY,
    CAST(c.CompanyId AS nvarchar(20)) AS ACTIVITY,
    NULLIF(LTRIM(RTRIM(c.CompanyName)), N'') AS ACTIVITYName,
    NULL AS PHONE,
    NULL AS MOBILE,
    NULL AS FAX,
    CAST(c.CompanyId AS nvarchar(20)) AS TRAN_BARNCH,
    NULL AS SMS_PHONE
FROM dbo.Companies c
ORDER BY c.CompanyId"#;

const INFINITY_RECEIPT_HEADER: &str = r#"SELECT TOP 1
    ISNULL(NULLIF(LTRIM(RTRIM(b.BranchName)), N''), N'Infinity Retail') AS company_name,
    ISNULL(NULLIF(LTRIM(RTRIM(b.BranchAddressLine1)), N''), N'') AS address,
    ISNULL(
        NULLIF(LTRIM(RTRIM(b.BranchPhone)), N''),
        ISNULL(NULLIF(LTRIM(RTRIM(b.BranchEmailAddress)), N''), N'')
    ) AS phone
FROM MyCompany.Config_Branchs b
ORDER BY CASE WHEN b.IsCurrentBranch = 1 THEN 0 ELSE 1 END, b.BranchID_PK"#;

pub struct ReceiptBusinessInfo {
    pub company_name: String,
    pub address: String,
    pub phone: String,
}

fn pick_column(columns: &[String], row: &[String], names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(i) = columns.iter().position(|c| c.eq_ignore_ascii_case(name)) {
            if let Some(v) = row.get(i) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

pub fn profile_from_query(result: &QueryResult, erp: ErpKind) -> BusinessProfile {
    let columns = &result.columns;
    let row = &result.rows[0];

    let mut profile = BusinessProfile {
        company_name: pick_column(columns, row, &["A_NAME"]),
        address: pick_column(columns, row, &["A_ADDRESS"]),
        city: pick_column(columns, row, &["CITY"]),
        activity_code: pick_column(columns, row, &["ACTIVITY"]),
        activity_name: pick_column(columns, row, &["ACTIVITYName", "ACTIVITYNAME"]),
        phone: pick_column(columns, row, &["PHONE"])
            .or_else(|| pick_column(columns, row, &["SMS_PHONE"])),
        mobile: pick_column(columns, row, &["MOBILE"]),
        fax: pick_column(columns, row, &["FAX"]),
        branch: pick_column(columns, row, &["TRAN_BARNCH"]),
        erp_kind: Some(erp.kind_id().to_string()),
        erp_label: Some(erp.display_name_ar().to_string()),
    };

    if profile.company_name.is_none() {
        profile.company_name = profile
            .activity_name
            .clone()
            .or_else(|| profile.activity_code.clone())
            .or_else(|| profile.branch.clone());
    }

    profile
}

pub fn empty_profile(erp: ErpKind) -> BusinessProfile {
    BusinessProfile {
        erp_kind: Some(erp.kind_id().to_string()),
        erp_label: Some(erp.display_name_ar().to_string()),
        ..BusinessProfile::default()
    }
}

async fn try_profile_query(
    conn: &SqlConnection,
    sql: &str,
    erp: ErpKind,
) -> Option<BusinessProfile> {
    match execute_sql_query(conn.clone(), sql.to_string()).await {
        Ok(result) if result.row_count > 0 => Some(profile_from_query(&result, erp)),
        _ => None,
    }
}

pub async fn fetch_business_profile(
    conn: &SqlConnection,
    erp: ErpKind,
) -> Result<BusinessProfile, String> {
    match erp {
        ErpKind::InfinityRetailDb => {
            for sql in [
                INFINITY_BRANCH_PROFILE,
                INFINITY_BRANCH_PROFILE_ALT,
                INFINITY_COMPANY_FALLBACK,
            ] {
                if let Some(profile) = try_profile_query(conn, sql, erp).await {
                    return Ok(profile);
                }
            }
            Ok(empty_profile(erp))
        }
        ErpKind::Marketing2026 | ErpKind::Unknown => {
            let full = erp_profile::business_profile_sql(erp);
            let basic = erp_profile::business_profile_basic_sql(erp);
            if let Some(p) = try_profile_query(conn, full, erp).await {
                return Ok(p);
            }
            if let Some(p) = try_profile_query(conn, basic, erp).await {
                return Ok(p);
            }
            Ok(empty_profile(erp))
        }
    }
}

pub async fn fetch_receipt_business(
    conn: &SqlConnection,
    erp: ErpKind,
) -> Result<ReceiptBusinessInfo, String> {
    match erp {
        ErpKind::InfinityRetailDb => {
            let result = execute_sql_query(conn.clone(), INFINITY_RECEIPT_HEADER.to_string())
                .await
                .map_err(|e| format!("خطأ في قراءة بيانات الفرع: {}", e))?;
            if result.row_count == 0 {
                return Ok(ReceiptBusinessInfo {
                    company_name: "Infinity Retail".to_string(),
                    address: String::new(),
                    phone: String::new(),
                });
            }
            let row = &result.rows[0];
            let cols = &result.columns;
            Ok(ReceiptBusinessInfo {
                company_name: pick_column(cols, row, &["company_name"])
                    .unwrap_or_else(|| "Infinity Retail".to_string()),
                address: pick_column(cols, row, &["address"]).unwrap_or_default(),
                phone: pick_column(cols, row, &["phone"]).unwrap_or_default(),
            })
        }
        _ => {
            let sql = r#"SELECT TOP 1
                COALESCE(
                    NULLIF(LTRIM(RTRIM(CAST(A_NAME AS nvarchar(200)))), N''),
                    NULLIF(LTRIM(RTRIM(CAST(ACTIVITYName AS nvarchar(200)))), N'')
                ) AS company_name,
                ISNULL(A_ADDRESS, N'') AS address,
                ISNULL(NULLIF(LTRIM(RTRIM(MOBILE)), N''), ISNULL(PHONE, N'')) AS phone
            FROM dbo.SITTEINGS"#;
            let result = execute_sql_query(conn.clone(), sql.to_string())
                .await
                .map_err(|e| format!("خطأ في قراءة SITTEINGS: {}", e))?;
            if result.row_count == 0 {
                return Err("لا توجد بيانات نشاط في SITTEINGS.".to_string());
            }
            let row = &result.rows[0];
            let cols = &result.columns;
            Ok(ReceiptBusinessInfo {
                company_name: pick_column(cols, row, &["company_name"]).unwrap_or_default(),
                address: pick_column(cols, row, &["address"]).unwrap_or_default(),
                phone: pick_column(cols, row, &["phone"]).unwrap_or_default(),
            })
        }
    }
}

pub fn infinity_product_search_sql(escaped: &str, top: u32) -> String {
    format!(
        "SET NOCOUNT ON;
SELECT TOP {top}
  CASE
    WHEN NULLIF(LTRIM(RTRIM(p.ProductCode)), N'') IS NOT NULL
      THEN p.ProductName + N' (' + p.ProductCode + N')'
    ELSE p.ProductName
  END AS label
FROM Inventory.Data_Products p
WHERE p.IsInActive = 0
  AND (p.ProductName LIKE N'%{escaped}%' OR p.ProductCode LIKE N'%{escaped}%')
ORDER BY p.ProductName"
    )
}

pub fn infinity_product_mentions_sql(escaped: &str, empty_query: bool) -> String {
    if empty_query {
        "SET NOCOUNT ON;
SELECT TOP 12 p.ProductName, ISNULL(p.ProductCode, N'') AS ProductCode
FROM Inventory.Data_Products p
WHERE p.IsInActive = 0
ORDER BY p.ModifiedDate DESC"
            .to_string()
    } else {
        format!(
            "SET NOCOUNT ON;
SELECT TOP 15 p.ProductName, ISNULL(p.ProductCode, N'') AS ProductCode
FROM Inventory.Data_Products p
WHERE p.IsInActive = 0
  AND (p.ProductName LIKE N'%{escaped}%' OR p.ProductCode LIKE N'%{escaped}%')
ORDER BY
  CASE WHEN p.ProductCode LIKE N'{escaped}%' THEN 0
       WHEN p.ProductName LIKE N'{escaped}%' THEN 1
       ELSE 2 END,
  p.ProductName"
        )
    }
}

pub fn infinity_pos_product_sql(where_clause: &str, order_by: &str, top: u32) -> String {
    format!(
        "SET NOCOUNT ON;
SELECT TOP {top}
  CAST(b.ProductBarcodeID_PK AS int) AS BAR_ID,
  CAST(p.ProductID_PK AS int) AS ITEM_ID,
  p.ProductName AS ITEM_NAME,
  ISNULL(p.ProductCode, N'') AS item_model,
  ISNULL(b.ProductBarcode, N'') AS barcode,
  CAST(ISNULL(b.UomID_FK, 0) AS int) AS unit_id,
  ISNULL(b.UOMName, N'') AS unit_desc,
  ISNULL(CAST(NULLIF(b.BaseUnitQYT, 0) AS float), 1) AS unit_qty,
  ISNULL(CAST(NULLIF(b.UomPrice1, 0) AS float), 0) AS price,
  ISNULL(CAST(b.UomLastCost AS float), 0) AS last_cost,
  ISNULL(CAST(b.UomLastCost AS float), 0) AS aver_cost,
  ISNULL(CAST(b.UomPrice1 AS float), 0) AS public_price,
  ISNULL(CAST(p.StockOnHand AS float), 0) AS stock_qty
FROM Inventory.Data_View_ProductUOMBarcodes b
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = b.ProductID_FK
WHERE p.IsInActive = 0
  AND ({where_clause})
ORDER BY {order_by};"
    )
}

pub fn cancelled_invoices_sql(erp: ErpKind, day: &str) -> String {
    match erp {
        ErpKind::InfinityRetailDb => format!(
            "SET NOCOUNT ON; SET DATEFORMAT ymd;
DECLARE @Day DATETIME;
SET @Day = '{day}';
SELECT
  inv.SalesInvoiceID_PK AS invoice_id,
  'sale' AS invoice_kind,
  N'مبيعات' AS invoice_kind_label,
  CONVERT(varchar(19), inv.SalesInvoiceDate, 120) AS invoice_time,
  CONVERT(varchar(19), inv.modifiedDate, 120) AS updated_at,
  ISNULL(NULLIF(LTRIM(RTRIM(c.CustomerName)), N''), N'—') AS party_name,
  ISNULL(inv.CreatedByUserName, N'—') AS employee_name,
  ISNULL(NULLIF(LTRIM(RTRIM(inv.Notes)), N''), N'—') AS note
FROM SALES.Data_SalesInvoices inv
INNER JOIN SALES.RefSalesInvoiceStates st ON st.SalesInvoiceStateID_PK = inv.SalesInvoiceStateID_FK
LEFT JOIN SALES.Data_Customers c ON c.CustomerID_PK = inv.CustomerID_FK
WHERE CONVERT(varchar(10), inv.SalesInvoiceDate, 120) = CONVERT(varchar(10), @Day, 120)
  AND (
    st.SalesInvoiceStateCaption LIKE N'%ملغ%'
    OR st.SalesInvoiceStateCaption LIKE N'%Cancel%'
    OR st.SalesInvoiceStateCaption LIKE N'%Void%'
    OR st.SalesInvoiceStateCaption LIKE N'%محذ%'
  )
UNION ALL
SELECT
  inv.InvoiceID_PK,
  'purchase',
  N'مشتريات',
  CONVERT(varchar(19), inv.InvoiceDate, 120),
  CONVERT(varchar(19), inv.modifiedDate, 120),
  ISNULL(NULLIF(LTRIM(RTRIM(s.SupplierName)), N''), N'—'),
  ISNULL(inv.CreatedByUserName, N'—'),
  ISNULL(NULLIF(LTRIM(RTRIM(inv.Notes)), N''), N'—')
FROM Purchase.Data_PurchaseInvoices inv
INNER JOIN Purchase.RefPurchaseInvoiceStates st ON st.PurchaseInvoiceStateID_PK = inv.PurchaseInvoiceStateID_PK
LEFT JOIN Purchase.Data_Suppliers s ON s.SupplierID_PK = inv.SupplierID_FK
WHERE CONVERT(varchar(10), inv.InvoiceDate, 120) = CONVERT(varchar(10), @Day, 120)
  AND (
    st.PurchaseInvoiceStateCaption LIKE N'%ملغ%'
    OR st.PurchaseInvoiceStateCaption LIKE N'%Cancel%'
    OR st.PurchaseInvoiceStateCaption LIKE N'%Void%'
    OR st.PurchaseInvoiceStateCaption LIKE N'%محذ%'
  )
ORDER BY invoice_time DESC;"
        ),
        ErpKind::Marketing2026 | ErpKind::Unknown => format!(
            "SET NOCOUNT ON; SET DATEFORMAT ymd;
DECLARE @Day DATETIME;
SET @Day = '{day}';
SELECT
  S.S_ID AS invoice_id,
  'sale' AS invoice_kind,
  N'مبيعات' AS invoice_kind_label,
  CONVERT(varchar(19), S.S_DATE, 120) AS invoice_time,
  CONVERT(varchar(19), S.S_UPDATE_DATE, 120) AS updated_at,
  ISNULL(NULLIF(LTRIM(RTRIM(S.CUST_NAME)), N''), N'—') AS party_name,
  ISNULL(U.FULL_NAME, N'—') AS employee_name,
  ISNULL(NULLIF(LTRIM(RTRIM(S.S_NOTE)), N''), N'—') AS note
FROM dbo.SALE_INVOICE S
LEFT JOIN dbo.USERS U ON S.USERS_ID = U.USERS_ID
WHERE CAST(S.S_STATUES AS int) = 2 AND CONVERT(varchar(10), S.S_DATE, 120) = CONVERT(varchar(10), @Day, 120)
UNION ALL
SELECT
  B.B_ID,
  'purchase',
  N'مشتريات',
  CONVERT(varchar(19), B.B_DATE, 120),
  CONVERT(varchar(19), B.B_UPDATE_DATE, 120),
  ISNULL(NULLIF(LTRIM(RTRIM(C.CUST_NAME)), N''), N'—'),
  ISNULL(U.FULL_NAME, N'—'),
  ISNULL(NULLIF(LTRIM(RTRIM(B.S_NOTE)), N''), N'—')
FROM dbo.BUY_INVOICE B
LEFT JOIN dbo.USERS U ON B.USERS_ID = U.USERS_ID
LEFT JOIN dbo.CUSTOMERS C ON B.CUST_ID = C.CUST_ID
WHERE CAST(B.B_STATUES AS int) = 2 AND CONVERT(varchar(10), B.B_DATE, 120) = CONVERT(varchar(10), @Day, 120)
ORDER BY invoice_time DESC;"
        ),
    }
}

fn escape_sql_term(term: &str) -> String {
    term.replace('\'', "''")
}

/// شروط البحث عن منتج (يدعم عدة قيم مفصولة بفاصلة)
pub fn build_product_search_condition(erp: ErpKind, search_term: &str) -> String {
    let parts: Vec<String> = search_term
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let e = escape_sql_term(t);
            match erp {
                ErpKind::InfinityRetailDb => format!(
                    "(p.ProductName LIKE N'%{e}%' OR p.ProductCode LIKE N'%{e}%' OR b.ProductBarcode LIKE N'%{e}%')"
                ),
                _ => format!(
                    "(I.ITEM_NAME LIKE N'%{e}%' OR I.ITEM_MODEL LIKE N'%{e}%' OR B.BARCODE LIKE N'%{e}%')"
                ),
            }
        })
        .collect();

    if parts.is_empty() {
        "1=1".to_string()
    } else {
        parts.join(" OR ")
    }
}

/// استعلام خفيف: هل المنتج/الباركود موجود في ERP النشط؟
pub fn product_probe_sql(erp: ErpKind, search_term: &str) -> String {
    let condition = build_product_search_condition(erp, search_term);
    match erp {
        ErpKind::InfinityRetailDb => format!(
            "SELECT TOP 1 \
             p.ProductCode AS [كود], \
             LEFT(p.ProductName, 80) AS [اسم], \
             ISNULL(b.ProductBarcode, N'') AS [باركود] \
             FROM Inventory.Data_Products p \
             LEFT JOIN Inventory.Data_View_ProductUOMBarcodes b ON b.ProductID_FK = p.ProductID_PK \
             WHERE p.IsInActive = 0 AND ({condition})"
        ),
        _ => format!(
            "SELECT TOP 1 \
             I.ITEM_MODEL AS [كود], \
             LEFT(I.ITEM_NAME, 80) AS [اسم], \
             ISNULL(B.BARCODE, N'') AS [باركود] \
             FROM dbo.ITEMS I \
             LEFT JOIN dbo.BARCODE B ON I.ITEM_ID = B.ITEM_ID \
             WHERE I.ITEM_INVISIBLE = 0 AND ({condition})"
        ),
    }
}

fn build_infinity_product_only_condition(search_term: &str) -> String {
    let parts: Vec<String> = search_term
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let e = escape_sql_term(t);
            format!("(p.ProductName LIKE N'%{e}%' OR p.ProductCode LIKE N'%{e}%')")
        })
        .collect();

    if parts.is_empty() {
        "1=1".to_string()
    } else {
        parts.join(" OR ")
    }
}

pub fn is_marketing_product_template(sql: &str) -> bool {
    sql.contains("dbo.ITEMS") && sql.contains("dbo.BARCODE")
}

pub fn is_marketing_supplier_template(sql: &str) -> bool {
    sql.contains("dbo.BUY_ITEMS") || sql.contains("best_supplier")
}

pub fn infinity_product_comprehensive_sql(condition: &str) -> String {
    format!(
        "SET NOCOUNT ON;
SELECT TOP 30
  ISNULL(p.ProductCode, N'') AS [الكود],
  LEFT(p.ProductName, 80) AS [اسم المنتج],
  ISNULL(b.UOMName, N'') AS [وحدة القياس],
  ISNULL(b.ProductBarcode, N'') AS [الباركود],
  CAST(ISNULL(b.UomPrice1, 0) AS decimal(18,3)) AS [السعر],
  CAST(ISNULL(b.UomPrice2, 0) AS decimal(18,3)) AS [سعر 2],
  CAST(ISNULL(b.UomPrice4, 0) AS decimal(18,3)) AS [سعر 4],
  CAST(ISNULL(b.UomLastCost, 0) AS decimal(18,3)) AS [آخر تكلفة],
  CAST(ISNULL(p.StockOnHand, 0) AS decimal(18,1)) AS [الكمية المتاحة],
  (
    SELECT TOP 1 ISNULL(s.SupplierName, N'')
    FROM Purchase.Data_PurchaseInvoiceItems pi
    INNER JOIN Purchase.Data_PurchaseInvoices inv ON pi.InvoiceID_FK = inv.InvoiceID_PK
    LEFT JOIN Purchase.Data_Suppliers s ON inv.SupplierID_FK = s.SupplierID_PK
    WHERE pi.ProductID_FK = p.ProductID_PK
    ORDER BY inv.InvoiceDate DESC
  ) AS [آخر مورد],
  CONVERT(varchar(10), b.ModifiedDate, 120) AS [تاريخ التعديل]
FROM Inventory.Data_Products p
INNER JOIN Inventory.Data_View_ProductUOMBarcodes b ON p.ProductID_PK = b.ProductID_FK
WHERE p.IsInActive = 0 AND ({condition})
ORDER BY p.ProductName, b.BaseUnitQYT"
    )
}

pub fn infinity_last_supplier_price_sql(search_term: &str) -> String {
    let condition = build_infinity_product_only_condition(search_term);
    format!(
        "SET NOCOUNT ON;
;WITH LastBuy AS (
  SELECT
    pi.ProductID_FK,
    pi.UnitCost AS LastCost,
    inv.InvoiceDate AS LastDate,
    s.SupplierName,
    ROW_NUMBER() OVER (PARTITION BY pi.ProductID_FK ORDER BY inv.InvoiceDate DESC) AS rn
  FROM Purchase.Data_PurchaseInvoiceItems pi
  INNER JOIN Purchase.Data_PurchaseInvoices inv ON inv.InvoiceID_PK = pi.InvoiceID_FK
  LEFT JOIN Purchase.Data_Suppliers s ON s.SupplierID_PK = inv.SupplierID_FK
)
SELECT TOP 50
  ISNULL(p.ProductCode, N'') AS [الكود],
  p.ProductName AS [اسم المنتج],
  ISNULL(lb.SupplierName, N'—') AS [المورد],
  CAST(lb.LastCost AS decimal(18,3)) AS [آخر سعر شراء],
  CONVERT(varchar(10), lb.LastDate, 120) AS [تاريخ الشراء]
FROM LastBuy lb
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = lb.ProductID_FK
WHERE lb.rn = 1 AND p.IsInActive = 0 AND ({condition})
ORDER BY lb.LastDate DESC"
    )
}

/// يبني SQL تقرير Supabase النهائي (ERP-aware)
pub fn finalize_supabase_report_sql(
    erp: ErpKind,
    sql_template: &str,
    search_term: &str,
    target_date: &str,
) -> String {
    let mut final_sql = resolve_search_report_sql(erp, sql_template, search_term);
    final_sql = final_sql.replace("{{TARGET_DATE}}", target_date);
    final_sql
}

/// يبني SQL التقرير النهائي حسب نوع ERP
pub fn resolve_search_report_sql(erp: ErpKind, sql_template: &str, search_term: &str) -> String {
    let condition = build_product_search_condition(erp, search_term);
    let escaped = escape_sql_term(search_term);

    if erp == ErpKind::InfinityRetailDb {
        if is_marketing_product_template(sql_template) {
            return infinity_product_comprehensive_sql(&condition);
        }
        if is_marketing_supplier_template(sql_template) {
            return infinity_last_supplier_price_sql(search_term);
        }
    }

    let mut final_sql = sql_template.replace("{{DAYS_RECENT}}", "60");
    final_sql = final_sql.replace("{{DAYS_TOTAL}}", "180");
    final_sql = final_sql.replace("{{SEARCH_CONDITION}}", &condition);
    final_sql = final_sql.replace("{{SEARCH_TERM}}", &escaped);
    final_sql = final_sql.replace("{{PRODUCTS_LIST}}", &format!("N'{escaped}'"));
    final_sql
}
