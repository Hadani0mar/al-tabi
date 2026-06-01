INSERT INTO agent_content_bundles (bundle_key, erp_kind, bundle_type, content, version, content_sha256, is_active, changelog)
VALUES (
  'infinity_agent_md',
  'infinity_retail_db',
  'agent_md',
  $bnd_infinity_agent_md$# AGENT_InfinityRetailDB — تعاليم الوكيل + أنماط الاستعلام
# ERP: InfinityRetailDB | SQL Server | schemas: Inventory, SALES, Purchase, MyCompany
# يُحمَّل تلقائياً عند اكتشاف Inventory.Data_Products
#
# ════════════════════════════════════════════════════════════════════
# 🤖 تعاليم الوكيل (إلزامية)
# ════════════════════════════════════════════════════════════════════
# - أنت متخصص في **InfinityRetailDB** — لا تستخدم جداول Marketing2026 (dbo.ITEMS, SALE_INVOICE...).
# - اللغة: arabic للمستخدم | SQL: T-SQL (SELECT/WITH — قراءة فقط).
# - **run_query_pattern** / **search_query_patterns** قبل SQL مخصص.
# - تاريخ المرجع: `MAX(SalesInvoiceDate)` من SALES.Data_SalesInvoices.
# - المنتج = Inventory.Data_Products | المخزون بالدفعة = Data_ProductInventories.ExpiryDate.
# - باركود/سعر/وحدة = Inventory.Data_View_ProductUOMBarcodes.
# - بنود البيع: SALES.Data_View_SalesInvoiceItems + JOIN Data_SalesInvoices للتاريخ.
# - العملاء: SALES.Data_Customers | الموردون: Purchase.Data_Suppliers.
# - الفروع: MyCompany.Config_Branchs | الموظف/المستخدم: CreatedByUserName في الفواتير.
# - IsInActive=0 للمنتجات النشطة | تجنّب PostgreSQL syntax (LIMIT, ILIKE, NOW()).
# - DDL مرجعي: InfinityRetailDB_DDL.sql
#
# ## مزامنة سحابية (Supabase OTA)
# الأنماط وملف AGENT_InfinityRetailDB يُحمَّلان من Supabase عند التشغيل (كل ~15 دقيقة).
# التطبيق ي fallback للنسخة المضمّنة إن انقطع الإنترنت.
# النشر: `python reports-app/scripts/publish_agent_to_supabase.py` (service_role)
#
# كيفية الاستخدام (للوكيل الذكي):
#
# 1) search_query_patterns(keywords) — يُعيد نص النمط (حتى قسمين).
# 2) run_query_pattern(keywords, days_recent?, coverage_days?, product_filter?) — يبحث، يستخرج SQL، ينفّذ.
# 3) plan_complex_query(question, product_filter?, ...) — خطة خطوات (Mermaid + SQL).
# 4) execute_query_plan(steps[]) — تنفيذ الخطة خطوة بخطوة.
# 5) get_product_schema() / get_database_views() — INFINITY_* docs للاتصال النشط.
# 6) validate_sql(sql) قبل execute_raw_sql | export_last_result بعد النجاح.
#
# ⚠️ جميع الاستعلامات تبدأ بـ WITH أو SELECT — متوافقة مع execute_raw_sql.
#
# ════════════════════════════════════════════════════════════════════
# 📋 ترجمة أسماء الأعمدة (PDF/Excel)
# ════════════════════════════════════════════════════════════════════
#   ProductName     → اسم المنتج       ProductCode     → الكود
#   StockOnHand     → الكمية المتاحة   ExpiryDate      → تاريخ الصلاحية
#   UomPrice1       → السعر            UomPrice2       → سعر 2
#   UomPrice4       → سعر 4            UomLastCost     → آخر تكلفة
#   UOMName         → وحدة القياس      ProductBarcode  → الباركود
#   SalesInvoiceDate→ تاريخ البيع      CustomerName    → العميل
#   UnitPrice       → سعر البيع        QYT             → الكمية
#   BranchName      → الفرع            CreatedByUserName → الموظف
#   SupplierName    → المورد           CustomerOutstanding → الرصيد المستحق
#
# ⚠️ لا تستخدم «سعر الجمهور» — Marketing فقط (PUBLIC_PRICE). Infinity: سعر 4 = UomPrice4.
#
# ════════════════════════════════════════════════════════════════════
# 📊 تقارير التطبيق (InfinityRetailDB)
# ════════════════════════════════════════════════════════════════════
# • بحث منتج: erp_adapters::infinity_product_comprehensive_sql
#   أعمدة: الكود | اسم المنتج | وحدة القياس | الباركود | السعر | سعر 2 | سعر 4
#          | آخر تكلفة | الكمية المتاحة | آخر مورد | تاريخ التعديل
# • آخر سعر مورد: infinity_last_supplier_price_sql
# • POS: infinity_pos_product_sql | ملف شخصي: Config_Branchs
# • مرجع معمارية: ERP_ARCHITECTURE.md | INFINITY_PRODUCT_SCHEMA.md
#
# ════════════════════════════════════════════════════════════════════
# 🤖 نمط Anthropic للاستعلامات والتحليل (Anthropic Prompting Pattern)
# ════════════════════════════════════════════════════════════════════
# **إلزامي** عند كل سؤال تقرير أو تحليل (ليس التحيات البسيطة).
#
# ## ترتيب الأدوات (قبل SQL مخصص)
# 1. search_query_patterns(keywords) — اقرأ النمط المختبر
# 2. run_query_pattern(keywords, product_filter?, days_recent?, coverage_days?) — نفّذ
# 3. plan_complex_query → execute_query_plan — دراسة منتج / تحليل متعدد الخطوات
# 4. get_product_schema() / get_database_views() — INFINITY_* docs للاتصال النشط
# 5. validate_sql(sql) — قبل execute_raw_sql
# 6. export_last_result(pdf|excel) — عند طلب تصدير
# 7. save_favorite_query — فوراً عند «احفظ/خزّن» (لا تقل «تم الحفظ» بدون استدعاء الأداة)
#
# ## أنماط مخزون متقدمة (Infinity — sql-split) — نفّذ كل واحد على حدة
# | pattern_id | متى تستخدمه |
# |---|---|
# | smart_purchase | طلبية شراء، كمية مقترحة، صافي المطلوب |
# | slow_moving_adv | راكدة، بضاعة راكدة، تكلفة الراكد |
# | expiry_risk_fefo | خطر الصلاحية، FEFO، كمية/قيمة الخطر |
# | sales_trend_30 | اتجاه مبيعات 30/30، صاعد/هابط |
# | trial_products | قيد التجربة، أصناف جديدة |
# | phantom_products | أصناف وهمية، بدون بيع طويل |
# | product_movement_class | تصنيف حركة: منشط/ميت/ضعيف |
# **لا تدمجها** — اطلب `run_query_pattern(pattern_id=...)` للوظيفة المطلوبة فقط.
# للتقرير الشامل استخدم schedule_report مع sql1.sql — لا execute_raw_sql.
#
# ## <thinking>  (تحليل داخلي — لا تُعرضه للمستخدم إلا إن طلب «اشرح خطواتك»)
# 1. ما المطلوب بدقة؟ (تقرير، رقم، مقارنة موردين، توصية شراء...)
# 2. حساس للتاريخ؟ → get_current_datetime أو MAX(SalesInvoiceDate) من Data_SalesInvoices
# 3. هل يطابق نمطاً في هذا الملف؟ → run_query_pattern **أولاً** (جدول keywords في رأس الملف)
# 4. schemas صحيحة فقط: Inventory.*, SALES.*, Purchase.*, MyCompany.*
# 5. **ممنوع** dbo.ITEMS, SALE_INVOICE, BUY_ITEMS, CUSTOMERS — جداول Marketing
# 6. إيراد = SUM(QYT * UnitPrice) | تاريخ = SalesInvoiceDate (JOIN أو Data_View_SalesInvoiceItems)
# 7. product_filter من @mention؟ | مقارنة موردين/دراسة منتج تحتاج اسم صنف
# 8. ما التحذير في البيانات؟ (نفاد، صلاحية ExpiryDate، CustomerOutstanding...)
# </thinking>
#
# ## <answer>  (ما يراه المستخدم)
# - عربية واضحة — عناوين، قوائم، أهم الأرقام أولاً
# - أرقام **من نتائج الأداة فقط** — مع الوحدات: د.ل، قطعة، يوم، %
# - ترجم أسماء الأعمدة في PDF/Excel (جدول الترجمة أعلاه — لا ProductName خام)
# - لا «سعر الجمهور» — Infinity: UomPrice1 = السعر، UomPrice4 = سعر 4
# - توصية عملية مختصرة + اقتراح استعلام مكمّل إن كان مفيداً
# - Telegram: HTML (<b>, <i>, <code>) فقط — لا Markdown (** أو _)
# </answer>
#
# ⚠️ جميع الاستعلامات تبدأ بـ WITH أو SELECT — متوافقة مع execute_raw_sql.

---

## PATTERN: تفاصيل-منتج-وحدات-أسعار
TRIGGERS: تفاصيل منتج, وحدات وأسعار, باركود منتج, سعر منتج, product details, units prices barcode
TABLES: Inventory.Data_Products, Inventory.Data_View_ProductUOMBarcodes, Inventory.Data_ProductInventories
NOTES: بحث بالاسم أو الكود. أظهر كل الوحدات والباركود والأسعار. {{PRODUCT_FILTER}} = LIKE على الاسم/الكود.
---

```sql
;WITH P AS (
  SELECT TOP 20
    p.ProductID_PK, p.ProductCode, p.ProductName,
    LEFT(p.SalesDecription, 120) AS SalesDescription,
    p.StockOnHand, p.MinStockLevel, p.MaxStockLevel
  FROM Inventory.Data_Products p
  WHERE p.IsInActive = 0
    AND (
      p.ProductName LIKE N'%{{PRODUCT_FILTER}}%'
      OR p.ProductCode LIKE N'%{{PRODUCT_FILTER}}%'
    )
  ORDER BY p.ProductName
)
SELECT
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  p.SalesDescription AS [الوصف],
  CAST(p.StockOnHand AS decimal(18,2)) AS [رصيد_إجمالي],
  u.UOMName AS [الوحدة],
  b.ProductBarcode AS [باركود],
  CAST(u.UomPrice1 AS decimal(18,2)) AS [سعر1],
  CAST(u.UomLastCost AS decimal(18,2)) AS [آخر_تكلفة]
FROM P p
LEFT JOIN Inventory.Data_View_ProductUOMBarcodes b ON b.ProductID_FK = p.ProductID_PK
LEFT JOIN Inventory.RefUOMs u ON u.UOMID_PK = b.UomID_FK
ORDER BY p.ProductName, u.UOMName;
```

---

## PATTERN: تقرير-الصلاحية
TRIGGERS: صلاحية, منتهية, قريبة الانتهاء, expiry, expiring products, تقرير الصلاحية
TABLES: Inventory.Data_ProductInventories, Inventory.Data_Products, MyCompany.Config_Branchs
NOTES: ExpiryDate من Data_ProductInventories. StockOnHand > 0. 90 يوم للتحذير.
---

```sql
SELECT TOP 100
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  b.BranchName AS [الفرع],
  CAST(i.StockOnHand AS decimal(18,2)) AS [الكمية],
  CAST(i.ExpiryDate AS date) AS [تاريخ_الصلاحية],
  DATEDIFF(day, CAST(GETDATE() AS date), CAST(i.ExpiryDate AS date)) AS [أيام_متبقية],
  CASE
    WHEN i.ExpiryDate < GETDATE() THEN N'منتهية'
    WHEN DATEDIFF(day, CAST(GETDATE() AS date), CAST(i.ExpiryDate AS date)) <= 90 THEN N'قريبة'
    ELSE N'سليمة'
  END AS [الحالة]
FROM Inventory.Data_ProductInventories i
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = i.ProductID_FK
LEFT JOIN MyCompany.Config_Branchs b ON b.BranchID_PK = i.BranchID_FK
WHERE p.IsInActive = 0
  AND i.StockOnHand > 0
  AND i.ExpiryDate IS NOT NULL
ORDER BY i.ExpiryDate ASC;
```

---

## PATTERN: متابعة-النواقص
TRIGGERS: نواقص, under stock, min level, منتجات تحت الحد, متابعة النواقص
TABLES: Inventory.Data_Products, Inventory.Data_ProductInventories
NOTES: MinStockLevel مقابل StockOnHand (إجمالي أو حسب فرع). الأولوية: رصيد=0 ثم تحت الحد.
---

```sql
;WITH Stock AS (
  SELECT ProductID_FK, SUM(StockOnHand) AS TotalQty
  FROM Inventory.Data_ProductInventories
  GROUP BY ProductID_FK
)
SELECT TOP 80
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  CAST(ISNULL(s.TotalQty, 0) AS decimal(18,2)) AS [الرصيد],
  CAST(p.MinStockLevel AS decimal(18,2)) AS [الحد_الأدنى],
  CAST(p.MaxStockLevel AS decimal(18,2)) AS [الحد_الأقصى],
  CASE
    WHEN ISNULL(s.TotalQty, 0) <= 0 THEN N'نفاد'
    WHEN ISNULL(s.TotalQty, 0) < p.MinStockLevel THEN N'تحت الحد'
    ELSE N'كافٍ'
  END AS [الأولوية]
FROM Inventory.Data_Products p
LEFT JOIN Stock s ON s.ProductID_FK = p.ProductID_PK
WHERE p.IsInActive = 0
  AND (ISNULL(s.TotalQty, 0) < p.MinStockLevel OR ISNULL(s.TotalQty, 0) <= 0)
ORDER BY ISNULL(s.TotalQty, 0) ASC, p.ProductName;
```

---

## PATTERN: طلبية-شراء-متقدمة
BATCH: yes
TRIGGERS: طلبية شراء, ماذا أشتري, شراء ذكي, صافي المطلوب, كمية مقترحة, suggested purchase, smart purchase
TABLES: Inventory.Data_Products, Inventory.Data_ProductInventories, SALES.*, Purchase.*
NOTES: Infinity فقط — من sql1.sql. معدل السحب واعٍ بأيام التوفّر. days_recent→@window_days (افتراض 60). coverage_days→@target_coverage_days (افتراض 35). يُرجع أصناف NetRequired>0 مع مورد مرن وتكلفة.
---

```sql
-- يُحمَّل من sql-split/01-purchase-order.sql
SELECT 1;
```

---

## PATTERN: آخر-منتجات-بيعت-اليوم
TRIGGERS: آخر منتجات بيعت اليوم, منتجات بيعت اليوم, ماذا بيع اليوم, last products sold today
TABLES: SALES.Data_View_SalesInvoiceItems, SALES.Data_SalesInvoices
NOTES: SalesInvoiceDate للتاريخ. إن فارغ استخدم MAX(SalesInvoiceDate).
---

```sql
;WITH SaleDay AS (
  SELECT CAST(GETDATE() AS date) AS d
)
SELECT TOP 100
  CAST(inv.SalesInvoiceDate AS datetime) AS [وقت_البيع],
  inv.InvoiceNumber AS [رقم_الفاتورة],
  vi.ProductCode AS [كود],
  vi.ProductName AS [اسم_المنتج],
  vi.UOMName AS [الوحدة],
  CAST(vi.QYT AS decimal(18,2)) AS [الكمية],
  CAST(vi.UnitPrice AS decimal(18,2)) AS [السعر],
  CAST(vi.QYT * vi.UnitPrice AS decimal(18,2)) AS [إجمالي_السطر],
  inv.CustomerName AS [العميل],
  inv.CreatedByUserName AS [الموظف],
  inv.BranchName AS [الفرع]
FROM SALES.Data_View_SalesInvoiceItems vi
INNER JOIN SALES.Data_View_SalesInvoices inv ON inv.SalesInvoiceID_PK = vi.SalesInvoiceID_FK
WHERE CAST(inv.SalesInvoiceDate AS date) = (SELECT d FROM SaleDay)
ORDER BY inv.SalesInvoiceDate DESC, vi.SalesInvoiceItemID_PK DESC;
```

---

## PATTERN: مبيعات-يومية-لكل-موظف
TRIGGERS: مبيعات يومية موظف, إجمالي مبيعات كل موظف, daily sales by employee
TABLES: SALES.Data_SalesInvoices, SALES.Data_SalesInvoiceItems
NOTES: CreatedByUserName = الموظف. الإيراد = SUM(QYT*UnitPrice). آخر 7 أيام من MAX(SalesInvoiceDate).
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
Daily AS (
  SELECT
    CAST(inv.SalesInvoiceDate AS date) AS SaleDay,
    inv.CreatedByUserName AS EmployeeName,
    SUM(si.QYT * si.UnitPrice) AS Revenue,
    COUNT(DISTINCT inv.SalesInvoiceID_PK) AS InvoiceCount
  FROM SALES.Data_SalesInvoices inv
  INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
  WHERE CAST(inv.SalesInvoiceDate AS date)
        BETWEEN DATEADD(day, -6, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  GROUP BY CAST(inv.SalesInvoiceDate AS date), inv.CreatedByUserName
)
SELECT
  SaleDay AS [التاريخ],
  EmployeeName AS [الموظف],
  CAST(Revenue AS decimal(18,2)) AS [إجمالي_المبيعات],
  InvoiceCount AS [عدد_الفواتير]
FROM Daily
ORDER BY SaleDay DESC, Revenue DESC;
```

---

## PATTERN: خصومات-وديون-موظفين
TRIGGERS: خصومات موظف, ديون موظف, خصومات الموظفين, employee discount, employee debt
TABLES: SysPermissions.Data_Users, SALES.Data_Customers, SALES.Data_SalesInvoices
NOTES: Infinity فقط. الموظف = Data_Users.FullName أو CreatedByUserName. حساب الزبون غالباً «الاسم - زبون». %EMPLOYEE% يُستبدل من keywords (فارغ = الكل).
---

```sql
;WITH CashierDiscounts AS (
  SELECT
    CreatedByUserName AS EmpName,
    SUM(InvoiceDiscountTotal) AS GivenDiscount,
    COUNT(DISTINCT SalesInvoiceID_PK) AS InvoiceCount
  FROM SALES.Data_SalesInvoices
  GROUP BY CreatedByUserName
),
RankedCustomer AS (
  SELECT
    u.FullName AS EmpName,
    c.CustomerName,
    c.CustomerOutstanding,
    c.TotalDiscountAmount,
    c.TotalSalesAmount,
    ROW_NUMBER() OVER (
      PARTITION BY u.FullName
      ORDER BY
        CASE WHEN c.CustomerName LIKE N'%- زبون%' THEN 0 ELSE 1 END,
        c.CustomerOutstanding DESC
    ) AS rn
  FROM SysPermissions.Data_Users u
  LEFT JOIN SALES.Data_Customers c ON c.CustomerName LIKE u.FullName + N'%'
),
BestCustomer AS (
  SELECT EmpName, CustomerName, CustomerOutstanding, TotalDiscountAmount, TotalSalesAmount
  FROM RankedCustomer
  WHERE rn = 1
)
SELECT
  COALESCE(e.EmpName, cd.EmpName) AS [الموظف],
  e.CustomerName AS [حساب_زبون],
  CAST(ISNULL(e.CustomerOutstanding, 0) AS decimal(18, 2)) AS [دين],
  CAST(ISNULL(e.TotalDiscountAmount, 0) AS decimal(18, 2)) AS [خصومات_تراكمية_عليه],
  CAST(ISNULL(e.TotalSalesAmount, 0) AS decimal(18, 2)) AS [مشتريات_شخصية],
  CAST(ISNULL(cd.GivenDiscount, 0) AS decimal(18, 2)) AS [خصومات_منحها_ككاشير],
  ISNULL(cd.InvoiceCount, 0) AS [عدد_فواتير]
FROM BestCustomer e
FULL OUTER JOIN CashierDiscounts cd ON cd.EmpName = e.EmpName
WHERE COALESCE(e.EmpName, cd.EmpName) LIKE N'%EMPLOYEE%'
  AND COALESCE(e.EmpName, cd.EmpName) IS NOT NULL
ORDER BY [دين] DESC, [خصومات_منحها_ككاشير] DESC;
```

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT TOP 50
  CAST(inv.SalesInvoiceDate AS date) AS [التاريخ],
  inv.CreatedByUserName AS [الموظف],
  inv.InvoiceNumber AS [الفاتورة],
  c.CustomerName AS [العميل],
  CAST(inv.InvoiceDiscountTotal AS decimal(18, 2)) AS [خصم_الفاتورة]
FROM SALES.Data_SalesInvoices inv
LEFT JOIN SALES.Data_Customers c ON c.CustomerID_PK = inv.CustomerID_FK
WHERE CAST(inv.SalesInvoiceDate AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
  AND inv.InvoiceDiscountTotal > 0
  AND inv.CreatedByUserName LIKE N'%EMPLOYEE%'
ORDER BY inv.SalesInvoiceDate DESC;
```

---

## PATTERN: أفضل-عملاء-مبيعات
TRIGGERS: أفضل عملاء, أكثر زبائن, top customers, customer sales ranking
TABLES: SALES.Data_SalesInvoices, SALES.Data_SalesInvoiceItems, SALES.Data_Customers
NOTES: نافذة 90 يوم من آخر تاريخ فاتورة.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT TOP 30
  c.CustomerName AS [العميل],
  COUNT(DISTINCT inv.SalesInvoiceID_PK) AS [عدد_الفواتير],
  CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS [إجمالي_المبيعات]
FROM SALES.Data_SalesInvoices inv
INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
INNER JOIN SALES.Data_Customers c ON c.CustomerID_PK = inv.CustomerID_FK
WHERE CAST(inv.SalesInvoiceDate AS date)
      BETWEEN DATEADD(day, -90, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
GROUP BY c.CustomerName
ORDER BY [إجمالي_المبيعات] DESC;
```

---

## PATTERN: دراسة-منتج-شاملة
TRIGGERS: دراسة منتج, تحليل منتج, product study, {{PRODUCT_FILTER}}
TABLES: Inventory.Data_Products, Data_ProductInventories, Data_View_ProductUOMBarcodes, SALES.Data_SalesInvoiceItems
NOTES: مرّر product_filter. خطوة واحدة مجمّعة: مخزون + وحدات + مبيعات 90 يوم.
---

```sql
;WITH P AS (
  SELECT TOP 1 ProductID_PK, ProductCode, ProductName, SalesDecription, MinStockLevel, MaxStockLevel
  FROM Inventory.Data_Products
  WHERE IsInActive = 0
    AND (
      ProductName LIKE N'%{{PRODUCT_FILTER}}%'
      OR ProductCode LIKE N'%{{PRODUCT_FILTER}}%'
      OR EXISTS (
        SELECT 1 FROM Inventory.Data_View_ProductUOMBarcodes BC
        WHERE BC.ProductID_FK = Inventory.Data_Products.ProductID_PK
          AND BC.ProductBarcode LIKE N'%{{PRODUCT_FILTER}}%'
      )
    )
  ORDER BY ProductName
),
Stock AS (
  SELECT SUM(i.StockOnHand) AS Qty, MIN(i.ExpiryDate) AS NearestExpiry
  FROM Inventory.Data_ProductInventories i
  INNER JOIN P ON P.ProductID_PK = i.ProductID_FK
),
Sales90 AS (
  SELECT SUM(si.QYT) AS SoldQty, MAX(inv.SalesInvoiceDate) AS LastSale
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  INNER JOIN P ON P.ProductID_PK = si.ProductID_FK
  WHERE inv.SalesInvoiceDate >= DATEADD(day, -90, GETDATE())
)
SELECT
  p.ProductCode AS [كود],
  p.ProductName AS [اسم],
  LEFT(p.SalesDecription, 100) AS [وصف],
  CAST(ISNULL(st.Qty, 0) AS decimal(18,2)) AS [المخزون],
  CAST(st.NearestExpiry AS date) AS [أقرب_صلاحية],
  CAST(ISNULL(s90.SoldQty, 0) AS decimal(18,2)) AS [مبيعات_90_يوم],
  s90.LastSale AS [آخر_بيع]
FROM P p
CROSS JOIN Stock st
CROSS JOIN Sales90 s90;
```

---

## PATTERN: حركة-صنف-تفصيلية
TRIGGERS: حركة صنف, تاريخ مبيعات منتج, product movement history, حركة شراء وبيع
TABLES: SALES.Data_SalesInvoiceItems, SALES.Data_SalesInvoices, Purchase.Data_PurchaseInvoiceItems, Inventory.Data_Products
NOTES: product_filter مطلوب. يجمع **بيع + شراء** — آخر 50 حركة.
---

```sql
;WITH P AS (
  SELECT ProductID_PK, ProductCode, ProductName FROM Inventory.Data_Products
  WHERE IsInActive = 0
    AND (ProductName LIKE N'%{{PRODUCT_FILTER}}%' OR ProductCode LIKE N'%{{PRODUCT_FILTER}}%')
)
SELECT TOP 50 MovType, TxDate, DocRef, Qty, Price, Party, Employee
FROM (
  SELECT N'بيع' AS MovType, CAST(inv.SalesInvoiceDate AS datetime) AS TxDate,
    inv.InvoiceNumber AS DocRef, si.QYT AS Qty, si.UnitPrice AS Price,
    inv.CustomerName AS Party, inv.CreatedByUserName AS Employee
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  INNER JOIN P ON P.ProductID_PK = si.ProductID_FK
  UNION ALL
  SELECT N'شراء', CAST(inv.InvoiceDate AS datetime), inv.InvoiceNumber,
    pi.QYT, pi.UnitCost, s.SupplierName, inv.CreatedByUserName
  FROM Purchase.Data_PurchaseInvoiceItems pi
  INNER JOIN Purchase.Data_PurchaseInvoices inv ON inv.InvoiceID_PK = pi.InvoiceID_FK
  INNER JOIN P ON P.ProductID_PK = pi.ProductID_FK
  LEFT JOIN Purchase.Data_Suppliers s ON s.SupplierID_PK = inv.SupplierID_FK
) X
ORDER BY TxDate DESC;
```

---

## PATTERN: آخر-سعر-شراء-مورد
TRIGGERS: آخر سعر شراء, purchase price supplier, سعر المورد
TABLES: Purchase.Data_PurchaseInvoiceItems, Purchase.Data_PurchaseInvoices, Purchase.Data_Suppliers, Inventory.Data_Products
NOTES: آخر فاتورة شراء لكل منتج (TOP 50).
---

```sql
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
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  lb.SupplierName AS [المورد],
  CAST(lb.LastCost AS decimal(18,2)) AS [آخر_سعر_شراء],
  CAST(lb.LastDate AS date) AS [التاريخ]
FROM LastBuy lb
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = lb.ProductID_FK
WHERE lb.rn = 1 AND p.IsInActive = 0
ORDER BY lb.LastDate DESC;
```

---

## PATTERN: جرد-المخزون-حسب-الفرع
TRIGGERS: جرد, inventory by branch, مخزون فرع, stock report branch
TABLES: Inventory.Data_ProductInventories, Inventory.Data_Products, MyCompany.Config_Branchs
NOTES: تجميع حسب فرع + منتج. TOP 200.
---

```sql
SELECT TOP 200
  b.BranchName AS [الفرع],
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  CAST(i.StockOnHand AS decimal(18,2)) AS [الكمية],
  CAST(i.ExpiryDate AS date) AS [الصلاحية]
FROM Inventory.Data_ProductInventories i
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = i.ProductID_FK
INNER JOIN MyCompany.Config_Branchs b ON b.BranchID_PK = i.BranchID_FK
WHERE p.IsInActive = 0 AND i.StockOnHand <> 0
ORDER BY b.BranchName, p.ProductName;
```

---

## PATTERN: معلومات-منتج-كاملة
TRIGGERS: معلومات منتج, معلومات عن, تفاصيل المنتج, معدل سحب, سعر البيع, صلاحية, باركود, product info, {{PRODUCT_FILTER}}
TABLES: Inventory.Data_Products, Data_ProductInventories, Data_View_ProductUOMBarcodes, SALES.Data_SalesInvoiceItems, Purchase.Data_PurchaseInvoiceItems
NOTES: **الافتراضي عند اسم/باركود فقط.** صف واحد: مخزون، سعر، تكلفة، معدل سحب، صلاحية، آخر مورد.
---

```sql
;WITH P AS (
  SELECT TOP 1 ProductID_PK, ProductCode, ProductName, MinStockLevel, MaxStockLevel
  FROM Inventory.Data_Products
  WHERE IsInActive = 0
    AND (
      ProductName LIKE N'%{{PRODUCT_FILTER}}%'
      OR ProductCode LIKE N'%{{PRODUCT_FILTER}}%'
      OR EXISTS (
        SELECT 1 FROM Inventory.Data_View_ProductUOMBarcodes BC
        WHERE BC.ProductID_FK = Inventory.Data_Products.ProductID_PK
          AND BC.ProductBarcode LIKE N'%{{PRODUCT_FILTER}}%'
      )
    )
  ORDER BY ProductName
),
UOM AS (
  SELECT TOP 1 b.ProductBarcode, b.UomPrice1, b.UomLastCost, b.UOMName
  FROM Inventory.Data_View_ProductUOMBarcodes b
  INNER JOIN P ON b.ProductID_FK = P.ProductID_PK
  ORDER BY b.BaseUnitQYT DESC, b.UomPrice1 DESC
),
Stock AS (
  SELECT SUM(i.StockOnHand) AS Qty, MIN(i.ExpiryDate) AS NearestExpiry
  FROM Inventory.Data_ProductInventories i
  INNER JOIN P ON P.ProductID_PK = i.ProductID_FK
),
Sales60 AS (
  SELECT SUM(si.QYT) AS SoldQty,
    COUNT(DISTINCT CAST(inv.SalesInvoiceDate AS date)) AS ActiveDays,
    MAX(inv.SalesInvoiceDate) AS LastSale
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  INNER JOIN P ON P.ProductID_PK = si.ProductID_FK
  WHERE inv.SalesInvoiceDate >= DATEADD(day, -60, GETDATE())
),
LastBuy AS (
  SELECT TOP 1 pi.UnitCost AS LastBuyPrice, s.SupplierName, inv.InvoiceDate AS LastBuyDate
  FROM Purchase.Data_PurchaseInvoiceItems pi
  INNER JOIN Purchase.Data_PurchaseInvoices inv ON pi.InvoiceID_FK = inv.InvoiceID_PK
  INNER JOIN P ON P.ProductID_PK = pi.ProductID_FK
  LEFT JOIN Purchase.Data_Suppliers s ON inv.SupplierID_FK = s.SupplierID_PK
  WHERE pi.UnitCost > 0
  ORDER BY inv.InvoiceDate DESC
)
SELECT
  p.ProductCode AS [كود],
  LEFT(p.ProductName, 80) AS [اسم],
  ISNULL(u.ProductBarcode, N'') AS [باركود],
  ISNULL(u.UOMName, N'') AS [الوحدة],
  CAST(ISNULL(u.UomPrice1, 0) AS decimal(18,2)) AS [سعر_البيع],
  CAST(ISNULL(u.UomLastCost, 0) AS decimal(18,2)) AS [آخر_تكلفة],
  CAST(ISNULL(st.Qty, 0) AS decimal(18,2)) AS [المخزون],
  CAST(st.NearestExpiry AS date) AS [أقرب_صلاحية],
  CAST(ISNULL(s60.SoldQty, 0) AS decimal(18,2)) AS [مبيعات_60_يوم],
  CAST(ISNULL(s60.SoldQty, 0) / NULLIF(CAST(s60.ActiveDays AS float), 0) AS decimal(12,3)) AS [معدل_السحب_اليومي],
  CAST(ISNULL(st.Qty, 0) / NULLIF(ISNULL(s60.SoldQty, 0) / NULLIF(CAST(s60.ActiveDays AS float), 0), 0) AS decimal(12,1)) AS [أيام_تغطية_المخزون],
  CAST(s60.LastSale AS date) AS [آخر_تاريخ_بيع],
  ISNULL(lb.SupplierName, N'—') AS [آخر_مورد],
  CAST(lb.LastBuyPrice AS decimal(18,2)) AS [آخر_سعر_شراء],
  CAST(lb.LastBuyDate AS date) AS [آخر_تاريخ_شراء]
FROM P p
LEFT JOIN UOM u ON 1 = 1
LEFT JOIN Stock st ON 1 = 1
LEFT JOIN Sales60 s60 ON 1 = 1
LEFT JOIN LastBuy lb ON 1 = 1;
```

---

## PATTERN: بحث-منتج-سريع
TRIGGERS: ابحث عن, find product, منتج, باركود, barcode lookup
TABLES: Inventory.Data_Products, Inventory.Data_View_ProductUOMBarcodes
NOTES: بحث عام بالاسم/الكود/الباركود — {{PRODUCT_FILTER}}.
---

```sql
SELECT TOP 25
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  b.ProductBarcode AS [باركود],
  CAST(p.StockOnHand AS decimal(18,2)) AS [رصيد],
  CAST(b.UomPrice1 AS decimal(18,2)) AS [سعر]
FROM Inventory.Data_Products p
LEFT JOIN Inventory.Data_View_ProductUOMBarcodes b ON b.ProductID_FK = p.ProductID_PK
WHERE p.IsInActive = 0
  AND (
    p.ProductName LIKE N'%{{PRODUCT_FILTER}}%'
    OR p.ProductCode LIKE N'%{{PRODUCT_FILTER}}%'
    OR b.ProductBarcode LIKE N'%{{PRODUCT_FILTER}}%'
  )
ORDER BY p.ProductName;
```

---

## PATTERN: نواقص-نشطة-مورد
TRIGGERS: نواقص نشطة, منتجات ناقصة تباع, أصناف ناقصة نشطة, shortage active supplier, نواقص آخر سعر شراء
TABLES: Inventory.Data_Products, Data_ProductInventories, SALES.Data_SalesInvoiceItems, Purchase.Data_PurchaseInvoiceItems
NOTES: **نشطة** = مبيعات > 0 في 60 يوم. **ناقصة** = StockOnHand <= MinStockLevel. آخر سعر من Purchase.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
Stock AS (
  SELECT ProductID_FK, SUM(StockOnHand) AS Qty FROM Inventory.Data_ProductInventories GROUP BY ProductID_FK
),
SalesRecent AS (
  SELECT si.ProductID_FK, SUM(si.QYT) AS SoldQty
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  WHERE CAST(inv.SalesInvoiceDate AS date) BETWEEN DATEADD(day, -60, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  GROUP BY si.ProductID_FK
),
LastBuy AS (
  SELECT pi.ProductID_FK, pi.UnitCost AS LastCost, s.SupplierName,
    ROW_NUMBER() OVER (PARTITION BY pi.ProductID_FK ORDER BY inv.InvoiceDate DESC) AS rn
  FROM Purchase.Data_PurchaseInvoiceItems pi
  INNER JOIN Purchase.Data_PurchaseInvoices inv ON pi.InvoiceID_FK = inv.InvoiceID_PK
  LEFT JOIN Purchase.Data_Suppliers s ON inv.SupplierID_FK = s.SupplierID_PK
)
SELECT TOP 100
  p.ProductCode AS [كود],
  LEFT(p.ProductName, 70) AS [اسم_المنتج],
  CAST(ISNULL(st.Qty, 0) AS decimal(18,2)) AS [الرصيد],
  CAST(p.MinStockLevel AS decimal(18,2)) AS [الحد_الأدنى],
  CAST(COALESCE(lb.LastCost, 0) AS decimal(18,2)) AS [آخر_سعر_شراء],
  ISNULL(lb.SupplierName, N'—') AS [المورد],
  CAST(sr.SoldQty AS decimal(18,2)) AS [مبيعات_60_يوم]
FROM Inventory.Data_Products p
INNER JOIN SalesRecent sr ON sr.ProductID_FK = p.ProductID_PK AND sr.SoldQty > 0
LEFT JOIN Stock st ON st.ProductID_FK = p.ProductID_PK
LEFT JOIN LastBuy lb ON lb.ProductID_FK = p.ProductID_PK AND lb.rn = 1
WHERE p.IsInActive = 0
  AND (ISNULL(st.Qty, 0) <= 0 OR ISNULL(st.Qty, 0) < p.MinStockLevel)
ORDER BY ISNULL(st.Qty, 0) ASC, sr.SoldQty DESC;
```

---

## PATTERN: مقارنة-أسعار-موردين
TRIGGERS: مقارنة أسعار, مقارنة أسعار الموردين, supplier price comparison, compare supplier prices
TABLES: Inventory.Data_Products, Purchase.Data_PurchaseInvoiceItems, Purchase.Data_PurchaseInvoices, Purchase.Data_Suppliers
NOTES: **لصنف واحد** — product_filter مطلوب. نافذة 36 شهر. ترتيب: أرخص آخر سعر.
---

```sql
;WITH P AS (
  SELECT TOP 1 ProductID_PK, ProductCode, ProductName
  FROM Inventory.Data_Products
  WHERE IsInActive = 0
    AND (
      ProductName LIKE N'%{{PRODUCT_FILTER}}%'
      OR ProductCode LIKE N'%{{PRODUCT_FILTER}}%'
      OR EXISTS (
        SELECT 1 FROM Inventory.Data_View_ProductUOMBarcodes BC
        WHERE BC.ProductID_FK = Inventory.Data_Products.ProductID_PK
          AND BC.ProductBarcode LIKE N'%{{PRODUCT_FILTER}}%'
      )
    )
  ORDER BY ProductName
),
Purchases AS (
  SELECT p.ProductID_PK, p.ProductName, p.ProductCode, inv.SupplierID_FK, s.SupplierName,
    pi.UnitCost AS Price, inv.InvoiceDate, pi.QYT,
    ROW_NUMBER() OVER (PARTITION BY inv.SupplierID_FK ORDER BY inv.InvoiceDate DESC) AS rn_last
  FROM P p
  INNER JOIN Purchase.Data_PurchaseInvoiceItems pi ON pi.ProductID_FK = p.ProductID_PK
  INNER JOIN Purchase.Data_PurchaseInvoices inv ON pi.InvoiceID_FK = inv.InvoiceID_PK
  LEFT JOIN Purchase.Data_Suppliers s ON inv.SupplierID_FK = s.SupplierID_PK
  WHERE pi.UnitCost > 0 AND inv.InvoiceDate >= DATEADD(month, -36, GETDATE())
),
BySupplier AS (
  SELECT ProductName, ProductCode, SupplierName,
    MAX(CASE WHEN rn_last = 1 THEN Price END) AS LastPrice,
    MAX(CASE WHEN rn_last = 1 THEN InvoiceDate END) AS LastBuyDate,
    MIN(Price) AS MinPrice, MAX(Price) AS MaxPrice, AVG(Price) AS AvgPrice,
    COUNT(*) AS PurchaseCount
  FROM Purchases GROUP BY ProductName, ProductCode, SupplierName
)
SELECT ProductName AS [اسم_المنتج], ProductCode AS [كود], SupplierName AS [المورد],
  CAST(LastPrice AS decimal(18,2)) AS [آخر_سعر_شراء],
  CAST(LastBuyDate AS date) AS [آخر_تاريخ],
  CAST(MinPrice AS decimal(18,2)) AS [أقل_سعر],
  CAST(MaxPrice AS decimal(18,2)) AS [أعلى_سعر],
  PurchaseCount AS [عدد_مرات_الشراء]
FROM BySupplier
ORDER BY LastPrice ASC, SupplierName;
```

---

## PATTERN: مبيعات-آخر-يوم-موظف
TRIGGERS: مبيعات آخر يوم, آخر يوم مبيعات, last sale day by employee, إيرادات آخر يوم
TABLES: SALES.Data_SalesInvoices, SALES.Data_SalesInvoiceItems
NOTES: @LastSaleDay = MAX(SalesInvoiceDate) — لا GETDATE() وحده.
---

```sql
;WITH LastDay AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
EmpSales AS (
  SELECT ISNULL(inv.CreatedByUserName, N'غير محدد') AS [الموظف],
    COUNT(DISTINCT inv.SalesInvoiceID_PK) AS [عدد_الفواتير],
    CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS [إيرادات], 0 AS SortOrder
  FROM SALES.Data_SalesInvoices inv
  INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
  WHERE CAST(inv.SalesInvoiceDate AS date) = (SELECT d FROM LastDay)
  GROUP BY inv.CreatedByUserName
),
Grand AS (
  SELECT N'═══ الإجمالي ═══' AS [الموظف],
    COUNT(DISTINCT inv.SalesInvoiceID_PK) AS [عدد_الفواتير],
    CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS [إيرادات], 1 AS SortOrder
  FROM SALES.Data_SalesInvoices inv
  INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
  WHERE CAST(inv.SalesInvoiceDate AS date) = (SELECT d FROM LastDay)
)
SELECT (SELECT d FROM LastDay) AS [تاريخ_آخر_مبيعات], [الموظف], [عدد_الفواتير], [إيرادات]
FROM (SELECT * FROM EmpSales UNION ALL SELECT * FROM Grand) X
ORDER BY SortOrder, [إيرادات] DESC;
```

---

## PATTERN: مبيعات-منتج-حسب-الوحدة
TRIGGERS: مبيعات الصنف بالوحدة, unit mix, sales by unit for product, أي وحدة تُباع أكثر
TABLES: SALES.Data_View_SalesInvoiceItems, Inventory.Data_Products
NOTES: product_filter مطلوب. آخر 90 يوم من MAX(SalesInvoiceDate).
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
P AS (
  SELECT TOP 1 ProductID_PK FROM Inventory.Data_Products
  WHERE IsInActive = 0
    AND (ProductName LIKE N'%{{PRODUCT_FILTER}}%' OR ProductCode LIKE N'%{{PRODUCT_FILTER}}%')
)
SELECT vi.UOMName AS [الوحدة],
  CAST(SUM(vi.QYT) AS decimal(18,1)) AS [كمية_صافية],
  CAST(SUM(vi.QYT * vi.UnitPrice) AS decimal(18,2)) AS [إيراد]
FROM SALES.Data_View_SalesInvoiceItems vi
INNER JOIN P ON vi.ProductID_FK = P.ProductID_PK
INNER JOIN SALES.Data_View_SalesInvoices inv ON inv.SalesInvoiceID_PK = vi.SalesInvoiceID_FK
WHERE CAST(inv.SalesInvoiceDate AS date)
      BETWEEN DATEADD(day, -90, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
GROUP BY vi.UOMName
ORDER BY [كمية_صافية] DESC;
```

---

## PATTERN: تحليل-المبيعات-والربحية
TRIGGERS: تحليل مبيعات, ربحية, هامش الربح, top sellers, best selling, sales analysis
TABLES: SALES.Data_SalesInvoiceItems, SALES.Data_SalesInvoices, Inventory.Data_Products, Inventory.Data_View_ProductUOMBarcodes
NOTES: هامش = (Revenue - Cost) / Revenue. Cost من UomLastCost × QYT.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
Sales AS (
  SELECT si.ProductID_FK,
    SUM(si.QYT) AS UnitsSold,
    SUM(si.QYT * si.UnitPrice) AS Revenue,
    SUM(si.QYT * ISNULL(b.UomLastCost, 0)) AS TotalCost
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  LEFT JOIN Inventory.Data_View_ProductUOMBarcodes b ON b.ProductID_FK = si.ProductID_FK
  WHERE CAST(inv.SalesInvoiceDate AS date)
        BETWEEN DATEADD(day, -90, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  GROUP BY si.ProductID_FK
)
SELECT TOP 20
  p.ProductCode AS [كود],
  LEFT(p.ProductName, 60) AS [اسم_المنتج],
  CAST(s.UnitsSold AS decimal(18,2)) AS [كمية],
  CAST(s.Revenue AS decimal(18,2)) AS [إيراد],
  CAST(s.TotalCost AS decimal(18,2)) AS [تكلفة_تقديرية],
  CAST(s.Revenue - s.TotalCost AS decimal(18,2)) AS [ربح_إجمالي],
  CAST(CASE WHEN s.Revenue > 0 THEN (s.Revenue - s.TotalCost) / s.Revenue * 100 ELSE 0 END AS decimal(10,1)) AS [هامش_٪]
FROM Sales s
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = s.ProductID_FK
WHERE p.IsInActive = 0
ORDER BY [ربح_إجمالي] DESC;
```

---

## PATTERN: متابعة-الديون
TRIGGERS: ديون, متابعة الديون, رصيد الزبائن, customer outstanding, ذمة
TABLES: SALES.Data_Customers
NOTES: CustomerOutstanding = الرصيد المستحق على Infinity (مُحدَّث في ERP). للتحليل التفصيلي راجع Financial.* لاحقاً.
---

```sql
SELECT TOP 100
  c.CustomerName AS [العميل],
  CAST(c.CustomerOutstanding AS decimal(18,2)) AS [الرصيد_المستحق],
  CASE WHEN c.IsAllowCreditSales = 1 THEN N'آجل' ELSE N'نقدي' END AS [نوع_الحساب]
FROM SALES.Data_Customers c
WHERE c.CustomerOutstanding >= 1
ORDER BY c.CustomerOutstanding DESC;
```

---

## PATTERN: ديون-الموردين
TRIGGERS: ديون الموردين, supplier balance, supplier debt, ما علي للموردين
TABLES: Purchase.Data_Suppliers
NOTES: SupplierAccountCurrentBalance — رصيد المورد على Infinity.
---

```sql
SELECT TOP 100
  s.SupplierName AS [المورد],
  CAST(s.SupplierAccountCurrentBalance AS decimal(18,2)) AS [الرصيد]
FROM Purchase.Data_Suppliers s
WHERE s.SupplierAccountCurrentBalance >= 1
ORDER BY s.SupplierAccountCurrentBalance DESC;
```

---

## PATTERN: ديون-وسلف-ومواعيد
TRIGGERS: ديون وسلف, سلف, قرض, مواعيد الدفع, ذمة, payment schedule, advances
TABLES: SALES.Data_Customers, Purchase.Data_Suppliers, Data_CustomerPaymentAppointments, Data_PaymentAppointments, Financial.Data_PaymentVouchers
NOTES: 3 أجزاء — (1) ديون زبائن+موردين (2) مواعيد دفع/تحصيل معلقة (3) سلف/ذمم شخصية. %PARTY% = فلتر اسم.
---

```sql
;WITH CustDebts AS (
  SELECT
    N'لي — زبون' AS [نوع_الذمة],
    c.CustomerName AS [الطرف],
    CAST(c.CustomerOutstanding AS decimal(18, 2)) AS [المبلغ_د_ل],
    CAST(c.CustomerCreditLimitValue AS decimal(18, 2)) AS [حد_الائتمان],
    CASE WHEN c.IsAllowCreditSales = 1 THEN N'آجل' ELSE N'نقدي' END AS [نوع_الحساب]
  FROM SALES.Data_Customers c
  WHERE c.CustomerOutstanding >= 1
    AND c.CustomerName LIKE N'%PARTY%'
),
SupDebts AS (
  SELECT
    N'علي — مورد' AS [نوع_الذمة],
    s.SupplierName AS [الطرف],
    CAST(s.SupplierAccountCurrentBalance AS decimal(18, 2)) AS [المبلغ_د_ل],
    CAST(NULL AS decimal(18, 2)) AS [حد_الائتمان],
    CASE WHEN s.IsAllowCreditPurchase = 1 THEN N'آجل' ELSE N'نقدي' END AS [نوع_الحساب]
  FROM Purchase.Data_Suppliers s
  WHERE s.SupplierAccountCurrentBalance >= 1
    AND s.SupplierName LIKE N'%PARTY%'
)
SELECT TOP 150 * FROM (
  SELECT * FROM CustDebts
  UNION ALL
  SELECT * FROM SupDebts
) x
ORDER BY [نوع_الذمة], [المبلغ_د_ل] DESC;
```

```sql
SELECT TOP 150
  [الاتجاه],
  [الطرف],
  CAST([موعد_الدفع] AS date) AS [موعد_الدفع],
  CAST([المبلغ_د_ل] AS decimal(18, 2)) AS [المبلغ_د_ل],
  [رقم_الفاتورة],
  CASE WHEN [تم] = 1 THEN N'تم' ELSE N'معلق' END AS [الحالة]
FROM (
  SELECT
    N'تحصيل — زبون' AS [الاتجاه],
    v.CustomerName AS [الطرف],
    v.PAppointmentDate AS [موعد_الدفع],
    v.PaymentAmount AS [المبلغ_د_ل],
    v.SalesInvoiceNumber AS [رقم_الفاتورة],
    CAST(v.IsDone AS int) AS [تم]
  FROM SALES.Data_View_CustomerPaymentAppointments v
  WHERE v.IsDone = 0
    AND v.CustomerName LIKE N'%PARTY%'
  UNION ALL
  SELECT
    N'دفع — مورد',
    v.SupplierName,
    v.PAppointmentDate,
    v.PaymentAmount,
    v.PurchaseInvoiceNumber,
    CAST(v.IsDone AS int)
  FROM Purchase.Data_View_PaymentAppointments v
  WHERE v.IsDone = 0
    AND v.SupplierName LIKE N'%PARTY%'
) s
ORDER BY [موعد_الدفع], [المبلغ_د_ل] DESC;
```

```sql
SELECT
  c.CustomerName AS [الطرف],
  CASE
    WHEN c.CustomerName LIKE N'%- زبون%' THEN N'سلف/ذمة موظف'
    ELSE N'ذمة زبون'
  END AS [النوع],
  CAST(c.CustomerOutstanding AS decimal(18, 2)) AS [دين_مفتوح],
  CAST(c.TotalSalesAmount AS decimal(18, 2)) AS [مشتريات_شخصية],
  CAST(c.TotalDiscountAmount AS decimal(18, 2)) AS [خصومات_تراكمية]
FROM SALES.Data_Customers c
WHERE c.CustomerName LIKE N'%PARTY%'
  AND (
    c.CustomerOutstanding >= 1
    OR (c.CustomerName LIKE N'%- زبون%' AND c.TotalSalesAmount >= 1)
  )
ORDER BY c.CustomerOutstanding DESC, c.TotalSalesAmount DESC;
```

---

## PATTERN: ملخص-مالي-شهري
TRIGGERS: ملخص مالي شهري, monthly summary, إيرادات الشهر, مبيعات شهرية
TABLES: SALES.Data_SalesInvoices, SALES.Data_SalesInvoiceItems
NOTES: ملخص مبسّط — إيرادات الشهر الحالي من آخر تاريخ فاتورة. للمصاريف/رواتب راجع Financial.* عند توفرها.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
MonthSales AS (
  SELECT
    YEAR(inv.SalesInvoiceDate) AS Y,
    MONTH(inv.SalesInvoiceDate) AS M,
    CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS Revenue,
    COUNT(DISTINCT inv.SalesInvoiceID_PK) AS InvoiceCount
  FROM SALES.Data_SalesInvoices inv
  INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
  WHERE YEAR(inv.SalesInvoiceDate) = YEAR((SELECT d FROM AsOf))
    AND MONTH(inv.SalesInvoiceDate) = MONTH((SELECT d FROM AsOf))
  GROUP BY YEAR(inv.SalesInvoiceDate), MONTH(inv.SalesInvoiceDate)
)
SELECT Y AS [السنة], M AS [الشهر], Revenue AS [إيرادات_الشهر], InvoiceCount AS [عدد_الفواتير]
FROM MonthSales;
```

---

## PATTERN: مردودات-مبيعات
TRIGGERS: مردودات مبيعات, مردود بيع, إرجاع من زبون, sales returns, refund, مرتجعات مبيعات
TABLES: SALES.Data_View_SalesInvoiceRefundSource, SALES.Data_View_SalesInvoices
NOTES: RefundSource = بنود المردود. انضم للفاتورة الأصلية للتاريخ والعميل. آخر 30 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT TOP 100
  CAST(inv.SalesInvoiceDate AS date) AS [تاريخ_المردود],
  inv.InvoiceNumber AS [رقم_الفاتورة],
  inv.CustomerName AS [العميل],
  rf.ProductCode AS [كود],
  rf.ProductName AS [اسم_المنتج],
  CAST(rf.QYT AS decimal(18,2)) AS [الكمية],
  CAST(rf.UnitPrice AS decimal(18,2)) AS [السعر],
  CAST(rf.SubTotal AS decimal(18,2)) AS [قيمة_السطر],
  inv.CreatedByUserName AS [الموظف]
FROM SALES.Data_View_SalesInvoiceRefundSource rf
INNER JOIN SALES.Data_View_SalesInvoices inv ON inv.SalesInvoiceID_PK = rf.SalesInvoiceID_FK
WHERE CAST(inv.SalesInvoiceDate AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
ORDER BY inv.SalesInvoiceDate DESC;
```

---

## PATTERN: مردودات-مشتريات
TRIGGERS: مردودات مشتريات, مردود شراء, إرجاع لمورد, purchase returns, مرتجعات شراء
TABLES: Purchase.Data_View_PurchaseInvoiceRefundSource, Purchase.Data_View_PurchaseInvoices
NOTES: آخر 30 يوماً من MAX(InvoiceDate).
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(InvoiceDate) AS date) AS d FROM Purchase.Data_PurchaseInvoices
)
SELECT TOP 100
  CAST(inv.InvoiceDate AS date) AS [تاريخ_المردود],
  inv.InvoiceNumber AS [رقم_الفاتورة],
  inv.SupplierName AS [المورد],
  rf.ProductCode AS [كود],
  rf.ProductName AS [اسم_المنتج],
  CAST(rf.QYT AS decimal(18,2)) AS [الكمية],
  CAST(rf.UnitCost AS decimal(18,2)) AS [التكلفة],
  CAST(rf.SubTotal AS decimal(18,2)) AS [قيمة_السطر]
FROM Purchase.Data_View_PurchaseInvoiceRefundSource rf
INNER JOIN Purchase.Data_View_PurchaseInvoices inv ON inv.InvoiceID_PK = rf.InvoiceID_FK
WHERE CAST(inv.InvoiceDate AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
ORDER BY inv.InvoiceDate DESC;
```

---

## PATTERN: تحصيلات-عملاء
TRIGGERS: تحصيلات, مقبوضات, مدفوعات العملاء, customer payments, sales payments, طرق الدفع
TABLES: SALES.Data_View_SalesInvoicePayments, SALES.Data_View_SalesInvoices
NOTES: LocCurrencyPaymentAmount = المبلغ بالعملة المحلية. آخر 30 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT TOP 100
  CAST(inv.SalesInvoiceDate AS date) AS [تاريخ_الفاتورة],
  inv.InvoiceNumber AS [رقم_الفاتورة],
  inv.CustomerName AS [العميل],
  pay.PaymentMethodCaption AS [طريقة_الدفع],
  CAST(pay.LocCurrencyPaymentAmount AS decimal(18,2)) AS [المبلغ_د_ل],
  ISNULL(pay.PaymentNote, N'') AS [ملاحظة]
FROM SALES.Data_View_SalesInvoicePayments pay
INNER JOIN SALES.Data_View_SalesInvoices inv ON inv.SalesInvoiceID_PK = pay.SalesInvoiceID_FK
WHERE CAST(inv.SalesInvoiceDate AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
ORDER BY inv.SalesInvoiceDate DESC;
```

---

## PATTERN: سندات-دفع-مالية
TRIGGERS: سندات دفع, سندات صرف, payment vouchers, Financial, مدفوعات مالية, صرف نقد
TABLES: Financial.Data_PaymentVouchers
NOTES: VoucherAmount = قيمة السند. آخر 60 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(VoucherDate) AS date) AS d FROM Financial.Data_PaymentVouchers
)
SELECT TOP 100
  CAST(v.VoucherDate AS date) AS [التاريخ],
  v.VoucherNumber AS [رقم_السند],
  CAST(v.VoucherAmount AS decimal(18,2)) AS [المبلغ_د_ل],
  v.PaymentDescriptionLine1 AS [البيان]
FROM Financial.Data_PaymentVouchers v
WHERE CAST(v.VoucherDate AS date) >= DATEADD(day, -60, (SELECT d FROM AsOf))
ORDER BY v.VoucherDate DESC;
```

---

## PATTERN: تحويلات-مخزون
TRIGGERS: تحويل مخزن, نقل مخزون, stock transfer, warehouse transfer, تحويل بين فروع
TABLES: Inventory.Data_View_StockTransfers, Inventory.Data_View_StockTransferProducts
NOTES: SourceLocationName → TargetLocationName. TransferredQYT = الكمية.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(DocumentDate) AS date) AS d FROM Inventory.Data_StockTransfers
)
SELECT TOP 100
  CAST(t.DocumentDate AS date) AS [التاريخ],
  t.StockTransferNumber AS [رقم_التحويل],
  t.BranchName AS [الفرع],
  t.SourceLocationName AS [من_موقع],
  t.TargetLocationName AS [إلى_موقع],
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  CAST(p.TransferredQYT AS decimal(18,2)) AS [الكمية],
  t.DocumentStateCaption AS [الحالة]
FROM Inventory.Data_View_StockTransfers t
INNER JOIN Inventory.Data_View_StockTransferProducts p ON p.StockTransferID_FK = t.StockTransferID_PK
WHERE CAST(t.DocumentDate AS date) >= DATEADD(day, -90, (SELECT d FROM AsOf))
ORDER BY t.DocumentDate DESC;
```

---

## PATTERN: أصناف-تالفة-متلفة
TRIGGERS: تالف, متلف, damaged items, spoiled, إتلاف, صنف تالف, damaged stock
TABLES: Inventory.Data_View_DamagedItems
NOTES: DamagedItemTypeCaption = نوع التلف. آخر 90 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(CreatedDate) AS date) AS d FROM Inventory.Data_DamagedItems
)
SELECT TOP 100
  CAST(d.CreatedDate AS date) AS [التاريخ],
  d.BranchName AS [الفرع],
  d.ProductCode AS [كود],
  d.ProductName AS [اسم_المنتج],
  d.DamagedItemTypeCaption AS [نوع_التلف],
  CAST(d.QYT AS decimal(18,2)) AS [الكمية],
  CAST(d.Cost AS decimal(18,2)) AS [التكلفة],
  CAST(d.ExpiryDate AS date) AS [الصلاحية]
