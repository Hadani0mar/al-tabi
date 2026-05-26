---
name: marketing-accounting-queries
description: Builds advanced T-SQL accounting and analytics queries for the Marketing2026 ERP database by reading Full_Marketing_Database_DDL.sql, leveraging dbo views and *_SEARCH_TRANS_TABLE report tables. Use when the user asks for accounting queries, financial reports, trial balance, customer/vendor balances, sales/profit analysis, bank/cash flows, journal entries (QYODAT), or any SQL against this marketing database schema.
---

# Marketing2026 — Smart Accounting Queries

## Quick Start

When the user requests an accounting or analytics query:

1. Read [schema-reference.md](schema-reference.md) for core tables, keys, and conventions.
2. Scan [views-index.md](views-index.md) and pick the richest **VIEW** first; fall back to base tables only when no view covers the need.
3. If the user wants a report the ERP already materializes, check `*_SEARCH_TRANS_TABLE` tables in the DDL before rebuilding logic from scratch.
4. Re-open `Full_Marketing_Database_DDL.sql` only to confirm columns, joins, or FK names not covered in the reference files.
5. Return production-ready **T-SQL** with Arabic-friendly aliases where helpful.

## Database Context

| Item | Value |
|------|-------|
| Engine | Microsoft SQL Server |
| Database | `Marketing2026` |
| Schema | `dbo` |
| Collation | `Arabic_CI_AS` |
| Amount type | `float` (match existing schema; do not cast to decimal unless user asks) |
| DDL source | `Full_Marketing_Database_DDL.sql` at project root |

**Important:** The DDL file repeats the same schema multiple times. Always use the **first** table/view definition block (tables ~lines 1–1700, views ~lines 1708–3731).

## Query Design Rules

### 1. Prefer Views Over Raw Joins

Views already embed customers, commissioners, accounts, banks, units, categories, and document metadata. Examples:

- Accounts: `ACCOUNT_VIEW`, `BALANCE_C_VIEW`, `BALANCE_T_VIEW`
- Cash/bank: `TAKE_VIEW`, `GIVE_VIEW`, `BANKS_VIEW`, `BANK_TRANSFER_VIEW`, `BANK_BALANCE_EDIT_VIEW`
- Sales/purchases: `SALE_INVOICE_VIEW`, `SALE_ITEMS_INVOICE_VIEW`, `BUY_INVOICE_VIEW`, `BUY_ITEMS_INVOICE_VIEW`
- Returns: `R_S_INVOICE_VIEW`, `B_R_INVOICE_VIEW`
- Journal: `QYODAT_VIEW`, `QYODAT_ITEMS_VIEW`, `QYODAT_ITEMS_INVOICE_VIEW`
- Customers: `CUSTOMERS_VIEW`
- Inventory cost: `ITEMS_VIEW`, `ITEM_SUB_VIEW`, `ITEMS_BARCODE_QTY_VIEW`

Build new logic **on top of views** when possible; this keeps queries shorter and aligned with the application.

### 2. Exploit Report Staging Tables

Tables ending in `_SEARCH_TRANS_TABLE` are pre-shaped for ERP search screens. Use them for:

- Customer aging / unpaid lists → `CUSTOMER_LIST_NOT_PAIED_SEARCH_TRANS_TABLE`, `SALES_AND_PAIED_SEARCH_TRANS_TABLE`
- Monthly customer balances → `SEARCH_TRANS10`
- Item movement & totals → `ITEMS_INVOICE_MOVMENT_SEARCH_TRANS_TABLE`, `ITEMS_TOTALS_SEARCH_TRANS_TABLE`, `ITEM_ALL_SEARCH_TRANS_TABLE`
- Cash documents → `TAKE_SEARCH_TRANS_TABLE`, `GIVE_SEARCH_TRANS_TABLE`
- Bank → `BANK_BALANCE_SEARCH_TRANS_TABLE`, `BANK_TRANSFER_SEARCH_TRANS_TABLE`
- Profit by customer → `CUSTOMERS_PROFET_SEARCH_TRANS_TABLE`

Prefer these when the user's question matches an existing report pattern.

### 3. Accounting Conventions

**Chart of accounts (`ACCOUNTS`):**
- Hierarchical: `AC_P_ID` → parent, `AC_LEVEL`, `PARENT`, `AC_NO`, `AC_NAME`
- `AC_DEBIT_CREDIT_TYPE`, `AC_TYPE`, `ACCOUNT_COLLECT` control aggregation behavior
- Default posting accounts live in `ACCOUNTS_DEFULTS` (sale, buy, take, give, store, tax, discount, etc.)

**Document identity:**
- Most financial documents use `(TRAN_NO, TRAN_BARNCH)` as a unique business key
- Status fields: `*_STATUES` joined to `TYPE.T_NAME` in views

**Debit/Credit on documents:**
- Invoices and vouchers store `ACC_DEBIT`, `ACC_CREDIT` (and sometimes tax/discount account columns)
- `TAKE` = receipt (قبض), `GIVE` = payment (صرف)
- `QYODAT` / `QYODAT_INVOICE` + `QYODAT_ITEMS` = manual journal entries (قيود يومية)

