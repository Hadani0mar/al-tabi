use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ROWS: usize = 50_000;

// ─── لوحة الألوان المهنية ────────────────────────────────────────────
const C_HEADER_BG:  u32 = 0x1A3869; // أزرق داكن — رأس الأعمدة
const C_TITLE_BG:   u32 = 0x0F2548; // أزرق أعمق — شريط العنوان
const C_ALT_ROW:    u32 = 0xECF2FB; // أزرق فاتح جداً — صفوف متناوبة
const C_TOTAL_BG:   u32 = 0xFFF8E1; // أصفر فاتح — صف الإجمالي
const C_TOTAL_FG:   u32 = 0x7B4F00; // بني داكن — نص الإجمالي
const C_DATE_FG:    u32 = 0x5A6472; // رمادي — صف التاريخ
const C_BORDER:     u32 = 0xB0BEC5; // رمادي فاتح — حدود الخلايا
const C_HEADER_FG:  u32 = 0xFFFFFF; // أبيض — نص رأس الأعمدة
const C_TITLE_FG:   u32 = 0xFFFFFF; // أبيض — نص العنوان

// ─── تاريخ ووقت الإنشاء (UTC+2 ليبيا) ──────────────────────────────
fn current_datetime_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let local = secs + 2 * 3600;
    let h = (local / 3600) % 24;
    let m = (local / 60) % 60;
    let days = local / 86400;

    let mut rem = days;
    let mut year = 1970u64;
    loop {
        let diy = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 366 } else { 365 };
        if rem < diy { break; }
        rem -= diy;
        year += 1;
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mdays: [u64; 12] = [31, if is_leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for &d in &mdays {
        if rem < d { break; }
        rem -= d;
        month += 1;
    }
    let day = rem + 1;
    format!("{:02}/{:02}/{}  {:02}:{:02}", day, month, year, h, m)
}

// ─── تقدير عرض العمود من محتواه ────────────────────────────────────
fn estimate_col_width(header: &str, rows: &[Vec<String>], col_idx: usize) -> f64 {
    let header_len = header.chars().count();
    let max_data_len = rows.iter()
        .filter_map(|r| r.get(col_idx))
        .map(|v| v.chars().count())
        .max()
        .unwrap_or(0);
    // أعمدة عربية تحتاج عرضاً أكبر قليلاً
    let base = header_len.max(max_data_len) as f64;
    // نضاعف قليلاً للعربية، ونحدّد min/max
    (base * 1.3 + 2.0).max(10.0).min(45.0)
}

// ─── هل القيمة رقمية؟ ───────────────────────────────────────────────
fn parse_number(s: &str) -> Option<f64> {
    let clean = s.trim().replace(',', "");
    clean.parse::<f64>().ok()
}

// ─── حساب مجموع عمود رقمي ───────────────────────────────────────────
fn col_sum(rows: &[Vec<String>], col: usize) -> Option<f64> {
    let nums: Vec<f64> = rows.iter()
        .filter_map(|r| r.get(col)?.trim().replace(',', "").parse::<f64>().ok())
        .collect();
    if nums.len() > 1 { Some(nums.iter().sum()) } else { None }
}

