/* Ù…Ù‚Ø§Ø±Ù†Ø© Ø£Ø³Ø¹Ø§Ø± Ø§Ù„Ù…ÙˆØ±Ø¯ÙŠÙ† Ù„Ù…Ù†ØªØ¬ Ù…Ø¹ÙŠÙ†
   sqlcmd -E -S localhost -d Marketing2026 -v ProductFilter="TRAMADOL NORMON" -i supplier_price_comparison.sql
   - product_filter / TRAMADOL NORMON = Ø¬Ø²Ø¡ Ù…Ù† Ø§Ù„Ø§Ø³Ù… Ø£Ùˆ Ø§Ù„ÙƒÙˆØ¯
   - Ù„ÙƒÙ„ Ù…ÙˆØ±Ø¯: Ø¢Ø®Ø± Ø³Ø¹Ø±ØŒ Ø£Ù‚Ù„/Ø£Ø¹Ù„Ù‰/Ù…ØªÙˆØ³Ø·ØŒ Ø¹Ø¯Ø¯ Ù…Ø±Ø§Øª Ø§Ù„Ø´Ø±Ø§Ø¡ØŒ ØªØ±ØªÙŠØ¨ Ø§Ù„Ø³Ø¹Ø±
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
            I.ITEM_MODEL LIKE N'TRAMADOL NORMON'
            OR I.ITEM_NAME LIKE N'TRAMADOL NORMON'
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
        CASE WHEN ITEM_MODEL LIKE N'TRAMADOL NORMON' THEN 0 ELSE 1 END,
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
    LEFT(ITEM_NAME, 70) AS [Ø§Ø³Ù… Ø§Ù„Ù…Ù†ØªØ¬],
    ITEM_MODEL AS [Ø§Ù„ÙƒÙˆØ¯],
    ISNULL(Supplier, N'â€”') AS [Ø§Ù„Ù…ÙˆØ±Ø¯],
    CAST(LastPrice AS decimal(18, 2)) AS [Ø¢Ø®Ø± Ø³Ø¹Ø± Ø´Ø±Ø§Ø¡],
    CAST(LastBuyDate AS date) AS [Ø¢Ø®Ø± ØªØ§Ø±ÙŠØ® Ø´Ø±Ø§Ø¡],
    CAST(LastQty AS decimal(18, 2)) AS [Ø¢Ø®Ø± ÙƒÙ…ÙŠØ©],
    MinPrice AS [Ø£Ù‚Ù„ Ø³Ø¹Ø±],
    MaxPrice AS [Ø£Ø¹Ù„Ù‰ Ø³Ø¹Ø±],
    AvgPrice AS [Ù…ØªÙˆØ³Ø· Ø§Ù„Ø³Ø¹Ø±],
    PurchaseCount AS [Ø¹Ø¯Ø¯ Ù…Ø±Ø§Øª Ø§Ù„Ø´Ø±Ø§Ø¡],
    DATEDIFF(day, CAST(LastBuyDate AS date), CAST(GETDATE() AS date)) AS [Ø£ÙŠØ§Ù… Ù…Ù†Ø° Ø¢Ø®Ø± Ø´Ø±Ø§Ø¡],
    ROW_NUMBER() OVER (ORDER BY LastPrice ASC, Supplier) AS [ØªØ±ØªÙŠØ¨ Ø§Ù„Ø³Ø¹Ø±]
FROM BySupplier
ORDER BY LastPrice ASC, Supplier;

