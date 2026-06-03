# QUERY_PATTERNS — (نسخة قديمة — Marketing2026 فقط)

> ⚠️ **InfinityRetailDB:** استخدم [`AGENT_InfinityRetailDB.md`](./AGENT_InfinityRetailDB.md) — لا تستخدم هذا الملف.  
> ⚠️ **Marketing2026:** المصدر الم preferred هو [`AGENT_Marketing2026.md`](./AGENT_Marketing2026.md).  
> ⚠️ **معمارية النظامين:** [`ERP_ARCHITECTURE.md`](./ERP_ARCHITECTURE.md)

# Marketing2026 | شركة توزيع الأدوية
#
# كيفية الاستخدام (للوكيل الذكي):
#
# 1) search_query_patterns(keywords) — يُعيد نص النمط (حتى قسمين) للقراءة والتعديل.
# 2) run_query_pattern(keywords, days_recent?, coverage_days?, product_filter?) — يبحث، يستخرج SQL، ينفّذ.
# 3) plan_complex_query(question, product_filter?, ...) — يرسم خطة خطوات (Mermaid + SQL جاهز لكل خطوة).
# 4) execute_query_plan(steps[]) — ينفّذ الخطة خطوة بخطوة ويجمع النتائج.
# 5) get_database_views() — Views وقواعد الربط (SALE_ITEMS_INVOICE_VIEW، الموظفين، anti-patterns).
#
# أمثلة:
#   search_query_patterns("طلبية شراء ذكية")
#   run_query_pattern("متابعة الديون")
#   run_query_pattern("طلبية شراء", days_recent=45, coverage_days=20)
#
# بعد تنفيذ ناجح: export_last_result(title, format=pdf|excel)
# قبل تنفيذ SQL جديد: validate_sql(sql_query)
#
# ⚠️ قاعدة صارمة: جميع الاستعلامات هنا تبدأ بـ WITH أو SELECT مباشرةً — لا DECLARE.
#    هذا يجعلها متوافقة مع execute_raw_sql وcreate_excel_report وcreate_pdf_report.
#    إذا أراد المستخدم تغيير معامل (مثل 60 يوماً → 30) فعدّل الرقم مباشرةً في SQL.
#
# ════════════════════════════════════════════════════════════════════
# 📋 قاعدة ترجمة أسماء الأعمدة (إلزامية لجميع التقارير)
# ════════════════════════════════════════════════════════════════════
# عند توليد أي تقرير PDF أو Excel، يجب ترجمة كل اسم عمود قبل تمريره للأداة.
# لا تُمرّر مسميات قاعدة البيانات (ITEM_NAME, QTY, PRICE...) مطلقاً.
#
# جدول الترجمة الرئيسي:
#   ITEM_NAME       → اسم المنتج        ITEM_MODEL      → الكود
#   QTY             → الكمية            PRICE           → السعر
#   LAST_COST       → آخر تكلفة         AVER_COST       → متوسط التكلفة
#   S_DATE          → تاريخ البيع       B_DATE          → تاريخ الشراء
#   CUST_NAME       → اسم العميل        FULL_NAME       → اسم الموظف
#   STORE_NAME      → المخزن            STORE_ID        → رقم المخزن
#   G_VALUE         → المبلغ المدفوع    T_VALUE         → المبلغ المحصَّل
#   G_DATE          → تاريخ الدفع       T_DATE          → تاريخ التحصيل
#   BAISC_SALARY    → الراتب الأساسي    OVER_TIME       → العمل الإضافي
#   BONCE           → المكافأة          BORROW_DISCOUNT → خصم السلفة
#   PENALTY         → خصم الجزاء        S_STATUES       → الحالة
#   DailyRate       → معدل البيع/يوم    CoverageDays    → أيام التغطية
#   SuggestedQty    → الكمية المقترحة   StockQty        → المخزون الحالي
#   SoldQty         → الكمية المباعة    LastPurchasePrice → آخر سعر شراء
#   TotalDebt       → إجمالي الدَّين    PaidAmount      → المدفوع
#   RemainingDebt   → المتبقي           ExpiryDate      → تاريخ انتهاء الصلاحية
#   BatchNo         → رقم الدفعة        SaleName        → اسم المندوب
#
# ════════════════════════════════════════════════════════════════════
# 🤖 نمط Anthropic للاستعلامات والتحليل (Anthropic Prompting Pattern)
# ════════════════════════════════════════════════════════════════════
# عند الإجابة على أسئلة تحليلية أو تقديم نصائح، اتبع هذا الهيكل:
#
# <thinking>
#   1. ما الذي يطلبه المستخدم فعلاً؟
#   2. هل أحتاج get_current_datetime أولاً؟
#   3. هل هذا استعلام معقد → run_query_pattern أو search_query_patterns؟
#   4. ما الجداول/الأعمدة المطلوبة؟
#   5. هل يوجد أي تحذير أو ملاحظة مهمة في البيانات؟
# </thinking>
#
# <answer>
#   - بيانات واضحة ومنظمة بعناوين وقوائم
#   - أرقام دقيقة مع وحدات (د.ل، قطعة، يوم...)
#   - توصية عملية إن كان السياق يستدعيها
#   - اقتراح استعلام مكمّل إن كان مفيداً
# </answer>

---

## PATTERN: طلبية-شراء-ذكية
TRIGGERS: طلبية شراء, ماذا أشتري, شراء ذكي, أيام تغطية, كم يكفي المخزون, سرعة البيع, نفاد, أولويات الشراء, تحليل نواقص مع كمية مقترحة, purchase order, smart purchase, stock coverage days, suggested buy qty
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES: استخدم هذا النمط عندما يريد المستخدم كمية شراء مقترحة أو "كم يوم يكفي المخزون". للمراقبة فقط بدون كمية مقترحة استخدم نمط متابعة-النواقص. القيم الافتراضية: 60 يوم نافذة مبيعات، 30 يوم تغطية مستهدفة. تاريخ المرجع = MAX(S_DATE) وليس GETDATE(). لتغيير المعاملات: استبدل 60 أو 30 مباشرةً في SQL.
---

**الصيغ الأساسية:**
- الرصيد = SUM(ITEMS_SUB.QTY) لكل ITEM_ID
- صافي المبيعات = SALE_ITEMS مطروحاً منه R_S_ITEMS (المردودات بكمية سالبة)
- معدل يومي = SoldQty / CAST(ActiveSaleDays AS float)  ← مهم: يجب CAST لـ float منعاً لقسمة صحيحة
- أيام التغطية = StockQty / DailyRate
- كمية الشراء المقترحة = MAX(0, DailyRate × 30 − StockQty)
- الأولوية: رصيد=0 ومبيعات>0 → "نفاد" | أيام<7 → "حرج" | أيام<30 → "شراء" | غير ذلك → "كافٍ"

**الأعمدة المُخرجة:** الكود، اسم المنتج، رصيد المخزون، مبيعات آخر 60 يوم، أيام بيع فعلية، معدل يومي، أيام تغطية الرصيد، كمية الشراء المقترحة، الأولوية، آخر سعر شراء، آخر مورد

```sql
-- طلبية شراء ذكية (60 يوم نافذة، 30 يوم تغطية مستهدفة)
-- لتغيير النافذة: استبدل 60. لتغيير التغطية: استبدل 30
;WITH
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT ITEM_ID, SUM(QTY) SoldQty,
         COUNT(DISTINCT CAST(S_DATE AS date)) ActiveSaleDays,
         MAX(S_DATE) LastSaleDate
  FROM (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X GROUP BY ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE LastBuyPrice, CU.CUST_NAME LastSupplier
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  WHERE BI.B_ITEM_ID IN (
    SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
    JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID=B2.B_ID GROUP BY BI2.ITEM_ID
  )
)
SELECT TOP 50 I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ISNULL(S.StockQty,0) AS Stock,
  SR.SoldQty, SR.ActiveSaleDays,
  CAST(SR.SoldQty / NULLIF(CAST(SR.ActiveSaleDays AS float),0) AS decimal(10,2)) AS DailyRate,
  CAST(ISNULL(S.StockQty,0) / NULLIF(SR.SoldQty / NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) AS decimal(10,1)) AS DaysCoverage,
  CASE
    WHEN ISNULL(S.StockQty,0)<=0 AND SR.SoldQty>0 THEN N'نفاد'
    WHEN ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) < 7 THEN N'حرج'
    WHEN ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) < 30 THEN N'شراء'
    ELSE N'كافٍ'
  END AS Priority,
  CAST(CASE
    WHEN (SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0))*30 - ISNULL(S.StockQty,0) < 0 THEN 0
    ELSE (SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0))*30 - ISNULL(S.StockQty,0)
  END AS decimal(10,1)) AS SuggestedBuy,
  LB.LastBuyPrice, LB.LastSupplier
FROM dbo.ITEMS I
JOIN SalesRecent SR ON I.ITEM_ID=SR.ITEM_ID
LEFT JOIN Stock S ON I.ITEM_ID=S.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID=LB.ITEM_ID
WHERE I.ITEM_INVISIBLE=0 AND SR.SoldQty>0
  AND (
    ISNULL(S.StockQty,0) <= I.MIN_LEVEL
    OR ISNULL(S.StockQty,0) = 0
    OR ISNULL(S.StockQty,0)/NULLIF(SR.SoldQty/NULLIF(CAST(SR.ActiveSaleDays AS float),0),0) < 30
  )
ORDER BY
  CASE WHEN ISNULL(S.StockQty,0)<=0 THEN 0 ELSE 1 END,
  DaysCoverage ASC,
  SR.SoldQty DESC;
```
ملف SQL الكامل المختبر: `reports-app/smart_purchase_order.sql`

---

## PATTERN: متابعة-النواقص
TRIGGERS: متابعة النواقص, قائمة النواقص, أصناف نافدة, تحت الحد الأدنى, فجوة المخزون, مراقبة المخزون, نواقص, shortage monitoring, items below min level, stock gap
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE
NOTES: للمراقبة فقط (الحالة + الفجوة مقابل MIN_LEVEL). لكمية الشراء المقترحة استخدم نمط طلبية-شراء-ذكية.
---

**القواعد:**
- الرصيد = SUM(ITEMS_SUB.QTY) لكل ITEM_ID
- صافي المبيعات = SALE_ITEMS مطروحاً منه R_S_ITEMS (مردودات) في آخر 60 يوم من MAX(S_DATE)
- فجوة النقص = MIN_LEVEL − Stock عندما MIN_LEVEL > 0
- الحالة: رصيد=0 + مبيعات>0 → "نفاد" | رصيد=0 → "نفاد بدون مبيعات" | رصيد≤MIN_LEVEL → "تحت الحد الأدنى" | رصيد < MIN_LEVEL×1.25 + مبيعات>0 → "قريب من النفاد"
- الترتيب: نفاد أولاً، ثم مبيعات حديثة تنازلياً