FROM Inventory.Data_View_DamagedItems d
WHERE CAST(d.CreatedDate AS date) >= DATEADD(day, -90, (SELECT d FROM AsOf))
ORDER BY d.CreatedDate DESC;
```

---

## PATTERN: تسوية-جرد-مخزون
TRIGGERS: تسوية جرد, stock adjustment, جرد فعلي, inventory adjustment, فرق جرد, stock count
TABLES: Inventory.Data_View_StockAdjustments
NOTES: StockAdjustmentReasonCaption = سبب التسوية. للتفاصيل على مستوى الأصناف راجع Data_View_StockAdjustmentProducts.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(CreatedDate) AS date) AS d FROM Inventory.Data_StockAdjustments
)
SELECT TOP 100
  CAST(a.CreatedDate AS date) AS [التاريخ],
  a.StockAdjustmentNumber AS [رقم_التسوية],
  a.BranchName AS [الفرع],
  a.LocationName AS [الموقع],
  a.StockAdjustmentTypeCaption AS [نوع_التسوية],
  a.StockAdjustmentReasonCaption AS [السبب],
  a.StockAdjustmentStateCaption AS [الحالة],
  a.CreatedByUserName AS [الموظف]
FROM Inventory.Data_View_StockAdjustments a
WHERE CAST(a.CreatedDate AS date) >= DATEADD(day, -180, (SELECT d FROM AsOf))
ORDER BY a.CreatedDate DESC;
```

