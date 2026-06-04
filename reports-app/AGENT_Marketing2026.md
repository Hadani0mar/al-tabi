# AGENT_Marketing2026 — أنماط SQL جاهزة للتنفيذ
# ERP: Marketing2026 | SQL Server 2005+ | schema: dbo
# انسخ SQL من ## PATTERN أدناه ونفّذه بـ execute_raw_sql. لا تخترع SQL.
# ⚠️ SQL 2005: لا CONVERT(varchar(10), x, 120) — استخدم CONVERT(varchar(10),x,120). لا STRING_AGG/IIF/FORMAT/OFFSET.
# ⚠️ SALE_ITEMS لا يحتوي S_DATE — استخدم JOIN مع SALE_INVOICE.
# ⚠️ تاريخ صريح → استخدمه مباشرةً. لا تستبدله بـ MAX(S_DATE).
# ⚠️ الديون: يجب طرح S_DISCOUNT (خصم فاتورة البيع) و B_DISCOUNT (خصم فاتورة الشراء).

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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
),
SalesWindow AS (
  SELECT SI.ITEM_ID, SUM(SI.QTY) AS SoldQty, SUM(SI.QTY * SI.PRICE) AS Revenue
  FROM dbo.SALE_ITEMS SI
  JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE CONVERT(varchar(10), INV.S_DATE, 120) >= DATEADD(day, -30, (SELECT d FROM AsOf))
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
),
SalesRecent AS (
  SELECT SI.ITEM_ID,
    SUM(SI.QTY) AS SoldQty,
    SUM(SI.QTY * SI.PRICE) AS Revenue,
    COUNT(DISTINCT CONVERT(varchar(10), INV.S_DATE, 120)) AS ActiveDays
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE CONVERT(varchar(10), INV.S_DATE, 120) >= DATEADD(day, -60, (SELECT d FROM AsOf))
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
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
  SELECT DATEADD(month, -1, CONVERT(varchar(10), MAX(S_DATE), 120)) AS d FROM dbo.SALE_INVOICE
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
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
  CONVERT(varchar(10), BS.B_DATE, 120)                            AS [تاريخ_آخر_شراء],
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT SI.ITEM_ID, SUM(SI.QTY) AS SoldQty
  FROM dbo.SALE_ITEMS SI
  JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  WHERE CONVERT(varchar(10), INV.S_DATE, 120) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
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
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
)
SELECT TOP 100
  LEFT(I.ITEM_NAME, 70) AS [اسم المنتج],
  CAST(SUM(S.QTY) AS decimal(18,2)) AS [الكمية],
  CONVERT(varchar(10), S.CATEOGRY3, 120) AS [تاريخ الانتهاء]
FROM dbo.ITEMS_SUB S
INNER JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CONVERT(varchar(10), S.CATEOGRY3, 120) < (SELECT d FROM AsOf)
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
GROUP BY I.ITEM_NAME, S.CATEOGRY3
ORDER BY
  CASE
    WHEN YEAR(S.CATEOGRY3) = YEAR(CAST((SELECT d FROM AsOf) AS date)) THEN 0
    WHEN YEAR(S.CATEOGRY3) < YEAR(CAST((SELECT d FROM AsOf) AS date)) THEN 1
    ELSE 2
  END,
  S.CATEOGRY3 ASC;
```

```sql
-- 2. المنتجات التي ستنتهي صلاحيتها خلال فترة مخصصة (أيام - الافتراضي 60 يوماً ويُستبدل ديناميكياً)
-- الأعمدة: [اسم المنتج]، [الكمية]، [تاريخ الانتهاء]، [الايام المتبقية]
;WITH AsOf AS (
  SELECT CONVERT(varchar(10), MAX(S_DATE), 120) AS d FROM dbo.SALE_INVOICE
)
SELECT TOP 100
  LEFT(I.ITEM_NAME, 70) AS [اسم المنتج],
  CAST(SUM(S.QTY) AS decimal(18,2)) AS [الكمية],
  CONVERT(varchar(10), S.CATEOGRY3, 120) AS [تاريخ الانتهاء],
  DATEDIFF(day, CAST((SELECT d FROM AsOf) AS date), CAST(S.CATEOGRY3 AS date)) AS [الايام المتبقية]
FROM dbo.ITEMS_SUB S
INNER JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CONVERT(varchar(10), S.CATEOGRY3, 120) >= (SELECT d FROM AsOf)
  AND CONVERT(varchar(10), S.CATEOGRY3, 120) <= DATEADD(day, 60, (SELECT d FROM AsOf)) -- 60 is replaced dynamically via days_recent!
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
GROUP BY I.ITEM_NAME, S.CATEOGRY3
ORDER BY
  CASE
    WHEN YEAR(S.CATEOGRY3) = YEAR(CAST((SELECT d FROM AsOf) AS date)) THEN 0
    WHEN YEAR(S.CATEOGRY3) < YEAR(CAST((SELECT d FROM AsOf) AS date)) THEN 1
    ELSE 2
  END,
  S.CATEOGRY3 ASC;
