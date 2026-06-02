//! كatalog الأنماط المنطقي — pattern_id ثابت لكل ERP

use crate::erp_profile::ErpKind;
use serde_json::json;

#[derive(Clone, Copy, Debug)]
pub struct PatternEntry {
    pub id: &'static str,
    pub name_ar: &'static str,
    pub section_marketing: &'static str,
    pub section_infinity: &'static str,
    pub marketing: bool,
    pub infinity: bool,
    pub needs_product_filter: bool,
    pub triggers: &'static [&'static str],
}

pub const CATALOG: &[PatternEntry] = &[
    PatternEntry {
        id: "expiry_report",
        name_ar: "تقرير الصلاحية",
        section_marketing: "تقرير-الصلاحية",
        section_infinity: "تقرير-الصلاحية",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "صلاحية",
            "منتهية",
            "expiry",
            "سينتهي قريباً",
            "ينتهي هذا الشهر",
            "منتهية الصلاحية",
            "صلاحيات",
            "تاريخ انتهاء",
            "سينخلص قريباً",
            "ستنتهي صلاحيتها",
            "expired",
            "الصلاحيات",
            "المنتهية",
        ],
    },
    PatternEntry {
        id: "last_purchase_price",
        name_ar: "آخر سعر شراء + الكمية الحالية (يتطلب اسم منتج)",
        section_marketing: "آخر-سعر-شراء-مورد",
        section_infinity: "آخر-سعر-شراء-مورد",
        marketing: true,
        infinity: true,
        needs_product_filter: true,
        triggers: &[
            "آخر سعر شراء",
            "سعر شراء",
            "last purchase price",
            "buy price",
            "آخر مشتريات",
            "آخر شراء",
            "سعر المورد",
            "آخر تكلفة شراء",
            "كم آخر مرة اشترينا",
            "من أين اشترينا",
            "كمية المنتج الآن",
            "مورد المنتج",
        ],
    },
    PatternEntry {
        id: "top_sellers",
        name_ar: "أكثر المنتجات مبيعاً (مع سعر الشراء والمورد)",
        section_marketing: "أعلى-منتجات-مبيعاً",
        section_infinity: "أعلى-منتجات-مبيعاً",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "أكثر مبيعاً",
            "أعلى منتجات",
            "top sellers",
            "best selling",
            "أكثر المنتجات بيعاً",
            "أكثر الاصناف",
            "رانكينج المبيعات",
            "أعلى إيرادات",
            "الأكثر طلباً",
            "مبيعات هذا الشهر",
            "مبيعات الشهر السابق",
            "توقعات مبيعات",
            "تنبؤات",
            "forecast",
        ],
    },
    PatternEntry {
        id: "monthly_expenses",
        name_ar: "المصروفات الشهرية (رواتب، إيجار، كهرباء، أخرى)",
        section_marketing: "المصروفات-الشهرية",
        section_infinity: "المصروفات-الشهرية",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "مصروفات",
            "مصاريف",
            "expenses",
            "رواتب وإيجار",
            "مصاريف الشهر",
            "كم صرفنا",
            "نفقات",
            "مصاريف شهرية",
            "مقارنة مصاريف",
            "مصاريف هذا الشهر",
            "مصاريف الشهر الماضي",
        ],
    },
    PatternEntry {
        id: "supplier_price_compare",
        name_ar: "مقارنة أسعار الموردين (يتطلب اسم منتج)",
        section_marketing: "مقارنة-أسعار-موردين",
        section_infinity: "مقارنة-أسعار-موردين",
        marketing: true,
        infinity: true,
        needs_product_filter: true,
        triggers: &[
            "مقارنة أسعار",
            "مقارنة موردين",
            "موردي منتج",
            "أرخص مورد",
            "أغلى مورد",
            "supplier prices",
            "compare suppliers",
            "افضل الموردين",
            "أفضل مورد",
            "موردين له",
        ],
    },
    PatternEntry {
        id: "shortage_supplier",
        name_ar: "نواقص نشطة (الكمية + آخر سعر شراء + المورد)",
        section_marketing: "نواقص-نشطة-مورد",
        section_infinity: "نواقص-نشطة-مورد",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "نواقص",
            "نواقصنا",
            "شن النواقص",
            "عندنا نواقص",
            "نفاد",
            "shortage",
            "نواقص نشطة",
            "تحت الحد",
            "ايش ناقصنا",
            "ماذا ينقصنا",
            "قائمة النواقص",
            "المنتجات الناقصة",
        ],
    },
    PatternEntry {
        id: "employee_ranking",
        name_ar: "ترتيب الموظفين (دخل + فواتير + معدل يومي)",
        section_marketing: "ترتيب-الموظفين",
        section_infinity: "ترتيب-الموظفين",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "ترتيب الموظفين",
            "أفضل موظف",
            "أعلى دخل",
            "أداء الموظفين",
            "موظف الشهر",
            "employee ranking",
            "معدل الدخل",
            "متوسط الفاتورة",
        ],
    },
    PatternEntry {
        id: "employee_debts",
        name_ar: "ديون وسلف الموظفين",
        section_marketing: "ديون-الموظفين",
        section_infinity: "ديون-الموظفين",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "ديون الموظفين",
            "ديون موظفين",
            "سلف الموظفين",
            "ذمة الموظفين",
            "employee debts",
            "سلف",
            "ديون العمال",
        ],
    },
    PatternEntry {
        id: "near_expiry_sales_hero",
        name_ar: "بطل بيع المنتجات القريبة من الصلاحية",
        section_marketing: "بطل-بيع-قرب-الصلاحية",
        section_infinity: "بطل-بيع-قرب-الصلاحية",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "بطل المبيعات",
            "الموظف المنقذ",
            "مبيعات قرب الصلاحية",
            "منتجات قرب الصلاحية",
            "خسارة تم تداركها",
            "بطل بيع الصلاحية",
            "near expiry sales hero",
            "saved expiry sales",
            "expiry sales by employee",
        ],
    },
    PatternEntry {
        id: "customer_debts",
        name_ar: "ديون الزبائن (+ آخر إيصال قبض)",
        section_marketing: "ديون-الزبائن",
        section_infinity: "ديون-الزبائن",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "ديون الزبائن",
            "ديون الزباين",
            "ديون العملاء",
            "اللي لي",
            "من يدينني",
            "customer debts",
            "ذمة الزبائن",
        ],
    },
    PatternEntry {
        id: "supplier_debts",
        name_ar: "ديون الموردين (+ آخر إيصال صرف)",
        section_marketing: "ديون-الموردين",
        section_infinity: "ديون-الموردين",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "ديون الموردين",
            "ديون موردين",
            "اللي علي",
            "من أدين له",
            "supplier debts",
            "ذمة الموردين",
        ],
    },
    PatternEntry {
        id: "sales_last_day_employee",
        name_ar: "مبيعات آخر يوم لكل موظف",
        section_marketing: "مبيعات-آخر-يوم-موظف",
        section_infinity: "مبيعات-آخر-يوم-موظف",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "مبيعات آخر يوم",
            "آخر يوم مبيعات",
            "last sale day",
            "مبيعات الموظفين اليومية",
            "المبيعات اليومية للموظفين",
            "مبيعات يومية للموظفين",
            "إيرادات آخر يوم",
            "مبيعات الموظفين آخر يوم",
        ],
    },
    PatternEntry {
        id: "sales_daily_employee",
        name_ar: "مبيعات يومية لكل موظف",
        section_marketing: "مبيعات-يومية-لكل-موظف",
        section_infinity: "مبيعات-يومية-لكل-موظف",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "مبيعات يومية",
            "مبيعات كل موظف",
            "daily sales employee",
            "مبيعات موظفين",
            "مبيعات الموظفين ليوم",
            "مبيعات موظفين تاريخ",
            "employee sales specific date",
            "أداء يومي موظف",
            "لخص المبيعات اليومية",
        ],
    },
];

