# توثيق ذاكرة الوكيل السريعة وتقليل التوكنز

هذا الملف يشرح ما تم تنفيذه في تطبيق التقارير، والغرض منه، وبنية جداول

```text
Supabase
```

والتغييرات في الكود. الهدف أن يستطيع أي وكيل آخر بعدي فهم النظام وتطويره بدون إعادة اكتشاف نفس الأشياء.

---

## الهدف

كانت المشكلة أن الوكيل يستهلك توكنز كثيرة لأنه في كل سؤال بيانات كان يرسل تعليمات طويلة للنموذج، ثم يترك النموذج يختار الأداة، وأحيانًا يدخل في استكشاف حر ويكتب

```text
SQL
```

على جداول غير صحيحة. هذا تسبب في:

- استدعاءات متعددة للنموذج لنفس السؤال.
- استهلاك زائد للتوكنز.
- تأخير في الرد.
- محاولات على جداول غير موجودة مثل:

```text
BILLS
BILL_DTL
M_SALE_INVOICE
```

الحل الذي طبقناه هو إنشاء مسار سريع:

```text
user message -> Supabase fast recipe -> run_query_pattern -> model summarizes only
```

أي أن التطبيق يحاول معرفة النمط المناسب من قاعدة البيانات قبل أن يطلب من النموذج التفكير.

---

## المشروع

المشروع المستخدم في قاعدة البيانات:

```text
delivery-app
```

معرف المشروع:

```text
nsgmhijtaaenpqxxgjds
```

حالة المشروع عند الفحص:

```text
ACTIVE_HEALTHY
```

---

## قاعدة الفكرة

النموذج لا يجب أن يتعلم كل مرة من الصفر. الأنماط المعروفة في التطبيق لها:

```text
pattern_id
triggers
tool_args_template
```

لذلك خزنّا هذه المعلومات في

```text
Supabase
```

داخل جدول وصفات. عند وصول سؤال جديد:

1. التطبيق يرسل نص السؤال إلى دالة في قاعدة البيانات.
2. الدالة تبحث في الوصفات المحفوظة.
3. إذا وجدت تطابقًا كافيًا، يرجع لها:

```text
pattern_id
tool_name
tool_args_template
score
slots
```

4. التطبيق ينفذ:

```text
run_query_pattern
```

5. ثم يطلب من النموذج تلخيص النتيجة فقط، بدون أدوات.

---

## الجداول الجديدة

### جدول وصفات الأدوات

```text
public.agent_tool_recipes
```

الغرض: تخزين ربط مباشر بين نية المستخدم والأداة المناسبة.

الأعمدة:

| العمود | النوع | الغرض |
|---|---|---|
| `id` | `bigint` | رقم داخلي للوصفة |
| `erp_kind` | `text` | نوع النظام: `Marketing2026` أو `InfinityRetailDB` |
| `intent_key` | `text` | وصف مختصر ومطبع لنية المستخدم |
| `title_ar` | `text` | اسم عربي للوصفة |
| `pattern_id` | `text` | معرف النمط المستخدم في التطبيق |
| `tool_name` | `text` | غالبًا `run_query_pattern` |
| `trigger_phrases` | `text[]` | عبارات تطابق سؤال المستخدم |
| `slots` | `jsonb` | شروط إضافية مثل احتياج اسم منتج |
| `tool_args_template` | `jsonb` | قالب مدخلات الأداة |
| `response_hint` | `text` | إرشاد مختصر للوكيل |
| `confidence` | `numeric` | ثقة الوصفة |
| `success_count` | `integer` | عدد النجاحات |
| `failure_count` | `integer` | عدد الإخفاقات |
| `avg_prompt_tokens` | `integer` | متوسط توكن الإدخال عند توفره |
| `avg_completion_tokens` | `integer` | متوسط توكن الإخراج عند توفره |
| `last_used_at` | `timestamptz` | آخر استخدام للوصفة |
| `is_active` | `boolean` | تفعيل أو تعطيل الوصفة |
| `created_at` | `timestamptz` | وقت الإنشاء |
| `updated_at` | `timestamptz` | وقت آخر تعديل |

مثال مهم:

```json
{
  "erp_kind": "Marketing2026",
  "pattern_id": "sales_last_day_employee",
  "tool_name": "run_query_pattern",
  "tool_args_template": {
    "pattern_id": "sales_last_day_employee"
  },
  "slots": {
    "requires_product_filter": false
  }
}
```

### جدول سجلات تشغيل الوصفات

```text
public.agent_tool_runs
```

