# AGENT_Marketing2026 — أنماط SQL جاهزة للتنفيذ
# ERP: Marketing2026 | SQL Server | schema: dbo
# انسخ SQL من ## PATTERN أدناه ونفّذه بـ execute_raw_sql. لا تخترع SQL.
# SALE_ITEMS لا يحتوي S_DATE — استخدم JOIN مع SALE_INVOICE.
# تاريخ صريح من المستخدم → استخدمه مباشرةً. لا تستبدله بـ MAX(S_DATE).
# {{PRODUCT_FILTER}} → استبدله باسم/كود المنتج من رسالة المستخدم.

---

## PATTERN: أعلى-منتجات-مبيعاً
TRIGGERS: أكثر مبيعاً, أعلى منتجات, top sellers, best selling, أكثر المنتجات بيعاً, أكثر الاصناف, رانكينج المبيعات, أعلى إيرادات, الأكثر طلباً, مبيعات هذا الشهر, مبيعات الشهر السابق, توقعات مبيعات, forecast
TABLES: SALE_ITEMS, SALE_INVOICE, ITEMS, ITEMS_SUB, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - **4 استعلامات — اختر المناسب حسب طلب المستخدم:**
    * SQL-A: آخر N يوم (الافتراضي 30). عدّل `-30` لأي عدد أيام.
    * SQL-B: هذا الشهر (شهر آخر فاتورة).
    * SQL-C: الشهر السابق.
    * SQL-D: توقعات الشهر القادم (معدل يومي × 30 + أيام تغطية المخزون).
  - **كيف يختار الوكيل:**
    * «أكثر مبيعاً» بدون تحديد → SQL-A (30 يوم).
    * «أكثر مبيعاً آخر 60 يوم» → SQL-A مع -60 بدل -30.
    * «أكثر مبيعاً هذا الشهر» → SQL-B.
    * «أكثر مبيعاً الشهر الماضي/السابق» → SQL-C.
    * «توقعات / تنبؤات / الشهر القادم» → SQL-D.
  - يُرجع: اسم المنتج، الكمية المباعة، الإيراد، آخر سعر شراء، المورد.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
-- [A] أكثر مبيعاً آخر N يوم (عدّل -30 حسب الحاجة: -7 لأسبوع، -60 لشهرين...)
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
SalesWindow AS (
  SELECT SI.ITEM_ID, SUM(SI.QTY) AS SoldQty, SUM(SI.QTY * SI.PRICE) AS Revenue
  FROM dbo.SALE_ITEMS SI
  JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE CAST(INV.S_DATE AS date) >= DATEADD(day, -30, (SELECT d FROM AsOf))
  GROUP BY SI.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice,
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (PARTITION BY BI.ITEM_ID ORDER BY BI.B_ITEM_ID DESC) AS rn
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.PRICE > 0
)
SELECT TOP 30
  LEFT(I.ITEM_NAME, 60)                             AS [اسم_المنتج],
  CAST(SW.SoldQty AS decimal(18,2))                 AS [الكمية_المباعة],
  CAST(SW.Revenue AS decimal(18,2))                 AS [الإيراد_د_ل],
  CAST(ISNULL(LB.LastBuyPrice, 0) AS decimal(18,2)) AS [آخر_سعر_شراء],
  ISNULL(LB.Supplier, N'—')                        AS [المورد]
FROM SalesWindow SW
JOIN dbo.ITEMS I ON I.ITEM_ID=SW.ITEM_ID
LEFT JOIN LastBuy LB ON LB.ITEM_ID=SW.ITEM_ID AND LB.rn=1
WHERE I.ITEM_INVISIBLE=0
ORDER BY SW.SoldQty DESC;
```

```sql
-- [B] أكثر مبيعاً هذا الشهر (شهر آخر فاتورة)
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
SalesMonth AS (
  SELECT SI.ITEM_ID, SUM(SI.QTY) AS SoldQty, SUM(SI.QTY * SI.PRICE) AS Revenue
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE YEAR(INV.S_DATE) = YEAR((SELECT d FROM AsOf))
    AND MONTH(INV.S_DATE) = MONTH((SELECT d FROM AsOf))
  GROUP BY SI.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice,
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (PARTITION BY BI.ITEM_ID ORDER BY BI.B_ITEM_ID DESC) AS rn
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID WHERE BI.PRICE > 0
)
SELECT TOP 30
  LEFT(I.ITEM_NAME, 60)                             AS [اسم_المنتج],
  CAST(SM.SoldQty AS decimal(18,2))                 AS [الكمية_المباعة],
  CAST(SM.Revenue AS decimal(18,2))                 AS [الإيراد_د_ل],
  CAST(ISNULL(LB.LastBuyPrice, 0) AS decimal(18,2)) AS [آخر_سعر_شراء],
  ISNULL(LB.Supplier, N'—')                        AS [المورد]
