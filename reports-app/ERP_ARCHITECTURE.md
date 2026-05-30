# ERP_ARCHITECTURE — دليل النظامين (Marketing2026 + InfinityRetailDB)

> **اقرأ هذا الملف أولاً** إذا كنت وكيلاً أو مطوّراً جديداً على المشروع.  
> التطبيق **reports-app** (Tauri + React + Rust) يدعم نظامين ERP على SQL Server ويختار المناسب تلقائياً.

---

## 1. نظرة عامة

| ERP | قاعدة البيانات النموذجية | المخطط | ملف الوكيل | DDL |
|-----|--------------------------|--------|------------|-----|
| **Marketing2026** | `Marketing2026` | `dbo.*` | `AGENT_Marketing2026.md` | `Full_Marketing_Database_DDL.sql` |
| **InfinityRetailDB** | `InfinityRetailDB` | `Inventory`, `SALES`, `Purchase`, `MyCompany` | `AGENT_InfinityRetailDB.md` | `InfinityRetailDB_DDL.sql` |

**قاعدة ذهبية:** لا تخلط جداول النظامين — `dbo.ITEMS` لا يوجد في Infinity، و`Inventory.Data_Products` لا يوجد في Marketing.

---

## 2. اكتشاف ERP (`src-tauri/src/erp_profile.rs`)

```
1) EXISTS Inventory.Data_Products  → InfinityRetailDb
2) EXISTS MyCompany.Config_Branchs → InfinityRetailDb
3) اسم DB يحتوي infinity/infinit   → InfinityRetailDb
4) EXISTS dbo.ITEMS                → Marketing2026
5) اسم DB يحتوي marketing          → Marketing2026
6) غير ذلك                         → Unknown (يُعامل كـ Marketing2026 في الأنماط)
```

- **`detect_erp_kind(conn)`** — عند الاتصال الأول
- **`resolve_erp_kind(state, conn)`** — يعيد الاكتشاف في كل طلب (لا يعتمد على كاش قديم)
- **`get_erp_kind`** — Tauri command → `"marketing2026"` | `"infinity_retail_db"` | `"unknown"`
- **`set_active_connection`** — يحفظ الاتصال + `erp_kind` في `AppState`

---

## 3. محوّلات ERP (`src-tauri/src/erp_adapters.rs`)

| الدالة | Marketing | Infinity |
|--------|-----------|----------|
| `fetch_business_profile` | `dbo.SITTEINGS` | `MyCompany.Config_Branchs` (+ fallbacks) |
| `fetch_receipt_business` | SITTEINGS | Config_Branchs |
| `infinity_product_search_sql` | — | autocomplete تقارير |
| `infinity_product_mentions_sql` | — | @ في الشات |
| `infinity_pos_product_sql` | — | فاتورة سريعة POS |
| `infinity_product_comprehensive_sql` | — | تقرير البحث عن منتج |
| `infinity_last_supplier_price_sql` | — | آخر سعر مورد |
| `cancelled_invoices_sql` | SALE_INVOICE + BUY_INVOICE | Data_SalesInvoices + Data_PurchaseInvoices |
| `resolve_search_report_sql` | قالب Marketing + `{{SEARCH_CONDITION}}` | يستبدل قوالب Marketing تلقائياً |

---

## 4. الوكيل الذكي — تحميل التعاليم

| المكوّن | السلوك |
|---------|--------|
| `load_agent_patterns(erp)` | Marketing → `AGENT_Marketing2026.md` ، Infinity → `AGENT_InfinityRetailDB.md` |
| `search_query_patterns_local(keywords, erp)` | بحث في الملف المناسب فقط |
| `run_query_pattern(..., erp)` | ينفّذ SQL من النمط المناسب |
| `domain_critical_facts(erp)` | حقائق مختصرة في system prompt |
| `get_product_schema()` | ✅ ERP-aware — `PRODUCT_SCHEMA.md` أو `INFINITY_PRODUCT_SCHEMA.md` |
| `get_database_views()` | ✅ ERP-aware — `DATABASE_VIEWS.md` أو `INFINITY_DATABASE_VIEWS.md` |

ملفات مُضمّنة في البناء (`tauri.conf.json` → `resources`):
- `AGENT_Marketing2026.md`
- `AGENT_InfinityRetailDB.md`

---

## 5. أقسام التطبيق — دعم ERP

| القسم | الملف | Marketing | Infinity |
|-------|------|-----------|----------|
| شات AI + @منتج | `ai-assistant-interface.tsx` | ✅ | ✅ |
| بحث منتج (تقارير) | `generic-report-page.tsx` | ✅ | ✅ |
| آخر سعر مورد | `supplier-price-page.tsx` | ✅ (Supabase SQL) | ✅ (محوّل تلقائي) |
| فاتورة سريعة POS | `quick-sale-addon.tsx` | ✅ | ✅ |
| فواتير ملغاة | `cancelled-invoices-addon.tsx` | ✅ | ✅ |
| بيانات المنشأة | `settings-page.tsx` | SITTEINGS | Config_Branchs |
| جدولة / Supabase reports | `scheduler-page.tsx` | ✅ | ⚠️ قوالب Supabase قد تكون Marketing-only |

---

## 6. تسميات أعمدة التقارير (إلزامية — لا تخلط بين النظامين)

### تقرير «البحث عن تفاصيل المنتج» — Marketing2026