```

---

---

## PATTERN: بطل-بيع-قرب-الصلاحية
TRIGGERS: بطل المبيعات, الموظف المنقذ, المنتجات قرب الصلاحية المباعة, مبيعات قرب الصلاحية, خسارة تم تداركها, بطل بيع الصلاحية, near expiry sales hero, saved expiry sales, expiry sales by employee
TABLES: SALE_INVOICE, SALE_ITEMS, USERS, SITTEINGS
NOTES:
  - يرتب الموظفين حسب قيمة المنتجات التي بيعت وهي داخل فترة خطر الصلاحية.
  - فترة الخطر = `SITTEINGS.EXPIRY_WORRNING`، والافتراضي 60 يوماً إذا لم توجد قيمة.
  - SQL-A: شهر محدد. غيّر `TargetMonth=5` و `TargetYear=2026` حسب طلب المستخدم.
  - SQL-B: شهر آخر فاتورة مبيعات تلقائياً.
  - لا يستخدم `CAST(x AS date)` حتى يبقى متوافقاً مع SQL Server 2005.
  - قيمة الخسارة المتداركة = `SUM(SALE_ITEMS.QTY * SALE_ITEMS.PRICE)`.
---

```sql
-- [A] بطل بيع المنتجات القريبة من الصلاحية لشهر محدد
-- غيّر TargetMonth و TargetYear حسب الشهر المطلوب
;WITH Params AS (
  SELECT 5 AS TargetMonth, 2026 AS TargetYear
),
SystemSettings AS (
  SELECT ISNULL(MAX(EXPIRY_WORRNING), 60) AS WarningDays
  FROM dbo.SITTEINGS
),
NearExpirySales AS (
  SELECT
    ISNULL(U.FULL_NAME, N'غير محدد') AS UserName,
    INV.S_ID,
    SI.QTY,
    SI.QTY * SI.PRICE AS TotalItemValue,
    DATEDIFF(day, CONVERT(varchar(10), INV.S_DATE, 120), SI.CATEOGRY3) AS DaysToExpiry
  FROM dbo.SALE_INVOICE INV
  INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
  CROSS JOIN SystemSettings SS
  CROSS JOIN Params P
  WHERE SI.CATEOGRY3 IS NOT NULL
    AND CONVERT(varchar(10), SI.CATEOGRY3, 120) >= CONVERT(varchar(10), INV.S_DATE, 120)
    AND DATEDIFF(day, CONVERT(varchar(10), INV.S_DATE, 120), SI.CATEOGRY3) <= SS.WarningDays
    AND MONTH(INV.S_DATE) = P.TargetMonth
    AND YEAR(INV.S_DATE) = P.TargetYear
)
SELECT
  UserName AS [الموظف_المنقذ],
  COUNT(DISTINCT S_ID) AS [عدد_الفواتير_المنقذة],
  CAST(SUM(QTY) AS decimal(18,2)) AS [الكمية_المباعة_فترة_الخطر],
  CAST(SUM(TotalItemValue) AS decimal(18,2)) AS [إجمالي_الخسارة_التي_تم_تداركها],
  MIN(DaysToExpiry) AS [أكثر_منتج_حرج_أيام_قبل_التلف]
FROM NearExpirySales
GROUP BY UserName
ORDER BY [إجمالي_الخسارة_التي_تم_تداركها] DESC;
```

```sql
-- [B] بطل بيع المنتجات القريبة من الصلاحية في شهر آخر فاتورة مبيعات
;WITH AsOf AS (
  SELECT MAX(S_DATE) AS d FROM dbo.SALE_INVOICE
),
SystemSettings AS (
  SELECT ISNULL(MAX(EXPIRY_WORRNING), 60) AS WarningDays
  FROM dbo.SITTEINGS
),
NearExpirySales AS (
  SELECT
    ISNULL(U.FULL_NAME, N'غير محدد') AS UserName,
    INV.S_ID,
    SI.QTY,
    SI.QTY * SI.PRICE AS TotalItemValue,
    DATEDIFF(day, CONVERT(varchar(10), INV.S_DATE, 120), SI.CATEOGRY3) AS DaysToExpiry
  FROM dbo.SALE_INVOICE INV
  INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
  CROSS JOIN SystemSettings SS
  WHERE SI.CATEOGRY3 IS NOT NULL
    AND CONVERT(varchar(10), SI.CATEOGRY3, 120) >= CONVERT(varchar(10), INV.S_DATE, 120)
    AND DATEDIFF(day, CONVERT(varchar(10), INV.S_DATE, 120), SI.CATEOGRY3) <= SS.WarningDays
    AND MONTH(INV.S_DATE) = MONTH((SELECT d FROM AsOf))
    AND YEAR(INV.S_DATE) = YEAR((SELECT d FROM AsOf))
)
SELECT
  UserName AS [الموظف_المنقذ],
  COUNT(DISTINCT S_ID) AS [عدد_الفواتير_المنقذة],
  CAST(SUM(QTY) AS decimal(18,2)) AS [الكمية_المباعة_فترة_الخطر],
  CAST(SUM(TotalItemValue) AS decimal(18,2)) AS [إجمالي_الخسارة_التي_تم_تداركها],
  MIN(DaysToExpiry) AS [أكثر_منتج_حرج_أيام_قبل_التلف]
FROM NearExpirySales
GROUP BY UserName
ORDER BY [إجمالي_الخسارة_التي_تم_تداركها] DESC;
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
  CONVERT(varchar(10), LB.B_DATE, 120) AS [تاريخ_آخر_شراء],
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
  CONVERT(varchar(10), B.B_DATE, 120) AS LastBuyDate,
  CU.CUST_NAME AS Supplier,
  CONVERT(varchar(10), BI.CATEOGRY3, 120) AS ExpiryDate
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

