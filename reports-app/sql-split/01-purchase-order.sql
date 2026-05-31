/* طلبية شراء ذكية — InfinityRetailDB
   كمية مطلوبة = (أيام التغطية × معدل السحب) − المخزون
   معدل السحب واعٍ بأيام التوفّر فقط
*/

SET NOCOUNT ON;

DECLARE @end DATE  = CAST(GETDATE() AS DATE);
DECLARE @window_days INT = 60;                      -- نافذة AvgDaily
DECLARE @start DATE = DATEADD(DAY, -(@window_days-1), @end);
DECLARE @target_coverage_days INT = 35;             -- هدف التغطية (ثابت)
DECLARE @round_to INT = 1;                          -- تقريب صافي المطلوب لمضاعفات العبوة
DECLARE @pre_window_days_target INT = CAST(FLOOR(0.5 * @window_days) AS INT);
DECLARE @hist_days  INT  = @window_days + @pre_window_days_target + 30;
DECLARE @hist_start DATE = DATEADD(DAY, -(@hist_days-1), @end);

------------------------------------------------------------
-- 1) المنتجات + التصنيفات + المورد الأساسي (من السكيما)
------------------------------------------------------------
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
-- 2) UOM: عامل العبوة (مرجع/مستنتج) + تكلفة مرجعية
------------------------------------------------------------
IF OBJECT_ID('tempdb..#UOM_Ref') IS NOT NULL DROP TABLE #UOM_Ref;
CREATE TABLE #UOM_Ref(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6), PackCost DECIMAL(18,2));

INSERT INTO #UOM_Ref
SELECT
  pu.ProductID_FK,
  CAST(pu.BaseUnitQYT AS DECIMAL(18,6)) AS PackFactor,
  CAST(COALESCE(pu.UomLastPurchaseCost, pu.UomLastCost, pu.UomPurchaseCost, pu.UomCost, 0.00) AS DECIMAL(18,2)) AS PackCost
FROM Inventory.Data_ProductUOMs pu
JOIN Inventory.RefUOMs u ON u.UOMID_PK=pu.UomID_FK
WHERE u.UOMName IN (N'عبوة',N'علبة',N'Pack',N'PACK')
   OR u.UOMName LIKE N'%عبو%' OR u.UOMName LIKE N'%علبة%';

IF OBJECT_ID('tempdb..#UOM_Inferred') IS NOT NULL DROP TABLE #UOM_Inferred;
CREATE TABLE #UOM_Inferred(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6));

;WITH F AS (
  SELECT
    sii.ProductID_FK AS ProductID,
    COALESCE(sii.UnitBaseQYT,1) AS Factor,
    ROW_NUMBER() OVER(PARTITION BY sii.ProductID_FK ORDER BY COUNT_BIG(*) DESC) rn
  FROM SALES.Data_SalesInvoiceItems sii
  GROUP BY sii.ProductID_FK, COALESCE(sii.UnitBaseQYT,1)
)
INSERT INTO #UOM_Inferred
SELECT ProductID, CAST(Factor AS DECIMAL(18,6)) FROM F WHERE rn=1;

IF OBJECT_ID('tempdb..#UOM') IS NOT NULL DROP TABLE #UOM;
CREATE TABLE #UOM(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6) NOT NULL, PackCostRef DECIMAL(18,2) NULL);

INSERT INTO #UOM
SELECT p.ProductID,
       COALESCE(r.PackFactor, i.PackFactor, 1.0) AS PackFactor,
       r.PackCost AS PackCostRef
FROM #PIG p
LEFT JOIN #UOM_Ref      r ON r.ProductID=p.ProductID
LEFT JOIN #UOM_Inferred i ON i.ProductID=p.ProductID;

------------------------------------------------------------
-- 3) تقويم + حركات يومية + رصيد EOD بالعبوة
------------------------------------------------------------
IF OBJECT_ID('tempdb..#Dates') IS NOT NULL DROP TABLE #Dates;
CREATE TABLE #Dates (d DATE PRIMARY KEY);

