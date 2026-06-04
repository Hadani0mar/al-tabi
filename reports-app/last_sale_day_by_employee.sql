-- ============================================================
-- تقرير آخر يوم مبيعات - كل فاتورة على سطر
-- LAST SALES DAY REPORT - Each invoice on separate row
-- ============================================================

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
