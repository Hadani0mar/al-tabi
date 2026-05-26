use printpdf::*;
use std::io::{BufWriter, Cursor};
use unicode_bidi::{BidiInfo, Level};

// ─── أبعاد الصفحة (A4 أفقي) ──────────────────────────────────────
const PW: f64 = 297.0;
const PH: f64 = 210.0;
const MARGIN: f64 = 8.0;

const TITLE_H: f64 = 11.0;
const CONT_H:  f64 = 7.5;
const HDR_H:   f64 = 8.0;
const ROW_H:   f64 = 6.5;
const FOOTER_H: f64 = 6.0;

// ─── الدالة الرئيسية ──────────────────────────────────────────────
pub fn generate_report_pdf(
    title: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> Result<Vec<u8>, String> {
    let n_cols = columns.len().max(1);

    // حجم الخط بحسب عدد الأعمدة
    let (title_fs, hdr_fs, body_fs, footer_fs) = adaptive_font_sizes(n_cols);

    // عرض كل عمود بحسب نوع محتواه
    let content_w = PW - 2.0 * MARGIN;
    let col_widths = compute_col_widths(columns, content_w);

    // عدد الصفوف في الصفحة
    let body_h_first = PH - 2.0 * MARGIN - TITLE_H - HDR_H - FOOTER_H;
    let body_h_rest  = PH - 2.0 * MARGIN - CONT_H  - HDR_H - FOOTER_H;
    let rpp_first = ((body_h_first / ROW_H).floor() as usize).max(1);
    let rpp_rest  = ((body_h_rest  / ROW_H).floor() as usize).max(1);

    let max_rows  = 2000usize;
    let row_data: Vec<&Vec<String>> = rows.iter().take(max_rows).collect();

    let n_pages = if row_data.is_empty() { 1 } else {
        let after_first = row_data.len().saturating_sub(rpp_first);
        1 + (after_first + rpp_rest - 1) / rpp_rest.max(1)
    };

    // ── إنشاء المستند ───────────────────────────────────────────────
    let (doc, p0, l0) = PdfDocument::new(title, Mm(PW), Mm(PH), "Layer 1");
    let font = load_arabic_font(&doc)?;
    let mut pages = vec![(p0, l0)];
    for _ in 1..n_pages {
        let (p, l) = doc.add_page(Mm(PW), Mm(PH), "Layer 1");
        pages.push((p, l));
    }

    let mut row_cursor = 0usize;

    for (pi, &(pr, lr)) in pages.iter().enumerate() {
        let layer = doc.get_page(pr).get_layer(lr);
        let top_y = PH - MARGIN;

        // خلفية بيضاء
        fill_rect(&layer, 0.0, 0.0, PW, PH, [1.0, 1.0, 1.0]);

        // ─── شريط العنوان ──────────────────────────────────────────
        let (strip_h, strip_fs) = if pi == 0 { (TITLE_H, title_fs) } else { (CONT_H, footer_fs) };
        fill_rect(&layer, MARGIN, top_y - strip_h, PW - MARGIN, top_y, [0.10, 0.22, 0.45]);
        layer.set_fill_color(Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)));
        let label = if pi == 0 { title.to_string() }
                    else { format!("{} - {} {}", title, pi + 1, "تابع") };
        // نحسب عرض النص المقدّر ثم نضعه محاذياً لليمين داخل الشريط
        let bar_inner_w = content_w - 6.0; // 3mm padding من كل طرف
        let label_prepared = prepare_text(&label);
        let label_fitted   = fit_text(&label_prepared, bar_inner_w, strip_fs);
        let n_chars        = label_fitted.chars().count() as f64;
        let est_text_w     = n_chars * strip_fs * 0.195; // تقدير عرض النص بـ mm
        // ابدأ من اليمين: نطرح عرض النص من الحافة اليمنى
        let title_x = (PW - MARGIN - 3.0 - est_text_w).max(MARGIN + 3.0);
        let title_y = top_y - strip_h + (strip_h - strip_fs * 0.35) / 2.0;
        layer.use_text(&label_fitted, strip_fs, Mm(title_x), Mm(title_y), &font);

        let hdr_top = top_y - strip_h;

        // ─── رأس الأعمدة ───────────────────────────────────────────
        fill_rect(&layer, MARGIN, hdr_top - HDR_H, PW - MARGIN, hdr_top, [0.18, 0.37, 0.65]);
        layer.set_fill_color(Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)));

        let mut cx = PW - MARGIN; // نبدأ من اليمين
        for (ci, col) in columns.iter().enumerate() {
            let cw = col_widths[ci];
            let cell_right = cx;
            let cell_left  = cx - cw;
            let text = fit_text(&prepare_text(col), cw - 3.0, hdr_fs);
            layer.use_text(&text, hdr_fs, Mm(cell_left + 1.5), Mm(hdr_top - HDR_H + 2.0), &font);
            // خط فاصل عمودي
            draw_vline(&layer, cell_left, hdr_top - HDR_H, hdr_top, [0.30, 0.48, 0.72]);
            cx -= cw;
            let _ = cell_right;
        }

        // ─── صفوف البيانات ─────────────────────────────────────────
        let rpp = if pi == 0 { rpp_first } else { rpp_rest };
        let body_top = hdr_top - HDR_H;
        let r_end = (row_cursor + rpp).min(row_data.len());

        for (ri, row) in row_data[row_cursor..r_end].iter().enumerate() {
            let row_top = body_top - ri as f64 * ROW_H;
            let row_bot = row_top - ROW_H;

            // خلفية متناوبة
            if ri % 2 == 1 {
                fill_rect(&layer, MARGIN, row_bot, PW - MARGIN, row_top, [0.95, 0.97, 1.00]);
            }

            // نص الخلايا + خطوط عمودية
            let mut cx2 = PW - MARGIN;
            for (ci, cell) in row.iter().enumerate() {
                let cw = col_widths[ci];
                let cell_left = cx2 - cw;

                // نص مقتطع ومنسّق
                layer.set_fill_color(Color::Rgb(Rgb::new(0.08, 0.08, 0.08, None)));
                let text = fit_text(&prepare_text(cell), cw - 3.0, body_fs);
                layer.use_text(&text, body_fs, Mm(cell_left + 1.5), Mm(row_bot + 1.8), &font);

                // خط فاصل عمودي
                draw_vline(&layer, cell_left, row_bot, row_top, [0.82, 0.82, 0.82]);
                cx2 -= cw;
            }

            // خط أفقي سفلي للصف
            layer.set_outline_color(Color::Rgb(Rgb::new(0.82, 0.82, 0.82, None)));
            layer.set_outline_thickness(0.15);
            layer.add_shape(Line {
                points: vec![
                    (Point::new(Mm(MARGIN),      Mm(row_bot)), false),
                    (Point::new(Mm(PW - MARGIN), Mm(row_bot)), false),
                ],
                is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false,
            });
        }

        row_cursor = r_end;

        // ─── إطار الجدول الخارجي ────────────────────────────────────
        let table_bot = MARGIN + FOOTER_H + 1.0;
        layer.set_outline_color(Color::Rgb(Rgb::new(0.55, 0.55, 0.55, None)));
        layer.set_outline_thickness(0.6);
        layer.add_shape(Line {
            points: vec![
                (Point::new(Mm(MARGIN),      Mm(table_bot)), false),
                (Point::new(Mm(PW - MARGIN), Mm(table_bot)), false),
                (Point::new(Mm(PW - MARGIN), Mm(top_y)),     false),
                (Point::new(Mm(MARGIN),      Mm(top_y)),     false),
            ],
            is_closed: true, has_fill: false, has_stroke: true, is_clipping_path: false,
        });

        // ─── الفوتر ────────────────────────────────────────────────
        fill_rect(&layer, MARGIN, MARGIN, PW - MARGIN, MARGIN + FOOTER_H, [0.92, 0.94, 0.97]);
        layer.set_fill_color(Color::Rgb(Rgb::new(0.35, 0.35, 0.35, None)));

        // يسار: رقم الصفحة
        layer.use_text(
            &prepare_text(&format!("{} / {}", pi + 1, n_pages)),
            footer_fs, Mm(MARGIN + 2.0), Mm(MARGIN + 1.5), &font,
        );
        // وسط: تحذير الحد الأقصى
        if rows.len() > max_rows {
            layer.use_text(
                &prepare_text(&format!("{} / {} صف", max_rows, rows.len())),
                footer_fs - 0.5, Mm(PW / 2.0 - 12.0), Mm(MARGIN + 1.5), &font,
            );
        }
        // يمين: إجمالي
        layer.use_text(
            &prepare_text(&format!("النتائج: {}", row_data.len())),
            footer_fs, Mm(PW - MARGIN - 24.0), Mm(MARGIN + 1.5), &font,
        );
    }

    // ── حفظ ─────────────────────────────────────────────────────────
    let mut output = Vec::new();
    doc.save(&mut BufWriter::new(Cursor::new(&mut output)))
        .map_err(|e| format!("فشل حفظ PDF: {}", e))?;
    Ok(output)
}