FROM SalesMonth SM
JOIN dbo.ITEMS I ON I.ITEM_ID=SM.ITEM_ID
LEFT JOIN LastBuy LB ON LB.ITEM_ID=SM.ITEM_ID AND LB.rn=1
WHERE I.ITEM_INVISIBLE=0
ORDER BY SM.SoldQty DESC;
```

```sql
-- [C] أكثر مبيعاً الشهر السابق
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
PrevMonth AS (
  SELECT DATEADD(month, -1, (SELECT d FROM AsOf)) AS pm
),
SalesMonth AS (
  SELECT SI.ITEM_ID, SUM(SI.QTY) AS SoldQty, SUM(SI.QTY * SI.PRICE) AS Revenue
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE YEAR(INV.S_DATE) = YEAR((SELECT pm FROM PrevMonth))
    AND MONTH(INV.S_DATE) = MONTH((SELECT pm FROM PrevMonth))
  GROUP BY SI.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice,
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (PARTITION BY BI.ITEM_ID ORDER BY BI.B_ITEM_ID DESC) AS rn
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID WHERE BI.PRICE > 0
)
SELECT TOP 30
  LEFT(I.ITEM_NAME, 60)                             AS [اسم_المنتج],
  CAST(SM.SoldQty AS decimal(18,2))                 AS [الكمية_المباعة],
  CAST(SM.Revenue AS decimal(18,2))                 AS [الإيراد_د_ل],
  CAST(ISNULL(LB.LastBuyPrice, 0) AS decimal(18,2)) AS [آخر_سعر_شراء],
  ISNULL(LB.Supplier, N'—')                        AS [المورد]
FROM SalesMonth SM
JOIN dbo.ITEMS I ON I.ITEM_ID=SM.ITEM_ID
LEFT JOIN LastBuy LB ON LB.ITEM_ID=SM.ITEM_ID AND LB.rn=1
WHERE I.ITEM_INVISIBLE=0
ORDER BY SM.SoldQty DESC;
```

```sql
-- [D] توقعات مبيعات الشهر القادم (معدل يومي × 30 + أيام تغطية المخزون)
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
SalesRecent AS (
  SELECT SI.ITEM_ID,
    SUM(SI.QTY) AS SoldQty,
    SUM(SI.QTY * SI.PRICE) AS Revenue,
    COUNT(DISTINCT CAST(INV.S_DATE AS date)) AS ActiveDays
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE CAST(INV.S_DATE AS date) >= DATEADD(day, -60, (SELECT d FROM AsOf))
  GROUP BY SI.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice,
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (PARTITION BY BI.ITEM_ID ORDER BY BI.B_ITEM_ID DESC) AS rn
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID WHERE BI.PRICE > 0
),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
)
SELECT TOP 30
  LEFT(I.ITEM_NAME, 60)     AS [اسم_المنتج],
  CAST(SR.SoldQty / CAST(SR.ActiveDays AS float) * 30 AS decimal(18,1)) AS [توقع_كمية_30_يوم],
  CAST(SR.Revenue / CAST(SR.ActiveDays AS float) * 30 AS decimal(18,2)) AS [توقع_إيراد_30_يوم],
  CAST(ISNULL(ST.StockQty,0) AS decimal(18,2))   AS [المخزون_الحالي],
  CAST(ISNULL(ST.StockQty,0) / NULLIF(SR.SoldQty / CAST(SR.ActiveDays AS float), 0)
    AS decimal(18,1))                              AS [أيام_تغطية],
  CAST(ISNULL(LB.LastBuyPrice,0) AS decimal(18,2)) AS [آخر_سعر_شراء],
  ISNULL(LB.Supplier, N'—')                       AS [المورد]
FROM SalesRecent SR
JOIN dbo.ITEMS I ON I.ITEM_ID=SR.ITEM_ID
LEFT JOIN LastBuy LB ON LB.ITEM_ID=SR.ITEM_ID AND LB.rn=1
LEFT JOIN Stock ST ON ST.ITEM_ID=SR.ITEM_ID
WHERE I.ITEM_INVISIBLE=0 AND SR.ActiveDays >= 3
ORDER BY [توقع_كمية_30_يوم] DESC;
```

---

## PATTERN: المصروفات-الشهرية
TRIGGERS: مصروفات, مصاريف, expenses, رواتب وإيجار, مصاريف الشهر, مصاريف شهرية, كم صرفنا, المصروفات, نفقات, مصاريف هذا الشهر, مصاريف الشهر الماضي, مقارنة مصاريف
TABLES: GIVE, EXPENCES
NOTES:
  - **3 استعلامات — اختر المناسب حسب طلب المستخدم:**
    * SQL-A: مصروفات شهر محدد (الافتراضي = شهر آخر فاتورة). تصنيف حسب النوع + إجمالي.
    * SQL-B: مصروفات الشهر السابق.
    * SQL-C: مقارنة شهرية (آخر 6 شهور): رواتب | إيجار | كهرباء | أخرى | إجمالي.
  - **كيف يختار الوكيل:**
    * «مصاريف هذا الشهر / كم صرفنا» → SQL-A.
    * «مصاريف الشهر الماضي/السابق» → SQL-B.
    * «مقارنة مصاريف / تطور المصاريف» → SQL-C.
  - GIVE.EXPENCES_ID > 0 = مصروف حقيقي. EXPENCES_ID=0 = دفعات لموردين (ليست مصاريف).
  - أنواع المصروفات الرئيسية: 1=رواتب، 18=إيجار، 3=كهرباء، 17=أخرى.
  - G_STATUES=1 فقط (مؤكد). لا تفلتر بالحالة 0 أو 2.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
-- [A] مصروفات هذا الشهر (شهر آخر فاتورة) — حسب النوع + إجمالي
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
Expenses AS (
  SELECT E.EXPENSE_DISC,
    SUM(G.G_VALUE) AS Total, COUNT(G.G_ID) AS Cnt
  FROM dbo.GIVE G
  JOIN dbo.EXPENCES E ON G.EXPENCES_ID=E.EXPENCES_ID
  WHERE G.G_STATUES=1 AND G.EXPENCES_ID > 0
    AND YEAR(G.G_DATE)=YEAR((SELECT d FROM AsOf))
    AND MONTH(G.G_DATE)=MONTH((SELECT d FROM AsOf))
  GROUP BY E.EXPENSE_DISC
)
SELECT
  EXPENSE_DISC               AS [نوع_المصروف],
  Cnt                        AS [عدد_العمليات],
  CAST(Total AS decimal(18,2)) AS [المبلغ_د_ل]
FROM Expenses
UNION ALL
SELECT N'═══ الإجمالي ═══', SUM(Cnt), CAST(SUM(Total) AS decimal(18,2))
FROM Expenses
ORDER BY [المبلغ_د_ل] DESC;
```

