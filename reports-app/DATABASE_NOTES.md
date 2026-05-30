# ملاحظات قاعدة البيانات — Marketing2026

> **InfinityRetailDB:** [`INFINITY_DATABASE_NOTES.md`](./INFINITY_DATABASE_NOTES.md)  
> **معمارية ERP:** [`ERP_ARCHITECTURE.md`](./ERP_ARCHITECTURE.md)

## بيانات الاتصال
- **السيرفر:** localhost (MSSQLSERVER — خدمة محلية)
- **قاعدة البيانات:** Marketing2026
- **المصادقة:** Windows Authentication تعمل مع sqlcmd -E

## العملة الرسمية في النظام
- **العملة:** د.ل (دينار ليبي)
- يتم إلحاق رمز العملة `د.ل` عند عرض أي مبالغ مالية، قيم، تكاليف، أو أسعار في ردود الوكيل الذكي (مثال: `150.00 د.ل`).

## هيكل الجداول المهمة

### COMMISSIONER (جدول المندوبين)
```
COMM_ID smallint IDENTITY(1,1) PK
COMM_NAME varchar(50)
```
**⚠️ مهم:** يحتوي سجلاً واحداً فقط: `COMM_ID=0 / COMM_NAME='N/A'`  
جدول المندوبين **غير مستخدم** في النظام — كل الفواتير مرتبطة بـ COMM_ID=0.

### BUY_INVOICE (فواتير الشراء)
```
B_ID int PK
B_DATE datetime
CUST_ID int FK→CUSTOMERS      ← اسم المورد (الشركة التي اشترينا منها)
COMM_ID smallint DEFAULT 0    ← دائماً 0، لا تستخدمه
USERS_ID int FK→USERS         ← من أدخل الفاتورة في النظام
```

### BUY_ITEMS (بنود فواتير الشراء)
```
B_ITEM_ID int PK
B_ID int FK→BUY_INVOICE
ITEM_ID int FK→ITEMS
PRICE float
QTY float
```

### CUSTOMERS (الموردون والزبائن — جدول موحد)
```
CUST_ID int PK
CUST_NAME varchar(100)        ← اسم شركة المورد
COMM1_ID..COMM4_ID smallint   ← دائماً 0، غير مستخدمة
```

### USERS (موظفو الشركة)
```
USERS_ID int PK
FULL_NAME varchar(50)         ← اسم الموظف (احمد مختي، محمد احمودة...)
```

## SALE_INVOICE (فواتير المبيعات)
```
S_ID int PK
S_DATE datetime           ← تاريخ الفاتورة (هنا فقط، ليس في SALE_ITEMS!)
CUST_ID int FK→CUSTOMERS  ← الزبون
CUST_NAME varchar(100)    ← اسم الزبون مكرّر مباشرة (لتسريع التقارير)
USERS_ID int FK→USERS     ← من أدخل الفاتورة
S_DISCOUNT, S_TAX1, S_TAX2, S_SHIPMENT float
S_NOTE varchar(100)
S_STATUES tinyint
WAIT bit                  ← معلَّقة
```

## SALE_ITEMS (بنود فواتير المبيعات)
```
S_ITEM_ID int PK
S_ID int FK→SALE_INVOICE  ← المفتاح للوصول للتاريخ
ITEM_ID int FK→ITEMS
STORE_ID, QTY float, PRICE float
LAST_COST, AVER_COST, PUBLIC_PRICE float
S_TIME datetime           ← وقت البيع لكل بند (اختياري)
CATEOGRY1 varchar=Batch, CATEOGRY3 datetime=Expiry
BARCODE varchar(50)
```
⚠️ **SALE_ITEMS لا يحتوي S_DATE** — للتاريخ يجب JOIN لـ SALE_INVOICE عبر S_ID.

## ITEMS (كتالوج المنتجات)
```
ITEM_ID PK, ITEM_MODEL varchar(50)=الكود
ITEM_NAME varchar(800)
LAST_COST float, AVER_COST float
MIN_LEVEL, MAX_LEVEL float
ITEM_INVISIBLE bit         ← محذوف soft-delete
ITEM_UPDATE_DATE datetime
PLACE varchar(30)
```

## ITEMS_SUB (المخزون الحالي لكل صنف/مخزن/دفعة)
```
ITEM_SUB_ID PK, ITEM_ID FK→ITEMS, STORE_ID FK→STORES
QTY float                  ← الكمية المتوفرة
CATEOGRY1 varchar(10)      ← Batch (رقم الدفعة)
CATEOGRY3 datetime         ← ⭐ تاريخ الصلاحية (مع INDEX على هذا العمود)
```