;WITH N AS (
  SELECT TOP (@hist_days) ROW_NUMBER() OVER(ORDER BY (SELECT NULL)) - 1 AS n
  FROM sys.all_objects
)
INSERT INTO #Dates
SELECT DATEADD(DAY,n,@hist_start) FROM N;

IF OBJECT_ID('tempdb..#DailyAgg') IS NOT NULL DROP TABLE #DailyAgg;
CREATE TABLE #DailyAgg(ProductID INT, d DATE, NetQTY_Pack DECIMAL(18,6));

INSERT INTO #DailyAgg
SELECT t.ProductID_FK, CAST(t.TransactionDate AS DATE),
       SUM(CAST(t.TransactionQYT AS DECIMAL(18,6))/NULLIF(u.PackFactor,0))
FROM Inventory.Data_InventoryTransactions t
JOIN #UOM u ON u.ProductID=t.ProductID_FK
WHERE CAST(t.TransactionDate AS DATE) BETWEEN @hist_start AND @end
  AND t.ProductID_FK IN (SELECT ProductID FROM #PIG)
GROUP BY t.ProductID_FK, CAST(t.TransactionDate AS DATE);

IF OBJECT_ID('tempdb..#Opening') IS NOT NULL DROP TABLE #Opening;
CREATE TABLE #Opening(ProductID INT PRIMARY KEY, OpeningQTY_Pack DECIMAL(18,6));

INSERT INTO #Opening
SELECT t.ProductID_FK, COALESCE(SUM(CAST(t.TransactionQYT AS DECIMAL(18,6))/NULLIF(u.PackFactor,0)),0)
FROM Inventory.Data_InventoryTransactions t
JOIN #UOM u ON u.ProductID=t.ProductID_FK
WHERE CAST(t.TransactionDate AS DATE) < @hist_start
  AND t.ProductID_FK IN (SELECT ProductID FROM #PIG)
GROUP BY t.ProductID_FK;

IF OBJECT_ID('tempdb..#Cumu') IS NOT NULL DROP TABLE #Cumu;
CREATE TABLE #Cumu(ProductID INT, d DATE, EOD_Pack DECIMAL(18,6));

INSERT INTO #Cumu
SELECT p.ProductID, dt.d,
       CAST(COALESCE(op.OpeningQTY_Pack,0)
         + SUM(COALESCE(da.NetQTY_Pack,0)) OVER (PARTITION BY p.ProductID ORDER BY dt.d ROWS UNBOUNDED PRECEDING)
         AS DECIMAL(18,6)) AS EOD_Pack
FROM #PIG p
CROSS JOIN #Dates dt
LEFT JOIN #DailyAgg da ON da.ProductID=p.ProductID AND da.d=dt.d
LEFT JOIN #Opening  op ON op.ProductID=p.ProductID;

------------------------------------------------------------
-- 4) أيام التوفّر + المبيعات المعتمدة + السحب/يوم
------------------------------------------------------------
IF OBJECT_ID('tempdb..#Avail') IS NOT NULL DROP TABLE #Avail;
CREATE TABLE #Avail(ProductID INT PRIMARY KEY, Days_in_window INT, PreRun_Capped INT, DaysApproved INT);

;WITH Win AS (
  SELECT ProductID, SUM(CASE WHEN d BETWEEN @start AND @end AND EOD_Pack>0 THEN 1 ELSE 0 END) AS Days_in_window
  FROM #Cumu GROUP BY ProductID
),
PreCnt AS (
  SELECT ProductID, SUM(CASE WHEN d<@start AND EOD_Pack>0 THEN 1 ELSE 0 END) AS PreHave
  FROM #Cumu GROUP BY ProductID
)
INSERT INTO #Avail
SELECT p.ProductID,
       COALESCE(w.Days_in_window,0),
       CASE WHEN COALESCE(pc.PreHave,0)>=@pre_window_days_target THEN @pre_window_days_target ELSE COALESCE(pc.PreHave,0) END,
       COALESCE(w.Days_in_window,0) + CASE WHEN COALESCE(pc.PreHave,0)>=@pre_window_days_target THEN @pre_window_days_target ELSE COALESCE(pc.PreHave,0) END
FROM #PIG p
LEFT JOIN Win   w  ON w.ProductID=p.ProductID
LEFT JOIN PreCnt pc ON pc.ProductID=p.ProductID;

IF OBJECT_ID('tempdb..#ApprovedDays') IS NOT NULL DROP TABLE #ApprovedDays;
CREATE TABLE #ApprovedDays(ProductID INT, d DATE);

-- 1) إضافة الأيام المعتمدة (داخل النافذة)
INSERT INTO #ApprovedDays
SELECT ProductID,d FROM #Cumu WHERE d BETWEEN @start AND @end AND EOD_Pack>0;