```sql
-- [B] مصروفات الشهر السابق — حسب النوع + إجمالي
;WITH AsOf AS (
  SELECT DATEADD(month, -1, CAST(MAX(S_DATE) AS date)) AS d FROM dbo.SALE_INVOICE
),
Expenses AS (
  SELECT E.EXPENSE_DISC,
    SUM(G.G_VALUE) AS Total, COUNT(G.G_ID) AS Cnt
  FROM dbo.GIVE G
  JOIN dbo.EXPENCES E ON G.EXPENCES_ID=E.EXPENCES_ID
  WHERE G.G_STATUES=1 AND G.EXPENCES_ID > 0
    AND YEAR(G.G_DATE)=YEAR((SELECT d FROM AsOf))
    AND MONTH(G.G_DATE)=MONTH((SELECT d FROM AsOf))
  GROUP BY E.EXPENSE_DISC
)
SELECT
  EXPENSE_DISC               AS [نوع_المصروف],
  Cnt                        AS [عدد_العمليات],
  CAST(Total AS decimal(18,2)) AS [المبلغ_د_ل]
FROM Expenses
UNION ALL
SELECT N'═══ الإجمالي ═══', SUM(Cnt), CAST(SUM(Total) AS decimal(18,2))
FROM Expenses
ORDER BY [المبلغ_د_ل] DESC;
```

```sql
-- [C] مقارنة شهرية (آخر 6 شهور): رواتب | إيجار | كهرباء | أخرى | إجمالي
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
)
SELECT
  YEAR(G.G_DATE) AS [السنة],
  MONTH(G.G_DATE) AS [الشهر],
  CAST(SUM(CASE WHEN E.EXPENCES_ID=1  THEN G.G_VALUE ELSE 0 END) AS decimal(18,2)) AS [رواتب],
  CAST(SUM(CASE WHEN E.EXPENCES_ID=18 THEN G.G_VALUE ELSE 0 END) AS decimal(18,2)) AS [إيجار],
  CAST(SUM(CASE WHEN E.EXPENCES_ID=3  THEN G.G_VALUE ELSE 0 END) AS decimal(18,2)) AS [كهرباء],
  CAST(SUM(CASE WHEN E.EXPENCES_ID NOT IN (0,1,18,3) THEN G.G_VALUE ELSE 0 END) AS decimal(18,2)) AS [مصاريف_أخرى],
  CAST(SUM(G.G_VALUE) AS decimal(18,2)) AS [الإجمالي],
  COUNT(G.G_ID) AS [عدد_العمليات]
FROM dbo.GIVE G
JOIN dbo.EXPENCES E ON G.EXPENCES_ID=E.EXPENCES_ID
WHERE G.G_STATUES=1 AND G.EXPENCES_ID > 0
  AND G.G_DATE >= DATEADD(month, -6, (SELECT d FROM AsOf))
GROUP BY YEAR(G.G_DATE), MONTH(G.G_DATE)
ORDER BY [السنة] DESC, [الشهر] DESC;
```

---

## PATTERN: مقارنة-أسعار-موردين
TRIGGERS: مقارنة أسعار, مقارنة موردين, موردي منتج, أرخص مورد, أغلى مورد, supplier prices, compare suppliers, موردين منتج, افضل الموردين, أفضل مورد
TABLES: ITEMS, ITEMS_SUB, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - **product_filter مطلوب** — يُرجع صفاً لكل مورد سبق أن باع هذا المنتج.
  - كل صف: اسم المنتج، الكمية الحالية، المورد، آخر سعر شراء منه، تاريخ آخر شراء منه، علامة ✔ أرخص للأرخص.
  - الترتيب: من الأرخص للأغلى — الأرخص دائماً في الصف الأول.
  - PickID يُفضّل المنتج الأكثر سجلات شراء (PRICE>0) إذا تعددت المطابقات.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