```sql
-- متابعة النواقص (60 يوم نافذة مبيعات)
-- لتغيير النافذة: استبدل 60
;WITH
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT ITEM_ID, SUM(ISNULL(QTY,0)) StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
  SELECT ITEM_ID, SUM(QTY) SoldQty, MAX(S_DATE) LastSaleDate
  FROM (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X GROUP BY ITEM_ID
)
SELECT TOP 100
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ISNULL(S.StockQty,0) AS Stock,
  I.MIN_LEVEL, I.MAX_LEVEL,
  CASE WHEN I.MIN_LEVEL>0 THEN I.MIN_LEVEL - ISNULL(S.StockQty,0) ELSE 0 END AS ShortageGap,
  ISNULL(SR.SoldQty,0) AS RecentSales,
  SR.LastSaleDate,
  CASE
    WHEN ISNULL(S.StockQty,0)<=0 AND ISNULL(SR.SoldQty,0)>0 THEN N'نفاد'
    WHEN ISNULL(S.StockQty,0)<=0 THEN N'نفاد بدون مبيعات'
    WHEN I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<=I.MIN_LEVEL THEN N'تحت الحد الأدنى'
    WHEN I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0)<I.MIN_LEVEL*1.25 AND ISNULL(SR.SoldQty,0)>0 THEN N'قريب من النفاد'
    ELSE N'مراقبة'
  END AS ShortageStatus
FROM dbo.ITEMS I
LEFT JOIN Stock S ON I.ITEM_ID=S.ITEM_ID
LEFT JOIN SalesRecent SR ON I.ITEM_ID=SR.ITEM_ID
WHERE I.ITEM_INVISIBLE=0
  AND (
    ISNULL(S.StockQty,0) <= 0
    OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) <= I.MIN_LEVEL)
    OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) < I.MIN_LEVEL*1.25 AND ISNULL(SR.SoldQty,0)>0)
  )
ORDER BY
  CASE WHEN ISNULL(S.StockQty,0)<=0 THEN 0 ELSE 1 END,
  ISNULL(SR.SoldQty,0) DESC;
```
ملف SQL الكامل المختبر: `reports-app/shortage_tracking.sql`

---

## PATTERN: نواقص-نشطة-مورد
TRIGGERS: نواقص نشطة, منتجات ناقصة تباع, أصناف ناقصة نشطة, نواقص بمورد, shortage active selling, active shortages supplier, منتجات نافدة ومبيعات, نواقص آخر سعر شراء
TABLES: ITEMS, ITEMS_SUB, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - **نشطة** = مبيعات صافية > 0 في آخر 60 يوم (من MAX(S_DATE)).
  - **ناقصة** = رصيد ≤ 0 أو ≤ MIN_LEVEL أو < MIN_LEVEL×1.25.
  - آخر سعر شراء من آخر BUY_ITEMS؛ إن لم يوجد → ITEMS.LAST_COST.
  - EXPENCES_ID=0 في GIVE ليس له علاقة — المورد من BUY_INVOICE.CUST_ID → CUSTOMERS.
  - ملف مُختبَر: `reports-app/active_shortage_tracking.sql`
  - للمراقبة بدون مورد/سعر استخدم نمط متابعة-النواقص.
---

```sql
DECLARE @DaysRecent int = 60;
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @RecentFrom date = DATEADD(day, -@DaysRecent, @AsOfDate);
;WITH Stock AS (
    SELECT ITEM_ID, SUM(ISNULL(QTY, 0)) AS StockQty FROM dbo.ITEMS_SUB GROUP BY ITEM_ID
),
SalesRecent AS (
    SELECT X.ITEM_ID, SUM(X.QTY) AS SoldQty, MAX(X.S_DATE) AS LastSaleDate
    FROM (
        SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE FROM dbo.SALE_ITEMS SI
        INNER JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
        WHERE CAST(INV.S_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
        UNION ALL
        SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE FROM dbo.R_S_ITEMS RSI
        INNER JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
        WHERE CAST(RINV.S_R_DATE AS date) BETWEEN @RecentFrom AND @AsOfDate
    ) X GROUP BY X.ITEM_ID
),
LastBuy AS (
    SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice, CU.CUST_NAME AS LastSupplier
    FROM dbo.BUY_ITEMS BI INNER JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
    WHERE BI.B_ITEM_ID IN (
        SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
        INNER JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID = B2.B_ID GROUP BY BI2.ITEM_ID
    )
)
SELECT TOP 150
    LEFT(I.ITEM_NAME, 80) AS [اسم المنتج],
    CAST(ISNULL(S.StockQty, 0) AS decimal(18,2)) AS [الكمية],
    CAST(COALESCE(LB.LastBuyPrice, I.LAST_COST, 0) AS decimal(18,2)) AS [آخر سعر شراء],
    ISNULL(LB.LastSupplier, N'—') AS [المورد],
    CAST(ISNULL(SR.SoldQty, 0) AS decimal(18,2)) AS [مبيعات النافذة]
FROM dbo.ITEMS I
INNER JOIN SalesRecent SR ON I.ITEM_ID = SR.ITEM_ID AND SR.SoldQty > 0
LEFT JOIN Stock S ON I.ITEM_ID = S.ITEM_ID
LEFT JOIN LastBuy LB ON I.ITEM_ID = LB.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (ISNULL(S.StockQty,0) <= 0 OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) <= I.MIN_LEVEL)
       OR (I.MIN_LEVEL>0 AND ISNULL(S.StockQty,0) < I.MIN_LEVEL*1.25))
ORDER BY CASE WHEN ISNULL(S.StockQty,0) <= 0 THEN 0 ELSE 1 END, SR.SoldQty DESC;
```

---

## PATTERN: متابعة-الديون
TRIGGERS: ديون, متابعة الديون, رصيد الزبائن, ديون الموردين, اللي لي, اللي علي, مقبوضات, مدفوعات, ما في الذمة, حسابات العملاء, debts receivable, debts payable, customer balance, supplier balance
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, BUY_INVOICE, BUY_ITEMS, B_R_INVOICE, B_R_ITEMS, TAKE, GIVE, BALANCE_EDIT
NOTES: جدول BALANCE_C فارغ في هذه القاعدة — لا تستخدمه أبداً. احسب الأرصدة دائماً من الفواتير + المدفوعات. TAKE = مقبوضات من الزبائن (T_STATUES: 0=مسودة، 1=مؤكد، 2=تم). GIVE = مدفوعات للموردين (G_STATUES: 0=مسودة، 1=مؤكد). لا تفلتر بالحالة — كل المدفوعات المدخلة محتسبة.
---

**الجداول:**
- مبيعات → SALE_INVOICE + SALE_ITEMS (قيمة السطر = QTY×PRICE)
- مردودات مبيعات → R_S_INVOICE + R_S_ITEMS
- مقبوضات من الزبائن → TAKE (T_VALUE, CUST_ID, T_DATE)
- مشتريات → BUY_INVOICE + BUY_ITEMS
- مردودات مشتريات → B_R_INVOICE + B_R_ITEMS
- مدفوعات للموردين → GIVE (G_VALUE, CUST_ID, G_DATE)
- تسويات/رصيد افتتاحي → BALANCE_EDIT (BL_DEBIT, BL_CREDIT لكل CUST_ID)

**الصيغ:**
- لي (زبون مدين) CUST_CUSTOM=1: Remaining = Sales − SaleReturns − TAKE + AdjBalance
- علي (مورد دائن) CUST_VENDOR=1: Remaining = Buys − BuyReturns − GIVE + AdjBalance
- فلتر: Remaining >= 1
- الترتيب: نوع الدين، ثم Remaining تنازلياً

```sql
-- متابعة الديون: "لي" (زبائن مدينون) و"علي" (موردون دائنون)
;WITH
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) SalesValue, MAX(SI.S_DATE) LastSaleDate
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) ReturnValue
  FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) PaidValue, MAX(T_DATE) LastTakeDate
  FROM dbo.TAKE GROUP BY CUST_ID
),
BuyTot AS (
  SELECT B.CUST_ID, SUM(BI.QTY*BI.PRICE) BuyValue, MAX(B.B_DATE) LastBuyDate
  FROM dbo.BUY_INVOICE B JOIN dbo.BUY_ITEMS BI ON B.B_ID=BI.B_ID GROUP BY B.CUST_ID
),
BuyReturnTot AS (
  SELECT BR.CUST_ID, SUM(BRI.QTY*BRI.PRICE) ReturnValue
  FROM dbo.B_R_INVOICE BR JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID=BRI.B_R_ID GROUP BY BR.CUST_ID
),
GiveTot AS (
  SELECT CUST_ID, SUM(G_VALUE) PaidValue, MAX(G_DATE) LastGiveDate
  FROM dbo.GIVE GROUP BY CUST_ID
),
Receivables AS (
  SELECT N'لي — زبون مدين' AS DebtType, C.CUST_NO, C.CUST_NAME,
    ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS Remaining,
    ISNULL(ST.SalesValue,0) AS TotalMovement, ISNULL(TT.PaidValue,0) AS TotalSettled,
    C.CUST_MAX_DEBIT, ST.LastSaleDate, TT.LastTakeDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN SaleTot ST ON C.CUST_ID=ST.CUST_ID
  LEFT JOIN SaleReturnTot SRT ON C.CUST_ID=SRT.CUST_ID
  LEFT JOIN TakeTot TT ON C.CUST_ID=TT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
  WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0
    AND ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) >= 1
),
Payables AS (
  SELECT N'علي — مورد دائن' AS DebtType, C.CUST_NO, C.CUST_NAME,
    ISNULL(BT.BuyValue,0)-ISNULL(BRT.ReturnValue,0)-ISNULL(GT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS Remaining,
    ISNULL(BT.BuyValue,0) AS TotalMovement, ISNULL(GT.PaidValue,0) AS TotalSettled,
    C.CUST_MAX_DEBIT, BT.LastBuyDate, GT.LastGiveDate
  FROM dbo.CUSTOMERS C
  LEFT JOIN BuyTot BT ON C.CUST_ID=BT.CUST_ID
  LEFT JOIN BuyReturnTot BRT ON C.CUST_ID=BRT.CUST_ID
  LEFT JOIN GiveTot GT ON C.CUST_ID=GT.CUST_ID
  LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
  WHERE C.CUST_VENDOR=1 AND C.CUST_INVISIBLE=0
    AND ISNULL(BT.BuyValue,0)-ISNULL(BRT.ReturnValue,0)-ISNULL(GT.PaidValue,0)+ISNULL(BA.AdjBalance,0) >= 1
)
SELECT TOP 100
  DebtType, CUST_NO, CUST_NAME,
  CAST(Remaining AS decimal(18,2)) AS Remaining,
  CAST(TotalMovement AS decimal(18,2)) AS TotalMovement,
  CAST(TotalSettled AS decimal(18,2)) AS TotalSettled,
  CUST_MAX_DEBIT, LastSaleDate, LastTakeDate
FROM (
  SELECT DebtType,CUST_NO,CUST_NAME,Remaining,TotalMovement,TotalSettled,
         CUST_MAX_DEBIT,LastSaleDate,LastTakeDate FROM Receivables
  UNION ALL
  SELECT DebtType,CUST_NO,CUST_NAME,Remaining,TotalMovement,TotalSettled,
         CUST_MAX_DEBIT,LastBuyDate,LastGiveDate FROM Payables
) D
ORDER BY DebtType, Remaining DESC;
```
ملف SQL الكامل المختبر: `reports-app/debts_tracking.sql`

