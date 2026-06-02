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

fn is_bare_barcode(text: &str) -> bool {
    text.split_whitespace().any(|word| {
        let digits: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
        (8..=14).contains(&digits.len())
    })
}

/// رسالة قصيرة = اسم/باركود منتج (بدون طلب تقرير عام)
fn looks_like_bare_product_query(h: &str) -> bool {
    let t = h.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_lowercase();
    const BLOCK: &[&str] = &[
        "ديون", "مبيعات", "موظف", "مورد", "ملخص", "نواقص", "طلبية", "عملاء", "مصاريف",
        "تقرير", "أعلى", "أكثر", "financial", "كم منتج", "عدد المنتجات", "best seller",
    ];
    if BLOCK.iter().any(|k| lower.contains(k)) {
        return false;
    }
    if is_bare_barcode(t) {
        return true;
    }
    const INFO: &[&str] = &[
        "معلومات", "تفاصيل", "اعرض", "اعرضلي", "عرض", "المنتج", "باركود", "product info",
        "about product", "هذا المنتج", "عن هذا",
    ];
    if INFO.iter().any(|k| lower.contains(k)) {
        return true;
    }
    let wc = t.split_whitespace().count();
    wc >= 1 && wc <= 6 && (4..=60).contains(&t.len())
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
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "get_current_datetime",
                "description": "Returns current date/time for month/day filters in patterns.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_available_patterns",
                "description": "Lists all report patterns available on the currently connected ERP. Call when user asks what you can do.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_query_pattern",
                "description": "Executes a pre-tested SQL pattern. REQUIRED for any data question. Prefer pattern_id over keywords.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern_id": { "type": "string", "description": "Stable id from list_available_patterns (preferred)." },
                        "keywords": { "type": "string", "description": "Fallback Arabic keywords if pattern_id unknown." },
                        "days_recent": { "type": "integer", "description": "Override sales window or expiration days window (default 60)." },
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
                "name": "execute_raw_sql",
                "description": "Executes a T-SQL SELECT/WITH query. Copy SQL from patterns in your system prompt, adjust dates/filters, then call this. No DECLARE — start with WITH or SELECT only.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sql_query": { "type": "string", "description": "The T-SQL query." }
                    },
                    "required": ["sql_query"]
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
) -> String {
    let pf_note = product_filter
        .filter(|s| !s.is_empty())
        .map(|pf| format!("\n\n**product_filter نشط:** `{pf}` — مرّره لـ run_query_pattern.\n"))
        .unwrap_or_default();

    format!(
        "<role>\n\
        منفّذ تقارير {erp} — **لا تكتب SQL** — **لا تخترع أرقاماً**.\n\
        </role>\n\n\
        <tone_and_dialect>\n\
        - **لهجة ليبية خفيفة وموجزة جداً (لتوفير التوكنز):**\n\
          - ابدأ بترحيب ليبي خفيف ومختصر للغاية (مثل: 'مرحبتين بيك.' أو 'أهلاً بيك. تفضل النتائج:').\n\
          - ممنوع نهائياً التملق، التحيات الطويلة، الكلام الفارغ، أو التبجيل والمبالغة (مثل 'يا فندم'، 'يسعدني خدمتكم'، 'بدقة متناهية').\n\
          - اعرض نتائج البيانات فوراً واختصر قدر الإمكان لتقليل استهلاك التوكنز.\n\
          - اقترح التصدير أو الخطوة التالية باختصار شديد ودون إطالة (مثل: 'تبيه إكسل أو PDF؟').\n\
        </tone_and_dialect>\n\n\
        <critical_rules>\n\
        1. **أي سؤال بيانات** → استدعِ `run_query_pattern` فوراً (pattern_id مفضل).\n\
        2. **ممنوع** الرد بجدول أو أرقام أو «تم التنفيذ» بدون tool call ناجح.\n\
        3. **ممنوع** أسئلة توضيحية (أ. ب. ج.) — نفّذ أو قل «غير مدعوم».\n\
        4. إن لم يوجد pattern → `list_available_patterns` ثم اقترح الأقرب.\n\
        5. تصدير → `export_last_result` بعد run_query_pattern.\n\
        6. التاريخ → `get_current_datetime` عند الحاجة.\n\
        7. احسب الإجماليات الفرعية والعامة للنتائج واعرضها بوضوح باختصار في نهاية ردك.\n\
        8. **الأسئلة العامة والاستشارية:** أجب باختصار شديد بلهجة ليبية ودية وموجزة دون أدوات قاعدة البيانات.\n\
        9. لخّص نتائج الأداة باختصار — العملة: د.ل.\n\
        </critical_rules>\n\n\
        <patterns>\n\
        ERP: **{erp}** | ملف SQL: `{agent_file}`\n\n\
        {table}\n\
        </patterns>{pf_note}\n\n\
        <mapping_hints>\n\
        - «أكثر مبيعاً» → top_sellers SQL-A | «هذا الشهر» → SQL-B | «الشهر السابق» → SQL-C | «توقعات/تنبؤات» → SQL-D\n\
        - «نواقص / نفاد / شن النواقص» → pattern_id=shortage_supplier\n\
        - «صلاحية / منتهية / ينتهي» → pattern_id=expiry_report\n\
        - «مصروفات / مصاريف / كم صرفنا» → monthly_expenses: SQL-A هذا الشهر | SQL-B الشهر السابق | SQL-C مقارنة 6 شهور\n\
        - «مقارنة أسعار / أرخص مورد / موردي منتج» → pattern_id=supplier_price_compare + product_filter\n\
        - «آخر سعر شراء / سعر المورد / كم اشترينا» → pattern_id=last_purchase_price + product_filter\n\
        - «ديون الزباين / ديون الزبائن / اللي لي على الزباين» → pattern_id=customer_debts\n\
        - «ديون الموردين / ديون مورد / آخر إيصال صرف مورد» → pattern_id=supplier_debts\n\
        - «مبيعات آخر يوم موظف / إيرادات اليوم» → pattern_id=sales_last_day_employee\n\
        - «مبيعات يومية موظف / مبيعات الموظفين ليوم X» → pattern_id=sales_daily_employee\n\
        - ⚠️ عند طلب تاريخ صريح (مثل «ليوم 21/5/2026»): استخدم sales_daily_employee وضع التاريخ في @TargetDate.\n\
        - ⚠️ لا تستبدل تاريخاً صريحاً بـ MAX(S_DATE) — استخدمه مباشرةً.\n\
        </mapping_hints>",
        erp = erp.display_name_ar(),
        agent_file = erp.agent_file_label(),
        table = prompt_table(erp),
        pf_note = pf_note,
    )
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
    fn catalog_has_ten_entries() {
        assert_eq!(CATALOG.len(), 12);
    }

    #[test]
    fn resolve_customer_debts() {
        let p = resolve_pattern_id("ديون الزبائن", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("customer_debts"));
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
}