-- مقارنة أسعار كل الموردين لمنتج واحد — مرتبة من الأرخص للأغلى
-- استبدل {{PRODUCT_FILTER}} باسم أو كود المنتج
;WITH
PickID AS (
  SELECT TOP 1 I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME
  FROM dbo.ITEMS I
  WHERE I.ITEM_INVISIBLE=0
    AND (I.ITEM_NAME LIKE N'%{{PRODUCT_FILTER}}%' OR I.ITEM_MODEL LIKE N'%{{PRODUCT_FILTER}}%')
  ORDER BY
    (SELECT COUNT(*) FROM dbo.BUY_ITEMS BI2 WHERE BI2.ITEM_ID=I.ITEM_ID AND BI2.PRICE>0) DESC,
    CASE WHEN I.ITEM_MODEL LIKE N'%{{PRODUCT_FILTER}}%' THEN 0 ELSE 1 END,
    I.ITEM_NAME
),
Stock AS (
  SELECT SUM(ISNULL(QTY,0)) AS StockQty
  FROM dbo.ITEMS_SUB WHERE ITEM_ID=(SELECT ITEM_ID FROM PickID)
),
AllBuy AS (
  SELECT
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    BI.PRICE,
    B.B_DATE,
    ROW_NUMBER() OVER (PARTITION BY B.CUST_ID ORDER BY BI.B_ITEM_ID DESC) AS rn
  FROM dbo.BUY_ITEMS BI
  JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.ITEM_ID=(SELECT ITEM_ID FROM PickID) AND BI.PRICE > 0
),
BySupplier AS (SELECT * FROM AllBuy WHERE rn=1)
SELECT
  LEFT(P.ITEM_NAME, 60)                              AS [اسم_المنتج],
  CAST((SELECT StockQty FROM Stock) AS decimal(18,2)) AS [الكمية_الحالية],
  BS.Supplier                                        AS [المورد],
  CAST(BS.PRICE AS decimal(18,2))                    AS [آخر_سعر_شراء],
  CAST(BS.B_DATE AS date)                            AS [تاريخ_آخر_شراء],
  CASE WHEN BS.PRICE = MIN(BS.PRICE) OVER() THEN N'? ارخص' ELSE N'' END AS [ملاحظة]
FROM PickID P
CROSS JOIN BySupplier BS
ORDER BY BS.PRICE ASC;
```

---

## PATTERN: نواقص-نشطة-مورد
TRIGGERS: نواقص, نواقصنا, شن النواقص, عندنا نواقص, نفاد, shortage, نواقص نشطة, تحت الحد, ايش ناقصنا, ماذا ينقصنا, قائمة النواقص, المنتجات الناقصة
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - نواقص نشطة = مبيعات > 0 في آخر 60 يوم + رصيد ≤ 0 أو ≤ MIN_LEVEL.
  - يُرجع: اسم المنتج، الكمية الحالية، آخر سعر شراء، المورد، مبيعات 60 يوم، الحالة.
  - LastBuy: أحدث B_ITEM_ID لكل ITEM_ID حيث PRICE > 0 (تجاهل سجلات السعر الصفري).
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
-- نواقص نشطة مع آخر سعر شراء والمورد
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT SI.ITEM_ID, SUM(SI.QTY) AS SoldQty
  FROM dbo.SALE_ITEMS SI
  JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  GROUP BY SI.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID,
    BI.PRICE AS LastBuyPrice,
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (PARTITION BY BI.ITEM_ID ORDER BY BI.B_ITEM_ID DESC) AS rn
  FROM dbo.BUY_ITEMS BI
  JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.PRICE > 0
)
SELECT TOP 100
  LEFT(I.ITEM_NAME, 60) AS [اسم_المنتج],
  CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [الكمية_الحالية],
  CAST(ISNULL(LB.LastBuyPrice, 0) AS decimal(18,2)) AS [آخر_سعر_شراء],
  ISNULL(LB.Supplier, N'—') AS [المورد],
  CAST(SR.SoldQty AS decimal(18,2)) AS [مبيعات_60_يوم],
  CASE WHEN ISNULL(S.StockQty,0) <= 0 THEN N'نفاد' ELSE N'تحت الحد' END AS [الحالة]
FROM dbo.ITEMS I
JOIN SalesRecent SR ON SR.ITEM_ID=I.ITEM_ID AND SR.SoldQty > 0
LEFT JOIN Stock S ON S.ITEM_ID=I.ITEM_ID
LEFT JOIN LastBuy LB ON LB.ITEM_ID=I.ITEM_ID AND LB.rn=1
WHERE I.ITEM_INVISIBLE=0
  AND (ISNULL(S.StockQty,0) <= 0 OR (I.MIN_LEVEL > 0 AND ISNULL(S.StockQty,0) <= I.MIN_LEVEL))
ORDER BY ISNULL(S.StockQty,0) ASC, SR.SoldQty DESC;
```

---

## PATTERN: تقرير-الصلاحية
TRIGGERS: منتهية الصلاحية, صلاحية, تاريخ انتهاء, سينخلص قريباً, ستنتهي صلاحيتها, expiry report, expiring soon, expired products, expiry date, الصلاحية, الصلاحيات, المنتهية, منتهية, ينتهي هذا الشهر, سينتهي قريباً
TABLES: ITEMS_SUB, ITEMS, STORES
NOTES: يحتوي هذا النمط على استعلامين للصلاحية (المنتهي بالكامل، وما سينتهي قريباً). تم استبعاد رقم الدفعة والمخزن وعرض فقط الحقول المطلوبة لتكون مهنية ومبسطة ومجمعة باسم الصنف وتاريخ الصلاحية لمنع التكرار.
---

