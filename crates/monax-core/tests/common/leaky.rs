//! Generated PDFs whose page 1 hides `*SECRET` strings in every shape a
//! redaction must reach: page text, a Form XObject, an image and a font
//! listed in resources page 0 can also see (shared, inherited, per-page
//! direct, or through a form page 0 draws), struct tree /Alt /ActualText
//! and an OBJR link, a note reached by /IRT, /Popup and /Parent from page 0,
//! a form field. Bookmarks, a named destination and /OpenAction point at it.
use super::scan::write_pdf;
use super::secret::{qdf, stream};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How page 0 can see page 1's resources.
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    /// Both pages name one indirect resource dictionary.
    Shared,
    /// The resource dictionary sits on /Pages.
    Inherited,
    /// Each page has its own direct copy listing the same objects.
    Direct,
    /// Page 0 draws a form whose resources are page 1's dictionary.
    Form,
}

pub const SHAPES: [Shape; 4] = [Shape::Shared, Shape::Inherited, Shape::Direct, Shape::Form];

/// Secrets only page 1 holds: none may survive its redaction.
pub const GONE: [&str; 10] = [
    "PAGESECRET",
    "FORMSECRET",
    "IMAGESECRET",
    "FONTSECRET",
    "ALTSECRET",
    "ACTSECRET",
    "URISECRET",
    "NOTESECRET",
    "POPUPSECRET",
    "WIDGETSECRET",
];

/// Document-level text naming page 1: dropped with its features, and noted.
pub const KEPT: [&str; 2] = ["OUTLINESECRET", "NAMEDSECRET"];

const RES: &str = "<< /Font << /F1 21 0 R /F2 23 0 R >> /XObject << /Fx 22 0 R /Im 24 0 R >> >>";

