/* حساب أيام توفر المخزون (Availability) — InfinityRetailDB
   تتبع الرصيد التراكمي لآخر 60 يوم (EOD) وحساب الأيام التي كان فيها الصنف متوفراً فعلياً.
*/

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
SELECT p.ProductID AS [معرف الصنف], p.ProductName AS [اسم الصنف], a.Days_in_window AS [أيام التوفر خلال النافذة], a.DaysApproved AS [إجمالي الأيام المعتمدة]
FROM #Avail a JOIN #PIG p ON p.ProductID = a.ProductID;
