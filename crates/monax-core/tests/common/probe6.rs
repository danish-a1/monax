//! gen6 redaction probes on the [`super::probe`] skeleton (page 0 kept,
//! page 1 redacted): containers and objects kept pages need that also name
//! page 1, form fields and /DR entries naming page 1 or its objects, kept
//! links to it, and a layer that is off on page 0.
use super::probe::{Two, assemble};
use super::scan::write_pdf;
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A text widget `t<n>` on page object `p` holding `val`.
fn widget(n: u32, p: u32, val: &str) -> String {
    format!(
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (t{n}) /V ({val}) /Rect [0 0 50 20] /P {p} 0 R >>"
    )
}

/// `<v>.pdf` in `dir` for gen6 probe `v` (each arm says what it holds).
pub fn probe6_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    let mut t = Two {
        cat: "",
        p0x: "",
        p1x: "",
        p0c: "BT /F1 12 Tf 20 100 Td (Hello) Tj ET",
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    let link = "<< /Type /Annot /Subtype /Link /Rect [0 0 10 10]";
    let piece = "/PieceInfo << /App << /LastModified (D:2020) /Private (PIECESECRET) >> >>";
    let (p0c, p1x);
    match v {
        // The page tree's /Kids is an indirect array.
        "kidsind" => return kidsind(dir),
        // Page 0's /Annots is an indirect array: a filled widget and a link
        // to page 1.
        "annotsind" => {
            t.cat = "/AcroForm << /Fields [71 0 R] /DA (/F1 0 Tf 0 g) /DR << /Font << /F1 21 0 R >> >> >>";
            t.p0x = "/Annots 60 0 R";
            o.insert(60, format!("[71 0 R {link} /Dest [4 0 R /Fit] >>]"));
            o.insert(71, widget(1, 3, "KEPTVALUE"));
        }
        // /Fields, /CO and /DR name page 1's content stream.
        "fieldsred" => {
            t.cat = "/AcroForm << /Fields [71 0 R 8 0 R] /CO [8 0 R] /NeedAppearances true \
                /DA (/F1 0 Tf 0 g) /DR << /Font << /F1 21 0 R >> /XObject << /Z 8 0 R >> >> >>";
            t.p0x = "/Annots [71 0 R]";
            o.insert(71, widget(1, 3, "KEPTVALUE"));
        }
        // The form's /DR font is page 1's own; the kept widget has no /AP.
        "drfont" => {
            t.cat = "/AcroForm << /Fields [71 0 R] /DA (/F2 0 Tf 0 g) /DR << /Font << /F2 22 0 R >> >> >>";
            (t.p0x, t.p1x) = (
                "/Annots [71 0 R]",
                "/Resources << /Font << /F1 22 0 R >> >>",
            );
            o.insert(71, widget(1, 3, "KEPTVALUE"));
            o.insert(
                22,
                "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".into(),
            );
        }
        // A kept link to page 1 (/Dest and /P); page 1 holds private data.
        "redlink" => {
            t.p0x = "/Annots [72 0 R]";
            o.insert(72, format!("{link} /Dest [4 0 R /Fit] /P 4 0 R >>"));
            p1x = format!("{piece} /Rotate 0");
            t.p1x = &p1x;
        }
        // A kept link's indirect GoTo to page 1 chains JavaScript.
        "actnext" => {
            t.p0x = "/Annots [72 0 R]";
            o.insert(72, format!("{link} /A 77 0 R >>"));
            o.insert(77, "<< /S /GoTo /D [4 0 R /Fit] /Next 78 0 R >>".into());
            o.insert(
                78,
                "<< /S /JavaScript /JS (app.alert\\('ACTSECRET'\\)) >>".into(),
            );
        }
        // Page 0 draws a form whose dictionary names page 1.
        "xodesc" => {
            let d = "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
                /Resources << /Font << /F1 21 0 R >> >> /Pg 4 0 R";
            o.insert(66, st(d, "BT /F1 12 Tf 20 50 Td (KEEPME) Tj ET"));
            t.p0x = "/Resources << /Font << /F1 21 0 R >> /XObject << /X 66 0 R >> >>";
            p0c = format!("{} /X Do", t.p0c);
            t.p0c = &p0c;
        }
        // A field with a widget on each page; page 1's has a caption, a
        // tooltip and its own appearance.
        "redwidgetfield" => {
            (t.cat, t.p0x, t.p1x) = (
                "/AcroForm << /Fields [70 0 R] >>",
                "/Annots [50 0 R]",
                "/Annots [51 0 R]",
            );
            o.insert(
                70,
                "<< /FT /Tx /T (f) /V (SHAREDVAL) /Kids [50 0 R 51 0 R] >>".into(),
            );
            let w = "/Type /Annot /Subtype /Widget /Parent 70 0 R /Rect [0 0 50 20]";
            o.insert(50, format!("<< {w} /P 3 0 R >>"));
            o.insert(
                51,
                format!(
                    "<< {w} /P 4 0 R /MK << /CA (MKSECRET) >> /TU (TUSECRET) /AP << /N 54 0 R >> >>"
                ),
            );
            let d = "/Type /XObject /Subtype /Form /BBox [0 0 50 20] /Resources << /Font << /F1 21 0 R >> >>";
            o.insert(54, st(d, "BT /F1 9 Tf 2 2 Td (APSECRET) Tj ET"));
        }
        // /Fields and /DR name page 1 itself, which holds private data.
        "drpage" => {
            t.cat = "/AcroForm << /Fields [71 0 R 4 0 R] /DA (/F1 0 Tf 0 g) \
                /DR << /Font << /F1 21 0 R >> /Foo 4 0 R /Bar [4 0 R] >> >>";
            t.p0x = "/Annots [71 0 R]";
            o.insert(71, widget(1, 3, "KEPTVALUE"));
            p1x = format!("{piece} /Foo (PAGEDICTSECRET)");
            t.p1x = &p1x;
        }
        // Page 0's own /PieceInfo names page 1.
        "keptpieceref" => {
            t.p0x = "/PieceInfo << /App << /LastModified (D:2020) /Private 4 0 R >> >>";
            t.p1x = "/Foo (PAGEDICTSECRET)";
        }
        // Page 0 fills itself in a layer that is off by default.
        "hiddenocg" => {
            t.cat = "/OCProperties << /OCGs [73 0 R] /D << /OFF [73 0 R] >> >>";
            o.insert(73, "<< /Type /OCG /Name (Hidden) >>".into());
            t.p0x = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R >> >>";
            t.p0c = "/OC /oc1 BDC 0 0 200 200 re f EMC";
        }
        _ => panic!("unknown probe {v}"),
    }
    assemble(dir, v, o, &t)
}

/// `kidsind.pdf`: the two-page skeleton with /Kids in object 6.
fn kidsind(dir: &Path) -> PathBuf {
    let st = |data: &str| String::from_utf8(stream("", data)).unwrap();
    let res = "/Resources << /Font << /F1 7 0 R >> >>";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids 6 0 R /Count 2 /MediaBox [0 0 200 200] >>".into(),
        format!("<< /Type /Page /Parent 2 0 R {res} /Contents 4 0 R >>"),
        st("BT /F1 12 Tf 20 100 Td (Hello) Tj ET"),
        format!("<< /Type /Page /Parent 2 0 R {res} /Contents 8 0 R >>"),
        "[3 0 R 5 0 R]".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        st("BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET"),
    ];
    let objs: Vec<Vec<u8>> = objs.into_iter().map(String::into_bytes).collect();
    write_pdf(dir, "kidsind.pdf", &objs)
}
