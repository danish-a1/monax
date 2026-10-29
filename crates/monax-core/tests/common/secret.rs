//! A generated PDF whose page 1 hides "XYZZY" everywhere a redaction must
//! reach (text, a page-only font, widget value and appearance, link, note,
//! /Thumb, a bookmark), qpdf readers to prove it is gone, and pixel checks
//! of where the black boxes landed.
use super::docs::{objects, qpdf};
use super::scan::write_pdf;
use monax_core::pages::Rect;
use monax_core::pdfium;
use monax_core::security::Redaction;
use std::path::{Path, PathBuf};

/// A redaction of `page` with the rect `l, b, r, t` (user space points).
pub fn rd(page: usize, l: i16, b: i16, r: i16, t: i16) -> Redaction {
    let [left, bottom, right, top] = [l, b, r, t].map(f32::from);
    let rect = Rect {
        left,
        bottom,
        right,
        top,
    };
    Redaction { page, rect }
}

pub fn stream(dict: &str, data: &str) -> Vec<u8> {
    format!(
        "<< {dict} /Length {} >>\nstream\n{data}\nendstream",
        data.len()
    )
    .into_bytes()
}

fn page(font: &str, contents: u32, extra: &str) -> Vec<u8> {
    format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
         /Resources << /Font << /F1 {font} 0 R >> >> /Contents {contents} 0 R {extra} >>"
    )
    .into_bytes()
}

/// `secret.pdf` in `dir`: three 200 x 200 pt pages showing "KeepAlpha",
/// "SECRETXYZZY" (Courier, used by no other page) and "PLUGHPLUGH".
pub fn secret_pdf(dir: &Path) -> PathBuf {
    let b = |s: &str| s.as_bytes().to_vec();
    let objs = vec![
        b("<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [10 0 R] >> /Outlines 14 0 R >>"),
        b("<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>"),
        page("6", 7, ""),
        page("8", 9, "/Annots [10 0 R 11 0 R 12 0 R] /Thumb 13 0 R"),
        page("6", 16, ""),
        b("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        stream("", "BT /F1 18 Tf 20 100 Td (KeepAlpha) Tj ET"),
        b("<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>"),
        stream("", "BT /F1 14 Tf 20 100 Td (SECRETXYZZY) Tj ET"),
        b(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (fieldxyzzy) /V (FIELDXYZZY) \
           /Rect [20 150 180 170] /P 4 0 R /F 4 /AP << /N 15 0 R >> >>",
        ),
        b("<< /Type /Annot /Subtype /Link /Rect [20 40 180 60] \
           /A << /S /URI /URI (https://linkxyzzy.invalid) >> >>"),
        b("<< /Type /Annot /Subtype /Text /Rect [150 20 170 40] /Contents (NOTEXYZZY) >>"),
        stream(
            "/Width 10 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            "THUMBXYZZY",
        ),
        b("<< /Type /Outlines /First 17 0 R /Last 17 0 R /Count 1 >>"),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 160 20] /Resources << /Font << /F1 6 0 R >> >>",
            "BT /F1 12 Tf 2 5 Td (FIELDXYZZY) Tj ET",
        ),
        stream("", "BT /F1 18 Tf 20 100 Td (PLUGHPLUGH) Tj ET"),
        b("<< /Title (Chapter) /Parent 14 0 R /Dest [4 0 R /Fit] >>"),
    ];
    write_pdf(dir, "secret.pdf", &objs)
}

/// `p` rewritten by `qpdf --qdf --object-streams=disable
/// --preserve-unreferenced`: every object in the file, referenced or not,
/// with its stream decoded.
pub fn qdf(p: &Path) -> String {
    let args = [
        "--qdf",
        "--object-streams=disable",
        "--preserve-unreferenced",
    ];
    qpdf(&[&args[..], &[p.to_str().unwrap(), "-"]].concat())
}

/// Decoded content stream of every page, in page order.
pub fn page_contents(p: &Path) -> Vec<String> {
    let s = p.to_str().unwrap();
    let objs = objects(p);
    let pages: serde_json::Value =
        serde_json::from_str(&qpdf(&["--json=2", "--json-key=pages", s])).unwrap();
    let pages = pages["pages"].as_array().unwrap();
    pages
        .iter()
        .map(|pg| {
            let obj = &objs[&format!("obj:{}", pg["object"].as_str().unwrap())];
            let c = obj["value"]["/Contents"].as_str().unwrap();
            let n = c.split(' ').next().unwrap();
            let show = format!("--show-object={n}");
            qpdf(&[&show, "--filtered-stream-data", s])
        })
        .collect()
}

/// `(x, y)` (72 dpi pixels, top-left origin) is near black.
pub fn dark(r: &pdfium::Raster, x: u32, y: u32) -> bool {
    let i = ((y * r.w + x) * 4) as usize;
    r.rgba[i..i + 3].iter().all(|&c| c < 64)
}

/// Asserts that the near-black pixels of `out`'s page `i` at 72 dpi over
/// white in `src` span `want` `(x0, y0, x1, y1)` (inclusive, top-left
/// origin) within one pixel; only pixels left of x 100 or below y 100 count
/// when `want` sits there, so other boxes and text do not interfere.
pub fn assert_box(src: &Path, out: &Path, i: usize, want: (u32, u32, u32, u32)) {
    let (a, b) = (
        pdfium::render(src, i, 72).unwrap(),
        pdfium::render(out, i, 72).unwrap(),
    );
    assert_eq!((a.w, a.h), (b.w, b.h), "displayed size kept");
    let mut got: Option<(u32, u32, u32, u32)> = None;
    for y in 0..b.h {
        for x in 0..b.w {
            let k = ((y * b.w + x) * 4) as usize;
            let white = a.rgba[k..k + 3].iter().all(|&c| c > 200);
            let near = x + 4 >= want.0 && x <= want.2 + 4 && y + 4 >= want.1 && y <= want.3 + 4;
            if white && near && dark(&b, x, y) {
                got = Some(got.map_or((x, y, x, y), |(x0, y0, x1, y1)| {
                    (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
                }));
            }
        }
    }
    let (g, w) = (got.expect("no black box"), want);
    let close = |p: u32, q: u32| p.abs_diff(q) <= 1;
    assert!(
        close(g.0, w.0) && close(g.1, w.1) && close(g.2, w.2) && close(g.3, w.3),
        "{g:?} != {w:?}"
    );
    // Everything inside the box is black, glyphs it covers included.
    let inside = (w.1 + 1..w.3).all(|y| (w.0 + 1..w.2).all(|x| dark(&b, x, y)));
    assert!(inside, "box {w:?} not solid black");
}