## PATTERN: تقرير-المبيعات-والديون-اليومي
TRIGGERS: تقرير المبيعات والديون, المبيعات والديون, daily_sales_report, مبيعات وديون, مبيعات الكاش والديون, كاش وديون الموظفين, تقرير الكاش والديون, مبيعات الموظفين والديون
TABLES: SALE_INVOICE, SALE_ITEMS, USERS, CUSTOMERS
NOTES:
  - هذا النمط يستبدل أي تقرير قديم للمبيعات والديون اليومية.
  - الفاتورة تُحسب مرة واحدة فقط حسب تاريخ إصدارها `SALE_INVOICE.S_DATE`.
  - لا تستخدم تاريخ إغلاق أو أي تاريخ آخر لهذا التقرير.
  - الكاش = `CUST_ID = 1`.
  - الديون = أي زبون غير النقدي `CUST_ID <> 1`.
  - الفترة تعمل بصيغة `[StartDate, EndDate)`، أي أن تاريخ النهاية غير مشمول.
  - الموظفون المعتمدون افتراضياً: `7, 9, 11, 13, 21`.
  - غيّر `@StartDate` و `@EndDate` عند وجود تاريخ صريح في طلب المستخدم.
  - الاستعلام الأول ملخص سريع، والثاني تفاصيل الفواتير وأسماء الزبائن.
---

```sql
DECLARE @StartDate DATETIME;
DECLARE @EndDate DATETIME;
SET @StartDate = '2026-06-03';
SET @EndDate = '2026-06-05';

SELECT
    u.USERS_ID as [#],
    u.FULL_NAME as [الموظف],
    CAST(ISNULL(SUM(CASE WHEN s.CUST_ID <> 1 THEN si.QTY * si.PRICE ELSE 0 END), 0) AS DECIMAL(10,2)) as [ديون],
    CAST(ISNULL(SUM(CASE WHEN s.CUST_ID = 1 THEN si.QTY * si.PRICE ELSE 0 END), 0) AS DECIMAL(10,2)) as [كاش],
    CAST(ISNULL(SUM(si.QTY * si.PRICE), 0) AS DECIMAL(10,2)) as [المجموع]
FROM USERS u
LEFT JOIN SALE_INVOICE s ON u.USERS_ID = s.USERS_ID 
    AND s.S_DATE >= @StartDate 
    AND s.S_DATE < @EndDate
LEFT JOIN SALE_ITEMS si ON s.S_ID = si.S_ID
WHERE u.USERS_ID IN (7, 9, 11, 13, 21)
GROUP BY u.USERS_ID, u.FULL_NAME
HAVING ISNULL(SUM(si.QTY * si.PRICE), 0) > 0
ORDER BY u.USERS_ID;
```

```sql
DECLARE @StartDate DATETIME;
DECLARE @EndDate DATETIME;
SET @StartDate = '2026-06-03';
SET @EndDate = '2026-06-05';

SELECT
    u.USERS_ID as [رقم],
    u.FULL_NAME as [الموظف],
    CASE WHEN s.CUST_ID = 1 THEN 'كاش' ELSE 'ديون' END as [النوع],
    c.CUST_NAME as [اسم_الزبون],
    s.S_ID as [رقم_الفاتورة],
    CONVERT(VARCHAR(19), s.S_DATE, 121) as [التاريخ_والوقت],
    CAST(SUM(si.QTY * si.PRICE) AS DECIMAL(10,2)) as [القيمة],
    s.CUST_ID
FROM SALE_INVOICE s
INNER JOIN USERS u ON s.USERS_ID = u.USERS_ID
LEFT JOIN SALE_ITEMS si ON s.S_ID = si.S_ID
LEFT JOIN CUSTOMERS c ON s.CUST_ID = c.CUST_ID
WHERE s.S_DATE >= @StartDate
  AND s.S_DATE < @EndDate
  AND u.USERS_ID IN (7, 9, 11, 13, 21)
GROUP BY 
    u.USERS_ID,
    u.FULL_NAME,
    s.CUST_ID,
    CASE WHEN s.CUST_ID = 1 THEN 'كاش' ELSE 'ديون' END,
    c.CUST_NAME,
    s.S_ID,
    CONVERT(VARCHAR(19), s.S_DATE, 121)
ORDER BY u.USERS_ID, s.CUST_ID DESC, s.S_ID DESC;
```

---

## PATTERN: مبيعات-آخر-يوم-موظف
TRIGGERS: مبيعات آخر يوم, آخر يوم فيه مبيعات, آخر يوم مبيعات, مبيعات آخر يوم لكل موظف, last sale day, last day with sales, إيرادات آخر يوم, مبيعات الموظفين آخر يوم, مبيعات الموظفين, إيرادات الموظفين, ايرادات الموظفين, مبيعات يومية موظف
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
NOTES:
  - **@LastSaleDay = CONVERT(varchar(10), MAX(S_DATE), 120) FROM SALE_INVOICE** — لا GETDATE() ولا تاريخ ثابت.
  - الإيراد = SUM(SI.QTY * SI.PRICE). SALE_ITEMS لا يحتوي S_DATE.
  - النتيجة تعرض كل فاتورة في سطر مستقل، مع إجمالي الموظف في أول سطر فقط.
  - هذا هو النمط الوحيد المعتمد لمبيعات الموظفين وإيراداتهم. لا تستخدم أي استعلام آخر لهذا الغرض.
  - لا تستخدمه لسؤال تاريخ محدد إلا إذا عُدّل @LastDate صراحة.
  - ملف مُختبَر: `reports-app/last_sale_day_by_employee.sql`
