1. استعلام فحص الأصناف ووحدات القياس (العبوات)
هذا الاستعلام مخصص فقط لسحب الأصناف وتحديد معامل التعبئة (PackFactor) والتكلفة المرجعية.

SQL
SET NOCOUNT ON;

-- 0) الإعدادات العامة
DECLARE @end DATE  = CAST(GETDATE() AS DATE);
DECLARE @window_days INT = 60;
DECLARE @start DATE = DATEADD(DAY, -(@window_days-1), @end);

-- 1) المنتجات والتصنيفات
IF OBJECT_ID('tempdb..#PIG') IS NOT NULL DROP TABLE #PIG;
CREATE TABLE #PIG (ProductID INT PRIMARY KEY, ProductName NVARCHAR(400), ProductCode NVARCHAR(100), TreeCategory NVARCHAR(200), MainCategory NVARCHAR(200), SubCategory NVARCHAR(200), GroupName NVARCHAR(200), ProductType NVARCHAR(200), Trademark NVARCHAR(200), MainSupplierID INT NULL);
INSERT INTO #PIG 
SELECT p.ProductID_PK, p.ProductName, p.ProductCode, ISNULL(cat.InventoryCategoryName, N'(غير محدد)'), ISNULL(dep.InvDepartmentName, N'(غير محدد)'), ISNULL(sub.InvSubDepartmentName, N'(غير محدد)'), ISNULL(pg.ProductGroupDescription, N'(غير محدد)'), ISNULL(pt.ProductTypeDescrption, N'(غير محدد)'), ISNULL(tm.ProductTrademarkDescrption, N'(غير محدد)'), p.MainSupplierID_FK
FROM Inventory.Data_Products p
LEFT JOIN Inventory.RefInventorySubDepartments sub ON sub.InvSubDepartmentID_PK = p.InvSubDepartmentID_FK
LEFT JOIN Inventory.RefInventoryDepartments dep ON dep.InvDepartmentID_PK = sub.InvDepartmentID_FK
LEFT JOIN Inventory.RefInventoryCategories cat ON cat.InventoryCategoryID_PK = dep.InventoryCategoryID_FK
LEFT JOIN Inventory.RefProductGroups pg ON pg.ProductGroupID_PK = p.ProductGroupID_FK
LEFT JOIN Inventory.RefProductTypes pt ON pt.ProductTypeID_PK = p.ProductTypeID_FK
LEFT JOIN Inventory.RefProductTrademarks tm ON tm.ProductTrademarkID_PK = p.ProductTrademarkID_FK;

-- 2) وحدات القياس (PackFactor)
IF OBJECT_ID('tempdb..#UOM_Ref') IS NOT NULL DROP TABLE #UOM_Ref;
CREATE TABLE #UOM_Ref(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6), PackCost DECIMAL(18,2));
INSERT INTO #UOM_Ref
SELECT pu.ProductID_FK, CAST(pu.BaseUnitQYT AS DECIMAL(18,6)), CAST(COALESCE(pu.UomLastPurchaseCost, pu.UomLastCost, pu.UomPurchaseCost, pu.UomCost, 0.00) AS DECIMAL(18,2))
FROM Inventory.Data_ProductUOMs pu JOIN Inventory.RefUOMs u ON u.UOMID_PK=pu.UomID_FK
WHERE u.UOMName IN (N'عبوة',N'علبة',N'Pack',N'PACK') OR u.UOMName LIKE N'%عبو%' OR u.UOMName LIKE N'%علبة%';

IF OBJECT_ID('tempdb..#UOM_Inferred') IS NOT NULL DROP TABLE #UOM_Inferred;
CREATE TABLE #UOM_Inferred(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6));
;WITH F AS (SELECT sii.ProductID_FK AS ProductID, COALESCE(sii.UnitBaseQYT,1) AS Factor, ROW_NUMBER() OVER(PARTITION BY sii.ProductID_FK ORDER BY COUNT_BIG(*) DESC) rn FROM SALES.Data_SalesInvoiceItems sii GROUP BY sii.ProductID_FK, COALESCE(sii.UnitBaseQYT,1))
INSERT INTO #UOM_Inferred SELECT ProductID, CAST(Factor AS DECIMAL(18,6)) FROM F WHERE rn=1;

