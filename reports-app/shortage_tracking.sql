/* متابعة النواقص — Marketing2026 */
DECLARE @DaysRecent int = 60;
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @RecentFrom date = DATEADD(day, -@DaysRecent, @AsOfDate);

;WITH Stock AS (
    SELECT ITEM_ID, SUM(ISNULL(QTY, 0)) AS StockQty
    FROM dbo.ITEMS_SUB
    GROUP BY ITEM_ID
),
SalesRecent AS (
    SELECT
        X.ITEM_ID,
        SUM(X.QTY) AS SoldQty,
        MAX(X.S_DATE) AS LastSaleDate
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
)
SELECT TOP 100
    I.ITEM_MODEL AS [الكود],
    LEFT(I.ITEM_NAME, 60) AS [اسم المنتج],
    CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [رصيد المخزون],
    CAST(I.MIN_LEVEL AS decimal(18,2)) AS [الحد الأدنى],
    CAST(I.MAX_LEVEL AS decimal(18,2)) AS [الحد الأعلى],
    CAST(
        CASE
            WHEN I.MIN_LEVEL > 0 THEN I.MIN_LEVEL - ISNULL(S.StockQty, 0)
            ELSE 0
        END AS decimal(18,2)
    ) AS [فجوة النقص],
    CAST(ISNULL(SR.SoldQty, 0) AS decimal(18,2)) AS [مبيعات النافذة],
    CONVERT(varchar(10), SR.LastSaleDate, 103) AS [آخر بيع],
    CASE
        WHEN ISNULL(S.StockQty, 0) <= 0 AND ISNULL(SR.SoldQty, 0) > 0 THEN N'نفاد'
        WHEN ISNULL(S.StockQty, 0) <= 0 THEN N'نفاد بدون مبيعات حديثة'
        WHEN I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) <= I.MIN_LEVEL THEN N'تحت الحد الأدنى'
        WHEN I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) < I.MIN_LEVEL * 1.25 THEN N'قريب من النفاد'
        ELSE N'مراقبة'
    END AS [حالة النقص],
    CASE
        WHEN ISNULL(S.StockQty, 0) <= 0 AND ISNULL(SR.SoldQty, 0) > 0 THEN 1
        WHEN ISNULL(S.StockQty, 0) <= 0 THEN 2
        WHEN I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) <= I.MIN_LEVEL THEN 3
        ELSE 4
    END AS SortKey
FROM dbo.ITEMS I
LEFT JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
LEFT JOIN SalesRecent SR ON I.ITEM_ID = SR.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (
        ISNULL(S.StockQty, 0) <= 0
        OR (I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) <= I.MIN_LEVEL)
        OR (I.MIN_LEVEL > 0 AND ISNULL(S.StockQty, 0) < I.MIN_LEVEL * 1.25 AND ISNULL(SR.SoldQty, 0) > 0)
  )
ORDER BY SortKey ASC, ISNULL(SR.SoldQty, 0) DESC, [فجوة النقص] DESC;