-- 2) إضافة الأيام المعتمدة (قبل النافذة)
;WITH PreCand AS (
  SELECT c.ProductID, c.d,
         -- نرتب الأيام (قبل النافذة) من الأحدث للأقدم
         ROW_NUMBER() OVER(PARTITION BY c.ProductID ORDER BY c.d DESC) rn
  FROM #Cumu c
  WHERE c.EOD_Pack > 0
    AND c.d < @start -- فقط الأيام التي تسبق نافذة التحليل
)
INSERT INTO #ApprovedDays
SELECT pc.ProductID, pc.d
FROM PreCand pc
JOIN #Avail af ON af.ProductID = pc.ProductID
WHERE pc.rn <= af.PreRun_Capped; -- نأخذ فقط العدد المسموح به (مثال: 30 يوم)

IF OBJECT_ID('tempdb..#SalesDaily') IS NOT NULL DROP TABLE #SalesDaily;
CREATE TABLE #SalesDaily(ProductID INT, d DATE, SalesBase DECIMAL(18,6));

INSERT INTO #SalesDaily
SELECT sii.ProductID_FK, CAST(si.SalesInvoiceDate AS DATE),
       SUM(CAST(COALESCE(sii.UnitBaseQYT,1) AS DECIMAL(18,6)) * sii.QYT)
