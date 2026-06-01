# دليل تطوير أنماط الاستعلام — al-tabi

مرجع كامل لأي وكيل AI يعمل على هذا المشروع.

---

## 1. معمارية النظام (ملخص سريع)

```
المستخدم → واجهة React → ask_local_ai (Rust)
  → resolve_app_secrets (Supabase Vault أولاً ← محلي احتياطي)
  → build_fast_system_prompt (يحشو AGENT_*.md كاملاً)
  → call_groq_api (OpenRouter: gemini-3.5-flash)
  → النموذج يقرأ الأنماط → ينسخ SQL → execute_raw_sql → النتائج
```

**الملفات الرئيسية:**
| الملف | الدور |
|-------|-------|
| `AGENT_Marketing2026.md` | SQL جاهز — يُحشى في system prompt |
| `AGENT_InfinityRetailDB.md` | نفس الشيء لـ Infinity ERP |
| `src-tauri/src/pattern_catalog.rs` | كتالوج الأنماط (triggers + metadata) |
| `src-tauri/src/ai_agent.rs` | الوكيل + system prompt + call_groq_api |
| `src-tauri/src/supabase_config.rs` | جلب مفتاح OpenRouter من Supabase Vault |

---

## 2. كيف تُنشئ استعلاماً جديداً (خطوة بخطوة)

### الخطوة 1: ادرس الـ DDL

```powershell
# اتصل بالقاعدة
sqlcmd -S localhost -d Marketing2026 -E

# استكشف الجداول
SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES ORDER BY TABLE_NAME;

# استكشف أعمدة جدول معين
SELECT COLUMN_NAME, DATA_TYPE FROM INFORMATION_SCHEMA.COLUMNS 
WHERE TABLE_NAME='ITEMS' ORDER BY ORDINAL_POSITION;

# تحقق من البيانات
SELECT TOP 5 * FROM dbo.ITEMS;
```

### الخطوة 2: اكتب واختبر SQL في sqlcmd

**⚠️ قاعدة ذهبية:** لا تضف أي استعلام للنمط قبل اختباره في sqlcmd.

```powershell
# اختبر بالعربي (لرؤية الأسماء العربية)
powershell -Command "& { [Console]::OutputEncoding = [Text.Encoding]::UTF8; sqlcmd -S localhost -d Marketing2026 -E -f 65001 -Q 'SELECT ...' }"
```

**نقاط مهمة أثناء الاختبار:**
- `PRICE > 0` دائماً عند BUY_ITEMS (توجد سجلات بسعر 0)
- `ITEM_INVISIBLE = 0` للمنتجات النشطة فقط
- `CUST_EMP=1` لا يكفي للموظفين — اربط أيضاً بـ `USERS.FULL_NAME`
- `SALE_ITEMS` لا يحتوي `S_DATE` — لازم JOIN مع `SALE_INVOICE`
- `BALANCE_C` فارغ — لا تستخدمه للديون
- `GIVE.EXPENCES_ID=0` = دفعات موردين. `>0` = مصاريف تشغيلية
- `TOP 1 ORDER BY ITEM_NAME` قد يختار منتجاً خاطئاً — رتّب بـ `COUNT(BUY_ITEMS) DESC`
- `CROSS JOIN` يُعطي 0 صف إذا أي CTE فارغ

### الخطوة 3: أضف النمط لـ AGENT_Marketing2026.md

```markdown
## PATTERN: اسم-النمط-بالعربي
TRIGGERS: كلمة1, كلمة2, كلمة3, english trigger
TABLES: TABLE1, TABLE2
NOTES:
  - وصف مختصر لما يفعله النمط.
  - أي ملاحظات خاصة (فلاتر، حالات حافة).
  - مُختبَر على Marketing2026 بـ sqlcmd ✓
---

```sql
-- وصف الاستعلام
;WITH ...
SELECT ...
```
```

**قواعد صارمة لملف AGENT_*.md:**
- **اختصر** — كل سطر إضافي = توكنز إضافية على المستخدم
- لا تكرر نمطاً موجوداً
- لا تكتب تعليمات عامة — فقط SQL + NOTES مختصرة
- الاستعلامات تبدأ بـ `;WITH` أو `SELECT` — لا `DECLARE`
- `{{PRODUCT_FILTER}}` = placeholder يُستبدل باسم المنتج

