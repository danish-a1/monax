//! Orientation parsing of hostile bytes: never panics, unknown reads as 1.
use super::orientation;

/// SOI, an APP1 holding `payload`, then a minimal frame header and scan.
fn jpeg(payload: &[u8]) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE1];
    b.extend(u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    b.extend(payload);
    b.extend([
        0xFF, 0xC0, 0, 11, 8, 0, 1, 0, 2, 1, 1, 0x11, 0, 0xFF, 0xDA, 0, 2, 0xFF, 0xD9,
    ]);
    b
}

/// Exif payload with one IFD0 entry `(tag, type, value)`.
fn exif(le: bool, tag: u16, ty: u16, value: u16) -> Vec<u8> {
    let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut v = b"Exif\0\0".to_vec();
    v.extend(if le { b"II" } else { b"MM" });
    v.extend(u16b(42));
    v.extend(u32b(8));
    v.extend(u16b(1));
    v.extend(u16b(tag));
    v.extend(u16b(ty));
    v.extend(u32b(1));
    v.extend(u16b(value));
    v.extend([0, 0, 0, 0, 0, 0]);
    v
}

#[test]
fn reads_every_orientation_in_both_byte_orders() {
    for o in 1..=8 {
        for le in [true, false] {
            assert_eq!(orientation(&jpeg(&exif(le, 0x0112, 3, o))), o, "{o} {le}");
        }
    }
}

#[test]
fn anything_else_is_upright() {
    assert_eq!(orientation(&jpeg(b"")), 1);
    assert_eq!(
        orientation(&jpeg(&exif(true, 0x0112, 3, 9))),
        1,
        "out of range"
    );
    assert_eq!(
        orientation(&jpeg(&exif(true, 0x0112, 4, 6))),
        1,
        "not a SHORT"
    );
    assert_eq!(
        orientation(&jpeg(&exif(false, 0x0110, 3, 6))),
        1,
        "other tag"
    );
    assert_eq!(orientation(&[0xFF, 0xD8]), 1);
    assert_eq!(orientation(&[]), 1);
    // A scan before any APP1: never searched past.
    let mut late = vec![0xFF, 0xD8, 0xFF, 0xDA, 0, 2];
    late.extend(jpeg(&exif(true, 0x0112, 3, 6)));
    assert_eq!(orientation(&late), 1);
}

#[test]
fn truncated_or_corrupt_bytes_never_panic() {
    let full = jpeg(&exif(false, 0x0112, 3, 6));
    for n in 0..full.len() {
        let o = orientation(&full[..n]);
        assert!((1..=8).contains(&o), "{n}");
    }
    // Every byte of the APP1 set to 0x00 and 0xFF in turn (lengths,
    // offsets and counts pointing anywhere).
    for i in 2..30 {
        for v in [0x00, 0xFF, 0x80] {
            let mut b = full.clone();
            b[i] = v;
            assert!((1..=8).contains(&orientation(&b)), "{i} {v}");
        }
    }
}
