# Repository Guidelines

## Project Purpose

تطبيق سطح مكتب (Windows) لشركة توزيع أدوية. يتصل بقاعدة بيانات MSSQL محلية (Marketing2026) ويُنفّذ تقارير SQL يولّدها وكيل AI ذكي. يُرسل النتائج عبر بوت Telegram كنص أو PDF أو Excel، ويعرض تقارير دورية مجدوَلة محلياً.

---

## Architecture Overview

```
reports-app/
├── src/                        # React 19 + TypeScript frontend
│   ├── App.tsx                 # Root: login → شاشات التطبيق الرئيسية
│   ├── components/ui/
│   │   ├── sql-login-page.tsx          # شاشة الاتصال بـ MSSQL
│   │   ├── ai-assistant-interface.tsx  # واجهة المحادثة مع الوكيل الذكي
│   │   ├── scheduler-page.tsx          # ⭐ التقارير المجدوَلة + الإشعارات
│   │   ├── generic-report-page.tsx     # تقرير بحث عام (صفحة البحث)
│   │   ├── settings-page.tsx           # إعدادات Telegram + API keys
│   │   ├── futuristic-nav.tsx          # شريط التنقل السفلي (6 تبويبات)
│   │   ├── supplier-price-page.tsx     # تقرير آخر سعر شراء (legacy)
│   │   └── button.tsx / input.tsx / checkbox.tsx / label.tsx
│   └── lib/utils.ts            # cn() helper (tailwind-merge + clsx)
└── src-tauri/src/
    ├── lib.rs                  # AppState، Tauri commands، MSSQL، تشفير، Scheduler init
    ├── scheduler.rs            # ⭐ نظام الجدولة: SharedScheduler + run_scheduler + persistence
    ├── telegram.rs             # بوت Telegram: polling + state machine + HTML formatter + RAG
    ├── ai_agent.rs             # وكيل AI: handle_with_groq + handle_with_groq_local + جميع أدوات الوكيل
    ├── pdf_generator.rs        # توليد PDF عربي (printpdf + reshaper + BiDi)
    ├── excel_generator.rs      # توليد Excel (.xlsx) عبر rust_xlsxwriter
    └── agent_tools.rs          # أدوات SQL المتقدمة (validate, patterns, favorites, export)
```

### Data Flow

1. **الوكيل الذكي (Desktop)** ← واجهة المحادثة → `ask_local_ai` → `handle_with_groq_local` → أدوات → MSSQL → نتيجة
2. **الجدولة التلقائية** ← `run_scheduler` (كل 5 ثواني) → SQL → PDF/Excel/Text → `report-notification` Tauri event → `scheduler-page.tsx`
3. **الإعدادات** ← مُشفَّرة بـ AES-256-GCM في `tauri-plugin-store` (`settings.json`)
4. **Telegram (نمط AI)** ← long-polling → `handle_with_groq` → RAG → OpenRouter → execute_raw_sql
5. **فتح الملفات** ← `open_local_file` command: PDF→المتصفح، Excel→Excel، غيره→التطبيق الافتراضي

### إكمال المنتجات في شات Desktop (`@`)

- في `ai-assistant-interface.tsx`: عند كتابة `@` تظهر قائمة منتجات من `dbo.ITEMS` عبر `search_product_mentions`.
- مع كل حرف بعد `@` يُصفَّى البحث (debounce 200ms). الاختيار يُدرج: `@اسم المنتج (كود)`.
- المستخدم يمكنه سؤال الوكيل عن عدة منتجات في رسالة واحدة بذكرها بهذا الشكل لتقليل أخطاء الأسماء.

### تجربة المستخدم في المحادثة (Chat UI)
- **جداول البيانات (Markdown Tables):** مغلفة بحاوية `overflow-x-auto` لتجنب كسر التصميم عند عرض جداول ضخمة، مع تنسيق متقدم للترويسة والخلايا.
- **حالة الأدوات الفورية (Real-time Tool Usage):** الواجهة تعرض الخطوات التي يتخذها الوكيل حالياً بنص صريح (مثلاً: "سأستخدم أداة الاستعلام (SQL)...") بدلاً من رسائل التحميل الجامدة، بناءً على حدث `tool-usage` المُرسل من `ai_agent.rs`.
- **الإكمال التلقائي (@):** القائمة المنسدلة مصممة كنافذة عائمة (Floating Menu) شفافة بتأثيرات حركية عبر Framer Motion لضمان تجربة عصرية.

---

## Build & Development Commands

