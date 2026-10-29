//! JPEG marker segments, read without decoding: the frame size (SOFn) and
//! the EXIF Orientation (R143: APP1 `Exif\0\0`, TIFF IFD0 tag 0x0112,
//! either byte order). The bytes are hostile: every read is bounds-checked
//! and anything malformed ends the walk.

/// Marker segments of the JPEG `b` (which starts with SOI) as `(marker,
/// payload)`, up to and including the first scan (SOS, its payload all the
/// bytes after the marker: header, then entropy-coded data).
/// Ends early at an end of image or anything malformed.
fn segments(b: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    let (mut at, mut done) = (2, false);
    std::iter::from_fn(move || {
        while !done {
            if *b.get(at)? != 0xFF {
                return None;
            }
            while *b.get(at)? == 0xFF {
                at += 1; // fill bytes
            }
            let m = *b.get(at)?;
            at += 1;
            match m {
                0x01 | 0xD0..=0xD8 => continue, // no length
                0xD9 => return None,            // image ends
                0xDA => done = true,            // the scan: last
                _ => {}
            }
            if done {
                return Some((m, &b[at..]));
            }
            let len = usize::from(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?])).max(2);
            let seg = b.get(at + 2..at + len)?;
            at += len;
            return Some((m, seg));
        }
        None
    })
}

/// `(width, height)` of a JPEG from its frame header (SOFn), when both are
/// above 0, a quantization table (DQT) came first and a scan (SOS) follows
/// with at least one entropy-coded byte, then more bytes (EOI or data);
/// never decodes.
pub(super) fn jpeg_size(b: &[u8]) -> Option<(u16, u16)> {
    let (mut size, mut dqt) = (None, false);
    for (m, p) in segments(b) {
        dqt |= m == 0xDB;
        if m == 0xDA {
            let len = usize::from(u16::from_be_bytes([*p.first()?, *p.get(1)?]));
            // A marker (0xFF not stuffed as FF 00) is no entropy-coded byte.
            let data = match p.get(len..)? {
                [0xFF, 0x00, _, ..] => true,
                [0xFF, ..] => false,
                d => d.len() >= 2,
            };
            return size.filter(|&(w, h)| dqt && data && w > 0 && h > 0);
        }
        if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            // Precision, height, width.
            let be = |i: usize| Some(u16::from_be_bytes([*p.get(i)?, *p.get(i + 1)?]));
            size = Some((be(3)?, be(1)?));
        }
    }
    None
}

/// EXIF Orientation of the JPEG `b`: 1..=8, 1 when absent or unreadable.
/// Only the first APP1 Exif segment before the scan counts.
pub(super) fn orientation(b: &[u8]) -> u16 {
    segments(b)
        .find_map(|(m, p)| (m == 0xE1).then(|| p.strip_prefix(b"Exif\0\0")).flatten())
        .and_then(tiff)
        .filter(|o| (1..=8).contains(o))
        .unwrap_or(1)
}

/// Orientation from IFD0 of TIFF data `t`, when it is a SHORT.
fn tiff(t: &[u8]) -> Option<u16> {
    let le = match t.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |i: usize| {
        let s = t.get(i..i.checked_add(2)?)?;
        let s = [s[0], s[1]];
        Some(if le {
            u16::from_le_bytes(s)
        } else {
            u16::from_be_bytes(s)
        })
    };
    let u32_at = |i: usize| {
        let s = t.get(i..i.checked_add(4)?)?;
        let s = [s[0], s[1], s[2], s[3]];
        Some(if le {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let ifd = usize::try_from(u32_at(4)?).ok()?;
    for k in 0..usize::from(u16_at(ifd)?) {
        let entry = ifd.checked_add(2)?.checked_add(k.checked_mul(12)?)?;
        if u16_at(entry)? == 0x0112 {
            // Type SHORT (3), its value in the entry's first value bytes.
            return (u16_at(entry.checked_add(2)?)? == 3)
                .then(|| u16_at(entry.checked_add(8)?))
                .flatten();
        }
    }
    None
}

#[cfg(test)]
mod tests;
