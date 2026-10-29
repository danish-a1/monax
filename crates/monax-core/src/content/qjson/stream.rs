//! New streams for qpdf JSON updates: the dictionary and the raw data,
//! base64 (qpdf adds /Length and compresses it).
use serde_json::{Value, json};

/// A qpdf JSON v2 stream object of `dict` holding `data`.
pub(crate) fn stream(dict: Value, data: &[u8]) -> Value {
    json!({ "stream": { "dict": dict, "data": base64(data) } })
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [0, 1, 2].map(|i| chunk.get(i).copied().unwrap_or(0));
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for k in 0..4 {
            out.push(match k <= chunk.len() {
                true => char::from(ALPHABET[(n >> (18 - 6 * k) & 63) as usize]),
                false => '=',
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_to_four() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(&[0xff, 0xfe, 0x00, 0x80]), "//4AgA==");
    }

    #[test]
    fn stream_holds_dict_and_data() {
        let s = stream(json!({ "/Type": "/XObject" }), b"q Q");
        assert_eq!(
            s,
            json!({ "stream": { "dict": { "/Type": "/XObject" }, "data": "cSBR" } })
        );
    }
}