### الخطوة 4: أضف للكتالوج في pattern_catalog.rs

```rust
PatternEntry {
    id: "pattern_id_english",
    name_ar: "الاسم بالعربي",
    section_marketing: "اسم-النمط-بالعربي",  // يطابق ## PATTERN:
    section_infinity: "اسم-النمط-بالعربي",
    marketing: true,    // متاح على Marketing2026؟
    infinity: false,    // متاح على InfinityRetailDB؟
    needs_product_filter: false,
    triggers: &["trigger1", "trigger2"],
},
```

### الخطوة 5: أضف تيست

```rust
#[test]
fn resolve_my_pattern() {
    let p = resolve_pattern_id("trigger1", ErpKind::Marketing2026);
    assert_eq!(p.map(|x| x.id), Some("pattern_id_english"));
}
```

### الخطوة 6: ابنِ واختبر

```powershell
cd reports-app/src-tauri
cargo test --lib pattern_catalog   # التيستات
cargo check                        # البناء
cd ..
npm run build                      # TypeScript
```

---

## 3. كيف تُعدّل system prompt الوكيل

**الملف:** `src-tauri/src/ai_agent.rs` → دالة `build_fast_system_prompt()`

```rust
fn build_fast_system_prompt(...) -> String {
    let agent_md = crate::erp_profile::load_agent_patterns(erp);
    format!(
        "أنت منفّذ تقارير {}. كل SQL جاهز أدناه — انسخه ونفّذه فوراً.\n\n\
        قواعد:\n\
        1. اختر النمط المناسب...\n\n\
        {}\n\n{}",
        erp.display_name_ar(),
        agent_md,      // ← ملف AGENT_*.md بالكامل
        product_note
    )
}
```

**⚠️ حجم system prompt:**
- ملف AGENT_*.md يُحشى كاملاً في كل طلب
- كل 1KB ≈ 250 توكن → ملف 40KB ≈ 10,000 توكن/طلب
- **اختصر قدر الإمكان** — لا تعليمات مكررة، لا أنماط غير مستخدمة

---

## 4. مفتاح OpenRouter API

**المصدر:** Supabase Vault (أولاً) ← محلي (احتياطي)

**الترتيب في الكود:**
```
supabase_config.rs → resolve_app_secrets()
  1. fetch_secrets_from_supabase() → RPC get_app_secrets
  2. load_legacy_secrets_from_store() → settings.json المحلي
```

**تحديث المفتاح في Supabase:**
```powershell
$body = '{"p_access_token":"tPg1lWttWj71HBpxPYHDIgBSkJMEjHlEG9CpZbM4N_k","p_secrets":{"openrouter_api_key":"sk-or-v1-..."}}'
Invoke-WebRequest -Uri "https://nsgmhijtaaenpqxxgjds.supabase.co/rest/v1/rpc/save_app_secrets" ...
```

**⚠️ لا تكرر البادئة** `sk-or-v1-` — حدث خطأ سابقاً بسبب `sk-or-v1-sk-or-v1-...`

---

## 5. إصدار نسخة جديدة (Release)

```powershell
# 1. رقم الإصدار في 3 ملفات (متطابق)
#    package.json, tauri.conf.json, Cargo.toml

# 2. تحديث Cargo.lock
cd reports-app/src-tauri && cargo check && cd ../..

# 3. بناء TypeScript
cd reports-app && npm run build && cd ..

# 4. commit + push
git add <files>
git commit -m "Release vX.Y.Z: <summary>"
git push

# 5. tag يُشغّل GitHub Actions
git tag vX.Y.Z
git push origin vX.Y.Z

# 6. انتظر ~10-15 دقيقة ← Release يظهر على GitHub
```

---

## 6. Marketing2026 — خريطة الجداول الأساسية

