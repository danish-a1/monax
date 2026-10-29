//! Overlay text rules: `%[page]` / `%[pages]` placeholders and the WinAnsi
//! character set of the standard-14 Helvetica font.

/// `template` with `%[page]` replaced by `page` and `%[pages]` by `pages`.
/// Independent of the file-name patterns in `names`.
pub(crate) fn expand(template: &str, page: u64, pages: usize) -> String {
    template
        .replace("%[page]", &page.to_string())
        .replace("%[pages]", &pages.to_string())
}

/// Fails with the first character of `text` that Helvetica (WinAnsiEncoding)
/// cannot show, so no glyph is ever dropped or substituted.
pub(crate) fn check_winansi(text: &str) -> Result<(), String> {
    match text.chars().find(|&c| !winansi(c)) {
        None => Ok(()),
        Some(c) => Err(format!(
            "character '{c}' (U+{:04X}) is not supported by Helvetica (WinAnsi)",
            u32::from(c)
        )),
    }
}

/// What WinAnsiEncoding (cp1252) maps 0x80..=0x9F to; `\0` where it maps
/// nothing.
const CP1252_HIGH: [char; 32] = [
    '\u{20ac}', '\0', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\0', '\u{17d}', '\0', '\0',
    '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{2dc}',
    '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\0', '\u{17e}', '\u{178}',
];

fn winansi(c: char) -> bool {
    winansi_byte(c).is_some()
}

/// The WinAnsiEncoding byte that shows `c` in Helvetica; `None` when it
/// has none (control characters included).
pub(crate) fn winansi_byte(c: char) -> Option<u8> {
    match c {
        ' '..='~' | '\u{a0}'..='\u{ff}' => u8::try_from(c).ok(),
        '\0' => None,
        _ => CP1252_HIGH
            .iter()
            .position(|&h| h == c)
            .and_then(|i| u8::try_from(0x80 + i).ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_page_and_total() {
        assert_eq!(expand("Page %[page] of %[pages]", 4, 9), "Page 4 of 9");
        assert_eq!(expand("%[page]%[page]", 12, 1), "1212");
        assert_eq!(expand("no tokens %[date]", 1, 1), "no tokens %[date]");
    }

    #[test]
    fn winansi_accepts_latin1_and_cp1252_extras() {
        assert!(check_winansi("Hello, World! 123 ~").is_ok());
        assert!(check_winansi("caf\u{e9} \u{a9} \u{ff}").is_ok());
        assert!(check_winansi("\u{20ac} \u{2014} \u{2122} \u{201c}x\u{201d}").is_ok());
    }

    #[test]
    fn winansi_bytes_are_cp1252() {
        assert_eq!(winansi_byte('A'), Some(b'A'));
        assert_eq!(winansi_byte('\u{e9}'), Some(0xe9));
        assert_eq!(winansi_byte('\u{20ac}'), Some(0x80));
        assert_eq!(winansi_byte('\u{2014}'), Some(0x97));
        assert_eq!(winansi_byte('\u{178}'), Some(0x9f));
        for c in ['\n', '\u{7f}', '\u{81}', '\0', '\u{3b1}'] {
            assert_eq!(winansi_byte(c), None, "{c:?}");
        }
    }

    #[test]
    fn winansi_names_first_unsupported_char() {
        let e = check_winansi("ok \u{3b1}\u{3b2}").unwrap_err();
        assert_eq!(
            e,
            "character '\u{3b1}' (U+03B1) is not supported by Helvetica (WinAnsi)"
        );
        assert!(check_winansi("a\nb").is_err());
        assert!(check_winansi("\u{7f}").is_err());
        assert!(check_winansi("\u{81}").is_err());
        assert!(check_winansi("\u{1f600}").is_err());
    }
}