الغرض: معرفة هل استُخدمت الذاكرة السريعة فعلًا، وهل نجحت أو فشلت.

الأعمدة:

| العمود | النوع | الغرض |
|---|---|---|
| `id` | `bigint` | رقم السجل |
| `recipe_id` | `bigint` | الوصفة المستخدمة |
| `request_id` | `text` | معرف طلب المحادثة |
| `erp_kind` | `text` | نوع النظام |
| `message_fingerprint` | `text` | بصمة السؤال بعد التطبيع |
| `message_preview` | `text` | جزء من رسالة المستخدم |
| `tool_name` | `text` | الأداة المنفذة |
| `pattern_id` | `text` | النمط المنفذ |
| `success` | `boolean` | هل نجح التنفيذ |
| `prompt_tokens` | `integer` | توكن الإدخال إن توفر |
| `completion_tokens` | `integer` | توكن الإخراج إن توفر |
| `total_tokens` | `integer` | الإجمالي إن توفر |
| `error_message` | `text` | رسالة الخطأ إن وجدت |
| `created_at` | `timestamptz` | وقت السجل |

إذا أردت التأكد أن الذاكرة اشتغلت:

```sql
select *
from public.agent_tool_runs
order by created_at desc
limit 20;
```

---

## الجداول الموجودة سابقًا والمستخدمة

### حقائق قاعدة البيانات المشتركة

```text
public.db_facts
```

الغرض: تخزين حقائق ثابتة عن الجداول والأعمدة والعلاقات.

لا تخزن فيها بيانات تشغيلية متغيرة مثل الأرصدة أو أرقام المبيعات.

الأعمدة المهمة:

| العمود | الغرض |
|---|---|
| `content` | نص الحقيقة |
| `category` | مثل `db_schema` أو `db_join` |
| `fingerprint` | منع التكرار |
| `embedding` | بحث دلالي |

### ذاكرة المستخدم الخاصة

```text
public.user_memories
```

الغرض: تفضيلات المستخدم أو ملاحظات خاصة مرتبطة بالتوكن.

الأعمدة المهمة:

| العمود | الغرض |
|---|---|
| `token_hash` | عزل ذاكرة المستخدم |
| `content` | محتوى الذاكرة |
| `category` | مثل `preference` |
| `fingerprint` | منع التكرار |
| `embedding` | بحث دلالي |

### المحادثات السحابية

```text
public.user_chats
```

الغرض: حفظ المحادثات بين الأجهزة.

الأعمدة المهمة:

| العمود | الغرض |
|---|---|
| `token_hash` | عزل مستخدم التطبيق |
| `chat_id` | معرف المحادثة |
| `title` | عنوان المحادثة |
| `messages` | الرسائل بصيغة `jsonb` |

### أخطاء الوكيل

```text
public.agent_errors
```

الغرض: تسجيل أخطاء الوكيل في الخلفية بدون إزعاج المستخدم.

الأعمدة:

| العمود | الغرض |
|---|---|
| `erp_kind` | نوع النظام |
| `tool_name` | الأداة |
| `error_msg` | الخطأ |
| `sql_text` | الاستعلام إن وجد |
| `extra` | تفاصيل إضافية |

---

## الدوال في قاعدة البيانات

### دوال المسار السريع

```text
public.get_agent_tool_recipes
```

المدخلات:

```text
p_token text
p_erp_kind text
p_user_message text
p_limit integer default 3
```

النتيجة:

```text
id
intent_key
title_ar
pattern_id
tool_name
tool_args_template
slots
response_hint
confidence
success_count
failure_count
score
```

هذه هي الدالة التي يستدعيها التطبيق قبل النموذج.

---

```text
public.record_agent_tool_result
```

الغرض: تسجيل نتيجة استخدام الوصفة.

تُحدّث:

```text
success_count
failure_count
last_used_at
avg_prompt_tokens
avg_completion_tokens
```

---

```text
public.upsert_agent_tool_recipe
```

الغرض: إضافة أو تحديث وصفة من التطبيق أو من وكيل لاحق.

لا تستخدمها بدون توكن تطبيق صحيح.

---

```text
public.agent_token_is_valid
```

الغرض: التحقق من أن توكن التطبيق موجود ونشط في:

```text
public.app_access
```

---

```text
public.agent_normalize_text
```

الغرض: توحيد النص قبل المطابقة:

- تحويل إلى أحرف صغيرة.
- إزالة تكرار المسافات.
- قص الفراغات.

---

## الأنماط المزروعة في الذاكرة

آخر حالة موثقة:

