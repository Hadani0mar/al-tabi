/* مصاريف شهرية — Marketing2026 (مُختبَر sqlcmd -E -S localhost -d Marketing2026)
   - إيصالات رواتب = dbo.GIVE WHERE EXPENCES_ID = 1 (مصاريف رواتب)
   - مصاريف تشغيلية/خاصة = dbo.GIVE WHERE EXPENCES_ID > 0 AND EXPENCES_ID <> 1
   - ⚠️ EXPENCES_ID = 0 → دفعات موردين/فواتير شراء — ليست مصاريف تشغيلية
   - ⚠️ dbo.SALARIES فارغ — لا تستخدمه للمدفوعات الفعلية
*/
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());

-- [1] إيصالات رواتب مُسدَّدة في الشهر
SELECT
    CAST(G.G_DATE AS date) AS [تاريخ الصرف],
    ISNULL(G.G_NO, CAST(G.G_ID AS varchar(20))) AS [رقم الإيصال],
    ISNULL(NULLIF(LTRIM(RTRIM(C.CUST_NAME)), N''), G.G_DISC) AS [المستفيد],
    G.G_DISC AS [البيان],
    CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ]
FROM dbo.GIVE G
LEFT JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID = 1
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
ORDER BY G.G_DATE DESC;

-- [2] مصاريف تشغيلية/خاصة مُسجَّلة في الشهر (غير رواتب)
SELECT
    CAST(G.G_DATE AS date) AS [التاريخ],
    ISNULL(G.G_NO, CAST(G.G_ID AS varchar(20))) AS [رقم الإيصال],
    ISNULL(E.EXPENSE_DISC, N'غير مصنف') AS [نوع المصروف],
    G.G_DISC AS [البيان],
    CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND G.EXPENCES_ID <> 1
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
ORDER BY G.G_DATE DESC;

-- [3] ملخص الشهر حسب نوع المصروف
SELECT
    ISNULL(E.EXPENSE_DISC, N'غير مصنف') AS [نوع المصروف],
    COUNT(*) AS [عدد العمليات],
    CAST(SUM(G.G_VALUE) AS decimal(18,2)) AS [الإجمالي]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
GROUP BY E.EXPENCES_ID, E.EXPENSE_DISC
ORDER BY [الإجمالي] DESC;
