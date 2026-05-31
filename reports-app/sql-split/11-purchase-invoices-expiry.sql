/* فواتير المشتريات وصلاحية الأصناف — InfinityRetailDB
   فحص فواتير الشراء لآخر 3 أشهر بدقة مع احتساب الخصومات والتكاليف، واستخراج تواريخ صلاحيات المنتجات.
*/

SET NOCOUNT ON;
DECLARE @purchase_months INT = 3;
DECLARE @purchase_from DATE = DATEADD(MONTH, -@purchase_months, CAST(GETDATE() AS DATE));

-- ملاحظة: يتطلب تشغيل استعلامات (1) المتقدمة لبناء الجداول المؤقتة #PIG و #UOM عند التنفيذ الفعلي الشامل.

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
SELECT pt.ProductID AS [معرف الصنف], pt.SupplierID AS [معرف المورد], pt.PurchaseDate AS [تاريخ الشراء], pt.InvoiceID AS [رقم الفاتورة], pt.QtyPack AS [الكمية (عبوة)], pt.CostPerPack AS [سعر التكلفة (عبوة)], pt.ExpireDate AS [تاريخ الصلاحية]
FROM #PurchaseTransactions pt ORDER BY pt.PurchaseDate DESC;