```bash
# تشغيل بيئة التطوير (المنفذ 1420 يجب أن يكون فارغاً)
npm run tauri dev

# بناء الإنتاج
npm run tauri build

# التحقق من Rust فقط (أسرع)
cd src-tauri && cargo check

# بناء Rust
cd src-tauri && cargo build

# بناء الـ Frontend فقط
npm run build

# إيقاف التطبيق قبل إعادة البناء
taskkill /f /im reports-app.exe
```

---

## تبويبات التطبيق (App.tsx)

```
التقارير (0) → SchedulerPage   ← التقارير المجدوَلة + عداد تنازلي
البحث    (1) → GenericReportPage
تنبيهات (2) → SchedulerPage   ← نفس الصفحة — تبويب الإشعارات
الذكاء  (3) → AIAssistantInterface
المحفوظات(4) → Placeholder
الإعدادات(5) → SettingsPage
```

> ⚠️ **تبويبا التقارير والتنبيهات يعرضان نفس `SchedulerPage`** — الفرق أن Scheduler-Page تبدأ بتبويب "التقارير المجدوَلة" ثم "الإشعارات" داخلياً.

---

## ⭐ نظام الجدولة (`scheduler.rs` + `lib.rs`)

### الأنواع الأساسية

```rust
pub struct ScheduledReport {
    pub id: String,                     // hex timestamp + random
    pub name: String,                   // اسم قصير عربي
    pub description: String,
    pub sql_query: String,              // SELECT فقط
    pub report_title: String,           // عنوان التقرير في الملف/الإشعار
    pub report_type: String,            // "text" | "pdf" | "excel"
    pub columns: Vec<String>,           // أسماء الأعمدة بالعربية (MANDATORY)
    pub interval_seconds: u64,          // 10=ثواني، 60=دقيقة، 3600=ساعة، 86400=يوم
    pub next_run_unix: u64,
    pub last_run_unix: Option<u64>,
    pub created_at_unix: u64,
    pub is_active: bool,
}

pub struct ReportNotification {
    pub id: String,
    pub schedule_id: String,
    pub schedule_name: String,
    pub title: String,
    pub generated_at_unix: u64,
    pub report_type: String,            // "text" | "pdf" | "excel"
    pub text_content: Option<String>,   // للنوع text
    pub file_path: Option<String>,      // للأنواع pdf/excel
    pub is_read: bool,
}

pub type SharedScheduler = Arc<Mutex<SchedulerState>>;
```

### الاستمرارية

- **تخزين:** `%APPDATA%\com.dell.reports-app\schedules.json` + `notifications.json`
- **تحميل:** عند بدء التطبيق في `.setup()` — `scheduler::load_state(&data_dir)`
- **حفظ:** فور كل تغيير — `save_schedules()` / `save_notifications()`
- **الحد:** آخر 100 إشعار فقط (`truncate(100)`)

### حلقة الخلفية (`run_scheduler`)

```rust
// تنبض كل 5 ثواني
// تُنفَّذ التقارير المستحقة (is_active && next_run_unix <= now)
// بعد التنفيذ: next_run_unix = now + interval_seconds
// يُرسل Tauri event: app_handle.emit("report-notification", &notification)
```

### Tauri Commands للجدولة

| الأمر | الوصف |
|-------|-------|
| `get_scheduled_reports` | قائمة كل الجداول |
| `add_scheduled_report` | إضافة جدول جديد يدوياً |
| `delete_scheduled_report(id)` | حذف جدول |
| `toggle_scheduled_report(id, active)` | إيقاف/تفعيل مؤقت |
| `get_notifications` | قائمة الإشعارات (آخر 100) |
| `mark_notification_read(id)` | تعليم كمقروء |
| `clear_all_notifications` | مسح الكل |

### ملف التقرير المحفوظ

PDF و Excel تُحفظ في:
```
%APPDATA%\com.dell.reports-app\reports\report_{id}_{timestamp}.{ext}
```

---

## ⭐ أدوات الوكيل الذكي — قائمة كاملة

### أدوات مشتركة (Telegram + Desktop)