---

```sql
DECLARE @LastDate VARCHAR(10)
SET @LastDate = (SELECT TOP 1 CONVERT(VARCHAR(10), S_DATE, 121) FROM SALE_INVOICE WHERE USERS_ID = 11 ORDER BY S_DATE DESC)

SELECT
  CASE WHEN ROW_NUMBER() OVER (PARTITION BY u.USERS_ID ORDER BY s.S_ID DESC) = 1
    THEN u.USER_NAMES
    ELSE ''
  END as 'الموظف',
  s.S_ID as 'رقم الفاتورة',
  CAST(SUM(si.QTY * si.PRICE) AS DECIMAL(10,2)) as 'قيمة الفاتورة',
  CASE WHEN ROW_NUMBER() OVER (PARTITION BY u.USERS_ID ORDER BY s.S_ID DESC) = 1
    THEN (SELECT CAST(SUM(si2.QTY * si2.PRICE) AS DECIMAL(10,2))
          FROM SALE_ITEMS si2
          JOIN SALE_INVOICE s2 ON si2.S_ID = s2.S_ID
          WHERE s2.USERS_ID = u.USERS_ID
          AND CONVERT(VARCHAR(10), s2.S_DATE, 121) = @LastDate)
    ELSE NULL
  END as 'الإجمالي'
FROM SALE_INVOICE s
LEFT JOIN SALE_ITEMS si ON s.S_ID = si.S_ID
LEFT JOIN USERS u ON s.USERS_ID = u.USERS_ID
WHERE CONVERT(VARCHAR(10), s.S_DATE, 121) = @LastDate
GROUP BY u.USER_NAMES, u.USERS_ID, s.S_ID
ORDER BY u.USER_NAMES, s.S_ID DESC

GO
```

## PATTERN: ترتيب-الموظفين
TRIGGERS: ترتيب الموظفين, أفضل موظف, أعلى دخل, أداء الموظفين, أعلى معدل بيع, موظف الشهر, employee ranking, best employee, معدل الدخل, متوسط الفاتورة
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
NOTES:
  - **3 أوضاع:** SQL-A آخر 30 يوم (أو 90). SQL-B هذا الشهر. SQL-C الشهر السابق.
  - «ترتيب/أداء» بدون تحديد → SQL-A. «هذا الشهر» → SQL-B. «الشهر السابق» → SQL-C.
  - مُختبَر SQL 2005 ✓
---

```sql
-- [A] ترتيب الموظفين آخر N يوم (عدّل -30 لأي فترة: -90 لـ 3 شهور)
;WITH AsOf AS (SELECT MAX(S_DATE) AS d FROM dbo.SALE_INVOICE),
EmpStats AS (
  SELECT ISNULL(U.FULL_NAME, N'غير محدد') AS Emp,
    COUNT(DISTINCT INV.S_ID) AS InvCount,
    CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS Revenue,
    COUNT(DISTINCT CONVERT(varchar(10), INV.S_DATE, 120)) AS ActiveDays
  FROM dbo.SALE_INVOICE INV JOIN dbo.SALE_ITEMS SI ON INV.S_ID=SI.S_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID=U.USERS_ID
  WHERE INV.S_DATE >= DATEADD(day,-30,(SELECT d FROM AsOf))
  GROUP BY U.USERS_ID, U.FULL_NAME
)
SELECT Emp AS [الموظف], InvCount AS [عدد_الفواتير], Revenue AS [الإيراد],
  ActiveDays AS [أيام_عمل],
  CAST(Revenue/NULLIF(CAST(ActiveDays AS float),0) AS decimal(18,2)) AS [معدل_يومي],
  CAST(Revenue/NULLIF(CAST(InvCount AS float),0) AS decimal(18,2)) AS [متوسط_الفاتورة],
  CAST(InvCount/NULLIF(CAST(ActiveDays AS float),0) AS decimal(18,1)) AS [فواتير_يوم]
FROM EmpStats ORDER BY Revenue DESC;
```

```sql
-- [B] ترتيب الموظفين هذا الشهر
;WITH AsOf AS (SELECT MAX(S_DATE) AS d FROM dbo.SALE_INVOICE),
EmpStats AS (
  SELECT ISNULL(U.FULL_NAME, N'غير محدد') AS Emp,
    COUNT(DISTINCT INV.S_ID) AS InvCount,
    CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS Revenue,
    COUNT(DISTINCT CONVERT(varchar(10), INV.S_DATE, 120)) AS ActiveDays
  FROM dbo.SALE_INVOICE INV JOIN dbo.SALE_ITEMS SI ON INV.S_ID=SI.S_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID=U.USERS_ID
  WHERE YEAR(INV.S_DATE)=YEAR((SELECT d FROM AsOf)) AND MONTH(INV.S_DATE)=MONTH((SELECT d FROM AsOf))
  GROUP BY U.USERS_ID, U.FULL_NAME
)
SELECT Emp AS [الموظف], InvCount AS [عدد_الفواتير], Revenue AS [الإيراد],
  ActiveDays AS [أيام_عمل],
  CAST(Revenue/NULLIF(CAST(ActiveDays AS float),0) AS decimal(18,2)) AS [معدل_يومي],
  CAST(Revenue/NULLIF(CAST(InvCount AS float),0) AS decimal(18,2)) AS [متوسط_الفاتورة],
  CAST(InvCount/NULLIF(CAST(ActiveDays AS float),0) AS decimal(18,1)) AS [فواتير_يوم]
FROM EmpStats ORDER BY Revenue DESC;
```

