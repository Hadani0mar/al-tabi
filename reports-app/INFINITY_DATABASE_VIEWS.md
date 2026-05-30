# INFINITY_DATABASE_VIEWS — Views وقواعد الربط (InfinityRetailDB)

> للوكيل على Infinity: `search_query_patterns` في `AGENT_InfinityRetailDB.md`  
> ⚠️ `get_database_views()` في التطبيق يُرجع Marketing2026 فقط — لا تستدعِه على Infinity.

---

## Views أساسية

### Inventory.Data_View_ProductUOMBarcodes

باركود + وحدة + أسعار (UomPrice1–4, UomLastCost) + ProductID_FK.

```sql
FROM Inventory.Data_Products p
INNER JOIN Inventory.Data_View_ProductUOMBarcodes b ON p.ProductID_PK = b.ProductID_FK
WHERE p.IsInActive = 0
```

### Inventory.Data_View_ProductUOMs

وحدات وأسعار بدون باركود.

### Inventory.Data_View_Products

منتج + UOM + مورد رئيسي (SupplierName).

### SALES.Data_View_SalesInvoiceItems

بنود مبيعات مع ProductName, UOMName, UnitPrice, QYT — JOIN `Data_SalesInvoices` للتاريخ.

### Purchase.Data_View_PurchaseInvoices

رأس فاتورة شراء + SupplierName + حالة.

---

## قواعد الربط

### إيرادات البيع

```sql
FROM SALES.Data_SalesInvoices inv
INNER JOIN SALES.Data_SalesInvoiceItems si ON si.SalesInvoiceID_FK = inv.SalesInvoiceID_PK
-- SUM(si.QYT * si.UnitPrice)
-- فلتر التاريخ: inv.SalesInvoiceDate
```

### آخر سعر شراء / مورد

```sql
FROM Purchase.Data_PurchaseInvoiceItems pi
INNER JOIN Purchase.Data_PurchaseInvoices inv ON inv.InvoiceID_PK = pi.InvoiceID_FK
LEFT JOIN Purchase.Data_Suppliers s ON s.SupplierID_PK = inv.SupplierID_FK
ORDER BY inv.InvoiceDate DESC
```

### مخزون + صلاحية

```sql
FROM Inventory.Data_ProductInventories i
INNER JOIN Inventory.Data_Products p ON p.ProductID_PK = i.ProductID_FK
WHERE i.StockOnHand > 0 AND i.ExpiryDate IS NOT NULL
```

---

## anti-patterns (Infinity)

| ❌ خطأ | ✅ صحيح |
|-------|---------|
| JOIN dbo.SALE_INVOICE | SALES.Data_SalesInvoices |
| تاريخ من بنود فقط | SalesInvoiceDate من الرأس |
| dbo.ITEMS_SUB.QTY | Data_ProductInventories.StockOnHand أو Products.StockOnHand |
| Config_View_Branchs فقط | Config_Branchs مباشرة للملف الشخصي |

---

## مرجع أنماط جاهزة

| الموضوع | نمط في AGENT_InfinityRetailDB.md |
|---------|----------------------------------|
| تفاصيل منتج | `تفاصيل-منتج-وحدات-أسعار` |
| صلاحية | `منتجات-قاربت-انتهاء` |
| نواقص | `متابعة-النواقص` |
| مبيعات يومية | `مبيعات-يومية-موظف` |
| آخر شراء | `آخر-سعر-شراء-مورد` |
| جرد فرع | `جرد-المخزون-حسب-الفرع` |
