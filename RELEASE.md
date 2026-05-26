# دليل الإصدار والتحديث عن بُعد

## الإعداد الأولي (مرّة واحدة فقط)

### 1. إضافة الأسرار إلى GitHub

اذهب إلى:

```text
https://github.com/Hadani0mar/al-tabi/settings/secrets/actions
```

أضف سرّين جديدين:

#### TAURI_SIGNING_PRIVATE_KEY

محتوى الملف:

```text
reports-app/src-tauri/al-tabi.key
```

افتح الملف بمحرر نصوص وانسخ كامل محتواه (سطر طويل base64).

#### TAURI_SIGNING_PRIVATE_KEY_PASSWORD

اترك القيمة **فارغة** (المفتاح أُنشئ بدون كلمة مرور).

> ⚠️ **مهم:** لا ترفع ملف `al-tabi.key` إلى المستودع أبداً. هو محمي عبر `.gitignore`.

### 2. حفظ نسخة احتياطية من المفتاح

احتفظ بنسخة آمنة من الملفات التالية خارج المستودع:

```text
reports-app/src-tauri/al-tabi.key
reports-app/src-tauri/al-tabi.key.pub
```

إذا فقدت المفتاح الخاص، لن يستطيع التطبيق المنشور قبول أي تحديث جديد.

---

## إصدار نسخة جديدة

### الخطوات

1. **حدّث رقم الإصدار** في الملفين:

   `reports-app/package.json`

   ```json
   "version": "0.2.0"
   ```

   `reports-app/src-tauri/tauri.conf.json`

   ```json
   "version": "0.2.0"
   ```

   `reports-app/src-tauri/Cargo.toml`

   ```toml
   version = "0.2.0"
   ```

2. **commit التغييرات:**

   ```powershell
   git add -A
   git commit -m "Release v0.2.0"
   git push
   ```

3. **أنشئ tag وادفعه:**

   ```powershell
   git tag v0.2.0
   git push origin v0.2.0
   ```

4. **انتظر اكتمال GitHub Actions** (~15 دقيقة لأول بناء، ثم أسرع بفضل cache):

   ```text
   https://github.com/Hadani0mar/al-tabi/actions
   ```

5. **تحقق من Release:**

   ```text
   https://github.com/Hadani0mar/al-tabi/releases
   ```

   يجب أن ترى ملفات:
   - `al-tabi_0.2.0_x64-setup.exe` — مثبّت Windows
   - `al-tabi_0.2.0_x64-setup.nsis.zip` — حزمة التحديث
   - `al-tabi_0.2.0_x64-setup.nsis.zip.sig` — توقيع
   - `latest.json` — بيان التحديث للتطبيق

---

## كيف يحصل المستخدم على التحديث

داخل التطبيق:

```text
الإعدادات → التحديثات → تحقق من التحديثات → تثبيت
```

التطبيق سيُنزّل التحديث، يتحقق من التوقيع، يثبّته، ويعيد التشغيل تلقائياً.

---

## نشر النسخة الأولى (v0.1.0)

```powershell
cd c:\Users\DELL\Desktop\al-tabi
git add -A
git commit -m "Setup updater + GitHub Actions"
git push
git tag v0.1.0
git push origin v0.1.0
```

ثم وزّع ملف `al-tabi_0.1.0_x64-setup.exe` من Releases على المستخدمين كتثبيت أوّلي. كل التحديثات بعدها ستتم تلقائياً من داخل التطبيق.