**Customers (`CUSTOMERS`):**
- Same entity for customer, vendor, employee: `CUST_CUSTOM`, `CUST_VENDOR`, `CUST_EMP`
- Running balance fields: `BL_DEBIT`, `BL_CREDIT`
- Linked GL account: `ACC_ID`

### 4. Write Clear, Powerful SQL

Every delivered query must include:

```sql
-- Purpose: [one line in Arabic or English]
-- Source:   [main VIEW/TABLE used]
-- Filters:  [parameters the user can change]
-- Notes:    [assumptions, e.g. posted only, date range]
```

Structure:

1. `DECLARE` or commented parameters for dates, customer, account, branch
2. CTEs with **meaningful names** (`SalesBase`, `Receipts`, `AccountBalances`)
3. Final SELECT with readable column aliases (`AS [اسم_الحساب]`, `AS [الرصيد]`)
4. `WHERE` on indexed keys when possible: dates, `CUST_ID`, `ACC_ID`, `TRAN_NO`, `ITEM_ID`
5. Comment non-obvious business rules (e.g. exclude `BALANCE_HIDE = 1`, filter `*_STATUES`)

Use advanced T-SQL when it adds clarity:

- `SUM() OVER (PARTITION BY ... ORDER BY ...)` for running balances
- `PIVOT` / conditional aggregation for monthly columns (see `SEARCH_TRANS10` pattern)
- `UNION ALL` to merge TAKE/GIVE/BALANCE_EDIT into a unified cash ledger
- Recursive CTE on `ACCOUNTS` for account tree rollups
- `CROSS APPLY` for per-invoice line totals when views are not enough

### 5. Performance & Safety

- Read-only queries only unless the user explicitly asks for DML
- Avoid `SELECT *`; project only needed columns
- Filter early on date and ID columns
- Do not assume SQL Views exist outside the first DDL block — there are **no** stored procedures in the DDL
- Warn if `float` rounding may affect trial-balance tie-out

## Workflow Checklist

```
Task Progress:
- [ ] Clarify report goal, period, and filters (customer, account, branch, store)
- [ ] Map goal → VIEW / SEARCH_TRANS_TABLE / base table
- [ ] Verify columns in DDL or reference files
- [ ] Draft query with CTEs and comments
- [ ] Validate debit/credit orientation and sign conventions
- [ ] Provide optional variant (summary vs detail, by branch, by commissioner)
```

## Report Routing Guide

| User need | Start here |
|-----------|------------|
| Trial balance / account balances | `BALANCE_C_VIEW`, `BALANCE_T_VIEW`, `ACCOUNT_VIEW` |
| Account statement | `QYODAT_VIEW` + `TAKE_VIEW` + `GIVE_VIEW` + invoice views filtered by `ACC_ID` |
| Customer statement | `CUSTOMERS_VIEW`, `SALE_INVOICE_VIEW`, `TAKE_VIEW`, `GIVE_VIEW`, `BALANCE_EDIT_VIEW` |
| Aging / unpaid customers | `CUSTOMER_LIST_NOT_PAIED_SEARCH_TRANS_TABLE`, `SALES_AND_PAIED_SEARCH_TRANS_TABLE` |
| Sales analysis | `SALE_INVOICE_VIEW`, `SALE_ITEMS_INVOICE_VIEW` |
| Purchase analysis | `BUY_INVOICE_VIEW`, `BUY_ITEMS_INVOICE_VIEW` |
| Gross profit | `CUSTOMERS_PROFET_SEARCH_TRANS_TABLE` or join sale lines with `AVER_COST`/`LAST_COST` |
| Inventory valuation | `ITEMS_TOTALS_SEARCH_TRANS_TABLE`, `ITEM_SUB_VIEW` |
| Bank reconciliation | `BANKS_VIEW`, `BANK_TRANSFER_VIEW`, `BANK_BALANCE_EDIT_VIEW`, `TAKE_VIEW`, `GIVE_VIEW` |
| Expenses | `EXPENCES_VIEW`, `EXPENCES_INVOICE_VIEW`, `GIVE_VIEW` |
| Salaries | `SALARIES_VIEW` |
| Journal vouchers | `QYODAT_ITEMS_INVOICE_VIEW`, `QYODAT_ITEMS_VIEW` |
| Document audit trail | Match `TRAN_NO` + `TRAN_BARNCH` across relevant views |

## Output Format

Deliver:

1. **Short explanation** (2–4 sentences): what the query returns and which sources it uses
2. **Complete SQL** in one fenced `sql` block, ready to run against `Marketing2026`
3. **Parameters section**: list placeholders the user should set
4. **Optional**: a simplified summary query if the main query is complex

## Additional Resources

- [schema-reference.md](schema-reference.md) — tables, keys, relationships
- [views-index.md](views-index.md) — all dbo views by domain
- [examples.md](examples.md) — reference query patterns
