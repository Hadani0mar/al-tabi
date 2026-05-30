# AGENTS.md — دليل الوكلاء

دليل مختصر لتطوير **reports-app**: تطبيق Tauri (React + Rust) يتصل بـ MSSQL ويدعم **نظامين ERP** (Marketing2026 + InfinityRetailDB) وينفّذ تقارير عبر وكيل AI.

> **ابدأ من:** [`ERP_ARCHITECTURE.md`](./ERP_ARCHITECTURE.md) — خريطة النظامين، الاكتشاف، المحوّلات، وتسميات الأعمدة.

---

## الهدف

- **Desktop:** محادثة AI → استعلامات SQL → نتائج نص/PDF/Excel
- **جدولة:** تقارير دورية + إشعارات محلية
- **Telegram:** بوت AI بنفس الأدوات (RAG + OpenRouter)
- **تحديث ذاتي:** Tauri Updater عبر GitHub Releases

---

## هيكل المشروع

```
reports-app/
├── src/                              # React 19 + TypeScript
│   ├── App.tsx                       # التوجيه بين الشاشات + auto-login
│   ├── lib/ai-config.ts              # FIXED_AI_MODEL = minimax/minimax-m2.7
│   └── components/ui/
│       ├── sql-login-page.tsx        # اتصال MSSQL + تذكر الدخول
│       ├── ai-assistant-interface.tsx # شات الوكيل
│       ├── saved-queries-page.tsx    # استعلامات محفوظة (بدون عرض SQL)
│       ├── scheduler-page.tsx        # جدولة + إشعارات
│       ├── generic-report-page.tsx   # بحث منتج (ERP-aware)
│       ├── settings-page.tsx         # Telegram + تحديثات + حساب + erp_label
│       └── futuristic-nav.tsx        # 6 تبويبات
├── ERP_ARCHITECTURE.md               # ⭐ نقطة البداية — نظامان ERP
├── AGENT_Marketing2026.md            # ⭐ أنماط + تعاليم Marketing
├── AGENT_InfinityRetailDB.md         # ⭐ أنماط + تعاليم Infinity
├── QUERY_PATTERNS.md                 # legacy Marketing — يُفضّل AGENT_Marketing2026.md
├── PRODUCT_SCHEMA.md                 # منتجات Marketing
├── INFINITY_PRODUCT_SCHEMA.md        # منتجات Infinity
├── DATABASE_NOTES.md                 # Marketing (+ إحالة Infinity)
├── INFINITY_DATABASE_NOTES.md        # Infinity
├── DATABASE_VIEWS.md                 # Views Marketing
├── INFINITY_DATABASE_VIEWS.md        # Views Infinity
└── src-tauri/src/
    ├── lib.rs          # AppState، Tauri commands، erp_kind
    ├── erp_profile.rs  # اكتشاف ERP + تحميل AGENT_*.md
    ├── erp_adapters.rs # SQL مشترك الشكل (profile, products, reports, POS)
    ├── ai_agent.rs     # handle_with_groq_local + system prompt حسب ERP
    ├── agent_tools.rs  # validate_sql، patterns، favorites، export
    ├── scheduler.rs    # تقارير مجدولة + notifications.json
    ├── telegram.rs     # بوت Telegram
    ├── supabase_config.rs  # مفاتيح OpenRouter (محلي أولاً ثم Supabase)
    ├── pdf_generator.rs / excel_generator.rs
```

### تدفق البيانات

| المسار | التسلسل |
|--------|---------|
| Desktop AI | `ai-assistant-interface` → `ask_local_ai` → `handle_with_groq_local` → أدوات → MSSQL |
| جدولة | `run_scheduler` (كل 5ث) → SQL → ملف/نص → event `report-notification` |
| Telegram | polling → `handle_with_groq` → OpenRouter + RAG |
| إعدادات | AES-256-GCM في `%APPDATA%\com.dell.reports-app\settings.json` |

---

## تشغيل وتطوير

```bash
npm run tauri dev          # تطوير (منفذ 1420)
npm run tauri build        # إنتاج
cd src-tauri && cargo check  # تحقق Rust سريع
taskkill /f /im reports-app.exe   # إيقاف قبل إعادة البناء (Windows)
```

**TypeScript strict** مفعّل — احذف المتغيرات غير المستخدمة قبل الـ commit.

---

## تبويبات التطبيق