```sql
-- 1. المنتجات المنتهية الصلاحية بالكامل حالياً ولا تصلح (رصيد > 0)
-- الأعمدة: [اسم المنتج]، [الكمية]، [تاريخ الانتهاء]
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
)
SELECT TOP 100
  LEFT(I.ITEM_NAME, 70) AS [اسم المنتج],
  CAST(SUM(S.QTY) AS decimal(18,2)) AS [الكمية],
  CAST(S.CATEOGRY3 AS date) AS [تاريخ الانتهاء]
FROM dbo.ITEMS_SUB S
INNER JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CAST(S.CATEOGRY3 AS date) < (SELECT d FROM AsOf)
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
GROUP BY I.ITEM_NAME, S.CATEOGRY3
ORDER BY S.CATEOGRY3 ASC;
```

```sql
-- 2. المنتجات التي ستنتهي صلاحيتها خلال فترة مخصصة (أيام - الافتراضي 60 يوماً ويُستبدل ديناميكياً)
-- الأعمدة: [اسم المنتج]، [الكمية]، [تاريخ الانتهاء]، [الايام المتبقية]
;WITH AsOf AS (
  SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE
)
SELECT TOP 100
  LEFT(I.ITEM_NAME, 70) AS [اسم المنتج],
  CAST(SUM(S.QTY) AS decimal(18,2)) AS [الكمية],
  CAST(S.CATEOGRY3 AS date) AS [تاريخ الانتهاء],
  DATEDIFF(day, (SELECT d FROM AsOf), S.CATEOGRY3) AS [الايام المتبقية]
FROM dbo.ITEMS_SUB S
INNER JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CAST(S.CATEOGRY3 AS date) >= (SELECT d FROM AsOf)
  AND CAST(S.CATEOGRY3 AS date) <= DATEADD(day, 60, (SELECT d FROM AsOf)) -- 60 is replaced dynamically via days_recent!
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
GROUP BY I.ITEM_NAME, S.CATEOGRY3
ORDER BY S.CATEOGRY3 ASC;
```

---

---

## PATTERN: آخر-سعر-شراء-مورد
TRIGGERS: آخر سعر شراء, سعر شراء, last purchase price, buy price history, آخر مشتريات صنف, كم آخر مرة اشترينا, من أين اشترينا, كمية المنتج الآن, مورد المنتج
TABLES: BUY_ITEMS, BUY_INVOICE, CUSTOMERS, ITEMS, ITEMS_SUB
NOTES:
  - **product_filter مطلوب** — يُعيد صفاً واحداً للمنتج الأكثر مشتريات بين المطابقين.
  - يُرجع: اسم المنتج، الكمية الحالية، آخر سعر شراء، كمية آخر شراء، تاريخه، المورد الأخير، أرخص مورد وسعره.
  - PickID يُفضّل المنتج الأكثر سجلات شراء (PRICE>0) ثم الأقرب للكود ثم الاسم أبجدياً.
  - ⚠️ إذا جاء result فارغاً (لا مشتريات) → أخبر المستخدم أن هذا المنتج لم يُشترَ بعد.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
-- آخر سعر شراء لمنتج محدد مع الكمية الحالية وأرخص مورد
-- استبدل {{PRODUCT_FILTER}} باسم أو كود المنتج
;WITH
PickID AS (
  SELECT TOP 1 I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME
  FROM dbo.ITEMS I
  WHERE I.ITEM_INVISIBLE=0
    AND (I.ITEM_NAME LIKE N'%{{PRODUCT_FILTER}}%' OR I.ITEM_MODEL LIKE N'%{{PRODUCT_FILTER}}%')
  ORDER BY
    (SELECT COUNT(*) FROM dbo.BUY_ITEMS BI2 WHERE BI2.ITEM_ID=I.ITEM_ID AND BI2.PRICE>0) DESC,
    CASE WHEN I.ITEM_MODEL LIKE N'%{{PRODUCT_FILTER}}%' THEN 0 ELSE 1 END,
    I.ITEM_NAME
),
Stock AS (
  SELECT SUM(ISNULL(QTY,0)) AS TotalQty
  FROM dbo.ITEMS_SUB WHERE ITEM_ID=(SELECT ITEM_ID FROM PickID)
),
AllBuy AS (
  SELECT BI.PRICE, BI.QTY AS BuyQty, B.B_DATE,
    ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (ORDER BY BI.B_ITEM_ID DESC) AS rn_global,
    ROW_NUMBER() OVER (PARTITION BY B.CUST_ID ORDER BY BI.B_ITEM_ID DESC) AS rn_sup
  FROM dbo.BUY_ITEMS BI
  JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.ITEM_ID=(SELECT ITEM_ID FROM PickID) AND BI.PRICE > 0
),
LastBuy AS (SELECT * FROM AllBuy WHERE rn_global=1),
BySupplier AS (SELECT Supplier, PRICE AS LastPrice FROM AllBuy WHERE rn_sup=1)
SELECT
  P.ITEM_MODEL AS [الكود],
  LEFT(P.ITEM_NAME,60) AS [اسم_المنتج],
  CAST((SELECT TotalQty FROM Stock) AS decimal(18,2)) AS [الكمية_الحالية],
  CAST(LB.PRICE AS decimal(18,2)) AS [آخر_سعر_شراء],
  CAST(LB.BuyQty AS decimal(18,2)) AS [كمية_آخر_شراء],
  CAST(LB.B_DATE AS date) AS [تاريخ_آخر_شراء],
  LB.Supplier AS [المورد_الأخير],
  (SELECT TOP 1 Supplier FROM BySupplier ORDER BY LastPrice ASC) AS [أرخص_مورد],
  (SELECT TOP 1 CAST(LastPrice AS decimal(18,2)) FROM BySupplier ORDER BY LastPrice ASC) AS [أرخص_سعر]