/// `leaky.pdf` in `dir`: two 200 x 200 pt pages; page 0 shows "Hello".
pub fn leaky_pdf(dir: &Path, shape: Shape) -> PathBuf {
    let (pages_res, res0, res1) = match shape {
        Shape::Shared => (
            String::new(),
            "/Resources 20 0 R".into(),
            "/Resources 20 0 R".into(),
        ),
        Shape::Inherited => (" /Resources 20 0 R".into(), String::new(), String::new()),
        Shape::Direct => (
            String::new(),
            format!("/Resources {RES}"),
            format!("/Resources {RES}"),
        ),
        Shape::Form => (
            String::new(),
            "/Resources << /XObject << /Wrap 25 0 R >> >>".into(),
            "/Resources 20 0 R".into(),
        ),
    };
    let hello = "BT /F1 12 Tf 20 100 Td (Hello) Tj ET";
    let page0 = if matches!(shape, Shape::Form) {
        "/Wrap Do"
    } else {
        hello
    };
    let s = |v: &str| v.as_bytes().to_vec();
    let mut o: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    o.insert(
        1,
        s(
            "<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 30 0 R /MarkInfo << /Marked true >> \
        /OpenAction [4 0 R /Fit] /Outlines 40 0 R /Names << /Dests 42 0 R >> \
        /AcroForm << /Fields [50 0 R 51 0 R] /CO [50 0 R] >> >>",
        ),
    );
    o.insert(
        2,
        format!(
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200]{pages_res} >>"
        )
        .into_bytes(),
    );
    o.insert(
        3,
        format!(
            "<< /Type /Page /Parent 2 0 R {res0} /Contents 5 0 R \
        /Annots [6 0 R 7 0 R 51 0 R 54 0 R] /StructParents 0 >>"
        )
        .into_bytes(),
    );
    o.insert(
        4,
        format!(
            "<< /Type /Page /Parent 2 0 R {res1} /Contents 8 0 R \
        /Annots [31 0 R 50 0 R 52 0 R 53 0 R] /StructParents 1 >>"
        )
        .into_bytes(),
    );
    o.insert(5, stream("", page0));
    o.insert(
        6,
        s("<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] /Dest [4 0 R /Fit] >>"),
    );
    o.insert(7, s("<< /Type /Annot /Subtype /Text /Rect [20 20 30 30] /Contents (reply) /IRT 52 0 R /Popup 53 0 R >>"));
    o.insert(
        8,
        stream(
            "",
            "/Fx Do q 100 0 0 10 0 0 cm /Im Do Q BT /F2 12 Tf 20 150 Td (PAGESECRET) Tj ET",
        ),
    );
    o.insert(20, s(RES));
    o.insert(
        21,
        s("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    );
    o.insert(22, stream("/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 21 0 R >> >>",
        "BT /F1 12 Tf 20 50 Td (FORMSECRET) Tj ET"));
    o.insert(
        23,
        s("<< /Type /Font /Subtype /Type1 /BaseFont /FONTSECRET >>"),
    );
    o.insert(24, stream("/Type /XObject /Subtype /Image /Width 11 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8", "IMAGESECRET"));
    o.insert(
        25,
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources 20 0 R",
            hello,
        ),
    );
    o.insert(30, s("<< /Type /StructTreeRoot /K [32 0 R] /ParentTree << /Nums [1 [32 0 R] 2 32 0 R] >> /ParentTreeNextKey 3 >>"));
    o.insert(31, s("<< /Type /Annot /Subtype /Link /Rect [0 0 50 50] /StructParent 2 /A << /S /URI /URI (http://URISECRET) >> >>"));
    o.insert(32, s("<< /Type /StructElem /S /P /P 30 0 R /Pg 4 0 R /Alt (ALTSECRET) /ActualText (ACTSECRET) \
        /K [0 << /Type /OBJR /Obj 31 0 R >>] >>"));
    o.insert(
        40,
        s("<< /Type /Outlines /First 41 0 R /Last 41 0 R /Count 1 >>"),
    );
    o.insert(
        41,
        s("<< /Title (OUTLINESECRET) /Parent 40 0 R /Dest [4 0 R /Fit] >>"),
    );
    o.insert(42, s("<< /Names [(NAMEDSECRET) [4 0 R /Fit]] >>"));
    o.insert(50, s("<< /Type /Annot /Subtype /Widget /FT /Tx /T (f1) /V (WIDGETSECRET) /Rect [20 170 180 190] /P 4 0 R >>"));
    o.insert(51, s("<< /Type /Annot /Subtype /Widget /FT /Tx /T (f2) /V (keep) /Rect [20 170 180 190] /P 3 0 R >>"));
    o.insert(
        52,
        s("<< /Type /Annot /Subtype /Text /Rect [40 40 50 50] /Contents (NOTESECRET) /P 4 0 R >>"),
    );
    o.insert(53, s("<< /Type /Annot /Subtype /Popup /Rect [60 60 90 90] /Contents (POPUPSECRET) /P 4 0 R >>"));
    o.insert(
        54,
        s("<< /Type /Annot /Subtype /Popup /Rect [60 60 90 90] /Parent 52 0 R >>"),
    );
    let n = *o.keys().last().unwrap();
    let objs: Vec<Vec<u8>> = (1..=n)
        .map(|i| o.remove(&i).unwrap_or_else(|| s("null")))
        .collect();
    write_pdf(dir, "leaky.pdf", &objs)
}

/// Those of `secrets` found in `p`'s raw bytes or in any object of its
/// `--preserve-unreferenced` qdf rewrite (streams decoded).
pub fn survivors(p: &Path, secrets: &[&str]) -> Vec<String> {
    let raw = String::from_utf8_lossy(&std::fs::read(p).unwrap()).into_owned();
    let all = qdf(p);
    let hit = |s: &&&str| raw.contains(**s) || all.contains(**s);
    secrets.iter().filter(hit).map(|s| s.to_string()).collect()
}

/// Panics if any of `secrets` survives in `p` (see [`survivors`]).
pub fn assert_absent(p: &Path, secrets: &[&str]) {
    let found = survivors(p, secrets);
    assert!(found.is_empty(), "{}: survived {found:?}", p.display());
}
