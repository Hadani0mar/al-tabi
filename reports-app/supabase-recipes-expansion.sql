-- ============================================================
-- توسيع agent_tool_recipes — تغطية أوسع للصياغات العربية
-- المشروع: nsgmhijtaaenpqxxgjds (delivery-app)
-- طبّق هذا في Supabase SQL Editor
-- ============================================================

-- ─── تحديث trigger_phrases للأنماط الموجودة بصياغات أكثر ────────────────────

-- top_sellers
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'أكثر مبيعاً','أعلى منتجات','top sellers','best selling',
  'أكثر المنتجات بيعاً','أكثر الاصناف','رانكينج المبيعات',
  'أعلى إيرادات','الأكثر طلباً','مبيعات هذا الشهر',
  'مبيعات الشهر السابق','توقعات مبيعات','تنبؤات',
  'شن الأكثر مبيعاً','أيش يتباع أكثر','الأصناف الرائجة',
  'أكثر أصناف بيعاً','الأصناف الأكثر طلباً','مبيعات الأسبوع',
  'أكثر مبيع','تحليل المبيعات','ماذا يشتري الزبائن أكثر'
]
WHERE pattern_id = 'top_sellers' AND erp_kind = 'Marketing2026';

UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'أكثر مبيعاً','أعلى منتجات','top sellers','best selling',
  'أكثر المنتجات بيعاً','الأكثر طلباً','مبيعات هذا الشهر',
  'شن الأكثر مبيعاً','أيش يتباع أكثر','الأصناف الرائجة',
  'أكثر مبيع','ماذا يشتري الزبائن أكثر','الأصناف الأكثر طلباً'
]
WHERE pattern_id = 'top_sellers' AND erp_kind = 'InfinityRetailDB';

-- expiry_report
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'صلاحية','منتهية','expiry','سينتهي قريباً','ينتهي هذا الشهر',
  'منتهية الصلاحية','صلاحيات','تاريخ انتهاء','سينخلص قريباً',
  'ستنتهي صلاحيتها','expired','الصلاحيات','المنتهية',
  'قريبة الانتهاء','تاريخ الصلاحية','الأدوية المنتهية',
  'شن منتهي الصلاحية','خلصت صلاحيتها','تقرير الصلاحية',
  'صلاحيتها قريبة','تلف الأصناف'
]
WHERE pattern_id = 'expiry_report';

-- shortage_supplier
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'نواقص','نواقصنا','شن النواقص','عندنا نواقص','نفاد',
  'shortage','نواقص نشطة','تحت الحد','ايش ناقصنا',
  'ماذا ينقصنا','قائمة النواقص','المنتجات الناقصة',
  'المخزون الناقص','أصناف ناقصة','نفاد المخزون',
  'أيش خلص عندنا','أيش لازم نشتري','قائمة الشراء',
  'المنتجات اللي خلصت','أصناف بدون مخزون'
]
WHERE pattern_id = 'shortage_supplier';

-- monthly_expenses
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'مصروفات','مصاريف','expenses','رواتب وإيجار',
  'مصاريف الشهر','كم صرفنا','نفقات','مصاريف شهرية',
  'مقارنة مصاريف','مصاريف هذا الشهر','مصاريف الشهر الماضي',
  'كم دفعنا','المصروفات الشهرية','رواتب الشهر',
  'تكاليف التشغيل','ميزانية الشهر','كم أنفقنا',
  'مصاريف الإيجار والرواتب','النفقات','كم الإنفاق'
]
WHERE pattern_id = 'monthly_expenses';

-- last_purchase_price
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'آخر سعر شراء','سعر شراء','last purchase price',
  'buy price','آخر مشتريات','آخر شراء','سعر المورد',
  'آخر تكلفة شراء','كم آخر مرة اشترينا','من أين اشترينا',
  'كمية المنتج الآن','مورد المنتج','كم سعر الدواء',
  'بكم نشتري','آخر سعر للمنتج','كم تكلفة الصنف',
  'آخر فاتورة شراء للصنف','تكلفة المنتج'
]
WHERE pattern_id = 'last_purchase_price';

-- supplier_price_compare
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'مقارنة أسعار','مقارنة موردين','موردي منتج','أرخص مورد',
  'أغلى مورد','supplier prices','compare suppliers',
  'افضل الموردين','أفضل مورد','موردين له',
  'كام مورد عندهم هذا الصنف','مقارنة أسعار الموردين',
  'أرخص من يبيع','من عنده أرخص سعر','مورد أرخص',
  'قارن الموردين','فرق الأسعار بين الموردين'
]
WHERE pattern_id = 'supplier_price_compare';

