# Marketing2026 Views Index

130 views in the first DDL block. Prefer these over manual joins.

## General Ledger & Accounts

| View | Purpose |
|------|---------|
| ACCOUNT_VIEW | Full account row + parent, type, user |
| ACCOUNT_PARENT_VIEW | Account hierarchy slice |
| ACCOUNTS_TO_TYPE_VIEW | Account ↔ type mapping |
| ACC_CREDIT / ACC_DEBIT | Account id/name helpers |
| BALANCE_C_VIEW | Customer-side trial balance lines |
| BALANCE_T_VIEW | Trial balance with parent account |
| QYODAT_VIEW | Posted journal lines flat |
| QYODAT_ITEMS_VIEW | Journal lines + account names |
| QYODAT_INVOICE_VIEW | Journal headers |
| QYODAT_ITEMS_INVOICE_VIEW | Journal header + lines |

## Cash, Bank & Balance

| View | Purpose |
|------|---------|
| TAKE_VIEW | Receipts with customer, bank, GL accounts |
| GIVE_VIEW | Payments with expense, bank, GL accounts |
| BALANCE_EDIT_VIEW | Customer balance adjustments |
| BANKS_VIEW | Banks + linked GL account |
| BANK2 | Simple bank list |
| BANK_TRANSFER_VIEW | Inter-bank transfers |
| BANK_BALANCE_EDIT_VIEW | Bank balance corrections |
| BANK_ACCESS_VIEW | Bank permissions by user group |
| FUNDS_VIEW | Letters of credit / funding |

## Sales & Returns

| View | Purpose |
|------|---------|
| SALE_INVOICE_VIEW | Sales header + customer + accounts + tax/discount |
| SALE_ITEMS_VIEW | Sale lines |
| SALE_ITEMS_INVOICE_VIEW | Sale lines + invoice + customer + categories |
| SALE_ITEMS_DELETED_VIEW | Deleted sale lines audit |
| SALE_INVOICE_VIEW_SIZE_WAIGHT | Sale qty by size/weight |
| R_S_INVOICE_VIEW | Sale return headers |
| R_S_ITEMS_VIEW / R_S_ITEMS_INVOICE_VIEW | Sale return lines |
| M_SALE_INVOICE_VIEW / M_SALE_ITEMS_VIEW | Recurring/template sales |

## Purchases & Returns

| View | Purpose |
|------|---------|
| BUY_INVOICE_VIEW | Purchase headers + GL |
| BUY_ITEMS_VIEW / BUY_ITEMS_INVOICE_VIEW | Purchase lines |
| BUY_INVOICE_EXPENCES_VIEW | Purchase-linked expenses |
| B_R_INVOICE_VIEW | Purchase return headers |
| B_R_ITEMS_VIEW / B_R_ITEMS_INVOICE_VIEW | Purchase return lines |
| M_BUY_INVOICE_VIEW / M_BUY_ITEMS_VIEW | Template purchases |
| C_BUY_INVOICE_VIEW / C_BUY_ITEMS_* | Contract purchases |

## Inventory & Costing

| View | Purpose |
|------|---------|
| ITEMS_VIEW | Items + all category names |
| ITEM_SUB_VIEW | Store-level stock |
| ITEMS_BARCODE_QTY_VIEW | Barcode, price, qty by store |
| BARCODE_VIEW / BARCODE_QTY_VIEW | Pricing & units |
| J_INVOICE_VIEW / J_ITEMS_* | Opening balance (Jared) |
| TRANSFER_INVOICE_VIEW / TRANSFER_ITEMS_* | Store transfers |
| SPOIL_INVOICE_VIEW / SPOIL_ITEMS_* | Spoilage/write-off |
| MANF_* / MASM_* | Manufacturing / assembly |

## Expenses, Payroll & Misc

| View | Purpose |
|------|---------|
| EXPENCES_VIEW | Expense types + GL account |
| EXPENCES_INVOICE_VIEW / EXPENCES_INVOICE_ITEMS_VIEW | Expense vouchers |
| SALARIES_VIEW | Payroll with GL accounts |
| INVO_DAY_MOVEMENT_VIEW | Daily invoice movement summary |

## Masters & Security

| View | Purpose |
|------|---------|
| CUSTOMERS_VIEW | Customers + region + commissioners |
| COMMISSIONER_VIEW | Sales reps |
| CATEGORY1_VIEW … CATEGORY9_VIEW | Item/customer categories |
| STORES_VIEW / STORE_VIEW1 / STORE_VIEW2 | Warehouses |
| CURRENCY_VIEW / REGION_VIEW / UNITES_VIEW | Reference data |
| TRANS_CATEGORY_VIEW | Document types |
| USER_VIEW / USER_GROUP_VIEW | Users & permissions |
| *_ACCESS_VIEW | Row-level access (bank, store, category) |

## Selection Strategy

1. **Header + lines needed?** → `*_INVOICE_VIEW` + `*_ITEMS_INVOICE_VIEW`
2. **GL account names needed?** → any view with `AC_NAME_DEBIT` / `AC_NAME_CREDIT`
3. **Customer context?** → start from `CUSTOMERS_VIEW` or invoice views (already joined)
4. **Stock + cost?** → `ITEM_SUB_VIEW` or `ITEMS_BARCODE_QTY_VIEW`
5. **Fast ERP-style report?** → matching `*_SEARCH_TRANS_TABLE` instead of views
