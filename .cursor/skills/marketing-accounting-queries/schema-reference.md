# Marketing2026 Schema Reference

Source: first block in `Full_Marketing_Database_DDL.sql`.

## Core Accounting

### ACCOUNTS
Chart of accounts (tree).

| Column | Role |
|--------|------|
| AC_ID | PK |
| AC_P_ID | Parent account |
| AC_LEVEL, PARENT | Hierarchy |
| AC_NO, AC_NAME | Account code & name |
| AC_DEBIT_CREDIT_TYPE | Natural balance side |
| AC_TYPE | Account classification |
| ACCOUNT_COLLECT | Sub-ledger rollup flag |
| MAX_BALANCE | Credit limit |

### ACCOUNTS_DEFULTS
Single-row default GL mappings for system posting (sale, buy, take, give, store, tax, discount, salary, etc.). Column names follow `ACC_{MODULE}_{DEBIT|CREDIT}` pattern.

### QYODAT / QYODAT_INVOICE / QYODAT_ITEMS
Manual journal entries.

- Header: `Q_ID`, `Q_DATE`, `Q_STATUES`, `TRAN_NO`, `TRAN_BARNCH`
- Lines: `ACC_ID`, `ACC_DEBIT`, `ACC_CREDIT`, `Q_DESC`, `Q_NO`

### BALANCE_C / BALANCE_T / BALANCE_K
Precomputed account balance snapshots joined via `BALANCE_*_VIEW`.

Columns: `ACC_ID`, `ACC_DEBIT`, `ACC_CREDIT`, `BALANCE`, `ACC_DEBIT_T`, `ACC_CREDIT_T`, `BALANCE_T`

## Cash & Bank

### TAKE (Receipts — قبض)
`T_ID`, `T_DATE`, `T_VALUE`, `CUST_ID`, `COMM_ID`, `BANK_ID`, `ACC_DEBIT`, `ACC_CREDIT`, `TRAN_NO`, `TRAN_BARNCH`, `T_STATUES`

### GIVE (Payments — صرف)
`G_ID`, `G_DATE`, `G_VALUE`, `CUST_ID`, `COMM_ID`, `BANK_ID`, `EXPENCES_ID`, `ACC_DEBIT`, `ACC_CREDIT`, `G_DISCOUNT`, `TRAN_NO`, `TRAN_BARNCH`

### BANKS
`BANK_ID`, `BANK_NAME`, `BANK_ACCOUNT`, `ACC_ID`

### BANK_TRANSFER / BANK_BALANCE_EDIT
Inter-bank transfers and bank balance adjustments with full GL account columns.

### BALANCE_EDIT
Customer balance opening/adjustment vouchers: `BL_DEBIT`, `BL_CREDIT`, `CUST_ID`, `COMM_ID`, `ACC_DEBIT`, `ACC_CREDIT`

## Sales & Purchases (GL-linked)

| Document | Header table | Items table | Key GL columns |
|----------|--------------|-------------|----------------|
| Sale | SALE_INVOICE | SALE_ITEMS | ACC_DEBIT, ACC_CREDIT, ACC_TAX_CREDIT, ACC_DIS_DEBIT |
| Sale return | R_S_INVOICE | R_S_ITEMS | ACC_DEBIT, ACC_CREDIT |
| Purchase | BUY_INVOICE | BUY_ITEMS | ACC_DEBIT, ACC_CREDIT, ACC_BUY_DIS_CREDIT |
| Purchase return | B_R_INVOICE | B_R_ITEMS | ACC_DEBIT, ACC_CREDIT |
| Opening stock | JARED_INVOICE | JARED_ITEMS | ACC_DEBIT, ACC_CREDIT |
| Spoilage | SPOIL_INVOICE | SPOIL_ITEMS | ACC_DEBIT, ACC_CREDIT |
| Transfer | TRANSFER_INVOICE | TRANSFER_ITEMS | inventory only |
| Expense invoice | EXPENCES_INVOICE | — | ACC_DEBIT, ACC_CREDIT |
| Salary | SALARIES | — | ACC_DEBIT, ACC_CREDIT |
| Letter of credit | FUNDS | — | ACC_DEBIT, ACC_CREDIT |

