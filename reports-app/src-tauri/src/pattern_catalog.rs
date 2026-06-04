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
        id: "client_balance_statement",
        name_ar: "كشف رصيد العميل",
        section_marketing: "كشف-حساب-عميل",
        section_infinity: "كشف-حساب-عميل",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "كشف حساب",
            "كشف حساب عميل",
            "كشف حساب شركة",
            "رصيد العميل",
            "رصيد شركة",
            "حساب العميل",
            "حساب شركة",
            "client balance",
            "customer balance statement",
            "CLIENTS_BALANCE",
        ],
    },
    PatternEntry {
        id: "client_balance_detailed",
        name_ar: "كشف حساب العميل المفصل",
        section_marketing: "كشف-حساب-عميل-مفصل",
        section_infinity: "كشف-حساب-عميل-مفصل",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "كشف حساب مفصل",
            "كشف حساب العميل مفصل",
            "كشف حساب كامل",
            "كشف حساب العميل كامل",
            "كشف حساب تفصيلي",
            "تفاصيل حساب العميل",
            "فواتير العميل وبنوده",
            "customer detailed balance",
            "detailed customer statement",
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
        id: "daily_sales_report",
        name_ar: "تقرير المبيعات والديون اليومي",
        section_marketing: "تقرير-المبيعات-والديون-اليومي",
        section_infinity: "تقرير-المبيعات-والديون-اليومي",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "تقرير المبيعات والديون",
            "المبيعات والديون",
            "daily_sales_report",
            "مبيعات وديون",
            "مبيعات الكاش والديون",
            "كاش وديون الموظفين",
            "تقرير الكاش والديون",
            "مبيعات الموظفين والديون",
            "مبيعات وديون الموظفين",
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
            "إيرادات آخر يوم",
            "مبيعات الموظفين آخر يوم",
        ],
    },
    PatternEntry {
        id: "sales_daily_employee",
        name_ar: "مبيعات يومية لكل موظف",
        section_marketing: "مبيعات-يومية-لكل-موظف",
        section_infinity: "مبيعات-يومية-لكل-موظف",
        marketing: false,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "مبيعات يومية",
            "daily sales employee",
            "مبيعات الموظفين ليوم",
            "مبيعات موظفين تاريخ",
            "employee sales specific date",
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
    if looks_like_detailed_customer_account_request(&h) {
        return find_by_id("client_balance_detailed").filter(|p| p.available_on(erp));
    }
    if (h.contains("كشف حساب") || h.contains("حساب العميل") || h.contains("رصيد العميل"))
        && (h.contains("مفصل")
            || h.contains("تفصيلي")
            || h.contains("كامل")
            || h.contains("الفواتير")
            || h.contains("البنود"))
    {
        return find_by_id("client_balance_detailed").filter(|p| p.available_on(erp));
    }
    if h.contains("كشف حساب") || h.contains("حساب العميل") || h.contains("رصيد العميل")
    {
        return find_by_id("client_balance_statement").filter(|p| p.available_on(erp));
    }
    if (h.contains("مبيعات") || h.contains("المبيعات"))
        && (h.contains("ديون") || h.contains("الديون") || h.contains("كاش"))
    {
        return find_by_id("daily_sales_report").filter(|p| p.available_on(erp));
    }
    if (h.contains("مبيعات") || h.contains("ايرادات") || h.contains("إيرادات"))
        && (h.contains("موظف") || h.contains("موظفين") || h.contains("الموظفين"))
    {
        return find_by_id("sales_last_day_employee").filter(|p| p.available_on(erp));
    }
    if h.contains("مبيعات يومية") || h.contains("المبيعات اليومية") {
        return find_by_id("sales_last_day_employee").filter(|p| p.available_on(erp));
    }
    if h.contains("مبيعات")
        && (h.contains("آخر يوم")
            || h.contains("اخر يوم")
            || h.contains("آخر يوم مبيعات")
            || h.contains("اخر يوم مبيعات"))
    {
        return find_by_id("sales_last_day_employee").filter(|p| p.available_on(erp));
    }
    if looks_like_named_customer_debt_query(&h) {
        return find_by_id("customer_debts").filter(|p| p.available_on(erp));
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

fn looks_like_detailed_customer_account_request(h: &str) -> bool {
    let customer_context = h.contains("عميل")
        || h.contains("العميل")
        || h.contains("زبون")
        || h.contains("شركة")
        || h.contains("له")
        || h.contains("لها")
        || h.contains("نفس العميل")
        || h.contains("نفس الزبون")
        || h.contains("نفس الشركة")
        || h.contains("حسابه")
        || h.contains("حسابها");
    if !customer_context {
        return false;
    }

    let account_context = h.contains("حركة")
        || h.contains("تحليل")
        || h.contains("حلل")
        || h.contains("رأيك")
        || h.contains("رايك")
        || h.contains("كشف شامل")
        || h.contains("شامل")
        || h.contains("كامل")
        || h.contains("الكامل")
        || h.contains("التقرير الكامل")
        || h.contains("بالتفصيل")
        || h.contains("تفاصيل")
        || h.contains("تفاصيله")
        || h.contains("تفاصيلها")
        || h.contains("تفصيلي")
        || h.contains("مفصل")
        || h.contains("فواتير")
        || h.contains("بنود");

    account_context
        && !h.contains("موظف")
        && !h.contains("مبيعات موظف")
        && !h.contains("مورد")
        && !h.contains("منتج")
        && !h.contains("صنف")
}

fn looks_like_named_customer_debt_query(h: &str) -> bool {
    if !h.contains("ديون") && !h.contains("دين") && !h.contains("ذمة") {
        return false;
    }
    if h.contains("موظف")
        || h.contains("موظفين")
        || h.contains("الموظف")
        || h.contains("سلف")
        || h.contains("راتب")
        || h.contains("مورد")
        || h.contains("موردين")
        || h.contains("اللي علي")
    {
        return false;
    }
    let generic = [
        "ديون",
        "دين",
        "ذمة",
        "زبون",
        "زبائن",
        "زباين",
        "عميل",
        "عملاء",
        "شركة",
        "اعرض",
        "عرض",
        "تقرير",
        "لي",
    ];
    let meaningful_words = h
        .split_whitespace()
        .filter(|w| w.chars().count() >= 2 && !generic.contains(w))
        .count();
    meaningful_words >= 1
}

pub fn list_for_erp(erp: ErpKind) -> Vec<&'static PatternEntry> {
    CATALOG.iter().filter(|p| p.available_on(erp)).collect()
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
            if p.needs_product_filter {
                "نعم"
            } else {
                "—"
            }
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
        استخدم run_erp_report للتقارير المعروفة، وrun_query_pattern كـ fallback فقط. لا تولّد HTML/PDF؛ التطبيق يطبع من النتيجة المحفوظة. العملة: د.ل.\n\
        الأنماط:\n{compact_list}{pf_line}\n\
        مطابقة مختصرة: customer_balance=كشف مختصر، customer_balance_detailed=كشف مفصل/كامل، customer_debt=ديون عميل، supplier_debt=ديون مورد، daily_sales_report=مبيعات وديون يومية، sales_last_day_employee=مبيعات آخر يوم، top_sellers=أكثر مبيعاً، expiry_report=صلاحية، shortage_supplier=نواقص، supplier_price_compare+pf=مقارنة موردين، last_purchase_price+pf=آخر سعر شراء. تاريخ صريح يُستخدم كما هو.",
        erp = erp.display_name_ar(),
        date_str = date_str,
        compact_list = compact_pattern_list(erp),
        pf_line = pf_line,
    )
}

