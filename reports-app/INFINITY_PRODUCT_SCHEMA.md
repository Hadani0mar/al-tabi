# INFINITY_PRODUCT_SCHEMA — مرجع أعمدة المنتجات (InfinityRetailDB)

> ERP: **InfinityRetailDB** | schemas: `Inventory`, `SALES`, `Purchase`  
> DDL: `InfinityRetailDB_DDL.sql` | أنماط جاهزة: `AGENT_InfinityRetailDB.md`  
> **لا تستخدم** `get_product_schema()` — تلك الأداة تُرجع Marketing2026 فقط.

---

## الجداول الأساسية

### Inventory.Data_Products — بطاقة المنتج

| العمود | الوصف | تسمية التقرير |
|--------|--------|---------------|
| ProductID_PK | bigint PK | — |
| ProductCode | nvarchar | **الكود** |
| ProductName | nvarchar | **اسم المنتج** |
| SalesDecription | nvarchar | وصف المبيعات |
| StockOnHand | decimal | **الكمية المتاحة** (إجمالي) |
| MinStockLevel / MaxStockLevel | decimal | حدود المخزون |
| IsInActive | bit | 0 = نشط (فلتر دائماً `IsInActive = 0`) |
| MainSupplierID_FK | int | مورد رئيسي (اختياري) |
| ModifiedDate | datetime | **تاريخ التعديل** |

---

### Inventory.Data_View_ProductUOMBarcodes — باركود + وحدة + أسعار

| العمود | الوصف | تسمية التقرير |
|--------|--------|---------------|
| ProductBarcode | nvarchar | **الباركود** |
| UOMName | nvarchar | **وحدة القياس** |
| UomPrice1 | decimal | **السعر** (سعر البيع الأساسي) |
| UomPrice2 | decimal | **سعر 2** |
| UomPrice3 / UomPrice4 | decimal | **سعر 4** (في تقرير البحث) |
| UomLastCost | decimal | **آخر تكلفة** |
| BaseUnitQYT | decimal | معامل التحويل للوحدة الأساسية |
| ProductID_FK | → Data_Products | — |

**View بديل:** `Inventory.Data_View_ProductUOMs` (بدون باركود).

---

### Inventory.Data_ProductInventories — مخزون بالفرع/الموقع/الدفعة

| العمود | الوصف |
|--------|--------|
| ProductID_FK | → Data_Products |
| BranchID_FK | → MyCompany.Config_Branchs |
| LocationID_FK | → Config_BranchLocations |
| StockOnHand | الكمية |
| ExpiryDate | **تاريخ الصلاحية** ⭐ |

---

## الموردون والشراء

| الجدول | الاستخدام |
|--------|-----------|
| Purchase.Data_Suppliers | SupplierName |
| Purchase.Data_PurchaseInvoices | InvoiceDate, SupplierID_FK |
| Purchase.Data_PurchaseInvoiceItems | ProductID_FK, UnitCost, QYT |

**آخر مورد:** أحدث `InvoiceDate` في `Data_PurchaseInvoices` مرتبط ببنود الشراء للمنتج.

---

## بحث المنتجات (LIKE)

```sql
WHERE p.IsInActive = 0
  AND (p.ProductName LIKE N'%term%' OR p.ProductCode LIKE N'%term%')
```

مع باركود:

```sql
AND (p.ProductName LIKE N'%term%' OR p.ProductCode LIKE N'%term%' OR b.ProductBarcode LIKE N'%term%')
```

---

## تقرير التطبيق: البحث عن تفاصيل المنتج

يُنفَّذ عبر `erp_adapters::infinity_product_comprehensive_sql` — الأعمدة:

`الكود | اسم المنتج | وحدة القياس | الباركود | السعر | سعر 2 | سعر 4 | آخر تكلفة | الكمية المتاحة | آخر مورد | تاريخ التعديل`

---

## anti-patterns

- ❌ `dbo.ITEMS`, `dbo.BARCODE`, `SALE_INVOICE`
- ❌ `LIMIT`, `ILIKE`, `NOW()` — استخدم `TOP`, `LIKE N'%'`, `GETDATE()`
- ❌ `IsInActive = 1` للمنتجات المعروضة
- ❌ تسمية `UomPrice4` بـ «سعر الجمهور» — Marketing فقط