IF OBJECT_ID('tempdb..#UOM_Priority') IS NOT NULL DROP TABLE #UOM_Priority;
CREATE TABLE #UOM_Priority(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6), PackCostRef DECIMAL(18,2));
INSERT INTO #UOM_Priority SELECT p.ProductID, COALESCE(r.PackFactor, i.PackFactor, 1.0), r.PackCost FROM #PIG p LEFT JOIN #UOM_Ref r ON r.ProductID=p.ProductID LEFT JOIN #UOM_Inferred i ON i.ProductID=p.ProductID;

IF OBJECT_ID('tempdb..#UOM') IS NOT NULL DROP TABLE #UOM;
CREATE TABLE #UOM(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6) NOT NULL, PackCostRef DECIMAL(18,2) NULL);
INSERT INTO #UOM SELECT ProductID, PackFactor, PackCostRef FROM #UOM_Priority;

-- 📌 نتيجة الاستعلام (المخرجات)
SELECT p.ProductID, p.ProductName, u.PackFactor, u.PackCostRef 
FROM #PIG p JOIN #UOM u ON p.ProductID = u.ProductID;
2. استعلام حساب أيام توفر المخزون (Availability)
هذا الاستعلام مخصص لاختبار اللوجيك الخاص بتتبع الرصيد التراكمي (EOD) وحساب الأيام اللي كان فيها الصنف متوفر فعلياً في الصيدلية.
(ملاحظة: هذا الاستعلام يتضمن الأكواد الخاصة بالأصناف والوحدات من الاستعلام الأول لأنه يعتمد عليها).

SQL
SET NOCOUNT ON;

DECLARE @end DATE = CAST(GETDATE() AS DATE);
DECLARE @window_days INT = 60;
DECLARE @start DATE = DATEADD(DAY, -(@window_days-1), @end);
DECLARE @pre_window_days_target INT = CAST(FLOOR(0.5 * @window_days) AS INT);
DECLARE @hist_days INT = @window_days + @pre_window_days_target + 30;
DECLARE @hist_start DATE = DATEADD(DAY, -(@hist_days-1), @end);

-- (إنشاء #PIG و #UOM باختصار)
IF OBJECT_ID('tempdb..#PIG') IS NOT NULL DROP TABLE #PIG;
CREATE TABLE #PIG (ProductID INT PRIMARY KEY, ProductName NVARCHAR(400));
INSERT INTO #PIG SELECT ProductID_PK, ProductName FROM Inventory.Data_Products;
IF OBJECT_ID('tempdb..#UOM') IS NOT NULL DROP TABLE #UOM;
CREATE TABLE #UOM(ProductID INT PRIMARY KEY, PackFactor DECIMAL(18,6) NOT NULL);
INSERT INTO #UOM SELECT ProductID_PK, 1 FROM Inventory.Data_Products; -- (نسخة مبسطة للاختبار)

-- 3) تقويم التواريخ
IF OBJECT_ID('tempdb..#Dates') IS NOT NULL DROP TABLE #Dates;
CREATE TABLE #Dates (d DATE PRIMARY KEY);
;WITH N AS (SELECT TOP (@hist_days) ROW_NUMBER() OVER(ORDER BY (SELECT NULL)) - 1 AS n FROM sys.all_objects)
INSERT INTO #Dates SELECT DATEADD(DAY,n,@hist_start) FROM N;

-- 4) الحركات اليومية والرصيد الافتتاحي
IF OBJECT_ID('tempdb..#DailyAgg') IS NOT NULL DROP TABLE #DailyAgg;
CREATE TABLE #DailyAgg(ProductID INT, d DATE, NetQTY_Pack DECIMAL(18,6));
INSERT INTO #DailyAgg SELECT t.ProductID_FK, CAST(t.TransactionDate AS DATE), SUM(CAST(t.TransactionQYT AS DECIMAL(18,6))/NULLIF(u.PackFactor,0)) FROM Inventory.Data_InventoryTransactions t JOIN #UOM u ON u.ProductID=t.ProductID_FK WHERE CAST(t.TransactionDate AS DATE) BETWEEN @hist_start AND @end AND t.ProductID_FK IN (SELECT ProductID FROM #PIG) GROUP BY t.ProductID_FK, CAST(t.TransactionDate AS DATE);