fn compact_pattern_list(erp: ErpKind) -> String {
    let mut lines = Vec::new();
    for p in list_for_erp(erp) {
        let pf = if p.needs_product_filter {
            " [product_filter]"
        } else {
            ""
        };
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
    fn resolve_daily_sales_report_by_id() {
        let p = resolve_pattern_id("daily_sales_report", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("daily_sales_report"));
    }

    #[test]
    fn resolve_daily_sales_report_by_trigger() {
        let p = resolve_pattern_id("تقرير المبيعات والديون", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("daily_sales_report"));
    }

    #[test]
    fn resolve_employee_sales_debts_to_daily_sales_report() {
        let p = resolve_pattern_id("مبيعات الموظفين والديون", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("daily_sales_report"));
    }

    #[test]
    fn resolve_sales_last_day_beats_daily_employee_overlap() {
        let p = resolve_pattern_id("اعرض مبيعات الموظفين اخر يوم", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_last_day_employee"));
    }

    #[test]
    fn resolve_sales_daily_employee() {
        let p = resolve_pattern_id("مبيعات يومية", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_last_day_employee"));
    }

    #[test]
    fn resolve_specific_date_to_daily_employee() {
        let p = resolve_pattern_id("مبيعات الموظفين ليوم", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_last_day_employee"));
    }

    #[test]
    fn resolve_employee_revenue_to_last_day_query() {
        let p = resolve_pattern_id("اعرض ايرادات الموظفين", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("sales_last_day_employee"));
    }

    #[test]
    fn resolve_by_id_infinity() {
        let p = resolve_pattern_id("expiry_report", ErpKind::InfinityRetailDb);
        assert!(p.is_some());
    }

    #[test]
    fn catalog_has_sixteen_entries() {
        assert_eq!(CATALOG.len(), 16);
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
    fn resolve_named_customer_debt_query() {
        let p = resolve_pattern_id("احمد مختي ديون", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("customer_debts"));
    }

    #[test]
    fn resolve_client_balance_statement() {
        let p = resolve_pattern_id("اعرض لي كشف حساب شركة الهشيم", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_statement"));
    }

    #[test]
    fn resolve_client_balance_detailed_statement() {
        let p = resolve_pattern_id("اعرض لي كشف حساب مفصل شركة الهشيم", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_detailed"));
    }

    #[test]
    fn resolve_customer_movement_analysis_to_detailed_balance() {
        let p = resolve_pattern_id("حلل لي حركة العميل احمد مختي ديون", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_detailed"));
    }

    #[test]
    fn resolve_followup_customer_account_analysis_to_detailed_balance() {
        let p = resolve_pattern_id("اعطيني رأيك في حركته نفس العميل", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_detailed"));
    }

    #[test]
    fn resolve_followup_full_report_to_detailed_balance() {
        let p = resolve_pattern_id("اريد التقرير الكامل له", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_detailed"));
    }

    #[test]
    fn resolve_followup_detailed_account_without_name() {
        let p = resolve_pattern_id("اعرض كشف حساب مفصل له", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_detailed"));
    }

    #[test]
    fn resolve_followup_same_customer_details() {
        let p = resolve_pattern_id("تفاصيل نفس العميل بالكامل", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("client_balance_detailed"));
    }

    #[test]
    fn resolve_balance_statement_for_customer_name_containing_debts() {
        let p = resolve_pattern_id(
            "اعرض لي كشف حساب العميل احمد مختي ديون",
            ErpKind::Marketing2026,
        );
        assert_eq!(p.map(|x| x.id), Some("client_balance_statement"));
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
        let prompt = build_executor_system_prompt(
            ErpKind::Marketing2026,
            None,
            "الاثنين 2/6/2026 | الشهر:6 | السنة:2026",
        );
        assert!(
            prompt.chars().count() < 2000,
            "system prompt too long: {} chars",
            prompt.chars().count()
        );
    }

    #[test]
    fn system_prompt_contains_date() {
        let prompt = build_executor_system_prompt(
            ErpKind::Marketing2026,
            None,
            "الاثنين 2/6/2026 | الشهر:6 | السنة:2026",
        );
        assert!(prompt.contains("2026"), "date not injected into prompt");
    }

    #[test]
    fn system_prompt_daily_employee_hint_present() {
        let prompt = build_executor_system_prompt(ErpKind::Marketing2026, None, "test");
        assert!(
            prompt.contains("sales_last_day_employee"),
            "sales_last_day_employee hint missing"
        );
        assert!(
            prompt.contains("تاريخ قديم") || prompt.contains("تاريخ صريح"),
            "historical date hint missing"
        );
    }
}
