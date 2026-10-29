//! Downsampling and JPEG encoding of decoded 8-bit image samples (`n`
//! interleaved channels per pixel: 1 gray, 3 RGB).

/// Area-averages `src` (`w` x `h` pixels) down to `tw` x `th` (each at most
/// the source size and at least 1).
pub(super) fn shrink(src: &[u8], (w, h): (u32, u32), n: usize, (tw, th): (u32, u32)) -> Vec<u8> {
    let (w, h, tw, th) = (w as usize, h as usize, tw as usize, th as usize);
    let mut out = Vec::with_capacity(tw * th * n);
    for ty in 0..th {
        let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let area = ((y1 - y0) * (x1 - x0)) as u32;
            for c in 0..n {
                let mut sum = 0u32;
                for y in y0..y1 {
                    let row = &src[(y * w + x0) * n..(y * w + x1) * n];
                    sum += row
                        .iter()
                        .skip(c)
                        .step_by(n)
                        .map(|&v| u32::from(v))
                        .sum::<u32>();
                }
                out.push(((sum + area / 2) / area) as u8);
            }
        }
    }
    out
}

/// JPEG bytes of `px` at `quality` (1..=100); `None` when the size does not
/// fit JPEG's 65535-pixel limit or `n` is not 1 or 3.
pub(crate) fn encode(px: &[u8], (w, h): (u32, u32), n: usize, quality: u8) -> Option<Vec<u8>> {
    use jpeg_encoder::{ColorType, Encoder};
    let color = match n {
        1 => ColorType::Luma,
        3 => ColorType::Rgb,
        _ => return None,
    };
    let (w, h) = (u16::try_from(w).ok()?, u16::try_from(h).ok()?);
    let mut out = Vec::new();
    Encoder::new(&mut out, quality)
        .encode(px, w, h, color)
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrink_averages_each_source_block() {
        // 4x2 gray: left 2x2 block 0/100/50/50 -> 50, right block all 200.
        let src = [0, 100, 200, 200, 50, 50, 200, 200];
        assert_eq!(shrink(&src, (4, 2), 1, (2, 1)), [50, 200]);
        // RGB keeps channels apart.
        let rgb = [10, 20, 30, 30, 40, 50];
        assert_eq!(shrink(&rgb, (2, 1), 3, (1, 1)), [20, 30, 40]);
    }

    #[test]
    fn shrink_to_same_size_is_identity() {
        let src: Vec<u8> = (0..27).collect();
        assert_eq!(shrink(&src, (3, 3), 3, (3, 3)), src);
    }

    #[test]
    fn encode_writes_a_jpeg_and_rejects_bad_shapes() {
        let px = vec![128u8; 16 * 8 * 3];
        let j = encode(&px, (16, 8), 3, 50).unwrap();
        assert_eq!(&j[..2], [0xFF, 0xD8]);
        assert!(
            encode(&px[..128], (16, 8), 1, 50)
                .unwrap()
                .starts_with(&[0xFF, 0xD8])
        );
        assert_eq!(encode(&px, (16, 8), 4, 50), None);
        assert_eq!(encode(&[0; 3], (70_000, 1), 3, 50), None);
    }
}