```sql
-- [C] ترتيب الموظفين الشهر السابق
;WITH AsOf AS (SELECT DATEADD(month,-1,MAX(S_DATE)) AS d FROM dbo.SALE_INVOICE),
EmpStats AS (
  SELECT ISNULL(U.FULL_NAME, N'غير محدد') AS Emp,
    COUNT(DISTINCT INV.S_ID) AS InvCount,
    CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS Revenue,
    COUNT(DISTINCT CONVERT(varchar(10), INV.S_DATE, 120)) AS ActiveDays
  FROM dbo.SALE_INVOICE INV JOIN dbo.SALE_ITEMS SI ON INV.S_ID=SI.S_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID=U.USERS_ID
  WHERE YEAR(INV.S_DATE)=YEAR((SELECT d FROM AsOf)) AND MONTH(INV.S_DATE)=MONTH((SELECT d FROM AsOf))
  GROUP BY U.USERS_ID, U.FULL_NAME
)
SELECT Emp AS [الموظف], InvCount AS [عدد_الفواتير], Revenue AS [الإيراد],
  ActiveDays AS [أيام_عمل],
  CAST(Revenue/NULLIF(CAST(ActiveDays AS float),0) AS decimal(18,2)) AS [معدل_يومي],
  CAST(Revenue/NULLIF(CAST(InvCount AS float),0) AS decimal(18,2)) AS [متوسط_الفاتورة],
  CAST(InvCount/NULLIF(CAST(ActiveDays AS float),0) AS decimal(18,1)) AS [فواتير_يوم]
FROM EmpStats ORDER BY Revenue DESC;
```

---

## PATTERN: ديون-الموظفين
TRIGGERS: ديون الموظفين, ديون موظفين, سلف الموظفين, ذمة الموظفين, employee debts, سلف, ديون العمال
TABLES: CUSTOMERS, USERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, TAKE, BALANCE_EDIT
NOTES:
  - الدين = مبيعات − خصم فواتير (S_DISCOUNT) − مردودات − مقبوضات + تسوية. SQL 2005 ✓
---

```sql
;WITH
Emp AS (
  SELECT DISTINCT C.CUST_ID, C.CUST_NAME FROM dbo.CUSTOMERS C WHERE C.CUST_EMP=1 AND C.CUST_INVISIBLE=0
  UNION
  SELECT C.CUST_ID, C.CUST_NAME FROM dbo.CUSTOMERS C
  INNER JOIN dbo.USERS U ON C.CUST_NAME LIKE N'%' + U.FULL_NAME + N'%' OR C.CUST_NAME=U.FULL_NAME
  WHERE C.CUST_INVISIBLE=0
),
BA AS (SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS Adj FROM dbo.BALANCE_EDIT GROUP BY CUST_ID),
ST AS (SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) AS GV, SUM(ISNULL(SI.S_DISCOUNT,0)) AS TD
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID),
SR AS (SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) AS V FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID),
TT AS (SELECT CUST_ID, SUM(T_VALUE) AS V FROM dbo.TAKE GROUP BY CUST_ID),
LT AS (SELECT CUST_ID, T_DATE, ROW_NUMBER() OVER (PARTITION BY CUST_ID ORDER BY T_ID DESC) AS rn FROM dbo.TAKE),
D AS (
  SELECT E.CUST_NAME AS N, ISNULL(ST.GV,0)-ISNULL(ST.TD,0)-ISNULL(SR.V,0)-ISNULL(TT.V,0)+ISNULL(BA.Adj,0) AS Debt, LT.T_DATE AS LD
  FROM Emp E LEFT JOIN ST ON E.CUST_ID=ST.CUST_ID LEFT JOIN SR ON E.CUST_ID=SR.CUST_ID
  LEFT JOIN TT ON E.CUST_ID=TT.CUST_ID LEFT JOIN BA ON E.CUST_ID=BA.CUST_ID
  LEFT JOIN LT ON E.CUST_ID=LT.CUST_ID AND LT.rn=1
  WHERE ISNULL(ST.GV,0)-ISNULL(ST.TD,0)-ISNULL(SR.V,0)-ISNULL(TT.V,0)+ISNULL(BA.Adj,0) >= 1
)
SELECT N AS [الموظف], CAST(Debt AS decimal(18,2)) AS [إجمالي_الدين], CONVERT(varchar(10),LD,120) AS [آخر_إيصال_قبض], 0 AS s FROM D
UNION ALL
SELECT N'═══ إجمالي ديون الموظفين ═══', CAST(SUM(Debt) AS decimal(18,2)), NULL, 1 FROM D
ORDER BY s, [إجمالي_الدين] DESC;
```

---