| # | التبويب | المكوّن |
|---|---------|---------|
| 0 | التقارير | `SchedulerPage` — الجداول المجدولة |
| 1 | البحث | `GenericReportPage` |
| 2 | تنبيهات | `SchedulerPage` — نفس الصفحة، تبويب الإشعارات داخلياً |
| 3 | الذكاء | `AIAssistantInterface` — **يبقى mounted** (hidden) لعدم قطع الطلبات |
| 4 | المحفوظات | `SavedQueriesPage` — تشغيل مباشر بدون AI |
| 5 | الإعدادات | `SettingsPage` — بوت، تحديثات، تسجيل خروج |

---

## الوكيل الذكي — قواعد إلزامية

### ERP أولاً

1. بعد الاتصال: `get_erp_kind` → `marketing2026` | `infinity_retail_db`
2. **Marketing:** `AGENT_Marketing2026.md` — جداول `dbo.*`
3. **Infinity:** `AGENT_InfinityRetailDB.md` — `Inventory.*`, `SALES.*`, `Purchase.*`
4. **لا تخلط** — `dbo.ITEMS` على Infinity = خطأ 208

### ترتيب القرار

1. وقت/تاريخ؟ → `get_current_datetime` أولاً
2. استعلام معروف؟ → `search_query_patterns` ثم `run_query_pattern`
3. SQL جديد؟ → `validate_sql` ثم `execute_raw_sql`
4. جدولة؟ → `schedule_report`
5. جدول غير معروف؟ → `search_schema` / `explore_local_schema`

### قواعد صارمة

- **SELECT فقط** — ممنوع INSERT/UPDATE/DELETE/DROP/EXEC
- **أداة SQL واحدة في كل دور** — لا `execute_raw_sql` متوازي
- **ترجمة الأعمدة للعربية** في PDF/Excel — راجع `ERP_ARCHITECTURE.md` §6 (تسميات مختلفة لكل ERP)
- **T-SQL فقط:** `TOP`، `GETDATE()`، `LIKE N'%…%'` — لا `LIMIT`/`ILIKE`
- **Marketing — SALE_ITEMS** لا يحتوي `S_DATE` — JOIN مع `SALE_INVOICE`
- **Infinity — بنود البيع** — JOIN `SALES.Data_SalesInvoices` للتاريخ (`SalesInvoiceDate`)
- **COMM_ID = 0** دائماً — تجاهل `COMMISSIONER`
- **Marketing — مرجع التاريخ:** `MAX(S_DATE)` من `SALE_INVOICE`
- **Infinity — مرجع التاريخ:** `MAX(SalesInvoiceDate)` من `SALES.Data_SalesInvoices`
- **DAYS_RECENT = 60** افتراضياً (ليس 30)
- **المبالغ:** أضف `د.ل` في الردود العربية
- **حفظ مفضلة:** بعد `execute_raw_sql` ناجح → **يجب** استدعاء `save_favorite_query` — لا تقل «تم الحفظ» بدون tool call

### النموذج الحالي

- Desktop + Telegram: `minimax/minimax-m2.7` عبر OpenRouter
- المفتاح: `groq_api_key` في settings (اسم legacy) = OpenRouter API key
- `supabase_config.rs`: يحمّل المفتاح المحلي أولاً، ثم Supabase (timeout 5ث)

### قفل وإلغاء الطلبات

```rust
// lib.rs — ask_local_ai
let _agent_slot = app_state.ai_request_lock.lock().await;  // طلب واحد في كل لحظة
```

- الواجهة ترسل `requestId` (UUID) مع كل رسالة
- الإيقاف: `cancel_local_ai({ requestId })` — زر ⏹ فقط
- **لا تلغِ الطلب السابق عند إرسال جديد** — الواجهة تمنع الإرسال المزدوج
- `finally` في الواجهة يمسح التحميل **فقط** إذا كان `requestId` لا يزال الأحدث

### إكمال المنتجات `@`

- `@` في الشات → `search_product_mentions` → `@اسم (كود)`
- مرّر `product_filter` لـ `run_query_pattern` / `plan_complex_query`

---

## أدوات الوكيل (ملخص)

| الأداة | متى |
|--------|-----|
| `run_query_pattern` | **الأولوية** لأي استعلام معقد |
| `search_query_patterns` | قراءة نمط قبل التعديل |
| `execute_raw_sql` | SELECT مخصص (حد 100 صف) |
| `validate_sql` / `explain_sql` | فحص أو شرح قبل التنفيذ |
| `get_database_views` | قبل مبيعات موظف/يومية |
| `plan_complex_query` + `execute_query_plan` | تحليل منتج مركب |
| `create_pdf_report` / `create_excel_report` | تصدير من SELECT |
| `export_last_result` | PDF/Excel لآخر نتيجة في الجلسة |
| `save_favorite_query` / `list_favorite_queries` | حفظ واسترجاع |
| `schedule_report` / `list_scheduled_reports` / `delete_scheduled_report` | جدولة |
| `get_current_datetime` | أي سؤال عن الوقت الحالي |