---

## PATTERN: ديون-الموردين-مبسط
TRIGGERS: ديون الموردين, ديون موردين, ديون الموردين فقط, تقرير ديون الموردين, الذي علي للموردين, اللي علي للموردين, كم علي للموردين, supplier debts, supplier balances only, vendor debts simple
TABLES: CUSTOMERS, BUY_INVOICE, BUY_ITEMS, B_R_INVOICE, B_R_ITEMS, GIVE, BALANCE_EDIT
NOTES: نسخة مبسّطة من «متابعة الديون» — تعرض ديون الموردين فقط بعمودين: اسم المورد، والدين. تخدم الحالات التي لا يحتاج فيها المستخدم تفاصيل المقبوضات/التسويات/التواريخ. الصيغة هي: مشتريات − مردودات مشتريات − GIVE + تسوية BALANCE_EDIT.
---

**الأعمدة:** فقط `اسم المورد` و `الدين` — لا تضف أعمدة أخرى مهما كان السياق.

```sql
-- ديون الموردين فقط: اسم المورد + الدين (د.ل)
;WITH
BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AS AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
BuyTot AS (
  SELECT B.CUST_ID, SUM(BI.QTY*BI.PRICE) AS BuyValue
  FROM dbo.BUY_INVOICE B JOIN dbo.BUY_ITEMS BI ON B.B_ID=BI.B_ID GROUP BY B.CUST_ID
),
BuyReturnTot AS (
  SELECT BR.CUST_ID, SUM(BRI.QTY*BRI.PRICE) AS ReturnValue
  FROM dbo.B_R_INVOICE BR JOIN dbo.B_R_ITEMS BRI ON BR.B_R_ID=BRI.B_R_ID GROUP BY BR.CUST_ID
),
GiveTot AS (
  SELECT CUST_ID, SUM(G_VALUE) AS PaidValue
  FROM dbo.GIVE GROUP BY CUST_ID
)
SELECT TOP 200
  C.CUST_NAME AS [اسم المورد],
  CAST(
    ISNULL(BT.BuyValue,0) - ISNULL(BRT.ReturnValue,0)
    - ISNULL(GT.PaidValue,0) + ISNULL(BA.AdjBalance,0)
    AS decimal(18,2)
  ) AS [الدين]
FROM dbo.CUSTOMERS C
LEFT JOIN BuyTot       BT  ON C.CUST_ID = BT.CUST_ID
LEFT JOIN BuyReturnTot BRT ON C.CUST_ID = BRT.CUST_ID
LEFT JOIN GiveTot      GT  ON C.CUST_ID = GT.CUST_ID
LEFT JOIN BalanceAdj   BA  ON C.CUST_ID = BA.CUST_ID
WHERE C.CUST_VENDOR = 1
  AND C.CUST_INVISIBLE = 0
  AND (ISNULL(BT.BuyValue,0) - ISNULL(BRT.ReturnValue,0)
       - ISNULL(GT.PaidValue,0) + ISNULL(BA.AdjBalance,0)) >= 1
ORDER BY [الدين] DESC;
```

---

## PATTERN: تقرير-الصلاحية
TRIGGERS: منتهية الصلاحية, صلاحية, تاريخ انتهاء, سينخلص قريباً, ستنتهي صلاحيتها, expiry report, expiring soon, expired products, expiry date
TABLES: ITEMS_SUB, ITEMS, STORES
NOTES: CATEOGRY3 هو عمود تاريخ الصلاحية (datetime) رغم اسمه المضلل. يوجد INDEX عليه. استخدمه دائماً من ITEMS_SUB. القيمة الافتراضية للإنذار المبكر: 90 يوم — عدّل الرقم مباشرةً.
---

```sql
-- المنتجات المنتهية الصلاحية حالياً (رصيد > 0)
SELECT TOP 50
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  S.CATEOGRY1 AS BatchNo,
  CAST(S.CATEOGRY3 AS date) AS ExpiryDate,
  S.QTY AS StockQty,
  ST.STORE_NAME,
  DATEDIFF(day, CAST(S.CATEOGRY3 AS date), CAST(GETDATE() AS date)) AS DaysExpired
FROM dbo.ITEMS_SUB S
JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON S.STORE_ID = ST.STORE_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CAST(S.CATEOGRY3 AS date) < CAST(GETDATE() AS date)
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
ORDER BY
  CASE
    WHEN YEAR(S.CATEOGRY3) = YEAR(GETDATE()) THEN 0
    WHEN YEAR(S.CATEOGRY3) < YEAR(GETDATE()) THEN 1
    ELSE 2
  END,
  S.CATEOGRY3 ASC;
```

```sql
-- المنتجات التي ستنتهي صلاحيتها خلال 90 يوماً (عدّل 90 حسب الحاجة)
SELECT TOP 50
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  S.CATEOGRY1 AS BatchNo,
  CAST(S.CATEOGRY3 AS date) AS ExpiryDate,
  S.QTY AS StockQty,
  ST.STORE_NAME,
  DATEDIFF(day, CAST(GETDATE() AS date), CAST(S.CATEOGRY3 AS date)) AS DaysRemaining
FROM dbo.ITEMS_SUB S
JOIN dbo.ITEMS I ON S.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON S.STORE_ID = ST.STORE_ID
WHERE S.CATEOGRY3 IS NOT NULL
  AND CAST(S.CATEOGRY3 AS date) >= CAST(GETDATE() AS date)
  AND CAST(S.CATEOGRY3 AS date) <= DATEADD(day, 90, CAST(GETDATE() AS date))
  AND S.QTY > 0
  AND I.ITEM_INVISIBLE = 0
ORDER BY
  CASE
    WHEN YEAR(S.CATEOGRY3) = YEAR(GETDATE()) THEN 0
    WHEN YEAR(S.CATEOGRY3) < YEAR(GETDATE()) THEN 1
    ELSE 2
  END,
  S.CATEOGRY3 ASC;
```

---

## PATTERN: تقرير-الجرد-الفعلي
TRIGGERS: جرد, جرد فعلي, جرد المخزون, مقارنة الجرد, فرق الجرد, inventory audit, physical inventory, stock count, jared
TABLES: JARED_INVOICE, JARED_ITEMS, ITEMS_SUB, ITEMS, STORES
NOTES: JARED = فواتير الجرد الفعلي للمخزون. ITEMS_SUB = المخزون النظري. المقارنة بينهما تكشف الفروقات. CATEOGRY3 في JARED_ITEMS = تاريخ الصلاحية. J_STATUES=1 يعني جرد مقفل/معتمد.
---

```sql
-- آخر جرد لكل صنف مقارنةً بالمخزون الحالي
;WITH
LastJared AS (
  SELECT
    JI.ITEM_ID, JI.STORE_ID,
    JI.QTY AS CountedQty,
    CAST(JI.CATEOGRY3 AS date) AS JaredExpiry,
    J.J_DATE, J.J_REF_DISC,
    ROW_NUMBER() OVER (PARTITION BY JI.ITEM_ID, JI.STORE_ID ORDER BY J.J_DATE DESC) AS rn
  FROM dbo.JARED_ITEMS JI
  JOIN dbo.JARED_INVOICE J ON JI.J_ID = J.J_ID
  WHERE J.J_STATUES = 1
),
CurrentStock AS (
  SELECT ITEM_ID, STORE_ID, SUM(ISNULL(QTY,0)) AS SystemQty
  FROM dbo.ITEMS_SUB GROUP BY ITEM_ID, STORE_ID
)
SELECT TOP 100
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  ST.STORE_NAME,
  LJ.CountedQty AS JaredQty,
  ISNULL(CS.SystemQty,0) AS SystemQty,
  ISNULL(CS.SystemQty,0) - ISNULL(LJ.CountedQty,0) AS Difference,
  CAST(LJ.J_DATE AS date) AS LastJaredDate,
  LJ.J_REF_DISC AS JaredRef
FROM LastJared LJ
JOIN dbo.ITEMS I ON LJ.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES ST ON LJ.STORE_ID = ST.STORE_ID
LEFT JOIN CurrentStock CS ON LJ.ITEM_ID = CS.ITEM_ID AND LJ.STORE_ID = CS.STORE_ID
WHERE LJ.rn = 1 AND I.ITEM_INVISIBLE = 0
ORDER BY ABS(ISNULL(CS.SystemQty,0) - ISNULL(LJ.CountedQty,0)) DESC;
```

---

---

## PATTERN: مقارنة-أسعار-موردين
TRIGGERS: مقارنة أسعار, مقارنة أسعار الموردين, أسعار الموردين لمنتج, أرخص مورد, أغلى مورد, فرق أسعار الشراء, supplier price comparison, compare supplier prices, buy price by vendor, product supplier prices
TABLES: ITEMS, BUY_ITEMS, BUY_INVOICE, CUSTOMERS
NOTES:
  - **لصنف واحد** — مرّر `product_filter` في `run_query_pattern` أو استبدل `%PRODUCT%` بجزء من الاسم/الكود (مثل `@PREGNYL` من الشات).
  - يختار الصنف الأنسب تلقائياً (أكثر سجل شراء) إن وُجدت عدة مطابقات في `ITEMS`.
  - **المورد** = `BUY_INVOICE.CUST_ID` → `CUSTOMERS.CUST_NAME` (CUST_VENDOR=1).
  - **السعر** = `BUY_ITEMS.PRICE` — NOT `GIVE` / NOT `ITEMS.PUBLIC_PRICE`.
  - نافذة افتراضية: آخر **36 شهراً** (`@MonthsBack=36`).
  - ترتيب النتائج: **أرخص آخر سعر أولاً** (`ترتيب السعر`).
  - للـ «آخر سعر شراء فقط» بدون مقارنة → نمط `آخر-سعر-شراء-مورد`.
  - ملف مُختبَر: `reports-app/supplier_price_comparison.sql`
  - مثال TRAMADOL NORMON: سبها 44.30، الامل الشافي 47.00، التسامي 48.00 (آخر سعر).
---

