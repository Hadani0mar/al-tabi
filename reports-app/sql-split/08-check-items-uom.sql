/* فحص الأصناف ووحدات القياس — InfinityRetailDB
   سحب الأصناف وتحديد معامل التعبئة (PackFactor) والتكلفة المرجعية.
*/

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
SELECT p.ProductID AS [معرف الصنف], p.ProductName AS [اسم الصنف], u.PackFactor AS [معامل التعبئة], u.PackCostRef AS [التكلفة المرجعية]
FROM #PIG p JOIN #UOM u ON p.ProductID = u.ProductID;