التفاصيل الكاملة: `agent_tools.rs` + `ai_agent.rs` (tool definitions).

---

## المحادثة Desktop — ملاحظات الواجهة

- `AIAssistantInterface` يبقى mounted في `App.tsx` حتى خارج تبويب الذكاء
- حالة الأدوات: event `tool-usage` من Rust
- جداول Markdown: `overflow-x-auto`
- **المحفوظات:** `FavoriteDto` لا يُرجع SQL — IP protection؛ التشغيل عبر `execute_favorite_query`

### Tauri commands — AI والمحفوظات

```
ask_local_ai(message, history, groq_key, ai_model, request_id)
cancel_local_ai(request_id)
list_favorite_queries() → FavoriteDto[]   # بدون sql
delete_favorite_query(id)
execute_favorite_query(id) → QueryResult
search_product_mentions(query)
open_local_file(path)   # PDF→متصفح، xlsx→Excel
```

---

## QUERY_PATTERNS / AGENT_*.md

**Marketing:** `AGENT_Marketing2026.md` | **Infinity:** `AGENT_InfinityRetailDB.md` | **معمارية:** `ERP_ARCHITECTURE.md`

**اقرأ الملف المناسب قبل SQL معقد.** أنماط Marketing الرئيسية (legacy في QUERY_PATTERNS.md):

| النمط | الاستخدام |
|-------|-----------|
| `طلبية-شراء-ذكية` | كمية شراء مقترحة، أيام تغطية |
| `مقارنة-أسعار-موردين` | يتطلب `product_filter` |
| `متابعة-النواقص` / `نواقص-نشطة-مورد` | نواقص المخزون |
| `متابعة-الديون` | ديون الزبائن |
| `ديون-الموردين-مبسط` | `[اسم المورد]` + `[الدين]` فقط |
| `رواتب-الموظفين-بعد-الخصم` | رواتب — `CAST(S_STATUES AS smallint)` |

⚠️ كل الأنماط تبدأ بـ `WITH` أو `SELECT` — **لا DECLARE** (توافق مع أدوات التصدير).

---

## قاعدة البيانات (Marketing2026)

```bash
sqlcmd -E -S localhost -d Marketing2026
```

### جداول أساسية

| الجدول | ملاحظة |
|--------|--------|
| `ITEMS` / `ITEMS_SUB` | منتجات + مخزون (QTY، CATEOGRY3=صلاحية) |
| `SALE_INVOICE` / `SALE_ITEMS` | مبيعات — التاريخ في INVOICE |
| `BUY_INVOICE` / `BUY_ITEMS` | مشتريات من الموردين |
| `CUSTOMERS` | زبون/مورد/موظف (`CUST_EMP=1`) |
| `USERS` | `FULL_NAME` للموظفين |
| `TAKE` / `GIVE` | تحصيلات / مدفوعات |
| `R_S_*` / `B_R_*` | مرتجعات مبيعات / مشتريات |

### حقائق متحققة

- بيانات البيع: ~2025-07 → 2026-04 | ~17K منتج | ~4K صف مخزون
- `SALARIES` غالباً فارغ → `CUSTOMERS.EMP_SALARY` احتياطي
- `BALANCE_C` فارغ — لا تعتمد عليه

**سجّل كل اكتشاف Marketing في `DATABASE_NOTES.md` — وInfinity في `INFINITY_DATABASE_NOTES.md`.**

---

## InfinityRetailDB (ملخص)

```bash
sqlcmd -E -S localhost -d InfinityRetailDB
```

`Inventory.Data_Products`, `SALES.Data_SalesInvoices`, `MyCompany.Config_Branchs` — راجع `INFINITY_DATABASE_NOTES.md`.

---

## الجدولة (`scheduler.rs`)

