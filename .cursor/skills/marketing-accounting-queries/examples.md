# Query Pattern Examples

Replace parameters before running. All examples target `Marketing2026.dbo`.

## 1. Trial Balance from Views

```sql
-- Purpose: ميزان مراجعة من BALANCE_T_VIEW
-- Filters: none (add WHERE on AC_NO prefix if needed)

SELECT
    v.AC_NO           AS [رقم_الحساب],
    v.AC_NAME         AS [اسم_الحساب],
    v.PARENT          AS [الأب],
    SUM(v.ACC_DEBIT)  AS [مدين],
    SUM(v.ACC_CREDIT) AS [دائن],
    SUM(v.BALANCE)    AS [الرصيد]
FROM dbo.BALANCE_T_VIEW v
GROUP BY v.AC_NO, v.AC_NAME, v.PARENT, v.AC_P_ID
ORDER BY v.AC_NO;
```

## 2. Customer Statement (Sales + Receipts + Payments)

```sql
-- Purpose: كشف حساب عميل
-- Parameters: @CustId, @FromDate, @ToDate

DECLARE @CustId INT = 1;
DECLARE @FromDate DATE = '2026-01-01';
DECLARE @ToDate   DATE = '2026-12-31';

;WITH Movements AS (
    SELECT s.S_DATE AS TrnDate, N'فاتورة مبيعات' AS TrnType,
           s.S_ID AS DocNo, si.LineTotal AS Debit, 0.0 AS Credit, s.TRAN_BARNCH AS Branch
    FROM dbo.SALE_INVOICE_VIEW s
    CROSS APPLY (
        SELECT SUM(i.QTY * i.PRICE) AS LineTotal
        FROM dbo.SALE_ITEMS_INVOICE_VIEW i
        WHERE i.S_ID = s.S_ID
    ) si
    WHERE s.CUST_ID = @CustId
      AND s.S_DATE >= @FromDate AND s.S_DATE < DATEADD(DAY, 1, @ToDate)

    UNION ALL
    SELECT t.T_DATE, N'قبض', t.T_ID, 0.0, t.T_VALUE, t.TRAN_BARNCH
    FROM dbo.TAKE_VIEW t
    WHERE t.CUST_ID = @CustId
      AND t.T_DATE >= @FromDate AND t.T_DATE < DATEADD(DAY, 1, @ToDate)

    UNION ALL
    SELECT g.G_DATE, N'صرف', g.G_ID, g.G_VALUE, 0.0, g.TRAN_BARNCH
    FROM dbo.GIVE_VIEW g
    WHERE g.CUST_ID = @CustId
      AND g.G_DATE >= @FromDate AND g.G_DATE < DATEADD(DAY, 1, @ToDate)
)
SELECT
    TrnDate,
    TrnType,
    DocNo,
    Branch,
    Debit,
    Credit,
    SUM(Debit - Credit) OVER (ORDER BY TrnDate, DocNo ROWS UNBOUNDED PRECEDING) AS [الرصيد_التراكمي]
FROM Movements
ORDER BY TrnDate, DocNo;
```

## 3. Monthly Sales Using Staging Table

```sql
-- Purpose: مبيعات شهرية جاهزة من جدول ERP
-- Source: SEARCH_TRANS10 (12-month matrix)

SELECT
    CUST_NO,
    CUST_NAME,
    COMM_NAME,
    MONTHS_1, MONTHS_2, MONTHS_3, MONTHS_4,
    MONTHS_5, MONTHS_6, MONTHS_7, MONTHS_8,
    MONTHS_9, MONTHS_10, MONTHS_11, MONTHS_12,
    (ISNULL(MONTHS_1,0)+ISNULL(MONTHS_2,0)+ISNULL(MONTHS_3,0)+ISNULL(MONTHS_4,0)+
     ISNULL(MONTHS_5,0)+ISNULL(MONTHS_6,0)+ISNULL(MONTHS_7,0)+ISNULL(MONTHS_8,0)+
     ISNULL(MONTHS_9,0)+ISNULL(MONTHS_10,0)+ISNULL(MONTHS_11,0)+ISNULL(MONTHS_12,0)) AS [إجمالي_السنة]
FROM dbo.SEARCH_TRANS10
ORDER BY CUST_NAME;
```

## 4. Gross Profit by Customer (View-based)

