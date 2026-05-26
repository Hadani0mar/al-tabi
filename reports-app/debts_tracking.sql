/* متابعة الديون — لي (زبائن) وعلي (موردين) — Marketing2026 */
DECLARE @MinBalance float = 1;

;WITH BalanceAdj AS (
    SELECT
        CUST_ID,
        SUM(ISNULL(BL_DEBIT, 0)) - SUM(ISNULL(BL_CREDIT, 0)) AS AdjBalance
    FROM dbo.BALANCE_EDIT
    GROUP BY CUST_ID
),
SaleTot AS (
    SELECT
        SI.CUST_ID,
        SUM(ISNULL(SI2.QTY, 0) * ISNULL(SI2.PRICE, 0)) AS SalesValue,
        MAX(SI.S_DATE) AS LastSaleDate
    FROM dbo.SALE_INVOICE SI
    INNER JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID = SI2.S_ID
    GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
    SELECT
        R.CUST_ID,
        SUM(ISNULL(RI.QTY, 0) * ISNULL(RI.PRICE, 0)) AS ReturnValue
    FROM dbo.R_S_INVOICE R
    INNER JOIN dbo.R_S_ITEMS RI ON R.S_R_ID = RI.S_R_ID
    GROUP BY R.CUST_ID
),
TakeTot AS (
    SELECT
        CUST_ID,
        SUM(ISNULL(T_VALUE, 0)) AS PaidValue,
        MAX(T_DATE) AS LastTakeDate
    FROM dbo.TAKE
    GROUP BY CUST_ID
),
BuyTot AS (
    SELECT
        B.CUST_ID,
        SUM(ISNULL(BI.QTY, 0) * ISNULL(BI.PRICE, 0)) AS BuyValue,
        MAX(B.B_DATE) AS LastBuyDate
    FROM dbo.BUY_INVOICE B
    INNER JOIN dbo.BUY_ITEMS BI ON B.B_ID = BI.B_ID
    GROUP BY B.CUST_ID
),
BuyReturnTot AS (
    SELECT
        BR.CUST_ID,
        SUM(ISNULL(BRI.QTY, 0) * ISNULL(BRI.PRICE, 0)) AS ReturnValue
    FROM dbo.B_R_INVOICE BR
    INNER JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID = BRI.B_R_ID
    GROUP BY BR.CUST_ID
),
GiveTot AS (
    SELECT
        CUST_ID,
        SUM(ISNULL(G_VALUE, 0)) AS PaidValue,
        MAX(G_DATE) AS LastGiveDate
    FROM dbo.GIVE
    GROUP BY CUST_ID
),
Receivables AS (
    SELECT
        N'لي — زبون مدين' AS [نوع الدين],
        C.CUST_NO AS [الرقم],
        C.CUST_NAME AS [الاسم],
        CAST(
            ISNULL(ST.SalesValue, 0)
            - ISNULL(SRT.ReturnValue, 0)
            - ISNULL(TT.PaidValue, 0)
            + ISNULL(BA.AdjBalance, 0)
        AS decimal(18,2)) AS [الرصيد المتبقي],
        CAST(ISNULL(ST.SalesValue, 0) AS decimal(18,2)) AS [إجمالي المبيعات],
        CAST(ISNULL(TT.PaidValue, 0) AS decimal(18,2)) AS [إجمالي المقبوضات],
        CAST(C.CUST_MAX_DEBIT AS decimal(18,2)) AS [حد الدين],
        CONVERT(varchar(10), ST.LastSaleDate, 103) AS [آخر بيع],
        CONVERT(varchar(10), TT.LastTakeDate, 103) AS [آخر قبض]
    FROM dbo.CUSTOMERS C
    LEFT JOIN SaleTot ST ON C.CUST_ID = ST.CUST_ID
    LEFT JOIN SaleReturnTot SRT ON C.CUST_ID = SRT.CUST_ID
    LEFT JOIN TakeTot TT ON C.CUST_ID = TT.CUST_ID
    LEFT JOIN BalanceAdj BA ON C.CUST_ID = BA.CUST_ID
    WHERE C.CUST_CUSTOM = 1
      AND C.CUST_INVISIBLE = 0
      AND (
            ISNULL(ST.SalesValue, 0)
            - ISNULL(SRT.ReturnValue, 0)
            - ISNULL(TT.PaidValue, 0)
            + ISNULL(BA.AdjBalance, 0)
          ) >= @MinBalance
),
Payables AS (
    SELECT
        N'علي — مورد دائن' AS [نوع الدين],
        C.CUST_NO AS [الرقم],
        C.CUST_NAME AS [الاسم],
        CAST(
            ISNULL(BT.BuyValue, 0)
            - ISNULL(BRT.ReturnValue, 0)
            - ISNULL(GT.PaidValue, 0)
            + ISNULL(BA.AdjBalance, 0)
        AS decimal(18,2)) AS [الرصيد المتبقي],
        CAST(ISNULL(BT.BuyValue, 0) AS decimal(18,2)) AS [إجمالي المشتريات],
        CAST(ISNULL(GT.PaidValue, 0) AS decimal(18,2)) AS [إجمالي المدفوعات],
        CAST(C.CUST_MAX_DEBIT AS decimal(18,2)) AS [حد الدين],
        CONVERT(varchar(10), BT.LastBuyDate, 103) AS [آخر شراء],
        CONVERT(varchar(10), GT.LastGiveDate, 103) AS [آخر دفع]
    FROM dbo.CUSTOMERS C
    LEFT JOIN BuyTot BT ON C.CUST_ID = BT.CUST_ID
    LEFT JOIN BuyReturnTot BRT ON C.CUST_ID = BRT.CUST_ID
    LEFT JOIN GiveTot GT ON C.CUST_ID = GT.CUST_ID
    LEFT JOIN BalanceAdj BA ON C.CUST_ID = BA.CUST_ID
    WHERE C.CUST_VENDOR = 1
      AND C.CUST_INVISIBLE = 0
      AND (
            ISNULL(BT.BuyValue, 0)
            - ISNULL(BRT.ReturnValue, 0)
            - ISNULL(GT.PaidValue, 0)
            + ISNULL(BA.AdjBalance, 0)
          ) >= @MinBalance
)
SELECT TOP 50 *
FROM (
    SELECT * FROM Receivables
    UNION ALL
    SELECT
        [نوع الدين],
        [الرقم],
        [الاسم],
        [الرصيد المتبقي],
        [إجمالي المشتريات] AS [إجمالي الحركة],
        [إجمالي المدفوعات] AS [إجمالي التسويات],
        [حد الدين],
        [آخر شراء] AS [آخر حركة],
        [آخر دفع] AS [آخر تسوية]
    FROM Payables
) D
ORDER BY [نوع الدين], [الرصيد المتبقي] DESC;
