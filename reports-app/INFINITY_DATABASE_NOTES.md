# INFINITY_DATABASE_NOTES — ملاحظات InfinityRetailDB

> ERP: **InfinityRetailDB** | SQL Server  
> DDL: `InfinityRetailDB_DDL.sql` | معمارية: `ERP_ARCHITECTURE.md`

---

## الاتصال

```bash
sqlcmd -E -S localhost -d InfinityRetailDB
```

- اكتشاف التطبيق: `Inventory.Data_Products` أو `MyCompany.Config_Branchs` أو اسم DB.

---

## المخططات (schemas)

| Schema | محتوى |
|--------|--------|
| **Inventory** | منتجات، مخزون، باركود، UOM، تحويلات |
| **SALES** | فواتير مبيعات، عملاء، مرتجعات |
| **Purchase** | فواتير مشتريات، موردون |
| **MyCompany** | فروع، إعدادات، عملات |
| **Financial** | حسابات، سندات |
| **POS** | نقاط البيع |
| **dbo** | جداول مساعدة (Companies — ليس بيانات الصيدلية الرئيسية) |

---

## بيانات المنشأة / الفرع

| المصدر | الاستخدام |
|--------|-----------|
| `MyCompany.Config_Branchs` | **الأول** — BranchName, BranchPhone, BranchAddressLine1, IsCurrentBranch |
| `MyCompany.Config_View_Branchs` | بديل (INNER JOIN عملات — قد يفشل إن نقصت عملة) |
| `dbo.Companies` | fallback اسم شركة فقط |

التطبيق: `erp_adapters::fetch_business_profile`

---

## المنتجات النشطة

```sql
WHERE p.IsInActive = 0
```

> Default في DDL: `IsInActive` default = 1 — تأكد من الفلتر.

---

## المبيعات

| الجدول | ملاحظة |
|--------|--------|
| SALES.Data_SalesInvoices | SalesInvoiceDate ⭐ للتاريخ |
| SALES.Data_SalesInvoiceItems | QYT × UnitPrice = قيمة البند |
| SALES.RefSalesInvoiceStates | حالة الفاتورة (ملغاة: LIKE `%ملغ%` أو `%Cancel%`) |
| SALES.Data_View_SalesInvoiceItems | View جاهز للتقارير |

**تاريخ مرجعي للوكيل:** `SELECT CAST(MAX(SalesInvoiceDate) AS date) FROM SALES.Data_SalesInvoices`

---

## المشتريات

| الجدول | ملاحظة |
|--------|--------|
| Purchase.Data_PurchaseInvoices | InvoiceDate, SupplierID_FK |
| Purchase.Data_PurchaseInvoiceItems | UnitCost, ProductID_FK |
| Purchase.Data_Suppliers | SupplierName |
| Purchase.RefPurchaseInvoiceStates | حالات ملغاة |

---

## الفواتير الملغاة (التطبيق)

`erp_adapters::cancelled_invoices_sql` — UNION مبيعات + مشتريات ليوم محدد.

---

## POS / فاتورة سريعة

- بحث: `Inventory.Data_View_ProductUOMBarcodes` + `Data_Products`
- رأس الإيصال: `Config_Branchs`
- **لا يكتب** في ERP — PDF محلي فقط

---

## سجّل هنا اكتشافات جديدة

<!-- مثال:
### 2026-05-30
- ...
-->