IF OBJECT_ID('tempdb..#Opening') IS NOT NULL DROP TABLE #Opening;
CREATE TABLE #Opening(ProductID INT PRIMARY KEY, OpeningQTY_Pack DECIMAL(18,6));
INSERT INTO #Opening SELECT t.ProductID_FK, COALESCE(SUM(CAST(t.TransactionQYT AS DECIMAL(18,6))/NULLIF(u.PackFactor,0)),0) FROM Inventory.Data_InventoryTransactions t JOIN #UOM u ON u.ProductID=t.ProductID_FK WHERE CAST(t.TransactionDate AS DATE) < @hist_start AND t.ProductID_FK IN (SELECT ProductID FROM #PIG) GROUP BY t.ProductID_FK;

-- 5) رصيد نهاية اليوم (EOD)
IF OBJECT_ID('tempdb..#Cumu') IS NOT NULL DROP TABLE #Cumu;
CREATE TABLE #Cumu(ProductID INT, d DATE, EOD_Pack DECIMAL(18,6));
INSERT INTO #Cumu SELECT p.ProductID, dt.d, CAST(COALESCE(op.OpeningQTY_Pack,0) + SUM(COALESCE(da.NetQTY_Pack,0)) OVER (PARTITION BY p.ProductID ORDER BY dt.d ROWS UNBOUNDED PRECEDING) AS DECIMAL(18,6)) AS EOD_Pack FROM #PIG p CROSS JOIN #Dates dt LEFT JOIN #DailyAgg da ON da.ProductID=p.ProductID AND da.d=dt.d LEFT JOIN #Opening op ON op.ProductID=p.ProductID;