| حقل DB | تسمية العمود العربية |
|--------|----------------------|
| ITEM_MODEL | الكود |
| ITEM_NAME | اسم المنتج |
| UNIT_DISC | وحدة البيع |
| BARCODE | الباركود |
| PRICE1 | سعر البيع |
| PRICE2 | سعر 2 |
| PUBLIC_PRICE | سعر الجمهور |
| LAST_COST | آخر تكلفة |
| SUM(ITEMS_SUB.QTY) | رصيد المخزون |
| آخر BUY_INVOICE | آخر مورد |
| UPDATE_DATE | تاريخ تحديث السعر |

### تقرير «البحث عن تفاصيل المنتج» — InfinityRetailDB

| حقل DB | تسمية العمود العربية |
|--------|----------------------|
| ProductCode | الكود |
| ProductName | اسم المنتج |
| UOMName | وحدة القياس |
| ProductBarcode | الباركود |
| UomPrice1 | السعر |
| UomPrice2 | سعر 2 |
| UomPrice4 | سعر 4 |
| UomLastCost | آخر تكلفة |
| StockOnHand | الكمية المتاحة |
| آخر Purchase Invoice | آخر مورد |
| ModifiedDate | تاريخ التعديل |

> **Infinity لا يستخدم «سعر الجمهور»** — استخدم **سعر 4** (`UomPrice4`).

---

## 7. جدول ترجمة سريع (PDF/Excel/شات)

### Marketing2026

```
ITEM_NAME→اسم المنتج  ITEM_MODEL→الكود  QTY→الكمية  PRICE→السعر
LAST_COST→آخر تكلفة  S_DATE→تاريخ البيع  CUST_NAME→اسم العميل
FULL_NAME→اسم الموظف  STORE_NAME→المخزن
```

### InfinityRetailDB

```
ProductName→اسم المنتج  ProductCode→الكود  StockOnHand→الكمية
UomPrice1→السعر  ProductBarcode→الباركود  UOMName→وحدة القياس
SalesInvoiceDate→تاريخ البيع  CustomerName→العميل
CreatedByUserName→الموظف  BranchName→الفرع  SupplierName→المورد
```

---

## 8. Tauri commands ذات صلة بـ ERP

```
set_active_connection(conn)     → يكتشف ERP ويُرجع الاسم العربي
get_erp_kind()                  → marketing2026 | infinity_retail_db
get_business_profile()          → BusinessProfile + erp_kind + erp_label
search_product_mentions(query)  → AppState + erp
search_products(conn, query)    → erp-aware
execute_search_report(conn, sqlTemplate, searchTerm) → erp-aware
list_cancelled_invoices(date)   → erp-aware
search_pos_products / print_pos_receipt → erp-aware
```

---

## 9. خريطة ملفات التوثيق

| الملف | الغرض | ERP |
|-------|--------|-----|
| **`ERP_ARCHITECTURE.md`** | **هذا الملف — نقطة البداية** | كلاهما |
| `AGENTS.md` | دليل تطوير + وكلاء Cursor | كلاهما |
| `AGENT_Marketing2026.md` | تعاليم + أنماط SQL | Marketing |
| `AGENT_InfinityRetailDB.md` | تعاليم + أنماط SQL | Infinity |
| `QUERY_PATTERNS.md` | legacy — يُشير إلى Marketing | Marketing |
| `PRODUCT_SCHEMA.md` | أعمدة منتجات | Marketing |
| `INFINITY_PRODUCT_SCHEMA.md` | أعمدة منتجات | Infinity |
| `DATABASE_NOTES.md` | اكتشافات تشغيلية | Marketing (+ مقدمة) |
| `INFINITY_DATABASE_NOTES.md` | اكتشافات Infinity | Infinity |
| `DATABASE_VIEWS.md` | Views وربط | Marketing |
| `INFINITY_DATABASE_VIEWS.md` | Views Infinity | Infinity |
| `AI_SYSTEM_PROMPT.md` | ملخص system prompt (Marketing-heavy) | مرجع |

---

## 10. قواعد للوكلاء / النماذج الجديدة

1. **حدّد ERP أولاً** — من `get_erp_kind` أو من سياق المستخدم (اسم DB).
2. **اقرأ ملف AGENT_* المناسب** قبل كتابة SQL.
3. **لا تعدّل `generic-report-page.tsx`** لإضافة SQL Infinity — ضعه في `erp_adapters.rs`.
4. **عند إضافة تقرير جديد:** أضف محوّل في `erp_adapters` + تسميات عربية منفصلة لكل ERP.
5. **حدّث هذا الملف + AGENT_* + INFINITY_* / DATABASE_* عند كل اكتشاف schema جديد.**
6. **`cargo check`** في `src-tauri` بعد أي تعديل Rust.
7. **لا ت commit** إلا بطلب المستخدم.

---

## 11. ما لم يُكتمل بعد (حالة 2026-05)

- [x] RAG / Supabase `documents` لـ Infinity DDL — `reembed_ddl.py --erp infinity --merge`
- [x] تقارير Supabase / Telegram — `finalize_supabase_report_sql`
- [x] `get_product_schema` / `get_database_views` — ERP-aware في Rust
- [x] `plan_complex_query` — Purchase.* على Infinity
- [x] Telegram + Desktop — prompts و `search_schema(erp)` ERP-aware
- [ ] مزامنة منتجات → Supabase للتطبيق العام
