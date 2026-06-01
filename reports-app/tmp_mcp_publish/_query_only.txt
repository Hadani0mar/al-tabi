
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