## BUY_ITEMS (بنود فواتير الشراء)
```
B_ITEM_ID PK, B_ID FK→BUY_INVOICE, ITEM_ID FK→ITEMS
STORE_ID, QTY, PRICE, UNIT_ID, UNIT_QTY, RATE, CURRENCY_ID
CATEOGRY1=Batch, CATEOGRY3=Expiry
BARCODE varchar(30)
```

## STORES (المخازن)
```
STORE_ID PK, STORE_NAME varchar(50)
```

## BALANCE_C (الأرصدة الحية)
```
ACC_ID, ACC_DEBIT float, ACC_CREDIT float, BALANCE float
```
JOIN: `CUSTOMERS.ACC_ID = BALANCE_C.ACC_ID` للرصيد الحالي.

## CUSTOMERS — الحقول المهمة الإضافية
```
CUST_VENDOR bit             ← 1 لو مورد
CUST_CUSTOM bit             ← 1 لو زبون
CUST_EMP bit                ← 1 لو موظف
CUST_MAX_DEBIT float        ← الحد الأقصى للدين المسموح
BL_DEBIT, BL_CREDIT float   ← رصيد ابتدائي
ACC_ID int                  ← مفتاح لـ BALANCE_C
```

## نطاق التواريخ في البيانات
- **أقدم بيانات مبيعات:** 2025-07-19
- **أحدث بيانات مبيعات:** 2026-04-07
- **اليوم:** ~2026-05-24
- **⚠️ مهم:** أحدث بيانات قبل ~47 يوم → يجب استخدام `DAYS_RECENT=60` وليس 30

## القيم الافتراضية في telegram.rs
```rust
DAYS_RECENT = "60"   // كان 30 — لا يغطي البيانات
DAYS_TOTAL  = "180"  // كان 90
```

## طلبية شراء ذكية (2026-05-25 — مُختبر بـ sqlcmd)

**الملف:** `reports-app/smart_purchase_order.sql`

**تاريخ المرجع:** `@AsOfDate = MAX(S_DATE)` من `SALE_INVOICE` ≈ `2026-04-07` — لا تستخدم `GETDATE()` كحد أقصى للنافذة لأن آخر مبيعات قبل ~48 يوم من اليوم.

**المخزون:** `SUM(ITEMS_SUB.QTY)` مجمّعاً على `ITEM_ID` (ليس من `ITEMS`).

**المبيعات الصافية:** `SALE_ITEMS` + `SALE_INVOICE.S_DATE` ناقص `R_S_ITEMS` (كميات سالبة) + `R_S_INVOICE.S_R_DATE`.

**معدل يومي:** `SoldQty / COUNT(DISTINCT sale_dates)` — أيام البيع الفعلية وليس `@DaysRecent` تقويمياً.

**أيام تغطية الرصيد:** `StockQty / DailyRate`.

**كمية شراء مقترحة:** `MAX(0, DailyRate * @CoverageDays - StockQty)` — `@CoverageDays` افتراضي 30.

**الوكيل:** القالب مُضمّن في `ai_agent.rs` → `SMART_PURCHASE_ORDER_TEMPLATE` (نفس نمط قوالب `**Q:**` في system prompt).

## متابعة النواقص (2026-05-25 — مُختبر بـ sqlcmd)

**الملف:** `reports-app/shortage_tracking.sql`

- مراقبة فقط (حالة + فجوة vs MIN_LEVEL) — ليس طلبية شراء ذكية
- حالات: `نفاد`، `تحت الحد الأدنى`، `قريب من النفاد`
- الوكيل: `SHORTAGE_TRACKING_TEMPLATE`

## متابعة الديون لي / علي (2026-05-25 — مُختبر بـ sqlcmd)

**الملف:** `reports-app/debts_tracking.sql`

- ⚠️ **`BALANCE_C` فارغ** (0 صفوف) — لا تعتمد عليه وحده
- **لي (زبائن):** `SALE` − `R_S` − `TAKE` + `BALANCE_EDIT`
- **علي (موردين):** `BUY` − `B_R` − `GIVE` + `BALANCE_EDIT`
- جداول الدفع: `dbo.TAKE` (1888 صف)، `dbo.GIVE` (1030 صف)
- الوكيل: `DEBTS_TRACKING_TEMPLATE`

## كيفية جلب "من اشترينا منه"

### اسم المورد (الشركة)
```sql
LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
-- CU.CUST_NAME → "شركة الشاملة الدوائية"، "الطبيبة الدوائية"...
```