// ─── حجم الخط التكيّفي ────────────────────────────────────────────
fn adaptive_font_sizes(n_cols: usize) -> (f64, f64, f64, f64) {
    // (title, header, body, footer)
    match n_cols {
        0..=5  => (12.0, 8.0, 7.5, 6.0),
        6..=8  => (11.0, 7.0, 6.5, 5.5),
        9..=11 => (10.0, 6.0, 5.8, 5.0),
        _      => ( 9.0, 5.5, 5.2, 4.8),
    }
}

// ─── حساب عرض الأعمدة بحسب النوع ────────────────────────────────
fn compute_col_widths(columns: &[String], total_w: f64) -> Vec<f64> {
    let weights: Vec<f64> = columns.iter().map(|c| col_weight(c)).collect();
    let sum: f64 = weights.iter().sum();
    weights.iter().map(|w| (w / sum) * total_w).collect()
}

/// وزن العمود النسبي بحسب اسمه
fn col_weight(name: &str) -> f64 {
    // أعمدة ضيّقة: أرقام / كميات / معدلات
    if contains_any(name, &["رصيد", "كمية", "معدل", "أيام", "ايام", "أولوية", "اولوية",
                             "اتجاه", "أتجاه", "مبيعات", "تغطية", "موديل", "رقم"]) {
        return 0.55;
    }
    // أعمدة ضيقة-متوسطة: أسعار / مبالغ
    if contains_any(name, &["سعر", "تكلفة", "تأمين", "تامين", "اجمالي", "إجمالي", "شراء", "بيع"]) {
        return 0.80;
    }
    // أعمدة متوسطة: تواريخ / أكواد
    if contains_any(name, &["تاريخ", "كود", "رمز"]) {
        return 0.85;
    }
    // أعمدة عريضة: أسماء
    if contains_any(name, &["اسم", "مورد", "منتج", "ادخله", "مستخدم"]) {
        return 1.40;
    }
    // افتراضي
    1.0
}