Common header fields: `*_DATE`, `*_STATUES`, `USERS_ID`, `TRAN_ID`, `TRAN_NO`, `TRAN_BARNCH`

## Master Data

### CUSTOMERS
Unified party master (customer/vendor/employee).

Notable: `CUST_NO`, `CUST_NAME`, `COMM1_ID..COMM4_ID`, `BL_DEBIT`, `BL_CREDIT`, `CUST_MAX_DEBIT`, `ACC_ID`, `CUST_VENDOR`, `CUST_CUSTOM`, `CUST_EMP`, `BALANCE_HIDE`

### COMMISSIONER
Sales reps: `COMM_ID`, `COMM_NO`, `COMM_NAME`, `ACC_ID`

### ITEMS / ITEMS_SUB
Items and store-level quantities. Costs: `LAST_COST`, `AVER_COST`, `AVER_COST_AFTER`

### STORES, UNITS, CURRENCY, REGION
Supporting masters with corresponding `*_VIEW` objects.

### TRANS_CATEGORY
Document type catalog: `TRAN_ID`, `TRAN_DESC`, `TRAN_SHORT_CUT`

## Report Staging Tables (*_SEARCH_TRANS_TABLE)

Populated by the application for search/report screens. Not normalized for writes.

| Table | Typical use |
|-------|-------------|
| SALE_SEARCH_TRANS_TABLE | Sales register |
| BUY_SEARCH_TRANS_TABLE | Purchase register |
| TAKE_SEARCH_TRANS_TABLE | Receipts register |
| GIVE_SEARCH_TRANS_TABLE | Payments register |
| BALANCE_SEARCH_TRANS_TABLE | Balance adjustments |
| BANK_BALANCE_SEARCH_TRANS_TABLE | Bank movements |
| SALES_AND_PAIED_SEARCH_TRANS_TABLE | Customer sales vs collections |
| CUSTOMER_LIST_NOT_PAIED_SEARCH_TRANS_TABLE | Unpaid customer analysis |
| CUSTOMERS_PROFET_SEARCH_TRANS_TABLE | Customer profitability |
| ITEMS_TOTALS_SEARCH_TRANS_TABLE | Item qty/value summary |
| ITEMS_INVOICE_MOVMENT_SEARCH_TRANS_TABLE | Detailed item movement |
| SEARCH_TRANS10 | Monthly customer balance matrix |
| SEARCH_TRANS101 | Employee/customer balance snapshot |

Common columns: `CUST_NO`, `CUST_NAME`, `COMM_NO`, `COMM_NAME`, `B_DATE`, `TOTAL`, `FLAG`, `T_NAME`, `FULL_NAME`

## Key Relationships

```
ACCOUNTS (AC_ID)
  ← ACCOUNTS.AC_P_ID (self)
  ← CUSTOMERS.ACC_ID, BANKS.ACC_ID, COMMISSIONER.ACC_ID, EXPENCES.AC_ID
  ← *.ACC_DEBIT / *.ACC_CREDIT on all financial documents

CUSTOMERS (CUST_ID)
  ← SALE_INVOICE, BUY_INVOICE, TAKE, GIVE, BALANCE_EDIT, SALARIES

SALE_INVOICE (S_ID) → SALE_ITEMS (S_ITEM_ID)
BUY_INVOICE (B_ID) → BUY_ITEMS (B_ITEM_ID)

TRAN_CATEGORY (TRAN_ID) ← *.TRAN_ID on documents
USERS (USERS_ID) ← *.USERS_ID
```

## Index Hints

Prefer filters on:

- Dates: `S_DATE`, `B_DATE`, `T_DATE`, `G_DATE`, `Q_DATE`, `BL_DATE`
- IDs: `CUST_ID`, `ACC_ID`, `ITEM_ID`, `BANK_ID`, `COMM_ID`
- Business keys: `TRAN_NO` + `TRAN_BARNCH` (unique on most documents)
- Customer name/code: `CUST_NO`, `CUST_NAME` (indexed on CUSTOMERS)
