-- Agent OTA seed (patterns + AGENT bundles)

-- >>> pattern_طلبية-شراء-متقدمة.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'طلبية-شراء-متقدمة',
  'infinity_retail_db',
  $pat_73982$﻿/* طلبية شراء ذكية — InfinityRetailDB
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
$pat_73982$,
  1,
  '7035b2eaaf25d0b8b6ffbb76a8ba2d74da5dcf220446fc266fdef864477242a7',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> pattern_أصناف-راكدة-متقدمة.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'أصناف-راكدة-متقدمة',
  'infinity_retail_db',
  $pat_47678$﻿/* أصناف راكدة — InfinityRetailDB
   الزائد عن هدف التغطية (35 يوم) × التكلفة الفعّالة
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

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  CAST(COALESCE(C.StockPI_Pack,0) AS DECIMAL(18,3)) AS [المخزون (Pack)],
  CAST(SA.AvgDaily_Pack AS DECIMAL(18,6)) AS [معدل السحب اليومي],
  CAST(C.DaysOfCover AS DECIMAL(18,1)) AS [مدة نفاذ المخزون],
  CAST(C.SlowQty_Pack AS DECIMAL(18,0)) AS [كمية البضاعة الراكدة],
  CAST(C.SlowCost AS DECIMAL(18,2)) AS [تكلفة البضاعة الراكدة],
  P.MainCategory AS [القسم الرئيسي],
  P.GroupName AS [المجموعة]
FROM #PIG P
JOIN #Calc C ON C.ProductID = P.ProductID
LEFT JOIN #SalesApproved SA ON SA.ProductID = P.ProductID
WHERE C.SlowQty_Pack IS NOT NULL AND C.SlowQty_Pack > 0
ORDER BY C.SlowCost DESC, C.SlowQty_Pack DESC;
$pat_47678$,
  1,
  '1520b265df12b403d3b3adda50a1162e5766cdc9617aaf419efc82a12dbc6817',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> pattern_خطر-الصلاحية-FEFO.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'خطر-الصلاحية-FEFO',
  'infinity_retail_db',
  $pat_65583$﻿/* خطر الصلاحية (FEFO) — InfinityRetailDB
   كمية قد تنتهي قبل بيعها حسب معدل السحب
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

------------------------------------------------------------
-- 11) خطر الصلاحية (ديناميكي)
------------------------------------------------------------
IF OBJECT_ID('tempdb..#Risk') IS NOT NULL DROP TABLE #Risk;
CREATE TABLE #Risk(
  ProductID    INT PRIMARY KEY,
  DaysToExpiry INT NULL,
  AtRisk       BIT,
  RiskQty      DECIMAL(18,6) NULL
);

INSERT INTO #Risk(ProductID, DaysToExpiry, AtRisk, RiskQty)
SELECT
  p.ProductID,
  CASE WHEN fl.FirstExpiry IS NOT NULL THEN DATEDIFF(DAY, @end, fl.FirstExpiry) ELSE NULL END AS DaysToExpiry,
  CAST(CASE
         WHEN fl.FirstExpiry IS NULL OR c.DaysOfCover IS NULL THEN 0
         WHEN DATEDIFF(DAY, @end, fl.FirstExpiry) < 0 THEN 0
         WHEN DATEDIFF(DAY, @end, fl.FirstExpiry) <= CEILING(c.DaysOfCover) THEN 1
         ELSE 0
       END AS BIT) AS AtRisk,
  CAST(
    CASE
      WHEN fl.FirstExpiry IS NULL OR sa.AvgDaily_Pack IS NULL OR sa.AvgDaily_Pack<=0 OR c.DaysOfCover IS NULL THEN NULL
      ELSE
        CASE
          WHEN DATEDIFF(DAY, @end, fl.FirstExpiry) >= 0
               AND c.DaysOfCover > DATEDIFF(DAY, @end, fl.FirstExpiry)
          THEN
            CASE
              WHEN ( (c.DaysOfCover - DATEDIFF(DAY, @end, fl.FirstExpiry)) * sa.AvgDaily_Pack ) <= 0
                THEN 0
              ELSE
                CASE
                  WHEN fl.FirstLotPack <= ( (c.DaysOfCover - DATEDIFF(DAY, @end, fl.FirstExpiry)) * sa.AvgDaily_Pack )
                    THEN fl.FirstLotPack
                  ELSE ( (c.DaysOfCover - DATEDIFF(DAY, @end, fl.FirstExpiry)) * sa.AvgDaily_Pack )
                END
            END
          ELSE 0
        END
    END AS DECIMAL(18,6)
  ) AS RiskQty
FROM #PIG p
LEFT JOIN #FirstLot      fl ON fl.ProductID = p.ProductID
LEFT JOIN #Calc          c  ON c.ProductID  = p.ProductID
LEFT JOIN #SalesApproved sa ON sa.ProductID = p.ProductID;

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  CAST(COALESCE(C.StockPI_Pack,0) AS DECIMAL(18,3)) AS [المخزون (Pack)],
  FL.FirstExpiry AS [أقرب صلاحية (من الرصيد)],
  R.DaysToExpiry AS [أيام حتى الصلاحية],
  CAST(C.DaysOfCover AS DECIMAL(18,1)) AS [مدة نفاذ المخزون],
  CAST(CASE WHEN R.RiskQty IS NULL THEN NULL ELSE ROUND(R.RiskQty,0) END AS DECIMAL(18,0)) AS [كمية الخطر],
  CAST((CASE WHEN R.RiskQty IS NULL THEN NULL ELSE ROUND(R.RiskQty,0) END) * COALESCE(CE.EffCostPerPack,0) AS DECIMAL(18,2)) AS [قيمة الخطر]
FROM #PIG P
LEFT JOIN #FirstLot FL ON FL.ProductID = P.ProductID
LEFT JOIN #Calc C ON C.ProductID = P.ProductID
LEFT JOIN #Risk R ON R.ProductID = P.ProductID
LEFT JOIN #CostEffective CE ON CE.ProductID = P.ProductID
WHERE R.AtRisk = 1 OR (R.RiskQty IS NOT NULL AND R.RiskQty > 0)
ORDER BY R.DaysToExpiry ASC, [قيمة الخطر] DESC;
$pat_65583$,
  1,
  '7332efc8bde829d5dd7260af8e95237d50eff3c1c1b4026c4c075e1df686a154',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> pattern_اتجاه-مبيعات-30-30.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'اتجاه-مبيعات-30-30',
  'infinity_retail_db',
  $pat_92371$﻿/* اتجاه المبيعات 30/30 (واعٍ بالتوفّر) — InfinityRetailDB
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

-- حدود التغيير 30/30
DECLARE @A_start DATE = DATEADD(DAY,-29,@end);      -- آخر 30
DECLARE @A_end   DATE = @end;
DECLARE @B_start DATE = DATEADD(DAY,-60,@end);      -- الـ 30 السابقة
DECLARE @B_end   DATE = DATEADD(DAY,-31,@end);

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

------------------------------------------------------------
-- 8) اتجاه المبيعات 30/30 "واعي بالتوفّر" + إظهار أثر التوفّر
------------------------------------------------------------
IF OBJECT_ID('tempdb..#Sales60') IS NOT NULL DROP TABLE #Sales60;
CREATE TABLE #Sales60(ProductID INT, d DATE, QtyPack DECIMAL(18,6));

INSERT INTO #Sales60(ProductID, d, QtyPack)
SELECT
  sii.ProductID_FK,
  CAST(si.SalesInvoiceDate AS DATE) AS d,
  CAST(
    SUM(CAST(COALESCE(sii.UnitBaseQYT,1) AS DECIMAL(18,6)) * sii.QYT)
    / NULLIF(u.PackFactor,0)
    AS DECIMAL(18,6)
  ) AS QtyPack
FROM SALES.Data_SalesInvoiceItems sii
JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
JOIN #UOM u ON u.ProductID = sii.ProductID_FK
WHERE CAST(si.SalesInvoiceDate AS DATE) BETWEEN @B_start AND @A_end
  AND sii.ProductID_FK IN (SELECT ProductID FROM #PIG)
GROUP BY sii.ProductID_FK, CAST(si.SalesInvoiceDate AS DATE), u.PackFactor;

IF OBJECT_ID('tempdb..#AvailWin') IS NOT NULL DROP TABLE #AvailWin;
CREATE TABLE #AvailWin(ProductID INT PRIMARY KEY, Days_A INT, Days_B INT);

INSERT INTO #AvailWin(ProductID, Days_A, Days_B)
SELECT
  c.ProductID,
  SUM(CASE WHEN c.d BETWEEN @A_start AND @A_end AND c.EOD_Pack>0 THEN 1 ELSE 0 END) AS Days_A,
  SUM(CASE WHEN c.d BETWEEN @B_start AND @B_end AND c.EOD_Pack>0 THEN 1 ELSE 0 END) AS Days_B
FROM #Cumu c
WHERE c.d BETWEEN @B_start AND @A_end
GROUP BY c.ProductID;

IF OBJECT_ID('tempdb..#SalesWinAvail') IS NOT NULL DROP TABLE #SalesWinAvail;
CREATE TABLE #SalesWinAvail(ProductID INT PRIMARY KEY, Total_A DECIMAL(18,6), Total_B DECIMAL(18,6));

INSERT INTO #SalesWinAvail(ProductID, Total_A, Total_B)
SELECT
  p.ProductID,
  CAST(SUM(CASE WHEN s.d BETWEEN @A_start AND @A_end AND cA.EOD_Pack>0 THEN COALESCE(s.QtyPack,0) ELSE 0 END) AS DECIMAL(18,6)) AS Total_A,
  CAST(SUM(CASE WHEN s.d BETWEEN @B_start AND @B_end AND cB.EOD_Pack>0 THEN COALESCE(s.QtyPack,0) ELSE 0 END) AS DECIMAL(18,6)) AS Total_B
FROM #PIG p
LEFT JOIN #Sales60 s    ON s.ProductID = p.ProductID
LEFT JOIN #Cumu cA     ON cA.ProductID = p.ProductID AND cA.d = s.d
LEFT JOIN #Cumu cB     ON cB.ProductID = p.ProductID AND cB.d = s.d
GROUP BY p.ProductID;

IF OBJECT_ID('tempdb..#Change30_Aware') IS NOT NULL DROP TABLE #Change30_Aware;
CREATE TABLE #Change30_Aware(
  ProductID     INT PRIMARY KEY,
  Avg_A         DECIMAL(18,6),
  Avg_B         DECIMAL(18,6),
  Days_A        INT,
  Days_B        INT,
  AvailPct_A    DECIMAL(18,2),
  AvailPct_B    DECIMAL(18,2),
  ChangePct     DECIMAL(18,2),
  ChangeLabel   NVARCHAR(30)
);

;WITH R AS (
  SELECT
    w.ProductID,
    CAST(CASE WHEN w.Days_A>0 THEN swa.Total_A * 1.0 / w.Days_A ELSE NULL END AS DECIMAL(18,6)) AS Avg_A,
    CAST(CASE WHEN w.Days_B>0 THEN swa.Total_B * 1.0 / w.Days_B ELSE NULL END AS DECIMAL(18,6)) AS Avg_B,
    w.Days_A, w.Days_B
  FROM #AvailWin w
  LEFT JOIN #SalesWinAvail swa ON swa.ProductID = w.ProductID
),
C AS (
  SELECT
    r.ProductID,
    r.Avg_A,
    r.Avg_B,
    r.Days_A,
    r.Days_B,
    CAST(r.Days_A * 100.0 / 30.0 AS DECIMAL(18,2)) AS AvailPct_A,
    CAST(r.Days_B * 100.0 / 30.0 AS DECIMAL(18,2)) AS AvailPct_B,
    CAST(
      CASE
        WHEN r.Avg_B > 0 THEN ((COALESCE(r.Avg_A,0) - r.Avg_B) * 100.0 / r.Avg_B)
        WHEN r.Avg_B = 0 AND COALESCE(r.Avg_A,0) > 0 THEN 100.0
        ELSE 0.0
      END AS DECIMAL(18,2)
    ) AS ChangePct_Raw
  FROM R r
)
INSERT INTO #Change30_Aware(ProductID, Avg_A, Avg_B, Days_A, Days_B, AvailPct_A, AvailPct_B, ChangePct, ChangeLabel)
SELECT
  c.ProductID,
  c.Avg_A,
  c.Avg_B,
  c.Days_A,
  c.Days_B,
  c.AvailPct_A,
  c.AvailPct_B,
  CAST(CASE WHEN c.Days_A = 0 THEN NULL ELSE c.ChangePct_Raw END AS DECIMAL(18,2)) AS ChangePct,
  CASE
    WHEN c.Days_A = 0 THEN N'غير متاح (نفاد)'
    WHEN c.Avg_B > 0 AND COALESCE(c.Avg_A,0) > c.Avg_B THEN N'صاعد (واعي بالتوفّر)'
    WHEN c.Avg_B > 0 AND COALESCE(c.Avg_A,0) < c.Avg_B THEN N'هابط (واعي بالتوفّر)'
    WHEN c.Avg_B = 0 AND COALESCE(c.Avg_A,0) > 0         THEN N'جديد (واعي بالتوفّر)'
    ELSE N'ثابت (واعي بالتوفّر)'
  END AS ChangeLabel
FROM C c;

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  CAST(C30.ChangePct AS DECIMAL(18,2)) AS [% تغيّر المبيعات (آخر 30/الـ30 السابقة)],
  C30.ChangeLabel AS [اتجاه آخر 60 يوم (وصفي)],
  C30.AvailPct_A AS [% توفّر آخر 30 يوم],
  C30.AvailPct_B AS [% توفّر الـ30 السابقة],
  CAST(C30.Avg_A AS DECIMAL(18,4)) AS [متوسط يومي_A],
  CAST(C30.Avg_B AS DECIMAL(18,4)) AS [متوسط يومي_B]
FROM #PIG P
JOIN #Change30_Aware C30 ON C30.ProductID = P.ProductID
WHERE C30.ChangePct IS NOT NULL OR C30.Days_A > 0
ORDER BY ABS(COALESCE(C30.ChangePct,0)) DESC, P.ProductName;
$pat_92371$,
  1,
  '5d5d923db05cd1d1d07c34c971acdd99f21d8ee2b7d5647cd8a0fb830c2c3f98',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> pattern_أصناف-قيد-التجربة.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'أصناف-قيد-التجربة',
  'infinity_retail_db',
  $pat_39069$﻿/* أصناف قيد التجربة — InfinityRetailDB
   جديدة: شراء حديث + مخزون كان صفراً + بدون مبيعات كبيرة
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

-- فترة التحقق من "قيد التجربة" (3 أشهر)
DECLARE @trial_months INT = 3;
DECLARE @trial_check_start DATE = DATEADD(MONTH, -@trial_months, @end);

-- فترة التجربة بعد الشراء (شهر واحد)
DECLARE @trial_period_days INT = 30;

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

-- 11.7) تحديد الأصناف "قيد التجربة" (جديدة - تم إدراجها خلال آخر 3 أشهر وكان مخزونها 0 قبل الشراء)
------------------------------------------------------------
IF OBJECT_ID('tempdb..#TrialProducts') IS NOT NULL DROP TABLE #TrialProducts;
CREATE TABLE #TrialProducts(ProductID INT PRIMARY KEY, IsTrial BIT);

-- تحديد تاريخ آخر إدراج لكل صنف خلال آخر 3 أشهر
;WITH LastPurchaseInPeriod AS (
  SELECT 
    pii.ProductID_FK AS ProductID,
    MAX(CAST(pi.Createddate AS DATE)) AS LastPurchaseDate
  FROM Purchase.Data_PurchaseInvoiceItems pii
  JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK = pii.InvoiceID_FK
  WHERE CAST(pi.Createddate AS DATE) >= @trial_check_start
    AND pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  GROUP BY pii.ProductID_FK
),
-- تحديد الأصناف التي كان مخزونها 0 قبل آخر شراء
ZeroStockBeforePurchase AS (
  SELECT DISTINCT
    lp.ProductID,
    lp.LastPurchaseDate
  FROM LastPurchaseInPeriod lp
  WHERE NOT EXISTS (
    -- التأكد من عدم وجود مخزون قبل تاريخ الشراء
    SELECT 1
    FROM #Cumu c
    WHERE c.ProductID = lp.ProductID
      AND c.d < lp.LastPurchaseDate
      AND c.EOD_Pack > 0
  )
),
-- الأصناف التي لديها مخزون حالياً
HasCurrentStock AS (
  SELECT DISTINCT ProductID
  FROM #StockPI
  WHERE StockPI_Pack > 0
),
-- الأصناف التي تم شراؤها خلال آخر 3 أشهر وكان مخزونها 0 قبل الشراء (قيد التجربة لمدة شهر)
TrialFromPurchase AS (
  SELECT 
    zsp.ProductID,
    CAST(1 AS BIT) AS IsTrial
  FROM ZeroStockBeforePurchase zsp
  INNER JOIN HasCurrentStock hcs ON hcs.ProductID = zsp.ProductID
  WHERE DATEDIFF(DAY, zsp.LastPurchaseDate, @end) <= @trial_period_days  -- لم يمر شهر بعد الشراء
    -- التأكد من عدم وجود مبيعات كبيرة خلال فترة التجربة
    AND NOT EXISTS (
      SELECT 1
      FROM SALES.Data_SalesInvoiceItems sii
      JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
      WHERE sii.ProductID_FK = zsp.ProductID
        AND CAST(si.SalesInvoiceDate AS DATE) >= zsp.LastPurchaseDate
        AND sii.QYT > 10  -- إذا كان هناك مبيعات كبيرة، لا يعتبر قيد تجربة
    )
),
-- الأصناف التي لم يتم إدراجها خلال آخر 3 أشهر وكان مخزونها 0 قبل ذلك
NoPurchase3Months AS (
  SELECT DISTINCT p.ProductID
  FROM #PIG p
  WHERE NOT EXISTS (
    SELECT 1
    FROM Purchase.Data_PurchaseInvoiceItems pii
    JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK = pii.InvoiceID_FK
    WHERE pii.ProductID_FK = p.ProductID
      AND CAST(pi.Createddate AS DATE) >= @trial_check_start
  )
),
ZeroStockBeforePeriod AS (
  SELECT DISTINCT c.ProductID
  FROM #Cumu c
  WHERE c.d < @trial_check_start
    AND c.EOD_Pack <= 0
    AND NOT EXISTS (
      -- التأكد من عدم وجود مخزون قبل فترة التحقق
      SELECT 1
      FROM #Cumu c2
      WHERE c2.ProductID = c.ProductID
        AND c2.d < @trial_check_start
        AND c2.EOD_Pack > 0
    )
),
TrialFromNoPurchase AS (
  SELECT 
    p.ProductID,
    CAST(1 AS BIT) AS IsTrial
  FROM #PIG p
  INNER JOIN NoPurchase3Months np ON np.ProductID = p.ProductID
  INNER JOIN ZeroStockBeforePeriod zs ON zs.ProductID = p.ProductID
  INNER JOIN HasCurrentStock hcs ON hcs.ProductID = p.ProductID
  -- التأكد من عدم وجود مبيعات كبيرة خلال آخر 3 أشهر
  WHERE NOT EXISTS (
    SELECT 1
    FROM SALES.Data_SalesInvoiceItems sii
    JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
    WHERE sii.ProductID_FK = p.ProductID
      AND CAST(si.SalesInvoiceDate AS DATE) >= @trial_check_start
      AND sii.QYT > 10  -- إذا كان هناك مبيعات كبيرة، لا يعتبر قيد تجربة
  )
)
INSERT INTO #TrialProducts(ProductID, IsTrial)
SELECT ProductID, IsTrial FROM TrialFromPurchase
UNION
SELECT ProductID, IsTrial FROM TrialFromNoPurchase;

-- إضافة الأصناف غير قيد التجربة (لضمان وجود جميع الأصناف في الجدول)
INSERT INTO #TrialProducts(ProductID, IsTrial)
SELECT p.ProductID, CAST(0 AS BIT)
FROM #PIG p
WHERE p.ProductID NOT IN (SELECT ProductID FROM #TrialProducts);

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  CAST(COALESCE(C.StockPI_Pack,0) AS DECIMAL(18,3)) AS [المخزون (Pack)],
  CAST(SA.AvgDaily_Pack AS DECIMAL(18,6)) AS [معدل السحب اليومي],
  N'نعم' AS [قيد التجربة],
  P.GroupName AS [المجموعة],
  P.MainCategory AS [القسم الرئيسي]
FROM #PIG P
JOIN #TrialProducts TP ON TP.ProductID = P.ProductID
LEFT JOIN #Calc C ON C.ProductID = P.ProductID
LEFT JOIN #SalesApproved SA ON SA.ProductID = P.ProductID
WHERE TP.IsTrial = 1
ORDER BY P.ProductName;
$pat_39069$,
  1,
  '7ce8944ca0c1804cab9c4826ba11d146c9851738e4f4963fd6ed069231963ce6',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> pattern_أصناف-وهمية.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'أصناف-وهمية',
  'infinity_retail_db',
  $pat_1946$﻿/* أصناف وهمية — InfinityRetailDB
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
$pat_1946$,
  1,
  'f15c0da35824070d5680adee057464412e9e69363936761b203b0e96a2eace5f',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> pattern_تصنيف-حركة-الصنف.sql
INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  'تصنيف-حركة-الصنف',
  'infinity_retail_db',
  $pat_10669$﻿/* تصنيف حركة الصنف — InfinityRetailDB
   منشط / نشط / راكد / ميت / قيد التجربة
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

-- فترة التحقق من "قيد التجربة" (3 أشهر)
DECLARE @trial_months INT = 3;
DECLARE @trial_check_start DATE = DATEADD(MONTH, -@trial_months, @end);

-- فترة التجربة بعد الشراء (شهر واحد)
DECLARE @trial_period_days INT = 30;

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

-- 11.7) تحديد الأصناف "قيد التجربة" (جديدة - تم إدراجها خلال آخر 3 أشهر وكان مخزونها 0 قبل الشراء)
------------------------------------------------------------
IF OBJECT_ID('tempdb..#TrialProducts') IS NOT NULL DROP TABLE #TrialProducts;
CREATE TABLE #TrialProducts(ProductID INT PRIMARY KEY, IsTrial BIT);

-- تحديد تاريخ آخر إدراج لكل صنف خلال آخر 3 أشهر
;WITH LastPurchaseInPeriod AS (
  SELECT 
    pii.ProductID_FK AS ProductID,
    MAX(CAST(pi.Createddate AS DATE)) AS LastPurchaseDate
  FROM Purchase.Data_PurchaseInvoiceItems pii
  JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK = pii.InvoiceID_FK
  WHERE CAST(pi.Createddate AS DATE) >= @trial_check_start
    AND pii.ProductID_FK IN (SELECT ProductID FROM #PIG)
  GROUP BY pii.ProductID_FK
),
-- تحديد الأصناف التي كان مخزونها 0 قبل آخر شراء
ZeroStockBeforePurchase AS (
  SELECT DISTINCT
    lp.ProductID,
    lp.LastPurchaseDate
  FROM LastPurchaseInPeriod lp
  WHERE NOT EXISTS (
    -- التأكد من عدم وجود مخزون قبل تاريخ الشراء
    SELECT 1
    FROM #Cumu c
    WHERE c.ProductID = lp.ProductID
      AND c.d < lp.LastPurchaseDate
      AND c.EOD_Pack > 0
  )
),
-- الأصناف التي لديها مخزون حالياً
HasCurrentStock AS (
  SELECT DISTINCT ProductID
  FROM #StockPI
  WHERE StockPI_Pack > 0
),
-- الأصناف التي تم شراؤها خلال آخر 3 أشهر وكان مخزونها 0 قبل الشراء (قيد التجربة لمدة شهر)
TrialFromPurchase AS (
  SELECT 
    zsp.ProductID,
    CAST(1 AS BIT) AS IsTrial
  FROM ZeroStockBeforePurchase zsp
  INNER JOIN HasCurrentStock hcs ON hcs.ProductID = zsp.ProductID
  WHERE DATEDIFF(DAY, zsp.LastPurchaseDate, @end) <= @trial_period_days  -- لم يمر شهر بعد الشراء
    -- التأكد من عدم وجود مبيعات كبيرة خلال فترة التجربة
    AND NOT EXISTS (
      SELECT 1
      FROM SALES.Data_SalesInvoiceItems sii
      JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
      WHERE sii.ProductID_FK = zsp.ProductID
        AND CAST(si.SalesInvoiceDate AS DATE) >= zsp.LastPurchaseDate
        AND sii.QYT > 10  -- إذا كان هناك مبيعات كبيرة، لا يعتبر قيد تجربة
    )
),
-- الأصناف التي لم يتم إدراجها خلال آخر 3 أشهر وكان مخزونها 0 قبل ذلك
NoPurchase3Months AS (
  SELECT DISTINCT p.ProductID
  FROM #PIG p
  WHERE NOT EXISTS (
    SELECT 1
    FROM Purchase.Data_PurchaseInvoiceItems pii
    JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK = pii.InvoiceID_FK
    WHERE pii.ProductID_FK = p.ProductID
      AND CAST(pi.Createddate AS DATE) >= @trial_check_start
  )
),
ZeroStockBeforePeriod AS (
  SELECT DISTINCT c.ProductID
  FROM #Cumu c
  WHERE c.d < @trial_check_start
    AND c.EOD_Pack <= 0
    AND NOT EXISTS (
      -- التأكد من عدم وجود مخزون قبل فترة التحقق
      SELECT 1
      FROM #Cumu c2
      WHERE c2.ProductID = c.ProductID
        AND c2.d < @trial_check_start
        AND c2.EOD_Pack > 0
    )
),
TrialFromNoPurchase AS (
  SELECT 
    p.ProductID,
    CAST(1 AS BIT) AS IsTrial
  FROM #PIG p
  INNER JOIN NoPurchase3Months np ON np.ProductID = p.ProductID
  INNER JOIN ZeroStockBeforePeriod zs ON zs.ProductID = p.ProductID
  INNER JOIN HasCurrentStock hcs ON hcs.ProductID = p.ProductID
  -- التأكد من عدم وجود مبيعات كبيرة خلال آخر 3 أشهر
  WHERE NOT EXISTS (
    SELECT 1
    FROM SALES.Data_SalesInvoiceItems sii
    JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK = sii.SalesInvoiceID_FK
    WHERE sii.ProductID_FK = p.ProductID
      AND CAST(si.SalesInvoiceDate AS DATE) >= @trial_check_start
      AND sii.QYT > 10  -- إذا كان هناك مبيعات كبيرة، لا يعتبر قيد تجربة
  )
)
INSERT INTO #TrialProducts(ProductID, IsTrial)
SELECT ProductID, IsTrial FROM TrialFromPurchase
UNION
SELECT ProductID, IsTrial FROM TrialFromNoPurchase;

-- إضافة الأصناف غير قيد التجربة (لضمان وجود جميع الأصناف في الجدول)
INSERT INTO #TrialProducts(ProductID, IsTrial)
SELECT p.ProductID, CAST(0 AS BIT)
FROM #PIG p
WHERE p.ProductID NOT IN (SELECT ProductID FROM #TrialProducts);

SELECT
  P.ProductID AS [معرف الصنف],
  P.ProductName AS [اسم الصنف],
  P.ProductCode AS [كود الصنف],
  CAST(SA.AvgDaily_Pack AS DECIMAL(18,6)) AS [معدل السحب اليومي],
  CAST(COALESCE(C.StockPI_Pack,0) AS DECIMAL(18,3)) AS [المخزون (Pack)],
  CASE
    WHEN COALESCE(TP.IsTrial,0) = 1 THEN N'قيد التجربة'
    WHEN COALESCE(SA.AvgDaily_Pack,0) >= 5 THEN N'منشط جداً'
    WHEN COALESCE(SA.AvgDaily_Pack,0) >= 1 THEN N'نشط'
    WHEN COALESCE(SA.AvgDaily_Pack,0) >= 0.5 THEN N'جيد الحركة'
    WHEN COALESCE(SA.AvgDaily_Pack,0) >= 0.2 THEN N'مستقر'
    WHEN COALESCE(SA.AvgDaily_Pack,0) > 0.03 THEN N'ضعيف الحركة'
    WHEN COALESCE(SA.AvgDaily_Pack,0) > 0.01 THEN N'ضعيف جداً'
    ELSE N'ميت'
  END AS [حركة الصنف],
  P.GroupName AS [المجموعة]
FROM #PIG P
LEFT JOIN #SalesApproved SA ON SA.ProductID = P.ProductID
LEFT JOIN #Calc C ON C.ProductID = P.ProductID
LEFT JOIN #TrialProducts TP ON TP.ProductID = P.ProductID
WHERE COALESCE(C.StockPI_Pack,0) > 0 OR COALESCE(SA.AvgDaily_Pack,0) > 0
ORDER BY SA.AvgDaily_Pack DESC, P.ProductName;
$pat_10669$,
  1,
  'e61a30651741f8a08186b1caa3a2c7c25060b098896dd2a423bb76c653496c88',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;

-- >>> bundle_infinity_agent_md.sql
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

-- >>> bundle_marketing_agent_md.sql
INSERT INTO agent_content_bundles (bundle_key, erp_kind, bundle_type, content, version, content_sha256, is_active, changelog)
VALUES (
  'marketing_agent_md',
  'marketing2026',
  'agent_md',
  $bnd_marketing_agent_md$# AGENT_Marketing2026 — تعاليم الوكيل + أنماط الاستعلام
# ERP: Marketing2026 | SQL Server | schema: dbo
# يُحمَّل تلقائياً عند اكتشاف جداول dbo.ITEMS / dbo.SALE_INVOICE
#
# ════════════════════════════════════════════════════════════════════
# 🤖 تعاليم الوكيل (إلزامية)
# ════════════════════════════════════════════════════════════════════
# - أنت متخصص في **Marketing2026** فقط — لا تستخدم جداول Infinity (Inventory.*, SALES.*).
# - اللغة: عربية للمستخدم | SQL: T-SQL على SQL Server (SELECT/WITH فقط).
# - **run_query_pattern** أو **search_query_patterns** قبل كتابة SQL من الصفر.
# - التاريخ المرجعي: `MAX(S_DATE)` من SALE_INVOICE — لا GETDATE() وحده للتقارير التاريخية.
# - SALE_ITEMS **لا** يحتوي S_DATE — JOIN عبر S_ID → SALE_INVOICE.
# - الصلاحية: ITEMS_SUB.CATEOGRY3 | الكمية: ITEMS_SUB.QTY | البحث: LIKE N'%...%'.
# - الديون: لا BALANCE_C — استخدم نمط «متابعة-الديون».
# - العملة: د.ل | ترجم أسماء الأعمدة في PDF/Excel (جدول الترجمة أدناه).
# - DDL مرجعي: Full_Marketing_Database_DDL.sql | ملاحظات: DATABASE_NOTES.md
#
# كيفية الاستخدام (للوكيل الذكي):
#
# 1) search_query_patterns(keywords) — يُعيد نص النمط (حتى قسمين) للقراءة والتعديل.
# 2) run_query_pattern(keywords, days_recent?, coverage_days?, product_filter?) — يبحث، يستخرج SQL، ينفّذ.
# 3) plan_complex_query(question, product_filter?, ...) — يرسم خطة خطوات (Mermaid + SQL جاهز لكل خطوة).
# 4) execute_query_plan(steps[]) — ينفّذ الخطة خطوة بخطوة ويجمع النتائج.
# 5) get_database_views() — Views وقواعد الربط (SALE_ITEMS_INVOICE_VIEW، الموظفين، anti-patterns).
#
# أمثلة:
#   search_query_patterns("طلبية شراء ذكية")
#   run_query_pattern("متابعة الديون")
#   run_query_pattern("طلبية شراء", days_recent=45, coverage_days=20)
#
# بعد تنفيذ ناجح: export_last_result(title, format=pdf|excel)
# قبل تنفيذ SQL جديد: validate_sql(sql_query)
#
# ⚠️ قاعدة صارمة: جميع الاستعلامات هنا تبدأ بـ WITH أو SELECT مباشرةً — لا DECLARE.
#    هذا يجعلها متوافقة مع execute_raw_sql وcreate_excel_report وcreate_pdf_report.
#    إذا أراد المستخدم تغيير معامل (مثل 60 يوماً → 30) فعدّل الرقم مباشرةً في SQL.
#
# ════════════════════════════════════════════════════════════════════
# 📋 قاعدة ترجمة أسماء الأعمدة (إلزامية لجميع التقارير)
# ════════════════════════════════════════════════════════════════════
# عند توليد أي تقرير PDF أو Excel، يجب ترجمة كل اسم عمود قبل تمريره للأداة.
# لا تُمرّر مسميات قاعدة البيانات (ITEM_NAME, QTY, PRICE...) مطلقاً.
#
# جدول الترجمة الرئيسي:
#   ITEM_NAME       → اسم المنتج        ITEM_MODEL      → الكود
#   QTY             → الكمية            PRICE           → السعر
#   LAST_COST       → آخر تكلفة         AVER_COST       → متوسط التكلفة
#   S_DATE          → تاريخ البيع       B_DATE          → تاريخ الشراء
#   CUST_NAME       → اسم العميل        FULL_NAME       → اسم الموظف
#   STORE_NAME      → المخزن            STORE_ID        → رقم المخزن
#   G_VALUE         → المبلغ المدفوع    T_VALUE         → المبلغ المحصَّل
#   G_DATE          → تاريخ الدفع       T_DATE          → تاريخ التحصيل
#   BAISC_SALARY    → الراتب الأساسي    OVER_TIME       → العمل الإضافي
#   BONCE           → المكافأة          BORROW_DISCOUNT → خصم السلفة
#   PENALTY         → خصم الجزاء        S_STATUES       → الحالة
#   UNIT_DISC       → وحدة البيع       BARCODE         → الباركود
#   PRICE1          → سعر البيع        PRICE2          → سعر 2
#   PUBLIC_PRICE    → سعر الجمهور      LAST_COST       → آخر تكلفة
#
# ════════════════════════════════════════════════════════════════════
# 📊 تقرير التطبيق: البحث عن تفاصيل المنتج (Marketing2026)
# ════════════════════════════════════════════════════════════════════
# يُنفَّذ من generic-report-page → execute_search_report → dbo.ITEMS+BARCODE
# أعمدة النتيجة (عربي):
#   الكود | اسم المنتج | وحدة البيع | الباركود | سعر البيع | سعر 2 | سعر الجمهور
#   | آخر تكلفة | رصيد المخزون | آخر مورد | تاريخ تحديث السعر
# ⚠️ لا تستخدم هذه التسميات على Infinity — هناك UomPrice1 = «السعر» وليس «سعر الجمهور»
#
# ⚠️ جميع الاستعلامات تبدأ بـ WITH أو SELECT — متوافقة مع execute_raw_sql.
#   DailyRate       → معدل البيع/يوم    CoverageDays    → أيام التغطية
#   SuggestedQty    → الكمية المقترحة   StockQty        → المخزون الحالي
#   SoldQty         → الكمية المباعة    LastPurchasePrice → آخر سعر شراء
#   TotalDebt       → إجمالي الدَّين    PaidAmount      → المدفوع
#   RemainingDebt   → المتبقي           ExpiryDate      → تاريخ انتهاء الصلاحية
#   BatchNo         → رقم الدفعة        SaleName        → اسم المندوب
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
# 4. get_database_views() — عند أخطاء تجميع مبيعات/موظفين
# 5. validate_sql(sql) — قبل execute_raw_sql
# 6. export_last_result(pdf|excel) — عند طلب تصدير
# 7. save_favorite_query — فوراً عند «احفظ/خزّن» (لا تقل «تم الحفظ» بدون استدعاء الأداة)
#
# ## <thinking>  (تحليل داخلي — لا تُعرضه للمستخدم إلا إن طلب «اشرح خطواتك»)
# 1. ما المطلوب بدقة؟ (تقرير، رقم، مقارنة، توصية، تصدير...)
# 2. حساس للتاريخ؟ → get_current_datetime أو MAX(S_DATE) من SALE_INVOICE
# 3. هل يطابق نمطاً في هذا الملف؟ → run_query_pattern **أولاً** (جدول keywords أعلاه)
# 4. جداول Marketing فقط — لا Inventory.* ولا SALES.Data_*
# 5. مخزون = ITEMS_SUB.QTY | تاريخ بيع = SALE_INVOICE.S_DATE (ليس من SALE_ITEMS)
# 6. ديون → نمط «متابعة-الديون» — BALANCE_C فارغ
# 7. product_filter من @mention في الرسالة؟
# 8. ما التحذير أو الفرصة في البيانات؟ (نفاد، صلاحية، تركيز ديون...)
# </thinking>
#
# ## <answer>  (ما يراه المستخدم)
# - عربية واضحة — عناوين، قوائم، أهم الأرقام أولاً
# - أرقام **من نتائج الأداة فقط** — مع الوحدات: د.ل، قطعة، يوم، %
# - ترجم أسماء الأعمدة في PDF/Excel (جدول الترجمة أعلاه — لا ITEM_NAME خام)
# - توصية عملية مختصرة + اقتراح استعلام مكمّل إن كان مفيداً
# - Telegram: HTML (<b>, <i>, <code>) فقط — لا Markdown (** أو _)
# </answer>

---

## PATTERN: طلبية-شراء-ذكية
TRIGGERS: طلبية شراء, ماذا أشتري, شراء ذكي, أيام تغطية, كم يكفي المخزون, سرعة البيع, نفاد, أولويات الشراء, تحليل نواقص مع كمية مقترحة, purchase order, smart purchase, stock coverage days, suggested buy qty
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES: استخدم هذا النمط عندما يريد المستخدم كمية شراء مقترحة أو "كم يوم يكفي المخزون". للمراقبة فقط بدون كمية مقترحة استخدم نمط متابعة-النواقص. القيم الافتراضية: 60 يوم نافذة مبيعات، 30 يوم تغطية مستهدفة. تاريخ المرجع = MAX(S_DATE) وليس GETDATE(). لتغيير المعاملات: استبدل 60 أو 30 مباشرةً في SQL.
---

**الصيغ الأساسية:**
- الرصيد = SUM(ITEMS_SUB.QTY) لكل ITEM_ID
- صافي المبيعات = SALE_ITEMS مطروحاً منه R_S_ITEMS (المردودات بكمية سالبة)
- معدل يومي = SoldQty / CAST(ActiveSaleDays AS float)  ← مهم: يجب CAST لـ float منعاً لقسمة صحيحة
- أيام التغطية = StockQty / DailyRate
- كمية الشراء المقترحة = MAX(0, DailyRate × 30 − StockQty)
- الأولوية: رصيد=0 ومبيعات>0 → "نفاد" | أيام<7 → "حرج" | أيام<30 → "شراء" | غير ذلك → "كافٍ"

**الأعمدة المُخرجة:** الكود، اسم المنتج، رصيد المخزون، مبيعات آخر 60 يوم، أيام بيع فعلية، معدل يومي، أيام تغطية الرصيد، كمية الشراء المقترحة، الأولوية، آخر سعر شراء، آخر مورد

```sql
-- طلبية شراء ذكية (60 يوم نافذة، 30 يوم تغطية مستهدفة)
-- لتغيير النافذة: استبدل 60. لتغيير التغطية: استبدل 30
;WITH
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT ITEM_ID, SUM(QTY) SoldQty,
         COUNT(DISTINCT CAST(S_DATE AS date)) ActiveSaleDays,
         MAX(S_DATE) LastSaleDate
  FROM (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X GROUP BY ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE LastBuyPrice, CU.CUST_NAME LastSupplier
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.B_ITEM_ID IN (
    SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
    JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID=B2.B_ID GROUP BY BI2.ITEM_ID
  )
)
SELECT TOP 50 I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ISNULL(S.StockQty,0) AS Stock,
  SR.SoldQty, SR.ActiveSaleDays,
  CAST(SR.SoldQty / NULLIF(CAST(SR.ActiveSaleDays AS float),0) AS decimal(10,2)) AS DailyRate,
  CAST(ISNULL(S.StockQty,0) / NULLIF(SR.SoldQty / NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) AS decimal(10,1)) AS DaysCoverage,
  CASE
    WHEN ISNULL(S.StockQty,0)<=0 AND SR.SoldQty>0 THEN N'نفاد'
    WHEN ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) < 7 THEN N'حرج'
    WHEN ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) < 30 THEN N'شراء'
    ELSE N'كافٍ'
  END AS Priority,
  CAST(CASE
    WHEN (SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0))*30 - ISNULL(S.StockQty,0) < 0 THEN 0
    ELSE (SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0))*30 - ISNULL(S.StockQty,0)
  END AS decimal(10,1)) AS SuggestedBuy,
  LB.LastBuyPrice, LB.LastSupplier
FROM dbo.ITEMS I
JOIN SalesRecent SR ON I.ITEM_ID=SR.ITEM_ID
LEFT JOIN Stock S ON I.ITEM_ID=S.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID=LB.ITEM_ID
WHERE I.ITEM_INVISIBLE=0 AND SR.SoldQty>0
  AND (
    ISNULL(S.StockQty,0) <= I.MIN_LEVEL
    OR ISNULL(S.StockQty,0) = 0
    OR ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) < 30
  )
ORDER BY
  CASE WHEN ISNULL(S.StockQty,0)<=0 THEN 0 ELSE 1 END,
  DaysCoverage ASC,
  SR.SoldQty DESC;
```
ملف SQL الكامل المختبر: `reports-app/smart_purchase_order.sql`

---

## PATTERN: متابعة-النواقص
TRIGGERS: متابعة النواقص, قائمة النواقص, أصناف نافدة, تحت الحد الأدنى, فجوة المخزون, مراقبة المخزون, نواقص, shortage monitoring, items below min level, stock gap
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE
NOTES: للمراقبة فقط (الحالة + الفجوة مقابل MIN_LEVEL). لكمية الشراء المقترحة استخدم نمط طلبية-شراء-ذكية.
---

**القواعد:**
- الرصيد = SUM(ITEMS_SUB.QTY) لكل ITEM_ID
- صافي المبيعات = SALE_ITEMS مطروحاً منه R_S_ITEMS (مردودات) في آخر 60 يوم من MAX(S_DATE)
- فجوة النقص = MIN_LEVEL − Stock عندما MIN_LEVEL > 0
- الحالة: رصيد=0 + مبيعات>0 → "نفاد" | رصيد=0 → "نفاد بدون مبيعات" | رصيد≤MIN_LEVEL → "تحت الحد الأدنى" | رصيد < MIN_LEVEL×1.25 + مبيعات>0 → "قريب من النفاد"
- الترتيب: نفاد أولاً، ثم مبيعات حديثة تنازلياً

```sql
-- متابعة النواقص (60 يوم نافذة مبيعات)
-- لتغيير النافذة: استبدل 60
;WITH
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT ITEM_ID, SUM(QTY) SoldQty, MAX(S_DATE) LastSaleDate
  FROM (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X GROUP BY ITEM_ID
)
SELECT TOP 100
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ISNULL(S.StockQty,0) AS Stock,
  I.MIN_LEVEL, I.MAX_LEVEL,
  CASE WHEN I.MIN_LEVEL>0 THEN I.MIN_LEVEL - ISNULL(S.StockQty,0) ELSE 0 END AS ShortageGap,
  ISNULL(SR.SoldQty,0) AS RecentSales,
  SR.LastSaleDate,
  CASE
    WHEN ISNULL(S.StockQty,0)<=0 AND ISNULL(SR.SoldQty,0)>0 THEN N'نفاد'
    WHEN ISNULL(S.StockQty,0)<=0 THEN N'نفاد بدون مبيعات'
    WHEN I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<=I.MIN_LEVEL THEN N'تحت الحد الأدنى'
    WHEN I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<I.MIN_LEVEL*1.25 AND ISNULL(SR.SoldQty,0)>0 THEN N'قريب من النفاد'
    ELSE N'مراقبة'
  END AS ShortageStatus
FROM dbo.ITEMS I
LEFT JOIN Stock S ON I.ITEM_ID=S.ITEM_ID
LEFT JOIN SalesRecent SR ON I.ITEM_ID=SR.ITEM_ID
WHERE I.ITEM_INVISIBLE=0
  AND (
    ISNULL(S.StockQty,0) <= 0
    OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) <= I.MIN_LEVEL)
    OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) < I.MIN_LEVEL*1.25 AND ISNULL(SR.SoldQty,0)>0)
  )
ORDER BY
  CASE WHEN ISNULL(S.StockQty,0)<=0 THEN 0 ELSE 1 END,
  ISNULL(SR.SoldQty,0) DESC;
```
ملف SQL الكامل المختبر: `reports-app/shortage_tracking.sql`

---

## PATTERN: نواقص-نشطة-مورد
TRIGGERS: نواقص نشطة, منتجات ناقصة تباع, أصناف ناقصة نشطة, نواقص بمورد, shortage active selling, active shortages supplier, منتجات نافدة ومبيعات, نواقص آخر سعر شراء
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - **نشطة** = مبيعات صافية > 0 في آخر 60 يوم (من MAX(S_DATE)).
  - **ناقصة** = رصيد ≤ 0 أو ≤ MIN_LEVEL أو < MIN_LEVEL×1.25.
  - آخر سعر شراء من آخر BUY_ITEMS؛ إن لم يوجد → ITEMS.LAST_COST.
  - EXPENCES_ID=0 في GIVE ليس له علاقة — المورد من BUY_INVOICE.CUST_ID → CUSTOMERS.
  - ملف مُختبَر: `reports-app/active_shortage_tracking.sql`
  - للمراقبة بدون مورد/سعر استخدم نمط متابعة-النواقص.
---

```sql
DECLARE @DaysRecent int = 60;
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @RecentFrom date = DATEADD(day, -@DaysRecent, @AsOfDate);
;WITH Stock AS (
    SELECT ITEM_ID, SUM(ISNULL(QTY, 0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
    SELECT X.ITEM_ID, SUM(X.QTY) AS SoldQty, MAX(X.S_DATE) AS LastSaleDate
    FROM (
        SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE FROM dbo.SALE_ITEMS SI
        INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
        WHERE CAST(INV.S_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
        UNION ALL
        SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE FROM dbo.R_S_ITEMS RSI
        INNER JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
        WHERE CAST(RINV.S_R_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
    ) X GROUP BY X.ITEM_ID
),
LastBuy AS (
    SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice, CU.CUST_NAME AS LastSupplier
    FROM dbo.BUY_ITEMS BI INNER JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
    WHERE BI.B_ITEM_ID IN (
        SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
        INNER JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID = B2.B_ID GROUP BY BI2.ITEM_ID
    )
)
SELECT TOP 150
    LEFT(I.ITEM_NAME, 80) AS [اسم المنتج],
    CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [الكمية],
    CAST(COALESCE(LB.LastBuyPrice, I.LAST_COST, 0) AS decimal(18,2)) AS [آخر سعر شراء],
    ISNULL(LB.LastSupplier, N'—') AS [المورد],
    CAST(ISNULL(SR.SoldQty, 0) AS decimal(18,2)) AS [مبيعات النافذة]
FROM dbo.ITEMS I
INNER JOIN SalesRecent SR ON I.ITEM_ID = SR.ITEM_ID AND SR.SoldQty > 0
LEFT JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID = LB.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (ISNULL(S.StockQty,0) <= 0 OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) <= I.MIN_LEVEL)
       OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) < I.MIN_LEVEL*1.25))
ORDER BY CASE WHEN ISNULL(S.StockQty,0) <= 0 THEN 0 ELSE 1 END, SR.SoldQty DESC;
```

---

## PATTERN: متابعة-الديون
TRIGGERS: ديون, متابعة الديون, رصيد الزبائن, ديون الموردين, اللي لي, اللي علي, مقبوضات, مدفوعات, ما في الذمة, حسابات العملاء, debts receivable, debts payable, customer balance, supplier balance
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, BUY_INVOICE, BUY_ITEMS, B_R_INVOICE, B_R_ITEMS, TAKE, GIVE, BALANCE_EDIT
NOTES: جدول BALANCE_C فارغ في هذه القاعدة — لا تستخدمه أبداً. احسب الأرصدة دائماً من الفواتير + المدفوعات. TAKE = مقبوضات من الزبائن (T_STATUES: 0=مسودة، 1=مؤكد، 2=تم). GIVE = مدفوعات للموردين (G_STATUES: 0=مسودة، 1=مؤكد). لا تفلتر بالحالة — كل المدفوعات المدخلة محتسبة.
---

**الجداول:**
- مبيعات → SALE_INVOICE + SALE_ITEMS (قيمة السطر = QTY×PRICE)
- مردودات مبيعات → R_S_INVOICE + R_S_ITEMS
- مقبوضات من الزبائن → TAKE (T_VALUE, CUST_ID, T_DATE)
- مشتريات → BUY_INVOICE + BUY_ITEMS
- مردودات مشتريات → B_R_INVOICE + B_R_ITEMS
- مدفوعات للموردين → GIVE (G_VALUE, CUST_ID, G_DATE)
- تسويات/رصيد افتتاحي → BALANCE_EDIT (BL_DEBIT, BL_CREDIT لكل CUST_ID)

**الصيغ:**
- لي (زبون مدين) CUST_CUSTOM=1: Remaining = Sales − SaleReturns − TAKE + AdjBalance
- علي (مورد دائن) CUST_VENDOR=1: Remaining = Buys − BuyReturns − GIVE + AdjBalance
- فلتر: Remaining >= 1
- الترتيب: نوع الدين، ثم Remaining تنازلياً

```sql
-- متابعة الديون: "لي" (زبائن مدينون) و"علي" (موردون دائنون)
;WITH
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) SalesValue, MAX(SI.S_DATE) LastSaleDate
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) ReturnValue
  FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) PaidValue, MAX(T_DATE) LastTakeDate
  FROM dbo.TAKE GROUP BY CUST_ID
),
BuyTot AS (
  SELECT B.CUST_ID, SUM(BI.QTY*BI.PRICE) BuyValue, MAX(B.B_DATE) LastBuyDate
  FROM dbo.BUY_INVOICE B JOIN dbo.BUY_ITEMS BI ON B.B_ID=BI.B_ID GROUP BY B.CUST_ID
),
BuyReturnTot AS (
  SELECT BR.CUST_ID, SUM(BRI.QTY*BRI.PRICE) ReturnValue
  FROM dbo.B_R_INVOICE BR JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID=BRI.B_R_ID GROUP BY BR.CUST_ID
),
GiveTot AS (
  SELECT CUST_ID, SUM(G_VALUE) PaidValue, MAX(G_DATE) LastGiveDate
  FROM dbo.GIVE GROUP BY CUST_ID
),
Receivables AS (
  SELECT N'لي — زبون مدين' AS DebtType, C.CUST_NO, C.CUST_NAME,
    ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS Remaining,
    ISNULL(ST.SalesValue,0) AS TotalMovement, ISNULL(TT.PaidValue,0) AS TotalSettled,
    C.CUST_MAX_DEBIT, ST.LastSaleDate, TT.LastTakeDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN SaleTot ST ON C.CUST_ID=ST.CUST_ID
  LEFT JOIN SaleReturnTot SRT ON C.CUST_ID=SRT.CUST_ID
  LEFT JOIN TakeTot TT ON C.CUST_ID=TT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
  WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0
    AND ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) >= 1
),
Payables AS (
  SELECT N'علي — مورد دائن' AS DebtType, C.CUST_NO, C.CUST_NAME,
    ISNULL(BT.BuyValue,0)-ISNULL(BRT.ReturnValue,0)-ISNULL(GT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS Remaining,
    ISNULL(BT.BuyValue,0) AS TotalMovement, ISNULL(GT.PaidValue,0) AS TotalSettled,
    C.CUST_MAX_DEBIT, BT.LastBuyDate, GT.LastGiveDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN BuyTot BT ON C.CUST_ID=BT.CUST_ID
  LEFT JOIN BuyReturnTot BRT ON C.CUST_ID=BRT.CUST_ID
  LEFT JOIN GiveTot GT ON C.CUST_ID=GT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
  WHERE C.CUST_VENDOR=1 AND C.CUST_INVISIBLE=0
    AND ISNULL(BT.BuyValue,0)-ISNULL(BRT.ReturnValue,0)-ISNULL(GT.PaidValue,0)+ISNULL(BA.AdjBalance,0) >= 1
)
SELECT TOP 100
  DebtType, CUST_NO, CUST_NAME,
  CAST(Remaining AS decimal(18,2)) AS Remaining,
  CAST(TotalMovement AS decimal(18,2)) AS TotalMovement,
  CAST(TotalSettled AS decimal(18,2)) AS TotalSettled,
  CUST_MAX_DEBIT, LastSaleDate, LastTakeDate
FROM (
  SELECT DebtType,CUST_NO,CUST_NAME,Remaining,TotalMovement,TotalSettled,
         CUST_MAX_DEBIT,LastSaleDate,LastTakeDate FROM Receivables
  UNION ALL
  SELECT DebtType,CUST_NO,CUST_NAME,Remaining,TotalMovement,TotalSettled,
         CUST_MAX_DEBIT,LastBuyDate,LastGiveDate FROM Payables
) D
ORDER BY DebtType, Remaining DESC;
```
ملف SQL الكامل المختبر: `reports-app/debts_tracking.sql`

---

## PATTERN: ديون-وسلف-ومواعيد
TRIGGERS: ديون وسلف, سلف, قرض, مواعيد الدفع, ذمة, payment schedule, advances
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_*, BUY_*, TAKE, GIVE, BALANCE_EDIT, SALARIES
NOTES: 3 أجزاء — (1) ديون لي/علي (2) عمر الذمة + آخر حركة (3) سلف موظفين لم تُسترد. Marketing لا جدول مواعيد — الجزء 2 بديل بالعمر. %PARTY% = فلتر اسم.
---

```sql
;WITH
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT, 0)) - SUM(ISNULL(BL_CREDIT, 0)) AS AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY * SI2.PRICE) AS SalesValue, MAX(SI.S_DATE) AS LastSaleDate
  FROM dbo.SALE_INVOICE SI
  INNER JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID = SI2.S_ID
  GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY * RI.PRICE) AS ReturnValue
  FROM dbo.R_S_INVOICE R
  INNER JOIN dbo.R_S_ITEMS RI ON R.S_R_ID = RI.S_R_ID
  GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) AS PaidValue, MAX(T_DATE) AS LastTakeDate
  FROM dbo.TAKE GROUP BY CUST_ID
),
BuyTot AS (
  SELECT B.CUST_ID, SUM(BI.QTY * BI.PRICE) AS BuyValue, MAX(B.B_DATE) AS LastBuyDate
  FROM dbo.BUY_INVOICE B
  INNER JOIN dbo.BUY_ITEMS BI ON B.B_ID = BI.B_ID
  GROUP BY B.CUST_ID
),
BuyReturnTot AS (
  SELECT BR.CUST_ID, SUM(BRI.QTY * BRI.PRICE) AS ReturnValue
  FROM dbo.B_R_INVOICE BR
  INNER JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID = BRI.B_R_ID
  GROUP BY BR.CUST_ID
),
GiveTot AS (
  SELECT CUST_ID, SUM(G_VALUE) AS PaidValue, MAX(G_DATE) AS LastGiveDate
  FROM dbo.GIVE GROUP BY CUST_ID
),
Receivables AS (
  SELECT
    N'لي — زبون' AS [نوع_الذمة],
    C.CUST_NAME AS [الطرف],
    CAST(
      ISNULL(ST.SalesValue, 0) - ISNULL(SRT.ReturnValue, 0)
      - ISNULL(TT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0)
    AS decimal(18, 2)) AS [المبلغ_د_ل],
    CAST(C.CUST_MAX_DEBIT AS decimal(18, 2)) AS [حد_الائتمان]
  FROM dbo.CUSTOMERS C
  LEFT JOIN SaleTot ST ON C.CUST_ID = ST.CUST_ID
  LEFT JOIN SaleReturnTot SRT ON C.CUST_ID = SRT.CUST_ID
  LEFT JOIN TakeTot TT ON C.CUST_ID = TT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID = BA.CUST_ID
  WHERE C.CUST_CUSTOM = 1 AND C.CUST_INVISIBLE = 0
    AND ISNULL(ST.SalesValue, 0) - ISNULL(SRT.ReturnValue, 0)
        - ISNULL(TT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0) >= 1
    AND C.CUST_NAME LIKE N'%PARTY%'
),
Payables AS (
  SELECT
    N'علي — مورد' AS [نوع_الذمة],
    C.CUST_NAME AS [الطرف],
    CAST(
      ISNULL(BT.BuyValue, 0) - ISNULL(BRT.ReturnValue, 0)
      - ISNULL(GT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0)
    AS decimal(18, 2)) AS [المبلغ_د_ل],
    CAST(C.CUST_MAX_DEBIT AS decimal(18, 2)) AS [حد_الائتمان]
  FROM dbo.CUSTOMERS C
  LEFT JOIN BuyTot BT ON C.CUST_ID = BT.CUST_ID
  LEFT JOIN BuyReturnTot BRT ON C.CUST_ID = BRT.CUST_ID
  LEFT JOIN GiveTot GT ON C.CUST_ID = GT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID = BA.CUST_ID
  WHERE C.CUST_VENDOR = 1 AND C.CUST_INVISIBLE = 0
    AND ISNULL(BT.BuyValue, 0) - ISNULL(BRT.ReturnValue, 0)
        - ISNULL(GT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0) >= 1
    AND C.CUST_NAME LIKE N'%PARTY%'
)
SELECT TOP 150 * FROM (
  SELECT * FROM Receivables
  UNION ALL
  SELECT * FROM Payables
) D
ORDER BY [نوع_الذمة], [المبلغ_د_ل] DESC;
```

```sql
;WITH
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT, 0)) - SUM(ISNULL(BL_CREDIT, 0)) AS AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY * SI2.PRICE) AS SalesValue, MAX(SI.S_DATE) AS LastSaleDate
  FROM dbo.SALE_INVOICE SI
  INNER JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID = SI2.S_ID
  GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY * RI.PRICE) AS ReturnValue
  FROM dbo.R_S_INVOICE R
  INNER JOIN dbo.R_S_ITEMS RI ON R.S_R_ID = RI.S_R_ID
  GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) AS PaidValue, MAX(T_DATE) AS LastTakeDate
  FROM dbo.TAKE GROUP BY CUST_ID
),
BuyTot AS (
  SELECT B.CUST_ID, SUM(BI.QTY * BI.PRICE) AS BuyValue, MAX(B.B_DATE) AS LastBuyDate
  FROM dbo.BUY_INVOICE B
  INNER JOIN dbo.BUY_ITEMS BI ON B.B_ID = BI.B_ID
  GROUP BY B.CUST_ID
),
BuyReturnTot AS (
  SELECT BR.CUST_ID, SUM(BRI.QTY * BRI.PRICE) AS ReturnValue
  FROM dbo.B_R_INVOICE BR
  INNER JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID = BRI.B_R_ID
  GROUP BY BR.CUST_ID
),
GiveTot AS (
  SELECT CUST_ID, SUM(G_VALUE) AS PaidValue, MAX(G_DATE) AS LastGiveDate
  FROM dbo.GIVE GROUP BY CUST_ID
),
Rec AS (
  SELECT
    C.CUST_NAME,
    ISNULL(ST.SalesValue, 0) - ISNULL(SRT.ReturnValue, 0)
      - ISNULL(TT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0) AS Remaining,
    ST.LastSaleDate,
    TT.LastTakeDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN SaleTot ST ON C.CUST_ID = ST.CUST_ID
  LEFT JOIN SaleReturnTot SRT ON C.CUST_ID = SRT.CUST_ID
  LEFT JOIN TakeTot TT ON C.CUST_ID = TT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID = BA.CUST_ID
  WHERE C.CUST_CUSTOM = 1 AND C.CUST_INVISIBLE = 0
    AND ISNULL(ST.SalesValue, 0) - ISNULL(SRT.ReturnValue, 0)
        - ISNULL(TT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0) >= 1
    AND C.CUST_NAME LIKE N'%PARTY%'
),
Pay AS (
  SELECT
    C.CUST_NAME,
    ISNULL(BT.BuyValue, 0) - ISNULL(BRT.ReturnValue, 0)
      - ISNULL(GT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0) AS Remaining,
    BT.LastBuyDate,
    GT.LastGiveDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN BuyTot BT ON C.CUST_ID = BT.CUST_ID
  LEFT JOIN BuyReturnTot BRT ON C.CUST_ID = BRT.CUST_ID
  LEFT JOIN GiveTot GT ON C.CUST_ID = GT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID = BA.CUST_ID
  WHERE C.CUST_VENDOR = 1 AND C.CUST_INVISIBLE = 0
    AND ISNULL(BT.BuyValue, 0) - ISNULL(BRT.ReturnValue, 0)
        - ISNULL(GT.PaidValue, 0) + ISNULL(BA.AdjBalance, 0) >= 1
    AND C.CUST_NAME LIKE N'%PARTY%'
)
SELECT TOP 150
  [الاتجاه],
  [الطرف],
  [المبلغ_د_ل],
  [آخر_حركة],
  [آخر_سداد],
  [أيام_من_آخر_حركة]
FROM (
  SELECT
    N'تحصيل — زبون' AS [الاتجاه],
    CUST_NAME AS [الطرف],
    CAST(Remaining AS decimal(18, 2)) AS [المبلغ_د_ل],
    CONVERT(varchar(10), LastSaleDate, 103) AS [آخر_حركة],
    CONVERT(varchar(10), LastTakeDate, 103) AS [آخر_سداد],
    DATEDIFF(day, LastSaleDate, GETDATE()) AS [أيام_من_آخر_حركة]
  FROM Rec
  UNION ALL
  SELECT
    N'دفع — مورد',
    CUST_NAME,
    CAST(Remaining AS decimal(18, 2)),
    CONVERT(varchar(10), LastBuyDate, 103),
    CONVERT(varchar(10), LastGiveDate, 103),
    DATEDIFF(day, LastBuyDate, GETDATE())
  FROM Pay
) S
ORDER BY [أيام_من_آخر_حركة] DESC, [المبلغ_د_ل] DESC;
```

```sql
;WITH EmpGive AS (
  SELECT C.CUST_ID, C.CUST_NAME, SUM(G.G_VALUE) AS GivenAdv
  FROM dbo.GIVE G
  INNER JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID
  WHERE C.CUST_EMP = 1
  GROUP BY C.CUST_ID, C.CUST_NAME
),
EmpTake AS (
  SELECT C.CUST_ID, SUM(T.T_VALUE) AS Repaid
  FROM dbo.TAKE T
  INNER JOIN dbo.CUSTOMERS C ON T.CUST_ID = C.CUST_ID
  WHERE C.CUST_EMP = 1
  GROUP BY C.CUST_ID
),
SalaryDeduct AS (
  SELECT CUST_ID, SUM(ISNULL(BORROW_DISCOUNT, 0)) AS Deducted
  FROM dbo.SALARIES
  GROUP BY CUST_ID
)
SELECT
  g.CUST_NAME AS [الموظف/الطرف],
  CAST(g.GivenAdv AS decimal(18, 2)) AS [سلف_صُرفت],
  CAST(ISNULL(t.Repaid, 0) AS decimal(18, 2)) AS [مسترد_تحصيل],
  CAST(ISNULL(s.Deducted, 0) AS decimal(18, 2)) AS [مخصوم_راتب],
  CAST(g.GivenAdv - ISNULL(t.Repaid, 0) - ISNULL(s.Deducted, 0) AS decimal(18, 2)) AS [متبقي_للاسترداد]
FROM EmpGive g
LEFT JOIN EmpTake t ON g.CUST_ID = t.CUST_ID
LEFT JOIN SalaryDeduct s ON g.CUST_ID = s.CUST_ID
WHERE g.CUST_NAME LIKE N'%PARTY%'
  AND g.GivenAdv - ISNULL(t.Repaid, 0) - ISNULL(s.Deducted, 0) >= 0.01
ORDER BY [متبقي_للاسترداد] DESC;
```

---

## PATTERN: ديون-الموردين-مبسط
TRIGGERS: ديون الموردين, ديون موردين, ديون الموردين فقط, تقرير ديون الموردين, الذي علي للموردين, اللي علي للموردين, كم علي للموردين, supplier debts, supplier balances only, vendor debts simple
TABLES: CUSTOMERS, BUY_INVOICE, BUY_ITEMS, B_R_INVOICE, B_R_ITEMS, GIVE, BALANCE_EDIT
NOTES: نسخة مبسّطة من «متابعة الديون» — تعرض ديون الموردين فقط بعمودين: اسم المورد، والدين. تخدم الحالات التي لا يحتاج فيها المستخدم تفاصيل المقبوضات/التسويات/التواريخ. الصيغة هي: مشتريات − مردودات مشتريات − GIVE + تسوية BALANCE_EDIT.
---

**الأعمدة:** فقط `اسم المورد` و `الدين` — لا تضف أعمدة أخرى مهما كان السياق.

```sql
-- ديون الموردين فقط: اسم المورد + الدين (د.ل)
;WITH
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
BuyTot AS (
  SELECT B.CUST_ID, SUM(BI.QTY*BI.PRICE) AS BuyValue
  FROM dbo.BUY_INVOICE B JOIN dbo.BUY_ITEMS BI ON B.B_ID=BI.B_ID GROUP BY B.CUST_ID
),
BuyReturnTot AS (
  SELECT BR.CUST_ID, SUM(BRI.QTY*BRI.PRICE) AS ReturnValue
  FROM dbo.B_R_INVOICE BR JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID=BRI.B_R_ID GROUP BY BR.CUST_ID
),
GiveTot AS (
  SELECT CUST_ID, SUM(G_VALUE) AS PaidValue
  FROM dbo.GIVE GROUP BY CUST_ID
)
SELECT TOP 200
  C.CUST_NAME AS [اسم المورد],
  CAST(
    ISNULL(BT.BuyValue,0) - ISNULL(BRT.ReturnValue,0)
    - ISNULL(GT.PaidValue,0) + ISNULL(BA.AdjBalance,0)
    AS decimal(18,2)
  ) AS [الدين]
FROM dbo.CUSTOMERS C
LEFT JOIN BuyTot       BT  ON C.CUST_ID = BT.CUST_ID
LEFT JOIN BuyReturnTot BRT ON C.CUST_ID = BRT.CUST_ID
LEFT JOIN GiveTot      GT  ON C.CUST_ID = GT.CUST_ID
LEFT JOIN BalanceAdj   BA  ON C.CUST_ID = BA.CUST_ID
WHERE C.CUST_VENDOR = 1
  AND C.CUST_INVISIBLE = 0
  AND (ISNULL(BT.BuyValue,0) - ISNULL(BRT.ReturnValue,0)
       - ISNULL(GT.PaidValue,0) + ISNULL(BA.AdjBalance,0)) >= 1
ORDER BY [الدين] DESC;
```

---

## PATTERN: تقرير-الصلاحية
TRIGGERS: منتهية الصلاحية, صلاحية, تاريخ انتهاء, سينخلص قريباً, ستنتهي صلاحيتها, expiry report, expiring soon, expired products, expiry date
TABLES: ITEMS_SUB, ITEMS, STORES
NOTES: CATEOGRY3 هو عمود تاريخ الصلاحية (datetime) رغم اسمه المضلل. يوجد INDEX عليه. استخدمه دائماً من ITEMS_SUB. القيمة الافتراضية للإنذار المبكر: 90 يوم — عدّل الرقم مباشرةً.
---

```sql
-- المنتجات المنتهية الصلاحية حالياً (رصيد > 0)
SELECT TOP 50
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  S.CATEOGRY1 AS BatchNo,
  CAST(S.CATEOGRY3 AS date) AS ExpiryDate,
  S.QTY AS StockQty,
  ST.STORE_NAME,
  DATEDIFF(day, S.CATEOGRY3, GETDATE()) AS DaysExpired
FROM dbo.ITEMS_SUB S
JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON S.STORE_ID = ST.STORE_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CAST(S.CATEOGRY3 AS date) < CAST(GETDATE() AS date)
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
ORDER BY S.CATEOGRY3 ASC;
```

```sql
-- المنتجات التي ستنتهي صلاحيتها خلال 90 يوماً (عدّل 90 حسب الحاجة)
SELECT TOP 50
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  S.CATEOGRY1 AS BatchNo,
  CAST(S.CATEOGRY3 AS date) AS ExpiryDate,
  S.QTY AS StockQty,
  ST.STORE_NAME,
  DATEDIFF(day, GETDATE(), S.CATEOGRY3) AS DaysRemaining
FROM dbo.ITEMS_SUB S
JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON S.STORE_ID = ST.STORE_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CAST(S.CATEOGRY3 AS date) >= CAST(GETDATE() AS date)
  AND CAST(S.CATEOGRY3 AS date) <= DATEADD(day, 90, CAST(GETDATE() AS date))
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
ORDER BY S.CATEOGRY3 ASC;
```

---

## PATTERN: تقرير-الجرد-الفعلي
TRIGGERS: جرد, جرد فعلي, جرد المخزون, مقارنة الجرد, فرق الجرد, inventory audit, physical inventory, stock count, jared
TABLES: JARED_INVOICE, JARED_ITEMS, ITEMS_SUB, ITEMS, STORES
NOTES: JARED = فواتير الجرد الفعلي للمخزون. ITEMS_SUB = المخزون النظري. المقارنة بينهما تكشف الفروقات. CATEOGRY3 في JARED_ITEMS = تاريخ الصلاحية. J_STATUES=1 يعني جرد مقفل/معتمد.
---

```sql
-- آخر جرد لكل صنف مقارنةً بالمخزون الحالي
;WITH
LastJared AS (
  SELECT
    JI.ITEM_ID, JI.STORE_ID,
    JI.QTY AS CountedQty,
    CAST(JI.CATEOGRY3 AS date) AS JaredExpiry,
    J.J_DATE, J.J_REF_DISC,
    ROW_NUMBER() OVER (PARTITION BY JI.ITEM_ID, JI.STORE_ID ORDER BY J.J_DATE DESC) AS rn
  FROM dbo.JARED_ITEMS JI
  JOIN dbo.JARED_INVOICE J ON JI.J_ID = J.J_ID
  WHERE J.J_STATUES = 1
),
CurrentStock AS (
  SELECT ITEM_ID, STORE_ID, SUM(ISNULL(QTY,0)) AS SystemQty
  FROM dbo.ITEMS_SUB GROUP BY ITEM_ID, STORE_ID
)
SELECT TOP 100
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ST.STORE_NAME,
  LJ.CountedQty AS JaredQty,
  ISNULL(CS.SystemQty,0) AS SystemQty,
  ISNULL(CS.SystemQty,0) - ISNULL(LJ.CountedQty,0) AS Difference,
  CAST(LJ.J_DATE AS date) AS LastJaredDate,
  LJ.J_REF_DISC AS JaredRef
FROM LastJared LJ
JOIN dbo.ITEMS I ON LJ.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON LJ.STORE_ID = ST.STORE_ID
LEFT JOIN CurrentStock CS ON LJ.ITEM_ID = CS.ITEM_ID AND LJ.STORE_ID = CS.STORE_ID
WHERE LJ.rn = 1 AND I.ITEM_INVISIBLE = 0
ORDER BY ABS(ISNULL(CS.SystemQty,0) - ISNULL(LJ.CountedQty,0)) DESC;
```

---

---

## PATTERN: مقارنة-أسعار-موردين
TRIGGERS: مقارنة أسعار, مقارنة أسعار الموردين, أسعار الموردين لمنتج, أرخص مورد, أغلى مورد, فرق أسعار الشراء, supplier price comparison, compare supplier prices, buy price by vendor, product supplier prices
TABLES: ITEMS, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - **لصنف واحد** — مرّر `product_filter` في `run_query_pattern` أو استبدل `%PRODUCT%` بجزء من الاسم/الكود (مثل `@PREGNYL` من الشات).
  - يختار الصنف الأنسب تلقائياً (أكثر سجل شراء) إن وُجدت عدة مطابقات في `ITEMS`.
  - **المورد** = `BUY_INVOICE.CUST_ID` → `CUSTOMERS.CUST_NAME` (CUST_VENDOR=1).
  - **السعر** = `BUY_ITEMS.PRICE` — NOT `GIVE` / NOT `ITEMS.PUBLIC_PRICE`.
  - نافذة افتراضية: آخر **36 شهراً** (`@MonthsBack=36`).
  - ترتيب النتائج: **أرخص آخر سعر أولاً** (`ترتيب السعر`).
  - للـ «آخر سعر شراء فقط» بدون مقارنة → نمط `آخر-سعر-شراء-مورد`.
  - ملف مُختبَر: `reports-app/supplier_price_comparison.sql`
  - مثال TRAMADOL NORMON: سبها 44.30، الامل الشافي 47.00، التسامي 48.00 (آخر سعر).
---

```sql
DECLARE @MonthsBack int = 36;
DECLARE @RecentFrom date = DATEADD(month, -@MonthsBack, CAST(GETDATE() AS date));
;WITH Matches AS (
    SELECT I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME,
        MAX(B.B_DATE) AS LastAnyBuy, COUNT(BI.B_ITEM_ID) AS BuyLineCount
    FROM dbo.ITEMS I
    LEFT JOIN dbo.BUY_ITEMS BI ON I.ITEM_ID = BI.ITEM_ID AND BI.PRICE > 0
    LEFT JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    WHERE I.ITEM_INVISIBLE = 0
      AND (
        I.ITEM_MODEL LIKE N'%PRODUCT%'
        OR I.ITEM_NAME LIKE N'%PRODUCT%'
        OR EXISTS (SELECT 1 FROM dbo.BARCODE BC WHERE BC.ITEM_ID = I.ITEM_ID AND BC.BARCODE LIKE N'%PRODUCT%')
      )
    GROUP BY I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME
),
ProductPick AS (
    SELECT TOP 1 ITEM_ID, ITEM_MODEL, ITEM_NAME FROM Matches
    ORDER BY CASE WHEN BuyLineCount > 0 THEN 0 ELSE 1 END, BuyLineCount DESC, LastAnyBuy DESC,
        CASE WHEN ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END, ITEM_ID DESC
),
Purchases AS (
    SELECT PP.ITEM_ID, PP.ITEM_MODEL, PP.ITEM_NAME, B.CUST_ID, CU.CUST_NAME AS Supplier,
        BI.PRICE, B.B_DATE, BI.QTY, BI.B_ITEM_ID,
        ROW_NUMBER() OVER (PARTITION BY PP.ITEM_ID, B.CUST_ID ORDER BY B.B_DATE DESC, BI.B_ITEM_ID DESC) AS rn_last
    FROM ProductPick PP
    INNER JOIN dbo.BUY_ITEMS BI ON PP.ITEM_ID = BI.ITEM_ID
    INNER JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
    WHERE BI.PRICE > 0 AND CAST(B.B_DATE AS date) >= @RecentFrom
),
BySupplier AS (
    SELECT ITEM_ID, ITEM_MODEL, ITEM_NAME, CUST_ID, Supplier,
        COUNT(*) AS PurchaseCount,
        CAST(MIN(PRICE) AS decimal(18,2)) AS MinPrice,
        CAST(MAX(PRICE) AS decimal(18,2)) AS MaxPrice,
        CAST(AVG(PRICE) AS decimal(18,2)) AS AvgPrice,
        MAX(CASE WHEN rn_last = 1 THEN PRICE END) AS LastPrice,
        MAX(CASE WHEN rn_last = 1 THEN B_DATE END) AS LastBuyDate,
        MAX(CASE WHEN rn_last = 1 THEN QTY END) AS LastQty
    FROM Purchases GROUP BY ITEM_ID, ITEM_MODEL, ITEM_NAME, CUST_ID, Supplier
)
SELECT
    LEFT(ITEM_NAME, 70) AS [اسم المنتج], ITEM_MODEL AS [الكود],
    ISNULL(Supplier, N'—') AS [المورد],
    CAST(LastPrice AS decimal(18,2)) AS [آخر سعر شراء],
    CAST(LastBuyDate AS date) AS [آخر تاريخ شراء],
    CAST(LastQty AS decimal(18,2)) AS [آخر كمية],
    MinPrice AS [أقل سعر], MaxPrice AS [أعلى سعر], AvgPrice AS [متوسط السعر],
    PurchaseCount AS [عدد مرات الشراء],
    DATEDIFF(day, CAST(LastBuyDate AS date), CAST(GETDATE() AS date)) AS [أيام منذ آخر شراء],
    ROW_NUMBER() OVER (ORDER BY LastPrice ASC, Supplier) AS [ترتيب السعر]
FROM BySupplier
ORDER BY LastPrice ASC, Supplier;
```

---

## PATTERN: آخر-سعر-شراء-مورد
TRIGGERS: آخر سعر شراء, سعر شراء, last purchase price, buy price history, آخر مشتريات صنف
TABLES: BUY_ITEMS, BUY_INVOICE, CUSTOMERS, ITEMS
NOTES: آخر سعر شراء = `BUY_ITEMS.PRICE` من أحدث `B_ITEM_ID` لكل `ITEM_ID`. **لمقارنة الموردين على نفس الصنف** → `run_query_pattern("مقارنة أسعار موردين", product_filter=...)`.
---

```sql
-- آخر سعر شراء لكل صنف (مع المورد والتاريخ)
;WITH LastBuyRow AS (
  SELECT BI.ITEM_ID, MAX(BI.B_ITEM_ID) AS MaxBItemID
  FROM dbo.BUY_ITEMS BI GROUP BY BI.ITEM_ID
)
SELECT TOP 100
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  BI.PRICE AS LastBuyPrice,
  CAST(B.B_DATE AS date) AS LastBuyDate,
  CU.CUST_NAME AS Supplier,
  CAST(BI.CATEOGRY3 AS date) AS ExpiryDate
FROM LastBuyRow LBR
JOIN dbo.BUY_ITEMS BI ON LBR.MaxBItemID = BI.B_ITEM_ID
JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
JOIN dbo.ITEMS I ON BI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
WHERE I.ITEM_INVISIBLE = 0
ORDER BY B.B_DATE DESC;
```

---

## PATTERN: رواتب-الموظفين-بعد-الخصم
TRIGGERS: رواتب, مرتبات, الرواتب, راتب الموظف, كشف الرواتب, الراتب بعد الخصم, خصم السلفة, خصم الغياب, مكافأة, عمل إضافي, أجور, salaries, payroll, salary after deduction, net salary, bonus deduction
TABLES: SALARIES, CUSTOMERS, USERS, USER_TIME_SHEET, GIVE, TAKE
NOTES:
  - الموظفون مُخزَّنون في CUSTOMERS حيث CUST_EMP=1 ، وراتبهم الأساسي في EMP_SALARY.
  - جدول SALARIES يحتوي سجلات الراتب: BAISC_SALARY+OVER_TIME+BONCE = إجمالي. BORROW_DISCOUNT+PENALTY = خصومات. صافي = إجمالي - خصومات.
  - S_STATUES: 0=مسودة ، 1=مُعتمد ومصروف.
  - السلف (GIVE للموظف) تُخصم من الصافي عند حساب "المتبقي للصرف".
  - جدول USER_TIME_SHEET لتتبع حضور/انصراف الموظفين. PERIOD = ساعات الوردية. للحصول على شهر معين: YEAR(TRANS_DATE)=X AND MONTH(TRANS_DATE)=Y.
  - إذا كانت SALARIES فارغة: اعرض EMP_SALARY من CUSTOMERS مع سطر "لم تُدخل بعد" في حالة الراتب.
  - CAST(S.S_STATUES AS smallint) ضروري لتفادي overflow في CASE.
---

```sql
-- ===== كشف رواتب الموظفين الكامل بعد الخصم =====
-- يعرض: الراتب الأساسي + العمل الإضافي + المكافأة - خصم السلفة - خصم الغياب/الجزاء
-- إذا لم تُسجَّل رواتب بعد يعرض EMP_SALARY من CUSTOMERS كراتب مرجعي
;WITH
Employees AS (
    SELECT C.CUST_ID, C.CUST_NAME AS EmpName, ISNULL(C.EMP_SALARY, 0) AS BaseSalary
    FROM dbo.CUSTOMERS C WHERE C.CUST_EMP = 1
),
SalaryData AS (
    SELECT
        S.CUST_ID, S.S_M AS Mo, CAST(S.S_Y AS int) AS Yr,
        ISNULL(S.BAISC_SALARY,    0) AS BasicSalary,
        ISNULL(S.OVER_TIME,       0) AS OvertimePay,
        ISNULL(S.BONCE,           0) AS Bonus,
        ISNULL(S.BORROW_DISCOUNT, 0) AS LoanDeduct,
        ISNULL(S.PENALTY,         0) AS PenaltyDeduct,
        S.BONCE_REASON, S.PENALTY_REASON, S.NOTES,
        S.TOTAL_HOURS, S.HOUR_VALUE,
        CAST(S.S_STATUES AS smallint) AS S_STATUES,
        S.S_DATE
    FROM dbo.SALARIES S
    -- لتصفية شهر بعينه: أضف: WHERE S.S_M = MONTH(GETDATE()) AND S.S_Y = YEAR(GETDATE())
),
AdvancesGiven AS (
    SELECT G.CUST_ID, YEAR(G.G_DATE) Yr, MONTH(G.G_DATE) Mo,
           SUM(G.G_VALUE) AS AdvancesPaid
    FROM dbo.GIVE G
    JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID AND C.CUST_EMP = 1
    WHERE G.G_STATUES = 1
    GROUP BY G.CUST_ID, YEAR(G.G_DATE), MONTH(G.G_DATE)
)
SELECT
    E.EmpName                                                  AS [الموظف],
    ISNULL(SD.Yr,  YEAR(GETDATE()))                            AS [السنة],
    ISNULL(SD.Mo,  MONTH(GETDATE()))                           AS [الشهر],
    ISNULL(SD.BasicSalary, E.BaseSalary)                       AS [الراتب الأساسي],
    ISNULL(SD.OvertimePay, 0)                                  AS [العمل الإضافي],
    ISNULL(SD.Bonus, 0)                                        AS [المكافأة],
    ISNULL(SD.BasicSalary, E.BaseSalary)
      + ISNULL(SD.OvertimePay, 0)
      + ISNULL(SD.Bonus, 0)                                    AS [الراتب الإجمالي],
    ISNULL(SD.LoanDeduct,    0)                                AS [خصم السلفة/القرض],
    ISNULL(SD.PenaltyDeduct, 0)                                AS [خصم الغياب/الجزاء],
    ISNULL(SD.LoanDeduct, 0) + ISNULL(SD.PenaltyDeduct, 0)    AS [إجمالي الخصومات],
    ISNULL(SD.BasicSalary, E.BaseSalary)
      + ISNULL(SD.OvertimePay, 0)
      + ISNULL(SD.Bonus, 0)
      - ISNULL(SD.LoanDeduct, 0)
      - ISNULL(SD.PenaltyDeduct, 0)                            AS [صافي الراتب],
    ISNULL(AG.AdvancesPaid, 0)                                 AS [سلف مدفوعة مسبقاً],
    ISNULL(SD.BasicSalary, E.BaseSalary)
      + ISNULL(SD.OvertimePay, 0)
      + ISNULL(SD.Bonus, 0)
      - ISNULL(SD.LoanDeduct, 0)
      - ISNULL(SD.PenaltyDeduct, 0)
      - ISNULL(AG.AdvancesPaid, 0)                             AS [المتبقي للصرف],
    CASE ISNULL(SD.S_STATUES, CAST(99 AS smallint))
        WHEN 0 THEN 'مسودة'
        WHEN 1 THEN 'مُعتمد ومصروف'
        ELSE 'لم تُدخل بعد'
    END                                                        AS [حالة الراتب],
    ISNULL(SD.TOTAL_HOURS, 0)                                  AS [ساعات العمل المسجلة],
    ISNULL(SD.HOUR_VALUE,  0)                                  AS [قيمة الساعة],
    ISNULL(SD.BONCE_REASON, '')                                AS [سبب المكافأة],
    ISNULL(SD.PENALTY_REASON, '')                              AS [سبب الخصم],
    ISNULL(SD.NOTES, '')                                       AS [ملاحظات]
FROM Employees E
LEFT JOIN SalaryData    SD ON E.CUST_ID = SD.CUST_ID
LEFT JOIN AdvancesGiven AG ON E.CUST_ID = AG.CUST_ID
                           AND AG.Yr = ISNULL(SD.Yr, YEAR(GETDATE()))
                           AND AG.Mo = ISNULL(SD.Mo, MONTH(GETDATE()))
ORDER BY [السنة] DESC, [الشهر] DESC, E.EmpName;
```

```sql
-- ===== تقرير الحضور والساعات من USER_TIME_SHEET =====
-- مفيد لحساب ساعات العمل الفعلية لكل موظف في شهر معين
-- TRANS_FLAG = دخول/خروج  |  PERIOD = ساعات الوردية (حضور → خروج)
;WITH
MonthlyHours AS (
    SELECT
        TS.USERS_ID,
        YEAR(TS.TRANS_DATE)  AS Yr,
        MONTH(TS.TRANS_DATE) AS Mo,
        SUM(CASE WHEN TS.PERIOD > 0 THEN TS.PERIOD ELSE 0 END) AS TotalHoursWorked,
        COUNT(CASE WHEN TS.PERIOD > 0 THEN 1 END)              AS ShiftsCount
    FROM dbo.USER_TIME_SHEET TS
    WHERE YEAR(TS.TRANS_DATE)  = YEAR(GETDATE())    -- عدّل السنة إن أردت
      AND MONTH(TS.TRANS_DATE) = MONTH(GETDATE())   -- عدّل الشهر إن أردت
    GROUP BY TS.USERS_ID, YEAR(TS.TRANS_DATE), MONTH(TS.TRANS_DATE)
)
SELECT
    U.FULL_NAME                                                AS [الموظف],
    MH.Yr                                                      AS [السنة],
    MH.Mo                                                      AS [الشهر],
    CAST(MH.TotalHoursWorked AS decimal(10,2))                 AS [إجمالي ساعات العمل],
    MH.ShiftsCount                                             AS [عدد الورديات],
    CAST(MH.TotalHoursWorked / NULLIF(MH.ShiftsCount, 0)
         AS decimal(10,2))                                     AS [متوسط ساعات الوردية],
    -- احتساب الساعات الإضافية (معيار 8 ساعة/يوم × 26 يوم = 208 ساعة شهرياً)
    CASE WHEN MH.TotalHoursWorked > 208
         THEN CAST(MH.TotalHoursWorked - 208 AS decimal(10,2))
         ELSE 0 END                                            AS [ساعات إضافية],
    -- ساعات الغياب (أقل من المعيار)
    CASE WHEN MH.TotalHoursWorked < 208
         THEN CAST(208 - MH.TotalHoursWorked AS decimal(10,2))
         ELSE 0 END                                            AS [ساعات غياب]
FROM MonthlyHours MH
JOIN dbo.USERS U ON MH.USERS_ID = U.USERS_ID
ORDER BY MH.TotalHoursWorked DESC;
```

---

## PATTERN: المصروفات-والنفقات-التشغيلية
TRIGGERS: مصروفات, نفقات, مصاريف تشغيلية, أنواع المصروفات, expenses, operational expenses, expense categories
TABLES: EXPENCES, EXPENCES_INVOICE, GIVE
NOTES: المصروفات تُسجَّل عبر GIVE مع EXPENCES_ID. GIVE.CUST_ID=0 يعني مصروف بدون طرف. EXPENCES_ID=0 = مدفوعات لزبائن/موردين عادية. G_TYPE: 0=عادي، 1=مصروف تشغيلي.
---

```sql
-- مدفوعات GIVE التي هي مصروفات (EXPENCES_ID > 0) لشهر محدد
SELECT TOP 100
  G.G_ID,
  CAST(G.G_DATE AS date)    AS [تاريخ الصرف],
  G.G_VALUE                  AS [المبلغ],
  G.G_DISC                   AS [البيان],
  CU.CUST_NAME               AS [المستفيد],
  E.EXPENSE_DISC             AS [نوع المصروف],
  U.FULL_NAME                AS [أدخله]
FROM dbo.GIVE G
LEFT JOIN dbo.CUSTOMERS CU ON G.CUST_ID = CU.CUST_ID
LEFT JOIN dbo.EXPENCES E   ON G.EXPENCES_ID = E.EXPENCES_ID
LEFT JOIN dbo.USERS U      ON G.USERS_ID = U.USERS_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND MONTH(G.G_DATE) = MONTH(GETDATE())   -- عدّل الشهر
  AND YEAR(G.G_DATE)  = YEAR(GETDATE())    -- عدّل السنة
ORDER BY G.G_DATE DESC;
```

```sql
-- ملخص المصروفات حسب النوع في فترة محددة
SELECT
  ISNULL(E.EXPENSE_DISC, 'غير محدد')  AS [نوع المصروف],
  COUNT(*)                              AS [عدد العمليات],
  CAST(SUM(G.G_VALUE) AS decimal(18,2)) AS [الإجمالي]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND G.G_DATE >= '2026-01-01'    -- عدّل بداية الفترة
  AND G.G_DATE <  '2026-06-01'    -- عدّل نهاية الفترة
GROUP BY E.EXPENCES_ID, E.EXPENSE_DISC
ORDER BY [الإجمالي] DESC;
```

---

## PATTERN: ملخص-مالي-شهري
TRIGGERS: ديون ومصاريف, مصاريف شهرية, الديون والمصاريف, رواتب شهرية, مصاريف تشغيلية شهرية, ديون الزبائن, اللي لي على الزبائن, monthly finances, debts and expenses
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, TAKE, GIVE, BALANCE_EDIT, SALARIES, EXPENCES
NOTES:
  - ⚠️ BALANCE_C فارغ (0 صف) — لا تستخدمه أبداً للديون.
  - **ديون لي (زبائن):** مبيعات − مردودات − TAKE + BALANCE_EDIT حيث CUST_CUSTOM=1 والرصيد > 0.
  - **رواتب مُسدَّدة:** جدول SALARIES فارغ — المدفوعات الفعلية في `dbo.GIVE` WHERE `EXPENCES_ID = 1` (مصاريف رواتب). البيان في `G_DISC` (مثل: مرتب عمر شهر 4).
  - **مصاريف تشغيلية/خاصة:** `dbo.GIVE` WHERE `EXPENCES_ID > 0` AND `EXPENCES_ID <> 1` AND `G_STATUES = 1`.
  - **⚠️ EXPENCES_ID = 0** = دفعات موردين/فواتير شراء — **ليست** مصاريف تشغيلية.
  - ملف مُختبَر: `reports-app/monthly_expenses_tracking.sql`
  - نفّذ 4 أقسام: [1] ديون، [2] رواتب مُسدَّدة، [3] مصاريف خاصة، [4] ملخص.
---

```sql
-- [1] أعلى 20 زبون مدين (لي — الديون التي لك عليهم)
DECLARE @MinBalance float = 1;
;WITH BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) SalesValue
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) ReturnValue
  FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) PaidValue FROM dbo.TAKE GROUP BY CUST_ID
)
SELECT TOP 20
  C.CUST_NAME AS [الزبون],
  CAST(ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS decimal(18,2)) AS [الرصيد المتبقي],
  CAST(ISNULL(ST.SalesValue,0) AS decimal(18,2)) AS [إجمالي المبيعات],
  CAST(ISNULL(TT.PaidValue,0) AS decimal(18,2)) AS [إجمالي المقبوضات]
FROM dbo.CUSTOMERS C
LEFT JOIN SaleTot ST ON C.CUST_ID=ST.CUST_ID
LEFT JOIN SaleReturnTot SRT ON C.CUST_ID=SRT.CUST_ID
LEFT JOIN TakeTot TT ON C.CUST_ID=TT.CUST_ID
LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0
  AND ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) >= @MinBalance
ORDER BY [الرصيد المتبقي] DESC;
```

```sql
-- [2] إيصالات رواتب مُسدَّدة في الشهر (SALARIES فارغ → GIVE EXPENCES_ID=1)
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());
SELECT
    CAST(G.G_DATE AS date) AS [تاريخ الصرف],
    ISNULL(G.G_NO, CAST(G.G_ID AS varchar(20))) AS [رقم الإيصال],
    ISNULL(NULLIF(LTRIM(RTRIM(C.CUST_NAME)), N''), G.G_DISC) AS [المستفيد],
    G.G_DISC AS [البيان],
    CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ]
FROM dbo.GIVE G
LEFT JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID = 1
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
ORDER BY G.G_DATE DESC;
```

```sql
-- [3] مصاريف تشغيلية/خاصة مُسجَّلة في الشهر (غير رواتب، EXPENCES_ID<>0)
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());
SELECT
    CAST(G.G_DATE AS date) AS [التاريخ],
    ISNULL(G.G_NO, CAST(G.G_ID AS varchar(20))) AS [رقم الإيصال],
    ISNULL(E.EXPENSE_DISC, N'غير مصنف') AS [نوع المصروف],
    G.G_DISC AS [البيان],
    CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND G.EXPENCES_ID <> 1
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
ORDER BY G.G_DATE DESC;
```

```sql
-- [4] ملخص مصاريف الشهر حسب النوع
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());
SELECT
    ISNULL(E.EXPENSE_DISC, N'غير مصنف') AS [نوع المصروف],
    COUNT(*) AS [عدد العمليات],
    CAST(SUM(G.G_VALUE) AS decimal(18,2)) AS [الإجمالي]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
GROUP BY E.EXPENCES_ID, E.EXPENSE_DISC
ORDER BY [الإجمالي] DESC;
```

---

## PATTERN: أفضل-عملاء-مبيعات
TRIGGERS: أفضل عملاء, أكثر عملاء مبيعاً, أعلى عملاء, ترتيب العملاء, مبيعات العملاء, تقرير مبيعات عملاء, top customers, best customers, customer sales ranking, زبائن الأكثر شراء
TABLES: SALE_INVOICE, SALE_ITEMS, CUSTOMERS
NOTES: **عملاء (زبائن) — ليس منتجات.** استخدم MAX(S_DATE) كمرجع. الترتيب حسب SUM(QTY*PRICE). النافذة الافتراضية 30 يوماً من آخر يوم مبيعات.
---

```sql
DECLARE @LastSaleDay date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @FromDate date = DATEADD(day, -30, @LastSaleDay);
SELECT TOP 20
  ISNULL(CAST(INV.CUST_ID AS varchar(20)), N'-') AS [رقم_العميل],
  ISNULL(NULLIF(LTRIM(RTRIM(INV.CUST_NAME)), N''), ISNULL(C.CUST_NAME, N'غير محدد')) AS [اسم_العميل],
  COUNT(DISTINCT INV.S_ID) AS [عدد_الفواتير],
  CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إجمالي_المبيعات_د_ل]
FROM dbo.SALE_INVOICE INV
INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
LEFT JOIN dbo.CUSTOMERS C ON INV.CUST_ID = C.CUST_ID
WHERE CAST(INV.S_DATE AS date) BETWEEN @FromDate AND @LastSaleDay
  AND (C.CUST_ID IS NULL OR C.CUST_CUSTOM = 1)
GROUP BY INV.CUST_ID, ISNULL(NULLIF(LTRIM(RTRIM(INV.CUST_NAME)), N''), ISNULL(C.CUST_NAME, N'غير محدد'))
HAVING SUM(SI.QTY * SI.PRICE) > 0
ORDER BY [إجمالي_المبيعات_د_ل] DESC;
```

---

## PATTERN: تحليل-المبيعات-والربحية
TRIGGERS: تحليل مبيعات, إيرادات, ربحية, هامش الربح, أفضل المنتجات مبيعاً, أكثر المنتجات ربحاً, sales analysis, revenue, profit margin, top sellers, best selling
TABLES: SALE_INVOICE, SALE_ITEMS, ITEMS, R_S_INVOICE, R_S_ITEMS
NOTES: هامش الربح = (Revenue - Cost) / Revenue × 100. صافي المبيعات يطرح المردودات. SALE_ITEMS لا يحتوي S_DATE — الربط بـ SALE_INVOICE ضروري للتصفية بالتاريخ. S_STATUES في SALE_INVOICE: لا تفلتر — جميع الحالات محتسبة ما لم يطلب المستخدم غير ذلك.
---

```sql
-- أفضل 20 منتجاً من حيث الإيرادات والربحية (عدّل التواريخ حسب الحاجة)
;WITH
NetSales AS (
  SELECT SI.ITEM_ID,
    SUM(SI.QTY * SI.PRICE) AS Revenue,
    SUM(SI.QTY) AS UnitsSold,
    SUM(SI.QTY * ISNULL(SI.AVER_COST,0)) AS TotalCost
  FROM dbo.SALE_ITEMS SI
  JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  WHERE CAST(INV.S_DATE AS date) BETWEEN '2026-01-01' AND '2026-04-07'
  GROUP BY SI.ITEM_ID
),
NetReturns AS (
  SELECT RSI.ITEM_ID,
    SUM(RSI.QTY * RSI.PRICE) AS ReturnRevenue,
    SUM(RSI.QTY) AS UnitsReturned
  FROM dbo.R_S_ITEMS RSI
  JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
  WHERE CAST(RINV.S_R_DATE AS date) BETWEEN '2026-01-01' AND '2026-04-07'
  GROUP BY RSI.ITEM_ID
)
SELECT TOP 20
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  NS.UnitsSold - ISNULL(NR.UnitsReturned,0) AS NetUnits,
  CAST(NS.Revenue - ISNULL(NR.ReturnRevenue,0) AS decimal(18,2)) AS NetRevenue,
  CAST(NS.TotalCost AS decimal(18,2)) AS EstimatedCost,
  CAST(NS.Revenue - ISNULL(NR.ReturnRevenue,0) - NS.TotalCost AS decimal(18,2)) AS GrossProfit,
  CAST(
    CASE WHEN NS.Revenue > 0
      THEN (NS.Revenue - ISNULL(NR.ReturnRevenue,0) - NS.TotalCost) / NS.Revenue * 100
      ELSE 0
    END AS decimal(10,1)
  ) AS ProfitMarginPct
FROM NetSales NS
JOIN dbo.ITEMS I ON NS.ITEM_ID = I.ITEM_ID
LEFT JOIN NetReturns NR ON NS.ITEM_ID = NR.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
ORDER BY GrossProfit DESC;
```

---

## PATTERN: حركة-صنف-تفصيلية
TRIGGERS: حركة صنف, سجل صنف, تاريخ صنف, حركة مستودع, product movement, item history, stock movement, item ledger
TABLES: SALE_INVOICE, SALE_ITEMS, BUY_INVOICE, BUY_ITEMS, R_S_INVOICE, R_S_ITEMS, B_R_INVOICE, B_R_ITEMS, SPOIL_INVOICE, SPOIL_ITEMS, ITEMS
NOTES: يجمع كل حركات صنف واحد. استبدل %اسم أو كود المنتج% بالكلمة المطلوبة. الفترة افتراضية 180 يوم — عدّل حسب الحاجة.
---

```sql
-- الحركة الكاملة لصنف واحد (آخر 180 يوم — استبدل %الاسم%  بالكلمة المطلوبة)
SELECT TOP 200 MovType, TxDate, DocRef, QtyIn, QtyOut, Price, RelatedParty, EnteredBy
FROM (
  SELECT N'شراء' AS MovType, CAST(B.B_DATE AS date) TxDate, ISNULL(B.S_REF_NO,'') AS DocRef,
    BI.QTY AS QtyIn, 0 AS QtyOut, BI.PRICE, CU.CUST_NAME AS RelatedParty, U.FULL_NAME AS EnteredBy
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  JOIN dbo.ITEMS I ON BI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  LEFT JOIN dbo.USERS U ON B.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND B.B_DATE >= DATEADD(day,-180,GETDATE())
  UNION ALL
  SELECT N'بيع', CAST(INV.S_DATE AS date), CAST(INV.S_ID AS varchar(20)),
    0, SI.QTY, SI.PRICE, INV.CUST_NAME, U.FULL_NAME
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  JOIN dbo.ITEMS I ON SI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND INV.S_DATE >= DATEADD(day,-180,GETDATE())
  UNION ALL
  SELECT N'مردود بيع', CAST(RINV.S_R_DATE AS date), CAST(RINV.S_R_ID AS varchar(20)),
    RSI.QTY, 0, RSI.PRICE, RINV.CUST_NAME, U.FULL_NAME
  FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
  JOIN dbo.ITEMS I ON RSI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.USERS U ON RINV.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND RINV.S_R_DATE >= DATEADD(day,-180,GETDATE())
  UNION ALL
  SELECT N'تالف', CAST(SP.SP_DATE AS date), ISNULL(SP.SP_NOTE,''),
    0, SPI.QTY, SPI.PRICE, N'إتلاف', U.FULL_NAME
  FROM dbo.SPOIL_ITEMS SPI JOIN dbo.SPOIL_INVOICE SP ON SPI.SP_ID=SP.SP_ID
  JOIN dbo.ITEMS I ON SPI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.USERS U ON SP.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND SP.SP_DATE >= DATEADD(day,-180,GETDATE())
) AllMovements
ORDER BY TxDate DESC, MovType;
```

---

## PATTERN: مبيعات-آخر-يوم-موظف
TRIGGERS: مبيعات آخر يوم, آخر يوم فيه مبيعات, آخر يوم مبيعات, مبيعات آخر يوم لكل موظف, last sale day, last day with sales, إيرادات آخر يوم, مبيعات الموظفين آخر يوم
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
NOTES:
  - **@LastSaleDay = CAST(MAX(S_DATE) AS date) FROM SALE_INVOICE** — لا GETDATE() ولا تاريخ ثابت 2026-04-07.
  - الإيراد = SUM(SI.QTY * SI.PRICE). SALE_ITEMS لا يحتوي S_DATE.
  - صف «═══ الإجمالي ═══» في النهاية = مجموع كل الموظفين.
  - ملف مُختبَر: `reports-app/last_sale_day_by_employee.sql`
---

```sql
DECLARE @LastSaleDay date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
;WITH EmpSales AS (
    SELECT ISNULL(U.FULL_NAME, N'غير محدد') AS [الموظف],
        COUNT(DISTINCT INV.S_ID) AS [عدد الفواتير],
        CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إيرادات], 0 AS SortOrder
    FROM dbo.SALE_INVOICE INV
    INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
    LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
    WHERE CAST(INV.S_DATE AS date) = @LastSaleDay
    GROUP BY U.USERS_ID, U.FULL_NAME
),
Grand AS (
    SELECT N'═══ الإجمالي ═══' AS [الموظف],
        COUNT(DISTINCT INV.S_ID) AS [عدد الفواتير],
        CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إيرادات], 1 AS SortOrder
    FROM dbo.SALE_INVOICE INV
    INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
    WHERE CAST(INV.S_DATE AS date) = @LastSaleDay
)
SELECT @LastSaleDay AS [تاريخ آخر مبيعات], [الموظف], [عدد الفواتير], [إيرادات]
FROM (SELECT [الموظف], [عدد الفواتير], [إيرادات], SortOrder FROM EmpSales
      UNION ALL SELECT [الموظف], [عدد الفواتير], [إيرادات], SortOrder FROM Grand) X
ORDER BY SortOrder, [إيرادات] DESC;
```

---

## PATTERN: آخر-منتجات-بيعت-اليوم
TRIGGERS: آخر منتجات بيعت اليوم, منتجات بيعت اليوم, الأصناف المباعة اليوم, آخر الأصناف المباعة, ماذا بيع اليوم, آخر مبيعات اليوم, last products sold today, products sold today, what sold today, recent sales today, آخر بنود مبيعات اليوم
TABLES: SALE_ITEMS, SALE_INVOICE, ITEMS
VIEWS: **dbo.SALE_ITEMS_INVOICE_VIEW** (مفضل — فيه S_DATE, ITEM_NAME, QTY, PRICE, FULL_NAME, CUST_NAME, TRAN_NO)
NOTES:
  - **سؤال «منتجات/أصناف بيعت اليوم» = هذا النمط** — ليس «مبيعات يومية موظف» (ذلك تجميع حسب الموظف).
  - **SALE_ITEMS لا يحتوي S_DATE** — لا تستعلم التاريخ من SALE_ITEMS مباشرة؛ استخدم VIEW أو JOIN إلى SALE_INVOICE.
  - **@SaleDay:** `CAST(GETDATE() AS date)` عندما يقصد المستخدم «اليوم» تقويمياً.
  - إن كانت النتيجة **فارغة** والمستخدم يريد آخر يوم فيه مبيعات → غيّر `@SaleDay` إلى `(SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE)`.
  - استبعد الملغاة: `S_STATUES <> 2` (2 = ملغاة في SALE_INVOICE).
  - إيراد السطر = `QTY * PRICE`. الترتيب: **الأحدث أولاً** (`S_DATE DESC`, `S_ITEM_ID DESC`).
  - أعمدة VIEW المفيدة: S_DATE, S_ITEM_ID, S_ID, TRAN_NO, ITEM_MODEL, ITEM_NAME, UNIT_DISC, QTY, PRICE, CUST_NAME, FULL_NAME (موظف), STORE_NAME.
---

```sql
DECLARE @SaleDay date = CAST(GETDATE() AS date);

SELECT TOP 100
    CAST(V.S_DATE AS datetime) AS [وقت_البيع],
    ISNULL(CAST(V.TRAN_NO AS nvarchar(20)), CAST(V.S_ID AS nvarchar(20))) AS [رقم_الفاتورة],
    ISNULL(CAST(V.ITEM_MODEL AS nvarchar(50)), N'') AS [كود_الصنف],
    V.ITEM_NAME AS [اسم_المنتج],
    ISNULL(V.UNIT_DISC, N'') AS [الوحدة],
    CAST(V.QTY AS decimal(18,2)) AS [الكمية],
    CAST(V.PRICE AS decimal(18,2)) AS [السعر],
    CAST(V.QTY * V.PRICE AS decimal(18,2)) AS [إجمالي_السطر],
    ISNULL(V.CUST_NAME, N'') AS [العميل],
    ISNULL(V.FULL_NAME, N'غير محدد') AS [الموظف],
    ISNULL(V.STORE_NAME, N'') AS [المخزن]
FROM dbo.SALE_ITEMS_INVOICE_VIEW V
WHERE CAST(V.S_DATE AS date) = @SaleDay
  AND ISNULL(V.S_STATUES, 0) <> 2
ORDER BY V.S_DATE DESC, V.S_ITEM_ID DESC;
```

---

## PATTERN: مبيعات-يومية-لكل-موظف
TRIGGERS: مبيعات يومية موظف, لخص المبيعات اليومية, إجمالي مبيعات كل موظف, مبيعات كل يوم بالموظف, daily sales by employee, employee daily summary, أداء يومي موظف, مبيعات الموظفين يومياً, لخص لي المبيعات اليومية
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
VIEWS: SALE_ITEMS_INVOICE_VIEW
NOTES: **لا subquery يجمع PRICE وحده.** الإيراد = SUM(QTY*PRICE). SALE_ITEMS **لا** S_DATE — استخدم INV.S_DATE. الموظف = SALE_INVOICE.USERS_ID → USERS.FULL_NAME. **التاريخ:** استخدم MAX(S_DATE) كمرجع — لا GETDATE() وحده.
---

```sql
-- مبيعات يومية لكل موظف — آخر 7 أيام من آخر يوم مبيعات
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @FromDate date = DATEADD(day, -7, @AsOfDate);
SELECT
  CAST(V.S_DATE AS date) AS SaleDay,
  ISNULL(V.FULL_NAME, N'غير محدد') AS EmployeeName,
  COUNT(DISTINCT V.S_ID) AS InvoiceCount,
  CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS TotalRevenue
FROM dbo.SALE_ITEMS_INVOICE_VIEW V
WHERE CAST(V.S_DATE AS date) BETWEEN @FromDate AND @AsOfDate
GROUP BY CAST(V.S_DATE AS date), V.USERS_ID, V.FULL_NAME
ORDER BY SaleDay DESC, TotalRevenue DESC;
```

```sql
-- بديل: جداول أساسية + CTE (نفس النتيجة)
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @FromDate date = DATEADD(day, -7, @AsOfDate);
;WITH LineSales AS (
  SELECT CAST(INV.S_DATE AS date) AS SaleDay, INV.USERS_ID,
    ISNULL(U.FULL_NAME, N'غير محدد') AS EmployeeName, SI.S_ID, SI.QTY * SI.PRICE AS LineValue
  FROM dbo.SALE_ITEMS SI
  INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
  WHERE CAST(INV.S_DATE AS date) BETWEEN @FromDate AND @AsOfDate
)
SELECT SaleDay, EmployeeName, COUNT(DISTINCT S_ID) AS InvoiceCount,
  CAST(SUM(LineValue) AS decimal(18,2)) AS TotalRevenue
FROM LineSales
GROUP BY SaleDay, USERS_ID, EmployeeName
ORDER BY SaleDay DESC, TotalRevenue DESC;
```

---

## PATTERN: مبيعات-موظف-مندوب
TRIGGERS: مبيعات موظف, أداء مبيعات, مبيعات المندوبين, إنجاز فريق المبيعات, مبيعات بالموظف, sales by employee, sales rep performance, user sales
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
VIEWS: SALE_ITEMS_INVOICE_VIEW
NOTES: USERS_ID في SALE_INVOICE = من أدخل الفاتورة. **ليس** COMMISSIONER. الإيراد = SUM(QTY*PRICE) بعد JOIN — لا subquery على PRICE فقط.
---

```sql
-- مبيعات كل موظف لمجموع فترة (عدّل التواريخ)
SELECT
  ISNULL(U.FULL_NAME, N'غير محدد') AS EmployeeName,
  COUNT(DISTINCT INV.S_ID) AS InvoiceCount,
  CAST(SUM(SI.QTY) AS decimal(18,1)) AS TotalUnits,
  CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS TotalRevenue,
  CAST(MAX(INV.S_DATE) AS date) AS LastSaleDate
FROM dbo.SALE_INVOICE INV
INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
WHERE CAST(INV.S_DATE AS date) BETWEEN '2026-05-01' AND '2026-05-21'
GROUP BY U.USERS_ID, U.FULL_NAME
ORDER BY TotalRevenue DESC;
```

---

## PATTERN: تقرير-المنتجات-المجمعة
TRIGGERS: مجموعة منتجات, باقة منتجات, كوليكت, collect, bundle, product bundle, مجمعة
TABLES: COLLECT, COLLECT_DETAILS, ITEMS, UNITS
NOTES: COLLECT = مجموعات منتجات (باقات/حزم). ليس مقبوضات! COLLECT_DETAILS تحتوي محتوى كل مجموعة.
---

```sql
-- قائمة المجموعات مع محتواها والمخزون الحالي
SELECT
  C.COLLECT_NAME AS BundleName,
  I.ITEM_MODEL,
  LEFT(I.ITEM_NAME,60) AS ItemName,
  CD.QTY AS BundleQty,
  U.UNIT_DISC AS Unit,
  CD.PRICE AS ItemPrice,
  ISNULL(SUM(SUB.QTY),0) AS CurrentStock
FROM dbo.COLLECT C
JOIN dbo.COLLECT_DETAILS CD ON C.COLLECT_ID = CD.COLLECT_ID
JOIN dbo.ITEMS I ON CD.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.UNITS U ON CD.UNIT_ID = U.UNIT_ID
LEFT JOIN dbo.ITEMS_SUB SUB ON I.ITEM_ID = SUB.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
GROUP BY C.COLLECT_NAME, I.ITEM_MODEL, I.ITEM_NAME, CD.QTY, U.UNIT_DISC, CD.PRICE
ORDER BY C.COLLECT_NAME, I.ITEM_NAME;
```

---

## PATTERN: التصنيع-والتحويل
TRIGGERS: تصنيع, تركيب, تحويل مخزون, تحويل بضاعة, manufacturing, production, stock conversion, warehouse transfer
TABLES: MANF_INVOICE, MANF_F_ITEMS, MANF_T_ITEMS, TRANSFER_INVOICE, TRANSFER_ITEMS, ITEMS, STORES
NOTES: MANF = فواتير التصنيع. MANF_F_ITEMS = المواد المستهلكة (From). MANF_T_ITEMS = المنتجات الناتجة (To). TRANSFER = تحويلات بين المخازن.
---

```sql
-- آخر 20 عملية تصنيع مع المدخلات والمخرجات
SELECT TOP 20
  M.MANF_ID,
  CAST(M.MANF_DATE AS date) AS ManfDate,
  M.MANF_NOTE,
  U.FULL_NAME AS EnteredBy,
  M.MANF_STATUES,
  (SELECT STRING_AGG(CONVERT(nvarchar(200), LEFT(IF2.ITEM_NAME,30)+N' ×'+CAST(MF2.QTY AS nvarchar(20))), N', ')
   FROM dbo.MANF_F_ITEMS MF2 JOIN dbo.ITEMS IF2 ON MF2.ITEM_ID=IF2.ITEM_ID
   WHERE MF2.MANF_ID=M.MANF_ID) AS InputItems,
  (SELECT STRING_AGG(CONVERT(nvarchar(200), LEFT(IT2.ITEM_NAME,30)+N' ×'+CAST(MT2.QTY AS nvarchar(20))), N', ')
   FROM dbo.MANF_T_ITEMS MT2 JOIN dbo.ITEMS IT2 ON MT2.ITEM_ID=IT2.ITEM_ID
   WHERE MT2.MANF_ID=M.MANF_ID) AS OutputItems
FROM dbo.MANF_INVOICE M
LEFT JOIN dbo.USERS U ON M.USERS_ID = U.USERS_ID
ORDER BY M.MANF_DATE DESC;
```

```sql
-- آخر 50 تحويل بين المخازن
SELECT TOP 50
  CAST(T.TR_DATE AS date) AS TransferDate,
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,50) AS ItemName,
  TI.QTY,
  SF.STORE_NAME AS FromStore,
  ST2.STORE_NAME AS ToStore,
  T.TR_NOTE,
  U.FULL_NAME AS EnteredBy
FROM dbo.TRANSFER_ITEMS TI
JOIN dbo.TRANSFER_INVOICE T ON TI.TR_ID = T.TR_ID
JOIN dbo.ITEMS I ON TI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES SF ON TI.STORE_F_ID = SF.STORE_ID
LEFT JOIN dbo.STORES ST2 ON TI.STORE_T_ID = ST2.STORE_ID
LEFT JOIN dbo.USERS U ON T.USERS_ID = U.USERS_ID
ORDER BY T.TR_DATE DESC;
```

---

## PATTERN: دراسة-منتج-شاملة
TRIGGERS: دراسة منتج, تحليل منتج, تقرير منتج, مخزون منتج, سرعة بيع, كم يكفي المخزون, طلبية شراء لمنتج, product study, item analysis, stock runway, days of stock, recommended purchase, صنف واحد, دراسة شاملة
TABLES: ITEMS, ITEMS_SUB, STORES, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS, BARCODE, UNITS
NOTES: **لصنف واحد.** استبدل `%PRODUCT%` بالكود أو جزء من الاسم (أو مرّر product_filter في run_query_pattern). النافذة الافتراضية 60 يوم مبيعات، 30 يوم تغطية مستهدفة. يُخرج صفاً واحداً بملخص: مخزون، مبيعات، معدل يومي، أيام تغطية، كمية شراء مقترحة، آخر شراء، أقرب صلاحية.
---

**المخرجات الرئيسية:** الكود، الاسم، مخزون إجمالي، تفصيل مخازن، مبيعات الفترة، أيام بيع فعلية، معدل يومي، أيام تغطية، كمية شراء مقترحة، أولوية، آخر سعر شراء، آخر مورد، أقرب صلاحية، حد أدنى/أعلى، متوسط تكلفة.

```sql
-- دراسة منتج واحد — استبدل %PRODUCT% (أو product_filter من الأداة)
;WITH
ItemPick AS (
  SELECT TOP 1 I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME, I.MIN_LEVEL, I.MAX_LEVEL,
    I.LAST_COST, I.AVER_COST, I.PLACE
  FROM dbo.ITEMS I
  WHERE I.ITEM_INVISIBLE = 0
    AND (
      I.ITEM_MODEL LIKE N'%PRODUCT%'
      OR I.ITEM_NAME LIKE N'%PRODUCT%'
      OR EXISTS (SELECT 1 FROM dbo.BARCODE BC WHERE BC.ITEM_ID = I.ITEM_ID AND BC.BARCODE LIKE N'%PRODUCT%')
    )
  ORDER BY CASE WHEN I.ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END, I.ITEM_ID DESC
),
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT IP.ITEM_ID,
    SUM(ISNULL(SUB.QTY,0)) AS TotalStock,
    MIN(CASE WHEN ISNULL(SUB.QTY,0) > 0 AND SUB.CATEOGRY3 IS NOT NULL THEN CAST(SUB.CATEOGRY3 AS date) END) AS NearestExpiry
  FROM ItemPick IP
  LEFT JOIN dbo.ITEMS_SUB SUB ON IP.ITEM_ID = SUB.ITEM_ID
  GROUP BY IP.ITEM_ID
),
StockByStore AS (
  SELECT IP.ITEM_ID, ST.STORE_NAME, SUM(ISNULL(SUB.QTY,0)) AS StoreQty
  FROM ItemPick IP
  JOIN dbo.ITEMS_SUB SUB ON IP.ITEM_ID = SUB.ITEM_ID
  JOIN dbo.STORES ST ON SUB.STORE_ID = ST.STORE_ID
  WHERE ISNULL(SUB.QTY,0) <> 0
  GROUP BY IP.ITEM_ID, ST.STORE_NAME
),
SalesRecent AS (
  SELECT IP.ITEM_ID, SUM(X.QTY) AS SoldQty,
    COUNT(DISTINCT CAST(X.S_DATE AS date)) AS ActiveSaleDays,
    MAX(X.S_DATE) AS LastSaleDate
  FROM ItemPick IP
  JOIN (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X ON IP.ITEM_ID = X.ITEM_ID
  GROUP BY IP.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice, CU.CUST_NAME AS LastSupplier, B.B_DATE AS LastBuyDate
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
  WHERE BI.B_ITEM_ID IN (
    SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
    JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID = B2.B_ID
    JOIN ItemPick IP2 ON BI2.ITEM_ID = IP2.ITEM_ID
    GROUP BY BI2.ITEM_ID
  )
)
SELECT
  IP.ITEM_MODEL AS ItemCode,
  LEFT(IP.ITEM_NAME, 80) AS ItemName,
  ISNULL(SK.TotalStock, 0) AS StockQty,
  (SELECT STRING_AGG(CONVERT(nvarchar(120), SBS.STORE_NAME + N': ' + CAST(CAST(SBS.StoreQty AS int) AS nvarchar(20))), N' | ')
   FROM StockByStore SBS WHERE SBS.ITEM_ID = IP.ITEM_ID) AS StockByStore,
  ISNULL(SR.SoldQty, 0) AS SoldQty60d,
  ISNULL(SR.ActiveSaleDays, 0) AS ActiveSaleDays,
  CAST(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0) AS decimal(12,3)) AS DailyRate,
  CAST(ISNULL(SK.TotalStock,0) / NULLIF(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) AS decimal(12,1)) AS DaysCoverage,
  CAST(CASE
    WHEN (ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0)) * 30 - ISNULL(SK.TotalStock,0) < 0 THEN 0
    ELSE (ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0)) * 30 - ISNULL(SK.TotalStock,0)
  END AS decimal(12,1)) AS SuggestedBuyQty,
  CASE
    WHEN ISNULL(SK.TotalStock,0) <= 0 AND ISNULL(SR.SoldQty,0) > 0 THEN N'نفاد'
    WHEN ISNULL(SK.TotalStock,0) / NULLIF(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) < 7 THEN N'حرج'
    WHEN ISNULL(SK.TotalStock,0) / NULLIF(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) < 30 THEN N'شراء'
    ELSE N'كافٍ'
  END AS Priority,
  IP.MIN_LEVEL, IP.MAX_LEVEL,
  IP.LAST_COST, IP.AVER_COST,
  LB.LastBuyPrice, LB.LastSupplier, CAST(LB.LastBuyDate AS date) AS LastBuyDate,
  SK.NearestExpiry,
  SR.LastSaleDate,
  IP.PLACE
FROM ItemPick IP
LEFT JOIN Stock SK ON IP.ITEM_ID = SK.ITEM_ID
LEFT JOIN SalesRecent SR ON IP.ITEM_ID = SR.ITEM_ID
LEFT JOIN LastBuy LB ON IP.ITEM_ID = LB.ITEM_ID;
```

---

## PATTERN: تفاصيل-منتج-وحدات-أسعار
TRIGGERS: وحدات المنتج, أسعار الوحدات, باركود, price1 price2, سعر البيع, تسعير, units prices, barcode, product units, أسعار الصنف
TABLES: ITEMS, BARCODE, UNITS
NOTES: يعرض كل BARCODE (وحدة بيع) مع UNIT_DISC و PRICE1–5 و PUBLIC_PRICE. فلتر `%PRODUCT%` أو product_filter.
---

```sql
-- وحدات وأسعار بيع لصنف — استبدل %PRODUCT%
SELECT TOP 30
  I.ITEM_MODEL AS ItemCode,
  LEFT(I.ITEM_NAME, 60) AS ItemName,
  U.UNIT_DISC AS UnitName,
  B.BARCODE,
  B.UNIT_QTY,
  B.PRICE1, B.PRICE2, B.PRICE3, B.PRICE4, B.PRICE5,
  B.PUBLIC_PRICE,
  B.PRICE_LESS,
  B.QTY AS PriceBreakQty,
  CAST(B.UPDATE_DATE AS date) AS PriceUpdated
FROM dbo.ITEMS I
JOIN dbo.BARCODE B ON I.ITEM_ID = B.ITEM_ID
LEFT JOIN dbo.UNITS U ON B.UNIT_ID = U.UNIT_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (I.ITEM_MODEL LIKE N'%PRODUCT%' OR I.ITEM_NAME LIKE N'%PRODUCT%')
ORDER BY B.UNIT_QTY, U.UNIT_DISC;
```

---

## PATTERN: مبيعات-منتج-حسب-الوحدة
TRIGGERS: مبيعات الصنف بالوحدة, أي وحدة تُباع أكثر, unit mix, sales by unit for product
TABLES: SALE_ITEMS, SALE_INVOICE, ITEMS, UNITS, R_S_ITEMS, R_S_INVOICE
NOTES: توزيع المبيعات على الوحدات لصنف في آخر 90 يوم. استبدل %PRODUCT%.
---

```sql
;WITH ItemPick AS (
  SELECT TOP 1 ITEM_ID FROM dbo.ITEMS
  WHERE ITEM_INVISIBLE = 0 AND (ITEM_MODEL LIKE N'%PRODUCT%' OR ITEM_NAME LIKE N'%PRODUCT%')
  ORDER BY CASE WHEN ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END
)
SELECT U.UNIT_DISC AS UnitName,
  CAST(SUM(X.QtyNet) AS decimal(18,1)) AS NetQty,
  CAST(SUM(X.Revenue) AS decimal(18,2)) AS Revenue
FROM ItemPick IP
JOIN (
  SELECT SI.ITEM_ID, SI.UNIT_ID, SI.QTY AS QtyNet, SI.QTY * SI.PRICE AS Revenue
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  WHERE SI.ITEM_ID = (SELECT ITEM_ID FROM ItemPick)
    AND INV.S_DATE >= DATEADD(day, -90, GETDATE())
  UNION ALL
  SELECT RSI.ITEM_ID, RSI.UNIT_ID, -RSI.QTY, -RSI.QTY * RSI.PRICE
  FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
  WHERE RSI.ITEM_ID = (SELECT ITEM_ID FROM ItemPick)
    AND RINV.S_R_DATE >= DATEADD(day, -90, GETDATE())
) X ON IP.ITEM_ID = X.ITEM_ID
LEFT JOIN dbo.UNITS U ON X.UNIT_ID = U.UNIT_ID
GROUP BY U.UNIT_DISC
ORDER BY NetQty DESC;
```

---

## PATTERN: مردودات-مبيعات
TRIGGERS: مردودات مبيعات, مردود بيع, إرجاع من زبون, sales returns, return invoice, R_S, مرتجعات مبيعات
TABLES: R_S_INVOICE, R_S_ITEMS, ITEMS
NOTES: آخر 30 يوماً من MAX(S_R_DATE). القيمة = QTY×PRICE.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(S_R_DATE) AS date) AS d FROM dbo.R_S_INVOICE
)
SELECT TOP 100
  CAST(R.S_R_DATE AS date) AS [تاريخ_المردود],
  R.S_R_ID AS [رقم_المردود],
  ISNULL(R.CUST_NAME, N'—') AS [العميل],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(RSI.QTY AS decimal(18,2)) AS [الكمية],
  CAST(RSI.PRICE AS decimal(18,2)) AS [السعر],
  CAST(RSI.QTY * RSI.PRICE AS decimal(18,2)) AS [قيمة_السطر]
FROM dbo.R_S_INVOICE R
INNER JOIN dbo.R_S_ITEMS RSI ON R.S_R_ID = RSI.S_R_ID
INNER JOIN dbo.ITEMS I ON RSI.ITEM_ID = I.ITEM_ID
WHERE CAST(R.S_R_DATE AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
  AND I.ITEM_INVISIBLE = 0
ORDER BY R.S_R_DATE DESC, R.S_R_ID DESC;
```

---

## PATTERN: مردودات-مشتريات
TRIGGERS: مردودات مشتريات, مردود شراء, إرجاع لمورد, purchase returns, B_R, مرتجعات شراء
TABLES: B_R_INVOICE, B_R_ITEMS, ITEMS, CUSTOMERS
NOTES: آخر 30 يوماً من MAX(B_R_DATE). المورد = CUSTOMERS.CUST_NAME.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(B_R_DATE) AS date) AS d FROM dbo.B_R_INVOICE
)
SELECT TOP 100
  CAST(BR.B_R_DATE AS date) AS [تاريخ_المردود],
  BR.B_R_ID AS [رقم_المردود],
  ISNULL(C.CUST_NAME, N'—') AS [المورد],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(BRI.QTY AS decimal(18,2)) AS [الكمية],
  CAST(BRI.PRICE AS decimal(18,2)) AS [السعر],
  CAST(BRI.QTY * BRI.PRICE AS decimal(18,2)) AS [قيمة_السطر]
FROM dbo.B_R_INVOICE BR
INNER JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID = BRI.B_R_ID
INNER JOIN dbo.ITEMS I ON BRI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.CUSTOMERS C ON BR.CUST_ID = C.CUST_ID
WHERE CAST(BR.B_R_DATE AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
  AND I.ITEM_INVISIBLE = 0
ORDER BY BR.B_R_DATE DESC;
```

---

## PATTERN: مقبوضات-تحصيلات
TRIGGERS: مقبوضات, تحصيلات, سندات قبض, TAKE, تحصيل من زبون, collections, customer receipts, مدفوعات واردة
TABLES: TAKE, CUSTOMERS
NOTES: T_VALUE = المبلغ المحصَّل. آخر 30 يوماً من MAX(T_DATE).
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(T_DATE) AS date) AS d FROM dbo.TAKE
)
SELECT TOP 100
  CAST(T.T_DATE AS date) AS [تاريخ_التحصيل],
  T.T_ID AS [رقم_السند],
  ISNULL(C.CUST_NAME, N'—') AS [العميل],
  CAST(T.T_VALUE AS decimal(18,2)) AS [المبلغ_د_ل],
  ISNULL(T.T_NOTE, N'') AS [ملاحظة]
FROM dbo.TAKE T
LEFT JOIN dbo.CUSTOMERS C ON T.CUST_ID = C.CUST_ID
WHERE CAST(T.T_DATE AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
ORDER BY T.T_DATE DESC, T.T_ID DESC;
```

---

## PATTERN: مدفوعات-موردين-سندات
TRIGGERS: مدفوعات موردين, سندات صرف, GIVE, دفع لمورد, supplier payments, disbursements, صرف نقد
TABLES: GIVE, CUSTOMERS
NOTES: G_VALUE = المبلغ المدفوع. EXPENCES_ID=0 عادةً لدفع مورد. آخر 30 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(G_DATE) AS date) AS d FROM dbo.GIVE
)
SELECT TOP 100
  CAST(G.G_DATE AS date) AS [تاريخ_الدفع],
  G.G_ID AS [رقم_السند],
  ISNULL(C.CUST_NAME, N'—') AS [المورد],
  CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ_د_ل],
  ISNULL(G.G_NOTE, N'') AS [ملاحظة]
FROM dbo.GIVE G
LEFT JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID
WHERE CAST(G.G_DATE AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
  AND ISNULL(G.EXPENCES_ID, 0) = 0
ORDER BY G.G_DATE DESC, G.G_ID DESC;
```

---

## PATTERN: تحويلات-مخازن
TRIGGERS: تحويل مخزن, تحويل بين مخازن, نقل مخزون, warehouse transfer, stock transfer, TRANSFER
TABLES: TRANSFER_INVOICE, TRANSFER_ITEMS, ITEMS, STORES
NOTES: STORE_F_ID = من | STORE_T_ID = إلى. آخر 90 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(TR_DATE) AS date) AS d FROM dbo.TRANSFER_INVOICE
)
SELECT TOP 100
  CAST(TR.TR_DATE AS date) AS [التاريخ],
  TR.TR_ID AS [رقم_التحويل],
  ISNULL(SF.STORE_NAME, N'—') AS [من_مخزن],
  ISNULL(ST.STORE_NAME, N'—') AS [إلى_مخزن],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(TI.QTY AS decimal(18,2)) AS [الكمية]
FROM dbo.TRANSFER_INVOICE TR
INNER JOIN dbo.TRANSFER_ITEMS TI ON TR.TR_ID = TI.TR_ID
INNER JOIN dbo.ITEMS I ON TI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES SF ON TI.STORE_F_ID = SF.STORE_ID
LEFT JOIN dbo.STORES ST ON TI.STORE_T_ID = ST.STORE_ID
WHERE CAST(TR.TR_DATE AS date) >= DATEADD(day, -90, (SELECT d FROM AsOf))
  AND I.ITEM_INVISIBLE = 0
ORDER BY TR.TR_DATE DESC;
```

---

## PATTERN: أصناف-تالفة
TRIGGERS: تالف, متلف, إتلاف, spoiled, damaged stock, SPOIL, أدوية تالفة, صنف تالف
TABLES: SPOIL_INVOICE, SPOIL_ITEMS, ITEMS, STORES
NOTES: SPOIL = فواتير الإتلاف. آخر 90 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(SP_DATE) AS date) AS d FROM dbo.SPOIL_INVOICE
)
SELECT TOP 100
  CAST(SP.SP_DATE AS date) AS [تاريخ_الإتلاف],
  SP.SP_ID AS [رقم_السند],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  ISNULL(ST.STORE_NAME, N'—') AS [المخزن],
  CAST(SPI.QTY AS decimal(18,2)) AS [الكمية],
  CAST(SPI.PRICE AS decimal(18,2)) AS [التكلفة],
  CAST(SPI.QTY * SPI.PRICE AS decimal(18,2)) AS [قيمة_التالف]
FROM dbo.SPOIL_INVOICE SP
INNER JOIN dbo.SPOIL_ITEMS SPI ON SP.SP_ID = SPI.SP_ID
INNER JOIN dbo.ITEMS I ON SPI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON SPI.STORE_ID = ST.STORE_ID
WHERE CAST(SP.SP_DATE AS date) >= DATEADD(day, -90, (SELECT d FROM AsOf))
  AND I.ITEM_INVISIBLE = 0
ORDER BY SP.SP_DATE DESC;
```

---

## PATTERN: أصناف-راكة
TRIGGERS: راكد, راكدة, بطيء الحركة, slow moving, dead stock, بدون مبيعات, stock no sales, راكد بالمخزن
TABLES: ITEMS, ITEMS_SUB, SALE_INVOICE, SALE_ITEMS
NOTES: مخزون > 0 ولا مبيعات في آخر 90 يوم (صافي بعد المردودات). مرتب حسب قيمة المخزون.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY, 0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
NetSales90 AS (
  SELECT X.ITEM_ID, SUM(X.QtyNet) AS SoldQty, MAX(X.TxDate) AS LastSaleDate
  FROM (
    SELECT SI.ITEM_ID, SI.QTY AS QtyNet, INV.S_DATE AS TxDate
    FROM dbo.SALE_ITEMS SI
    INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day, -90, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI
    INNER JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day, -90, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X
  GROUP BY X.ITEM_ID
)
SELECT TOP 100
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(S.StockQty AS decimal(18,2)) AS [المخزون],
  CAST(ISNULL(NS.SoldQty, 0) AS decimal(18,2)) AS [مبيعات_90_يوم],
  CAST(NS.LastSaleDate AS date) AS [آخر_بيع],
  CAST(S.StockQty * ISNULL(I.LAST_COST, I.AVER_COST) AS decimal(18,2)) AS [قيمة_تقديرية_د_ل]
FROM dbo.ITEMS I
INNER JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
LEFT JOIN NetSales90 NS ON I.ITEM_ID = NS.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND S.StockQty > 0
  AND ISNULL(NS.SoldQty, 0) <= 0
ORDER BY S.StockQty * ISNULL(I.LAST_COST, I.AVER_COST) DESC;
```

---

## PATTERN: مقارنة-مبيعات-شهرية
TRIGGERS: مقارنة شهرية, مبيعات الشهر, الشهر الماضي, month over month, monthly comparison, نمو المبيعات, مقارنة أشهر
TABLES: SALE_INVOICE, SALE_ITEMS
NOTES: يقارن الشهر الحالي (حسب MAX(S_DATE)) بالشهر السابق. الإيراد = SUM(QTY×PRICE).
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
MonthSales AS (
  SELECT
    YEAR(CAST(INV.S_DATE AS date)) AS Y,
    MONTH(CAST(INV.S_DATE AS date)) AS M,
    CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS Revenue,
    COUNT(DISTINCT INV.S_ID) AS InvoiceCount
  FROM dbo.SALE_INVOICE INV
  INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
  WHERE CAST(INV.S_DATE AS date) >= DATEADD(month, -2, DATEFROMPARTS(YEAR((SELECT d FROM AsOf)), MONTH((SELECT d FROM AsOf)), 1))
  GROUP BY YEAR(CAST(INV.S_DATE AS date)), MONTH(CAST(INV.S_DATE AS date))
),
Cur AS (
  SELECT * FROM MonthSales
  WHERE Y = YEAR((SELECT d FROM AsOf)) AND M = MONTH((SELECT d FROM AsOf))
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
TRIGGERS: سجل عميل, مشتريات عميل, فواتير زبون, customer history, customer purchases, ماذا اشترى, تاريخ مبيعات عميل
TABLES: SALE_INVOICE, SALE_ITEMS, ITEMS, CUSTOMERS
NOTES: استبدل %CUSTOMER% بجزء من اسم العميل. آخر 180 يوماً.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
)
SELECT TOP 100
  CAST(INV.S_DATE AS date) AS [التاريخ],
  INV.S_ID AS [رقم_الفاتورة],
  ISNULL(INV.CUST_NAME, C.CUST_NAME) AS [العميل],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(SI.QTY AS decimal(18,2)) AS [الكمية],
  CAST(SI.PRICE AS decimal(18,2)) AS [السعر],
  CAST(SI.QTY * SI.PRICE AS decimal(18,2)) AS [إجمالي_السطر]
FROM dbo.SALE_INVOICE INV
INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
INNER JOIN dbo.ITEMS I ON SI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.CUSTOMERS C ON INV.CUST_ID = C.CUST_ID
WHERE CAST(INV.S_DATE AS date) >= DATEADD(day, -180, (SELECT d FROM AsOf))
  AND (
    ISNULL(INV.CUST_NAME, N'') LIKE N'%CUSTOMER%'
    OR ISNULL(C.CUST_NAME, N'') LIKE N'%CUSTOMER%'
  )
ORDER BY INV.S_DATE DESC;
```

---

## PATTERN: فواتير-شراء-حديثة
TRIGGERS: فواتير شراء, آخر مشتريات, purchase invoices, recent buys, فاتورة شراء, مشتريات حديثة
TABLES: BUY_INVOICE, BUY_ITEMS, ITEMS, CUSTOMERS
NOTES: آخر 30 يوماً من MAX(B_DATE). المورد = CUSTOMERS.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(B_DATE) AS date) AS d FROM dbo.BUY_INVOICE
)
SELECT TOP 100
  CAST(B.B_DATE AS date) AS [تاريخ_الشراء],
  B.B_ID AS [رقم_الفاتورة],
  ISNULL(C.CUST_NAME, N'—') AS [المورد],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(BI.QTY AS decimal(18,2)) AS [الكمية],
  CAST(BI.PRICE AS decimal(18,2)) AS [سعر_الشراء],
  CAST(BI.QTY * BI.PRICE AS decimal(18,2)) AS [قيمة_السطر]
FROM dbo.BUY_INVOICE B
INNER JOIN dbo.BUY_ITEMS BI ON B.B_ID = BI.B_ID
INNER JOIN dbo.ITEMS I ON BI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.CUSTOMERS C ON B.CUST_ID = C.CUST_ID
WHERE CAST(B.B_DATE AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
  AND I.ITEM_INVISIBLE = 0
ORDER BY B.B_DATE DESC, B.B_ID DESC;
```

---

## PATTERN: معلومات-منتج-كاملة
TRIGGERS: معلومات منتج, معلومات عن, تفاصيل المنتج, بيانات المنتج, معدل السحب, معدل سحب, سرعة البيع, سعر البيع, صلاحية, باركود, product info, اعرضلي, اعرض لي, ابحث عن منتج
TABLES: ITEMS, ITEMS_SUB, BARCODE, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES: **الافتراضي عند إرسال اسم أو باركود فقط.** صف واحد: مخزون، سعر بيع، تكلفة، معدل سحب يومي، أيام تغطية، صلاحية، آخر مورد. مرّر product_filter.
---

```sql
;WITH ItemPick AS (
  SELECT TOP 1 I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME, I.MIN_LEVEL, I.MAX_LEVEL,
    I.LAST_COST, I.AVER_COST
  FROM dbo.ITEMS I
  WHERE I.ITEM_INVISIBLE = 0
    AND (
      I.ITEM_MODEL LIKE N'%PRODUCT%'
      OR I.ITEM_NAME LIKE N'%PRODUCT%'
      OR EXISTS (SELECT 1 FROM dbo.BARCODE BC WHERE BC.ITEM_ID = I.ITEM_ID AND BC.BARCODE LIKE N'%PRODUCT%')
    )
  ORDER BY CASE WHEN I.ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END, I.ITEM_ID DESC
),
UnitPick AS (
  SELECT TOP 1 BC.BARCODE, BC.PRICE1, BC.UNIT_DISC
  FROM dbo.BARCODE BC
  INNER JOIN ItemPick IP ON BC.ITEM_ID = IP.ITEM_ID
  ORDER BY BC.PRICE1 DESC, BC.BARCODE
),
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT IP.ITEM_ID,
    SUM(ISNULL(SUB.QTY, 0)) AS TotalStock,
    MIN(CASE WHEN ISNULL(SUB.QTY, 0) > 0 AND SUB.CATEOGRY3 IS NOT NULL THEN CAST(SUB.CATEOGRY3 AS date) END) AS NearestExpiry
  FROM ItemPick IP
  LEFT JOIN dbo.ITEMS_SUB SUB ON IP.ITEM_ID = SUB.ITEM_ID
  GROUP BY IP.ITEM_ID
),
SalesRecent AS (
  SELECT IP.ITEM_ID, SUM(X.QTY) AS SoldQty,
    COUNT(DISTINCT CAST(X.S_DATE AS date)) AS ActiveSaleDays,
    MAX(X.S_DATE) AS LastSaleDate
  FROM ItemPick IP
  JOIN (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day, -60, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day, -60, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X ON IP.ITEM_ID = X.ITEM_ID
  GROUP BY IP.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice, CU.CUST_NAME AS LastSupplier, B.B_DATE AS LastBuyDate
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
  WHERE BI.B_ITEM_ID IN (
    SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
    JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID = B2.B_ID
    JOIN ItemPick IP2 ON BI2.ITEM_ID = IP2.ITEM_ID
    GROUP BY BI2.ITEM_ID
  )
)
SELECT
  IP.ITEM_MODEL AS [الكود],
  LEFT(IP.ITEM_NAME, 80) AS [الاسم],
  ISNULL(U.BARCODE, N'') AS [الباركود],
  ISNULL(U.UNIT_DISC, N'') AS [الوحدة],
  CAST(ISNULL(U.PRICE1, 0) AS decimal(18,2)) AS [سعر_البيع],
  CAST(IP.LAST_COST AS decimal(18,2)) AS [آخر_تكلفة],
  CAST(IP.AVER_COST AS decimal(18,2)) AS [متوسط_التكلفة],
  CAST(ISNULL(SK.TotalStock, 0) AS decimal(18,2)) AS [المخزون],
  CAST(SK.NearestExpiry AS date) AS [أقرب_صلاحية],
  CAST(ISNULL(SR.SoldQty, 0) AS decimal(18,2)) AS [مبيعات_60_يوم],
  CAST(ISNULL(SR.SoldQty, 0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0) AS decimal(12,3)) AS [معدل_السحب_اليومي],
  CAST(ISNULL(SK.TotalStock, 0) / NULLIF(ISNULL(SR.SoldQty, 0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) AS decimal(12,1)) AS [أيام_تغطية_المخزون],
  CAST(SR.LastSaleDate AS date) AS [آخر_تاريخ_بيع],
  ISNULL(LB.LastSupplier, N'—') AS [آخر_مورد],
  CAST(LB.LastBuyPrice AS decimal(18,2)) AS [آخر_سعر_شراء],
  CAST(LB.LastBuyDate AS date) AS [آخر_تاريخ_شراء],
  CASE
    WHEN ISNULL(SK.TotalStock, 0) <= 0 AND ISNULL(SR.SoldQty, 0) > 0 THEN N'نفاد'
    WHEN ISNULL(SK.TotalStock, 0) / NULLIF(ISNULL(SR.SoldQty, 0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) < 7 THEN N'حرج'
    WHEN ISNULL(SK.TotalStock, 0) / NULLIF(ISNULL(SR.SoldQty, 0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) < 30 THEN N'يحتاج شراء'
    ELSE N'كافٍ'
  END AS [حالة_المخزون]
FROM ItemPick IP
LEFT JOIN UnitPick U ON 1 = 1
LEFT JOIN Stock SK ON IP.ITEM_ID = SK.ITEM_ID
LEFT JOIN SalesRecent SR ON IP.ITEM_ID = SR.ITEM_ID
LEFT JOIN LastBuy LB ON IP.ITEM_ID = LB.ITEM_ID;
```

---

## PATTERN: بحث-منتج-سريع
TRIGGERS: ابحث عن, find product, منتج, باركود, barcode lookup, بحث منتج, product search
TABLES: ITEMS, BARCODE, ITEMS_SUB
NOTES: بحث بالاسم/الكود/الباركود — استبدل %PRODUCT% أو مرّر product_filter.
---

```sql
;WITH Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY, 0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
)
SELECT TOP 25
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 70) AS [اسم_المنتج],
  B.BARCODE AS [باركود],
  CAST(B.PRICE1 AS decimal(18,2)) AS [سعر_البيع],
  CAST(I.LAST_COST AS decimal(18,2)) AS [آخر_تكلفة],
  CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [رصيد_المخزون]
FROM dbo.ITEMS I
LEFT JOIN dbo.BARCODE B ON I.ITEM_ID = B.ITEM_ID
LEFT JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (
    I.ITEM_MODEL LIKE N'%PRODUCT%'
    OR I.ITEM_NAME LIKE N'%PRODUCT%'
    OR B.BARCODE LIKE N'%PRODUCT%'
  )
ORDER BY I.ITEM_NAME;
```

---

## PATTERN: جرد-مخزون-حسب-المخزن
TRIGGERS: مخزون حسب المخزن, رصيد المخازن, stock by store, inventory by warehouse, جرد مخزن
TABLES: ITEMS_SUB, ITEMS, STORES
NOTES: تجميع ITEMS_SUB حسب STORE + ITEM. TOP 200.
---

```sql
SELECT TOP 200
  ST.STORE_NAME AS [المخزن],
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(SUM(ISNULL(SUB.QTY, 0)) AS decimal(18,2)) AS [الكمية],
  CAST(MIN(SUB.CATEOGRY3) AS date) AS [أقرب_صلاحية]
FROM dbo.ITEMS_SUB SUB
INNER JOIN dbo.ITEMS I ON SUB.ITEM_ID = I.ITEM_ID
INNER JOIN dbo.STORES ST ON SUB.STORE_ID = ST.STORE_ID
WHERE I.ITEM_INVISIBLE = 0 AND ISNULL(SUB.QTY, 0) <> 0
GROUP BY ST.STORE_NAME, I.ITEM_MODEL, I.ITEM_NAME
ORDER BY ST.STORE_NAME, I.ITEM_NAME;
```

---

## PATTERN: أعلى-منتجات-مبيعاً
TRIGGERS: أعلى منتجات, أكثر مبيعاً, best sellers, top products, الأكثر مبيعاً, ranking products
TABLES: SALE_INVOICE, SALE_ITEMS, ITEMS, R_S_INVOICE, R_S_ITEMS
NOTES: آخر 30 يوماً صافي (مبيعات − مردودات). ترتيب حسب الكمية.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
NetSales AS (
  SELECT X.ITEM_ID, SUM(X.QtyNet) AS NetQty, SUM(X.Revenue) AS NetRevenue
  FROM (
    SELECT SI.ITEM_ID, SI.QTY AS QtyNet, SI.QTY * SI.PRICE AS Revenue
    FROM dbo.SALE_ITEMS SI
    INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day, -30, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, -RSI.QTY * RSI.PRICE
    FROM dbo.R_S_ITEMS RSI
    INNER JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day, -30, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X
  GROUP BY X.ITEM_ID
)
SELECT TOP 30
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(NS.NetQty AS decimal(18,2)) AS [كمية_صافية],
  CAST(NS.NetRevenue AS decimal(18,2)) AS [إيراد_صافي_د_ل]
FROM NetSales NS
INNER JOIN dbo.ITEMS I ON NS.ITEM_ID = I.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0 AND NS.NetQty > 0
ORDER BY NS.NetQty DESC;
```

---

## PATTERN: مبيعات-حسب-المخزن
TRIGGERS: مبيعات مخزن, إيرادات المخزن, sales by store, warehouse revenue, أداء المخزن
TABLES: SALE_INVOICE, SALE_ITEMS, STORES
NOTES: آخر 30 يوماً. STORE_ID من SALE_ITEMS.
---

```sql
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
)
SELECT
  ISNULL(ST.STORE_NAME, N'غير محدد') AS [المخزن],
  COUNT(DISTINCT INV.S_ID) AS [عدد_الفواتير],
  CAST(SUM(SI.QTY) AS decimal(18,1)) AS [الوحدات],
  CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [الإيراد_د_ل]
FROM dbo.SALE_INVOICE INV
INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
LEFT JOIN dbo.STORES ST ON SI.STORE_ID = ST.STORE_ID
WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day, -30, (SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
GROUP BY ST.STORE_NAME
ORDER BY [الإيراد_د_ل] DESC;
```

---

## PATTERN: عدد-المنتجات
TRIGGERS: عدد المنتجات, كم منتج, count products, عدد الاصناف, كم صنف
TABLES: ITEMS
NOTES: عدد الأصناف النشطة فقط — بدون أسماء.
---

```sql
SELECT COUNT(*) AS [عدد_المنتجات_النشطة]
FROM dbo.ITEMS
WHERE ITEM_INVISIBLE = 0;
```

---

## PATTERN: أعلى-منتجات-كل-الوقت
TRIGGERS: أعلى منتجات كل الوقت, بدون تاريخ, all time sellers, أكثر مبيعاً تاريخياً
TABLES: SALE_ITEMS, R_S_ITEMS, ITEMS
NOTES: **بدون نافذة زمنية** — صافي مبيعات − مردودات من كل السجل.
---

```sql
;WITH NetSales AS (
  SELECT X.ITEM_ID, SUM(X.QtyNet) AS NetQty, SUM(X.Revenue) AS NetRevenue
  FROM (
    SELECT SI.ITEM_ID, SI.QTY AS QtyNet, SI.QTY * SI.PRICE AS Revenue
    FROM dbo.SALE_ITEMS SI
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, -RSI.QTY * RSI.PRICE
    FROM dbo.R_S_ITEMS RSI
  ) X
  GROUP BY X.ITEM_ID
)
SELECT TOP 30
  I.ITEM_MODEL AS [كود],
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(NS.NetQty AS decimal(18,2)) AS [كمية_صافية],
  CAST(NS.NetRevenue AS decimal(18,2)) AS [إيراد_صافي_د_ل]
FROM NetSales NS
INNER JOIN dbo.ITEMS I ON NS.ITEM_ID = I.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0 AND NS.NetQty > 0
ORDER BY NS.NetQty DESC;
```

---
# نهاية الملف
$bnd_marketing_agent_md$,
  1,
  '9365c4823768b0f7308a014ea2da13e9f47fa6659ab89e1962d0a361240a3ab5',
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