fn contains_any(s: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| s.contains(k))
}

// ─── اقتطاع النص ليناسب عرض الخلية ──────────────────────────────
/// يقدّر عدد الأحرف التي تتسع في cell_w mm عند حجم font_size pt
/// ثم يقتطع النص مع "..." إن كان أطول.
fn fit_text(text: &str, cell_w_mm: f64, font_size: f64) -> String {
    if cell_w_mm <= 0.0 { return String::new(); }
    // تقدير عرض الحرف: حرف عربي أوسع من اللاتيني بعد الشكل
    let char_w = font_size * 0.195; // mm لكل حرف (تقريب تجريبي)
    let max_chars = (cell_w_mm / char_w).floor() as usize;
    let max_chars = max_chars.max(3);
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        text.to_string()
    } else {
        let cut = max_chars.saturating_sub(2);
        chars[..cut].iter().collect::<String>() + ".."
    }
}

// ─── خط فاصل عمودي ───────────────────────────────────────────────
fn draw_vline(layer: &PdfLayerReference, x: f64, y_bot: f64, y_top: f64, rgb: [f64; 3]) {
    layer.set_outline_color(Color::Rgb(Rgb::new(rgb[0], rgb[1], rgb[2], None)));
    layer.set_outline_thickness(0.3);
    layer.add_shape(Line {
        points: vec![
            (Point::new(Mm(x), Mm(y_bot)), false),
            (Point::new(Mm(x), Mm(y_top)), false),
        ],
        is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false,
    });
}

// ─── إعداد النص: تشكيل عربي + ترتيب بصري ────────────────────────
fn prepare_text(text: &str) -> String {
    visual_order(&reshape_arabic(text))
}