- تخزين: `%APPDATA%\com.dell.reports-app\schedules.json` + `notifications.json`
- `interval_seconds`: 60=دقيقة، 3600=ساعة، 86400=يوم
- `columns` إلزامية بالعربية في `schedule_report`
- ملفات PDF/Excel: `%APPDATA%\com.dell.reports-app\reports\`

---

## Telegram (مختصر)

- إعدادات مشفرة: `telegram_bot_token`, `telegram_chat_id`, `groq_api_key`, `openai_api_key`
- مخرجات: 0 صف → رسالة | 1-5 → HTML | 6+ → PDF
- RAG: Supabase `documents` — chunks قديمة/مكررة؛ إعادة تضمين عبر `reembed_ddl.py` عند الحاجة

---

## ⭐ التحديث عن بُعد + GitHub Actions

التطبيق مهيّأ لتحديث ذاتي عبر **Tauri Updater** + **GitHub Releases**. أي تعديل يصل للمستخدمين دون إعادة تثبيت يدوي.

### المعمارية

```text
المطوّر يدفع tag v0.X.Y
        ↓
GitHub Actions (.github/workflows/release.yml) يبني على windows-latest
        ↓
ينشر إلى Releases: .exe + .msi + .sig + latest.json
        ↓
التطبيق المثبّت يفحص: https://github.com/Hadani0mar/al-tabi/releases/latest/download/latest.json
        ↓
يقارن الإصدار، يُنزّل، يتحقق من التوقيع، يثبّت، يُعيد التشغيل
```

### الإعدادات الموجودة (لا تعدّلها بدون فهم)

| الموقع | الغرض |
|--------|-------|
| `reports-app/src-tauri/tauri.conf.json` → `plugins.updater` | endpoint + pubkey العام |
| `reports-app/src-tauri/Cargo.toml` | `tauri-plugin-updater` + `tauri-plugin-process` |
| `reports-app/src-tauri/src/lib.rs` | `.plugin(tauri_plugin_updater::Builder::new().build())` |
| `reports-app/src-tauri/capabilities/default.json` | `updater:default` + `process:default` |
| `reports-app/src/components/ui/settings-page.tsx` | تبويب «التحديثات» (`check()` + `downloadAndInstall()` + `relaunch()`) |
| `.github/workflows/release.yml` | بناء + نشر تلقائي عند push tag `v*` |
| `reports-app/src-tauri/al-tabi.key.pub` | المفتاح العام (يُرفع للمستودع) |
| `reports-app/src-tauri/al-tabi.key` | **المفتاح الخاص — في `.gitignore`، لا ترفعه أبداً** |

### الأسرار في GitHub Actions

| السرّ | المحتوى | كيف يُرفع |
|-------|---------|-----------|
| `TAURI_SIGNING_PRIVATE_KEY` | محتوى `al-tabi.key` كاملاً (sealed-box encrypted) | عبر PyNaCl + GitHub API |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | كلمة مرور المفتاح (حالياً: `altabi2026`) | نفس الطريقة |
| `GITHUB_TOKEN` | يُولَّد تلقائياً | لا حاجة لرفعه |

> ⚠️ المفتاح الخاص وكلمة مروره **في حوزة المطوّر فقط**. لا يمكن لأي وكيل توليد توقيع صالح بدونهما.

### خطوات إصدار نسخة جديدة (نمط ثابت)

```powershell
# 1) رفع رقم الإصدار في 3 ملفات (متطابق تماماً)
#    reports-app/package.json          → "version": "0.X.Y"
#    reports-app/src-tauri/Cargo.toml  → version = "0.X.Y"
#    reports-app/src-tauri/tauri.conf.json → "version": "0.X.Y"

# 2) تحديث Cargo.lock بعد تغيير Cargo.toml (مهم — وإلا فشل البناء على CI)
cd reports-app/src-tauri ; cargo check ; cd ../..

# 3) اختبار محلي قبل الـ tag — TypeScript strict سيكشف الـ unused
cd reports-app ; npm run build ; cd ..

# 4) commit + push على main
git add -A
git commit -m "Release v0.X.Y: <ملخص قصير>"
git push

# 5) إنشاء tag ودفعه — هذا ما يُشغّل الـ workflow
git tag v0.X.Y
git push origin v0.X.Y
```

### استعادة بعد فشل البناء

إذا فشل البناء وتريد إعادة المحاولة بنفس رقم الإصدار:

```powershell
# احذف الـ tag محلياً ومن origin
git push --delete origin v0.X.Y
git tag -d v0.X.Y

# طبّق الإصلاح وادفعه
git add -A ; git commit -m "fix: ..." ; git push