```text
Marketing2026: 13 recipes
InfinityRetailDB: 7 recipes
```

أمثلة:

| النظام | النمط | يحتاج منتج؟ |
|---|---|---|
| `Marketing2026` | `expiry_report` | لا |
| `Marketing2026` | `last_purchase_price` | نعم |
| `Marketing2026` | `top_sellers` | لا |
| `Marketing2026` | `monthly_expenses` | لا |
| `Marketing2026` | `supplier_price_compare` | نعم |
| `Marketing2026` | `shortage_supplier` | لا |
| `Marketing2026` | `employee_ranking` | لا |
| `Marketing2026` | `employee_debts` | لا |
| `Marketing2026` | `near_expiry_sales_hero` | لا |
| `Marketing2026` | `customer_debts` | لا |
| `Marketing2026` | `supplier_debts` | لا |
| `Marketing2026` | `sales_last_day_employee` | لا |
| `Marketing2026` | `sales_daily_employee` | لا |
| `InfinityRetailDB` | `expiry_report` | لا |
| `InfinityRetailDB` | `last_purchase_price` | نعم |
| `InfinityRetailDB` | `top_sellers` | لا |
| `InfinityRetailDB` | `supplier_price_compare` | نعم |
| `InfinityRetailDB` | `shortage_supplier` | لا |
| `InfinityRetailDB` | `sales_last_day_employee` | لا |
| `InfinityRetailDB` | `sales_daily_employee` | لا |

ملاحظة مهمة: أي نمط موجود فقط في ملف التعليمات الطويل وليس مسجلًا في:

```text
pattern_catalog.rs
```

لن يدخل المسار السريع تلقائيًا حتى تزرع له وصفة في:

```text
agent_tool_recipes
```

---

## تغييرات الكود

### ملف ذاكرة الوكيل

```text
src-tauri/src/agent_memory.rs
```

أضيفت:

```text
AgentToolRecipe
fetch_agent_tool_recipes
record_agent_tool_result
```

الغرض:

- جلب وصفة مناسبة من قاعدة البيانات.
- تسجيل نجاح أو فشل استخدامها.
- قراءة `slots` مثل:

```json
{
  "requires_product_filter": true
}
```

إذا كانت الوصفة تحتاج منتجًا ولا يوجد:

```text
product_filter
```

يتم تخطيها ولا تنفذ عشوائيًا.

---

### ملف الوكيل الرئيسي

```text
src-tauri/src/ai_agent.rs
```

أهم التغييرات:

1. النموذج الافتراضي:

```text
minimax/minimax-m3
```

2. بناء النظام السريع صار يستخدم:

```text
pattern_catalog::build_executor_system_prompt
```

بدل حقن ملف التعليمات الطويل كاملًا.

3. قبل أول طلب إلى النموذج، ينفذ التطبيق:

```text
fetch_agent_tool_recipes
```

4. إذا وجد وصفة بدرجة:

```text
score >= 0.30
```

وكانت الأداة:

```text
run_query_pattern
```

ينفذها مباشرة.

5. بعد نجاح المسار السريع، أول طلب للنموذج يكون:

```text
tool_choice: none
```

حتى لا يكرر أدوات أو يدخل دوامة.

6. أضيف حارس يمنع:

```text
execute_raw_sql
```

في الوضع غير المتقدم.

الرسالة عند الرفض:

```text
الوضع السريع لا يسمح بـ execute_raw_sql. استخدم run_query_pattern(pattern_id=...) فقط.
```

---

### كتالوج الأنماط

```text
src-tauri/src/pattern_catalog.rs
```

أهم تغيير:

تم حذف:

```text
execute_raw_sql
```

من أدوات وضع التنفيذ السريع.

الآن الوضع السريع يجب أن يعرض:

```text
4 tools
```

بدل:

```text
5 tools
```

الأدوات المتوقعة:

```text
get_current_datetime
list_available_patterns
run_query_pattern
export_last_result
```

---

### واجهة المحادثة

```text
src/components/ui/ai-assistant-interface.tsx
```

أضيف عرض التوكن لكل رسالة مساعد.

يعرض:

```text
model
tokens
in
out
source
```

تم تعديل العرض حتى يظهر الرقم خامًا:

```text
tokens: 5359
```

وليس كرقم يبدو عشريًا:

```text
5.359
```

---

## استخدام أرقام التوكن

التطبيق يستقبل أرقام التوكن من استجابة:

```text
OpenRouter Chat Completion
```

من حقل:

```text
usage
```

