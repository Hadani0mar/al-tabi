# al-tabi — موجّه الهوية البصرية ونظام التصميم

> **الغرض من هذا الملف:** إدخال لأداة توليد Design System / Visual Identity.  
> **المنتج:** تطبيق سطح مكتب (Windows) — `al-tabi` v0.1.x  
> **اللغة الأساسية للواجهة:** العربية (RTL)  
> **القطاع:** ERP / توزيع أدوية / محاسبة وتقارير (Marketing2026 + SQL Server)

---

## 1. ملخص المنتج (Elevator Pitch)

**al-tabi** تطبيق سطح مكتب ذكي يربط مباشرة بقاعدة بيانات ERP المحلية (Marketing2026 على SQL Server) ويحوّل أسئلة المستخدم بالعربية إلى تقارير فورية — نص، PDF، Excel — عبر وكيل AI متخصص في المحاسبة والمخزون والمبيعات.

لا يحل محل ERP؛ بل **طبقة ذكاء فوقه**: يسأل المستخدم بلغته، والتطبيق يفهم الجداول، ينفّذ الاستعلامات بأمان (قراءة فقط)، يجدول التقارير، ويرسلها إلى Telegram عند الحاجة.

---

## 2. ماذا يفعل التطبيق؟ (Capabilities)

| القدرة | الوصف للمستخدم |
|--------|----------------|
| **محادثة AI** | «كم مبيعات اليوم؟»، «ديون الموردين»، «أصناف قاربت على النفاد» → استعلام + جدول + تحليل |
| **تقارير مجدولة** | جدولة SQL دورية (يومي/أسبوعي/…) + إشعار محلي + إرسال تلقائي لبوت Telegram |
| **بحث وتقارير جاهزة** | تقارير محفوظة من Supabase + بحث عام في البيانات |
| **محفوظات** | حفظ استعلامات ناجحة وتشغيلها بنقرة دون AI |
| **تصدير** | PDF و Excel بعناوين أعمدة عربية |
| **بوت Telegram** | نفس قدرات الوكيل من الهاتف |
| **إعدادات** | اتصال SQL، Telegram، ثيمات، تحديثات، بيانات النشاط التجاري |
| **ذاكرة ذكية** | حقائق schema مشتركة + تفضيلات محلية — دون تخزين بيانات تشغيل حساسة |

**تدفق أساسي:** اتصال بقاعدة البيانات → سؤال بالعربية → AI ينفّذ → نتيجة + خيار حفظ/تصدير/جدولة.

---

## 3. لمن موجّه؟ (Target Audience)

### Primary
- **مالك أو مدير** شركة توزيع أدوية / تجارة جملة (5–50 موظف)
- **محاسب أو مراجع** يحتاج تقارير سريعة دون بناء SQL يدوياً
- **مدير مخزون / مشتريات** يتابع النواقص، الصلاحية، طلبيات الشراء

### Secondary
- **IT محلي** يثبت التطبيق ويربط SQL Server
- **مستخدم Telegram** يريد تقارير خارج المكتب

### Demographics & Context
- **السوق:** ليبيا والمنطقة العربية (عملة: د.ل، تواريخ، أسماء عربية)
- **البيئة:** Windows desktop، شبكة محلية أو VPN للـ SQL
- **مستوى تقني:** منخفض إلى متوسط — لا يعرف SQL
- **الوقت:** قرارات يومية تحت ضغط (مبيعات، ديون، مخزون)

### Pain Points يحلّها التطبيق
- ERP قديم واجهته ثقيلة؛ التقرير يحتاج خبير
- Excel يدوي متكرر كل صباح
- صعوبة تذكّر أسماء الجداول والأعمدة
- رغبة في «اسأل وخذ الجواب» بدل قوائم وقوائم فرعية

---

## 4. شخصية العلامة (Brand Personality)

### Archetype
**«المرشد الخبير الهادئ»** — ليس روبوتاً بارداً ولا مساعداً فوضوياً.

| صفة | معنىها في al-tabi |
|-----|-------------------|
| **موثوق** | أرقام دقيقة، لا يخترع بيانات، يعترف إن لم يجد |
| **عملي** | يركز على القرار: «ماذا تفعل الآن؟» |
| **محترم** | يخاطب المستخدم باحترام دون تكلّف |
| **سريع** | واجهة خفيفة، ردود مباشرة، أقل خطوات |
| **محلي** | عربي أولاً، يفهم مصطلحات التجارة المحلية |
| **حذر** | read-only، لا يلعب بإعدادات ERP |