impl PatternEntry {
    pub fn available_on(self, erp: ErpKind) -> bool {
        match erp {
            ErpKind::Marketing2026 | ErpKind::Unknown => self.marketing,
            ErpKind::InfinityRetailDb => self.infinity,
        }
    }

    pub fn section_slug(self, erp: ErpKind) -> Option<&'static str> {
        if !self.available_on(erp) {
            return None;
        }
        Some(match erp {
            ErpKind::InfinityRetailDb => self.section_infinity,
            ErpKind::Marketing2026 | ErpKind::Unknown => self.section_marketing,
        })
    }
}

pub fn find_by_id(id: &str) -> Option<&'static PatternEntry> {
    let key = id.trim().to_lowercase();
    CATALOG.iter().find(|p| p.id == key)
}

pub fn resolve_pattern_id(hint: &str, erp: ErpKind) -> Option<&'static PatternEntry> {
    let h = hint.trim().to_lowercase();
    if h.is_empty() {
        return None;
    }
    if let Some(p) = find_by_id(&h) {
        return if p.available_on(erp) { Some(p) } else { None };
    }
    let mut best: Option<(&PatternEntry, usize)> = None;
    for entry in CATALOG.iter() {
        if !entry.available_on(erp) {
            continue;
        }
        let mut score = 0usize;
        if entry.name_ar.contains(hint.trim()) || h.contains(entry.id) {
            score += 10;
        }
        for t in entry.triggers {
            let tl = t.to_lowercase();
            if h.contains(&tl) || tl.contains(&h) {
                score += 5;
            }
            for w in h.split_whitespace() {
                if w.len() >= 3 && tl.contains(w) {
                    score += 2;
                }
            }
        }
        if score > 0 {
            if best.map(|(_, s)| score > s).unwrap_or(true) {
                best = Some((entry, score));
            }
        }
    }
    best.map(|(e, _)| e)
}