```sql
DECLARE @MonthsBack int = 36;
DECLARE @RecentFrom date = DATEADD(month, -@MonthsBack, CAST(GETDATE() AS date));
;WITH Matches AS (
    SELECT I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME,
        MAX(B.B_DATE) AS LastAnyBuy, COUNT(BI.B_ITEM_ID) AS BuyLineCount
    FROM dbo.ITEMS I
    LEFT JOIN dbo.BUY_ITEMS BI ON I.ITEM_ID = BI.ITEM_ID AND BI.PRICE > 0
    LEFT JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    WHERE I.ITEM_INVISIBLE = 0
      AND (I.ITEM_MODEL LIKE N'%PRODUCT%' OR I.ITEM_NAME LIKE N'%PRODUCT%')
    GROUP BY I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME
),
ProductPick AS (
    SELECT TOP 1 ITEM_ID, ITEM_MODEL, ITEM_NAME FROM Matches
    ORDER BY CASE WHEN BuyLineCount > 0 THEN 0 ELSE 1 END, BuyLineCount DESC, LastAnyBuy DESC,
        CASE WHEN ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END, ITEM_ID DESC
),
Purchases AS (
    SELECT PP.ITEM_ID, PP.ITEM_MODEL, PP.ITEM_NAME, B.CUST_ID, CU.CUST_NAME AS Supplier,
        BI.PRICE, B.B_DATE, BI.QTY, BI.B_ITEM_ID,
        ROW_NUMBER() OVER (PARTITION BY PP.ITEM_ID, B.CUST_ID ORDER BY B.B_DATE DESC, BI.B_ITEM_ID DESC) AS rn_last
    FROM ProductPick PP
    INNER JOIN dbo.BUY_ITEMS BI ON PP.ITEM_ID = BI.ITEM_ID
    INNER JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
    LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
    WHERE BI.PRICE > 0 AND CAST(B.B_DATE AS date) >= @RecentFrom
),
BySupplier AS (
    SELECT ITEM_ID, ITEM_MODEL, ITEM_NAME, CUST_ID, Supplier,
        COUNT(*) AS PurchaseCount,
        CAST(MIN(PRICE) AS decimal(18,2)) AS MinPrice,
        CAST(MAX(PRICE) AS decimal(18,2)) AS MaxPrice,
        CAST(AVG(PRICE) AS decimal(18,2)) AS AvgPrice,
        MAX(CASE WHEN rn_last = 1 THEN PRICE END) AS LastPrice,
        MAX(CASE WHEN rn_last = 1 THEN B_DATE END) AS LastBuyDate,
        MAX(CASE WHEN rn_last = 1 THEN QTY END) AS LastQty
    FROM Purchases GROUP BY ITEM_ID, ITEM_MODEL, ITEM_NAME, CUST_ID, Supplier
)
SELECT
    LEFT(ITEM_NAME, 70) AS [اسم المنتج], ITEM_MODEL AS [الكود],
    ISNULL(Supplier, N'—') AS [المورد],
    CAST(LastPrice AS decimal(18,2)) AS [آخر سعر شراء],
    CAST(LastBuyDate AS date) AS [آخر تاريخ شراء],
    CAST(LastQty AS decimal(18,2)) AS [آخر كمية],
    MinPrice AS [أقل سعر], MaxPrice AS [أعلى سعر], AvgPrice AS [متوسط السعر],
    PurchaseCount AS [عدد مرات الشراء],
    DATEDIFF(day, CAST(LastBuyDate AS date), CAST(GETDATE() AS date)) AS [أيام منذ آخر شراء],
    ROW_NUMBER() OVER (ORDER BY LastPrice ASC, Supplier) AS [ترتيب السعر]
FROM BySupplier
ORDER BY LastPrice ASC, Supplier;
```

---

## PATTERN: آخر-سعر-شراء-مورد
TRIGGERS: آخر سعر شراء, سعر شراء, last purchase price, buy price history, آخر مشتريات صنف
TABLES: BUY_ITEMS, BUY_INVOICE, CUSTOMERS, ITEMS
NOTES: آخر سعر شراء = `BUY_ITEMS.PRICE` من أحدث `B_ITEM_ID` لكل `ITEM_ID`. **لمقارنة الموردين على نفس الصنف** → `run_query_pattern("مقارنة أسعار موردين", product_filter=...)`.
---

```sql
-- آخر سعر شراء لكل صنف (مع المورد والتاريخ)
;WITH LastBuyRow AS (
  SELECT BI.ITEM_ID, MAX(BI.B_ITEM_ID) AS MaxBItemID
  FROM dbo.BUY_ITEMS BI GROUP BY BI.ITEM_ID
)
SELECT TOP 100
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  BI.PRICE AS LastBuyPrice,
  CAST(B.B_DATE AS date) AS LastBuyDate,
  CU.CUST_NAME AS Supplier,
  CAST(BI.CATEOGRY3 AS date) AS ExpiryDate
FROM LastBuyRow LBR
JOIN dbo.BUY_ITEMS BI ON LBR.MaxBItemID = BI.B_ITEM_ID
JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
JOIN dbo.ITEMS I ON BI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
WHERE I.ITEM_INVISIBLE = 0
ORDER BY B.B_DATE DESC;
```

---

## PATTERN: رواتب-الموظفين-بعد-الخصم
TRIGGERS: رواتب, مرتبات, الرواتب, راتب الموظف, كشف الرواتب, الراتب بعد الخصم, خصم السلفة, خصم الغياب, مكافأة, عمل إضافي, أجور, salaries, payroll, salary after deduction, net salary, bonus deduction
TABLES: SALARIES, CUSTOMERS, USERS, USER_TIME_SHEET, GIVE, TAKE
NOTES:
  - الموظفون مُخزَّنون في CUSTOMERS حيث CUST_EMP=1 ، وراتبهم الأساسي في EMP_SALARY.
  - جدول SALARIES يحتوي سجلات الراتب: BAISC_SALARY+OVER_TIME+BONCE = إجمالي. BORROW_DISCOUNT+PENALTY = خصومات. صافي = إجمالي - خصومات.
  - S_STATUES: 0=مسودة ، 1=مُعتمد ومصروف.
  - السلف (GIVE للموظف) تُخصم من الصافي عند حساب "المتبقي للصرف".
  - جدول USER_TIME_SHEET لتتبع حضور/انصراف الموظفين. PERIOD = ساعات الوردية. للحصول على شهر معين: YEAR(TRANS_DATE)=X AND MONTH(TRANS_DATE)=Y.
  - إذا كانت SALARIES فارغة: اعرض EMP_SALARY من CUSTOMERS مع سطر "لم تُدخل بعد" في حالة الراتب.
  - CAST(S.S_STATUES AS smallint) ضروري لتفادي overflow في CASE.
---

```sql
-- ===== كشف رواتب الموظفين الكامل بعد الخصم =====
-- يعرض: الراتب الأساسي + العمل الإضافي + المكافأة - خصم السلفة - خصم الغياب/الجزاء
-- إذا لم تُسجَّل رواتب بعد يعرض EMP_SALARY من CUSTOMERS كراتب مرجعي
;WITH
Employees AS (
    SELECT C.CUST_ID, C.CUST_NAME AS EmpName, ISNULL(C.EMP_SALARY, 0) AS BaseSalary
    FROM dbo.CUSTOMERS C WHERE C.CUST_EMP = 1
),
SalaryData AS (
    SELECT
        S.CUST_ID, S.S_M AS Mo, CAST(S.S_Y AS int) AS Yr,
        ISNULL(S.BAISC_SALARY,    0) AS BasicSalary,
        ISNULL(S.OVER_TIME,       0) AS OvertimePay,
        ISNULL(S.BONCE,           0) AS Bonus,
        ISNULL(S.BORROW_DISCOUNT, 0) AS LoanDeduct,
        ISNULL(S.PENALTY,         0) AS PenaltyDeduct,
        S.BONCE_REASON, S.PENALTY_REASON, S.NOTES,
        S.TOTAL_HOURS, S.HOUR_VALUE,
        CAST(S.S_STATUES AS smallint) AS S_STATUES,
        S.S_DATE
    FROM dbo.SALARIES S
    -- لتصفية شهر بعينه: أضف: WHERE S.S_M = MONTH(GETDATE()) AND S.S_Y = YEAR(GETDATE())
),
AdvancesGiven AS (
    SELECT G.CUST_ID, YEAR(G.G_DATE) Yr, MONTH(G.G_DATE) Mo,
           SUM(G.G_VALUE) AS AdvancesPaid
    FROM dbo.GIVE G
    JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID AND C.CUST_EMP = 1
    WHERE G.G_STATUES = 1
    GROUP BY G.CUST_ID, YEAR(G.G_DATE), MONTH(G.G_DATE)
)
SELECT
    E.EmpName                                                  AS [الموظف],
    ISNULL(SD.Yr,  YEAR(GETDATE()))                            AS [السنة],
    ISNULL(SD.Mo,  MONTH(GETDATE()))                           AS [الشهر],
    ISNULL(SD.BasicSalary, E.BaseSalary)                       AS [الراتب الأساسي],
    ISNULL(SD.OvertimePay, 0)                                  AS [العمل الإضافي],
    ISNULL(SD.Bonus, 0)                                        AS [المكافأة],
    ISNULL(SD.BasicSalary, E.BaseSalary)
      + ISNULL(SD.OvertimePay, 0)
      + ISNULL(SD.Bonus, 0)                                    AS [الراتب الإجمالي],
    ISNULL(SD.LoanDeduct,    0)                                AS [خصم السلفة/القرض],
    ISNULL(SD.PenaltyDeduct, 0)                                AS [خصم الغياب/الجزاء],
    ISNULL(SD.LoanDeduct, 0) + ISNULL(SD.PenaltyDeduct, 0)    AS [إجمالي الخصومات],
    ISNULL(SD.BasicSalary, E.BaseSalary)
      + ISNULL(SD.OvertimePay, 0)
      + ISNULL(SD.Bonus, 0)
      - ISNULL(SD.LoanDeduct, 0)
      - ISNULL(SD.PenaltyDeduct, 0)                            AS [صافي الراتب],
    ISNULL(AG.AdvancesPaid, 0)                                 AS [سلف مدفوعة مسبقاً],
    ISNULL(SD.BasicSalary, E.BaseSalary)
      + ISNULL(SD.OvertimePay, 0)
      + ISNULL(SD.Bonus, 0)
      - ISNULL(SD.LoanDeduct, 0)
      - ISNULL(SD.PenaltyDeduct, 0)
      - ISNULL(AG.AdvancesPaid, 0)                             AS [المتبقي للصرف],
    CASE ISNULL(SD.S_STATUES, CAST(99 AS smallint))
        WHEN 0 THEN 'مسودة'
        WHEN 1 THEN 'مُعتمد ومصروف'
        ELSE 'لم تُدخل بعد'
    END                                                        AS [حالة الراتب],
    ISNULL(SD.TOTAL_HOURS, 0)                                  AS [ساعات العمل المسجلة],
    ISNULL(SD.HOUR_VALUE,  0)                                  AS [قيمة الساعة],
    ISNULL(SD.BONCE_REASON, '')                                AS [سبب المكافأة],
    ISNULL(SD.PENALTY_REASON, '')                              AS [سبب الخصم],
    ISNULL(SD.NOTES, '')                                       AS [ملاحظات]
FROM Employees E
LEFT JOIN SalaryData    SD ON E.CUST_ID = SD.CUST_ID
LEFT JOIN AdvancesGiven AG ON E.CUST_ID = AG.CUST_ID
                           AND AG.Yr = ISNULL(SD.Yr, YEAR(GETDATE()))
                           AND AG.Mo = ISNULL(SD.Mo, MONTH(GETDATE()))
ORDER BY [السنة] DESC, [الشهر] DESC, E.EmpName;
```