ثم يحاول جلب تفاصيل الجيل من:

```text
GET https://openrouter.ai/api/v1/generation?id=<generation_id>
```

إذا نجح، يظهر المصدر:

```text
openrouter_generation
```

إذا لم ينجح، يظهر:

```text
chat_completion_usage
```

ملاحظة: أثناء التجربة ظهرت رسائل:

```text
generation usage unavailable
```

هذا لا يعني أن حساب التوكن يدوي. يعني فقط أن endpoint الجيل لم يرجع التفاصيل في تلك اللحظة، فيستخدم التطبيق حقل:

```text
usage
```

الموجود في استجابة الشركة نفسها.

---

## السجلات المهمة

عند نجاح الذاكرة السريعة يجب أن ترى:

```text
[agent_memory] recipe candidate id=... tool=run_query_pattern score=...
[agent_memory] fast recipe hit id=... tool=run_query_pattern pattern=...
```

إذا رأيت:

```text
[agent_memory] recipe skipped
```

فمعناه أن الوصفة موجودة لكن لم تُقبل. راجع:

- قيمة `score`.
- هل النمط يحتاج `product_filter`.
- هل `trigger_phrases` ناقصة.

إذا دخل النموذج إلى:

```text
execute_raw_sql
```

في الوضع غير المتقدم، فهذا خطأ يجب ألا يحدث بعد التعديل الأخير.

---

## طريقة إضافة نمط جديد مستقبلًا

1. أضف النمط في:

```text
src-tauri/src/pattern_catalog.rs
```

2. أضف SQL أو قسم النمط في:

```text
AGENT_Marketing2026.md
AGENT_InfinityRetailDB.md
```

حسب النظام.

3. ازرع وصفة في:

```text
public.agent_tool_recipes
```

أقل قالب:

```sql
insert into public.agent_tool_recipes (
  erp_kind,
  intent_key,
  title_ar,
  pattern_id,
  tool_name,
  trigger_phrases,
  slots,
  tool_args_template,
  response_hint,
  confidence
) values (
  'Marketing2026',
  public.agent_normalize_text('كلمات تصف نية المستخدم'),
  'اسم عربي مختصر',
  'pattern_id_here',
  'run_query_pattern',
  array['عبارة 1', 'عبارة 2'],
  '{"requires_product_filter": false}'::jsonb,
  '{"pattern_id": "pattern_id_here"}'::jsonb,
  'نفذ النمط مباشرة ثم لخص النتيجة فقط.',
  0.94
);
```

إذا يحتاج اسم منتج:

```json
{
  "requires_product_filter": true
}
```

4. شغل:

```powershell
cargo test --lib pattern_catalog
cargo check
npm run build
```

---

## قواعد مهمة للوكيل التالي

- لا تعيد إدخال ملف التعليمات الطويل في النظام السريع.
- لا ترجع `execute_raw_sql` إلى أدوات الوضع السريع.
- لا تخفض العتبة أقل من `0.30` بدون سبب واضح.
- إذا زادت المطابقات الخاطئة، حسّن `trigger_phrases` بدل فتح SQL الحر.
- الأنماط التي تحتاج منتجًا يجب أن تبقى:

```json
{
  "requires_product_filter": true
}
```

- لا تحفظ أسرار أو مفاتيح في التوثيق.
- لا ترفع توكنات أو مفاتيح Supabase إلى GitHub.

---

## أوامر التحقق التي نجحت بعد التغييرات

```powershell
cargo check
cargo test --lib pattern_catalog
npm run build
```

كلها نجحت بعد آخر تعديل.

---

## مشاكل معروفة وملاحظات

### المطابقة العربية

دالة المطابقة تعتمد على:

```text
pg_trgm similarity
```

مع عبارات:

```text
trigger_phrases
```

لذلك جودة الذاكرة تعتمد على جودة العبارات. إذا سؤال معروف لم يضرب الذاكرة، أضف عبارته إلى الوصفة بدل ترك النموذج يتعلمها لاحقًا.

### Endpoint الجيل في OpenRouter

قد يرجع غير متاح مباشرة بعد الطلب. لذلك التطبيق يعمل fallback إلى:

```text
usage
```

من استجابة المحادثة.

### RLS

أثناء الفحص ظهر أن بعض الجداول القديمة عندها:

```text
RLS disabled
```

خصوصًا جداول التقارير والوثائق. لم يتم تفعيلها تلقائيًا حتى لا ينكسر التطبيق. أي وكيل يعمل على الأمان يجب أن يضع سياسات مناسبة قبل تفعيلها.