pub fn list_for_erp(erp: ErpKind) -> Vec<&'static PatternEntry> {
    CATALOG
        .iter()
        .filter(|p| p.available_on(erp))
        .collect()
}

#[allow(dead_code)]
pub fn prompt_table(erp: ErpKind) -> String {
    let mut lines = vec![
        "| pattern_id | التقرير | product_filter |".to_string(),
        "|---|---|---|".to_string(),
    ];
    for p in list_for_erp(erp) {
        lines.push(format!(
            "| `{}` | {} | {} |",
            p.id,
            p.name_ar,
            if p.needs_product_filter { "نعم" } else { "—" }
        ));
    }
    lines.join("\n")
}

pub fn handle_list_available_patterns(erp: ErpKind) -> serde_json::Value {
    let patterns: Vec<_> = list_for_erp(erp)
        .into_iter()
        .map(|p| {
            json!({
                "pattern_id": p.id,
                "name_ar": p.name_ar,
                "needs_product_filter": p.needs_product_filter,
            })
        })
        .collect();
    json!({
        "erp": erp.display_name_ar(),
        "pattern_count": patterns.len(),
        "patterns": patterns,
        "message": "اختر pattern_id ومرّره لـ run_query_pattern — لا SQL حر."
    })
}

pub fn executor_tool_definitions() -> Vec<serde_json::Value> {
    // أداتان فقط — التاريخ محقون في system prompt مباشرةً بدل tool call
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "run_query_pattern",
                "description": "Executes a pre-tested SQL pattern. REQUIRED for any data question. Use pattern_id from the <patterns> table in system prompt.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern_id": { "type": "string", "description": "ID from the <patterns> table (e.g. top_sellers, expiry_report)." },
                        "keywords": { "type": "string", "description": "Fallback Arabic keywords if pattern_id unknown." },
                        "days_recent": { "type": "integer", "description": "Override sales window in days (default 60)." },
                        "coverage_days": { "type": "integer", "description": "Purchase coverage days (default 30)." },
                        "product_filter": { "type": "string", "description": "Product name/code for patterns that need it." }
                    },
                    "required": []
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "export_last_result",
                "description": "Export last query result as PDF or Excel when user asks for export/اكسل/pdf.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Arabic report title." },
                        "format": { "type": "string", "enum": ["pdf", "excel"], "description": "pdf or excel." }
                    },
                    "required": ["title", "format"]
                }
            }
        }),
    ]
}