```sql
-- ===== تقرير الحضور والساعات من USER_TIME_SHEET =====
-- مفيد لحساب ساعات العمل الفعلية لكل موظف في شهر معين
-- TRANS_FLAG = دخول/خروج  |  PERIOD = ساعات الوردية (حضور → خروج)
;WITH
MonthlyHours AS (
    SELECT
        TS.USERS_ID,
        YEAR(TS.TRANS_DATE)  AS Yr,
        MONTH(TS.TRANS_DATE) AS Mo,
        SUM(CASE WHEN TS.PERIOD > 0 THEN TS.PERIOD ELSE 0 END) AS TotalHoursWorked,
        COUNT(CASE WHEN TS.PERIOD > 0 THEN 1 END)              AS ShiftsCount
    FROM dbo.USER_TIME_SHEET TS
    WHERE YEAR(TS.TRANS_DATE)  = YEAR(GETDATE())    -- عدّل السنة إن أردت
      AND MONTH(TS.TRANS_DATE) = MONTH(GETDATE())   -- عدّل الشهر إن أردت
    GROUP BY TS.USERS_ID, YEAR(TS.TRANS_DATE), MONTH(TS.TRANS_DATE)
)
SELECT
    U.FULL_NAME                                                AS [الموظف],
    MH.Yr                                                      AS [السنة],
    MH.Mo                                                      AS [الشهر],
    CAST(MH.TotalHoursWorked AS decimal(10,2))                 AS [إجمالي ساعات العمل],
    MH.ShiftsCount                                             AS [عدد الورديات],
    CAST(MH.TotalHoursWorked / NULLIF(MH.ShiftsCount, 0)
         AS decimal(10,2))                                     AS [متوسط ساعات الوردية],
    -- احتساب الساعات الإضافية (معيار 8 ساعة/يوم × 26 يوم = 208 ساعة شهرياً)
    CASE WHEN MH.TotalHoursWorked > 208
         THEN CAST(MH.TotalHoursWorked - 208 AS decimal(10,2))
         ELSE 0 END                                            AS [ساعات إضافية],
    -- ساعات الغياب (أقل من المعيار)
    CASE WHEN MH.TotalHoursWorked < 208
         THEN CAST(208 - MH.TotalHoursWorked AS decimal(10,2))
         ELSE 0 END                                            AS [ساعات غياب]
FROM MonthlyHours MH
JOIN dbo.USERS U ON MH.USERS_ID = U.USERS_ID
ORDER BY MH.TotalHoursWorked DESC;
```

---

## PATTERN: المصروفات-والنفقات-التشغيلية
TRIGGERS: مصروفات, نفقات, مصاريف تشغيلية, أنواع المصروفات, expenses, operational expenses, expense categories
TABLES: EXPENCES, EXPENCES_INVOICE, GIVE
NOTES: المصروفات تُسجَّل عبر GIVE مع EXPENCES_ID. GIVE.CUST_ID=0 يعني مصروف بدون طرف. EXPENCES_ID=0 = مدفوعات لزبائن/موردين عادية. G_TYPE: 0=عادي، 1=مصروف تشغيلي.
---

```sql
-- مدفوعات GIVE التي هي مصروفات (EXPENCES_ID > 0) لشهر محدد
SELECT TOP 100
  G.G_ID,
  CAST(G.G_DATE AS date)    AS [تاريخ الصرف],
  G.G_VALUE                  AS [المبلغ],
  G.G_DISC                   AS [البيان],
  CU.CUST_NAME               AS [المستفيد],
  E.EXPENSE_DISC             AS [نوع المصروف],
  U.FULL_NAME                AS [أدخله]
FROM dbo.GIVE G
LEFT JOIN dbo.CUSTOMERS CU ON G.CUST_ID = CU.CUST_ID
LEFT JOIN dbo.EXPENCES E   ON G.EXPENCES_ID = E.EXPENCES_ID
LEFT JOIN dbo.USERS U      ON G.USERS_ID = U.USERS_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND MONTH(G.G_DATE) = MONTH(GETDATE())   -- عدّل الشهر
  AND YEAR(G.G_DATE)  = YEAR(GETDATE())    -- عدّل السنة
ORDER BY G.G_DATE DESC;
```

```sql
-- ملخص المصروفات حسب النوع في فترة محددة
SELECT
  ISNULL(E.EXPENSE_DISC, 'غير محدد')  AS [نوع المصروف],
  COUNT(*)                              AS [عدد العمليات],
  CAST(SUM(G.G_VALUE) AS decimal(18,2)) AS [الإجمالي]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND G.G_DATE >= '2026-01-01'    -- عدّل بداية الفترة
  AND G.G_DATE <  '2026-06-01'    -- عدّل نهاية الفترة
GROUP BY E.EXPENCES_ID, E.EXPENSE_DISC
ORDER BY [الإجمالي] DESC;
```

---

## PATTERN: ملخص-مالي-شهري
TRIGGERS: ديون ومصاريف, مصاريف شهرية, الديون والمصاريف, رواتب شهرية, مصاريف تشغيلية شهرية, ديون الزبائن, اللي لي على الزبائن, monthly finances, debts and expenses
TABLES: CUSTOMERS, SALE_INVOICE, SALE_ITEMS, R_S_INVOICE, R_S_ITEMS, TAKE, GIVE, BALANCE_EDIT, SALARIES, EXPENCES
NOTES:
  - ⚠️ BALANCE_C فارغ (0 صف) — لا تستخدمه أبداً للديون.
  - **ديون لي (زبائن):** مبيعات − مردودات − TAKE + BALANCE_EDIT حيث CUST_CUSTOM=1 والرصيد > 0.
  - **رواتب مُسدَّدة:** جدول SALARIES فارغ — المدفوعات الفعلية في `dbo.GIVE` WHERE `EXPENCES_ID = 1` (مصاريف رواتب). البيان في `G_DISC` (مثل: مرتب عمر شهر 4).
  - **مصاريف تشغيلية/خاصة:** `dbo.GIVE` WHERE `EXPENCES_ID > 0` AND `EXPENCES_ID <> 1` AND `G_STATUES = 1`.
  - **⚠️ EXPENCES_ID = 0** = دفعات موردين/فواتير شراء — **ليست** مصاريف تشغيلية.
  - ملف مُختبَر: `reports-app/monthly_expenses_tracking.sql`
  - نفّذ 4 أقسام: [1] ديون، [2] رواتب مُسدَّدة، [3] مصاريف خاصة، [4] ملخص.
---

```sql
-- [1] أعلى 20 زبون مدين (لي — الديون التي لك عليهم)
DECLARE @MinBalance float = 1;
;WITH BalanceAdj AS (
  SELECT CUST_ID, SUM(ISNULL(BL_DEBIT,0))-SUM(ISNULL(BL_CREDIT,0)) AdjBalance
  FROM dbo.BALANCE_EDIT GROUP BY CUST_ID
),
SaleTot AS (
  SELECT SI.CUST_ID, SUM(SI2.QTY*SI2.PRICE) SalesValue
  FROM dbo.SALE_INVOICE SI JOIN dbo.SALE_ITEMS SI2 ON SI.S_ID=SI2.S_ID GROUP BY SI.CUST_ID
),
SaleReturnTot AS (
  SELECT R.CUST_ID, SUM(RI.QTY*RI.PRICE) ReturnValue
  FROM dbo.R_S_INVOICE R JOIN dbo.R_S_ITEMS RI ON R.S_R_ID=RI.S_R_ID GROUP BY R.CUST_ID
),
TakeTot AS (
  SELECT CUST_ID, SUM(T_VALUE) PaidValue FROM dbo.TAKE GROUP BY CUST_ID
)
SELECT TOP 20
  C.CUST_NAME AS [الزبون],
  CAST(ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) AS decimal(18,2)) AS [الرصيد المتبقي],
  CAST(ISNULL(ST.SalesValue,0) AS decimal(18,2)) AS [إجمالي المبيعات],
  CAST(ISNULL(TT.PaidValue,0) AS decimal(18,2)) AS [إجمالي المقبوضات]
FROM dbo.CUSTOMERS C
LEFT JOIN SaleTot ST ON C.CUST_ID=ST.CUST_ID
LEFT JOIN SaleReturnTot SRT ON C.CUST_ID=SRT.CUST_ID
LEFT JOIN TakeTot TT ON C.CUST_ID=TT.CUST_ID
LEFT JOIN BalanceAdj BA ON C.CUST_ID=BA.CUST_ID
WHERE C.CUST_CUSTOM=1 AND C.CUST_INVISIBLE=0
  AND ISNULL(ST.SalesValue,0)-ISNULL(SRT.ReturnValue,0)-ISNULL(TT.PaidValue,0)+ISNULL(BA.AdjBalance,0) >= @MinBalance
ORDER BY [الرصيد المتبقي] DESC;
```

```sql
-- [2] إيصالات رواتب مُسدَّدة في الشهر (SALARIES فارغ → GIVE EXPENCES_ID=1)
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());
SELECT
    CAST(G.G_DATE AS date) AS [تاريخ الصرف],
    ISNULL(G.G_NO, CAST(G.G_ID AS varchar(20))) AS [رقم الإيصال],
    ISNULL(NULLIF(LTRIM(RTRIM(C.CUST_NAME)), N''), G.G_DISC) AS [المستفيد],
    G.G_DISC AS [البيان],
    CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ]
FROM dbo.GIVE G
LEFT JOIN dbo.CUSTOMERS C ON G.CUST_ID = C.CUST_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID = 1
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
ORDER BY G.G_DATE DESC;
```

```sql
-- [3] مصاريف تشغيلية/خاصة مُسجَّلة في الشهر (غير رواتب، EXPENCES_ID<>0)
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());
SELECT
    CAST(G.G_DATE AS date) AS [التاريخ],
    ISNULL(G.G_NO, CAST(G.G_ID AS varchar(20))) AS [رقم الإيصال],
    ISNULL(E.EXPENSE_DISC, N'غير مصنف') AS [نوع المصروف],
    G.G_DISC AS [البيان],
    CAST(G.G_VALUE AS decimal(18,2)) AS [المبلغ]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND G.EXPENCES_ID <> 1
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
ORDER BY G.G_DATE DESC;
```

```sql
-- [4] ملخص مصاريف الشهر حسب النوع
DECLARE @Year int = YEAR(GETDATE());
DECLARE @Month int = MONTH(GETDATE());
SELECT
    ISNULL(E.EXPENSE_DISC, N'غير مصنف') AS [نوع المصروف],
    COUNT(*) AS [عدد العمليات],
    CAST(SUM(G.G_VALUE) AS decimal(18,2)) AS [الإجمالي]
FROM dbo.GIVE G
LEFT JOIN dbo.EXPENCES E ON G.EXPENCES_ID = E.EXPENCES_ID
WHERE G.G_STATUES = 1
  AND G.EXPENCES_ID > 0
  AND YEAR(G.G_DATE) = @Year
  AND MONTH(G.G_DATE) = @Month
GROUP BY E.EXPENCES_ID, E.EXPENSE_DISC
ORDER BY [الإجمالي] DESC;
```

---

## PATTERN: أفضل-عملاء-مبيعات
TRIGGERS: أفضل عملاء, أكثر عملاء مبيعاً, أعلى عملاء, ترتيب العملاء, مبيعات العملاء, تقرير مبيعات عملاء, top customers, best customers, customer sales ranking, زبائن الأكثر شراء
TABLES: SALE_INVOICE, SALE_ITEMS, CUSTOMERS
NOTES: **عملاء (زبائن) — ليس منتجات.** استخدم MAX(S_DATE) كمرجع. الترتيب حسب SUM(QTY*PRICE). النافذة الافتراضية 30 يوماً من آخر يوم مبيعات.
---