---

## PATTERN: أصناف-راكة
TRIGGERS: راكد, راكدة, slow moving, dead stock, بدون مبيعات, stock no sales
TABLES: Inventory.Data_Products, Inventory.Data_ProductInventories, SALES.Data_SalesInvoiceItems, SALES.Data_SalesInvoices
NOTES: مخزون > 0 ولا مبيعات في 90 يوم. IsInActive=0.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
Stock AS (
  SELECT ProductID_FK, SUM(StockOnHand) AS Qty FROM Inventory.Data_ProductInventories GROUP BY ProductID_FK
),
Sales90 AS (
  SELECT si.ProductID_FK, SUM(si.QYT) AS SoldQty, MAX(inv.SalesInvoiceDate) AS LastSale
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  WHERE CAST(inv.SalesInvoiceDate AS date) BETWEEN DATEADD(day, -90, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  GROUP BY si.ProductID_FK
)
SELECT TOP 100
  p.ProductCode AS [كود],
  LEFT(p.ProductName, 70) AS [اسم_المنتج],
  CAST(ISNULL(st.Qty, 0) AS decimal(18,2)) AS [المخزون],
  CAST(ISNULL(s.SoldQty, 0) AS decimal(18,2)) AS [مبيعات_90_يوم],
  CAST(s.LastSale AS date) AS [آخر_بيع]
FROM Inventory.Data_Products p
INNER JOIN Stock st ON p.ProductID_PK = st.ProductID_FK
LEFT JOIN Sales90 s ON p.ProductID_PK = s.ProductID_FK
WHERE p.IsInActive = 0
  AND ISNULL(st.Qty, 0) > 0
  AND ISNULL(s.SoldQty, 0) <= 0
ORDER BY st.Qty DESC;
```

---

## PATTERN: مقارنة-مبيعات-شهرية
TRIGGERS: مقارنة شهرية, مبيعات الشهر, الشهر الماضي, month over month, monthly comparison, نمو المبيعات
TABLES: SALES.Data_SalesInvoices, SALES.Data_SalesInvoiceItems
NOTES: يقارن الشهر الحالي (MAX SalesInvoiceDate) بالشهر السابق.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
MonthSales AS (
  SELECT
    YEAR(inv.SalesInvoiceDate) AS Y,
    MONTH(inv.SalesInvoiceDate) AS M,
    CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS Revenue,
    COUNT(DISTINCT inv.SalesInvoiceID_PK) AS InvoiceCount
  FROM SALES.Data_SalesInvoices inv
  INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
  WHERE CAST(inv.SalesInvoiceDate AS date) >= DATEADD(month, -2, DATEFROMPARTS(YEAR((SELECT d FROM AsOf)), MONTH((SELECT d FROM AsOf)), 1))
  GROUP BY YEAR(inv.SalesInvoiceDate), MONTH(inv.SalesInvoiceDate)
),
Cur AS (
  SELECT * FROM MonthSales WHERE Y = YEAR((SELECT d FROM AsOf)) AND M = MONTH((SELECT d FROM AsOf))
),
Prev AS (
  SELECT * FROM MonthSales
  WHERE DATEFROMPARTS(Y, M, 1) = DATEADD(month, -1, DATEFROMPARTS(YEAR((SELECT d FROM AsOf)), MONTH((SELECT d FROM AsOf)), 1))
)
SELECT
  N'الشهر الحالي' AS [الفترة],
  C.Y AS [السنة], C.M AS [الشهر],
  C.Revenue AS [الإيراد_د_ل],
  C.InvoiceCount AS [عدد_الفواتير],
  P.Revenue AS [إيراد_الشهر_السابق],
  CAST(C.Revenue - ISNULL(P.Revenue, 0) AS decimal(18,2)) AS [الفرق_د_ل],
  CAST(CASE WHEN ISNULL(P.Revenue, 0) > 0 THEN (C.Revenue - P.Revenue) / P.Revenue * 100 ELSE NULL END AS decimal(10,1)) AS [نسبة_التغير_%]
FROM Cur C
LEFT JOIN Prev P ON 1 = 1;
```

---

## PATTERN: سجل-مبيعات-عميل
TRIGGERS: سجل عميل, مشتريات عميل, فواتير زبون, customer history, customer purchases, تاريخ مبيعات عميل
TABLES: SALES.Data_View_SalesInvoiceItems, SALES.Data_View_SalesInvoices
NOTES: استبدل %CUSTOMER% بجزء من اسم العميل. آخر 180 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT TOP 100
  CAST(inv.SalesInvoiceDate AS date) AS [التاريخ],
  inv.InvoiceNumber AS [رقم_الفاتورة],
  inv.CustomerName AS [العميل],
  vi.ProductCode AS [كود],
  vi.ProductName AS [اسم_المنتج],
  CAST(vi.QYT AS decimal(18,2)) AS [الكمية],
  CAST(vi.UnitPrice AS decimal(18,2)) AS [السعر],
  CAST(vi.QYT * vi.UnitPrice AS decimal(18,2)) AS [إجمالي_السطر]
FROM SALES.Data_View_SalesInvoiceItems vi
INNER JOIN SALES.Data_View_SalesInvoices inv ON inv.SalesInvoiceID_PK = vi.SalesInvoiceID_FK
WHERE CAST(inv.SalesInvoiceDate AS date) >= DATEADD(day, -180, (SELECT d FROM AsOf))
  AND inv.CustomerName LIKE N'%CUSTOMER%'
ORDER BY inv.SalesInvoiceDate DESC;
```

---

## PATTERN: فواتير-شراء-حديثة
TRIGGERS: فواتير شراء, آخر مشتريات, purchase invoices, recent buys, مشتريات حديثة
TABLES: Purchase.Data_View_PurchaseInvoices, Purchase.Data_PurchaseInvoiceItems, Inventory.Data_View_Products
NOTES: آخر 30 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(InvoiceDate) AS date) AS d FROM Purchase.Data_PurchaseInvoices
)
SELECT TOP 100
  CAST(inv.InvoiceDate AS date) AS [تاريخ_الشراء],
  inv.InvoiceNumber AS [رقم_الفاتورة],
  inv.SupplierName AS [المورد],
  p.ProductCode AS [كود],
  p.ProductName AS [اسم_المنتج],
  CAST(pi.QYT AS decimal(18,2)) AS [الكمية],
  CAST(pi.UnitCost AS decimal(18,2)) AS [التكلفة],
  CAST(pi.SubTotal AS decimal(18,2)) AS [قيمة_السطر]
FROM Purchase.Data_View_PurchaseInvoices inv
INNER JOIN Purchase.Data_PurchaseInvoiceItems pi ON pi.InvoiceID_FK = inv.InvoiceID_PK
INNER JOIN Inventory.Data_View_Products p ON p.ProductID_PK = pi.ProductID_FK
WHERE CAST(inv.InvoiceDate AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
ORDER BY inv.InvoiceDate DESC;
```

---

## PATTERN: أعلى-منتجات-مبيعاً
TRIGGERS: أعلى منتجات, أكثر مبيعاً, best sellers, top products, الأكثر مبيعاً
TABLES: SALES.Data_SalesInvoiceItems, SALES.Data_SalesInvoices, Inventory.Data_Products
NOTES: آخر 30 يوماً. ترتيب حسب الكمية.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
),
Sales30 AS (
  SELECT si.ProductID_FK,
    SUM(si.QYT) AS NetQty,
    SUM(si.QYT * si.UnitPrice) AS NetRevenue
  FROM SALES.Data_SalesInvoiceItems si
  INNER JOIN SALES.Data_SalesInvoices inv ON inv.SalesInvoiceID_PK = si.SalesInvoiceID_FK
  WHERE CAST(inv.SalesInvoiceDate AS date) BETWEEN DATEADD(day, -30, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  GROUP BY si.ProductID_FK
)
SELECT TOP 30
  p.ProductCode AS [كود],
  LEFT(p.ProductName, 70) AS [اسم_المنتج],
  CAST(s.NetQty AS decimal(18,2)) AS [كمية],
  CAST(s.NetRevenue AS decimal(18,2)) AS [إيراد_د_ل]
FROM Sales30 s
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = s.ProductID_FK
WHERE p.IsInActive = 0 AND s.NetQty > 0
ORDER BY s.NetQty DESC;
```

---

## PATTERN: مبيعات-حسب-الفرع
TRIGGERS: مبيعات فرع, إيرادات الفرع, sales by branch, branch revenue, أداء الفروع
TABLES: SALES.Data_View_SalesInvoices, SALES.Data_SalesInvoiceItems
NOTES: BranchName من View. آخر 30 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT
  inv.BranchName AS [الفرع],
  COUNT(DISTINCT inv.SalesInvoiceID_PK) AS [عدد_الفواتير],
  CAST(SUM(si.QYT) AS decimal(18,1)) AS [الوحدات],
  CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS [الإيراد_د_ل]
FROM SALES.Data_View_SalesInvoices inv
INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
WHERE CAST(inv.SalesInvoiceDate AS date) BETWEEN DATEADD(day, -30, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
GROUP BY inv.BranchName
ORDER BY [الإيراد_د_ل] DESC;
```

---

## PATTERN: مبيعات-المندوب
TRIGGERS: مبيعات مندوب, أداء المندوب, sales rep, sale person, مندوب مبيعات, SalePerson
TABLES: SALES.Data_View_SalesInvoices, SALES.Data_SalesInvoiceItems, SALES.Config_SalePersons
NOTES: SalePersonName = المندوب. آخر 30 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SalesInvoiceDate) AS date) AS d FROM SALES.Data_SalesInvoices
)
SELECT
  ISNULL(inv.SalePersonName, N'غير محدد') AS [المندوب],
  COUNT(DISTINCT inv.SalesInvoiceID_PK) AS [عدد_الفواتير],
  CAST(SUM(si.QYT * si.UnitPrice) AS decimal(18,2)) AS [الإيراد_د_ل]