pub fn build_executor_system_prompt(
    erp: ErpKind,
    product_filter: Option<&str>,
    date_str: &str,
) -> String {
    let pf_line = product_filter
        .filter(|s| !s.is_empty())
        .map(|pf| format!("\nproduct_filter نشط: \"{pf}\" — مرّره لـ run_query_pattern."))
        .unwrap_or_default();

    format!(
        "منفّذ تقارير {erp} | لا SQL حر | لا أرقام مخترعة | اليوم: {date_str}\n\
        لهجة ليبية قصيرة — لا تملق — ابدأ بالنتائج فوراً — اقترح ('إكسل أو PDF؟').\n\
        \n\
        ## القواعد\n\
        1. أي سؤال بيانات → run_query_pattern بـ pattern_id فوراً (لا تتردد ولا تسأل).\n\
        2. ممنوع جداول/أرقام بدون tool call ناجح.\n\
        3. تصدير → export_last_result بعد run_query_pattern.\n\
        4. إجماليات في نهاية الرد | العملة: د.ل.\n\
        5. أسئلة عامة → أجب باختصار بدون أدوات.\n\
        \n\
        ## الأنماط المتاحة — {erp}\n\
        {compact_list}{pf_line}\n\
        \n\
        ## مطابقة\n\
        top_sellers: أكثر مبيعاً | days_recent=N للتاريخ القديم | B=هذا الشهر | C=السابق | D=توقعات\n\
        shortage_supplier: نواقص/نفاد/شن النواقص | expiry_report: صلاحية/منتهية\n\
        monthly_expenses: مصروفات/مصاريف | A=هذا | B=السابق | C=مقارنة 6 شهور\n\
        supplier_price_compare+pf: مقارنة أسعار/أرخص مورد | last_purchase_price+pf: آخر سعر شراء\n\
        customer_debts: ديون الزباين/اللي لي | supplier_debts: اللي علي/ديون الموردين\n\
        sales_last_day_employee: مبيعات آخر يوم (بدون تحديد)\n\
        sales_daily_employee: تاريخ محدد أو قديم → days_recent=N أو @TargetDate مباشرةً\n\
        employee_ranking: ترتيب/أداء الموظفين | near_expiry_sales_hero: بطل المبيعات\n\
        ⚠️ تاريخ صريح → استخدمه كما هو، لا تستبدله بـ MAX(S_DATE).",
        erp = erp.display_name_ar(),
        date_str = date_str,
        compact_list = compact_pattern_list(erp),
        pf_line = pf_line,
    )
}

