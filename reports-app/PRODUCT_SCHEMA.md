# PRODUCT_SCHEMA — مرجع أعمدة المنتجات (Marketing2026)

> **InfinityRetailDB:** [`INFINITY_PRODUCT_SCHEMA.md`](./INFINITY_PRODUCT_SCHEMA.md)  
> **معمارية ERP:** [`ERP_ARCHITECTURE.md`](./ERP_ARCHITECTURE.md)  
> ⚠️ أداة `get_product_schema()` تُرجع **Marketing فقط** — على Infinity استخدم `search_query_patterns`.

# مستخرج من Full_Marketing_Database_DDL.sql — للوكيل الذكي
#
# استدعِ الأداة: get_product_schema()
# أو ابحث في QUERY_PATTERNS عن: دراسة-منتج-شاملة، تفاصيل-منتج-وحدات-أسعار

---

## الجداول الأساسية للمنتج

### dbo.ITEMS — بطاقة الصنف (الكتالوج)

| العمود | النوع | الوصف |
|--------|------|--------|
| ITEM_ID | int PK | المفتاح — يُربط به كل شيء |
| ITEM_MODEL | varchar(50) | **كود المنتج** (البحث الأدق) |
| ITEM_NAME | varchar(800) | **اسم المنتج** (وصف الاسم التجاري) |
| CAT1_ID … CAT7_ID | smallint | تصنيفات → CATEGORY1 … CATEGORY7 |
| MIN_LEVEL / MAX_LEVEL | float | حد أدنى/أعلى للمخزون |
| LAST_COST | float | آخر تكلفة شراء مسجّلة على البطاقة |
| AVER_COST | float | متوسط التكلفة |
| AVER_COST_DIS | float | متوسط بعد خصم |
| AVER_COST_AFTER | float | متوسط بعد تعديل |
| PLACE | varchar(30) | مكان التخزين / الرف |
| ITEM_INVISIBLE | bit | 1 = مخفي/محذوف — فلتر دائماً = 0 |
| ITEM_UPDATE_DATE | datetime | آخر تحديث للبطاقة |
| SERIAL | bit | تتبع سيريال |
| NO_COST | bit | بدون تكلفة |
| PRICE_QTY | bit | تسعير حسب الكمية |

**ملاحظة:** لا يوجد عمود «وصف طويل» منفصل — الاسم في ITEM_NAME هو الوصف الرئيسي.

---

### dbo.ITEMS_SUB — المخزون الفعلي (مصدر الحقيقة للكميات)

| العمود | الوصف |
|--------|--------|
| ITEM_SUB_ID | مفتاح |
| ITEM_ID | → ITEMS |
| STORE_ID | → STORES |
| QTY | **الكمية في المخزن** |
| CATEOGRY1 | رقم دفعة (Batch) — نص |
| CATEOGRY2 | Sub-batch |
| CATEOGRY3 | **تاريخ الصلاحية** (datetime) — مهم للصلاحية |
| OFS / CAT3 | أعلام نظام |

**الرصيد الإجمالي:** `SUM(QTY) GROUP BY ITEM_ID` من ITEMS_SUB.

---

### dbo.UNITS — تعريف الوحدات

| العمود | الوصف |
|--------|--------|
| UNIT_ID | مفتاح |
| UNIT_DISC | **وصف الوحدة** (علبة، شريط، كرتون…) |
| UNIT_QTY | معامل التحويل للوحدة الأساسية |
| UNIT_INVISIBLE | مخفية |

---

### dbo.BARCODE — وحدات البيع + أسعار التسعير لكل صنف

| العمود | الوصف |
|--------|--------|
| BAR_ID | مفتاح |
| ITEM_ID | → ITEMS |
| UNIT_ID | → UNITS |
| BARCODE | الباركود |
| UNIT_QTY | كمية الوحدة |
| PRICE1 … PRICE5 | **أسعار البيع** (مستويات 1–5) |
| PUBLIC_PRICE | سعر الجمهور |
| PRICE_LESS | أقل سعر مسموح |
| QTY / PRICE_QTY | تسعير بالكمية |
| FREE_LEVEL / FREE_QTY | عروض مجانية (حتى 6 مستويات) |
| UPDATE_DATE | آخر تحديث سعر |

**ربط الوحدة بالاسم:** `BARCODE JOIN UNITS ON UNIT_ID`.

---

### بنود الحركة (وحدة + سعر في كل معاملة)

| جدول | أعمدة الوحدة/السعر |
|------|---------------------|
| SALE_ITEMS | UNIT_ID, UNIT_QTY, PRICE, QTY, DISCOUNT |
| BUY_ITEMS | UNIT_ID, UNIT_QTY, PRICE, QTY |
| R_S_ITEMS | UNIT_ID, UNIT_QTY, PRICE, QTY |
| B_R_ITEMS | UNIT_ID, UNIT_QTY, PRICE, QTY |
| TRANSFER_ITEMS | UNIT_ID, UNIT_QTY, QTY |
| COLLECT_DETAILS | UNIT_ID, UNIT_QTY, PRICE, QTY (باقات) |

---

### dbo.REORDER_ITEMS — حد إعادة الطلب (إن وُجد)

| العمود | الوصف |
|--------|--------|
| ITEM_ID | الصنف |
| QTY | كمية إعادة الطلب المقترحة في النظام |

---

## علاقات التصنيف

```
ITEMS.CAT1_ID → CATEGORY1.CAT1_NAME
ITEMS.CAT2_ID → CATEGORY2.CAT2_NAME
… حتى CAT7
```

---

## صيغ تحليل منتج واحد (مرجع سريع)

| المقياس | الصيغة |
|---------|--------|
| مخزون حالي | SUM(ITEMS_SUB.QTY) |
| مبيعات الفترة | SALE_ITEMS − R_S_ITEMS (مردودات سالبة) |
| معدل يومي | SoldQty / CAST(ActiveSaleDays AS float) |
| أيام تغطية | Stock / DailyRate |
| كمية شراء مقترحة | MAX(0, DailyRate × أيام_تغطية − Stock) |
| تاريخ مرجع المبيعات | MAX(S_DATE) من SALE_INVOICE وليس GETDATE() |

---

## أخطاء شائعة

- لا تستخدم BALANCE_C — فارغ.
- CATEOGRY3 في ITEMS_SUB = صلاحية وليس «فئة 3».
- COLLECT = باقات منتجات وليس مقبوضات (TAKE).
- للبحث عن صنف: `ITEM_MODEL LIKE N'%x%' OR ITEM_NAME LIKE N'%x%'`.

---

# نهاية الملف