| الأداة | الوصف | المتى |
|--------|-------|-------|
| `search_query_patterns` | يبحث في QUERY_PATTERNS.md ويُعيد SQL مختبر | **أولاً** لأي استعلام معقد |
| `search_schema` | RAG في Supabase DDL | لجداول غير معروفة |
| `explore_local_schema` | INFORMATION_SCHEMA محلي | عند الحاجة لأعمدة دقيقة |
| `execute_raw_sql` | SELECT على MSSQL (حد 100 صف) | تنفيذ استعلام |
| `get_current_datetime` | التاريخ/الوقت بالعربية (UTC+2 ليبيا) | **أولاً** عند أي طلب يخص الوقت الحالي |
| `execute_report` | تقرير Supabase محدد بـ id | لتقارير محفوظة |
| `generate_pdf` | PDF من تقرير Supabase | طلب PDF لتقرير محفوظ |
| `generate_custom_pdf` | PDF من أعمدة+صفوف جاهزة | عندما البيانات في الذاكرة |
| `create_pdf_report` | PDF من SELECT | عند طلب PDF صريح |
| `generate_excel` | Excel من تقرير Supabase | طلب Excel لتقرير محفوظ |
| `generate_custom_excel` | Excel من أعمدة+صفوف جاهزة | عندما البيانات في الذاكرة |
| `create_excel_report` | Excel من SELECT | عند طلب Excel صريح |
| `schedule_report` | **⭐ جدولة تقرير متكرر** | عند ذكر: يومياً، كل ساعة، كل X دقائق |
| `list_scheduled_reports` | **⭐ عرض قائمة الجداول** | عند سؤال عن الجداول الموجودة |
| `delete_scheduled_report` | **⭐ حذف جدول** | عند طلب إيقاف/حذف جدول |
| `validate_sql` | فحص SELECT قبل التنفيذ | استعلام جديد أو معقد |
| `explain_sql` | شرح الاستعلام بالعربية | المستخدم يسأل «ماذا يفعل هذا SQL؟» |
| `get_table_sample` | `TOP N` من جدول `dbo.*` | شكل الجدول غير واضح |
| `run_query_pattern` | بحث + تنفيذ نمط (+ `product_filter` اختياري) | **أفضل من كتابة SQL معقد يدوياً** |
| `get_product_schema` | مرجع PRODUCT_SCHEMA.md (وحدات، أسعار، مخزون) | قبل تحليل منتج معقد |
| `get_database_views` | Views + قواعد JOIN (SALE_ITEMS_INVOICE_VIEW، SUM(QTY*PRICE)) | **قبل مبيعات موظف/يومية** |
| `plan_complex_query` | رسم خطة Mermaid + خطوات SQL | **دراسة منتج**، تحليل مركب |
| `execute_query_plan` | تنفيذ خطوات الخطة بالتتابع | بعد `plan_complex_query` |
| `compare_periods` | مقارنة مبيعات/مشتريات بين فترتين | «قارن شهر X بشهر Y» |
| `suggest_indexes` | اقتراح فهارس heuristic | استعلام بطيء |
| `save_favorite_query` | حفظ استعلام ناجح | بعد `execute_raw_sql` ناجح |
| `list_favorite_queries` | قائمة المحفوظة | إعادة استخدام استعلام |
| `export_last_result` | PDF/Excel لآخر نتيجة | بعد تنفيذ ناجح + طلب تصدير |

### أدوات حصرية للـ Desktop فقط

| الأداة | الوصف |
|--------|-------|
| `send_pdf_to_telegram` | إرسال PDF المحفوظ للبوت |
| `send_excel_to_telegram` | إرسال Excel المحفوظ للبوت |

### أداة `schedule_report` — المعاملات الكاملة

```json
{
  "name": "تقرير المبيعات اليومي",
  "description": "ملخص مبيعات كل يوم",
  "sql_query": "SELECT TOP 20 ...",
  "report_title": "تقرير مبيعات اليوم",
  "report_type": "text",            // "text" | "pdf" | "excel"
  "columns": ["اسم المنتج", "الكمية", "السعر"],  // MANDATORY — دائماً بالعربية
  "interval_seconds": 86400,        // 86400=يومي، 3600=ساعي، 300=5دقائق، 60=دقيقة
  "first_run_offset_seconds": 0     // 0=الآن، 3600=بعد ساعة
}
```

---

## قواعد الوكيل الذكي — أهم ما يجب تذكره

### 1. ترتيب اتخاذ القرار (Anthropic Pattern)

```xml
<thinking>
  1. ما الذي يطلبه المستخدم فعلاً؟
  2. هل يخص وقتاً/تاريخاً؟ → استدعِ get_current_datetime أولاً
  3. هل هو استعلام معقد؟ → run_query_pattern أو search_query_patterns أولاً
  3b. استعلام جديد؟ → validate_sql ثم execute_raw_sql
  4. هل يطلب جدولة؟ → استدعِ schedule_report
  5. هل تحتاج جداول غير معروفة؟ → search_schema
</thinking>
```