FROM SALES.Data_SalesInvoiceItems sii
JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK=sii.SalesInvoiceID_FK
WHERE CAST(si.SalesInvoiceDate AS DATE) BETWEEN @hist_start AND @end
  AND sii.ProductID_FK IN (SELECT ProductID FROM #PIG)
GROUP BY sii.ProductID_FK, CAST(si.SalesInvoiceDate AS DATE);

IF OBJECT_ID('tempdb..#SalesApproved') IS NOT NULL DROP TABLE #SalesApproved;
CREATE TABLE #SalesApproved(ProductID INT PRIMARY KEY, SalesQTY_Pack DECIMAL(18,6), AvgDaily_Pack DECIMAL(18,6));

INSERT INTO #SalesApproved(ProductID, SalesQTY_Pack, AvgDaily_Pack)
SELECT ad.ProductID,
       CAST(SUM(COALESCE(sd.SalesBase,0))/NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS SalesQTY_Pack,
       CAST(
         CASE WHEN af.DaysApproved>0
              THEN (SUM(COALESCE(sd.SalesBase,0))/NULLIF(u.PackFactor,0)) * 1.0 / af.DaysApproved
              ELSE 0 END
         AS DECIMAL(18,6)
       ) AS AvgDaily_Pack
FROM #ApprovedDays ad
LEFT JOIN #SalesDaily sd ON sd.ProductID=ad.ProductID AND sd.d=ad.d
LEFT JOIN #UOM u        ON u.ProductID=ad.ProductID
JOIN #Avail af           ON af.ProductID=ad.ProductID
GROUP BY ad.ProductID, u.PackFactor, af.DaysApproved;

------------------------------------------------------------
------------------------------------------------------------
-- 5) مخزون فعلي + أقرب صلاحية FEFO من ProductInventories
------------------------------------------------------------
IF OBJECT_ID('tempdb..#StockPI') IS NOT NULL DROP TABLE #StockPI;
CREATE TABLE #StockPI(ProductID INT PRIMARY KEY, StockPI_Pack DECIMAL(18,6));

INSERT INTO #StockPI(ProductID, StockPI_Pack)
SELECT
  pi.ProductID_FK,
  CAST(SUM(CAST(pi.StockOnHand AS DECIMAL(18,6))) / NULLIF(u.PackFactor,0) AS DECIMAL(18,6))
FROM Inventory.Data_ProductInventories pi
JOIN #UOM u ON u.ProductID = pi.ProductID_FK
WHERE pi.ProductID_FK IN (SELECT ProductID FROM #PIG)
  AND CAST(pi.StockOnHand AS DECIMAL(18,6)) > 0
GROUP BY pi.ProductID_FK, u.PackFactor;

IF OBJECT_ID('tempdb..#FirstLot') IS NOT NULL DROP TABLE #FirstLot;
CREATE TABLE #FirstLot(
  ProductID    INT PRIMARY KEY,
  FirstExpiry  DATE,
  FirstLotBase DECIMAL(18,6),
  FirstLotPack DECIMAL(18,6)
);

;WITH InvLots AS (
  SELECT
    pi.ProductID_FK                    AS ProductID,
    CAST(pi.ExpiryDate AS DATE)        AS Expiry,
    SUM(CAST(pi.StockOnHand AS DECIMAL(18,6))) AS StockBase
  FROM Inventory.Data_ProductInventories pi
  WHERE pi.ProductID_FK IN (SELECT ProductID FROM #PIG)
    AND pi.ExpiryDate IS NOT NULL
    -- 🟢 الفلتر الجديد: استبعاد التواريخ الوهمية التي تضعها المنظومات للأصناف التي ليس لها صلاحية
    AND YEAR(pi.ExpiryDate) BETWEEN 2000 AND 2080 
    -- 🟢 التأكد من وجود رصيد فعلي لتجنب قراءة التواريخ من بواقي الكميات الصفرية (الفواصل العشرية الميتة)
    AND CAST(pi.StockOnHand AS DECIMAL(18,3)) > 0 
  GROUP BY pi.ProductID_FK, CAST(pi.ExpiryDate AS DATE)
),
Pick AS (
  SELECT il.ProductID, il.Expiry, il.StockBase,
         ROW_NUMBER() OVER(PARTITION BY il.ProductID ORDER BY il.Expiry ASC) rn
  FROM InvLots il
  -- 🟢 تم إزالة شرط تخطي التواريخ المنتهية لكي يقرأ أقرب تاريخ حقيقي حتى لو كان منتهياً
)
INSERT INTO #FirstLot(ProductID, FirstExpiry, FirstLotBase, FirstLotPack)
SELECT p.ProductID,
       p.Expiry AS FirstExpiry,
       p.StockBase AS FirstLotBase,
       CAST(p.StockBase/NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS FirstLotPack
FROM Pick p
JOIN #UOM u ON u.ProductID=p.ProductID
WHERE p.rn=1;
------------------------------------------------------------
-- 6) تكلفة فعّالة للعبوة (EffCostPerPack) لأغراض الخطر/الراكدة
------------------------------------------------------------
IF OBJECT_ID('tempdb..#LastCost') IS NOT NULL DROP TABLE #LastCost;
CREATE TABLE #LastCost(ProductID INT PRIMARY KEY, LastCostPerPack DECIMAL(18,6), LastPurchaseDate DATE);

;WITH LP AS (
  SELECT x.ProductID, x.Createddate,
         CAST(x.UnitCost / NULLIF(COALESCE(x.UnitBaseQYT,1),0) AS DECIMAL(18,6)) AS CostPerBase,
         ROW_NUMBER() OVER(PARTITION BY x.ProductID ORDER BY x.Createddate DESC, x.PurchaseInvoiceItemID_PK DESC) rn
  FROM (
    SELECT pii.ProductID_FK AS ProductID, pi.Createddate, pii.UnitCost, pii.UnitBaseQYT, pii.PurchaseInvoiceItemID_PK
    FROM Purchase.Data_PurchaseInvoiceItems pii
    JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK=pii.InvoiceID_FK
    WHERE pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  ) x
)
INSERT INTO #LastCost
SELECT lp.ProductID,
       CAST(lp.CostPerBase * NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS LastCostPerPack,
       lp.Createddate
FROM LP lp
JOIN #UOM u ON u.ProductID=lp.ProductID
WHERE lp.rn=1;

IF OBJECT_ID('tempdb..#SalesAvgCostPack') IS NOT NULL DROP TABLE #SalesAvgCostPack;
CREATE TABLE #SalesAvgCostPack(ProductID INT PRIMARY KEY, SalesAvgCostPerPack DECIMAL(18,6) NULL);

INSERT INTO #SalesAvgCostPack(ProductID, SalesAvgCostPerPack)
SELECT
  sii.ProductID_FK,
  CAST(
    NULLIF(SUM(COALESCE(sii.UnitCost,0) * sii.QYT),0)
    / NULLIF(SUM(COALESCE(sii.UnitBaseQYT,1) * sii.QYT),0)
    * NULLIF(u.PackFactor,0)
  AS DECIMAL(18,6)) AS SalesAvgCostPerPack
FROM SALES.Data_SalesInvoiceItems sii
JOIN #UOM u ON u.ProductID = sii.ProductID_FK
WHERE sii.ProductID_FK IN (SELECT ProductID FROM #PIG)
GROUP BY sii.ProductID_FK, u.PackFactor;

IF OBJECT_ID('tempdb..#CostEffective') IS NOT NULL DROP TABLE #CostEffective;
CREATE TABLE #CostEffective(ProductID INT PRIMARY KEY, EffCostPerPack DECIMAL(18,6) NULL);

INSERT INTO #CostEffective(ProductID, EffCostPerPack)
SELECT
  p.ProductID,
  CAST(
    COALESCE(
      NULLIF(lc.LastCostPerPack, 0),
      NULLIF(u.PackCostRef, 0),
      NULLIF(sac.SalesAvgCostPerPack, 0),
      0
    ) AS DECIMAL(18,6)
  ) AS EffCostPerPack
FROM #PIG p
LEFT JOIN #LastCost lc             ON lc.ProductID = p.ProductID
LEFT JOIN #UOM u                   ON u.ProductID  = p.ProductID
LEFT JOIN #SalesAvgCostPack sac    ON sac.ProductID= p.ProductID;

------------------------------------------------------------
-- 7) الحسابات الأساسية: الهدف/المطلوب/النفاذ/الراكدة/القيم
------------------------------------------------------------
IF OBJECT_ID('tempdb..#Calc') IS NOT NULL DROP TABLE #Calc;
CREATE TABLE #Calc(
  ProductID INT PRIMARY KEY,
  TargetQty_Pack DECIMAL(18,6),
  StockPI_Pack DECIMAL(18,6),
  NetRequired_Pack DECIMAL(18,6),
  DaysOfCover DECIMAL(18,3),
  SlowQty_Pack DECIMAL(18,6),
  SlowCost DECIMAL(18,2),
  NetRequired_Value DECIMAL(18,2)
);

INSERT INTO #Calc(ProductID, TargetQty_Pack, StockPI_Pack, NetRequired_Pack, DaysOfCover, SlowQty_Pack, SlowCost, NetRequired_Value)
SELECT
  p.ProductID,
  CAST(@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) AS DECIMAL(18,6)) AS TargetQty_Pack,
  CAST(COALESCE(spi.StockPI_Pack,0) AS DECIMAL(18,6)) AS StockPI_Pack,
  CAST(
    CASE
      WHEN (@round_to > 1) THEN
        CEILING(
          CASE WHEN (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0)) < 0
               THEN 0
               ELSE (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0))
          END / @round_to
        ) * @round_to
      ELSE
        CASE WHEN (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0)) < 0
             THEN 0
             ELSE (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0))
        END
    END AS DECIMAL(18,6)
  ) AS NetRequired_Pack,
  CAST(
    CASE WHEN COALESCE(sa.AvgDaily_Pack,0) > 0
         THEN COALESCE(spi.StockPI_Pack,0) / sa.AvgDaily_Pack
         ELSE NULL END
    AS DECIMAL(18,3)
  ) AS DaysOfCover,
  CAST(
    CASE
      WHEN COALESCE(sa.AvgDaily_Pack,0) > 0
        THEN CASE WHEN (COALESCE(spi.StockPI_Pack,0) - (@target_coverage_days * sa.AvgDaily_Pack)) > 0
                  THEN (COALESCE(spi.StockPI_Pack,0) - (@target_coverage_days * sa.AvgDaily_Pack))
                  ELSE 0 END
      ELSE NULL
    END AS DECIMAL(18,6)
  ) AS SlowQty_Pack,
  CAST( CAST(
        CASE
          WHEN COALESCE(sa.AvgDaily_Pack,0) > 0
            THEN CASE WHEN (COALESCE(spi.StockPI_Pack,0) - (@target_coverage_days * sa.AvgDaily_Pack)) > 0
                      THEN (COALESCE(spi.StockPI_Pack,0) - (@target_coverage_days * sa.AvgDaily_Pack))
                      ELSE 0 END
          ELSE NULL
        END
      AS DECIMAL(18,6)) * COALESCE(ce.EffCostPerPack,0) AS DECIMAL(18,2)) AS SlowCost,
  CAST( CAST(
        CASE
          WHEN (@round_to > 1) THEN
            CEILING(
              CASE WHEN (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0)) < 0
                   THEN 0 ELSE (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0)) END
              / @round_to
            ) * @round_to
          ELSE
            CASE WHEN (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0)) < 0
                 THEN 0 ELSE (@target_coverage_days * COALESCE(sa.AvgDaily_Pack,0) - COALESCE(spi.StockPI_Pack,0)) END
        END
      AS DECIMAL(18,6)) * COALESCE(ce.EffCostPerPack,0) AS DECIMAL(18,2)) AS NetRequired_Value