-- 6) الأيام المعتمدة
IF OBJECT_ID('tempdb..#Avail') IS NOT NULL DROP TABLE #Avail;
CREATE TABLE #Avail(ProductID INT PRIMARY KEY, Days_in_window INT, PreRun_Capped INT, DaysApproved INT);
;WITH Win AS (SELECT ProductID, SUM(CASE WHEN d BETWEEN @start AND @end AND EOD_Pack>0 THEN 1 ELSE 0 END) AS Days_in_window FROM #Cumu GROUP BY ProductID), PreCnt AS (SELECT ProductID, SUM(CASE WHEN d<@start AND EOD_Pack>0 THEN 1 ELSE 0 END) AS PreHave FROM #Cumu GROUP BY ProductID)
INSERT INTO #Avail SELECT p.ProductID, COALESCE(w.Days_in_window,0), CASE WHEN COALESCE(pc.PreHave,0)>=@pre_window_days_target THEN @pre_window_days_target ELSE COALESCE(pc.PreHave,0) END, COALESCE(w.Days_in_window,0) + CASE WHEN COALESCE(pc.PreHave,0)>=@pre_window_days_target THEN @pre_window_days_target ELSE COALESCE(pc.PreHave,0) END FROM #PIG p LEFT JOIN Win w ON w.ProductID=p.ProductID LEFT JOIN PreCnt pc ON pc.ProductID=p.ProductID;

-- 📌 نتيجة الاستعلام (المخرجات)
SELECT p.ProductID, p.ProductName, a.Days_in_window AS [أيام التوفر خلال النافذة], a.DaysApproved AS [إجمالي الأيام المعتمدة]
FROM #Avail a JOIN #PIG p ON p.ProductID = a.ProductID;
3. استعلام المبيعات وصافي المطلوب (Net Required)
هذا هو اللوجيك الأساسي لحساب متوسط البيع اليومي الدقيق بناءً على أيام التوفر، ومن ثم حساب صافي الأصناف المطلوبة لتغطية 30 يوم.

SQL
SET NOCOUNT ON;

DECLARE @end DATE = CAST(GETDATE() AS DATE);
DECLARE @window_days INT = 60;
DECLARE @start DATE = DATEADD(DAY, -(@window_days-1), @end);
DECLARE @target_coverage_days INT = 30;
DECLARE @round_to INT = 1;

-- (إنشاء الجداول الوهمية الأساسية للاختبار فقط - في الواقع يجب دمج الأكواد السابقة هنا لكي تكون البيانات حقيقية بنسبة 100%)
-- لتشغيل هذا الاستعلام في بيئتك بشكل دقيق، يجب تشغيل أكواد (1) و (2) أعلاه أولاً، ثم تشغيل هذا الكود.

IF OBJECT_ID('tempdb..#SalesDaily') IS NOT NULL DROP TABLE #SalesDaily;
CREATE TABLE #SalesDaily(ProductID INT, d DATE, SalesBase DECIMAL(18,6));
INSERT INTO #SalesDaily
SELECT sii.ProductID_FK, CAST(si.SalesInvoiceDate AS DATE), SUM(CAST(COALESCE(sii.UnitBaseQYT,1) AS DECIMAL(18,6)) * sii.QYT)
FROM SALES.Data_SalesInvoiceItems sii JOIN SALES.Data_SalesInvoices si ON si.SalesInvoiceID_PK=sii.SalesInvoiceID_FK
WHERE CAST(si.SalesInvoiceDate AS DATE) BETWEEN @start AND @end GROUP BY sii.ProductID_FK, CAST(si.SalesInvoiceDate AS DATE);

IF OBJECT_ID('tempdb..#SalesApproved') IS NOT NULL DROP TABLE #SalesApproved;
CREATE TABLE #SalesApproved(ProductID INT PRIMARY KEY, SalesQTY_Pack DECIMAL(18,6), AvgDaily_Pack DECIMAL(18,6));
INSERT INTO #SalesApproved(ProductID, SalesQTY_Pack, AvgDaily_Pack)
SELECT ad.ProductID, CAST(SUM(COALESCE(sd.SalesBase,0))/NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS SalesQTY_Pack, CAST(CASE WHEN af.DaysApproved>0 THEN (SUM(COALESCE(sd.SalesBase,0))/NULLIF(u.PackFactor,0)) * 1.0 / af.DaysApproved ELSE 0 END AS DECIMAL(18,6)) AS AvgDaily_Pack
FROM #ApprovedDays ad LEFT JOIN #SalesDaily sd ON sd.ProductID=ad.ProductID AND sd.d=ad.d LEFT JOIN #UOM u ON u.ProductID=ad.ProductID JOIN #Avail af ON af.ProductID=ad.ProductID GROUP BY ad.ProductID, u.PackFactor, af.DaysApproved;

IF OBJECT_ID('tempdb..#Req') IS NOT NULL DROP TABLE #Req;
CREATE TABLE #Req(ProductID INT PRIMARY KEY, NetRequired_Pack DECIMAL(18,6));
INSERT INTO #Req(ProductID, NetRequired_Pack)
SELECT p.ProductID, CAST(CASE WHEN (@round_to > 1) THEN CEILING(CASE WHEN (@target_coverage_days * COALESCE(SA.AvgDaily_Pack,0) - COALESCE(EOP.Stock_Pack,0)) < 0 THEN 0 ELSE (@target_coverage_days * COALESCE(SA.AvgDaily_Pack,0) - COALESCE(EOP.Stock_Pack,0)) END / @round_to) * @round_to ELSE CASE WHEN (@target_coverage_days * COALESCE(SA.AvgDaily_Pack,0) - COALESCE(EOP.Stock_Pack,0)) < 0 THEN 0 ELSE (@target_coverage_days * COALESCE(SA.AvgDaily_Pack,0) - COALESCE(EOP.Stock_Pack,0)) END END AS DECIMAL(18,6))
FROM #PIG p LEFT JOIN #SalesApproved SA ON SA.ProductID=p.ProductID LEFT JOIN #EODPack EOP ON EOP.ProductID=p.ProductID;

-- 📌 نتيجة الاستعلام (المخرجات)
SELECT r.ProductID, s.AvgDaily_Pack AS [المتوسط اليومي الدقيق], r.NetRequired_Pack AS [صافي المطلوب (عبوة)]
FROM #Req r JOIN #SalesApproved s ON r.ProductID = s.ProductID;
4. استعلام فواتير المشتريات وصلاحية الأصناف
هذا الجزء مخصص لفحص جلب فواتير آخر 3 أشهر بدقة، مع حساب التكاليف، واستخراج تواريخ الصلاحية (ExpireDate).

SQL
SET NOCOUNT ON;
DECLARE @purchase_months INT = 3;
DECLARE @purchase_from DATE = DATEADD(MONTH, -@purchase_months, CAST(GETDATE() AS DATE));

-- افتراض وجود #PIG و #UOM لتشغيل الكود
IF OBJECT_ID('tempdb..#ItemDiscount') IS NOT NULL DROP TABLE #ItemDiscount;
CREATE TABLE #ItemDiscount(PurchaseInvoiceItemID_PK INT PRIMARY KEY, InvoiceID_FK INT, ProductID_FK INT, ItemDiscountAmount DECIMAL(18,6));
;WITH InvoiceTotals AS (SELECT pi.InvoiceID_PK, COALESCE(pi.InvoiceLocCurrencyDiscountTotal, 0) AS InvoiceDiscountTotal, SUM(pii.UnitCost * pii.QYT) AS InvoiceSubTotal FROM Purchase.Data_PurchaseInvoices pi JOIN Purchase.Data_PurchaseInvoiceItems pii ON pii.InvoiceID_FK = pi.InvoiceID_PK GROUP BY pi.InvoiceID_PK, pi.InvoiceLocCurrencyDiscountTotal)
INSERT INTO #ItemDiscount SELECT pii.PurchaseInvoiceItemID_PK, pii.InvoiceID_FK, pii.ProductID_FK, CAST(CASE WHEN it.InvoiceSubTotal > 0 THEN (pii.UnitCost * pii.QYT) / it.InvoiceSubTotal * it.InvoiceDiscountTotal ELSE 0 END AS DECIMAL(18,6)) FROM Purchase.Data_PurchaseInvoiceItems pii JOIN InvoiceTotals it ON it.InvoiceID_PK = pii.InvoiceID_FK;

IF OBJECT_ID('tempdb..#PurchaseTransactions') IS NOT NULL DROP TABLE #PurchaseTransactions;
CREATE TABLE #PurchaseTransactions(ProductID INT, SupplierID INT, PurchaseDate DATE, InvoiceID INT, CostPerBase DECIMAL(18,6), CostPerPack DECIMAL(18,6), QtyBase DECIMAL(18,6), QtyPack DECIMAL(18,6), LineValue DECIMAL(18,2), LineKey INT, ExpireDate DATE);

INSERT INTO #PurchaseTransactions
SELECT pii.ProductID_FK AS ProductID, pi.SupplierID_FK AS SupplierID, CAST(pi.Createddate AS DATE) AS PurchaseDate, pi.InvoiceID_PK AS InvoiceID, 
  CAST((pii.UnitCost / NULLIF(COALESCE(pii.UnitBaseQYT,1),0)) - (COALESCE(id.ItemDiscountAmount,0) / NULLIF(pii.QYT * COALESCE(pii.UnitBaseQYT,1),0)) AS DECIMAL(18,6)) AS CostPerBase, 
  CAST(((pii.UnitCost / NULLIF(COALESCE(pii.UnitBaseQYT,1),0)) - (COALESCE(id.ItemDiscountAmount,0) / NULLIF(pii.QYT * COALESCE(pii.UnitBaseQYT,1),0))) * NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS CostPerPack, 
  CAST(pii.QYT * COALESCE(pii.UnitBaseQYT,1) AS DECIMAL(18,6)) AS QtyBase, CAST((pii.QYT * COALESCE(pii.UnitBaseQYT,1)) / NULLIF(u.PackFactor,0) AS DECIMAL(18,6)) AS QtyPack, 
  CAST(((pii.UnitCost / NULLIF(COALESCE(pii.UnitBaseQYT,1),0)) - (COALESCE(id.ItemDiscountAmount,0) / NULLIF(pii.QYT * COALESCE(pii.UnitBaseQYT,1),0))) * pii.QYT * COALESCE(pii.UnitBaseQYT,1) AS DECIMAL(18,2)) AS LineValue, 
  pii.PurchaseInvoiceItemID_PK AS LineKey, CAST(pii.ExpireDate AS DATE) AS ExpireDate
FROM Purchase.Data_PurchaseInvoiceItems pii
JOIN Purchase.Data_PurchaseInvoices pi ON pi.InvoiceID_PK=pii.InvoiceID_FK
JOIN #UOM u ON u.ProductID=pii.ProductID_FK
LEFT JOIN #ItemDiscount id ON id.PurchaseInvoiceItemID_PK = pii.PurchaseInvoiceItemID_PK
WHERE CAST(pi.Createddate AS DATE) >= @purchase_from;

-- 📌 نتيجة الاستعلام (المخرجات)
SELECT pt.ProductID, pt.SupplierID, pt.PurchaseDate, pt.InvoiceID, pt.QtyPack, pt.CostPerPack, pt.ExpireDate AS [تاريخ الصلاحية]
FROM #PurchaseTransactions pt ORDER BY pt.PurchaseDate DESC;