### 2. ترجمة أسماء الأعمدة (إلزامي)

**NEVER** تمرّر أسماء DB خام للتقارير. دائماً ترجم:

| DB Column | العربية |
|-----------|---------|
| `ITEM_NAME` | اسم المنتج |
| `ITEM_MODEL` | الكود |
| `QTY` | الكمية |
| `PRICE` | السعر |
| `LAST_COST` | آخر تكلفة |
| `AVER_COST` | متوسط التكلفة |
| `S_DATE` | تاريخ البيع |
| `B_DATE` | تاريخ الشراء |
| `CUST_NAME` | اسم العميل / المورد |
| `FULL_NAME` | اسم الموظف |
| `STORE_NAME` | المخزن |
| `G_VALUE` | المبلغ المدفوع |
| `T_VALUE` | المبلغ المحصَّل |

### 3. حماية قراءة-فقط

```rust
// ممنوع في execute_raw_sql:
INSERT, UPDATE, DELETE, DROP, ALTER, TRUNCATE, EXEC, GRANT, REVOKE

// ممنوع إضافةً في تقارير PDF:
MERGE, CREATE, BACKUP, RESTORE
```

### 4. استدعاء أداة واحدة في الدور

```
❌ لا تستدعي execute_raw_sql مرتين في نفس الدور
✅ انتظر النتيجة ثم قرر الخطوة التالية
```

### 5. مفتاح العملة

دائماً أضف `د.ل` لأي مبلغ مالي في الردود العربية.

---

## Tauri Commands — القائمة الكاملة

```rust
// اتصال
test_sql_connection(conn: SqlConnection) → ConnectionResult
execute_sql_query(conn, sql_query) → QueryResult
execute_search_report(conn, sql_template, search_term) → QueryResult
search_products(conn, query) → Vec<String>
set_active_connection(conn) → ()

// تشفير
encrypt_value(value) → String
decrypt_value(encrypted) → String

// Telegram
update_telegram_settings(app) → ()
test_telegram_bot(token, chat_id) → String

// AI
ask_local_ai(message, history, groq_key, ai_model, ...) → String

// ملفات
open_local_file(path) → ()  // PDF→browser، xlsx→Excel، else→default

// جدولة ⭐
get_scheduled_reports() → Vec<ScheduledReport>
add_scheduled_report(report) → ScheduledReport
delete_scheduled_report(id) → ()
toggle_scheduled_report(id, active) → ()
get_notifications() → Vec<ReportNotification>
mark_notification_read(id) → ()
clear_all_notifications() → ()
```

### `open_local_file` — السلوك

```rust
// Windows:
ext == "pdf"  → cmd /C start "" "file:///path/to/file.pdf"  ← يفتح في المتصفح
ext == "xlsx" → cmd /C start excel.exe "path"               ← يفتح في Excel مباشرة
              → (fallback) cmd /C start "" "path"
else          → cmd /C start "" "path"                       ← التطبيق الافتراضي
```

---

## `AppState` — الهيكل الكامل

```rust
pub struct AppState {
    pub conn:       Arc<Mutex<Option<SqlConnection>>>,
    pub bot_cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub scheduler:  scheduler::SharedScheduler,  // ⭐ Arc<Mutex<SchedulerState>>
}

// لا تستخدم #[derive(Default)] — AppState لها impl Default يدوي
// يجب دائماً تمرير scheduler.clone() عند بناء AppState جديد
// في update_telegram_settings:
AppState {
    conn: state.conn.clone(),
    bot_cancel: Arc::new(Mutex::new(None)),
    scheduler: state.scheduler.clone(),  // ⭐ لا تنسَ هذا السطر!
}
```

---

## أدوات SQL المتقدمة (`agent_tools.rs`)

| ملف / حالة | المسار |
|------------|--------|
| مفضلة الاستعلامات | `%APPDATA%\com.dell.reports-app\agent_favorites.json` |
| آخر نتيجة (جلسة) | `AppState.agent_session.last_result` — تُحدَّث بعد `execute_raw_sql` و`run_query_pattern` |

### `run_query_pattern`