```sql
-- Purpose: أرباح العملاء من SALE_ITEMS_INVOICE_VIEW
-- Parameters: @FromDate, @ToDate

DECLARE @FromDate DATE = '2026-01-01';
DECLARE @ToDate   DATE = '2026-12-31';

SELECT
    v.CUST_NO,
    v.CUST_NAME,
    SUM(v.QTY * v.PRICE)                          AS [إجمالي_المبيعات],
    SUM(v.QTY * ISNULL(v.AVER_COST, v.LAST_COST)) AS [إجمالي_التكلفة],
    SUM(v.QTY * v.PRICE)
      - SUM(v.QTY * ISNULL(v.AVER_COST, v.LAST_COST)) AS [مجمل_الربح]
FROM dbo.SALE_ITEMS_INVOICE_VIEW v
WHERE v.S_DATE >= @FromDate
  AND v.S_DATE < DATEADD(DAY, 1, @ToDate)
GROUP BY v.CUST_NO, v.CUST_NAME
ORDER BY [مجمل_الربح] DESC;
```

## 5. Account Tree Rollup (Recursive CTE)

```sql
-- Purpose: أرصدة مجمعة حسب شجرة الحسابات
-- Source: BALANCE_T_VIEW + ACCOUNT_VIEW

;WITH AcctTree AS (
    SELECT AC_ID, AC_P_ID, AC_NO, AC_NAME, AC_LEVEL
    FROM dbo.ACCOUNT_VIEW
    WHERE AC_P_ID IS NULL OR AC_LEVEL = 1

    UNION ALL
    SELECT c.AC_ID, c.AC_P_ID, c.AC_NO, c.AC_NAME, c.AC_LEVEL
    FROM dbo.ACCOUNT_VIEW c
    INNER JOIN AcctTree p ON c.AC_P_ID = p.AC_ID
)
SELECT
    t.AC_NO,
    t.AC_NAME,
    t.AC_LEVEL,
    SUM(b.BALANCE) AS [الرصيد_المجمع]
FROM AcctTree t
LEFT JOIN dbo.BALANCE_T_VIEW b ON b.ACC_ID = t.AC_ID
GROUP BY t.AC_NO, t.AC_NAME, t.AC_LEVEL
ORDER BY t.AC_NO;
```

## 6. Unified Cash/Bank Ledger

```sql
-- Purpose: دفتر نقدية/بنك موحد
-- Parameters: @FromDate, @ToDate, optional @BankId

DECLARE @FromDate DATE = '2026-01-01';
DECLARE @ToDate   DATE = '2026-12-31';
DECLARE @BankId   INT = NULL;

SELECT T_DATE AS TrnDate, N'قبض' AS TrnType, T_NO AS RefNo,
       BANK_NAME, T_VALUE AS Credit, 0.0 AS Debit, AC_NAME_DEBIT, AC_NAME_CREDIT
FROM dbo.TAKE_VIEW
WHERE T_DATE >= @FromDate AND T_DATE < DATEADD(DAY,1,@ToDate)
  AND (@BankId IS NULL OR BANK_ID = @BankId)

UNION ALL
SELECT G_DATE, N'صرف', G_NO, BANK_NAME, 0.0, G_VALUE, AC_NAME_DEBIT, AC_NAME_CREDIT
FROM dbo.GIVE_VIEW
WHERE G_DATE >= @FromDate AND G_DATE < DATEADD(DAY,1,@ToDate)
  AND (@BankId IS NULL OR BANK_ID = @BankId)

UNION ALL
SELECT B_S_DATE, N'تسوية بنك', CAST(B_S_ID AS VARCHAR(20)), BANK_NAME,
       B_S_CREDIT, B_S_DEBIT, AC_NAME_DEBIT, AC_NAME_CREDIT
FROM dbo.BANK_BALANCE_EDIT_VIEW
WHERE B_S_DATE >= @FromDate AND B_S_DATE < DATEADD(DAY,1,@ToDate)
  AND (@BankId IS NULL OR BANK_ID = @BankId)

ORDER BY TrnDate, TrnType;
```

## 7. Journal Entry Detail

```sql
-- Purpose: تفاصيل القيود اليومية
-- Parameters: @FromDate, @ToDate

DECLARE @FromDate DATE = '2026-01-01';
DECLARE @ToDate   DATE = '2026-12-31';

SELECT
    h.Q_DATE,
    h.TRAN_NO,
    h.TRAN_BARNCH,
    l.Q_NO,
    l.AC_NO,
    l.AC_NAME,
    l.ACC_DEBIT,
    l.ACC_CREDIT,
    l.Q_DESC
FROM dbo.QYODAT_ITEMS_INVOICE_VIEW l
INNER JOIN dbo.QYODAT_INVOICE_VIEW h ON h.Q_ID = l.Q_ID
WHERE h.Q_DATE >= @FromDate
  AND h.Q_DATE < DATEADD(DAY, 1, @ToDate)
ORDER BY h.Q_DATE, h.TRAN_NO, l.Q_NO;
```
