//! qpdf JSON strings and names as text. qpdf writes a string as `u:` text
//! only when it is UTF-16 or plain ASCII; others come as `b:` hex bytes,
//! decoded here as a PDF text string (PDF 32000 7.9.2.2).
use serde_json::Value;

/// A qpdf JSON string as text: `u:` as is, `b:` bytes as UTF-16BE (with
/// its BOM), UTF-8 (with its BOM) or PDFDocEncoding.
pub(super) fn text(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    if let Some(t) = s.strip_prefix("u:") {
        return Some(t.to_string());
    }
    let hex = s.strip_prefix("b:")?;
    let b: Vec<u8> = (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok())
        .collect();
    Some(match b.as_slice() {
        [0xFE, 0xFF, rest @ ..] => {
            let units = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c));
            char::decode_utf16(units)
                .map(|c| c.unwrap_or('\u{FFFD}'))
                .collect()
        }
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => b.iter().map(|&c| pdfdoc(c)).collect(),
    })
}

/// PDFDocEncoding: Latin-1 but for 0x18..=0x1F and 0x80..=0xA0.
fn pdfdoc(c: u8) -> char {
    const LOW: [u32; 8] = [0x2D8, 0x2C7, 0x2C6, 0x2D9, 0x2DD, 0x2DB, 0x2DA, 0x2DC];
    const HIGH: [u32; 33] = [
        0x2022, 0x2020, 0x2021, 0x2026, 0x2014, 0x2013, 0x192, 0x2044, 0x2039, 0x203A, 0x2212,
        0x2030, 0x201E, 0x201C, 0x201D, 0x2018, 0x2019, 0x201A, 0x2122, 0xFB01, 0xFB02, 0x141,
        0x152, 0x160, 0x178, 0x17D, 0x131, 0x142, 0x153, 0x161, 0x17E, 0xFFFD, 0x20AC,
    ];
    let u = match c {
        0x18..=0x1F => LOW[usize::from(c - 0x18)],
        0x80..=0xA0 => HIGH[usize::from(c - 0x80)],
        0x7F | 0xAD => 0xFFFD,
        _ => u32::from(c),
    };
    char::from_u32(u).unwrap_or('\u{FFFD}')
}

/// A qpdf JSON name (`/Agreed`; `n:/A#20b` when not UTF-8) as text.
pub(super) fn name(raw: &str) -> String {
    let Some(esc) = raw.strip_prefix("n:/") else {
        return raw.strip_prefix('/').unwrap_or(raw).to_string();
    };
    let (b, mut out, mut i) = (esc.as_bytes(), Vec::new(), 0);
    while i < b.len() {
        let hex = esc
            .get(i + 1..i + 3)
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b[i], hex) {
            (b'#', Some(x)) => (out.push(x), i += 3),
            (c, _) => (out.push(c), i += 1),
        };
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_decode_as_pdf_text_strings() {
        let t = |s: &str| text(&json!(s));
        assert_eq!(t("u:Ann").as_deref(), Some("Ann"));
        assert_eq!(t("b:5a656420dc6eef").as_deref(), Some("Zed Ünï"));
        assert_eq!(t("b:84a018").as_deref(), Some("\u{2014}\u{20AC}\u{2D8}"));
        assert_eq!(t("b:feff00410416").as_deref(), Some("A\u{416}"));
        assert_eq!(t("b:efbbbfc3a9").as_deref(), Some("é"));
        assert_eq!(t("b:7fad9f").as_deref(), Some("\u{FFFD}\u{FFFD}\u{FFFD}"));
        assert_eq!(text(&json!(3)), None);
        assert_eq!(t("/Name"), None);
    }

    #[test]
    fn names_drop_the_slash_and_decode_escapes() {
        assert_eq!(name("/Agreed"), "Agreed");
        assert_eq!(name("n:/A#20b#e9"), "A b\u{FFFD}");
        assert_eq!(name("n:/x#2"), "x#2");
    }
}