### ليس هذا
- ❌ لعب أطفال / cartoon mascots
- ❌ «ستارتاپ سيليكون فالي» مبالغ فيه
- ❌ cyberpunk عدواني أو hacker aesthetic
- ❌ ERP enterprise رمادي ممل من 2005
- ❌ chatbot عام مثل ChatGPT

---

## 5. نبرة الصوت (Tone of Voice)

### في الواجهة (UI Copy)
- **قصير، فعلي، مطمئن**
- عربية فصيحة بسيطة — تجنّب Anglicisms غير ضرورية
- أفعال واضحة: «تشغيل»، «حفظ»، «جدولة»، «تصدير»

**أمثلة:**
- ✅ «تم توليد التقرير — 47 صفاً»
- ✅ «جارٍ الاتصال بقاعدة البيانات…»
- ✅ «لا توجد نتائج في هذه الفترة»
- ❌ «WOW! لقد أنشأنا لك تقريراً رائعاً! 🎉»
- ❌ «Error 0x80004005»

### في ردود الوكيل AI
- **محلل senior** يشرح بلغة تجارة
- يذكر الأرقام مع **د.ل** عند الحاجة
- يلخّص ثم يعطي تفاصيل عند الطلب
- لا يسأل أسئلة تأكيد كثيرة — ينفّذ بالافتراضات الذكية

### Emotional Tone
| الموقف | الشعور المطلوب |
|--------|----------------|
| تسجيل الدخول | أمان، بساطة |
| انتظار AI | صبر هادئ — «يعمل» لا «يعلّق» |
| نتيجة ناجحة | إنجاز، وضوح |
| خطأ | صريح، حل مقترح — بدون لوم المستخدم |
| جدولة | راحة — «النظام يتابع عنك» |

---

## 6. الرسالة والقيم (Messaging)

### Tagline مقترحة (اختر أو ادمج)
1. **«اسأل ERP — واحصل على التقرير»**
2. **«ذكاء التقارير فوق Marketing2026»**
3. **«من السؤال إلى Excel في دقائق»**
4. **«محاسبتك تتكلم معك»**

### Value Propositions
1. **سرعة القرار** — ديون، مبيعات، مخزون بدون SQL
2. **أمان** — قراءة فقط، بيانات على جهازك وشبكتك
3. **عربية أصيلة** — RTL، أعمدة مترجمة، مصطلحات محلية
4. **استمرارية** — جدولة + Telegram + محفوظات
5. **ذكاء متخصص** — ليس chat عام؛ مدرب على schema ERP

---

## 7. السياق البصري الحالي (Baseline)

| عنصر | الحالة الحالية |
|------|----------------|
| **Platform** | Tauri desktop، نافذة ~800×600+ |
| **Layout** | RTL، Cairo font، bottom nav bar (LumaBar) |
| **Navigation** | 6 تبويبات: تقارير، بحث، تنبيهات، ذكاء، محفوظات، إعدادات |
| **Motion** | Framer Motion — glow يتبع التبويب النشط |
| **Themes** | 3: Default (indigo/violet)، Elegant Luxury (burgundy/gold)، Cosmic Night |
| **Components** | Tailwind، glassmorphism خفيف، gradients بنفسجية |
| **AI tab** | أيقونة `/ai.svg` مميزة في الشريط السفلي |

---

## 8. توجيهات Design System (للأداة المولّدة)

### Visual Direction المطلوب
- **Professional + Modern + Arabic-first**
- **Trust & Clarity** أهم من **Wow**
- Desktop-native (ليس mobile-first)
- كثافة معلومات **متوسطة** — جداول وأرقام كثيرة
- **Contrast جيد** للقراءة الطويلة
- دعم **Dark mode** (موجود جزئياً عبر themes)

### Color Psychology
- **Primary:** indigo/violet — ثقة، تقنية، ذكاء (ليس أزرق bank قاتم)
- **Accent:** للإجراءات والـ AI — تمييز واضح
- **Success / Warning / Error:** للتنبيهات والجدولة
- **Neutral:** خلفيات هادئة — الأرقام هي البطل

### Typography
- **Arabic:** Cairo (or similar) — أوزان 400–700
- **Latin/Numbers in tables:** monospace أو tabular nums للمحاذاة
- **Hierarchy:** عناوين قصيرة، body 14–16px، tables 13–14px

### Iconography
- Lucide-style خطوط نظيفة
- AI tab: يمكن icon مخصص — «دماغ/نجمة» دون cliché روبوت
- تجنّب emoji كعناصر UI أساسية

