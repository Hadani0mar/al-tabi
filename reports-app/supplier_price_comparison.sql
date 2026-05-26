/* مقارنة أسعار الموردين لمنتج معين
   sqlcmd -E -S localhost -d Marketing2026 -i supplier_price_comparison.sql
   PowerShell (استبدال المنتج):
     (Get-Content supplier_price_comparison.sql -Raw).Replace("N'%PRODUCT%'", "N'%PARACETAMOL%'") | Set-Content $env:TEMP\spc.sql -Encoding utf8NoBOM
     sqlcmd -E -S localhost -d Marketing2026 -W -i $env:TEMP\spc.sql
   - product_filter / N'%PRODUCT%' = جزء من الاسم أو الكود
   - لكل مورد: آخر سعر، أقل/أعلى/متوسط، عدد مرات الشراء، ترتيب السعر
*/
DECLARE @MonthsBack int = 36;
DECLARE @RecentFrom date = DATEADD(month, -@MonthsBack, CAST(GETDATE() AS date));

;WITH Matches AS (
    SELECT
        I.ITEM_ID,
        I.ITEM_MODEL,
        I.ITEM_NAME,
        MAX(B.B_DATE) AS LastAnyBuy,
        COUNT(BI.B_ITEM_ID) AS BuyLineCount
    FROM dbo.ITEMS I
    LEFT JOIN dbo.BUY_ITEMS BI ON I.ITEM_ID = BI.ITEM_ID AND BI.PRICE > 0
    LEFT JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    WHERE I.ITEM_INVISIBLE = 0
      AND (
            I.ITEM_MODEL LIKE N'%PRODUCT%'
            OR I.ITEM_NAME LIKE N'%PRODUCT%'
      )
    GROUP BY I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME
),
ProductPick AS (
    SELECT TOP 1
        ITEM_ID,
        ITEM_MODEL,
        ITEM_NAME
    FROM Matches
    ORDER BY
        CASE WHEN BuyLineCount > 0 THEN 0 ELSE 1 END,
        BuyLineCount DESC,
        LastAnyBuy DESC,
        CASE WHEN ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END,
        ITEM_ID DESC
),
Purchases AS (
    SELECT
        PP.ITEM_ID,
        PP.ITEM_MODEL,
        PP.ITEM_NAME,
        B.CUST_ID,
        CU.CUST_NAME AS Supplier,
        BI.PRICE,
        B.B_DATE,
        BI.QTY,
        BI.B_ITEM_ID,
        ROW_NUMBER() OVER (
            PARTITION BY PP.ITEM_ID, B.CUST_ID
            ORDER BY B.B_DATE DESC, BI.B_ITEM_ID DESC
        ) AS rn_last
    FROM ProductPick PP
    INNER JOIN dbo.BUY_ITEMS BI ON PP.ITEM_ID = BI.ITEM_ID
    INNER JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
    WHERE BI.PRICE > 0
      AND CAST(B.B_DATE AS date) >= @RecentFrom
),
BySupplier AS (
    SELECT
        ITEM_ID,
        ITEM_MODEL,
        ITEM_NAME,
        CUST_ID,
        Supplier,
        COUNT(*) AS PurchaseCount,
        CAST(MIN(PRICE) AS decimal(18, 2)) AS MinPrice,
        CAST(MAX(PRICE) AS decimal(18, 2)) AS MaxPrice,
        CAST(AVG(PRICE) AS decimal(18, 2)) AS AvgPrice,
        MAX(CASE WHEN rn_last = 1 THEN PRICE END) AS LastPrice,
        MAX(CASE WHEN rn_last = 1 THEN B_DATE END) AS LastBuyDate,
        MAX(CASE WHEN rn_last = 1 THEN QTY END) AS LastQty
    FROM Purchases
    GROUP BY ITEM_ID, ITEM_MODEL, ITEM_NAME, CUST_ID, Supplier
)
SELECT
    LEFT(ITEM_NAME, 70) AS [اسم المنتج],
    ITEM_MODEL AS [الكود],
    ISNULL(Supplier, N'—') AS [المورد],
    CAST(LastPrice AS decimal(18, 2)) AS [آخر سعر شراء],
    CAST(LastBuyDate AS date) AS [آخر تاريخ شراء],
    CAST(LastQty AS decimal(18, 2)) AS [آخر كمية],
    MinPrice AS [أقل سعر],
    MaxPrice AS [أعلى سعر],
    AvgPrice AS [متوسط السعر],
    PurchaseCount AS [عدد مرات الشراء],
    DATEDIFF(day, CAST(LastBuyDate AS date), CAST(GETDATE() AS date)) AS [أيام منذ آخر شراء],
    ROW_NUMBER() OVER (ORDER BY LastPrice ASC, Supplier) AS [ترتيب السعر]
FROM BySupplier
ORDER BY LastPrice ASC, Supplier;
