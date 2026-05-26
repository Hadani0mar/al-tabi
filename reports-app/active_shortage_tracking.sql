/* نواقص نشطة — منتجات ناقصة في المخزون وتُباع فعلاً
   sqlcmd -E -S localhost -d Marketing2026 -i active_shortage_tracking.sql
   - الرصيد من ITEMS_SUB
   - «نشطة» = مبيعات صافية > 0 في آخر @DaysRecent يوم (من MAX(S_DATE))
   - آخر سعر شراء + المورد من آخر BUY_ITEMS
*/
DECLARE @DaysRecent int = 60;
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @RecentFrom date = DATEADD(day, -@DaysRecent, @AsOfDate);

;WITH Stock AS (
    SELECT ITEM_ID, SUM(ISNULL(QTY, 0)) AS StockQty
    FROM dbo.ITEMS_SUB
    GROUP BY ITEM_ID
),
SalesRecent AS (
    SELECT X.ITEM_ID, SUM(X.QTY) AS SoldQty, MAX(X.S_DATE) AS LastSaleDate
    FROM (
        SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
        FROM dbo.SALE_ITEMS SI
        INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
        WHERE CAST(INV.S_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
        UNION ALL
        SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
        FROM dbo.R_S_ITEMS RSI
        INNER JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
        WHERE CAST(RINV.S_R_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
    ) X
    GROUP BY X.ITEM_ID
),
LastBuy AS (
    SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice, B.B_DATE AS LastBuyDate, CU.CUST_NAME AS LastSupplier
    FROM dbo.BUY_ITEMS BI
    INNER JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
    WHERE BI.B_ITEM_ID IN (
        SELECT MAX(BI2.B_ITEM_ID)
        FROM dbo.BUY_ITEMS BI2
        INNER JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID = B2.B_ID
        GROUP BY BI2.ITEM_ID
    )
)
SELECT TOP 150
    LEFT(I.ITEM_NAME, 80) AS [اسم المنتج],
    CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [الكمية],
    CAST(COALESCE(LB.LastBuyPrice, I.LAST_COST, 0) AS decimal(18,2)) AS [آخر سعر شراء],
    ISNULL(LB.LastSupplier, N'—') AS [المورد],
    CAST(ISNULL(SR.SoldQty, 0) AS decimal(18,2)) AS [مبيعات النافذة],
    CAST(I.MIN_LEVEL AS decimal(18,2)) AS [الحد الأدنى],
    CASE
        WHEN ISNULL(S.StockQty, 0) <= 0 THEN N'نفاد'
        WHEN I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) <= I.MIN_LEVEL THEN N'تحت الحد الأدنى'
        ELSE N'قريب من النفاد'
    END AS [حالة النقص]
FROM dbo.ITEMS I
INNER JOIN SalesRecent SR ON I.ITEM_ID = SR.ITEM_ID AND SR.SoldQty > 0
LEFT JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID = LB.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (
        ISNULL(S.StockQty, 0) <= 0
        OR (I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) <= I.MIN_LEVEL)
        OR (I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) < I.MIN_LEVEL * 1.25)
  )
ORDER BY
    CASE WHEN ISNULL(S.StockQty, 0) <= 0 THEN 0 ELSE 1 END,
    SR.SoldQty DESC,
    ISNULL(S.StockQty, 0) ASC;
