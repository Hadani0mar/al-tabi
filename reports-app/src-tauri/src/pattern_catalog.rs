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
        id: "product_count",
        name_ar: "عدد المنتجات",
        section_marketing: "عدد-المنتجات",
        section_infinity: "عدد-المنتجات",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["عدد المنتجات", "كم منتج", "count products", "عدد الاصناف"],
    },
    PatternEntry {
        id: "top_sellers",
        name_ar: "أعلى منتجات مبيعاً (30 يوم)",
        section_marketing: "أعلى-منتجات-مبيعاً",
        section_infinity: "أعلى-منتجات-مبيعاً",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["أعلى منتجات", "أكثر مبيعاً", "best sellers", "top products"],
    },
    PatternEntry {
        id: "top_sellers_all_time",
        name_ar: "أعلى منتجات مبيعاً (كل الوقت)",
        section_marketing: "أعلى-منتجات-كل-الوقت",
        section_infinity: "أعلى-منتجات-كل-الوقت",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "كل الوقت",
            "بدون تاريخ",
            "all time",
            "على مستوى قاعدة",
            "مستوى قاعدة",
            "ليس في تاريخ",
            "ليس اخر 30",
            "بدون فترة",
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
        triggers: &["مبيعات آخر يوم", "آخر يوم مبيعات", "last sale day"],
    },
    PatternEntry {
        id: "sales_today_employee",
        name_ar: "مبيعات اليوم لكل موظف",
        section_marketing: "مبيعات-اليوم-للموظفين",
        section_infinity: "",
        marketing: true,
        infinity: false,
        needs_product_filter: false,
        triggers: &[
            "مبيعات اليوم",
            "مبيعات اليوم للموظفين",
            "مبيعات الموظفين اليوم",
            "إيرادات اليوم",
            "today sales by employee",
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
        triggers: &["مبيعات يومية", "مبيعات كل موظف", "daily sales employee"],
    },
    PatternEntry {
        id: "products_sold_today",
        name_ar: "آخر منتجات بيعت اليوم",
        section_marketing: "آخر-منتجات-بيعت-اليوم",
        section_infinity: "آخر-منتجات-بيعت-اليوم",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["منتجات بيعت اليوم", "ماذا بيع اليوم", "what sold today"],
    },
    PatternEntry {
        id: "debts_full",
        name_ar: "متابعة الديون",
        section_marketing: "متابعة-الديون",
        section_infinity: "متابعة-الديون",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["متابعة الديون", "ديون", "اللي لي", "اللي علي"],
    },
    PatternEntry {
        id: "debts_suppliers",
        name_ar: "ديون الموردين",
        section_marketing: "ديون-الموردين-مبسط",
        section_infinity: "ديون-الموردين",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["ديون الموردين", "supplier debts"],
    },
    PatternEntry {
        id: "debts_advances_schedule",
        name_ar: "ديون وسلف ومواعيد الدفع",
        section_marketing: "ديون-وسلف-ومواعيد",
        section_infinity: "ديون-وسلف-ومواعيد",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "ديون وسلف",
            "سلف ومواعيد",
            "مواعيد الدفع",
            "موعد الدفع",
            "سلف",
            "قرض",
            "ذمة",
            "payment schedule",
            "payment due",
            "advances",
            "سلف مسترد",
        ],
    },
    PatternEntry {
        id: "financial_summary",
        name_ar: "ملخص مالي شهري",
        section_marketing: "ملخص-مالي-شهري",
        section_infinity: "ملخص-مالي-شهري",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["ملخص مالي", "مصاريف شهرية", "رواتب شهرية"],
    },
    PatternEntry {
        id: "shortage_monitor",
        name_ar: "متابعة النواقص",
        section_marketing: "متابعة-النواقص",
        section_infinity: "متابعة-النواقص",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["متابعة النواقص", "نواقص", "shortage"],
    },
    PatternEntry {
        id: "shortage_supplier",
        name_ar: "نواقص نشطة مع المورد",
        section_marketing: "نواقص-نشطة-مورد",
        section_infinity: "نواقص-نشطة-مورد",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["نواقص نشطة", "آخر سعر شراء"],
    },
    PatternEntry {
        id: "smart_purchase",
        name_ar: "طلبية شراء ذكية",
        section_marketing: "طلبية-شراء-ذكية",
        section_infinity: "طلبية-شراء-متقدمة",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "طلبية شراء",
            "شراء ذكي",
            "smart purchase",
            "ماذا أشتري",
            "صافي المطلوب",
            "كمية مقترحة",
            "طلبية",
        ],
    },
    PatternEntry {
        id: "supplier_prices",
        name_ar: "مقارنة أسعار الموردين",
        section_marketing: "مقارنة-أسعار-موردين",
        section_infinity: "مقارنة-أسعار-موردين",
        marketing: true,
        infinity: true,
        needs_product_filter: true,
        triggers: &["مقارنة أسعار", "موردين", "supplier prices", "موردي", "افضل الموردين", "أفضل مورد", "أفضل الموردين", "موردين له"],
    },
    PatternEntry {
        id: "product_info",
        name_ar: "معلومات منتج كاملة",
        section_marketing: "معلومات-منتج-كاملة",
        section_infinity: "معلومات-منتج-كاملة",
        marketing: true,
        infinity: true,
        needs_product_filter: true,
        triggers: &[
            "معلومات منتج",
            "معلومات عن",
            "تفاصيل المنتج",
            "بيانات المنتج",
            "معدل السحب",
            "معدل سحب",
            "سرعة البيع",
            "سعر البيع",
            "الصلاحية",
            "product info",
            "اعرضلي",
            "اعرض لي",
            "ابحث عن منتج",
        ],
    },
    PatternEntry {
        id: "product_study",
        name_ar: "دراسة منتج شاملة",
        section_marketing: "دراسة-منتج-شاملة",
        section_infinity: "دراسة-منتج-شاملة",
        marketing: true,
        infinity: true,
        needs_product_filter: true,
        triggers: &["دراسة منتج", "تفاصيل منتج", "كل شي", "product study"],
    },
    PatternEntry {
        id: "expiry_report",
        name_ar: "تقرير الصلاحية",
        section_marketing: "تقرير-الصلاحية",
        section_infinity: "تقرير-الصلاحية",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["صلاحية", "منتهية", "expiry"],
    },
    PatternEntry {
        id: "best_customers",
        name_ar: "أفضل عملاء مبيعات",
        section_marketing: "أفضل-عملاء-مبيعات",
        section_infinity: "أفضل-عملاء-مبيعات",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["أفضل عملاء", "top customers"],
    },
    PatternEntry {
        id: "employee_discounts_debts",
        name_ar: "خصومات وديون الموظفين",
        section_marketing: "خصومات-وديون-موظفين",
        section_infinity: "خصومات-وديون-موظفين",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "خصومات موظف",
            "خصومات الموظفين",
            "ديون موظف",
            "ديون الموظفين",
            "employee discount",
            "employee debt",
            "خصم موظف",
            "ذمة موظف",
        ],
    },
    PatternEntry {
        id: "sales_by_location",
        name_ar: "مبيعات حسب المخزن/الفرع",
        section_marketing: "مبيعات-حسب-المخزن",
        section_infinity: "مبيعات-حسب-الفرع",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &["مبيعات مخزن", "مبيعات فرع", "sales by store"],
    },
    PatternEntry {
        id: "product_search",
        name_ar: "بحث منتج سريع",
        section_marketing: "بحث-منتج-سريع",
        section_infinity: "بحث-منتج-سريع",
        marketing: true,
        infinity: true,
        needs_product_filter: true,
        triggers: &["بحث منتج", "product search", "باركود", "barcode"],
    },
    PatternEntry {
        id: "slow_moving_adv",
        name_ar: "أصناف راكدة (تحليل متقدم)",
        section_marketing: "أصناف-راكة",
        section_infinity: "أصناف-راكدة-متقدمة",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "راكدة",
            "راكد",
            "بضاعة راكدة",
            "slow moving",
            "dead stock",
            "تكلفة الراكد",
        ],
    },
    PatternEntry {
        id: "expiry_risk_fefo",
        name_ar: "خطر الصلاحية FEFO",
        section_marketing: "تقرير-الصلاحية",
        section_infinity: "خطر-الصلاحية-FEFO",
        marketing: true,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "خطر الصلاحية",
            "كمية الخطر",
            "FEFO",
            "صلاحية خطر",
            "قيمة الخطر",
            "expiry risk",
        ],
    },
    PatternEntry {
        id: "sales_trend_30",
        name_ar: "اتجاه المبيعات 30/30",
        section_marketing: "مقارنة-مبيعات-شهرية",
        section_infinity: "اتجاه-مبيعات-30-30",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "اتجاه المبيعات",
            "30/30",
            "تغيّر المبيعات",
            "صاعد",
            "هابط",
            "sales trend",
            "واعي بالتوفّر",
        ],
    },
    PatternEntry {
        id: "trial_products",
        name_ar: "أصناف قيد التجربة",
        section_marketing: "دراسة-منتج-شاملة",
        section_infinity: "أصناف-قيد-التجربة",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &["قيد التجربة", "أصناف تجريبية", "trial products", "صنف جديد"],
    },
    PatternEntry {
        id: "phantom_products",
        name_ar: "أصناف وهمية",
        section_marketing: "أصناف-راكة",
        section_infinity: "أصناف-وهمية",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "وهمي",
            "وهمية",
            "صنف وهمي",
            "phantom",
            "بدون بيع",
            "متابعة وهمي",
        ],
    },
    PatternEntry {
        id: "product_movement_class",
        name_ar: "تصنيف حركة الصنف",
        section_marketing: "معلومات-منتج-كاملة",
        section_infinity: "تصنيف-حركة-الصنف",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "حركة الصنف",
            "منشط",
            "ضعيف الحركة",
            "صنف ميت",
            "تصنيف الحركة",
            "product movement class",
        ],
    },
    PatternEntry {
        id: "check_items_uom",
        name_ar: "فحص الأصناف والعبوات",
        section_marketing: "معلومات-منتج-كاملة",
        section_infinity: "فحص-الأصناف-والوحدات",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "فحص الأصناف والعبوات",
            "معامل التعبئة",
            "check items uom",
            "فحص الأصناف",
        ],
    },
    PatternEntry {
        id: "check_availability",
        name_ar: "حساب توفر المخزون",
        section_marketing: "معلومات-منتج-كاملة",
        section_infinity: "حساب-توفر-المخزون",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "حساب توفر المخزون",
            "توفر المخزون",
            "أيام التوفر",
            "رصيد نهاية اليوم",
            "check availability",
        ],
    },
    PatternEntry {
        id: "net_required_check",
        name_ar: "المبيعات وصافي المطلوب",
        section_marketing: "معلومات-منتج-كاملة",
        section_infinity: "المبيعات-وصافي-المطلوب",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "المبيعات وصافي المطلوب",
            "صافي المطلوب لتغطية",
            "متوسط البيع اليومي الدقيق",
            "net required",
        ],
    },
    PatternEntry {
        id: "purchase_invoices_expiry",
        name_ar: "فواتير المشتريات والصلاحية",
        section_marketing: "معلومات-منتج-كاملة",
        section_infinity: "فواتير-المشتريات-والصلاحية",
        marketing: false,
        infinity: true,
        needs_product_filter: false,
        triggers: &[
            "فواتير المشتريات والصلاحية",
            "فواتير آخر 3 أشهر",
            "فحص فواتير الشراء",
            "تواريخ صلاحيات المشتريات",
            "purchase invoices expiry",
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
    if looks_like_bare_product_query(hint) {
        if let Some(p) = find_by_id("product_info") {
            if p.available_on(erp) {
                return Some(p);
            }
        }
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
                        "days_recent": { "type": "integer", "description": "Override sales window (default 60)." },
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
) -> String {
    let pf_note = product_filter
        .filter(|s| !s.is_empty())
        .map(|pf| format!("\n\n**product_filter نشط:** `{pf}` — مرّره لـ run_query_pattern.\n"))
        .unwrap_or_default();

    format!(
        "<role>\n\
        منفّذ تقارير {erp} — **لا تكتب SQL** — **لا تخترع أرقاماً**.\n\
        </role>\n\n\
        <critical_rules>\n\
        1. **أي سؤال بيانات** → استدعِ `run_query_pattern` فوراً (pattern_id مفضل).\n\
        2. **ممنوع** الرد بجدول أو أرقام أو «تم التنفيذ» بدون tool call ناجح.\n\
        3. **ممنوع** أسئلة توضيحية (أ. ب. ج.) — نفّذ أو قل «غير مدعوم».\n\
        4. إن لم يوجد pattern → `list_available_patterns` ثم اقترح الأقرب.\n\
        5. تصدير → `export_last_result` بعد run_query_pattern.\n\
        6. التاريخ → `get_current_datetime` عند الحاجة.\n\
        7. لخّص نتائج الأداة بالعربية — العملة: د.ل.\n\
        </critical_rules>\n\n\
        <patterns>\n\
        ERP: **{erp}** | ملف SQL: `{agent_file}`\n\n\
        {table}\n\
        </patterns>{pf_note}\n\n\
        <mapping_hints>\n\
        - «كم منتج» → pattern_id=product_count\n\
        - «أعلى مبيعاً بدون تاريخ / كل الوقت» → top_sellers_all_time\n\
        - «أعلى مبيعاً» (30 يوم) → top_sellers\n\
        - «مبيعات آخر يوم موظف» → sales_last_day_employee\n\
        - «خصومات/ديون الموظفين» → employee_discounts_debts (Infinity فقط)\n\
        - «ديون وسلف/مواعيد الدفع/سلف» → debts_advances_schedule (3 أجزاء)\n\
        - «ديون ومصاريف شهرية» → financial_summary\n\
        - «موردي @منتج» أو «افضل الموردين له» → supplier_prices + product_filter\n\
        - **اسم أو باركود فقط** أو «معلومات عن منتج» → pattern_id=product_info + product_filter\n\
        - product_info يُرجع: مخزون، سعر بيع، تكلفة، معدل سحب، أيام تغطية، صلاحية، آخر مورد\n\
        - باركود (8–14 رقم) بدون سياق آخر → product_info (ليس product_search)\n\
        - إن row_count=0 و product_found=true: المنتج موجود لكن لا مشتريات موردين — لا تقل «غير موجود»\n\
        - إن product_found=false: اذكر active_erp و database من نتيجة الأداة\n\
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
    fn resolve_top_sellers_all_time() {
        let p = resolve_pattern_id("ليس في تاريخ معين أعلى منتج", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("top_sellers_all_time"));
    }

    #[test]
    fn resolve_by_id() {
        let p = resolve_pattern_id("product_count", ErpKind::Marketing2026);
        assert!(p.is_some());
    }

    #[test]
    fn resolve_barcode_to_product_info() {
        let p = resolve_pattern_id("8718951291010", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("product_info"));
    }

    #[test]
    fn resolve_supplier_query_not_product_info() {
        let p = resolve_pattern_id("ما افضل الموردين لمنتج 8718951291010", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("supplier_prices"));
    }

    #[test]
    fn resolve_employee_discounts_infinity() {
        let p = resolve_pattern_id("خصومات وديون الموظفين", ErpKind::InfinityRetailDb);
        assert_eq!(p.map(|x| x.id), Some("employee_discounts_debts"));
    }

    #[test]
    fn resolve_employee_discounts_not_on_marketing() {
        let p = resolve_pattern_id("خصومات الموظفين", ErpKind::Marketing2026);
        assert!(p.is_none());
    }

    #[test]
    fn resolve_debts_advances_schedule() {
        let p = resolve_pattern_id("ديون وسلف ومواعيد الدفع", ErpKind::Marketing2026);
        assert_eq!(p.map(|x| x.id), Some("debts_advances_schedule"));
        let p2 = resolve_pattern_id("مواعيد الدفع للموردين", ErpKind::InfinityRetailDb);
        assert_eq!(p2.map(|x| x.id), Some("debts_advances_schedule"));
    }
}