1. يبحث في `QUERY_PATTERNS.md` (نفس منطق `search_query_patterns`)
2. يستخرج أول كتلة ` ```sql `
3. يستبدل `60` / `30` إن مُرّرت `days_recent` / `coverage_days`
4. ينفّذ ويحفظ النتيجة لـ `export_last_result`

### `export_last_result`

يتطلب تنفيذاً ناجحاً مسبقاً في نفس الجلسة. Desktop → `[FILE_PATH:...]` | Telegram → إرسال ملف.

---

## Excel Generation (`excel_generator.rs`)

### لوحة الألوان المهنية

```rust
const C_HEADER_BG: u32 = 0x1A3869;  // أزرق داكن — رأس الأعمدة
const C_TITLE_BG:  u32 = 0x0F2548;  // أزرق أعمق — شريط العنوان
const C_ALT_ROW:   u32 = 0xECF2FB;  // أزرق فاتح — صفوف متناوبة
const C_TOTAL_BG:  u32 = 0xFFF8E1;  // أصفر — صف الإجمالي
```

### التخطيط

```
الصف 0: عنوان التقرير (merge_range كل الأعمدة)
الصف 1: تاريخ/وقت الإنشاء + عدد السجلات
الصف 2: رأس الأعمدة (ثابت عند التمرير — set_freeze_panes(3,0))
الصف 3+: البيانات (صفوف متناوبة، أرقام كـ f64، autofilter)
آخر صف: الإجمالي (إن وجدت أعمدة رقمية > 1 صف)
```

### API الصحيح لـ rust_xlsxwriter

```rust
// هذه لا تُعيد Result — استدعِها مباشرة بدون ?
ws.set_right_to_left(true);
ws.set_tab_color(Color::RGB(C_HEADER_BG));