### من أدخل الفاتورة (الموظف الداخلي)
```sql
LEFT JOIN dbo.USERS U ON B.USERS_ID = U.USERS_ID
-- U.FULL_NAME → "احمد مختي"، "محمد احمودة"...
```

### ❌ لا تستخدم COMMISSIONER — دائماً N/A
```sql
-- هذا لا يعطي نتيجة مفيدة:
LEFT JOIN dbo.COMMISSIONER C ON B.COMM_ID = C.COMM_ID
```

## التقارير في Supabase (project: nsgmhijtaaenpqxxgjds)

| ID | الاسم | ملاحظات |
|----|-------|---------|
| ed26a179... | تحليل النواقص وأولويات الشراء | يستخدم DAYS_RECENT/DAYS_TOTAL، يُظهر المورد والمستخدم |
| 8119bfd5... | معرفة آخر سعر شراء لمنتج | يبحث بـ SEARCH_TERM، يُظهر المورد والمستخدم |

## ملاحظات PDF (pdf_generator.rs)
- الخط: يحاول arialuni.ttf أولاً ثم arial.ttf
- العربية: reshape_arabic() → visual_order() قبل كل use_text()
- reshape_arabic: يحوّل لـ Unicode Presentation Forms (FE70-FEFF)
- visual_order: unicode-bidi للترتيب RTL→LTR

## ⭐ اكتشاف حاسم: تاريخ الصلاحية في النظام = `CATEOGRY3` (datetime)

النمط المُتكرر في كل جداول البنود والمخزون:
```
CATEOGRY1 varchar(10)   ← رقم الدفعة (Batch / Lot number)
CATEOGRY2 varchar(10)   ← رقم دفعة فرعي (Sub-batch)
CATEOGRY3 datetime      ← ⭐ تاريخ الصلاحية (Expiry date)
```

اسم العمود `CATEOGRY3` مضلِّل — لكنه **datetime** وليس فئة (CATEGORY). يوجد جدول منفصل اسمه `dbo.CATEOGRY3` يحتوي `CAT3_ID, CAT3_NAME` للفئات الفعلية، لكن العمود `CATEOGRY3` داخل جداول البنود **هو تاريخ الصلاحية**.

### الجداول التي تحتوي `CATEOGRY3 datetime` (تاريخ الصلاحية)

| الجدول | الاستخدام |
|--------|----------|
| `dbo.ITEMS_SUB` | **المخزون الحالي** لكل صنف/مخزن (الأهم — يحتوي INDEX على CATEOGRY3) |
| `dbo.BUY_ITEMS` | بنود فواتير الشراء (الدفعة وتاريخها وقت الشراء) |
| `dbo.SALE_ITEMS` | بنود فواتير المبيعات |
| `dbo.JARED_ITEMS_B` | الجرد الفعلي |
| `dbo.MANF_F_ITEMS` / `MANF_T_ITEMS` | مخرجات/مدخلات التصنيع |
| `dbo.C_BUY_ITEMS` | فواتير شراء مغلقة |
| `dbo.CLOSE_ITEMS_SUB` | المخزون عند إغلاق الفترة |
| `dbo.ITEMS_SUB_DATA_COLLECTOR` | جامع بيانات للمخزون |
| كل جداول `*_DELETED` | المحذوفات |

### مثال: المنتجات منتهية الصلاحية في المخزون الحالي
```sql
SELECT TOP 10 
    I.ITEM_NAME,
    [IS].CATEOGRY3 AS Expiry,
    [IS].CATEOGRY1 AS Batch,
    [IS].QTY
FROM dbo.ITEMS_SUB [IS]
LEFT JOIN dbo.ITEMS I ON [IS].ITEM_ID = I.ITEM_ID
WHERE [IS].CATEOGRY3 IS NOT NULL
  AND [IS].CATEOGRY3 < GETDATE()
  AND [IS].QTY > 0
ORDER BY [IS].CATEOGRY3 ASC;
```
> ملاحظة: `IS` كلمة محجوزة في SQL Server، استخدم `[IS]` أو alias مختلف مثل `S` أو `SUB`.

### الجدول الثاني: `dbo.Invoice_Items` (مختلف عن البقية!)
هذا الجدول لـ بنود فاتورة واحدة فقط (مؤقت، يُمسح بعد الحفظ):
- `Expiry varchar(30)` — تاريخ كنص (مثل `15/03/2026`)
- `Batch varchar(20)`
- استخدمه فقط لاستعلامات الفاتورة الجارية، ليس للمخزون الكلّي.

للمقارنة عند الحاجة:
```sql
WHERE ISDATE(Expiry) = 1 
  AND TRY_CONVERT(date, Expiry, 103) < GETDATE()
```
(`103` = `dd/mm/yyyy`، `105` = `dd-mm-yyyy`)

