# al-tabi

تطبيق تقارير ومحاسبة لنظام Marketing2026 ERP — واجهة سطح مكتب (Tauri + React) مع وكيل ذكاء اصطناعي لاستعلامات SQL والتقارير.

## المحتويات

- `reports-app/` — التطبيق الرئيسي (Tauri + React + TypeScript + Rust)
- `Full_Marketing_Database_DDL.sql` — مخطط قاعدة البيانات
- `.cursor/skills/` — مهارات الوكيل لاستعلامات المحاسبة

## التشغيل

```powershell
cd reports-app
npm install
npm run tauri dev
```

## المتطلبات

- Node.js 18+
- Rust (via rustup)
- SQL Server مع قاعدة Marketing2026
- مفتاح OpenRouter (يُخزَّن من إعدادات التطبيق)

## ملاحظات

- لا ترفع ملفات `.env` أو مفاتيح API إلى المستودع.
- إعدادات الاتصال والمفاتيح تُدار عبر Supabase أو التخزين المحلي المشفّر في التطبيق.