// هذه تُعيد Result
ws.set_name("التقرير").map_err(|e| e.to_string())?;
ws.set_freeze_panes(3, 0).map_err(|e| e.to_string())?;  // وليس freeze_panes()
ws.autofilter(2, 0, last_row, last_col).map_err(|e| e.to_string())?;
ws.merge_range(0, 0, 0, last_col, title, &fmt).map_err(|e| e.to_string())?;
```

---

## PDF Generation (`pdf_generator.rs`)

### القاعدة الذهبية

كل نص عربي **يجب أن يمر عبر `prepare_text()`** قبل `layer.use_text()`:

```rust
layer.use_text(&prepare_text(text), font_size, Mm(x), Mm(y), &font);
```

Pipeline: `reshape_arabic()` → `visual_order()` → `use_text()`

### Layout (A4 أفقي 297×210mm)

```rust
MARGIN=8.0  TITLE_H=11.0  CONT_H=7.5  HDR_H=8.0  ROW_H=6.5  FOOTER_H=6.0
```

### أحجام الخط التكيّفية

| عدد الأعمدة | title | header | body |
|------------|-------|--------|------|
| ≤5 | 12 | 8 | 7.5 |
| 6-8 | 11 | 7 | 6.5 |
| 9-11 | 10 | 6 | 5.8 |
| 12+ | 9 | 5.5 | 5.2 |

---

## Scheduler Page (`scheduler-page.tsx`)

### المكونات

- **`ScheduleCard`**: بطاقة جدول مع شريط تقدم ديناميكي + عداد `mm:ss` تنازلي
- **`ProgressBar`**: يُحسب من `(interval - remaining) / interval × 100%`
- **`NotificationCard`**: زر "فتح الملف" أو طيّ/بسط النص + تعليم كمقروء
- **`Badge`**: مُعرَّف محلياً (لا يوجد ملف badge.tsx منفصل)

### التحديث الفوري

```tsx
// يستمع لأحداث الجدولة الواردة من الخلفية
const unlisten = listen<ReportNotification>("report-notification", (event) => {
  setNotifications((prev) => [event.payload, ...prev.slice(0, 99)]);
  setActiveTab("notifications");  // ينتقل تلقائياً لتبويب الإشعارات
});
```

---

## ReactMarkdown — معالج الروابط في `ai-assistant-interface.tsx`

الوكيل يُضيف `[FILE_PATH:C:\...\report.pdf]` في ردوده — يُحوَّل لرابط. معالج مخصص يمنع تحويل الصفحة:

```tsx
components={{
  a: ({ href, children }) => {
    const handleClick = (e: React.MouseEvent) => {
      e.preventDefault();
      if (!href) return;
      // مسار محلي → open_local_file
      if (/^[A-Za-z]:[\\\/]/.test(href) || href.startsWith("file://")) {
        const localPath = href.startsWith("file://")
          ? decodeURIComponent(href.replace(/^file:\/\/\/?/, "").replace(/\//g, "\\"))
          : href;
        invoke("open_local_file", { path: localPath }).catch(...);
      } else {
        invoke("open_local_file", { path: href }).catch(() => {
          window.open(href, "_blank", "noopener,noreferrer");
        });
      }
    };
    return <a href={href} onClick={handleClick}>...</a>;
  }
}}
```

> ⚠️ بدون هذا المعالج، الروابط تحوّل التطبيق لصفحة تسجيل الدخول (React Router مشكلة).

---

## `get_current_datetime` — أداة الوقت

تُعيد الوقت الحالي بتوقيت UTC+2 ليبيا مع:
- اليوم بالعربية (الأحد–السبت)
- الشهر بالعربية والرقمي
- الوقت 12h + 24h
- SQL جاهز: `MONTH(GETDATE())=X AND YEAR(GETDATE())=Y`

**متى تُستدعى:** أي طلب يذكر: اليوم، الشهر الحالي، هذه السنة، الساعة الآن، الرواتب، الحضور.

---

## QUERY_PATTERNS.md — الأنماط المتاحة

الملف في `reports-app/QUERY_PATTERNS.md` — يُبحث فيه عبر أداة `search_query_patterns`.

### الأنماط الحالية

| النمط (TRIGGERS) | متى تُستدعى |
|-----------------|------------|
| `طلبية-شراء-ذكية` | طلبية شراء، ماذا أشتري، أيام تغطية، سرعة البيع |
| `مقارنة-أسعار-موردين` | مقارنة أسعار، أرخص مورد، أسعار الموردين لمنتج — يتطلب `product_filter` |
| `نواقص-نشطة-مورد` | نواقص نشطة + مورد + آخر سعر شراء |
| `متابعة-النواقص` | نواقص، أصناف نافدة، تحت الحد الأدنى |
| `متابعة-الديون` | ديون، اللي لي، اللي علي، رصيد الزبائن |
| `رواتب-الموظفين-بعد-الخصم` | رواتب، كشف رواتب، خصم السلفة، راتب صافي |
| `المصروفات-والنفقات-التشغيلية` | مصروفات، نفقات |

### استعلام الرواتب — ملاحظة حرجة

```sql
-- SALARIES.S_STATUES هو tinyint (0-255)
-- الخطأ: CASE WHEN S.S_STATUES = -1  ← overflow
-- الصحيح:
CAST(S.S_STATUES AS smallint)
-- المصدر الاحتياطي إن كانت SALARIES فارغة:
CUSTOMERS WHERE CUST_EMP=1 → EMP_SALARY
```

---

## Database (Marketing2026 — MSSQL محلي)

```bash
Windows Auth: sqlcmd -E -S localhost -d Marketing2026
```

### الجداول الرئيسية

```sql
ITEMS         (ITEM_ID, ITEM_NAME, ITEM_MODEL, LAST_COST, AVER_COST, MIN_LEVEL, MAX_LEVEL, ITEM_INVISIBLE)
ITEMS_SUB     (ITEM_ID, STORE_ID, QTY, CATEOGRY1=Batch, CATEOGRY3=ExpiryDate)
BUY_INVOICE   (B_ID, B_DATE, CUST_ID→CUSTOMERS=مورد, USERS_ID→USERS, COMM_ID=0 دائماً)
BUY_ITEMS     (B_ITEM_ID, B_ID, ITEM_ID, PRICE, QTY, CATEOGRY3=Expiry)
SALE_INVOICE  (S_ID, S_DATE, CUST_ID, CUST_NAME, USERS_ID, S_STATUES tinyint, WAIT bit)
SALE_ITEMS    (S_ITEM_ID, S_ID, ITEM_ID, PRICE, QTY, LAST_COST, AVER_COST)
  ⚠️ SALE_ITEMS ليس فيه S_DATE — دائماً JOIN إلى SALE_INVOICE
CUSTOMERS     (CUST_ID, CUST_NAME, CUST_VENDOR=مورد, CUST_CUSTOM=زبون, CUST_EMP=موظف)
USERS         (USERS_ID, FULL_NAME)
STORES        (STORE_ID, STORE_NAME)
TAKE          (T_ID, CUST_ID, T_VALUE, T_DATE) ← تحصيلات من الزبائن
GIVE          (G_ID, CUST_ID, G_VALUE, G_DATE) ← مدفوعات للموردين
BALANCE_EDIT  (CUST_ID, BL_DEBIT, BL_CREDIT)  ← تسويات
BALANCE_C     ← فارغ في DB الحالي، لا تعتمد عليه
COMMISSIONER  ← مهمل تماماً (COMM_ID=0 / COMM_NAME='N/A' فقط)
R_S_INVOICE   (S_R_ID, S_R_DATE, CUST_ID, USERS_ID) ← مرتجع المبيعات (من الزبون)
R_S_ITEMS     (S_R_ITEM_ID, S_R_ID, ITEM_ID, QTY, PRICE) ← أصناف مرتجع المبيعات
B_R_INVOICE   (B_R_ID, B_R_DATE, CUST_ID, USERS_ID) ← مرتجع المشتريات (للمورد)
B_R_ITEMS     (B_R_ITEM_ID, B_R_ID, ITEM_ID, QTY, PRICE) ← أصناف مرتجع المشتريات
SPOIL_INVOICE (SP_ID, SP_DATE, USERS_ID) ← فواتير التالف
SPOIL_ITEMS   (SP_ITEM_ID, SP_ID, ITEM_ID, QTY, PRICE) ← أصناف التالف
TRANSFER_INVOICE (TR_ID, TR_DATE, FROM_STORE, TO_STORE) ← تحويل بين المخازن
TRANSFER_ITEMS   (TR_ITEM_ID, TR_ID, ITEM_ID, QTY, PRICE)
UNITS         (UNIT_ID, UNIT_NAME, UNIT_SIZE) ← وحدات القياس للمنتجات
SITTEINGS     (رقم_هاتف_1, شعار_الشركة, etc) ← جدول إعدادات من صف واحد (لا تقم بـ JOIN معه)
```

### حقائق مُتحقق منها

- نطاق فواتير البيع: `2025-07-20 → 2026-04-07` (2140 فاتورة)
- نطاق فواتير الشراء: `2025-07-22 → 2026-04-05` (1473 فاتورة)
- ITEMS: 17,616 منتج
- ITEMS_SUB: 4,110 صف، جميعها QTY > 0
- تواريخ الصلاحية: `2025-03-01 → 2032-06-30`
- `SALARIES` غالباً فارغ — استخدم `CUSTOMERS.CUST_EMP=1 + EMP_SALARY`

### قواعد SQL

```sql
-- ✅ T-SQL فقط: TOP، GETDATE()، DATEADD، CONVERT، ISNULL
-- ❌ ممنوع: LIMIT، NOW()، ILIKE (PostgreSQL syntax)
-- ✅ دائماً: الأسماء UPPERCASE_SNAKE_CASE كما في DDL
-- ✅ للبحث: LIKE N'%term%' (وليس =)
-- ✅ مرجع التاريخ: MAX(S_DATE) من SALE_INVOICE (ليس GETDATE() وحده)
-- ✅ DAYS_RECENT=60 (ليس 30) — آخر بيانات قبل ~47 يوم
```

---

## Telegram Bot

### الإعدادات

| المفتاح | النوع | الوصف |
|---------|-------|-------|
| `telegram_bot_token` | AES-256-GCM | توكن @BotFather |
| `telegram_chat_id` | AES-256-GCM | معرّف المحادثة المصرّح بها |
| `telegram_enable_queries` | bool | تشغيل البوت |
| `groq_api_key` | AES-256-GCM | مفتاح OpenRouter |
| `openai_api_key` | AES-256-GCM | مفتاح OpenAI للـ RAG |
| `ai_model` | string | النموذج الافتراضي |

- مفتاح التشفير: `b"ReportsApp-SecureKey-2026-v1.0!!"` (ثابت في `lib.rs`)
- ملف الإعدادات: `%APPDATA%\com.dell.reports-app\settings.json`

### نماذج AI (OpenRouter — مجاني مؤقتاً)

1. `qwen/qwen3-coder:free` (افتراضي — أدوات + SQL)
2. `deepseek/deepseek-v4-flash:free` (احتياطي)
3. `meta-llama/llama-3.3-70b-instruct:free` (احتياطي)

بعد شحن الرصيد: يمكن العودة إلى `google/gemini-2.5-flash` في `ai-config.ts` و `ai_agent.rs`.

### قاعدة إخراج Telegram

```
0 صفوف   → send_message "📭 لم يُعثر على نتائج"
1-5 صفوف → send_html (format_rows_as_html)
6+ صفوف  → send_pdf (PDF بالعربية)
```

### تنسيق HTML لـ Telegram

```
he(val)   ← إلزامي على كل قيمة من DB (يهرب & < >)
<b>       ← الأسعار والتكاليف
<code>    ← الأكواد والأرقام
send_html ← parse_mode: "HTML" (ليس Markdown!)
```

---

## نظام RAG

### المعمارية

```
استفسار → enrich_query_with_english() → OpenAI embedding
        → Supabase match_documents (cosine similarity, threshold=0.05, count=15)
        → (fallback) ILIKE على keywords ≥ 3 أحرف
        → merge_ddl_schemas(existing, new) ← ذاكرة تراكمية عبر المحادثة
        → system prompt → LLM
```

### الذاكرة التراكمية

`merge_ddl_schemas()` تدمج مخطط الجلسة السابقة مع الجديد — الوكيل لا ينسى الجداول التي رآها في نفس المحادثة.

### مشاكل RAG المعروفة

1. **جذري:** 3040 chunk في Supabase مُكرَّر ومقطَّع — الحل: `python reembed_ddl.py --openai-key sk-...`
2. **كلمات قصيرة:** الـ fallback يتجاهل كلمات < 3 أحرف

---

## Frontend Conventions

- **TypeScript Strict:** `noUnusedLocals`, `noUnusedParameters` مفعّلة
- **Path alias:** `@/` → `src/`
- **RTL:** `dir="rtl"` على root div في App.tsx
- **Styling:** Tailwind CSS v4 (vite plugin) — لا CSS modules
- **`Badge`:** لا يوجد ملف `badge.tsx` — عرّفه محلياً أو استخدم `<span>` مباشرة
- **`cn()`** من `src/lib/utils.ts` لدمج class names

---

## Rust Crates المهمة

| Crate | الاستخدام | ملاحظات |
|-------|-----------|---------|
| `tiberius 0.12` | MSSQL client | features: sql-browser-tokio, chrono, rust_decimal |
| `rust_xlsxwriter` | Excel generation | set_right_to_left/set_tab_color تُعيد ()، set_freeze_panes يُعيد Result |
| `printpdf 0.5` | PDF generation | لا يدعم RTL — مرر النص عبر prepare_text() |
| `unicode-bidi 0.3` | BiDi للعربية | مطلوب لعرض العربية في PDF |
| `aes-gcm 0.10` | تشفير AES-256 | للإعدادات الحساسة |
| `reqwest 0.12` | HTTP | features: json, multipart |
| `tauri-plugin-store 2` | تخزين الإعدادات | settings.json مُشفَّر |
| `rand` | توليد IDs | في scheduler::new_id() |

---

## ملفات مرجعية مهمة

| الملف | المحتوى |
|-------|---------|
| `AGENTS.md` | دليل المشروع الكامل (هذا الملف) |
| `AI_SYSTEM_PROMPT.md` | رسالة النظام للوكيل — للنماذج اللاحقة |
| `QUERY_PATTERNS.md` | أنماط SQL — `search_query_patterns` و `run_query_pattern` |
| `src-tauri/src/agent_tools.rs` | تنفيذ الأدوات المتقدمة (validate, export, favorites…) |
| `DATABASE_NOTES.md` | ملاحظات حية عن Marketing2026 — **حدّثه عند كل اكتشاف جديد** |
| `Full_Marketing_Database_DDL.sql` | DDL كامل (~2.3 MB) — اقرأ بـ Grep/offset |
| `smart_purchase_order.sql` | استعلام الطلبية الذكية المختبر |
| `shortage_tracking.sql` | استعلام متابعة النواقص المختبر |
| `debts_tracking.sql` | استعلام الديون المختبر |

### قاعدة: كل اكتشاف عن DB يُسجَّل في `DATABASE_NOTES.md`

كلما اكتشفت شيئاً جديداً عن قاعدة البيانات — سجّله فوراً:
- جدول جديد أو علاقة FK
- عمود يتصرف بشكل غير متوقع
- استعلام نجح أو فشل مع سبب
- نطاق تواريخ تغيّر

---

## Supabase

- **Project ID:** `nsgmhijtaaenpqxxgjds`
- **Anon Key:** `eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im5zZ21oaWp0YWFlbnBxeHhnamRzIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxODU1NTMsImV4cCI6MjA5NDc2MTU1M30.bva5PiwsoBiLR7u2upQV7q2spl6GhAg-JqrQ8nnUC8E`

جدول `reports` → `id, name, name_ar, sql_query, has_parameters, is_active, sort_order`
جدول `documents` → DDL chunks للـ RAG (3040 chunk — يحتاج إعادة تضمين)

```bash
# إعادة بناء الـ RAG (مهم لتحسين دقة الوكيل):
cd C:\Users\DELL\Desktop\al-tabi
python reembed_ddl.py --openai-key sk-... --dry-run   # اختبار جاف
python reembed_ddl.py --openai-key sk-...              # تنفيذ فعلي
```