fn compact_pattern_list(erp: ErpKind) -> String {
    let mut lines = Vec::new();
    for p in list_for_erp(erp) {
        let pf = if p.needs_product_filter { " [product_filter]" } else { "" };
        lines.push(format!("- {}: {}{}", p.id, p.name_ar, pf));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_expiry_by_id() {
        let p = resolve_pattern_id("expiry_report", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("expiry_report"));
    }

    #[test]
    fn resolve_expiry_by_trigger() {
        let p = resolve_pattern_id("منتهية الصلاحية", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("expiry_report"));
    }

    #[test]
    fn resolve_last_purchase_price() {
        let p = resolve_pattern_id("آخر سعر شراء", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("last_purchase_price"));
    }

    #[test]
    fn resolve_sales_last_day() {
        let p = resolve_pattern_id("مبيعات آخر يوم", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_last_day_employee"));
    }

    #[test]
    fn resolve_sales_daily_employee() {
        let p = resolve_pattern_id("مبيعات يومية", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_daily_employee"));
    }

    #[test]
    fn resolve_specific_date_to_daily_employee() {
        let p = resolve_pattern_id("مبيعات الموظفين ليوم", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_daily_employee"));
    }

    #[test]
    fn resolve_by_id_infinity() {
        let p = resolve_pattern_id("expiry_report", ErpKind::InfinityRetailDb);
        assert!(p.is_some());
    }

    #[test]
    fn catalog_has_thirteen_entries() {
        assert_eq!(CATALOG.len(), 13);
    }

    #[test]
    fn resolve_customer_debts() {
        let p = resolve_pattern_id("ديون الزبائن", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("customer_debts"));
    }

    #[test]
    fn resolve_near_expiry_sales_hero() {
        let p = resolve_pattern_id("بطل المبيعات قرب الصلاحية", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("near_expiry_sales_hero"));
    }

    #[test]
    fn resolve_supplier_debts() {
        let p = resolve_pattern_id("ديون الموردين", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("supplier_debts"));
    }

    #[test]
    fn resolve_customer_debts_dialect() {
        let p = resolve_pattern_id("ديون الزباين", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("customer_debts"));
    }

    #[test]
    fn customer_debts_not_on_infinity() {
        let p = resolve_pattern_id("ديون الزباين", ErpKind::InfinityRetailDb);
        assert!(p.is_none());
    }

    #[test]
    fn supplier_debts_not_on_infinity() {
        let p = resolve_pattern_id("supplier_debts", ErpKind::InfinityRetailDb);
        assert!(p.is_none());
    }

    #[test]
    fn resolve_top_sellers() {
        let p = resolve_pattern_id("أكثر مبيعاً", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("top_sellers"));
    }

    #[test]
    fn resolve_top_sellers_this_month() {
        let p = resolve_pattern_id("مبيعات هذا الشهر", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("top_sellers"));
    }

    #[test]
    fn resolve_top_sellers_forecast() {
        let p = resolve_pattern_id("توقعات مبيعات", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("top_sellers"));
    }

    #[test]
    fn resolve_monthly_expenses() {
        let p = resolve_pattern_id("مصروفات", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("monthly_expenses"));
    }

    #[test]
    fn resolve_expenses_not_on_infinity() {
        let p = resolve_pattern_id("مصروفات", ErpKind::InfinityRetailDb);
        assert!(p.is_none());
    }

    #[test]
    fn resolve_supplier_price_compare() {
        let p = resolve_pattern_id("مقارنة أسعار", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("supplier_price_compare"));
    }

    #[test]
    fn resolve_cheapest_supplier_trigger() {
        let p = resolve_pattern_id("أرخص مورد", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("supplier_price_compare"));
    }

    #[test]
    fn resolve_shortage_supplier() {
        let p = resolve_pattern_id("نواقص", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("shortage_supplier"));
    }

    #[test]
    fn resolve_shortage_dialect() {
        let p = resolve_pattern_id("شن النواقص", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("shortage_supplier"));
    }

    #[test]
    fn system_prompt_under_2000_chars() {
        let prompt = build_executor_system_prompt(ErpKind::Marketing2026, None, "الاثنين 2/6/2026 | الشهر:6 | السنة:2026");
        assert!(
            prompt.chars().count() < 2000,
            "system prompt too long: {} chars",
            prompt.chars().count()
        );
    }

    #[test]
    fn system_prompt_contains_date() {
        let prompt = build_executor_system_prompt(ErpKind::Marketing2026, None, "الاثنين 2/6/2026 | الشهر:6 | السنة:2026");
        assert!(prompt.contains("2026"), "date not injected into prompt");
    }

    #[test]
    fn system_prompt_daily_employee_hint_present() {
        let prompt = build_executor_system_prompt(ErpKind::Marketing2026, None, "test");
        assert!(prompt.contains("sales_daily_employee"), "sales_daily_employee hint missing");
        assert!(prompt.contains("تاريخ قديم") || prompt.contains("تاريخ صريح"), "historical date hint missing");
    }
}
