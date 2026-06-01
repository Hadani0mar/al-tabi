//! اكتشاف نوع ERP وتحميل ملف تعاليمات/أنماط الوكيل المناسب.

use crate::{execute_sql_query, AppState, SqlConnection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErpKind {
    Marketing2026,
    InfinityRetailDb,
    Unknown,
}

impl ErpKind {
    pub fn display_name_ar(self) -> &'static str {
        match self {
            ErpKind::Marketing2026 => "Marketing2026",
            ErpKind::InfinityRetailDb => "InfinityRetailDB",
            ErpKind::Unknown => "غير معروف",
        }
    }

    pub fn agent_file_label(self) -> &'static str {
        match self {
            ErpKind::Marketing2026 => "AGENT_Marketing2026.md",
            ErpKind::InfinityRetailDb => "AGENT_InfinityRetailDB.md",
            ErpKind::Unknown => "AGENT_Marketing2026.md",
        }
    }

    pub fn kind_id(self) -> &'static str {
        match self {
            ErpKind::Marketing2026 => "marketing2026",
            ErpKind::InfinityRetailDb => "infinity_retail_db",
            ErpKind::Unknown => "unknown",
        }
    }

    pub fn from_database_name(db: &str) -> Self {
        let d = db.to_lowercase();
        if d.contains("infinity")
            || d.contains("infinit")
            || d.contains("infinityretail")
            || d == "infinityretaildb"
        {
            return ErpKind::InfinityRetailDb;
        }
        if d.contains("marketing") {
            return ErpKind::Marketing2026;
        }
        ErpKind::Unknown
    }
}

static AGENT_MARKETING_CACHE: OnceLock<String> = OnceLock::new();
static AGENT_INFINITY_CACHE: OnceLock<String> = OnceLock::new();

const AGENT_MARKETING_EMBED: &str = include_str!("../../AGENT_Marketing2026.md");
const AGENT_INFINITY_EMBED: &str = include_str!("../../AGENT_InfinityRetailDB.md");
const QUERY_PATTERNS_LEGACY_EMBED: &str = include_str!("../../QUERY_PATTERNS.md");

const INFINITY_PROBE: &str = r#"
SELECT TOP 1 1 AS ok
FROM sys.tables t
INNER JOIN sys.schemas s ON t.schema_id = s.schema_id
WHERE s.name = N'Inventory' AND t.name = N'Data_Products'
"#;

const INFINITY_BRANCH_PROBE: &str = r#"
SELECT TOP 1 1 AS ok
FROM sys.tables t
INNER JOIN sys.schemas s ON t.schema_id = s.schema_id
WHERE s.name = N'MyCompany' AND t.name = N'Config_Branchs'
"#;

const MARKETING_PROBE: &str = r#"
SELECT TOP 1 1 AS ok
FROM INFORMATION_SCHEMA.TABLES
WHERE TABLE_SCHEMA = N'dbo' AND TABLE_NAME = N'ITEMS'
"#;

/// يكتشف نوع ERP من المخطط (أولاً) ثم اسم قاعدة البيانات.
pub async fn detect_erp_kind(conn: &SqlConnection) -> ErpKind {
    if let Ok(result) = execute_sql_query(conn.clone(), INFINITY_PROBE.to_string()).await {
        if result.row_count > 0 {
            return ErpKind::InfinityRetailDb;
        }
    }

    if let Ok(result) = execute_sql_query(conn.clone(), INFINITY_BRANCH_PROBE.to_string()).await {
        if result.row_count > 0 {
            return ErpKind::InfinityRetailDb;
        }
    }

    let db_hint = ErpKind::from_database_name(&conn.database);
    if db_hint == ErpKind::InfinityRetailDb {
        return ErpKind::InfinityRetailDb;
    }

    if let Ok(result) = execute_sql_query(conn.clone(), MARKETING_PROBE.to_string()).await {
        if result.row_count > 0 {
            return ErpKind::Marketing2026;
        }
    }

    if db_hint != ErpKind::Unknown {
        return db_hint;
    }
    ErpKind::Unknown
}