## 🔄 جداول المردودات والتالف والتحويل والوحدات والإعدادات

### 1. مردودات المبيعات (Sales Returns)
- **`dbo.R_S_INVOICE`** (فواتير مردودات المبيعات):
  - `S_R_ID` PK
  - `S_R_DATE` datetime (تاريخ المردود)
  - `CUST_ID` int FK (الزبون)
  - `CUST_NAME` varchar(100) (اسم الزبون)
  - `USERS_ID` int FK (من أدخل الفاتورة)
  - `S_R_NOTE` varchar(100) (ملاحظة الفاتورة)
- **`dbo.R_S_ITEMS`** (بنود مردودات المبيعات):
  - `S_R_ITEM_ID` PK
  - `S_R_ID` int FK (معرف الفاتورة)
  - `ITEM_ID` int FK (المنتج)
  - `STORE_ID` int FK (المخزن)
  - `QTY` float (الكمية المرجعة)
  - `PRICE` float (سعر الإرجاع للزبون)
  - `CATEOGRY3` datetime (تاريخ صلاحية البند المرجع)

### 2. مردودات المشتريات (Purchase Returns)
- **`dbo.B_R_INVOICE`** (فواتير مردودات المشتريات للموردين):
  - `B_R_ID` PK
  - `B_R_DATE` datetime (تاريخ مردود الشراء)
  - `CUST_ID` int FK (المورد)
  - `USERS_ID` int FK (المستخدم)
  - `B_R_NOTE` varchar(100)
- **`dbo.B_R_ITEMS`** (بنود مردودات المشتريات):
  - `B_R_ITEM_ID` PK
  - `B_R_ID` int FK
  - `ITEM_ID` int FK
  - `STORE_ID` int FK
  - `QTY` float (الكمية المرجعة للمورد)
  - `PRICE` float (سعر الإرجاع للمورد)
  - `CATEOGRY3` datetime (الصلاحية)

### 3. الأدوية التالفة / المتلفة (Spoiled Products)
- **`dbo.SPOIL_INVOICE`** (فواتير التالف):
  - `SP_ID` PK
  - `SP_DATE` datetime (تاريخ الإتلاف)
  - `SP_NOTE` varchar(100)
  - `USERS_ID` int FK
- **`dbo.SPOIL_ITEMS`** (بنود التالف):
  - `SP_ITEM_ID` PK
  - `SP_ID` int FK
  - `ITEM_ID` int FK
  - `STORE_ID` int FK
  - `QTY` float (الكمية التالفة)
  - `PRICE` float (سعر التكلفة للتالف)
  - `CATEOGRY3` datetime (صلاحية التالف)

### 4. تحويلات الأصناف بين المخازن (Warehouse Transfers)
- **`dbo.TRANSFER_INVOICE`** (فواتير التحويل):
  - `TR_ID` PK
  - `TR_DATE` datetime (تاريخ التحويل)
  - `TR_NOTE` varchar(100)
  - `USERS_ID` int FK
- **`dbo.TRANSFER_ITEMS`** (بنود التحويل):
  - `TR_ITEM_ID` PK
  - `TR_ID` int FK
  - `ITEM_ID` int FK
  - `QTY` float (الكمية المحولة)
  - `STORE_F_ID` int FK (المخزن المحول منه - From Store)
  - `STORE_T_ID` int FK (المخزن المحول إليه - To Store)
  - `CATEOGRY3` datetime (الصلاحية)

### 5. وحدات القياس (Units of Products)
- **`dbo.UNITS`** (جدول الوحدات):
  - `UNIT_ID` PK
  - `UNIT_DISC` varchar(20) (اسم الوحدة: علبة، شريط، حبة...)
  - `UNIT_QTY` float (النسبة للوحدة الأساسية)

### 6. إعدادات النظام العامة (Global Settings)
- **`dbo.SITTEINGS`** (ملاحظة هجاء الكلمة بـ double T وحرفي EI):
  - `A_NAME` varchar(100) (اسم الشركة / الصيدلية)
  - `PHONE`, `MOBILE`, `FAX` (معلومات الاتصال)
  - **⚠️ تحذير حاسم للوكيل:** هذا الجدول **لا يحتوي على حقول معرّفات (IDs) ولا علاقة مفاتيح (FK)** مع جداول البنود أو الفواتير. هو عبارة عن **صف واحد فقط** يحتوي على الإعدادات والمعلومات العامة للشركة. لا يجوز عمل JOIN له مع الجداول الأخرى بناءً على معرّفات الأصناف أو الفواتير.