# أعد إنشاء الـ tag
git tag v0.X.Y
git push origin v0.X.Y
```

### مراقبة الـ workflow عبر GitHub MCP

```text
github_list_workflow_runs(owner, repo, per_page)           → آخر التشغيلات
github_list_workflow_run_jobs(owner, repo, run_id)         → خطوات تشغيل محدد
github_get_workflow_run(owner, repo, run_id)               → ملخص حالة
github_download_workflow_run_logs(owner, repo, run_id)     → ZIP بالـ logs
github_get_release_by_tag(owner, repo, tag)                → التحقق من Release
```

> الـ MCP `github_download_workflow_run_logs` يعيد ZIP — لا تحاول parse كـ JSON. استخدم Python داخلياً.

### الأخطاء الشائعة وحلولها

| الخطأ | السبب | الحل |
|-------|-------|------|
| `failed to decode secret key: Wrong password` | عدم تطابق `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` مع المفتاح | جدّد المفتاح + اختبر التوقيع محلياً قبل الرفع |
| `error TS6133: 'X' is declared but its value is never read` | متغير غير مستخدم — TypeScript strict | احذف المتغير أو استخدمه |
| `Cargo.lock is out of date` على CI | نسيت `cargo check` بعد تغيير `Cargo.toml` | اعمل cargo check محلياً واتمم |
| Workflow معلّق في `Build and release Tauri app` لـ +15 دقيقة | طبيعي للبناء الأول؛ مع الـ cache يصبح ~5-7 دقائق | انتظر أو افحص اللوغ |
| المستخدم لا يرى التحديث | المستودع كان خاصاً أو لم يحدّث | تأكد أن `visibility: public` و أن `latest.json` متاح عبر `curl` |

### اختبار التوقيع محلياً (قبل CI)

```powershell
cd reports-app

# قراءة المفتاح كنصّ ثم تمريره كـ -k (الـ -k يأخذ القيمة، ليس المسار)
$keyValue = Get-Content src-tauri\al-tabi.key -Raw
$testFile = "src-tauri\test_sign.txt"
"test" | Out-File -Encoding utf8 $testFile

npx @tauri-apps/cli signer sign -k $keyValue -p altabi2026 $testFile

# لو نجح → "Your file was signed successfully"
# لو فشل → كلمة المرور خطأ → جدّد المفتاح
Remove-Item $testFile, "$testFile.sig" -ErrorAction SilentlyContinue
```

### رفع أسرار GitHub برمجياً

GitHub Secrets تتطلب تشفير قيمتها بمفتاح المستودع العام عبر libsodium sealed-box. الطريقة الموثّقة:

```python
# يتطلب: pip install pynacl requests
# تشغيل: set GH_TOKEN=ghp_...  ;  python upload_secret.py
import base64, os, requests
from nacl import encoding, public

OWNER, REPO = "Hadani0mar", "al-tabi"
api = f"https://api.github.com/repos/{OWNER}/{REPO}/actions/secrets"
hdr = {"Authorization": f"Bearer {os.environ['GH_TOKEN']}",
       "Accept": "application/vnd.github+json",
       "X-GitHub-Api-Version": "2022-11-28"}

pk = requests.get(f"{api}/public-key", headers=hdr).json()
sealed = public.SealedBox(public.PublicKey(pk["key"].encode(), encoding.Base64Encoder()))
enc = base64.b64encode(sealed.encrypt(b"SECRET_VALUE")).decode()

requests.put(f"{api}/SECRET_NAME", headers=hdr,
             json={"encrypted_value": enc, "key_id": pk["key_id"]}).raise_for_status()