pub async fn resolve_erp_kind(app_state: &AppState, conn: &SqlConnection) -> ErpKind {
    let kind = detect_erp_kind(conn).await;
    *app_state.erp_kind.lock().await = Some(kind);
    kind
}

pub async fn current_erp_kind(app_state: &AppState) -> ErpKind {
    let conn_opt = app_state.conn.lock().await.clone();
    if let Some(conn) = conn_opt {
        return resolve_erp_kind(app_state, &conn).await;
    }
    ErpKind::Unknown
}

fn candidate_paths(file_name: &str) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();

    vec![
        exe_dir.join(file_name),
        exe_dir.join("..").join(file_name),
        exe_dir.join("../..").join(file_name),
        exe_dir.join("../../..").join(file_name),
        exe_dir.join("../../../..").join(file_name),
        PathBuf::from(r"C:\Users\DELL\Desktop\al-tabi\reports-app").join(file_name),
    ]
}

fn load_file_from_disk(file_name: &str) -> Option<String> {
    for path in candidate_paths(file_name) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("## PATTERN:") {
                return Some(content);
            }
        }
    }
    None
}

fn load_marketing_agent_content() -> String {
    AGENT_MARKETING_CACHE
        .get_or_init(|| {
            load_file_from_disk("AGENT_Marketing2026.md")
                .or_else(|| load_file_from_disk("QUERY_PATTERNS.md"))
                .unwrap_or_else(|| {
                    if AGENT_MARKETING_EMBED.contains("## PATTERN:") {
                        AGENT_MARKETING_EMBED.to_string()
                    } else if QUERY_PATTERNS_LEGACY_EMBED.contains("## PATTERN:") {
                        QUERY_PATTERNS_LEGACY_EMBED.to_string()
                    } else {
                        "# AGENT_Marketing2026 not found\n".to_string()
                    }
                })
        })
        .clone()
}

fn load_infinity_agent_content() -> String {
    AGENT_INFINITY_CACHE
        .get_or_init(|| {
            load_file_from_disk("AGENT_InfinityRetailDB.md").unwrap_or_else(|| {
                if AGENT_INFINITY_EMBED.contains("## PATTERN:") {
                    AGENT_INFINITY_EMBED.to_string()
                } else {
                    "# AGENT_InfinityRetailDB not found\n".to_string()
                }
            })
        })
        .clone()
}

pub fn load_agent_patterns(erp: ErpKind) -> String {
    match erp {
        ErpKind::InfinityRetailDb => load_infinity_agent_content(),
        ErpKind::Marketing2026 | ErpKind::Unknown => load_marketing_agent_content(),
    }
}

pub fn domain_critical_facts(erp: ErpKind) -> &'static str {
    match erp {
        ErpKind::InfinityRetailDb => INFINITY_DOMAIN_CRITICAL_FACTS,
        ErpKind::Marketing2026 | ErpKind::Unknown => MARKETING_DOMAIN_CRITICAL_FACTS,
    }
}

pub const MARKETING_DOMAIN_CRITICAL_FACTS: &str = r#"<DOMAIN_CRITICAL_FACTS>
### ERP: Marketing2026 (dbo schema)
- **ITEMS**: ITEM_ID, ITEM_MODEL, ITEM_NAME, LAST_COST, AVER_COST, MIN_LEVEL, MAX_LEVEL
- **ITEMS_SUB**: ITEM_ID, STORE_ID, QTY, CATEOGRY1=Batch, CATEOGRY3=Expiry ⭐
- **SALE_INVOICE**: S_ID, S_DATE ⭐, CUST_ID, USERS_ID | **SALE_ITEMS**: S_ID→SALE_INVOICE (⚠️ لا S_DATE)
- **BUY_INVOICE/BUY_ITEMS**: B_DATE, CUST_ID=مورد, ITEM_ID, QTY, PRICE
- **CUSTOMERS**: CUST_ID, CUST_NAME, CUST_VENDOR, CUST_CUSTOM | **USERS**: USERS_ID, FULL_NAME
- **View**: dbo.SALE_ITEMS_INVOICE_VIEW | **BALANCE_C**: ⚠️ فارغ — لا للديون
- بحث منتج: LIKE N'%name%' | @AsOfDate = MAX(S_DATE)
</DOMAIN_CRITICAL_FACTS>"#;