## PATTERN: كشف-حساب-عميل
TRIGGERS: كشف حساب, كشف حساب عميل, كشف حساب شركة, رصيد العميل, رصيد شركة, حساب العميل, حساب شركة, client balance, customer balance statement, CLIENTS_BALANCE
TABLES: CUSTOMERS, CLIENTS_BALANCE
NOTES:
  - ضع اسم العميل أو جزءًا منه في @SEARCH عبر %PARTY%.
  - ابحث أولًا عن كل الأسماء المشابهة في CUSTOMERS.
  - اختر أفضل تطابق من نتائج البحث بالترتيب، ثم استخدم رقمه في CLIENTS_BALANCE.
  - لا تحسب الرصيد يدويًا. استخدم الإجراء المخزن CLIENTS_BALANCE كما هو.
  - النتيجة النهائية تكون تقريرًا عربيًا مختصرًا: رقم العميل، اسم العميل، المدين، الدائن، الرصيد.
---

```sql
DECLARE @SEARCH NVARCHAR(100)
SET @SEARCH = N'%PARTY%'
DECLARE @CUST_ID INT
DECLARE @SELECTED_CUST_ID INT
DECLARE @SELECTED_CUST_NAME NVARCHAR(255)
DECLARE @BALANCE FLOAT
DECLARE @TOTAL_CREDIT FLOAT
DECLARE @TOTAL_DEBIT FLOAT
DECLARE @TOTAL_CREDIT_N FLOAT
DECLARE @TOTAL_DEBIT_N FLOAT
DECLARE @BALANCE_N FLOAT

DECLARE @MATCHES TABLE (
  CUST_ID INT,
  CUST_NAME NVARCHAR(255),
  MATCH_SCORE INT
)

INSERT INTO @MATCHES (CUST_ID, CUST_NAME, MATCH_SCORE)
SELECT TOP 20
  CUST_ID,
  CUST_NAME,
  %PARTY_SCORE% AS MATCH_SCORE
FROM dbo.CUSTOMERS
WHERE CUST_NAME LIKE N'%' + @SEARCH + N'%' OR %PARTY_CONDITION%
ORDER BY
  CASE WHEN CUST_NAME LIKE N'%' + @SEARCH + N'%' THEN 0 ELSE %PARTY_SCORE% END,
  CUST_NAME

SELECT TOP 1
  @SELECTED_CUST_ID = CUST_ID,
  @SELECTED_CUST_NAME = CUST_NAME
FROM @MATCHES
ORDER BY MATCH_SCORE, CUST_NAME

SET @CUST_ID = @SELECTED_CUST_ID

IF @CUST_ID IS NOT NULL
BEGIN
  EXEC [CLIENTS_BALANCE]
    @TRAN_BARNCH1 = 'A',
    @CUST_ID = @CUST_ID,
    @COMM_ID = 0,
    @DT_FROM = '2025-01-01',
    @DT_TO = '2026-12-31',
    @ALL = 1,
    @TRAN_ID = 0,
    @CAT8_ID = 0,
    @BALANCE = @BALANCE OUTPUT,
    @TOTAL_CREDIT = @TOTAL_CREDIT OUTPUT,
    @TOTAL_DEBIT = @TOTAL_DEBIT OUTPUT,
    @TOTAL_CREDIT_N = @TOTAL_CREDIT_N OUTPUT,
    @TOTAL_DEBIT_N = @TOTAL_DEBIT_N OUTPUT,
    @BALANCE_N = @BALANCE_N OUTPUT

  SELECT
    @SELECTED_CUST_ID AS [رقم العميل],
    @SELECTED_CUST_NAME AS [اسم العميل],
    CAST(@TOTAL_DEBIT AS DECIMAL(10,2)) AS [المدين],
    CAST(@TOTAL_CREDIT AS DECIMAL(10,2)) AS [الدائن],
    CAST(@BALANCE AS DECIMAL(10,2)) AS [الرصيد]
END
ELSE
BEGIN
  SELECT
    CAST(NULL AS INT) AS [رقم العميل],
    CAST(NULL AS NVARCHAR(255)) AS [اسم العميل],
    CAST(NULL AS DECIMAL(10,2)) AS [المدين],
    CAST(NULL AS DECIMAL(10,2)) AS [الدائن],
    CAST(NULL AS DECIMAL(10,2)) AS [الرصيد]
  WHERE 1=0
END
```

---

## PATTERN: كشف-حساب-عميل-مفصل
TRIGGERS: كشف حساب مفصل, كشف حساب العميل مفصل, كشف حساب كامل, كشف حساب العميل كامل, كشف حساب تفصيلي, تفاصيل حساب العميل, فواتير العميل وبنوده, customer detailed balance, detailed customer statement
TABLES: CUSTOMERS, CLIENTS_BALANCE, SALE_INVOICE, SALE_ITEMS
NOTES:
  - هذا هو كشف الحساب المفصل بجانب النمط المختصر.
  - ضع اسم العميل أو جزءًا منه في @SEARCH عبر %PARTY%.
  - الجزء الأول يعطي ملخص المدين/الدائن/الرصيد من CLIENTS_BALANCE.
  - الجزء الثاني يعرض فواتير بيع العميل.
  - الجزء الثالث يعرض بنود فواتير العميل.
  - استخدمه فقط عندما يطلب المستخدم: مفصل، كامل، تفصيلي، الفواتير، البنود.
---

