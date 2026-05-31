/* أصناف وهمية — InfinityRetailDB
   بدون بيع منذ 90/180/270 يوم أو شراء بعد آخر بيع
   استعلام خفيف — لا يحتاج حساب الرصيد اليومي
*/

SET NOCOUNT ON;

DECLARE @end DATE = CAST(GETDATE() AS DATE);

IF OBJECT_ID('tempdb..#PIG') IS NOT NULL DROP TABLE #PIG;
CREATE TABLE #PIG(
  ProductID      INT PRIMARY KEY,
  ProductName    NVARCHAR(400),
  ProductCode    NVARCHAR(100),
  TreeCategory   NVARCHAR(200),
  MainCategory   NVARCHAR(200),
  SubCategory    NVARCHAR(200),
  GroupName      NVARCHAR(200),
  ProductType    NVARCHAR(200),
  Trademark      NVARCHAR(200),
  MainSupplierID INT NULL
);

INSERT INTO #PIG
SELECT
  p.ProductID_PK,
  p.ProductName,
  p.ProductCode,
  ISNULL(cat.InventoryCategoryName, N'(غير محدد)'),
  ISNULL(dep.InvDepartmentName,    N'(غير محدد)'),
  ISNULL(sub.InvSubDepartmentName, N'(غير محدد)'),
  ISNULL(pg.ProductGroupDescription, N'(غير محدد)'),
  ISNULL(pt.ProductTypeDescrption,   N'(غير محدد)'),
  ISNULL(tm.ProductTrademarkDescrption, N'(غير محدد)'),
  p.MainSupplierID_FK
FROM Inventory.Data_Products p
LEFT JOIN Inventory.RefInventorySubDepartments sub  ON sub.InvSubDepartmentID_PK = p.InvSubDepartmentID_FK
LEFT JOIN Inventory.RefInventoryDepartments    dep  ON dep.InvDepartmentID_PK    = sub.InvDepartmentID_FK
LEFT JOIN Inventory.RefInventoryCategories     cat  ON cat.InventoryCategoryID_PK = dep.InventoryCategoryID_FK
LEFT JOIN Inventory.RefProductGroups           pg   ON pg.ProductGroupID_PK      = p.ProductGroupID_FK
LEFT JOIN Inventory.RefProductTypes            pt   ON pt.ProductTypeID_PK       = p.ProductTypeID_FK
LEFT JOIN Inventory.RefProductTrademarks       tm   ON tm.ProductTrademarkID_PK  = p.ProductTrademarkID_FK;

------------------------------------------------------------
-- 11.8) تحديد الأصناف الوهمية بناءً على الفترة بين آخر إدراج وآخر بيع
------------------------------------------------------------
IF OBJECT_ID('tempdb..#PhantomProducts') IS NOT NULL DROP TABLE #PhantomProducts;
CREATE TABLE #PhantomProducts(ProductID INT PRIMARY KEY, PhantomStatus NVARCHAR(50));

-- تحديد آخر تاريخ إدراج لكل صنف
;WITH LastPurchaseDate AS (
  SELECT 
    pii.ProductID_FK AS ProductID,
    MAX(CAST(pi.Createddate AS DATE)) AS LastPurchaseDate
  FROM Purchase.Data_PurchaseInvoiceItems pii
  JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK = pii.InvoiceID_FK
  WHERE pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  GROUP BY pii.ProductID_FK
),
-- تحديد آخر تاريخ بيع لكل صنف
LastSaleDate AS (
  SELECT 
    sii.ProductID_FK AS ProductID,
    MAX(CAST(si.SalesInvoiceDate AS DATE)) AS LastSaleDate
  FROM SALES.Data_SalesInvoiceItems sii
  JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
  WHERE sii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  GROUP BY sii.ProductID_FK
),
-- حساب الفترة بين آخر إدراج وآخر بيع (أو حتى الآن إذا لم يكن هناك بيع)
DaysSinceLastSale AS (
  SELECT 
    p.ProductID,
    lpd.LastPurchaseDate,
    lsd.LastSaleDate,
    CASE 
      WHEN lsd.LastSaleDate IS NOT NULL THEN DATEDIFF(DAY, lsd.LastSaleDate, @end)
      WHEN lpd.LastPurchaseDate IS NOT NULL THEN DATEDIFF(DAY, lpd.LastPurchaseDate, @end)
      ELSE NULL
    END AS DaysSinceLastSale,
    CASE
      WHEN lsd.LastSaleDate IS NOT NULL AND lpd.LastPurchaseDate IS NOT NULL 
           AND lsd.LastSaleDate < lpd.LastPurchaseDate THEN 1  -- تم الشراء بعد آخر بيع
      ELSE 0
    END AS PurchasedAfterLastSale
  FROM #PIG p
  LEFT JOIN LastPurchaseDate lpd ON lpd.ProductID = p.ProductID
  LEFT JOIN LastSaleDate lsd ON lsd.ProductID = p.ProductID
  WHERE lpd.LastPurchaseDate IS NOT NULL  -- فقط الأصناف التي تم إدراجها
)
INSERT INTO #PhantomProducts(ProductID, PhantomStatus)
SELECT 
  dsls.ProductID,
  CASE
    -- إذا تم الشراء بعد آخر بيع، نحسب من تاريخ الإدراج
    WHEN dsls.PurchasedAfterLastSale = 1 THEN
      CASE
        WHEN DATEDIFF(DAY, dsls.LastPurchaseDate, @end) >= 270 THEN N'صنف وهمي'  -- 9 أشهر
        WHEN DATEDIFF(DAY, dsls.LastPurchaseDate, @end) >= 180 THEN N'ربما يكون صنف وهمي'  -- 6 أشهر
        WHEN DATEDIFF(DAY, dsls.LastPurchaseDate, @end) >= 90 THEN N'تحتاج إلى متابعة'  -- 3 أشهر
        ELSE NULL  -- أقل من 3 أشهر، لا يعتبر وهمي
      END
    -- إذا كان آخر بيع بعد آخر شراء أو في نفس الوقت
    ELSE
      CASE
        WHEN dsls.DaysSinceLastSale >= 270 THEN N'صنف وهمي'  -- 9 أشهر
        WHEN dsls.DaysSinceLastSale >= 180 THEN N'ربما يكون صنف وهمي'  -- 6 أشهر
        WHEN dsls.DaysSinceLastSale >= 90 THEN N'تحتاج إلى متابعة'  -- 3 أشهر
        ELSE NULL  -- أقل من 3 أشهر، لا يعتبر وهمي
      END
  END AS PhantomStatus
FROM DaysSinceLastSale dsls;

-- إضافة الأصناف غير الوهمية (لضمان وجود جميع الأصناف في الجدول)
INSERT INTO #PhantomProducts(ProductID, PhantomStatus)
SELECT p.ProductID, NULL
FROM #PIG p
WHERE p.ProductID NOT IN (SELECT ProductID FROM #PhantomProducts);

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  PP.PhantomStatus AS [الأصناف الوهمية],
  P.MainCategory AS [القسم الرئيسي],
  P.GroupName AS [المجموعة]
FROM #PIG P
JOIN #PhantomProducts PP ON PP.ProductID = P.ProductID
WHERE PP.PhantomStatus IS NOT NULL
ORDER BY PP.PhantomStatus DESC, P.ProductName;
