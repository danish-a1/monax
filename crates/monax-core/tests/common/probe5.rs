//! gen5 redaction probes on the [`super::probe`] skeleton (page 0 kept,
//! page 1 redacted): document-level roots that name page 1's objects,
//! private dictionaries with /P and /Parent keys, Type3 font cycles, deep
//! form chains, a radio field whose first /Opt entry goes, and a kept
//! appearance drawing a form.
use super::probe::{Two, assemble};
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FONT1: &str = "/Resources << /Font << /F1 21 0 R >> >>";
const DRAWS_X: &str = "/Resources << /Font << /F1 21 0 R >> /XObject << /X 66 0 R >> >>";
const T3: &str = "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1 1] /FontMatrix [0.001 0 0 0.001 0 0] \
    /CharProcs << /a 91 0 R >> /Encoding << /Differences [97 /a] >> /FirstChar 97 /LastChar 97 /Widths [1000]";

/// `<v>.pdf` in `dir` for probe `v` (see the gen5 generator; each arm names
/// what it hides on page 1). `deep` takes
/// a chain of `n` nested forms.
pub fn probe5_pdf(dir: &Path, v: &str, n: u32) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    let form = |txt: &str| {
        let d = format!("/Type /XObject /Subtype /Form /BBox [0 0 200 200] {FONT1}");
        st(&d, &format!("BT /F1 12 Tf 20 50 Td ({txt}) Tj ET"))
    };
    let piece = st("", "BT (PIECESECRET) Tj ET");
    let p1c;
    let mut t = Two {
        cat: "",
        p0x: "",
        p1x: "",
        p0c: "BT /F1 12 Tf 20 100 Td (Hello) Tj ET",
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    let ocg = |o: &mut BTreeMap<u32, String>, x: &str| {
        o.insert(70, format!("<< /Type /OCG /Name (L) /X {x} 0 R >>"));
    };
    match v {
        // Page 1's content named by a layer, by /AcroForm /DR (with and
        // without a kept field), and its form by /DR.
        "ocpcontent" => {
            t.cat = "/OCProperties << /OCGs [70 0 R] /D << /Order [70 0 R] >> >>";
            ocg(&mut o, "8");
        }
        "drcontent" => t.cat = "/AcroForm << /Fields [] /DR << /XObject << /Z 8 0 R >> >> >>",
        "drcontent2" => {
            t.cat = "/AcroForm << /Fields [71 0 R] /DR << /Font << /F1 21 0 R >> \
                /XObject << /Z 8 0 R >> >> /DA (/F1 0 Tf 0 g) >>";
            t.p0x = "/Annots [71 0 R]";
            o.insert(71, "<< /Type /Annot /Subtype /Widget /FT /Tx /T (t) /V (hi) /Rect [0 0 50 20] /P 3 0 R >>".into());
        }
        "drform" => {
            t.cat = "/AcroForm << /Fields [] /DR << /XObject << /Z 66 0 R >> >> >>";
            o.insert(66, form("FORMSECRET"));
            p1c = format!("{} /X Do", t.p1c);
            (t.p1x, t.p1c) = (DRAWS_X, &p1c);
        }
        // Page 1's private /PieceInfo stream behind /P, /Parent, a layer.
        "pkey" | "parentkey" | "ocppiece" => {
            t.p1x = match v {
                "pkey" => {
                    "/PieceInfo << /App << /LastModified (D:2020) /Private << /P 60 0 R >> >> >>"
                }
                "parentkey" => {
                    "/PieceInfo << /App << /LastModified (D:2020) /Private << /Parent 60 0 R >> >> >>"
                }
                _ => "/PieceInfo << /App << /LastModified (D:2020) /Private 60 0 R >> >>",
            };
            if v == "ocppiece" {
                t.cat = "/OCProperties << /OCGs [70 0 R] /D << /Order [70 0 R] >> >>";
                ocg(&mut o, "60");
            } else {
                t.cat = "/Foo 60 0 R";
            }
            o.insert(60, piece);
        }
        // Type3 fonts on page 0 naming themselves, or each other.
        "t3cycle" | "t3pair" | "t3used" => {
            t.p0x = "/Resources << /Font << /F1 21 0 R /T3 90 0 R >> >>";
            let other = if v == "t3pair" { 92 } else { 90 };
            o.insert(
                90,
                format!("{T3} /Resources << /Font << /T {other} 0 R >> >> >>"),
            );
            if v == "t3pair" {
                o.insert(
                    92,
                    format!("{T3} /Resources << /Font << /T 90 0 R >> >> >>"),
                );
            }
            if v == "t3used" {
                t.p0c = "BT /T3 12 Tf 20 20 Td (a) Tj ET";
            }
            o.insert(91, st("", "1000 0 d0 0 0 1000 1000 re f"));
        }
        // Page 0 lists, but never draws, page 1's form / image.
        "unusedres" | "unusedimg" => {
            let (x, draw) = if v == "unusedres" {
                (form("FORMSECRET"), " /X Do")
            } else {
                let d = "/Type /XObject /Subtype /Image /Width 11 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8";
                (st(d, "IMAGESECRET"), " q 11 0 0 1 0 0 cm /X Do Q")
            };
            o.insert(66, x);
            p1c = format!("{}{draw}", t.p1c);
            (t.p0x, t.p1x, t.p1c) = (DRAWS_X, DRAWS_X, &p1c);
        }
        // A radio field whose first kid (and /Opt entry) is on page 1.
        "optfirst" => {
            (t.cat, t.p0x, t.p1x) = (
                "/AcroForm << /Fields [70 0 R] >>",
                "/Annots [50 0 R]",
                "/Annots [51 0 R]",
            );
            o.insert(70, "<< /FT /Btn /Ff 49152 /T (r) /V /1 /Opt [(OPTSECRET) (Healthy)] /Kids [51 0 R 50 0 R] >>".into());
            let w = "/Type /Annot /Subtype /Widget /Parent 70 0 R /Rect [20 170 40 190]";
            o.insert(
                50,
                format!("<< {w} /P 3 0 R /AS /1 /AP << /N << /1 54 0 R /Off 54 0 R >> >> >>"),
            );
            o.insert(
                51,
                format!("<< {w} /P 4 0 R /AS /Off /AP << /N << /0 54 0 R /Off 54 0 R >> >> >>"),
            );
            o.insert(
                54,
                st(
                    "/Type /XObject /Subtype /Form /BBox [0 0 20 20]",
                    "0 0 20 20 re f",
                ),
            );
        }
        // A kept popup whose markup is on page 1 and points back.
        "popupback" => {
            (t.p0x, t.p1x) = ("/Annots [80 0 R]", "/Annots [81 0 R]");
            o.insert(
                80,
                "<< /Type /Annot /Subtype /Popup /Rect [0 0 50 50] /Parent 81 0 R /P 3 0 R >>"
                    .into(),
            );
            o.insert(81, "<< /Type /Annot /Subtype /Text /Rect [0 0 10 10] /Contents (NOTESECRET) /Popup 80 0 R /P 4 0 R >>".into());
        }
        // A kept stamp whose appearance draws a form saying page 1's word.
        "frm" => {
            t.p0x = "/Annots [67 0 R]";
            o.insert(
                67,
                "<< /Type /Annot /Subtype /Stamp /Rect [0 0 200 200] /AP << /N 68 0 R >> >>".into(),
            );
            let d = "/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /XObject << /FRM 69 0 R >> >>";
            o.insert(68, st(d, "/FRM Do"));
            o.insert(69, form("PAGESECRET"));
        }
        // Page 0 draws a chain of `n` nested forms.
        "deep" => {
            t.p0x = "/Resources << /Font << /F1 21 0 R >> /XObject << /X 100 0 R >> >>";
            for i in 0..n {
                let next = match i + 1 < n {
                    true => format!("/Resources << /XObject << /X {} 0 R >> >>", 101 + i),
                    false => String::new(),
                };
                o.insert(
                    100 + i,
                    st(
                        &format!("/Type /XObject /Subtype /Form /BBox [0 0 1 1] {next}"),
                        "",
                    ),
                );
            }
        }
        _ => panic!("unknown probe {v}"),
    }
    assemble(dir, v, o, &t)
}