```sql
DECLARE @SEARCH NVARCHAR(100)
SET @SEARCH = N'%PARTY%'
DECLARE @CUST_ID INT
DECLARE @SELECTED_CUST_NAME NVARCHAR(255)
DECLARE @BALANCE FLOAT
DECLARE @TOTAL_CREDIT FLOAT
DECLARE @TOTAL_DEBIT FLOAT
DECLARE @TOTAL_CREDIT_N FLOAT
DECLARE @TOTAL_DEBIT_N FLOAT
DECLARE @BALANCE_N FLOAT

SELECT TOP 1
  @CUST_ID = CUST_ID,
  @SELECTED_CUST_NAME = CUST_NAME
FROM dbo.CUSTOMERS
WHERE CUST_NAME LIKE N'%' + @SEARCH + N'%' OR %PARTY_CONDITION%
ORDER BY
  CASE WHEN CUST_NAME LIKE N'%' + @SEARCH + N'%' THEN 0 ELSE %PARTY_SCORE% END,
  CUST_NAME

IF @CUST_ID IS NOT NULL
BEGIN
  EXEC [CLIENTS_BALANCE]
    @TRAN_BARNCH1 = 'A',
    @CUST_ID = @CUST_ID,
    @COMM_ID = 0,
    @DT_FROM = '2025-01-01',
    @DT_TO = '2026-12-31',
    @ALL = 1,
    @TRAN_ID = 0,
    @CAT8_ID = 0,
    @BALANCE = @BALANCE OUTPUT,
    @TOTAL_CREDIT = @TOTAL_CREDIT OUTPUT,
    @TOTAL_DEBIT = @TOTAL_DEBIT OUTPUT,
    @TOTAL_CREDIT_N = @TOTAL_CREDIT_N OUTPUT,
    @TOTAL_DEBIT_N = @TOTAL_DEBIT_N OUTPUT,
    @BALANCE_N = @BALANCE_N OUTPUT

  SELECT
    @CUST_ID AS [رقم العميل],
    @SELECTED_CUST_NAME AS [اسم العميل],
    CAST(@TOTAL_DEBIT AS DECIMAL(10,2)) AS [المدين],
    CAST(@TOTAL_CREDIT AS DECIMAL(10,2)) AS [الدائن],
    CAST(@BALANCE AS DECIMAL(10,2)) AS [الرصيد]
END
ELSE
BEGIN
  SELECT
    CAST(NULL AS INT) AS [رقم العميل],
    CAST(NULL AS NVARCHAR(255)) AS [اسم العميل],
    CAST(NULL AS DECIMAL(10,2)) AS [المدين],
    CAST(NULL AS DECIMAL(10,2)) AS [الدائن],
    CAST(NULL AS DECIMAL(10,2)) AS [الرصيد]
  WHERE 1=0
END
```

```sql
DECLARE @SEARCH NVARCHAR(100)
SET @SEARCH = N'%PARTY%'
;WITH PickedCustomer AS (
  SELECT TOP 1 CUST_ID, CUST_NAME
  FROM dbo.CUSTOMERS
  WHERE CUST_NAME LIKE N'%' + @SEARCH + N'%' OR %PARTY_CONDITION%
  ORDER BY
    CASE WHEN CUST_NAME LIKE N'%' + @SEARCH + N'%' THEN 0 ELSE %PARTY_SCORE% END,
    CUST_NAME
)
SELECT
  P.CUST_ID AS [رقم العميل],
  P.CUST_NAME AS [اسم العميل],
  S.S_ID AS [الفاتورة],
  CONVERT(VARCHAR(10), S.S_DATE, 121) AS [التاريخ],
  COUNT(DISTINCT SI.ITEM_ID) AS [البنود],
  CAST(SUM(SI.QTY * SI.PRICE) AS DECIMAL(10,2)) AS [الإجمالي],
  CAST(ISNULL(S.S_DISCOUNT, 0) AS DECIMAL(10,2)) AS [الخصم],
  CAST(SUM(SI.QTY * SI.PRICE) - ISNULL(S.S_DISCOUNT, 0) AS DECIMAL(10,2)) AS [الصافي]
FROM PickedCustomer P
INNER JOIN dbo.SALE_INVOICE S ON S.CUST_ID = P.CUST_ID
LEFT JOIN dbo.SALE_ITEMS SI ON S.S_ID = SI.S_ID
GROUP BY P.CUST_ID, P.CUST_NAME, S.S_ID, S.S_DATE, S.S_DISCOUNT
ORDER BY S.S_DATE DESC, S.S_ID DESC
```

```sql
DECLARE @SEARCH NVARCHAR(100)
SET @SEARCH = N'%PARTY%'
;WITH PickedCustomer AS (
  SELECT TOP 1 CUST_ID, CUST_NAME
  FROM dbo.CUSTOMERS
  WHERE CUST_NAME LIKE N'%' + @SEARCH + N'%' OR %PARTY_CONDITION%
  ORDER BY
    CASE WHEN CUST_NAME LIKE N'%' + @SEARCH + N'%' THEN 0 ELSE %PARTY_SCORE% END,
    CUST_NAME
)
SELECT
  P.CUST_ID AS [رقم العميل],
  P.CUST_NAME AS [اسم العميل],
  S.S_ID AS [الفاتورة],
  CONVERT(VARCHAR(10), S.S_DATE, 121) AS [التاريخ],
  SI.ITEM_ID AS [المنتج],
  SI.QTY AS [الكمية],
  SI.PRICE AS [السعر],
  CAST(SI.QTY * SI.PRICE AS DECIMAL(10,2)) AS [القيمة]
FROM PickedCustomer P
INNER JOIN dbo.SALE_INVOICE S ON S.CUST_ID = P.CUST_ID
LEFT JOIN dbo.SALE_ITEMS SI ON S.S_ID = SI.S_ID
ORDER BY S.S_DATE DESC, S.S_ID DESC, SI.ITEM_ID
```

