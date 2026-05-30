# DATABASE_VIEWS — Views وقواعد الربط (Marketing2026)

> **InfinityRetailDB:** [`INFINITY_DATABASE_VIEWS.md`](./INFINITY_DATABASE_VIEWS.md)  
> ⚠️ أداة `get_database_views()` تُرجع **Marketing فقط**.

# للوكيل: get_database_views() | search_query_patterns("مبيعات يومية موظف")
# مستخرج من DDL + INFORMATION_SCHEMA + اختبار sqlcmd

---

## ⚠️ قواعد ذهبية قبل كتابة SQL (تمنع أخطاء التجميع)

### 1. إيرادات البيع = على مستوى **البند** وليس الرأس

```sql
-- ✅ صحيح: JOIN ثم SUM(QTY * PRICE)
FROM dbo.SALE_ITEMS SI
JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
-- GROUP BY ...

-- ❌ خطأ شائع: SUM(PRICE) داخل subquery على SALE_ITEMS ثم JOIN — يضاعف/يُDistorts الإجمالي
-- ❌ خطأ: استخدام SALE_ITEMS بدون JOIN لـ SALE_INVOICE عند فلترة بالتاريخ (لا يوجد S_DATE في SALE_ITEMS)
```

### 2. التاريخ دائماً من رأس الفاتورة

| الجدول | عمود التاريخ |
|--------|--------------|
| SALE_INVOICE | S_DATE |
| BUY_INVOICE | B_DATE |
| R_S_INVOICE | S_R_DATE |
| SALE_ITEMS | **لا يوجد** — استخدم INV.S_DATE |

### 3. الموظف / المستخدم

| السؤال | المصدر |
|--------|--------|
| من **أدخل** فاتورة البيع؟ | `SALE_INVOICE.USERS_ID` → `USERS.FULL_NAME` |
| مندوب عمولة (COMM) | `COMMISSIONER` — **غير مستخدم** (COMM_ID=0 فقط) |
| موظف كـ **مورد/زبون** | `CUSTOMERS.CUST_EMP=1` — ليس مبيعاته |

### 4. WHEN to use VIEW vs base tables

| Prefer VIEW | When |
|-------------|------|
| `SALE_ITEMS_INVOICE_VIEW` | تقارير مبيعات: بند + تاريخ + موظف + منتج + عميل جاهز |
| `SALE_INVOICE_VIEW` | رأس فاتورة + FULL_NAME + CUST_NAME بدون بنود |
| `BUY_ITEMS_INVOICE_VIEW` | مشتريات بنفس الفكرة |
| Base tables | تجميع معقد، CTEs، أو عندما VIEW ثقيل |

### 5. صافي المبيعات (مع مردودات)

```sql
-- مبيعات
SELECT SI.QTY * SI.PRICE FROM SALE_ITEMS SI JOIN SALE_INVOICE INV ...
UNION ALL
-- مردودات (سالب)
SELECT -RSI.QTY * RSI.PRICE FROM R_S_ITEMS RSI JOIN R_S_INVOICE RINV ...
```

---

## Views الأساسية للمبيعات والموظفين

### dbo.SALE_ITEMS_INVOICE_VIEW ⭐ الأهم لتقارير المبيعات

**ي joins:** SALE_ITEMS + SALE_INVOICE + USERS + ITEMS + CUSTOMERS + STORES + UNITS + …

| عمود م useful | الوصف |
|---------------|--------|
| S_ID, S_ITEM_ID | فاتورة / بند |
| S_DATE | **تاريخ البيع** |
| USERS_ID, FULL_NAME | الموظف المُدخل |
| QTY, PRICE, UNIT_DISC | الكمية والسعر والوحدة |
| ITEM_MODEL, ITEM_NAME | المنتج |
| CUST_NAME, CUST_NO | العميل |
| STORE_NAME | المخزن |
| S_STATUES | حالة الفاتورة |

**إيراد البند:** `QTY * PRICE`

### dbo.SALE_INVOICE_VIEW

رأس الفاتورة فقط + FULL_NAME + COMM_NAME + CUST_NAME — **بدون** QTY/PRICE للبنود.

### dbo.BUY_ITEMS_INVOICE_VIEW / dbo.BUY_INVOICE_VIEW

نفس نمط المشتريات: B_DATE, FULL_NAME, QTY, PRICE.

### dbo.USER_VIEW

USERS_ID, FULL_NAME, USER_NAMES — قائمة المستخدمين.

### dbo.ITEMS_VIEW

ITEM_ID, ITEM_MODEL, ITEM_NAME, LAST_COST, AVER_COST, BARCODE fields.

### dbo.ITEM_SUB_VIEW / dbo.ITEMS_SUB_BARCODE_VIEW

مخزون + منتج + مخزن.

---

## Views أخرى (133 view في القاعدة)

| View | الاستخدام |
|------|-----------|
| R_S_ITEMS_INVOICE_VIEW | مردودات مبيعات |
| B_R_ITEMS_INVOICE_VIEW | مردودات مشتريات |
| M_SALE_INVOICE_VIEW / M_SALE_ITEMS_VIEW | مبيعات مجمّعة (MASM) |
| INVO_DAY_MOVEMENT_VIEW | حركة يومية |
| SALARIES_VIEW | رواتب |
| TAKE_VIEW / GIVE_VIEW | مقبوضات / مدفوعات |
| CUSTOMERS_VIEW | عملاء/موردين |

**تجنّب للتحليل:** `*_SEARCH_TRANS_TABLE` (جداول وليست views)، `*_DELETED_*`, `Invoice_Items` (فارغ).

---

## خوارزمية: مبيعات يومية لكل موظف

```
1. LineSales = SALE_ITEMS JOIN SALE_INVOICE [→ USERS]
2. LineValue = QTY * PRICE  (لكل بند)
3. GROUP BY CAST(S_DATE AS date), USERS_ID, FULL_NAME
4. SUM(LineValue) = Revenue, COUNT(DISTINCT S_ID) = Invoices
```

**بديل أسرع:** `SALE_ITEMS_INVOICE_VIEW` — نفس GROUP BY على S_DATE + FULL_NAME.

---

## خوارزمية: مبيعات أمس / اليوم

```sql
DECLARE @Day date = DATEADD(day, -1, CAST(GETDATE() AS date)); -- أمس
-- أو: @Day = CAST(GETDATE() AS date) لليوم
-- تحقق: SELECT MAX(S_DATE) FROM SALE_INVOICE — إن @Day أحدث من آخر بيع = لا بيانات
WHERE CAST(INV.S_DATE AS date) = @Day
```

**Anchor للبيانات القديمة:** `(SELECT CAST(MAX(S_DATE) AS date) FROM SALE_INVOICE)`

---

## أخطاء شائعة (لا تكررها)

1. `SUM(SI.PRICE)` بدون `* QTY`
2. Subquery في SELECT يجمع PRICE ثم JOIN على INV — **خطأ**
3. GROUP BY INV.S_ID فقط ثم SUM — يُكرّر إذا لم تُجمع البنود أولاً
4. فلترة `SALE_ITEMS.S_TIME` بدل `SALE_INVOICE.S_DATE` (S_TIME قد يختلف)
5. استخدام COMMISSIONER للموظفين
6. `GETDATE()` لـ «أمس» دون التحقق من MAX(S_DATE)

---

# نهاية الملف