FROM #PIG p
LEFT JOIN #SalesApproved sa ON sa.ProductID=p.ProductID
LEFT JOIN #StockPI      spi ON spi.ProductID=p.ProductID
LEFT JOIN #CostEffective ce  ON ce.ProductID=p.ProductID;

-- 12) مرجع الموردين: (أرخص "سعر حالي" لمورد خلال 90 يوم) + كتل الشراء
------------------------------------------------------------
IF OBJECT_ID('tempdb..#Sup') IS NOT NULL DROP TABLE #Sup;
CREATE TABLE #Sup(SupplierID INT PRIMARY KEY, SupplierName NVARCHAR(200));
INSERT INTO #Sup SELECT SupplierID_PK, SupplierName FROM Purchase.Data_Suppliers;

IF OBJECT_ID('tempdb..#LastPurchaseIDs') IS NOT NULL DROP TABLE #LastPurchaseIDs;
CREATE TABLE #LastPurchaseIDs(
  ProductID INT PRIMARY KEY, 
  LastPurchaseSupplierID INT NULL, 
  LastPurchaseDate DATE NULL, 
  LastUnitCost_Pack DECIMAL(18,4) NULL
);

-- تحديد آخر عملية إدراج مطلقة (للاستخدام الاحتياطي عند عدم وجود إدراج حديث)
;WITH LP AS (
  SELECT x.ProductID,x.Createddate,x.SupplierID_FK,x.CostPerBase,
         ROW_NUMBER() OVER(PARTITION BY x.ProductID ORDER BY x.Createddate DESC, x.PurchaseInvoiceItemID_PK DESC) rn
  FROM (
    SELECT 
      pii.ProductID_FK AS ProductID, 
      pi.Createddate, 
      pi.SupplierID_FK,
      CAST(pii.UnitCost / NULLIF(COALESCE(pii.UnitBaseQYT,1),0) AS DECIMAL(18,6)) AS CostPerBase,
      pii.PurchaseInvoiceItemID_PK
    FROM Purchase.Data_PurchaseInvoiceItems pii
    JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK=pii.InvoiceID_FK
    WHERE pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  ) x 
)
INSERT INTO #LastPurchaseIDs
SELECT lp.ProductID, lp.SupplierID_FK, lp.Createddate, 
       CAST(lp.CostPerBase*NULLIF(u.PackFactor,0) AS DECIMAL(18,4))