-- employee_ranking
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'ترتيب الموظفين','أفضل موظف','أعلى دخل','أداء الموظفين',
  'موظف الشهر','employee ranking','معدل الدخل','متوسط الفاتورة',
  'من أكثر موظف باع','من الأفضل','مبيعات الموظفين',
  'إنتاجية الموظفين','كفاءة الموظفين','تقييم الموظفين',
  'أعلى موظف مبيعاً','ترتيب أداء الموظفين','إحصائيات الموظفين'
]
WHERE pattern_id = 'employee_ranking';

-- customer_debts
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'ديون الزبائن','ديون الزباين','ديون العملاء','اللي لي',
  'من يدينني','customer debts','ذمة الزبائن','ديون لي',
  'الزبائن المدينين','من يدين لي','عندهم ديون',
  'متأخرين في الدفع','الذمم المدينة','الزبائن اللي عندهم دين',
  'من ما دفع','الزبائن المتأخرين','أرصدة الزبائن'
]
WHERE pattern_id = 'customer_debts';

-- supplier_debts
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'ديون الموردين','ديون موردين','اللي علي','من أدين له',
  'supplier debts','ذمة الموردين','ديون علي',
  'كم ديني للموردين','ما علي من ديون','الموردين اللي أدين لهم',
  'فواتير غير مدفوعة للموردين','مديونياتي','ما دفعناه للموردين',
  'الذمم الدائنة للموردين','مبالغ مستحقة للموردين'
]
WHERE pattern_id = 'supplier_debts';

-- employee_debts
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'ديون الموظفين','ديون موظفين','سلف الموظفين',
  'ذمة الموظفين','employee debts','سلف','ديون العمال',
  'سلفيات الموظفين','الموظفين المدينين','من من الموظفين عنده دين',
  'مديونيات الموظفين','استحقاقات الموظفين'
]
WHERE pattern_id = 'employee_debts';

-- near_expiry_sales_hero
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'بطل المبيعات','الموظف المنقذ','مبيعات قرب الصلاحية',
  'منتجات قرب الصلاحية المباعة','خسارة تم تداركها',
  'بطل بيع الصلاحية','near expiry sales hero',
  'من باع قرب الصلاحية','الموظف اللي باع الصلاحيات',
  'توفير خسائر الصلاحية','منقذ المخزون'
]
WHERE pattern_id = 'near_expiry_sales_hero';

-- sales_last_day_employee
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'مبيعات آخر يوم','آخر يوم مبيعات','last sale day',
  'مبيعات الموظفين اليومية','المبيعات اليومية للموظفين',
  'مبيعات يومية للموظفين','إيرادات آخر يوم',
  'مبيعات الموظفين آخر يوم','ماذا باع الموظفون اليوم',
  'مبيعات اليوم','إيرادات اليوم','مبيعات آخر يوم عمل',
  'مبيعات الأمس','من باع الأكثر اليوم'
]
WHERE pattern_id = 'sales_last_day_employee';

-- sales_daily_employee
UPDATE public.agent_tool_recipes
SET trigger_phrases = ARRAY[
  'مبيعات يومية','مبيعات كل موظف','daily sales employee',
  'مبيعات موظفين','مبيعات الموظفين ليوم','مبيعات موظفين تاريخ',
  'employee sales specific date','أداء يومي موظف',
  'لخص المبيعات اليومية','مبيعات يوم محدد','تقرير يومي',
  'مبيعات الأسبوع الماضي','مبيعات هذا الأسبوع',
  'مبيعات آخر أسبوع','مبيعات الموظفين أسبوعياً'
]
WHERE pattern_id = 'sales_daily_employee';

-- ─── إضافة أنماط InfinityRetailDB الناقصة ───────────────────────────────────

INSERT INTO public.agent_tool_recipes (
  erp_kind, intent_key, title_ar, pattern_id, tool_name,
  trigger_phrases, slots, tool_args_template, response_hint, confidence
)
SELECT
  'InfinityRetailDB',
  public.agent_normalize_text('ترتيب الموظفين Infinity'),
  'ترتيب الموظفين (Infinity)',
  'employee_ranking',
  'run_query_pattern',
  ARRAY['ترتيب الموظفين','أفضل موظف','أداء الموظفين','أعلى موظف مبيعاً','من الأفضل','كفاءة الموظفين'],
  '{"requires_product_filter": false}'::jsonb,
  '{"pattern_id": "employee_ranking"}'::jsonb,
  'نفّذ النمط مباشرة ولخص النتيجة.',
  0.92
WHERE NOT EXISTS (
  SELECT 1 FROM public.agent_tool_recipes
  WHERE pattern_id = 'employee_ranking' AND erp_kind = 'InfinityRetailDB'
);