pub const INFINITY_DOMAIN_CRITICAL_FACTS: &str = r#"<DOMAIN_CRITICAL_FACTS>
### ERP: InfinityRetailDB (schemas: Inventory, SALES, Purchase, MyCompany)
- **Inventory.Data_Products**: ProductID_PK, ProductCode, ProductName, SalesDecription, StockOnHand, MinStockLevel, MaxStockLevel, IsInActive
- **Inventory.Data_ProductInventories**: ProductID_FK, BranchID_FK, StockOnHand, ExpiryDate ⭐
- **Inventory.Data_ProductUOMs**: UomPrice1..4, UomLastCost, BaseUnitQYT | **Data_ProductBarcodes**: ProductBarcode
- **View**: Inventory.Data_View_ProductUOMBarcodes (باركود + سعر + وحدة)
- **SALES.Data_SalesInvoices**: SalesInvoiceID_PK, SalesInvoiceDate ⭐, CustomerID_FK, SalesInvoiceStateID_FK, CreatedByUserName
- **SALES.Data_SalesInvoiceItems**: SalesInvoiceID_FK, ProductID_FK, QYT, UnitPrice, ExpireDate
- **View**: SALES.Data_View_SalesInvoiceItems (ProductName, UOMName, UnitPrice, QYT)
- **SALES.Data_Customers**: CustomerID_PK, CustomerName, CustomerOutstanding, IsAllowCreditSales
- **Purchase.Data_Suppliers** + **Purchase.Data_PurchaseInvoices/Items** للموردين والشراء
- **MyCompany.Config_Branchs**: BranchID_PK, BranchName
- بحث منتج: ProductName/ProductCode LIKE N'%...%' | تاريخ مرجعي = MAX(SalesInvoiceDate)
- ⚠️ لا تستخدم جداول Marketing2026 (ITEMS, SALE_INVOICE...) على InfinityRetailDB
</DOMAIN_CRITICAL_FACTS>"#;

pub async fn refresh_erp_kind(app_state: &Arc<AppState>, conn: &SqlConnection) -> ErpKind {
    let kind = detect_erp_kind(conn).await;
    *app_state.erp_kind.lock().await = Some(kind);
    kind
}

/// بيانات النشاط — Marketing2026 من dbo.SITTEINGS
pub const MARKETING_BUSINESS_PROFILE_SQL: &str = r#"SELECT TOP 1
    COALESCE(
        NULLIF(LTRIM(RTRIM(CAST(A_NAME AS nvarchar(200)))), N''),
        NULLIF(LTRIM(RTRIM(CAST(ACTIVITYName AS nvarchar(200)))), N''),
        NULLIF(LTRIM(RTRIM(CAST(ACTIVITY AS nvarchar(200)))), N'')
    ) AS A_NAME,
    NULLIF(LTRIM(RTRIM(CAST(A_ADDRESS AS nvarchar(500)))), N'') AS A_ADDRESS,
    NULLIF(LTRIM(RTRIM(CAST(CITY AS nvarchar(100)))), N'') AS CITY,
    NULLIF(LTRIM(RTRIM(CAST(ACTIVITY AS nvarchar(100)))), N'') AS ACTIVITY,
    NULLIF(LTRIM(RTRIM(CAST(ACTIVITYName AS nvarchar(200)))), N'') AS ACTIVITYName,
    NULLIF(LTRIM(RTRIM(CAST(PHONE AS nvarchar(50)))), N'') AS PHONE,
    NULLIF(LTRIM(RTRIM(CAST(MOBILE AS nvarchar(50)))), N'') AS MOBILE,
    NULLIF(LTRIM(RTRIM(CAST(FAX AS nvarchar(50)))), N'') AS FAX,
    NULLIF(LTRIM(RTRIM(CAST(TRAN_BARNCH AS nvarchar(50)))), N'') AS TRAN_BARNCH,
    NULLIF(LTRIM(RTRIM(CAST(SMS_PHONE AS nvarchar(50)))), N'') AS SMS_PHONE
