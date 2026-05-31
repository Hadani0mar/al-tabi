//! SQL أنماط مخزون Infinity المتقدمة — Supabase OTA ثم sql-split مضمّن.

use crate::erp_profile::ErpKind;
use std::borrow::Cow;

fn embedded_sql(slug: &str) -> Option<&'static str> {
    match slug.trim() {
        "طلبية-شراء-متقدمة" => Some(include_str!("../../sql-split/01-purchase-order.sql")),
        "أصناف-راكدة-متقدمة" => Some(include_str!("../../sql-split/02-slow-moving.sql")),
        "خطر-الصلاحية-FEFO" => Some(include_str!("../../sql-split/03-expiry-risk.sql")),
        "اتجاه-مبيعات-30-30" => Some(include_str!("../../sql-split/04-sales-trend-30-30.sql")),
        "أصناف-قيد-التجربة" => Some(include_str!("../../sql-split/05-trial-products.sql")),
        "أصناف-وهمية" => Some(include_str!("../../sql-split/06-phantom-products.sql")),
        "تصنيف-حركة-الصنف" => Some(include_str!("../../sql-split/07-product-movement.sql")),
        "فحص-الأصناف-والوحدات" => Some(include_str!("../../sql-split/08-check-items-uom.sql")),
        "حساب-توفر-المخزون" => Some(include_str!("../../sql-split/09-check-availability.sql")),
        "المبيعات-وصافي-المطلوب" => Some(include_str!("../../sql-split/10-net-required.sql")),
        "فواتير-المشتريات-والصلاحية" => Some(include_str!("../../sql-split/11-purchase-invoices-expiry.sql")),
        _ => None,
    }
}

/// slug يطابق ## PATTERN: في AGENT_InfinityRetailDB.md
pub fn sql_for_slug(slug: &str) -> Option<Cow<'static, str>> {
    if let Some(remote) =
        crate::agent_content_sync::load_cached_pattern_sql(ErpKind::InfinityRetailDb, slug)
    {
        if !remote.trim().is_empty() {
            return Some(Cow::Owned(remote));
        }
    }
    embedded_sql(slug).map(Cow::Borrowed)
}

pub fn is_infinity_batch_slug(slug: &str) -> bool {
    sql_for_slug(slug).is_some()
}

/// يستبدل كتلة ```sql في قسم النمط بملف sql-split أو نسخة Supabase
pub fn augment_pattern_section(section: &str, slug: &str, erp: ErpKind) -> String {
    if erp != ErpKind::InfinityRetailDb {
        return section.to_string();
    }
    let Some(sql) = sql_for_slug(slug) else {
        return section.to_string();
    };

    if let (Some(start), Some(end)) = (section.find("```sql"), section.rfind("```")) {
        if end > start {
            let before = &section[..start];
            let after = &section[end + 3..];
            return format!("{before}```sql\n{sql}\n```{after}");
        }
    }

    format!("{section}\n\n```sql\n{sql}\n```")
}
