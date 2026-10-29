//! gen11 redaction probes (page 0 kept, page 1 redacted): groups pdfium
//! reads that no /Type says are groups (untyped, inherited from /Pages,
//! behind odd OCMDs, an indirect /Type) holding a private key to page 1's
//! content; objects that are both a group and something else; view and
//! base states and intents pdfium compares as bytes. Page 0 draws "Hello"
//! and a bar in layer `oc1` (off by default unless the arm says otherwise).
use super::scan::write_pdf;
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every gen11 probe; `listred` (/OCGs lists page 1's font), `fontpriv` and
/// `annotpriv` (a kept font / annotation with a private key to page 1's
/// content) are ours.
pub const GEN11: &str = "inhtl inhtyped tlpriv venest annotoc ocmdfoo memfoo typeref dualocg dualuntyped \
    dualfont utf16 vsstr vsnull basestr dualfield listred fontpriv annotpriv";

const FONT: &str = "/Font << /F1 21 0 R >>";
const TL: &str = "<< /Name (L) /Priv 8 0 R /P2 (TLSECRET) >>";
const SQ: &str =
    "/Subtype /Square /Rect [10 10 190 190] /C [0 0 0] /IC [0 0 0] /BS << /W 5 >> /F 4";

fn res(props: &str) -> String {
    format!("/Resources << {FONT} /Properties << {props} >> >>")
}

/// `<v>.pdf` in `dir` for gen11 probe `v` (each arm says what it holds).
pub fn probe11_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let mut cat = "/OCProperties << /OCGs [73 0 R] /D << /OFF [73 0 R] >> >>".to_string();
    let (mut node, mut p0x, mut p1x) = (
        String::new(),
        res("/oc1 73 0 R"),
        format!("/Resources << {FONT} >>"),
    );
    let mut p0c = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC";
    o.insert(
        21,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    );
    let ocg = |extra: &str| format!("<< /Type /OCG /Name (L) {extra} >>");
    match v {
        // An untyped (or typed) group only in /Resources inherited from /Pages.
        "inhtl" | "inhtyped" => {
            let g = if v == "inhtl" {
                TL.into()
            } else {
                ocg("/Priv 8 0 R /P2 (TLSECRET)")
            };
            o.insert(73, g);
            (node, p0x, p1x) = (res("/oc1 73 0 R"), String::new(), String::new());
        }
        // An untyped group used directly.
        "tlpriv" => drop(o.insert(73, TL.into())),
        // An untyped group only behind nested indirect /VE.
        "venest" => {
            o.insert(73, TL.into());
            o.insert(90, "<< /Type /OCMD /VE 91 0 R >>".into());
            o.insert(91, "[/And 92 0 R]".into());
            o.insert(92, "[/Or [/Not [/Not 73 0 R]]]".into());
            p0x = res("/oc1 90 0 R");
        }
        // An untyped group only on a kept annotation's /OC.
        "annotoc" => {
            o.insert(73, TL.into());
            o.insert(87, format!("<< /Type /Annot {SQ} /OC 73 0 R >>"));
            p0x = format!("/Resources << {FONT} >> /Annots [87 0 R]");
            p0c = "BT /F1 12 Tf 20 100 Td (Hello) Tj ET";
        }
        // An OCMD typed /Foo naming an untyped group.
        "ocmdfoo" => {
            o.insert(73, TL.into());
            o.insert(90, "<< /Type /Foo /OCGs 73 0 R >>".into());
            p0x = res("/oc1 90 0 R");
        }
        // An OCMD member typed /Foo.
        "memfoo" => {
            o.insert(
                73,
                "<< /Type /Foo /Name (L) /Priv 8 0 R /P2 (TLSECRET) >>".into(),
            );
            o.insert(90, "<< /Type /OCMD /OCGs 73 0 R >>".into());
            p0x = res("/oc1 90 0 R");
        }
        // /Type an indirect /OCG.
        "typeref" => {
            o.insert(
                73,
                "<< /Type 95 0 R /Name (L) /Priv 8 0 R /P2 (TLSECRET) >>".into(),
            );
            o.insert(95, "/OCG".into());
        }
        // A kept annotation typed /OCG, listed in /OCGs only.
        "dualocg" => {
            o.insert(73, ocg(""));
            o.insert(87, format!("<< /Type /OCG {SQ} >>"));
            cat = "/OCProperties << /OCGs [73 0 R 87 0 R] /D << /OFF [73 0 R] >> >>".into();
            p0x += " /Annots [87 0 R]";
        }
        // An untyped annotation also used as optional content.
        "dualuntyped" => {
            o.insert(87, format!("<< {SQ} >>"));
            cat = "/OCProperties << /OCGs [87 0 R] /D << /ON [87 0 R] >> >>".into();
            p0x = res("/oc1 87 0 R") + " /Annots [87 0 R]";
        }
        // Page 0's font typed /OCG and listed in /OCGs.
        "dualfont" => {
            o.insert(
                21,
                "<< /Type /OCG /Subtype /Type1 /BaseFont /Courier /Name (L) >>".into(),
            );
            cat = "/OCProperties << /OCGs [21 0 R] >>".into();
            p0c = "BT /F1 40 Tf 5 100 Td (Hello) Tj ET";
            p0x = format!("/Resources << {FONT} >>");
        }
        // /Intent UTF-16 "View": pdfium compares bytes, so no View intent.
        "utf16" => drop(o.insert(73, ocg("/Intent <FEFF0056006900650077>"))),
        // /ViewState the string (OFF): pdfium hides.
        "vsstr" => {
            o.insert(73, ocg("/Usage << /View << /ViewState (OFF) >> >>"));
            cat = "/OCProperties << /OCGs [73 0 R] /D << >> >>".into();
        }
        // /ViewState a dangling reference: pdfium shows.
        "vsnull" => drop(o.insert(73, ocg("/Usage << /View << /ViewState 99 0 R >> >>"))),
        // /BaseState the string (OFF): pdfium hides what is not listed.
        "basestr" => {
            o.insert(73, ocg(""));
            cat = "/OCProperties << /OCGs [73 0 R] /D << /BaseState (OFF) >> >>".into();
        }
        // A group that is also a field with a widget on each page.
        "dualfield" => {
            o.insert(
                73,
                "<< /Type /OCG /Name (L) /FT /Tx /T (f) /V (FIELDVAL) /Kids [61 0 R 62 0 R] >>"
                    .into(),
            );
            let w = |p: u32| {
                format!(
                    "<< /Type /Annot /Subtype /Widget /Parent 73 0 R /Rect [0 0 100 30] /P {p} 0 R >>"
                )
            };
            o.insert(61, w(3));
            o.insert(62, w(4));
            cat = "/OCProperties << /OCGs [73 0 R] >> /AcroForm << /Fields [73 0 R] >>".into();
            p0x += " /Annots [61 0 R]";
            p1x += " /Annots [62 0 R]";
        }
        // /OCGs lists page 1's own font: no group of it.
        "listred" => {
            o.insert(73, ocg(""));
            o.insert(
                22,
                "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".into(),
            );
            cat = "/OCProperties << /OCGs [73 0 R 22 0 R] /D << /OFF [73 0 R] >> >>".into();
            p1x = "/Resources << /Font << /F1 22 0 R >> >>".into();
        }
        "fontpriv" => drop(o.insert(73, ocg(""))),
        "annotpriv" => {
            o.insert(73, ocg(""));
            o.insert(87, format!("<< /Type /Annot {SQ} /Priv 8 0 R >>"));
            p0x += " /Annots [87 0 R]";
        }
        _ => panic!("unknown probe {v}"),
    }
    if v == "fontpriv" {
        let f = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Priv 8 0 R >>";
        o.insert(21, f.into());
    }
    let st = |data: &str| String::from_utf8(stream("", data)).unwrap();
    o.insert(1, format!("<< /Type /Catalog /Pages 2 0 R {cat} >>"));
    o.insert(
        2,
        format!("<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200] {node} >>"),
    );
    o.insert(
        3,
        format!("<< /Type /Page /Parent 2 0 R {p0x} /Contents 5 0 R >>"),
    );
    o.insert(
        4,
        format!("<< /Type /Page /Parent 2 0 R {p1x} /Contents 8 0 R >>"),
    );
    o.insert(5, st(p0c));
    o.insert(8, st("BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET"));
    let n = *o.keys().last().unwrap();
    let objs: Vec<Vec<u8>> = (1..=n)
        .map(|i| o.remove(&i).unwrap_or_else(|| "null".into()).into_bytes())
        .collect();
    write_pdf(dir, &format!("{v}.pdf"), &objs)
}
