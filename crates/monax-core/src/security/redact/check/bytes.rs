//! Byte-level reading of content streams for the leak check: words,
//! string operands, qpdf JSON base64.
use super::Words;

/// ASCII alphanumeric runs of at least 4 bytes in `text`.
pub(super) fn words(text: &[u8]) -> Words {
    let runs = text.split(|b| !b.is_ascii_alphanumeric());
    runs.filter(|w| w.len() >= 4).map(<[u8]>::to_vec).collect()
}

/// The bytes of every string operand in content `data` (literal strings
/// with escapes and nesting, hex strings), separated by spaces; the strings
/// of one array (a TJ operand) are joined without one.
pub(super) fn strings(data: &[u8]) -> Vec<u8> {
    let (mut out, mut i, mut array) = (Vec::new(), 0, false);
    while i < data.len() {
        match data[i] {
            b'(' => {
                let mut depth = 1;
                i += 1;
                while i < data.len() && depth > 0 {
                    match data[i] {
                        b'\\' => i += 1, // escaped byte: kept as is
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    if depth > 0 && i < data.len() {
                        out.push(data[i]);
                    }
                    i += 1;
                }
            }
            b'<' if data.get(i + 1) != Some(&b'<') => {
                let end = data[i..]
                    .iter()
                    .position(|&b| b == b'>')
                    .map_or(data.len(), |e| i + e);
                let hex: Vec<u8> = data[i + 1..end]
                    .iter()
                    .copied()
                    .filter(u8::is_ascii_hexdigit)
                    .collect();
                let byte = |h: &[u8]| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok();
                out.extend(hex.chunks(2).filter_map(byte));
                i = end + 1;
            }
            b'<' => {
                i += 2;
                continue;
            }
            b'[' => (array, i) = (true, i + 1),
            b']' => (array, i) = (false, i + 1),
            _ => {
                i += 1;
                continue;
            }
        }
        if !array {
            out.push(b' ');
        }
    }
    out
}

/// Standard base64 (qpdf JSON stream data); `None` when malformed.
pub(super) fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for c in s.chunks(4) {
        let v: Vec<u8> = c.iter().map(|&b| val(b)).collect::<Option<_>>()?;
        let n = v.iter().fold(0u32, |n, &x| n << 6 | u32::from(x)) << (6 * (4 - v.len()));
        out.extend(&n.to_be_bytes()[1..v.len()]);
    }
    Some(out)
}