// ─── الدالة الرئيسية ─────────────────────────────────────────────────
pub fn generate_report_excel(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Result<Vec<u8>, String> {
    let mut workbook = Workbook::new();
    let ws = workbook.add_worksheet();

    // ── RTL + اتجاه عربي ──────────────────────────────────────────
    ws.set_right_to_left(true);
    ws.set_tab_color(Color::RGB(C_HEADER_BG));
    ws.set_name("التقرير").map_err(|e| e.to_string())?;

    let n_cols = columns.len().max(1) as u16;
    let last_col = n_cols - 1;
    let datetime = current_datetime_str();

    // ─────────── تنسيقات الخلايا ───────────────────────────────────
    let fmt_title = Format::new()
        .set_bold()
        .set_font_size(15.0)
        .set_font_color(Color::RGB(C_TITLE_FG))
        .set_background_color(Color::RGB(C_TITLE_BG))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Medium)
        .set_border_color(Color::RGB(C_HEADER_BG));

    let fmt_date = Format::new()
        .set_font_size(9.0)
        .set_italic()
        .set_font_color(Color::RGB(C_DATE_FG))
        .set_background_color(Color::RGB(0xF5F7FA))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(C_BORDER));

    let fmt_header = Format::new()
        .set_bold()
        .set_font_size(11.0)
        .set_font_color(Color::RGB(C_HEADER_FG))
        .set_background_color(Color::RGB(C_HEADER_BG))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Medium)
        .set_border_color(Color::RGB(0x0F2548))
        .set_text_wrap();

    let fmt_cell_plain = Format::new()
        .set_font_size(10.0)
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(C_BORDER));

    let fmt_cell_alt = Format::new()
        .set_font_size(10.0)
        .set_background_color(Color::RGB(C_ALT_ROW))
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(C_BORDER));

    let fmt_number = Format::new()
        .set_font_size(10.0)
        .set_num_format("#,##0.##")
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(C_BORDER));

    let fmt_number_alt = Format::new()
        .set_font_size(10.0)
        .set_num_format("#,##0.##")
        .set_background_color(Color::RGB(C_ALT_ROW))
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(C_BORDER));

    let fmt_total_label = Format::new()
        .set_bold()
        .set_font_size(10.5)
        .set_font_color(Color::RGB(C_TOTAL_FG))
        .set_background_color(Color::RGB(C_TOTAL_BG))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Medium)
        .set_border_color(Color::RGB(0xD4A017));

    let fmt_total_num = Format::new()
        .set_bold()
        .set_font_size(10.5)
        .set_num_format("#,##0.##")
        .set_font_color(Color::RGB(C_TOTAL_FG))
        .set_background_color(Color::RGB(C_TOTAL_BG))
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Medium)
        .set_border_color(Color::RGB(0xD4A017));

    let fmt_total_empty = Format::new()
        .set_background_color(Color::RGB(C_TOTAL_BG))
        .set_border(FormatBorder::Medium)
        .set_border_color(Color::RGB(0xD4A017));

    // ─────────── الصفوف التخطيطية ──────────────────────────────────
    // صف 0: عنوان التقرير (دمج كل الأعمدة)
    ws.set_row_height(0, 28.0).ok();
    ws.merge_range(0, 0, 0, last_col, title, &fmt_title)
        .map_err(|e| e.to_string())?;

    // صف 1: تاريخ/وقت الإنشاء
    ws.set_row_height(1, 18.0).ok();
    let date_label = format!("تاريخ الإنشاء: {}  |  عدد السجلات: {}", datetime, rows.len().min(MAX_ROWS));
    ws.merge_range(1, 0, 1, last_col, &date_label, &fmt_date)
        .map_err(|e| e.to_string())?;

    // صف 2: رأس الأعمدة
    ws.set_row_height(2, 22.0).ok();
    for (c, col_name) in columns.iter().enumerate() {
        ws.write_string_with_format(2, c as u16, col_name, &fmt_header)
            .map_err(|e| e.to_string())?;
    }

    // ─────────── صفوف البيانات ──────────────────────────────────────
    let row_count = rows.len().min(MAX_ROWS);
    for (r, row) in rows.iter().take(MAX_ROWS).enumerate() {
        let excel_row = 3 + r as u32;
        ws.set_row_height(excel_row, 17.0).ok();
        let is_alt = r % 2 == 1;

        for (c, val) in row.iter().enumerate() {
            let trimmed = val.trim();
            if let Some(num) = parse_number(trimmed) {
                let fmt = if is_alt { &fmt_number_alt } else { &fmt_number };
                ws.write_number_with_format(excel_row, c as u16, num, fmt)
                    .map_err(|e| e.to_string())?;
            } else {
                let fmt = if is_alt { &fmt_cell_alt } else { &fmt_cell_plain };
                ws.write_string_with_format(excel_row, c as u16, val, fmt)
                    .map_err(|e| e.to_string())?;
            }
        }
        // ملء الخلايا الفارغة بتنسيق (لعرض الحدود)
        for c in row.len()..columns.len() {
            let fmt = if is_alt { &fmt_cell_alt } else { &fmt_cell_plain };
            ws.write_string_with_format(excel_row, c as u16, "", fmt)
                .map_err(|e| e.to_string())?;
        }
    }

    // ─────────── صف الإجمالي (إن وجدت أعمدة رقمية) ─────────────────
    let has_numbers = columns.iter().enumerate().any(|(c, _)| {
        col_sum(rows, c).is_some()
    });
    if has_numbers && row_count > 0 {
        let total_row = 3 + row_count as u32;
        ws.set_row_height(total_row, 20.0).ok();
        let mut any_total = false;
        for c in 0..columns.len() {
            if let Some(total) = col_sum(rows, c) {
                ws.write_number_with_format(total_row, c as u16, total, &fmt_total_num)
                    .map_err(|e| e.to_string())?;
                any_total = true;
            } else if !any_total && c == 0 {
                ws.write_string_with_format(total_row, 0, "الإجمالي", &fmt_total_label)
                    .map_err(|e| e.to_string())?;
            } else {
                ws.write_string_with_format(total_row, c as u16, "", &fmt_total_empty)
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    // ─────────── عرض الأعمدة التلقائي ──────────────────────────────
    for (c, col_name) in columns.iter().enumerate() {
        let w = estimate_col_width(col_name, rows, c);
        ws.set_column_width(c as u16, w).ok();
    }

    // ─────────── تجميد رأس الأعمدة + فلتر تلقائي ────────────────────
    ws.set_freeze_panes(3, 0).map_err(|e| e.to_string())?;
    if !columns.is_empty() && row_count > 0 {
        let last_data_row = 3 + row_count as u32 - 1;
        ws.autofilter(2, 0, last_data_row, last_col)
            .map_err(|e| e.to_string())?;
    }

    workbook.save_to_buffer().map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke_excel_arabic() {
        let cols = vec!["\u{062A}\u{0627}\u{0631}\u{064A}\u{062E}".into(), "\u{0645}\u{0628}\u{0644}\u{063A}".into()];
        let rows = vec![vec!["2026-05-21".into(), "1084.0".into()]];
        let title = "\u{0645}\u{0628}\u{064A}\u{0639}\u{0627}\u{062A}";
        let bytes = generate_report_excel(title, &cols, &rows).unwrap();
        assert!(bytes.len() > 100);
    }
}
