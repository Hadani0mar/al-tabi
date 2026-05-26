/* طلبية شراء ذكية — Marketing2026
   غيّر المتغيرات أدناه حسب النافذة الزمنية المطلوبة */
DECLARE @DaysRecent    int = 60;   -- نافذة حساب سرعة البيع (أيام)
DECLARE @DaysTotal     int = 180;  -- نافذة أوسع للمقارنة
DECLARE @CoverageDays  int = 30;   -- أيام التغطية المستهدفة بعد الشراء
DECLARE @MinDailySales float = 0.01; -- تجاهل أصناف بمبيعات شبه معدومة

DECLARE @AsOfDate date = (
    SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE
);
DECLARE @RecentFrom date = DATEADD(day, -@DaysRecent, @AsOfDate);
DECLARE @TotalFrom    date = DATEADD(day, -@DaysTotal, @AsOfDate);

;WITH Stock AS (
    SELECT
        ITEM_ID,
        SUM(ISNULL(QTY, 0)) AS StockQty
    FROM dbo.ITEMS_SUB
    GROUP BY ITEM_ID
),
SalesRecent AS (
    SELECT
        SI.ITEM_ID,
        SUM(ISNULL(SI.QTY, 0)) AS SoldQty,
        COUNT(DISTINCT CAST(SI.S_DATE AS date)) AS ActiveSaleDays,
        MAX(SI.S_DATE) AS LastSaleDate
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
    ) SI
    GROUP BY SI.ITEM_ID
),
SalesTotal AS (
    SELECT
        SI.ITEM_ID,
        SUM(ISNULL(SI.QTY, 0)) AS SoldQtyTotal
    FROM (
        SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
        FROM dbo.SALE_ITEMS SI
        INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
        WHERE CAST(INV.S_DATE AS date) BETWEEN @TotalFrom AND @AsOfDate

        UNION ALL

        SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
        FROM dbo.R_S_ITEMS RSI
        INNER JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
        WHERE CAST(RINV.S_R_DATE AS date) BETWEEN @TotalFrom AND @AsOfDate
    ) SI
    GROUP BY SI.ITEM_ID
),
LastBuy AS (
    SELECT
        BI.ITEM_ID,
        BI.PRICE AS LastBuyPrice,
        B.B_DATE AS LastBuyDate,
        CU.CUST_NAME AS LastSupplier
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
SELECT TOP 50
    I.ITEM_MODEL AS [الكود],
    LEFT(I.ITEM_NAME, 60) AS [اسم المنتج],
    CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [رصيد المخزون],
    CAST(ISNULL(SR.SoldQty, 0) AS decimal(18,2)) AS [مبيعات آخر نافذة],
    ISNULL(SR.ActiveSaleDays, 0) AS [أيام بيع فعلية],
    CAST(
        CASE
            WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
            THEN ISNULL(SR.SoldQty, 0) / SR.ActiveSaleDays
            ELSE ISNULL(SR.SoldQty, 0) / NULLIF(@DaysRecent, 0)
        END AS decimal(18,3)
    ) AS [معدل يومي],
    CAST(
        CASE
            WHEN ISNULL(SR.SoldQty, 0) <= 0 THEN NULL
            WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                 AND (ISNULL(SR.SoldQty, 0) / SR.ActiveSaleDays) >= @MinDailySales
            THEN ISNULL(S.StockQty, 0) / (SR.SoldQty / SR.ActiveSaleDays)
            WHEN (ISNULL(SR.SoldQty, 0) / NULLIF(@DaysRecent, 0)) >= @MinDailySales
            THEN ISNULL(S.StockQty, 0) / (SR.SoldQty / @DaysRecent)
            ELSE NULL
        END AS decimal(18,1)
    ) AS [أيام تغطية الرصيد],
    CONVERT(varchar(10), SR.LastSaleDate, 103) AS [آخر بيع],
    CAST(I.MIN_LEVEL AS decimal(18,2)) AS [حد أدنى],
    CAST(I.MAX_LEVEL AS decimal(18,2)) AS [حد أعلى],
    CAST(
        CASE
            WHEN ISNULL(SR.SoldQty, 0) <= 0 THEN 0
            ELSE CASE
                WHEN (
                    CASE
                        WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                        THEN (SR.SoldQty / SR.ActiveSaleDays) * @CoverageDays
                        ELSE (SR.SoldQty / @DaysRecent) * @CoverageDays
                    END - ISNULL(S.StockQty, 0)
                ) < 0 THEN 0
                ELSE (
                    CASE
                        WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                        THEN (SR.SoldQty / SR.ActiveSaleDays) * @CoverageDays
                        ELSE (SR.SoldQty / @DaysRecent) * @CoverageDays
                    END - ISNULL(S.StockQty, 0)
                )
            END
        END AS decimal(18,2)
    ) AS [كمية الشراء المقترحة],
    CASE
        WHEN ISNULL(S.StockQty, 0) <= 0 AND ISNULL(SR.SoldQty, 0) > 0 THEN N'نفاد — شراء عاجل'
        WHEN ISNULL(SR.SoldQty, 0) <= 0 THEN N'بدون مبيعات في النافذة'
        WHEN (
            CASE
                WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                THEN ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / SR.ActiveSaleDays, 0)
                ELSE ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / @DaysRecent, 0)
            END
        ) < 7 THEN N'حرج'
        WHEN (
            CASE
                WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                THEN ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / SR.ActiveSaleDays, 0)
                ELSE ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / @DaysRecent, 0)
            END
        ) < @CoverageDays THEN N'يُنصح بالشراء'
        ELSE N'كافٍ حالياً'
    END AS [الأولوية],
    CAST(LB.LastBuyPrice AS decimal(18,2)) AS [آخر سعر شراء],
    CONVERT(varchar(10), LB.LastBuyDate, 103) AS [آخر شراء],
    LB.LastSupplier AS [آخر مورد]
FROM dbo.ITEMS I
LEFT JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
LEFT JOIN SalesRecent SR ON I.ITEM_ID = SR.ITEM_ID
LEFT JOIN SalesTotal ST ON I.ITEM_ID = ST.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID = LB.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND ISNULL(SR.SoldQty, 0) > 0
  AND (
        ISNULL(S.StockQty, 0) <= I.MIN_LEVEL
        OR ISNULL(S.StockQty, 0) = 0
        OR (
            CASE
                WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                THEN ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / SR.ActiveSaleDays, 0)
                ELSE ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / @DaysRecent, 0)
            END
        ) < @CoverageDays
  )
ORDER BY
    CASE
        WHEN ISNULL(S.StockQty, 0) <= 0 THEN 0
        WHEN (
            CASE
                WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                THEN ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / SR.ActiveSaleDays, 0)
                ELSE ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / @DaysRecent, 0)
            END
        ) IS NULL THEN 1
        ELSE (
            CASE
                WHEN ISNULL(SR.ActiveSaleDays, 0) > 0
                THEN ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / SR.ActiveSaleDays, 0)
                ELSE ISNULL(S.StockQty, 0) / NULLIF(SR.SoldQty / @DaysRecent, 0)
            END
        )
    END ASC,
    ISNULL(SR.SoldQty, 0) DESC;