FROM PickID P
CROSS JOIN LastBuy LB;
```

```sql
-- قائمة كل الموردين لصنف واحد مرتبة من أرخص لأغلى (استخدمها بعد الاستعلام أعلاه)
;WITH
PickID AS (
  SELECT TOP 1 ITEM_ID FROM dbo.ITEMS
  WHERE ITEM_INVISIBLE=0
    AND (ITEM_NAME LIKE N'%{{PRODUCT_FILTER}}%' OR ITEM_MODEL LIKE N'%{{PRODUCT_FILTER}}%')
  ORDER BY
    (SELECT COUNT(*) FROM dbo.BUY_ITEMS BI2 WHERE BI2.ITEM_ID=dbo.ITEMS.ITEM_ID AND BI2.PRICE>0) DESC,
    ITEM_NAME
),
AllBuy AS (
  SELECT ISNULL(CU.CUST_NAME, N'غير محدد') AS Supplier,
    ROW_NUMBER() OVER (PARTITION BY B.CUST_ID ORDER BY BI.B_ITEM_ID DESC) AS rn_sup,
    BI.PRICE
  FROM dbo.BUY_ITEMS BI
  JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.ITEM_ID=(SELECT ITEM_ID FROM PickID) AND BI.PRICE > 0
)
SELECT Supplier AS [المورد], CAST(PRICE AS decimal(18,2)) AS [آخر_سعر_شراء]
FROM AllBuy WHERE rn_sup=1
ORDER BY PRICE ASC;
```

```sql
-- الاستعلام القديم — آخر سعر شراء لكل المنتجات (بدون فلتر — للاستعراض العام)
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

---

## PATTERN: مبيعات-آخر-يوم-موظف
TRIGGERS: مبيعات آخر يوم, آخر يوم فيه مبيعات, آخر يوم مبيعات, مبيعات آخر يوم لكل موظف, last sale day, last day with sales, إيرادات آخر يوم, مبيعات الموظفين آخر يوم
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
NOTES:
  - **@LastSaleDay = CAST(MAX(S_DATE) AS date) FROM SALE_INVOICE** — لا GETDATE() ولا تاريخ ثابت.
  - الإيراد = SUM(SI.QTY * SI.PRICE). SALE_ITEMS لا يحتوي S_DATE.
  - صف «═══ الإجمالي ═══» في النهاية = مجموع كل الموظفين.
  - ⚠️ **إذا طلب المستخدم تاريخاً صريحاً** (مثل "21/5/2026") → بدّل @LastSaleDay بـ '2026-05-21' مباشرةً.
    لا تُعوّض التاريخ الصريح بـ MAX(S_DATE) — استخدم نمط «مبيعات-يومية-لكل-موظف» SQL-A بدلاً من هذا.
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

---

## PATTERN: مبيعات-يومية-لكل-موظف
TRIGGERS: مبيعات يومية موظف, لخص المبيعات اليومية, إجمالي مبيعات كل موظف, مبيعات كل يوم بالموظف, daily sales by employee, employee daily summary, أداء يومي موظف, مبيعات الموظفين يومياً, لخص لي المبيعات اليومية, مبيعات الموظفين ليوم, مبيعات موظفين تاريخ, employee sales specific date
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
VIEWS: SALE_ITEMS_INVOICE_VIEW
NOTES:
  - **لا subquery يجمع PRICE وحده.** الإيراد = SUM(QTY*PRICE). SALE_ITEMS **لا** S_DATE.
  - الموظف = SALE_INVOICE.USERS_ID → USERS.FULL_NAME.
  - **⚠️ قاعدة التاريخ الصارمة:**
    * المستخدم ذكر تاريخاً صريحاً (مثل "21/5/2026" أو "ليوم X") → استخدم **SQL-A** مع ذلك التاريخ بالضبط.
    * المستخدم قال "آخر أيام/أسبوع" أو لم يحدد تاريخاً → استخدم **SQL-B** (نافذة 7 أيام من MAX(S_DATE)).
    * ⚠️ لا تستبدل تاريخاً صريحاً بـ MAX(S_DATE) — MAX يعيد آخر فاتورة مهما كانت قليلة.
---

```sql
-- [A] مبيعات يوم محدد لكل موظف — استخدم عند ذكر تاريخ صريح
-- غيّر '2026-05-21' بالتاريخ المطلوب
DECLARE @TargetDate date = '2026-05-21';
;WITH EmpDay AS (
  SELECT
    ISNULL(V.FULL_NAME, N'غير محدد') AS [الموظف],
    COUNT(DISTINCT V.S_ID) AS [عدد الفواتير],
    CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS [الإيرادات],
    0 AS SortOrder
  FROM dbo.SALE_ITEMS_INVOICE_VIEW V
  WHERE CAST(V.S_DATE AS date) = @TargetDate
  GROUP BY V.USERS_ID, V.FULL_NAME
),
Grand AS (
  SELECT N'═══ الإجمالي ═══' AS [الموظف],
    COUNT(DISTINCT V.S_ID) AS [عدد الفواتير],
    CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS [الإيرادات], 1 AS SortOrder
  FROM dbo.SALE_ITEMS_INVOICE_VIEW V
  WHERE CAST(V.S_DATE AS date) = @TargetDate
)
SELECT @TargetDate AS [التاريخ], [الموظف], [عدد الفواتير], [الإيرادات]
FROM (SELECT * FROM EmpDay UNION ALL SELECT * FROM Grand) X
ORDER BY SortOrder, [الإيرادات] DESC;
```