```

> ⚠️ لا تترك هذا السكربت في المستودع. أنشئه مؤقتاً، نفّذه، احذفه.

### مفتاح التوقيع — قواعد ذهبية

1. **لا تجدّد المفتاح بدون داعٍ** — كل المستخدمين الذين يحملون النسخة القديمة لن يستطيعوا قبول التحديث لأن `pubkey` تغيّر.
2. إذا اضطُررت لتجديد المفتاح، خطّط لتوزيع نسخة كاملة جديدة للمستخدمين يدوياً.
3. احتفظ بنسخة احتياطية من `al-tabi.key` و `al-tabi.key.pub` خارج الجهاز (USB / SecureStorage).
4. كلمة المرور موثّقة هنا فقط ولا تُكتب في أي ملف داخل المستودع.

### قواعد تعديل واجهة الإعدادات (Updates Tab)

- زرّا «حفظ الإعدادات» و«اختبار الإرسال» يجب أن يظهرا **فقط** في تبويب البوت (`activeTab === "bot"`).
- تبويب AI يعرض ملاحظة «إدارة المطوّر» — بدون حقول إدخال.
- تبويب التحديثات يستخدم `check()`, `downloadAndInstall()`, `relaunch()` من `@tauri-apps/plugin-updater` و `@tauri-apps/plugin-process`.
- إصدار التطبيق يُقرأ عبر `getVersion()` من `@tauri-apps/api/app`.

### مهارة استكشاف الأخطاء — تسلسل ثابت

عندما يفشل الـ workflow:

1. `github_list_workflow_runs` → خذ آخر run_id فاشل
2. `github_list_workflow_run_jobs` → اعرف أي خطوة فشلت
3. `github_download_workflow_run_logs` → نزّل ZIP، استخرج، اقرأ آخر ~50 سطر من الـ txt
4. صنّف الخطأ (تشفير / TS / Cargo / network / signing)
5. أصلح، احذف الـ tag، أعد المحاولة

> القاعدة: **اقرأ اللوغ قبل التخمين**.

### كيف يحصل المستخدم على التحديث

```text
الإعدادات → التحديثات → تحقق من التحديثات → تثبيت
```

التطبيق يُنزّل التحديث، يتحقق من التوقيع، يثبّته، ويعيد التشغيل تلقائياً.

ملفات Release المتوقعة:
- `al-tabi_0.X.Y_x64-setup.exe` — مثبّت Windows
- `al-tabi_0.X.Y_x64-setup.nsis.zip` — حزمة التحديث
- `al-tabi_0.X.Y_x64-setup.nsis.zip.sig` — توقيع
- `latest.json` — بيان التحديث للتطبيق

> دليل إضافي: [`RELEASE.md`](../RELEASE.md) في جذر المستودع.

---

## Supabase

- Project: `nsgmhijtaaenpqxxgjds`
- `reports` — تقارير محفوظة | `documents` — RAG DDL chunks
- Anon key في `supabase_config.rs` — إن فشل (401) يُستخدم المفتاح المحلي

---

## Frontend

- RTL: `dir="rtl"` | Tailwind v4 | alias `@/` → `src/`
- `cn()` من `lib/utils.ts`
- لا ملف `badge.tsx` — عرّف Badge محلياً عند الحاجة
- روابط `[FILE_PATH:…]` في الشات → `open_local_file` (معالج مخصص في `ai-assistant-interface.tsx`)

---

## ملفات مرجعية

| الملف | متى تقرأه |
|-------|-----------|
| **`ERP_ARCHITECTURE.md`** | **أولاً — نظامان ERP، محوّلات، أعمدة تقارير** |
| `AGENT_Marketing2026.md` | SQL معقد على Marketing |
| `AGENT_InfinityRetailDB.md` | SQL معقد على Infinity |
| `QUERY_PATTERNS.md` | legacy Marketing |
| `PRODUCT_SCHEMA.md` / `INFINITY_PRODUCT_SCHEMA.md` | أعمدة منتجات |
| `DATABASE_NOTES.md` / `INFINITY_DATABASE_NOTES.md` | اكتشافات تشغيلية |
| `DATABASE_VIEWS.md` / `INFINITY_DATABASE_VIEWS.md` | Views وربط |
| `src-tauri/src/erp_adapters.rs` | تقارير UI + POS + profile |
| `src-tauri/src/erp_profile.rs` | اكتشاف ERP |
| `src-tauri/src/ai_agent.rs` | system prompt + tool loop |
| `src-tauri/src/agent_tools.rs` | تنفيذ الأدوات |
| `Full_Marketing_Database_DDL.sql` / `InfinityRetailDB_DDL.sql` | DDL |
| `.github/workflows/release.yml` | CI/CD |
| `RELEASE.md` | دليل الإصدار |

---

## قواعد للوكلاء

1. **اقرأ قبل التعديل** — `ERP_ARCHITECTURE.md` + AGENT_* + DATABASE_* / INFINITY_*
2. **أقل diff ممكن** — لا ت refactor واسع بدون طلب
3. **لا secrets في git** — مفاتيح API، `al-tabi.key`
4. **لا tag/release** إلا بطلب صريح من المستخدم
5. **لا تعرض SQL للمستخدم** في صفحة المحفوظات
6. **حدّث** `DATABASE_NOTES.md` أو `INFINITY_DATABASE_NOTES.md` عند كل اكتشاف DB
7. **SQL تقارير Infinity** — في `erp_adapters.rs` وليس في React