```sql
DECLARE @LastSaleDay date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @FromDate date = DATEADD(day, -30, @LastSaleDay);
SELECT TOP 20
  ISNULL(CAST(INV.CUST_ID AS varchar(20)), N'-') AS [رقم_العميل],
  ISNULL(NULLIF(LTRIM(RTRIM(INV.CUST_NAME)), N''), ISNULL(C.CUST_NAME, N'غير محدد')) AS [اسم_العميل],
  COUNT(DISTINCT INV.S_ID) AS [عدد_الفواتير],
  CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إجمالي_المبيعات_د_ل]
FROM dbo.SALE_INVOICE INV
INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
LEFT JOIN dbo.CUSTOMERS C ON INV.CUST_ID = C.CUST_ID
WHERE CAST(INV.S_DATE AS date) BETWEEN @FromDate AND @LastSaleDay
  AND (C.CUST_ID IS NULL OR C.CUST_CUSTOM = 1)
GROUP BY INV.CUST_ID, ISNULL(NULLIF(LTRIM(RTRIM(INV.CUST_NAME)), N''), ISNULL(C.CUST_NAME, N'غير محدد'))
HAVING SUM(SI.QTY * SI.PRICE) > 0
ORDER BY [إجمالي_المبيعات_د_ل] DESC;
```

---

## PATTERN: تحليل-المبيعات-والربحية
TRIGGERS: تحليل مبيعات, إيرادات, ربحية, هامش الربح, أفضل المنتجات مبيعاً, أكثر المنتجات ربحاً, sales analysis, revenue, profit margin, top sellers, best selling
TABLES: SALE_INVOICE, SALE_ITEMS, ITEMS, R_S_INVOICE, R_S_ITEMS
NOTES: هامش الربح = (Revenue - Cost) / Revenue × 100. صافي المبيعات يطرح المردودات. SALE_ITEMS لا يحتوي S_DATE — الربط بـ SALE_INVOICE ضروري للتصفية بالتاريخ. S_STATUES في SALE_INVOICE: لا تفلتر — جميع الحالات محتسبة ما لم يطلب المستخدم غير ذلك.
---

```sql
-- أفضل 20 منتجاً من حيث الإيرادات والربحية (عدّل التواريخ حسب الحاجة)
;WITH
NetSales AS (
  SELECT SI.ITEM_ID,
    SUM(SI.QTY * SI.PRICE) AS Revenue,
    SUM(SI.QTY) AS UnitsSold,
    SUM(SI.QTY * ISNULL(SI.AVER_COST,0)) AS TotalCost
  FROM dbo.SALE_ITEMS SI
  JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  WHERE CAST(INV.S_DATE AS date) BETWEEN '2026-01-01' AND '2026-04-07'
  GROUP BY SI.ITEM_ID
),
NetReturns AS (
  SELECT RSI.ITEM_ID,
    SUM(RSI.QTY * RSI.PRICE) AS ReturnRevenue,
    SUM(RSI.QTY) AS UnitsReturned
  FROM dbo.R_S_ITEMS RSI
  JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
  WHERE CAST(RINV.S_R_DATE AS date) BETWEEN '2026-01-01' AND '2026-04-07'
  GROUP BY RSI.ITEM_ID
)
SELECT TOP 20
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,60) AS ItemName,
  NS.UnitsSold - ISNULL(NR.UnitsReturned,0) AS NetUnits,
  CAST(NS.Revenue - ISNULL(NR.ReturnRevenue,0) AS decimal(18,2)) AS NetRevenue,
  CAST(NS.TotalCost AS decimal(18,2)) AS EstimatedCost,
  CAST(NS.Revenue - ISNULL(NR.ReturnRevenue,0) - NS.TotalCost AS decimal(18,2)) AS GrossProfit,
  CAST(
    CASE WHEN NS.Revenue > 0
      THEN (NS.Revenue - ISNULL(NR.ReturnRevenue,0) - NS.TotalCost) / NS.Revenue * 100
      ELSE 0
    END AS decimal(10,1)
  ) AS ProfitMarginPct
FROM NetSales NS
JOIN dbo.ITEMS I ON NS.ITEM_ID = I.ITEM_ID
LEFT JOIN NetReturns NR ON NS.ITEM_ID = NR.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
ORDER BY GrossProfit DESC;
```

---

## PATTERN: حركة-صنف-تفصيلية
TRIGGERS: حركة صنف, سجل صنف, تاريخ صنف, حركة مستودع, product movement, item history, stock movement, item ledger
TABLES: SALE_INVOICE, SALE_ITEMS, BUY_INVOICE, BUY_ITEMS, R_S_INVOICE, R_S_ITEMS, B_R_INVOICE, B_R_ITEMS, SPOIL_INVOICE, SPOIL_ITEMS, ITEMS
NOTES: يجمع كل حركات صنف واحد. استبدل %اسم أو كود المنتج% بالكلمة المطلوبة. الفترة افتراضية 180 يوم — عدّل حسب الحاجة.
---

```sql
-- الحركة الكاملة لصنف واحد (آخر 180 يوم — استبدل %الاسم%  بالكلمة المطلوبة)
SELECT TOP 200 MovType, TxDate, DocRef, QtyIn, QtyOut, Price, RelatedParty, EnteredBy
FROM (
  SELECT N'شراء' AS MovType, CAST(B.B_DATE AS date) TxDate, ISNULL(B.S_REF_NO,'') AS DocRef,
    BI.QTY AS QtyIn, 0 AS QtyOut, BI.PRICE, CU.CUST_NAME AS RelatedParty, U.FULL_NAME AS EnteredBy
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID=B.B_ID
  JOIN dbo.ITEMS I ON BI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID=CU.CUST_ID
  LEFT JOIN dbo.USERS U ON B.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND B.B_DATE >= DATEADD(day,-180,GETDATE())
  UNION ALL
  SELECT N'بيع', CAST(INV.S_DATE AS date), CAST(INV.S_ID AS varchar(20)),
    0, SI.QTY, SI.PRICE, INV.CUST_NAME, U.FULL_NAME
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID=INV.S_ID
  JOIN dbo.ITEMS I ON SI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.USERS U ON INV.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND INV.S_DATE >= DATEADD(day,-180,GETDATE())
  UNION ALL
  SELECT N'مردود بيع', CAST(RINV.S_R_DATE AS date), CAST(RINV.S_R_ID AS varchar(20)),
    RSI.QTY, 0, RSI.PRICE, RINV.CUST_NAME, U.FULL_NAME
  FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID=RINV.S_R_ID
  JOIN dbo.ITEMS I ON RSI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.USERS U ON RINV.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND RINV.S_R_DATE >= DATEADD(day,-180,GETDATE())
  UNION ALL
  SELECT N'تالف', CAST(SP.SP_DATE AS date), ISNULL(SP.SP_NOTE,''),
    0, SPI.QTY, SPI.PRICE, N'إتلاف', U.FULL_NAME
  FROM dbo.SPOIL_ITEMS SPI JOIN dbo.SPOIL_INVOICE SP ON SPI.SP_ID=SP.SP_ID
  JOIN dbo.ITEMS I ON SPI.ITEM_ID=I.ITEM_ID
  LEFT JOIN dbo.USERS U ON SP.USERS_ID=U.USERS_ID
  WHERE (I.ITEM_NAME LIKE N'%الاسم%' OR I.ITEM_MODEL LIKE N'%الاسم%')
    AND SP.SP_DATE >= DATEADD(day,-180,GETDATE())
) AllMovements
ORDER BY TxDate DESC, MovType;
```

---

## PATTERN: مبيعات-آخر-يوم-موظف
TRIGGERS: مبيعات آخر يوم, آخر يوم فيه مبيعات, آخر يوم مبيعات, مبيعات آخر يوم لكل موظف, last sale day, last day with sales, إيرادات آخر يوم, مبيعات الموظفين آخر يوم
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
NOTES:
  - **@LastSaleDay = CAST(MAX(S_DATE) AS date) FROM SALE_INVOICE** — لا GETDATE() ولا تاريخ ثابت 2026-04-07.
  - الإيراد = SUM(SI.QTY * SI.PRICE). SALE_ITEMS لا يحتوي S_DATE.
  - صف «═══ الإجمالي ═══» في النهاية = مجموع كل الموظفين.
  - ملف مُختبَر: `reports-app/last_sale_day_by_employee.sql`
---

```sql
DECLARE @LastSaleDay date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
;WITH EmpSales AS (
    SELECT ISNULL(U.FULL_NAME, N'غير محدد') AS [الموظف],
        COUNT(DISTINCT INV.S_ID) AS [عدد الفواتير],
        CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إيرادات], 0 AS SortOrder
    FROM dbo.SALE_INVOICE INV
    INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
    LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
    WHERE CAST(INV.S_DATE AS date) = @LastSaleDay
    GROUP BY U.USERS_ID, U.FULL_NAME
),
Grand AS (
    SELECT N'═══ الإجمالي ═══' AS [الموظف],
        COUNT(DISTINCT INV.S_ID) AS [عدد الفواتير],
        CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS [إيرادات], 1 AS SortOrder
    FROM dbo.SALE_INVOICE INV
    INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
    WHERE CAST(INV.S_DATE AS date) = @LastSaleDay
)
SELECT @LastSaleDay AS [تاريخ آخر مبيعات], [الموظف], [عدد الفواتير], [إيرادات]
FROM (SELECT [الموظف], [عدد الفواتير], [إيرادات], SortOrder FROM EmpSales
      UNION ALL SELECT [الموظف], [عدد الفواتير], [إيرادات], SortOrder FROM Grand) X
ORDER BY SortOrder, [إيرادات] DESC;
```

---

## PATTERN: آخر-منتجات-بيعت-اليوم
TRIGGERS: آخر منتجات بيعت اليوم, منتجات بيعت اليوم, الأصناف المباعة اليوم, آخر الأصناف المباعة, ماذا بيع اليوم, آخر مبيعات اليوم, last products sold today, products sold today, what sold today, recent sales today, آخر بنود مبيعات اليوم
TABLES: SALE_ITEMS, SALE_INVOICE, ITEMS
VIEWS: **dbo.SALE_ITEMS_INVOICE_VIEW** (مفضل — فيه S_DATE, ITEM_NAME, QTY, PRICE, FULL_NAME, CUST_NAME, TRAN_NO)
NOTES:
  - **سؤال «منتجات/أصناف بيعت اليوم» = هذا النمط** — ليس «مبيعات يومية موظف» (ذلك تجميع حسب الموظف).
  - **SALE_ITEMS لا يحتوي S_DATE** — لا تستعلم التاريخ من SALE_ITEMS مباشرة؛ استخدم VIEW أو JOIN إلى SALE_INVOICE.
  - **@SaleDay:** `CAST(GETDATE() AS date)` عندما يقصد المستخدم «اليوم» تقويمياً.
  - إن كانت النتيجة **فارغة** والمستخدم يريد آخر يوم فيه مبيعات → غيّر `@SaleDay` إلى `(SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE)`.
  - استبعد الملغاة: `S_STATUES <> 2` (2 = ملغاة في SALE_INVOICE).
  - إيراد السطر = `QTY * PRICE`. الترتيب: **الأحدث أولاً** (`S_DATE DESC`, `S_ITEM_ID DESC`).
  - أعمدة VIEW المفيدة: S_DATE, S_ITEM_ID, S_ID, TRAN_NO, ITEM_MODEL, ITEM_NAME, UNIT_DISC, QTY, PRICE, CUST_NAME, FULL_NAME (موظف), STORE_NAME.