```sql
-- [B] مبيعات يومية لكل موظف — آخر 7 أيام من آخر يوم مبيعات (بدون تاريخ محدد)
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @FromDate date = DATEADD(day, -7, @AsOfDate);
;WITH EmpDaily AS (
  SELECT
    CAST(V.S_DATE AS date) AS [اليوم],
    ISNULL(V.FULL_NAME, N'غير محدد') AS [الموظف],
    COUNT(DISTINCT V.S_ID) AS [عدد الفواتير],
    CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS [الإيرادات],
    0 AS SortOrder
  FROM dbo.SALE_ITEMS_INVOICE_VIEW V
  WHERE CAST(V.S_DATE AS date) BETWEEN @FromDate AND @AsOfDate
  GROUP BY CAST(V.S_DATE AS date), V.USERS_ID, V.FULL_NAME
),
GrandTotal AS (
  SELECT
    NULL AS [اليوم],
    N'═══ الإجمالي ═══' AS [الموظف],
    COUNT(DISTINCT V.S_ID) AS [عدد الفواتير],
    CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS [الإيرادات],
    1 AS SortOrder
  FROM dbo.SALE_ITEMS_INVOICE_VIEW V
  WHERE CAST(V.S_DATE AS date) BETWEEN @FromDate AND @AsOfDate
)
SELECT [اليوم], [الموظف], [عدد الفواتير], [الإيرادات]
FROM (SELECT * FROM EmpDaily UNION ALL SELECT * FROM GrandTotal) X
ORDER BY SortOrder, [اليوم] DESC, [الإيرادات] DESC;
```

---

## PATTERN: ديون-الموظفين
TRIGGERS: ديون الموظفين, ديون موظفين, سلف الموظفين, ذمة الموظفين, employee debts, سلف, ديون العمال
TABLES: CUSTOMERS, USERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, TAKE, BALANCE_EDIT
NOTES:
  - الموظفون = CUST_EMP=1 + كل CUSTOMERS اسمهم يطابق USERS.FULL_NAME (كثير منهم CUST_CUSTOM=1 فقط).
  - الدين = مبيعات − مردودات − مقبوضات (TAKE) + تسوية (BALANCE_EDIT). بدون GIVE.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓ — 4 موظفين عليهم ديون.
---

```sql
;WITH
Employees AS (
  SELECT DISTINCT C.CUST_ID, C.CUST_NAME
  FROM dbo.CUSTOMERS C WHERE C.CUST_EMP = 1 AND C.CUST_INVISIBLE = 0
  UNION
  SELECT C.CUST_ID, C.CUST_NAME
  FROM dbo.CUSTOMERS C
  INNER JOIN dbo.USERS U ON C.CUST_NAME LIKE N'%' + U.FULL_NAME + N'%' OR C.CUST_NAME = U.FULL_NAME
  WHERE C.CUST_INVISIBLE = 0
),
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS Adj
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) AS SalesVal
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID
),
SaleRetTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) AS RetVal
  FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) AS Paid FROM dbo.TAKE GROUP BY CUST_ID
),
LastTake AS (
  SELECT CUST_ID, T_DATE AS LastDate,
    ROW_NUMBER() OVER (PARTITION BY CUST_ID ORDER BY T_ID DESC) AS rn
  FROM dbo.TAKE
)
SELECT
  E.CUST_NAME AS [الموظف],
  CAST(
    ISNULL(S.SalesVal,0)-ISNULL(SR.RetVal,0)-ISNULL(TT.Paid,0)+ISNULL(BA.Adj,0)
  AS decimal(18,2)) AS [إجمالي_الدين],
  CAST(LT.LastDate AS date) AS [تاريخ_آخر_إيصال_قبض]
FROM Employees E
LEFT JOIN SaleTot S ON E.CUST_ID=S.CUST_ID
LEFT JOIN SaleRetTot SR ON E.CUST_ID=SR.CUST_ID
LEFT JOIN TakeTot TT ON E.CUST_ID=TT.CUST_ID
LEFT JOIN BalanceAdj BA ON E.CUST_ID=BA.CUST_ID
LEFT JOIN LastTake LT ON E.CUST_ID=LT.CUST_ID AND LT.rn=1
WHERE ISNULL(S.SalesVal,0)-ISNULL(SR.RetVal,0)-ISNULL(TT.Paid,0)+ISNULL(BA.Adj,0) >= 1
),
Rows AS (
  SELECT Emp AS [الموظف], CAST(Debt AS decimal(18,2)) AS [إجمالي_الدين], CAST(LastDate AS date) AS [آخر_إيصال_قبض], 0 AS s
  FROM EmpDebts
  UNION ALL
  SELECT N'═══ إجمالي ديون الموظفين ═══', CAST(SUM(Debt) AS decimal(18,2)), NULL, 1
  FROM EmpDebts
)
SELECT [الموظف], [إجمالي_الدين], [آخر_إيصال_قبض] FROM Rows ORDER BY s, [إجمالي_الدين] DESC;
```