FROM LP lp
JOIN #UOM u ON u.ProductID=lp.ProductID
WHERE lp.rn=1;

-- حساب "أرخص مورد" وسعره بناءً على آخر سعر لكل مورد (وقت الإدراج)
IF OBJECT_ID('tempdb..#Cost90d') IS NOT NULL DROP TABLE #Cost90d;
CREATE TABLE #Cost90d(
  ProductID INT PRIMARY KEY, 
  MinCost90d_SupplierID INT NULL, -- المورد الأرخص حالياً
  MinCost90d_Pack DECIMAL(18,4) NULL -- سعر المورد الأرخص حالياً
);

;WITH RawPurch AS (
  -- 1. جلب جميع المشتريات خلال 90 يوم حسب وقت الإدراج
  SELECT 
    pii.ProductID_FK AS ProductID,
    pi.SupplierID_FK,
    pi.Createddate,
    pii.PurchaseInvoiceItemID_PK,
    (pii.UnitCost / NULLIF(COALESCE(pii.UnitBaseQYT,1),0)) AS CostPerBase
  FROM Purchase.Data_PurchaseInvoiceItems pii
  JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK=pii.InvoiceID_FK
  WHERE CAST(pi.Createddate AS DATE) >= DATEADD(DAY, -90, @end)
    AND pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
),
LatestPricePerSupplier AS (
  -- 2. تحديد "آخر سعر" لكل مورد على حدة
  SELECT 
    r.ProductID,
    r.SupplierID_FK,
    CAST(r.CostPerBase * NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS CurrentPackCost,
    ROW_NUMBER() OVER(PARTITION BY r.ProductID, r.SupplierID_FK ORDER BY r.Createddate DESC, r.PurchaseInvoiceItemID_PK DESC) as rn_latest
  FROM RawPurch r
  JOIN #UOM u ON u.ProductID = r.ProductID
),
ValidSuppliersPrices AS (
  -- 3. تصفية الجدول لأخذ السعر الأحدث فقط لكل مورد
  SELECT 
    ProductID, 
    SupplierID_FK, 
    CurrentPackCost
  FROM LatestPricePerSupplier
  WHERE rn_latest = 1
),
CheapestSupplier AS (
  -- 4. المقارنة لاختيار الأرخص والسعر الخاص به
  SELECT 
    ProductID,
    SupplierID_FK AS BestSupplierID,
    CurrentPackCost AS BestPrice,
    ROW_NUMBER() OVER(PARTITION BY ProductID ORDER BY CurrentPackCost ASC) as rn_cheap
  FROM ValidSuppliersPrices
)
INSERT INTO #Cost90d(ProductID, MinCost90d_SupplierID, MinCost90d_Pack)
SELECT ProductID, BestSupplierID, BestPrice
FROM CheapestSupplier
WHERE rn_cheap = 1;


-- دمج المعلومات
IF OBJECT_ID('tempdb..#PrimarySupplier') IS NOT NULL DROP TABLE #PrimarySupplier;
CREATE TABLE #PrimarySupplier(ProductID INT PRIMARY KEY, SupplierID INT NULL);

INSERT INTO #PrimarySupplier(ProductID, SupplierID)
SELECT 
  p.ProductID,
  COALESCE(
     c90.MinCost90d_SupplierID,   -- 1. الأرخص (بناءً على أحدث أسعارهم)
     lpi.LastPurchaseSupplierID,  -- 2. آخر مورد تم الشراء منه
     p.MainSupplierID             -- 3. المورد الأساسي
  )
FROM #PIG p
LEFT JOIN #Cost90d c90         ON c90.ProductID = p.ProductID
LEFT JOIN #LastPurchaseIDs lpi ON lpi.ProductID = p.ProductID;

-- (حساب التكاليف كما هي باستخدام وقت الإدراج)
IF OBJECT_ID('tempdb..#CostPack') IS NOT NULL DROP TABLE #CostPack;
CREATE TABLE #CostPack(ProductID INT PRIMARY KEY, UnitCost_Pack DECIMAL(18,4));

;WITH LastPurchase AS (
  SELECT x.ProductID, CAST(x.CostPerBase AS DECIMAL(18,6)) AS CostPerBase
  FROM (
    SELECT 
      pii.ProductID_FK AS ProductID,
      CAST(pii.UnitCost / NULLIF(COALESCE(pii.UnitBaseQYT,1),0) AS DECIMAL(18,6)) AS CostPerBase,
      ROW_NUMBER() OVER(PARTITION BY pii.ProductID_FK 
                        ORDER BY pi.Createddate DESC, pii.PurchaseInvoiceItemID_PK DESC) rn
    FROM Purchase.Data_PurchaseInvoiceItems pii
    JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK=pii.InvoiceID_FK
    WHERE pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  ) x WHERE x.rn=1
),
SalesAvgCost AS (
  SELECT sii.ProductID_FK AS ProductID,
         CAST(
           NULLIF(SUM(COALESCE(sii.UnitCost,0)*sii.QYT),0) 
           / NULLIF(SUM(COALESCE(sii.UnitBaseQYT,1)*sii.QYT),0)
           AS DECIMAL(18,6)
         ) AS CostPerBase
  FROM SALES.Data_SalesInvoiceItems sii
  WHERE sii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  GROUP BY sii.ProductID_FK
)
INSERT INTO #CostPack
SELECT p.ProductID,
       CAST(
         COALESCE(u.PackCostRef, sac.CostPerBase*u.PackFactor, lp.CostPerBase*u.PackFactor, 0.00)
         AS DECIMAL(18,4)
       ) AS UnitCost_Pack
FROM #PIG p
LEFT JOIN #UOM u           ON u.ProductID=p.ProductID
LEFT JOIN LastPurchase lp  ON lp.ProductID=p.ProductID
LEFT JOIN SalesAvgCost sac ON sac.ProductID=p.ProductID;

-- 12.5) تاريخ آخر عملية بيع
------------------------------------------------------------
IF OBJECT_ID('tempdb..#LastSaleFinal') IS NOT NULL DROP TABLE #LastSaleFinal;
CREATE TABLE #LastSaleFinal(ProductID INT PRIMARY KEY, LastSaleDate DATE);