// ─── Arabic Reshaper ─────────────────────────────────────────────
fn reshape_arabic(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n * 3);
    let mut i = 0;
    while i < n {
        let ch = chars[i];
        // ربط لام-ألف
        if ch == '\u{0644}' && i + 1 < n {
            let mut j = i + 1;
            while j < n && is_transparent(chars[j]) { j += 1; }
            if j < n {
                if let Some((fin_f, iso_f)) = lam_alef_ligature(chars[j]) {
                    let prev_joins = i > 0 && left_joins(chars[i - 1]);
                    out.push(if prev_joins { fin_f } else { iso_f });
                    for k in (i + 1)..j { out.push(chars[k]); }
                    i = j + 1;
                    continue;
                }
            }
        }
        if let Some(forms) = arabic_letter_forms(ch) {
            let pj = prev_joinable(&chars, i).map_or(false, |c| left_joins(c));
            let nj = is_dual_joining(ch) && next_joinable(&chars, i).map_or(false, |c| right_joins(c));
            let idx = match (pj, nj) { (false,false)=>0,(true,false)=>1,(false,true)=>2,(true,true)=>3 };
            out.push(forms[idx]);
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}

fn prev_joinable(chars: &[char], i: usize) -> Option<char> {
    if i == 0 { return None; }
    let mut j = i as isize - 1;
    while j >= 0 { let c = chars[j as usize]; if !is_transparent(c) { return Some(c); } j -= 1; }
    None
}
fn next_joinable(chars: &[char], i: usize) -> Option<char> {
    let mut j = i + 1;
    while j < chars.len() { let c = chars[j]; if !is_transparent(c) { return Some(c); } j += 1; }
    None
}
fn is_dual_joining(ch: char) -> bool {
    matches!(ch, '\u{0626}'|'\u{0628}'|'\u{062A}'|'\u{062B}'|'\u{062C}'|'\u{062D}'|'\u{062E}'
               |'\u{0633}'|'\u{0634}'|'\u{0635}'|'\u{0636}'|'\u{0637}'|'\u{0638}'|'\u{0639}'
               |'\u{063A}'|'\u{0641}'|'\u{0642}'|'\u{0643}'|'\u{0644}'|'\u{0645}'|'\u{0646}'
               |'\u{0647}'|'\u{064A}'|'\u{0640}')
}
fn right_joins(ch: char) -> bool {
    is_dual_joining(ch) || matches!(ch,
        '\u{0622}'|'\u{0623}'|'\u{0624}'|'\u{0625}'|'\u{0627}'|'\u{0629}'
       |'\u{062F}'|'\u{0630}'|'\u{0631}'|'\u{0632}'|'\u{0648}'|'\u{0649}')
}
fn left_joins(ch: char) -> bool { is_dual_joining(ch) }
fn is_transparent(ch: char) -> bool {
    matches!(ch as u32, 0x064B..=0x065F|0x0670|0x06D6..=0x06DC|0x06DF..=0x06E4|0x06E7..=0x06E8|0x06EA..=0x06ED)
}
fn arabic_letter_forms(ch: char) -> Option<[char; 4]> {
    match ch {
        '\u{0621}'=>Some(['\u{FE80}','\u{FE80}','\u{FE80}','\u{FE80}']),
        '\u{0622}'=>Some(['\u{FE81}','\u{FE82}','\u{FE81}','\u{FE82}']),
        '\u{0623}'=>Some(['\u{FE83}','\u{FE84}','\u{FE83}','\u{FE84}']),
        '\u{0624}'=>Some(['\u{FE85}','\u{FE86}','\u{FE85}','\u{FE86}']),
        '\u{0625}'=>Some(['\u{FE87}','\u{FE88}','\u{FE87}','\u{FE88}']),
        '\u{0626}'=>Some(['\u{FE89}','\u{FE8A}','\u{FE8B}','\u{FE8C}']),
        '\u{0627}'=>Some(['\u{FE8D}','\u{FE8E}','\u{FE8D}','\u{FE8E}']),
        '\u{0628}'=>Some(['\u{FE8F}','\u{FE90}','\u{FE91}','\u{FE92}']),
        '\u{0629}'=>Some(['\u{FE93}','\u{FE94}','\u{FE93}','\u{FE94}']),
        '\u{062A}'=>Some(['\u{FE95}','\u{FE96}','\u{FE97}','\u{FE98}']),
        '\u{062B}'=>Some(['\u{FE99}','\u{FE9A}','\u{FE9B}','\u{FE9C}']),
        '\u{062C}'=>Some(['\u{FE9D}','\u{FE9E}','\u{FE9F}','\u{FEA0}']),
        '\u{062D}'=>Some(['\u{FEA1}','\u{FEA2}','\u{FEA3}','\u{FEA4}']),
        '\u{062E}'=>Some(['\u{FEA5}','\u{FEA6}','\u{FEA7}','\u{FEA8}']),
        '\u{062F}'=>Some(['\u{FEA9}','\u{FEAA}','\u{FEA9}','\u{FEAA}']),
        '\u{0630}'=>Some(['\u{FEAB}','\u{FEAC}','\u{FEAB}','\u{FEAC}']),
        '\u{0631}'=>Some(['\u{FEAD}','\u{FEAE}','\u{FEAD}','\u{FEAE}']),
        '\u{0632}'=>Some(['\u{FEAF}','\u{FEB0}','\u{FEAF}','\u{FEB0}']),
        '\u{0633}'=>Some(['\u{FEB1}','\u{FEB2}','\u{FEB3}','\u{FEB4}']),
        '\u{0634}'=>Some(['\u{FEB5}','\u{FEB6}','\u{FEB7}','\u{FEB8}']),
        '\u{0635}'=>Some(['\u{FEB9}','\u{FEBA}','\u{FEBB}','\u{FEBC}']),
        '\u{0636}'=>Some(['\u{FEBD}','\u{FEBE}','\u{FEBF}','\u{FEC0}']),
        '\u{0637}'=>Some(['\u{FEC1}','\u{FEC2}','\u{FEC3}','\u{FEC4}']),
        '\u{0638}'=>Some(['\u{FEC5}','\u{FEC6}','\u{FEC7}','\u{FEC8}']),
        '\u{0639}'=>Some(['\u{FEC9}','\u{FECA}','\u{FECB}','\u{FECC}']),
        '\u{063A}'=>Some(['\u{FECD}','\u{FECE}','\u{FECF}','\u{FED0}']),
        '\u{0641}'=>Some(['\u{FED1}','\u{FED2}','\u{FED3}','\u{FED4}']),
        '\u{0642}'=>Some(['\u{FED5}','\u{FED6}','\u{FED7}','\u{FED8}']),
        '\u{0643}'=>Some(['\u{FED9}','\u{FEDA}','\u{FEDB}','\u{FEDC}']),
        '\u{0644}'=>Some(['\u{FEDD}','\u{FEDE}','\u{FEDF}','\u{FEE0}']),
        '\u{0645}'=>Some(['\u{FEE1}','\u{FEE2}','\u{FEE3}','\u{FEE4}']),
        '\u{0646}'=>Some(['\u{FEE5}','\u{FEE6}','\u{FEE7}','\u{FEE8}']),
        '\u{0647}'=>Some(['\u{FEE9}','\u{FEEA}','\u{FEEB}','\u{FEEC}']),
        '\u{0648}'=>Some(['\u{FEED}','\u{FEEE}','\u{FEED}','\u{FEEE}']),
        '\u{0649}'=>Some(['\u{FEEF}','\u{FEF0}','\u{FEEF}','\u{FEF0}']),
        '\u{064A}'=>Some(['\u{FEF1}','\u{FEF2}','\u{FEF3}','\u{FEF4}']),
        _ => None,
    }
}
fn lam_alef_ligature(ch: char) -> Option<(char, char)> {
    match ch {
        '\u{0622}'=>Some(('\u{FEF8}','\u{FEF7}')),
        '\u{0623}'=>Some(('\u{FEF6}','\u{FEF5}')),
        '\u{0625}'=>Some(('\u{FEFA}','\u{FEF9}')),
        '\u{0627}'=>Some(('\u{FEFC}','\u{FEFB}')),
        _ => None,
    }
}

// ─── ترتيب بصري BiDi ─────────────────────────────────────────────
fn visual_order(text: &str) -> String {
    if text.trim().is_empty() { return text.to_string(); }
    let bidi = BidiInfo::new(text, Some(Level::rtl()));
    if bidi.paragraphs.is_empty() { return text.to_string(); }
    let para = &bidi.paragraphs[0];
    bidi.reorder_line(para, para.range.clone()).into_owned()
}

// ─── تحميل الخط ──────────────────────────────────────────────────
fn load_arabic_font(doc: &PdfDocumentReference) -> Result<IndirectFontRef, String> {
    let candidates = [
        r"C:\Windows\Fonts\arialuni.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
        r"C:\Windows\Fonts\calibri.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = doc.add_external_font(Cursor::new(bytes)) {
                return Ok(font);
            }
        }
    }
    doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| format!("فشل الخط: {}", e))
}

// ─── مستطيل ممتلئ ────────────────────────────────────────────────
fn fill_rect(layer: &PdfLayerReference, x1: f64, y1: f64, x2: f64, y2: f64, rgb: [f64; 3]) {
    layer.set_fill_color(Color::Rgb(Rgb::new(rgb[0], rgb[1], rgb[2], None)));
    layer.add_shape(Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y2)), false),
            (Point::new(Mm(x1), Mm(y2)), false),
        ],
        is_closed: true, has_fill: true, has_stroke: false, is_clipping_path: false,
    });
}