### Spacing & Shape
- **Rounded:** lg–2xl للبطاقات والشريط السفلي
- **Radius full** للـ bottom nav pill
- Padding سخي في forms؛ compact في tables

### Components Priority (Design System)
1. Buttons (primary / secondary / ghost / danger)
2. Input + SQL connection form
3. Data table (sort, empty, loading)
4. Chat bubbles (user / assistant / tool status)
5. Cards (report, schedule, notification)
6. Bottom navigation bar
7. Toast / inline alerts
8. Modal (confirm delete schedule, etc.)
9. Settings sections
10. Badge / status (connected, running, scheduled)

---

## 9. الشاشات الرئيسية (Information Architecture)

```
[Login SQL] → [App Shell + Bottom Nav]
    ├── التقارير (Scheduler — create/edit schedules)
    ├── بحث (Generic reports catalog)
    ├── تنبيهات (Generated notifications list)
    ├── الذكاء (AI chat — always mounted)
    ├── المحفوظات (Saved queries — run without AI)
    └── الإعدادات (Account, Telegram, Themes, Updates, Business profile)
```

**شاشة الدخol:** server, database, Windows auth — أول انطباع = أمان + سرعة اتصال.

**شاشة AI:** الأكثر استخداماً — chat + نتائج tabular + أزرار export/save.

---

## 10. Competitive Frame

| البديل | ضعفه | موقف al-tabi |
|--------|------|--------------|
| ERP built-in reports | بطيء، يحتاج training | «اسأل بالعربية» |
| Excel manual | متعب، أخطاء | AI + export |
| Power BI | setup ثقيل | desktop خفيف، SQL مباشر |
| ChatGPT generic | لا يعرف schema | متخصص Marketing2026 |

---

## 11. Do's & Don'ts للمصمم / AI

### Do
- ✅ RTL native — mirrors، محاذاة أرقام، icons
- ✅ Empty states مفيدة («لا تقارير مجدولة — أنشئ واحداً»)
- ✅ Loading states للـ AI (ثوانٍ إلى دقيقة)
- ✅ تمييز بصري بين «بيانات حقيقية» و«رد AI نصي»
- ✅ Accessibility: focus rings، contrast WCAG AA

### Don't
- ❌ Illustrations طفولية أو stock photos
- ❌ Neon hacker / matrix green
- ❌ Over-glassmorphism ي obscures tables
- ❌ English-first layouts
- ❌ Too many colors — ERP users need calm

---

## 12. Brand Name Notes

| الاسم | ملاحظة |
|-------|--------|
| **al-tabi** | الاسم التقني / package (`com.dell.reports-app`) |
| **التابي** | للعرض العربي — قصير، محلي، سهل النطق |
| **بدائل display name** | «التابي للتقارير»، «Tabi Reports» للسوق الإنجليزي |

**Logo direction:** monogram «ت» أو «T» + chart/spark line — minimal، يعمل 32px و128px.

---

## 13. Prompt جاهز لأداة توليد Design System

انسخ ما يلي إلى أداة التصميم:

```text
Design a complete visual identity and design system for "al-tabi" (التابي):
an Arabic-first RTL desktop reporting app (Tauri/Windows) that connects to
Marketing2026 ERP (SQL Server) and uses AI to answer business questions in Arabic
(sales, inventory, debts, purchases) and export PDF/Excel.

Audience: pharmacy distributors & wholesale accountants in Libya/MENA — non-technical,
time-pressed, need trust and clarity.

Personality: calm expert advisor — professional, practical, trustworthy, locally grounded.
NOT playful, NOT cyberpunk, NOT generic chatbot.

Tone: concise Arabic UI copy; reassuring loading states; honest errors.

Visual: modern professional SaaS-meets-desktop; indigo/violet intelligence accent;
support 3 theme variants (default, elegant luxury burgundy/gold, cosmic night purple).
Key UI: bottom pill navigation, data tables, AI chat, scheduler cards, SQL login.

Deliver: color tokens, typography (Cairo Arabic), spacing, radius, shadows,
component specs (buttons, inputs, tables, chat, nav bar, cards, toasts),
light/dark, RTL guidelines, logo concept, app icon 512px.
```

---

## 14. مراجع داخل المشروع

- `src/lib/themes.ts` — الثيمات الحالية
- `src/components/ui/futuristic-nav.tsx` — bottom navigation
- `src/components/ui/ai-assistant-interface.tsx` — chat UI
- `AGENTS.md` — قدرات الوكيل والمصطلحات

---

*آخر تحديث: 2026-05-27 — للاستخدام مع أدوات توليد Design System*