INSERT INTO #LastSaleFinal(ProductID, LastSaleDate)
SELECT 
  sii.ProductID_FK,
  MAX(CAST(si.SalesInvoiceDate AS DATE))
FROM SALES.Data_SalesInvoiceItems sii
JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
WHERE sii.ProductID_FK IN (SELECT ProductID FROM #PIG)
GROUP BY sii.ProductID_FK;

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  S0.SupplierName AS [المورد الأساسي (مرن)],
  CAST(SA.AvgDaily_Pack AS DECIMAL(18,6)) AS [معدل السحب اليومي],
  CAST(COALESCE(C.StockPI_Pack,0) AS DECIMAL(18,3)) AS [المخزون (Pack)],
  CAST(C.NetRequired_Pack AS DECIMAL(18,0)) AS [صافي الأصناف المطلوبة],
  CAST(C.NetRequired_Pack * COALESCE(CP.UnitCost_Pack,0) AS DECIMAL(18,2)) AS [القيمة التقديرية],
  CAST(C.DaysOfCover AS DECIMAL(18,1)) AS [مدة نفاذ المخزون],
  CAST(LC.LastCostPerPack AS DECIMAL(18,2)) AS [آخر سعر شراء (للعبوة)],
  LC.LastPurchaseDate AS [تاريخ آخر عملية شراء],
  LSF.LastSaleDate AS [تاريخ آخر عملية بيع],
  @target_coverage_days AS [أيام التغطية (ثابت)]
FROM #PIG P
LEFT JOIN #SalesApproved SA ON SA.ProductID = P.ProductID
LEFT JOIN #Calc C ON C.ProductID = P.ProductID
LEFT JOIN #PrimarySupplier PS ON PS.ProductID = P.ProductID
LEFT JOIN #Sup S0 ON S0.SupplierID = PS.SupplierID
LEFT JOIN #CostPack CP ON CP.ProductID = P.ProductID
LEFT JOIN #LastCost LC ON LC.ProductID = P.ProductID
LEFT JOIN #LastSaleFinal LSF ON LSF.ProductID = P.ProductID
WHERE COALESCE(C.NetRequired_Pack,0) > 0
ORDER BY C.NetRequired_Pack DESC, P.ProductName;