FROM SALES.Data_View_SalesInvoices inv
INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
WHERE CAST(inv.SalesInvoiceDate AS date) BETWEEN DATEADD(day, -30, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
GROUP BY inv.SalePersonName
ORDER BY [الإيراد_د_ل] DESC;
```

---

## PATTERN: عدد-المنتجات
TRIGGERS: عدد المنتجات, كم منتج, count products, عدد الاصناف
TABLES: Inventory.Data_Products
NOTES: عدد الأصناف النشطة فقط.
---

```sql
SELECT COUNT(*) AS [عدد_المنتجات_النشطة]
FROM Inventory.Data_Products
WHERE IsInActive = 0;
```

---

## PATTERN: أعلى-منتجات-كل-الوقت
TRIGGERS: أعلى منتجات كل الوقت, بدون تاريخ, all time sellers
TABLES: SALES.Data_SalesInvoiceItems, Inventory.Data_Products
NOTES: بدون نافذة زمنية — كل المبيعات.
---

```sql
;WITH NetSales AS (
  SELECT si.ProductID_FK AS ProductID,
    SUM(si.QYT) AS NetQty,
    SUM(si.QYT * si.UnitPrice) AS NetRevenue
  FROM SALES.Data_SalesInvoiceItems si
  GROUP BY si.ProductID_FK
)
SELECT TOP 30
  p.ProductCode AS [كود],
  LEFT(p.ProductName, 70) AS [اسم_المنتج],
  CAST(s.NetQty AS decimal(18,2)) AS [كمية],
  CAST(s.NetRevenue AS decimal(18,2)) AS [إيراد_د_ل]
FROM NetSales s
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = s.ProductID
WHERE p.IsInActive = 0 AND s.NetQty > 0
ORDER BY s.NetQty DESC;
```

---

## PATTERN: أصناف-راكدة-متقدمة
BATCH: yes
TRIGGERS: راكدة, راكد, بضاعة راكدة, slow moving, dead stock, تكلفة الراكد
TABLES: Inventory.*, SALES.*
NOTES: Infinity — الزائد عن هدف التغطية × التكلفة. pattern_id=slow_moving_adv. SQL من sql-split/02-slow-moving.sql
---

```sql
SELECT 1;
```

---

## PATTERN: خطر-الصلاحية-FEFO
BATCH: yes
TRIGGERS: خطر الصلاحية, كمية الخطر, FEFO, expiry risk, قيمة الخطر
TABLES: Inventory.Data_ProductInventories
NOTES: Infinity — كمية قد تنتهي قبل بيعها. pattern_id=expiry_risk_fefo. للقائمة البسيطة استخدم تقرير-الصلاحية.
---

```sql
SELECT 1;
```

---

## PATTERN: اتجاه-مبيعات-30-30
BATCH: yes
TRIGGERS: اتجاه المبيعات, 30/30, تغيّر المبيعات, صاعد, هابط, sales trend
TABLES: SALES.*, Inventory.*
NOTES: Infinity — مقارنة آخر 30 يوم vs السابق (واعٍ بالتوفّر). pattern_id=sales_trend_30
---

```sql
SELECT 1;
```

---

## PATTERN: أصناف-قيد-التجربة
BATCH: yes
TRIGGERS: قيد التجربة, أصناف تجريبية, trial products, صنف جديد
TABLES: Inventory.*, Purchase.*, SALES.*
NOTES: Infinity — أصناف جديدة تحت المراقبة. pattern_id=trial_products
---

```sql
SELECT 1;
```

---

## PATTERN: أصناف-وهمية
BATCH: yes
TRIGGERS: وهمي, وهمية, صنف وهمي, phantom, بدون بيع, متابعة وهمي
TABLES: Inventory.Data_Products, Purchase.*, SALES.*
NOTES: Infinity — بدون بيع 90/180/270 يوم. pattern_id=phantom_products. استعلام خفيف.
---

```sql
SELECT 1;
```

---

## PATTERN: تصنيف-حركة-الصنف
BATCH: yes
TRIGGERS: حركة الصنف, منشط, ضعيف الحركة, صنف ميت, تصنيف الحركة
TABLES: Inventory.*, SALES.*
NOTES: Infinity — منشط جداً/نشط/ميت/قيد التجربة. pattern_id=product_movement_class. لتاريخ حركة منتج واحد استخدم حركة-صنف-تفصيلية + product_filter.
---

```sql
SELECT 1;
```

---
# نهاية الملف
$bnd_infinity_agent_md$,
  1,
  '534f2faa6466bb1ebf6d16d8b91b16fcf11d3b017d48b7a563b594ce8c4a3608',
  true,
  'Published via Supabase MCP'
)
ON CONFLICT (bundle_key) DO UPDATE SET
  content = EXCLUDED.content,
  version = agent_content_bundles.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true,
  changelog = EXCLUDED.changelog;