/* مبيعات آخر يوم فيه مبيعات — لكل موظف + إجمالي — Marketing2026
   sqlcmd -E -S localhost -d Marketing2026 -i last_sale_day_by_employee.sql
   ⚠️ لا تستخدم GETDATE() ولا تاريخاً ثابتاً — @LastSaleDay = MAX(S_DATE)
*/
DECLARE @LastSaleDay date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);

;WITH EmpSales AS (
    SELECT
        ISNULL(U.FULL_NAME, N'غير محدد') AS [الموظف],
        COUNT(DISTINCT INV.S_ID) AS [عدد الفواتير],
        CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إيرادات],
        0 AS SortOrder
    FROM dbo.SALE_INVOICE INV
    INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
    LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
    WHERE CAST(INV.S_DATE AS date) = @LastSaleDay
    GROUP BY U.USERS_ID, U.FULL_NAME
),
Grand AS (
    SELECT
        N'═══ الإجمالي ═══' AS [الموظف],
        COUNT(DISTINCT INV.S_ID) AS [عدد الفواتير],
        CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إيرادات],
        1 AS SortOrder
    FROM dbo.SALE_INVOICE INV
    INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
    WHERE CAST(INV.S_DATE AS date) = @LastSaleDay
)
SELECT
    @LastSaleDay AS [تاريخ آخر مبيعات],
    [الموظف],
    [عدد الفواتير],
    [إيرادات]
FROM (
    SELECT [الموظف], [عدد الفواتير], [إيرادات], SortOrder FROM EmpSales
    UNION ALL
    SELECT [الموظف], [عدد الفواتير], [إيرادات], SortOrder FROM Grand
) X
ORDER BY SortOrder, [إيرادات] DESC;