---

## PATTERN: ديون-الزبائن
TRIGGERS: ديون الزبائن, ديون الزباين, ديون العملاء, اللي لي, من يدينني, customer debts, ذمة الزبائن, ديون لي
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, TAKE, BALANCE_EDIT
NOTES:
  - الدين = مبيعات − S_DISCOUNT − مردودات − مقبوضات + تسوية. SQL 2005 ✓
  - إذا ذكر المستخدم اسم عميل مع كلمة ديون، استخدم %PARTY_PICK_CONDITION% لاختيار أفضل عميل واحد.
  - إذا لم يذكر اسمًا محددًا، %PARTY_PICK_CONDITION% تتحول إلى 1=1 ويظهر تقرير كل الزبائن.
---

```sql
;WITH
BA AS (SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS Adj FROM dbo.BALANCE_EDIT GROUP BY CUST_ID),
ST AS (SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) AS GV, SUM(ISNULL(SI.S_DISCOUNT,0)) AS TD
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID),
SR AS (SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) AS V FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID),
TT AS (SELECT CUST_ID, SUM(T_VALUE) AS V FROM dbo.TAKE GROUP BY CUST_ID),
LT AS (SELECT CUST_ID, T_DATE, ROW_NUMBER() OVER (PARTITION BY CUST_ID ORDER BY T_ID DESC) AS rn FROM dbo.TAKE),
D AS (
  SELECT C.CUST_NAME AS N, ISNULL(ST.GV,0)-ISNULL(ST.TD,0)-ISNULL(SR.V,0)-ISNULL(TT.V,0)+ISNULL(BA.Adj,0) AS Debt, LT.T_DATE AS LD
  FROM dbo.CUSTOMERS C LEFT JOIN ST ON C.CUST_ID=ST.CUST_ID LEFT JOIN SR ON C.CUST_ID=SR.CUST_ID
  LEFT JOIN TT ON C.CUST_ID=TT.CUST_ID LEFT JOIN BA ON C.CUST_ID=BA.CUST_ID
  LEFT JOIN LT ON C.CUST_ID=LT.CUST_ID AND LT.rn=1
  WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0
    AND (%PARTY_PICK_CONDITION%)
    AND ISNULL(ST.GV,0)-ISNULL(ST.TD,0)-ISNULL(SR.V,0)-ISNULL(TT.V,0)+ISNULL(BA.Adj,0) >= 1
),
R AS (
  SELECT N AS [الزبون], CAST(Debt AS decimal(18,2)) AS [إجمالي_الدين], CONVERT(varchar(10),LD,120) AS [آخر_إيصال_قبض], 0 AS s FROM D
  UNION ALL
  SELECT N'═══ إجمالي ديون الزبائن ═══', CAST(SUM(Debt) AS decimal(18,2)), NULL, 1 FROM D
)
SELECT [الزبون], [إجمالي_الدين], [آخر_إيصال_قبض]
FROM R
ORDER BY s, [إجمالي_الدين] DESC;
```

---

## PATTERN: ديون-الموردين
TRIGGERS: ديون الموردين, ديون موردين, اللي علي, من أدين له, supplier debts, ذمة الموردين, ديون علي
TABLES: CUSTOMERS, BUY_INVOICE, BUY_ITEMS, B_R_INVOICE, B_R_ITEMS, GIVE, BALANCE_EDIT
NOTES:
  - الدين = مشتريات − B_DISCOUNT − مردودات − مدفوعات (GIVE EXPENCES_ID=0) + تسوية. SQL 2005 ✓
---

```sql
SELECT
    c.CUST_NAME AS [المورد],

    FORMAT(ISNULL(p.TotalPurchases,0),'N2') AS [إجمالي المشتريات],

    FORMAT(ISNULL(g.TotalPaid,0),'N2') AS [إجمالي المدفوع],

    FORMAT(
        ISNULL(p.TotalPurchases,0) - ISNULL(g.TotalPaid,0),
        'N2'
    ) AS [الرصيد المستحق]

FROM dbo.CUSTOMERS c
LEFT JOIN
(
    SELECT
        bi.CUST_ID,
        SUM(ISNULL(bd.QTY,0) * ISNULL(bd.PRICE,0)) AS TotalPurchases
    FROM dbo.BUY_INVOICE bi
    INNER JOIN dbo.BUY_ITEMS bd
        ON bi.B_ID = bd.B_ID
    GROUP BY bi.CUST_ID
) p
ON c.CUST_ID = p.CUST_ID

LEFT JOIN
(
    SELECT
        CUST_ID,
        SUM(ISNULL(G_VALUE,0)) AS TotalPaid
    FROM dbo.GIVE
    WHERE G_STATUES = 1
    GROUP BY CUST_ID
) g
ON c.CUST_ID = g.CUST_ID

WHERE c.CUST_VENDOR = 1
AND (ISNULL(p.TotalPurchases,0) - ISNULL(g.TotalPaid,0)) > 0

ORDER BY
    ISNULL(p.TotalPurchases,0) - ISNULL(g.TotalPaid,0) DESC;
```

---