FROM dbo.SITTEINGS"#;

pub const MARKETING_BUSINESS_PROFILE_BASIC_SQL: &str = r#"SELECT TOP 1
    COALESCE(
        NULLIF(LTRIM(RTRIM(CAST(A_NAME AS nvarchar(200)))), N''),
        NULLIF(LTRIM(RTRIM(CAST(SMS_PHONE AS nvarchar(200)))), N'')
    ) AS A_NAME,
    NULLIF(LTRIM(RTRIM(CAST(A_ADDRESS AS nvarchar(500)))), N'') AS A_ADDRESS,
    NULLIF(LTRIM(RTRIM(CAST(PHONE AS nvarchar(50)))), N'') AS PHONE,
    NULLIF(LTRIM(RTRIM(CAST(MOBILE AS nvarchar(50)))), N'') AS MOBILE,
    NULLIF(LTRIM(RTRIM(CAST(FAX AS nvarchar(50)))), N'') AS FAX,
    NULLIF(LTRIM(RTRIM(CAST(TRAN_BARNCH AS nvarchar(50)))), N'') AS TRAN_BARNCH,
    NULLIF(LTRIM(RTRIM(CAST(SMS_PHONE AS nvarchar(50)))), N'') AS SMS_PHONE
FROM dbo.SITTEINGS"#;

/// بيانات النشاط — InfinityRetailDB من الفرع الحالي (MyCompany.Config_Branchs)
pub const INFINITY_BUSINESS_PROFILE_SQL: &str = r#"SELECT TOP 1
    NULLIF(LTRIM(RTRIM(b.BranchName)), N'') AS A_NAME,
    NULLIF(LTRIM(RTRIM(CONCAT(
        ISNULL(NULLIF(LTRIM(RTRIM(b.BranchAddressLine1)), N''), N''),
        CASE WHEN NULLIF(LTRIM(RTRIM(b.BranchAddressLine2)), N'') IS NOT NULL
             THEN N' — ' + LTRIM(RTRIM(b.BranchAddressLine2)) ELSE N'' END,
        CASE WHEN NULLIF(LTRIM(RTRIM(b.BranchAddressLine3)), N'') IS NOT NULL
             THEN N' — ' + LTRIM(RTRIM(b.BranchAddressLine3)) ELSE N'' END
    ))), N'') AS A_ADDRESS,
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

pub const INFINITY_BUSINESS_PROFILE_BASIC_SQL: &str = r#"SELECT TOP 1
    b.BranchName AS A_NAME,
    NULLIF(LTRIM(RTRIM(b.BranchAddressLine1)), N'') AS A_ADDRESS,
    NULLIF(LTRIM(RTRIM(b.BranchAddressLine3)), N'') AS CITY,
    CAST(b.BranchTypeID_FK AS nvarchar(20)) AS ACTIVITY,
    b.BranchName AS ACTIVITYName,
    NULLIF(LTRIM(RTRIM(b.BranchPhone)), N'') AS PHONE,
    NULL AS MOBILE,
    NULL AS FAX,
    CAST(b.BranchID_PK AS nvarchar(20)) AS TRAN_BARNCH,
    NULL AS SMS_PHONE
FROM MyCompany.Config_Branchs b
ORDER BY CASE WHEN b.IsCurrentBranch = 1 THEN 0 ELSE 1 END, b.BranchID_PK"#;

pub fn business_profile_sql(erp: ErpKind) -> &'static str {
    match erp {
        ErpKind::InfinityRetailDb => INFINITY_BUSINESS_PROFILE_SQL,
        ErpKind::Marketing2026 | ErpKind::Unknown => MARKETING_BUSINESS_PROFILE_SQL,
    }
}

pub fn business_profile_basic_sql(erp: ErpKind) -> &'static str {
    match erp {
        ErpKind::InfinityRetailDb => INFINITY_BUSINESS_PROFILE_BASIC_SQL,
        ErpKind::Marketing2026 | ErpKind::Unknown => MARKETING_BUSINESS_PROFILE_BASIC_SQL,
    }
}