---

```sql
DECLARE @SaleDay date = CAST(GETDATE() AS date);

SELECT TOP 100
    CAST(V.S_DATE AS datetime) AS [وقت_البيع],
    ISNULL(CAST(V.TRAN_NO AS nvarchar(20)), CAST(V.S_ID AS nvarchar(20))) AS [رقم_الفاتورة],
    ISNULL(CAST(V.ITEM_MODEL AS nvarchar(50)), N'') AS [كود_الصنف],
    V.ITEM_NAME AS [اسم_المنتج],
    ISNULL(V.UNIT_DISC, N'') AS [الوحدة],
    CAST(V.QTY AS decimal(18,2)) AS [الكمية],
    CAST(V.PRICE AS decimal(18,2)) AS [السعر],
    CAST(V.QTY * V.PRICE AS decimal(18,2)) AS [إجمالي_السطر],
    ISNULL(V.CUST_NAME, N'') AS [العميل],
    ISNULL(V.FULL_NAME, N'غير محدد') AS [الموظف],
    ISNULL(V.STORE_NAME, N'') AS [المخزن]
FROM dbo.SALE_ITEMS_INVOICE_VIEW V
WHERE CAST(V.S_DATE AS date) = @SaleDay
  AND ISNULL(V.S_STATUES, 0) <> 2
ORDER BY V.S_DATE DESC, V.S_ITEM_ID DESC;
```

---

## PATTERN: مبيعات-يومية-لكل-موظف
TRIGGERS: مبيعات يومية موظف, لخص المبيعات اليومية, إجمالي مبيعات كل موظف, مبيعات كل يوم بالموظف, daily sales by employee, employee daily summary, أداء يومي موظف, مبيعات الموظفين يومياً, لخص لي المبيعات اليومية
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
VIEWS: SALE_ITEMS_INVOICE_VIEW
NOTES: **لا subquery يجمع PRICE وحده.** الإيراد = SUM(QTY*PRICE). SALE_ITEMS **لا** S_DATE — استخدم INV.S_DATE. الموظف = SALE_INVOICE.USERS_ID → USERS.FULL_NAME. **التاريخ:** استخدم MAX(S_DATE) كمرجع — لا GETDATE() وحده.
---

```sql
-- مبيعات يومية لكل موظف — آخر 7 أيام من آخر يوم مبيعات
DECLARE @AsOfDate date = (SELECT CAST(MAX(S_DATE) AS date) FROM dbo.SALE_INVOICE);
DECLARE @FromDate date = DATEADD(day, -7, @AsOfDate);
;WITH EmpDaily AS (
  SELECT
    CAST(V.S_DATE AS date) AS [اليوم],
    ISNULL(V.FULL_NAME, N'غير محدد') AS [الموظف],
    COUNT(DISTINCT V.S_ID) AS [عدد الفواتير],
    CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS [الإيرادات],
    0 AS SortOrder
  FROM dbo.SALE_ITEMS_INVOICE_VIEW V
  WHERE CAST(V.S_DATE AS date) BETWEEN @FromDate AND @AsOfDate
  GROUP BY CAST(V.S_DATE AS date), V.USERS_ID, V.FULL_NAME
),
GrandTotal AS (
  SELECT
    NULL AS [اليوم],
    N'═══ الإجمالي ═══' AS [الموظف],
    COUNT(DISTINCT V.S_ID) AS [عدد الفواتير],
    CAST(SUM(V.QTY * V.PRICE) AS decimal(18,2)) AS [الإيرادات],
    1 AS SortOrder
  FROM dbo.SALE_ITEMS_INVOICE_VIEW V
  WHERE CAST(V.S_DATE AS date) BETWEEN @FromDate AND @AsOfDate
)
SELECT [اليوم], [الموظف], [عدد الفواتير], [الإيرادات]
FROM (SELECT * FROM EmpDaily UNION ALL SELECT * FROM GrandTotal) X
ORDER BY SortOrder, [اليوم] DESC, [الإيرادات] DESC;
```

---

## PATTERN: مبيعات-موظف-مندوب
TRIGGERS: مبيعات موظف, أداء مبيعات, مبيعات المندوبين, إنجاز فريق المبيعات, مبيعات بالموظف, sales by employee, sales rep performance, user sales
TABLES: SALE_INVOICE, SALE_ITEMS, USERS
VIEWS: SALE_ITEMS_INVOICE_VIEW
NOTES: USERS_ID في SALE_INVOICE = من أدخل الفاتورة. **ليس** COMMISSIONER. الإيراد = SUM(QTY*PRICE) بعد JOIN — لا subquery على PRICE فقط.
---

```sql
-- مبيعات كل موظف لمجموع فترة (عدّل التواريخ)
SELECT
  ISNULL(U.FULL_NAME, N'غير محدد') AS EmployeeName,
  COUNT(DISTINCT INV.S_ID) AS InvoiceCount,
  CAST(SUM(SI.QTY) AS decimal(18,1)) AS TotalUnits,
  CAST(SUM(SI.QTY * SI.PRICE) AS decimal(18,2)) AS TotalRevenue,
  CAST(MAX(INV.S_DATE) AS date) AS LastSaleDate
FROM dbo.SALE_INVOICE INV
INNER JOIN dbo.SALE_ITEMS SI ON INV.S_ID = SI.S_ID
LEFT JOIN dbo.USERS U ON INV.USERS_ID = U.USERS_ID
WHERE CAST(INV.S_DATE AS date) BETWEEN '2026-05-01' AND '2026-05-21'
GROUP BY U.USERS_ID, U.FULL_NAME
ORDER BY TotalRevenue DESC;
```

---

## PATTERN: تقرير-المنتجات-المجمعة
TRIGGERS: مجموعة منتجات, باقة منتجات, كوليكت, collect, bundle, product bundle, مجمعة
TABLES: COLLECT, COLLECT_DETAILS, ITEMS, UNITS
NOTES: COLLECT = مجموعات منتجات (باقات/حزم). ليس مقبوضات! COLLECT_DETAILS تحتوي محتوى كل مجموعة.
---

```sql
-- قائمة المجموعات مع محتواها والمخزون الحالي
SELECT
  C.COLLECT_NAME AS BundleName,
  I.ITEM_MODEL,
  LEFT(I.ITEM_NAME,60) AS ItemName,
  CD.QTY AS BundleQty,
  U.UNIT_DISC AS Unit,
  CD.PRICE AS ItemPrice,
  ISNULL(SUM(SUB.QTY),0) AS CurrentStock
FROM dbo.COLLECT C
JOIN dbo.COLLECT_DETAILS CD ON C.COLLECT_ID = CD.COLLECT_ID
JOIN dbo.ITEMS I ON CD.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.UNITS U ON CD.UNIT_ID = U.UNIT_ID
LEFT JOIN dbo.ITEMS_SUB SUB ON I.ITEM_ID = SUB.ITEM_ID
WHERE I.ITEM_INVISIBLE = 0
GROUP BY C.COLLECT_NAME, I.ITEM_MODEL, I.ITEM_NAME, CD.QTY, U.UNIT_DISC, CD.PRICE
ORDER BY C.COLLECT_NAME, I.ITEM_NAME;
```

---

## PATTERN: التصنيع-والتحويل
TRIGGERS: تصنيع, تركيب, تحويل مخزون, تحويل بضاعة, manufacturing, production, stock conversion, warehouse transfer
TABLES: MANF_INVOICE, MANF_F_ITEMS, MANF_T_ITEMS, TRANSFER_INVOICE, TRANSFER_ITEMS, ITEMS, STORES
NOTES: MANF = فواتير التصنيع. MANF_F_ITEMS = المواد المستهلكة (From). MANF_T_ITEMS = المنتجات الناتجة (To). TRANSFER = تحويلات بين المخازن.
---

```sql
-- آخر 20 عملية تصنيع مع المدخلات والمخرجات
SELECT TOP 20
  M.MANF_ID,
  CAST(M.MANF_DATE AS date) AS ManfDate,
  M.MANF_NOTE,
  U.FULL_NAME AS EnteredBy,
  M.MANF_STATUES,
  (SELECT STRING_AGG(CONVERT(nvarchar(200), LEFT(IF2.ITEM_NAME,30)+N' ×'+CAST(MF2.QTY AS nvarchar(20))), N', ')
   FROM dbo.MANF_F_ITEMS MF2 JOIN dbo.ITEMS IF2 ON MF2.ITEM_ID=IF2.ITEM_ID
   WHERE MF2.MANF_ID=M.MANF_ID) AS InputItems,
  (SELECT STRING_AGG(CONVERT(nvarchar(200), LEFT(IT2.ITEM_NAME,30)+N' ×'+CAST(MT2.QTY AS nvarchar(20))), N', ')
   FROM dbo.MANF_T_ITEMS MT2 JOIN dbo.ITEMS IT2 ON MT2.ITEM_ID=IT2.ITEM_ID
   WHERE MT2.MANF_ID=M.MANF_ID) AS OutputItems
FROM dbo.MANF_INVOICE M
LEFT JOIN dbo.USERS U ON M.USERS_ID = U.USERS_ID
ORDER BY M.MANF_DATE DESC;
```

```sql
-- آخر 50 تحويل بين المخازن
SELECT TOP 50
  CAST(T.TR_DATE AS date) AS TransferDate,
  I.ITEM_MODEL, LEFT(I.ITEM_NAME,50) AS ItemName,
  TI.QTY,
  SF.STORE_NAME AS FromStore,
  ST2.STORE_NAME AS ToStore,
  T.TR_NOTE,
  U.FULL_NAME AS EnteredBy
FROM dbo.TRANSFER_ITEMS TI
JOIN dbo.TRANSFER_INVOICE T ON TI.TR_ID = T.TR_ID
JOIN dbo.ITEMS I ON TI.ITEM_ID = I.ITEM_ID
LEFT JOIN dbo.STORES SF ON TI.STORE_F_ID = SF.STORE_ID
LEFT JOIN dbo.STORES ST2 ON TI.STORE_T_ID = ST2.STORE_ID
LEFT JOIN dbo.USERS U ON T.USERS_ID = U.USERS_ID
ORDER BY T.TR_DATE DESC;
```

---