INSERT INTO public.agent_tool_recipes (
  erp_kind, intent_key, title_ar, pattern_id, tool_name,
  trigger_phrases, slots, tool_args_template, response_hint, confidence
)
SELECT
  'InfinityRetailDB',
  public.agent_normalize_text('ديون الزبائن Infinity'),
  'ديون الزبائن (Infinity)',
  'customer_debts',
  'run_query_pattern',
  ARRAY['ديون الزبائن','ديون الزباين','اللي لي','الزبائن المدينين','ذمة الزبائن','متأخرين في الدفع'],
  '{"requires_product_filter": false}'::jsonb,
  '{"pattern_id": "customer_debts"}'::jsonb,
  'نفّذ النمط مباشرة ولخص النتيجة.',
  0.91
WHERE NOT EXISTS (
  SELECT 1 FROM public.agent_tool_recipes
  WHERE pattern_id = 'customer_debts' AND erp_kind = 'InfinityRetailDB'
);

INSERT INTO public.agent_tool_recipes (
  erp_kind, intent_key, title_ar, pattern_id, tool_name,
  trigger_phrases, slots, tool_args_template, response_hint, confidence
)
SELECT
  'InfinityRetailDB',
  public.agent_normalize_text('ديون الموردين Infinity'),
  'ديون الموردين (Infinity)',
  'supplier_debts',
  'run_query_pattern',
  ARRAY['ديون الموردين','اللي علي','ما علي من ديون','مبالغ مستحقة للموردين','ذمة الموردين'],
  '{"requires_product_filter": false}'::jsonb,
  '{"pattern_id": "supplier_debts"}'::jsonb,
  'نفّذ النمط مباشرة ولخص النتيجة.',
  0.91
WHERE NOT EXISTS (
  SELECT 1 FROM public.agent_tool_recipes
  WHERE pattern_id = 'supplier_debts' AND erp_kind = 'InfinityRetailDB'
);

INSERT INTO public.agent_tool_recipes (
  erp_kind, intent_key, title_ar, pattern_id, tool_name,
  trigger_phrases, slots, tool_args_template, response_hint, confidence
)
SELECT
  'InfinityRetailDB',
  public.agent_normalize_text('بطل مبيعات قرب الصلاحية Infinity'),
  'بطل بيع قرب الصلاحية (Infinity)',
  'near_expiry_sales_hero',
  'run_query_pattern',
  ARRAY['بطل المبيعات','الموظف المنقذ','مبيعات قرب الصلاحية','بطل بيع الصلاحية','منقذ المخزون'],
  '{"requires_product_filter": false}'::jsonb,
  '{"pattern_id": "near_expiry_sales_hero"}'::jsonb,
  'نفّذ النمط مباشرة ولخص النتيجة.',
  0.90
WHERE NOT EXISTS (
  SELECT 1 FROM public.agent_tool_recipes
  WHERE pattern_id = 'near_expiry_sales_hero' AND erp_kind = 'InfinityRetailDB'
);

-- ─── نمط export_last_result عام لكلا النظامين ────────────────────────────────

INSERT INTO public.agent_tool_recipes (
  erp_kind, intent_key, title_ar, pattern_id, tool_name,
  trigger_phrases, slots, tool_args_template, response_hint, confidence
)
SELECT
  e.erp,
  public.agent_normalize_text('تصدير آخر نتيجة ' || e.erp),
  'تصدير آخر نتيجة',
  NULL,
  'export_last_result',
  ARRAY['اعمل pdf','اطبع pdf','صدر pdf','اعمل اكسل','احفظ اكسل',
        'export excel','export pdf','صدر نتيجة','احفظ التقرير',
        'ملف Excel','ملف PDF','اصدر النتيجة','حفظ النتيجة',
        'تصدير','شاور لي pdf','ابعث اكسل'],
  '{"requires_product_filter": false}'::jsonb,
  '{"format": "pdf"}'::jsonb,
  'صدّر آخر نتيجة استعلام. إذا طلب المستخدم Excel استخدم format=excel.',
  0.88
FROM (VALUES ('Marketing2026'), ('InfinityRetailDB')) AS e(erp)
WHERE NOT EXISTS (
  SELECT 1 FROM public.agent_tool_recipes
  WHERE tool_name = 'export_last_result' AND erp_kind = e.erp
);

-- التحقق من النتائج
SELECT erp_kind, pattern_id, tool_name, array_length(trigger_phrases, 1) AS phrases_count
FROM public.agent_tool_recipes
ORDER BY erp_kind, pattern_id;
