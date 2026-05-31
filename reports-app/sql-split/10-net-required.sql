/* المبيعات وصافي المطلوب (Net Required) — InfinityRetailDB
   حساب متوسط البيع اليومي الدقيق بناءً على أيام التوفر، وحساب صافي الأصناف المطلوبة لتغطية 30 يوماً.
*/

SET NOCOUNT ON;

DECLARE @end DATE = CAST(GETDATE() AS DATE);
DECLARE @window_days INT = 60;
DECLARE @start DATE = DATEADD(DAY, -(@window_days-1), @end);
DECLARE @target_coverage_days INT = 30;
DECLARE @round_to INT = 1;

-- ملاحظة: يتطلب تشغيل استعلامات (1) و (2) المتقدمة لبناء الجداول المؤقتة #PIG و #UOM و #ApprovedDays و #Avail و #EODPack عند التنفيذ الفعلي الشامل.

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
SELECT r.ProductID AS [معرف الصنف], s.AvgDaily_Pack AS [المتوسط اليومي الدقيق], r.NetRequired_Pack AS [صافي المطلوب (عبوة)]
FROM #Req r JOIN #SalesApproved s ON r.ProductID = s.ProductID;