## PATTERN: دراسة-منتج-شاملة
TRIGGERS: دراسة منتج, تحليل منتج, تقرير منتج, مخزون منتج, سرعة بيع, كم يكفي المخزون, طلبية شراء لمنتج, product study, item analysis, stock runway, days of stock, recommended purchase, صنف واحد, دراسة شاملة
TABLES: ITEMS, ITEMS_SUB, STORES, SALE_ITEMS, SALE_INVOICE, R_S_ITEMS, R_S_INVOICE, BUY_ITEMS, BUY_INVOICE, CUSTOMERS, BARCODE, UNITS
NOTES: **لصنف واحد.** استبدل `%PRODUCT%` بالكود أو جزء من الاسم (أو مرّر product_filter في run_query_pattern). النافذة الافتراضية 60 يوم مبيعات، 30 يوم تغطية مستهدفة. يُخرج صفاً واحداً بملخص: مخزون، مبيعات، معدل يومي، أيام تغطية، كمية شراء مقترحة، آخر شراء، أقرب صلاحية.
---

**المخرجات الرئيسية:** الكود، الاسم، مخزون إجمالي، تفصيل مخازن، مبيعات الفترة، أيام بيع فعلية، معدل يومي، أيام تغطية، كمية شراء مقترحة، أولوية، آخر سعر شراء، آخر مورد، أقرب صلاحية، حد أدنى/أعلى، متوسط تكلفة.

```sql
-- دراسة منتج واحد — استبدل %PRODUCT% (أو product_filter من الأداة)
;WITH
ItemPick AS (
  SELECT TOP 1 I.ITEM_ID, I.ITEM_MODEL, I.ITEM_NAME, I.MIN_LEVEL, I.MAX_LEVEL,
    I.LAST_COST, I.AVER_COST, I.PLACE
  FROM dbo.ITEMS I
  WHERE I.ITEM_INVISIBLE = 0
    AND (I.ITEM_MODEL LIKE N'%PRODUCT%' OR I.ITEM_NAME LIKE N'%PRODUCT%')
  ORDER BY CASE WHEN I.ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END, I.ITEM_ID DESC
),
AsOf AS (SELECT CAST(MAX(S_DATE) AS date) AS d FROM dbo.SALE_INVOICE),
Stock AS (
  SELECT IP.ITEM_ID,
    SUM(ISNULL(SUB.QTY,0)) AS TotalStock,
    MIN(CASE WHEN ISNULL(SUB.QTY,0) > 0 AND SUB.CATEOGRY3 IS NOT NULL THEN CAST(SUB.CATEOGRY3 AS date) END) AS NearestExpiry
  FROM ItemPick IP
  LEFT JOIN dbo.ITEMS_SUB SUB ON IP.ITEM_ID = SUB.ITEM_ID
  GROUP BY IP.ITEM_ID
),
StockByStore AS (
  SELECT IP.ITEM_ID, ST.STORE_NAME, SUM(ISNULL(SUB.QTY,0)) AS StoreQty
  FROM ItemPick IP
  JOIN dbo.ITEMS_SUB SUB ON IP.ITEM_ID = SUB.ITEM_ID
  JOIN dbo.STORES ST ON SUB.STORE_ID = ST.STORE_ID
  WHERE ISNULL(SUB.QTY,0) <> 0
  GROUP BY IP.ITEM_ID, ST.STORE_NAME
),
SalesRecent AS (
  SELECT IP.ITEM_ID, SUM(X.QTY) AS SoldQty,
    COUNT(DISTINCT CAST(X.S_DATE AS date)) AS ActiveSaleDays,
    MAX(X.S_DATE) AS LastSaleDate
  FROM ItemPick IP
  JOIN (
    SELECT SI.ITEM_ID, SI.QTY, INV.S_DATE
    FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
    WHERE CAST(INV.S_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
    UNION ALL
    SELECT RSI.ITEM_ID, -RSI.QTY, RINV.S_R_DATE
    FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
    WHERE CAST(RINV.S_R_DATE AS date) BETWEEN DATEADD(day,-60,(SELECT d FROM AsOf)) AND (SELECT d FROM AsOf)
  ) X ON IP.ITEM_ID = X.ITEM_ID
  GROUP BY IP.ITEM_ID
),
LastBuy AS (
  SELECT BI.ITEM_ID, BI.PRICE AS LastBuyPrice, CU.CUST_NAME AS LastSupplier, B.B_DATE AS LastBuyDate
  FROM dbo.BUY_ITEMS BI JOIN dbo.BUY_INVOICE B ON BI.B_ID = B.B_ID
  LEFT JOIN dbo.CUSTOMERS CU ON B.CUST_ID = CU.CUST_ID
  WHERE BI.B_ITEM_ID IN (
    SELECT MAX(BI2.B_ITEM_ID) FROM dbo.BUY_ITEMS BI2
    JOIN dbo.BUY_INVOICE B2 ON BI2.B_ID = B2.B_ID
    JOIN ItemPick IP2 ON BI2.ITEM_ID = IP2.ITEM_ID
    GROUP BY BI2.ITEM_ID
  )
)
SELECT
  IP.ITEM_MODEL AS ItemCode,
  LEFT(IP.ITEM_NAME, 80) AS ItemName,
  ISNULL(SK.TotalStock, 0) AS StockQty,
  (SELECT STRING_AGG(CONVERT(nvarchar(120), SBS.STORE_NAME + N': ' + CAST(CAST(SBS.StoreQty AS int) AS nvarchar(20))), N' | ')
   FROM StockByStore SBS WHERE SBS.ITEM_ID = IP.ITEM_ID) AS StockByStore,
  ISNULL(SR.SoldQty, 0) AS SoldQty60d,
  ISNULL(SR.ActiveSaleDays, 0) AS ActiveSaleDays,
  CAST(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0) AS decimal(12,3)) AS DailyRate,
  CAST(ISNULL(SK.TotalStock,0) / NULLIF(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) AS decimal(12,1)) AS DaysCoverage,
  CAST(CASE
    WHEN (ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0)) * 30 - ISNULL(SK.TotalStock,0) < 0 THEN 0
    ELSE (ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0)) * 30 - ISNULL(SK.TotalStock,0)
  END AS decimal(12,1)) AS SuggestedBuyQty,
  CASE
    WHEN ISNULL(SK.TotalStock,0) <= 0 AND ISNULL(SR.SoldQty,0) > 0 THEN N'نفاد'
    WHEN ISNULL(SK.TotalStock,0) / NULLIF(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) < 7 THEN N'حرج'
    WHEN ISNULL(SK.TotalStock,0) / NULLIF(ISNULL(SR.SoldQty,0) / NULLIF(CAST(SR.ActiveSaleDays AS float), 0), 0) < 30 THEN N'شراء'
    ELSE N'كافٍ'
  END AS Priority,
  IP.MIN_LEVEL, IP.MAX_LEVEL,
  IP.LAST_COST, IP.AVER_COST,
  LB.LastBuyPrice, LB.LastSupplier, CAST(LB.LastBuyDate AS date) AS LastBuyDate,
  SK.NearestExpiry,
  SR.LastSaleDate,
  IP.PLACE
FROM ItemPick IP
LEFT JOIN Stock SK ON IP.ITEM_ID = SK.ITEM_ID
LEFT JOIN SalesRecent SR ON IP.ITEM_ID = SR.ITEM_ID
LEFT JOIN LastBuy LB ON IP.ITEM_ID = LB.ITEM_ID;
```

---

## PATTERN: تفاصيل-منتج-وحدات-أسعار
TRIGGERS: وحدات المنتج, أسعار الوحدات, باركود, price1 price2, سعر البيع, تسعير, units prices, barcode, product units, أسعار الصنف
TABLES: ITEMS, BARCODE, UNITS
NOTES: يعرض كل BARCODE (وحدة بيع) مع UNIT_DISC و PRICE1–5 و PUBLIC_PRICE. فلتر `%PRODUCT%` أو product_filter.
---

```sql
-- وحدات وأسعار بيع لصنف — استبدل %PRODUCT%
SELECT TOP 30
  I.ITEM_MODEL AS ItemCode,
  LEFT(I.ITEM_NAME, 60) AS ItemName,
  U.UNIT_DISC AS UnitName,
  B.BARCODE,
  B.UNIT_QTY,
  B.PRICE1, B.PRICE2, B.PRICE3, B.PRICE4, B.PRICE5,
  B.PUBLIC_PRICE,
  B.PRICE_LESS,
  B.QTY AS PriceBreakQty,
  CAST(B.UPDATE_DATE AS date) AS PriceUpdated
FROM dbo.ITEMS I
JOIN dbo.BARCODE B ON I.ITEM_ID = B.ITEM_ID
LEFT JOIN dbo.UNITS U ON B.UNIT_ID = U.UNIT_ID
WHERE I.ITEM_INVISIBLE = 0
  AND (I.ITEM_MODEL LIKE N'%PRODUCT%' OR I.ITEM_NAME LIKE N'%PRODUCT%')
ORDER BY B.UNIT_QTY, U.UNIT_DISC;
```

---

## PATTERN: مبيعات-منتج-حسب-الوحدة
TRIGGERS: مبيعات الصنف بالوحدة, أي وحدة تُباع أكثر, unit mix, sales by unit for product
TABLES: SALE_ITEMS, SALE_INVOICE, ITEMS, UNITS, R_S_ITEMS, R_S_INVOICE
NOTES: توزيع المبيعات على الوحدات لصنف في آخر 90 يوم. استبدل %PRODUCT%.
---

```sql
;WITH ItemPick AS (
  SELECT TOP 1 ITEM_ID FROM dbo.ITEMS
  WHERE ITEM_INVISIBLE = 0 AND (ITEM_MODEL LIKE N'%PRODUCT%' OR ITEM_NAME LIKE N'%PRODUCT%')
  ORDER BY CASE WHEN ITEM_MODEL LIKE N'%PRODUCT%' THEN 0 ELSE 1 END
)
SELECT U.UNIT_DISC AS UnitName,
  CAST(SUM(X.QtyNet) AS decimal(18,1)) AS NetQty,
  CAST(SUM(X.Revenue) AS decimal(18,2)) AS Revenue
FROM ItemPick IP
JOIN (
  SELECT SI.ITEM_ID, SI.UNIT_ID, SI.QTY AS QtyNet, SI.QTY * SI.PRICE AS Revenue
  FROM dbo.SALE_ITEMS SI JOIN dbo.SALE_INVOICE INV ON SI.S_ID = INV.S_ID
  WHERE SI.ITEM_ID = (SELECT ITEM_ID FROM ItemPick)
    AND INV.S_DATE >= DATEADD(day, -90, GETDATE())
  UNION ALL
  SELECT RSI.ITEM_ID, RSI.UNIT_ID, -RSI.QTY, -RSI.QTY * RSI.PRICE
  FROM dbo.R_S_ITEMS RSI JOIN dbo.R_S_INVOICE RINV ON RSI.S_R_ID = RINV.S_R_ID
  WHERE RSI.ITEM_ID = (SELECT ITEM_ID FROM ItemPick)
    AND RINV.S_R_DATE >= DATEADD(day, -90, GETDATE())
) X ON IP.ITEM_ID = X.ITEM_ID
LEFT JOIN dbo.UNITS U ON X.UNIT_ID = U.UNIT_ID
GROUP BY U.UNIT_DISC
ORDER BY NetQty DESC;
```

---
# نهاية الملف