| الجدول | الدور | ملاحظات |
|--------|-------|---------|
| `ITEMS` | منتجات | `ITEM_INVISIBLE=0` للنشط |
| `ITEMS_SUB` | مخزون | `QTY`, `CATEOGRY3`=صلاحية |
| `SALE_INVOICE` | فواتير بيع | `S_DATE`, `CUST_ID`, `USERS_ID` |
| `SALE_ITEMS` | بنود بيع | **لا يحتوي S_DATE** — JOIN مع SALE_INVOICE |
| `BUY_INVOICE` | فواتير شراء | `B_DATE`, `CUST_ID` |
| `BUY_ITEMS` | بنود شراء | `PRICE`, فلتر `PRICE>0` |
| `CUSTOMERS` | زبون/مورد/موظف | `CUST_CUSTOM=1`=زبون, `CUST_VENDOR=1`=مورد, `CUST_EMP=1`=موظف |
| `USERS` | مستخدمو النظام | `FULL_NAME` — ربطهم بـ CUSTOMERS عبر الاسم |
| `TAKE` | مقبوضات من الزبائن | `T_VALUE`, `T_DATE` |
| `GIVE` | مدفوعات | `EXPENCES_ID=0`=لموردين, `>0`=مصاريف |
| `EXPENCES` | أنواع المصروفات | 1=رواتب, 18=إيجار, 3=كهرباء, 17=أخرى |
| `R_S_INVOICE/ITEMS` | مردودات مبيعات | |
| `B_R_INVOICE/ITEMS` | مردودات مشتريات | |
| `BALANCE_EDIT` | تسويات | `BL_DEBIT - BL_CREDIT` |
| `SALARIES` | رواتب | غالباً فارغ — `BORROW_DISCOUNT`=خصم سلفة |
| `BALANCE_C` | ❌ فارغ | لا تستخدمه أبداً |

---

## 7. أخطاء شائعة وحلولها

| الخطأ | السبب | الحل |
|-------|-------|------|
| `User not found 401` | مفتاح OpenRouter ملغي أو مكرر البادئة | تحقق من Supabase Vault |
| النموذج يستدعي `list_available_patterns` بدل التنفيذ | ملف AGENT_*.md ضخم جداً | احذف الأنماط غير المستخدمة |
| `0 rows` على تاريخ موجود | النمط يستخدم `MAX(S_DATE)` بنافذة ضيقة | استخدم التاريخ الصريح |
| موظف واحد فقط يظهر | `CUST_EMP=1` لا يكفي | اربط بـ `USERS.FULL_NAME` أيضاً |
| `CROSS JOIN` يُرجع 0 صف | أحد CTEs فارغ | استخدم `LEFT JOIN` أو تحقق مسبقاً |
| `PRICE=0` في BUY_ITEMS | سجلات خاطئة | فلتر `BI.PRICE > 0` دائماً |
| `Cargo.lock out of date` على CI | نسيت `cargo check` | اعمل `cargo check` قبل commit |

---

## 8. الكتالوج الحالي (11 نمط)

| pattern_id | الوصف | product_filter |
|---|---|---|
| `expiry_report` | تقرير الصلاحية | لا |
| `last_purchase_price` | آخر سعر شراء + كمية + مورد | **نعم** |
| `top_sellers` | أكثر مبيعاً (4 أوضاع: أيام/شهر/سابق/توقعات) | لا |
| `monthly_expenses` | مصروفات شهرية (3 أوضاع) | لا |
| `supplier_price_compare` | مقارنة أسعار موردين | **نعم** |
| `shortage_supplier` | نواقص نشطة + مورد | لا |
| `employee_debts` | ديون وسلف الموظفين | لا |
| `customer_debts` | ديون الزبائن + آخر إيصال قبض | لا |
| `supplier_debts` | ديون الموردين + آخر إيصال صرف | لا |
| `sales_last_day_employee` | مبيعات آخر يوم لكل موظف | لا |
| `sales_daily_employee` | مبيعات يومية (يدعم تاريخ محدد) | لا |

---

## 9. قواعد ذهبية

1. **اختبر في sqlcmd أولاً** — دائماً قبل إضافة أي نمط
2. **اختصر** — كل توكن إضافي في AGENT_*.md = تكلفة على المستخدم
3. **لا تكرر** — نمط واحد لكل تقرير
4. **لا تخترع SQL** — النموذج ينسخ فقط مما في الملف
5. **Supabase Vault أولاً** — المفتاح المحلي احتياطي فقط
6. **`trim()` على كل شيء** — مفاتيح API، أسماء منتجات، فلاتر
7. **لا تنسَ `cargo test`** — قبل كل push