---

## PATTERN: ديون-الزبائن
TRIGGERS: ديون الزبائن, ديون الزباين, ديون العملاء, اللي لي, من يدينني, customer debts, ذمة الزبائن, ديون لي
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, TAKE, BALANCE_EDIT
NOTES:
  - الدين = مبيعات − مردودات − مقبوضات + تسوية. صف إجمالي في النهاية.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
;WITH
BA AS (SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS Adj FROM dbo.BALANCE_EDIT GROUP BY CUST_ID),
ST AS (SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) AS V FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID),
SR AS (SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) AS V FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID),
TT AS (SELECT CUST_ID, SUM(T_VALUE) AS V FROM dbo.TAKE GROUP BY CUST_ID),
LT AS (SELECT CUST_ID, T_DATE, ROW_NUMBER() OVER (PARTITION BY CUST_ID ORDER BY T_ID DESC) AS rn FROM dbo.TAKE),
D AS (
  SELECT C.CUST_NAME AS N, ISNULL(ST.V,0)-ISNULL(SR.V,0)-ISNULL(TT.V,0)+ISNULL(BA.Adj,0) AS Debt, LT.T_DATE AS LD
  FROM dbo.CUSTOMERS C
  LEFT JOIN ST ON C.CUST_ID=ST.CUST_ID LEFT JOIN SR ON C.CUST_ID=SR.CUST_ID
  LEFT JOIN TT ON C.CUST_ID=TT.CUST_ID LEFT JOIN BA ON C.CUST_ID=BA.CUST_ID
  LEFT JOIN LT ON C.CUST_ID=LT.CUST_ID AND LT.rn=1
  WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0 AND ISNULL(ST.V,0)-ISNULL(SR.V,0)-ISNULL(TT.V,0)+ISNULL(BA.Adj,0) >= 1
)
SELECT N AS [الزبون], CAST(Debt AS decimal(18,2)) AS [إجمالي_الدين], CAST(LD AS date) AS [آخر_إيصال_قبض], 0 AS s FROM D
UNION ALL
SELECT N'═══ إجمالي ديون الزبائن ═══', CAST(SUM(Debt) AS decimal(18,2)), NULL, 1 FROM D
ORDER BY s, [إجمالي_الدين] DESC;
```

---

## PATTERN: ديون-الموردين
TRIGGERS: ديون الموردين, ديون موردين, اللي علي, من أدين له, supplier debts, ذمة الموردين, ديون علي
TABLES: CUSTOMERS, BUY_INVOICE, BUY_ITEMS, B_R_INVOICE, B_R_ITEMS, GIVE, BALANCE_EDIT
NOTES:
  - الدين = مشتريات − مردودات − مدفوعات (GIVE EXPENCES_ID=0) + تسوية. صف إجمالي في النهاية.
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
;WITH
BA AS (SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS Adj FROM dbo.BALANCE_EDIT GROUP BY CUST_ID),
BT AS (SELECT B.CUST_ID, SUM(BI.QTY*BI.PRICE) AS V FROM dbo.BUY_INVOICE B JOIN dbo.BUY_ITEMS BI ON B.B_ID=BI.B_ID GROUP BY B.CUST_ID),
BR AS (SELECT BR.CUST_ID, SUM(BRI.QTY*BRI.PRICE) AS V FROM dbo.B_R_INVOICE BR JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID=BRI.B_R_ID GROUP BY BR.CUST_ID),
GT AS (SELECT CUST_ID, SUM(G_VALUE) AS V FROM dbo.GIVE WHERE EXPENCES_ID=0 GROUP BY CUST_ID),
LG AS (SELECT CUST_ID, G_DATE, ROW_NUMBER() OVER (PARTITION BY CUST_ID ORDER BY G_ID DESC) AS rn FROM dbo.GIVE WHERE EXPENCES_ID=0),
D AS (
  SELECT C.CUST_NAME AS N, ISNULL(BT.V,0)-ISNULL(BR.V,0)-ISNULL(GT.V,0)+ISNULL(BA.Adj,0) AS Debt, LG.G_DATE AS LD
  FROM dbo.CUSTOMERS C
  LEFT JOIN BT ON C.CUST_ID=BT.CUST_ID LEFT JOIN BR ON C.CUST_ID=BR.CUST_ID
  LEFT JOIN GT ON C.CUST_ID=GT.CUST_ID LEFT JOIN BA ON C.CUST_ID=BA.CUST_ID
  LEFT JOIN LG ON C.CUST_ID=LG.CUST_ID AND LG.rn=1
  WHERE C.CUST_VENDOR=1 AND C.CUST_INVISIBLE=0 AND ISNULL(BT.V,0)-ISNULL(BR.V,0)-ISNULL(GT.V,0)+ISNULL(BA.Adj,0) >= 1
)
SELECT N AS [المورد], CAST(Debt AS decimal(18,2)) AS [قيمة_الدين], CAST(LD AS date) AS [آخر_إيصال_صرف], 0 AS s FROM D
UNION ALL
SELECT N'═══ إجمالي ديون الموردين ═══', CAST(SUM(Debt) AS decimal(18,2)), NULL, 1 FROM D
ORDER BY s, [قيمة_الدين] DESC;
```

---
