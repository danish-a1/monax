//! Generated two-page PDFs, one shape each, for redaction leak and
//! false-alarm probes: page 0 shows "Hello" (kept), page 1 is redacted.
use super::scan::write_pdf;
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CMAP: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
    /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def \
    /CMapName /Adobe-Identity-UCS def /CMapType 2 def 1 begincodespacerange <00> <FF> \
    endcodespacerange 1 beginbfrange <20> <7E> <0020> endbfrange endcmap CMapName \
    currentdict /CMapType defineresource pop end end";

/// `<v>.pdf` in `dir` for probe `v`: `hide` (a kept link's hide action
/// names page 1's note), `catannot` / `catcontent` (catalog /PieceInfo
/// names page 1's note / content), `twowidget` / `radio` / `nested` (a
/// field with widgets on page 1), `ocg` (page 1 in a named layer),
/// `tounicode` (page 1 says the words of a ToUnicode CMap), `kern` (page 1
/// says "port", page 0 kerns "Re" "port").
pub fn probe_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let (mut cat, mut p0x, mut p1x) = ("", "", "");
    let mut p0c = "BT /F1 12 Tf 20 100 Td (Hello) Tj ET";
    let mut p1c = "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET";
    let mut ins = |k: u32, v: &str| drop(o.insert(k, v.to_string()));
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    ins(21, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    let note =
        "<< /Type /Annot /Subtype /Text /Rect [40 40 50 50] /Contents (NOTESECRET) /P 4 0 R >>";
    match v {
        "tounicode" => {
            ins(
                21,
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 22 0 R >>",
            );
            ins(22, &st("", CMAP));
            p1c = "BT /F1 12 Tf 20 150 Td (Made with Adobe Identity) Tj ET";
        }
        "kern" => {
            p0c = "BT /F1 12 Tf 20 100 Td [(Re) -10 (port)] TJ ET";
            p1c = "BT /F1 12 Tf 20 150 Td (the port) Tj ET";
        }
        "hide" => {
            (p0x, p1x) = ("/Annots [6 0 R]", "/Annots [52 0 R]");
            ins(
                6,
                "<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] /A << /S /Hide /T 52 0 R >> >>",
            );
            ins(52, note);
        }
        "twowidget" => {
            (cat, p0x, p1x) = (
                "/AcroForm << /Fields [60 0 R] >>",
                "/Annots [50 0 R]",
                "/Annots [51 0 R]",
            );
            ins(60, "<< /FT /Tx /T (f) /V (shown) /Kids [50 0 R 51 0 R] >>");
            ins(
                50,
                "<< /Type /Annot /Subtype /Widget /Parent 60 0 R /Rect [20 170 180 190] /P 3 0 R >>",
            );
            ins(
                51,
                "<< /Type /Annot /Subtype /Widget /Parent 60 0 R /Rect [20 170 180 190] /P 4 0 R \
                /MK << /CA (CAPSECRET) >> /AP << /N 53 0 R >> >>",
            );
            ins(
                53,
                &st(
                    "/Type /XObject /Subtype /Form /BBox [0 0 160 20] /Resources << /Font << /F1 21 0 R >> >>",
                    "BT /F1 12 Tf 2 5 Td (APSECRET) Tj ET",
                ),
            );
        }
        "radio" => {
            (cat, p0x, p1x) = (
                "/AcroForm << /Fields [60 0 R] >>",
                "/Annots [50 0 R]",
                "/Annots [51 0 R]",
            );
            ins(
                60,
                "<< /FT /Btn /Ff 49152 /T (dx) /V /RADIOSECRET /Kids [50 0 R 51 0 R] >>",
            );
            ins(
                50,
                "<< /Type /Annot /Subtype /Widget /Parent 60 0 R /Rect [20 170 40 190] /P 3 0 R \
                /AS /Off /AP << /N << /Healthy 54 0 R /Off 54 0 R >> >> >>",
            );
            ins(
                51,
                "<< /Type /Annot /Subtype /Widget /Parent 60 0 R /Rect [20 170 40 190] /P 4 0 R \
                /AS /RADIOSECRET /AP << /N << /RADIOSECRET 54 0 R /Off 54 0 R >> >> >>",
            );
            ins(
                54,
                &st(
                    "/Type /XObject /Subtype /Form /BBox [0 0 20 20]",
                    "0 0 20 20 re f",
                ),
            );
        }
        "nested" => {
            (cat, p1x) = (
                "/AcroForm << /Fields [60 0 R] /CO [60 0 R] >>",
                "/Annots [50 0 R]",
            );
            ins(
                60,
                "<< /FT /Tx /T (parent) /V (NESTSECRET) /Kids [61 0 R] >>",
            );
            ins(61, "<< /T (child) /Kids [50 0 R] /Parent 60 0 R >>");
            ins(
                50,
                "<< /Type /Annot /Subtype /Widget /Parent 61 0 R /Rect [20 170 180 190] /P 4 0 R >>",
            );
        }
        "catcontent" => cat = "/PieceInfo << /X << /Private 8 0 R >> >>",
        "catannot" => {
            (cat, p1x) = (
                "/PieceInfo << /X << /Private 52 0 R >> >>",
                "/Annots [52 0 R]",
            );
            ins(52, note);
        }
        "ocg" => {
            cat = "/OCProperties << /OCGs [70 0 R] /D << /Order [70 0 R] >> >>";
            ins(70, "<< /Type /OCG /Name (OCGSECRET) >>");
            p1x = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 70 0 R >> >>";
            p1c = "/OC /oc1 BDC BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET EMC";
        }
        _ => panic!("unknown probe {v}"),
    }
    let two = Two {
        cat,
        p0x,
        p1x,
        p0c,
        p1c,
    };
    assemble(dir, v, o, &two)
}

/// What the probes vary on the two-page skeleton.
pub struct Two<'a> {
    pub cat: &'a str,
    pub p0x: &'a str,
    pub p1x: &'a str,
    pub p0c: &'a str,
    pub p1c: &'a str,
}

/// `<name>.pdf` in `dir`: objects `o` plus catalog 1 (with `cat`), pages 2,
/// page 0 (obj 3, extra keys `p0x`, content 5 `p0c`) and page 1 (obj 4,
/// `p1x`, content 8 `p1c` unless `o` has an 8). Pages without /Resources
/// get Helvetica as /F1 (obj 21).
pub fn assemble(dir: &Path, name: &str, mut o: BTreeMap<u32, String>, t: &Two) -> PathBuf {
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    let rs = |x: &str| match x.contains("/Resources") {
        true => x.to_string(),
        false => format!("/Resources << /Font << /F1 21 0 R >> >> {x}"),
    };
    o.entry(21)
        .or_insert_with(|| "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    o.insert(1, format!("<< /Type /Catalog /Pages 2 0 R {} >>", t.cat));
    o.insert(
        2,
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200] >>".into(),
    );
    o.insert(
        3,
        format!(
            "<< /Type /Page /Parent 2 0 R {} /Contents 5 0 R >>",
            rs(t.p0x)
        ),
    );
    o.insert(
        4,
        format!(
            "<< /Type /Page /Parent 2 0 R {} /Contents 8 0 R >>",
            rs(t.p1x)
        ),
    );
    o.insert(5, st("", t.p0c));
    o.entry(8).or_insert_with(|| st("", t.p1c));
    let n = *o.keys().last().unwrap();
    let objs: Vec<Vec<u8>> = (1..=n)
        .map(|i| o.remove(&i).unwrap_or_else(|| "null".into()).into_bytes())
        .collect();
    write_pdf(dir, &format!("{name}.pdf"), &objs)
}
