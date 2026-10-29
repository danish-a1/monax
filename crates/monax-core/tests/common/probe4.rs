//! More redaction probes (gen4) on the [`super::probe`] skeleton: objects
//! owned by page 1 that a catalog entry also names, forms drawn only by
//! page 1, page 1's content doubling as page 0's form, and radio fields.
use super::probe::{Two, assemble};
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const RADIO: &str = "/AcroForm << /Fields [70 0 R] >>";
const W0: &str = "<< /Type /Annot /Subtype /Widget /Parent 70 0 R /Rect [20 170 40 190] /P 3 0 R";
const W1: &str = "<< /Type /Annot /Subtype /Widget /Parent 70 0 R /Rect [20 170 40 190] /P 4 0 R";

/// `<v>.pdf` in `dir` for gen4 probe `v`: `piece`, `thumb`, `metaref`,
/// `turef`, `aaref`, `catform`, `splittj` (page 1's /PieceInfo, /Thumb,
/// /Metadata, ToUnicode font, /AA JavaScript, form, split-Tj form, each also
/// named from the catalog; `piecelocal`, `meta`, `tu`, `aa` not named),
/// `contentform` (page 1's content is a form page
/// 0 draws), `apkept` (a form page 1 draws is a kept annotation's
/// appearance), `opt` / `dv` / `nref` (a radio field across both pages).
pub fn probe4_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    let form = |txt: &str| {
        let d = "/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 21 0 R >> >>";
        st(d, &format!("BT /F1 12 Tf 20 50 Td ({txt}) Tj ET"))
    };
    let draws_x = "/Resources << /Font << /F1 21 0 R >> /XObject << /X 66 0 R >> >>";
    let p1 = "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET";
    let p1_x = "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET /X Do";
    let mut t = Two {
        cat: "",
        p0x: "",
        p1x: "",
        p0c: "BT /F1 12 Tf 20 100 Td (Hello) Tj ET",
        p1c: p1,
    };
    // The `*ref` probes (and `piece`) also name the object from the catalog.
    let named = |cat: &'static str| {
        if v.ends_with("ref") || v == "piece" {
            cat
        } else {
            ""
        }
    };
    let tiny = st(
        "/Type /XObject /Subtype /Form /BBox [0 0 20 20]",
        "0 0 20 20 re f",
    );
    match v {
        "piece" | "piecelocal" => {
            (t.p1x, t.cat) = (
                "/PieceInfo << /App << /LastModified (D:2020) /Private 60 0 R >> >>",
                named("/Foo 60 0 R"),
            );
            o.insert(60, st("", "BT (PIECESECRET) Tj ET"));
        }
        "thumb" => {
            (t.p1x, t.cat) = ("/Thumb 60 0 R", "/Foo 60 0 R");
            o.insert(
                60,
                st(
                    "/Width 11 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
                    "THUMBSECRET",
                ),
            );
        }
        "metaref" | "meta" => {
            (t.p1x, t.cat) = ("/Metadata 60 0 R", named("/Foo 60 0 R"));
            o.insert(60, st("/Type /Metadata /Subtype /XML", "<x>METASECRET</x>"));
        }
        "turef" | "tu" => {
            (t.p1x, t.cat) = (
                "/Resources << /Font << /F1 21 0 R /F2 63 0 R >> >>",
                named("/Foo 63 0 R"),
            );
            o.insert(
                63,
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 64 0 R >>".into(),
            );
            o.insert(64, st("", "/CMapName /TUSECRET def"));
            t.p1c = "BT /F2 12 Tf 20 150 Td (PAGESECRET) Tj ET";
        }
        "aaref" | "aa" => {
            (t.p1x, t.cat) = match v {
                "aa" => (
                    "/AA << /O << /S /JavaScript /JS (app.alert('JSSECRET')) >> /C 65 0 R >>",
                    "",
                ),
                _ => ("/AA << /O 65 0 R >>", "/Foo 65 0 R"),
            };
            o.insert(65, "<< /S /JavaScript /JS 66 0 R >>".into());
            o.insert(66, st("", "app.alert('JSSTREAMSECRET')"));
        }
        "apkept" => {
            o.insert(66, form("APKEPTTEXT"));
            (t.p1x, t.p1c, t.p0x) = (draws_x, p1_x, "/Annots [67 0 R]");
            o.insert(
                67,
                "<< /Type /Annot /Subtype /Stamp /Rect [0 0 200 200] /AP << /N 66 0 R >> >>".into(),
            );
        }
        "catform" => {
            o.insert(66, form("FORMSECRET"));
            (t.p1x, t.p1c, t.cat) = (draws_x, p1_x, "/PieceInfo << /X << /Private 66 0 R >> >>");
        }
        "splittj" => {
            let d = "/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 21 0 R >> >>";
            o.insert(66, st(d, "BT /F1 12 Tf 20 50 Td (SPLIT) Tj (WORDS) Tj ET"));
            (t.p1x, t.p1c, t.cat) = (draws_x, p1_x, "/Foo 66 0 R");
        }
        "contentform" => {
            o.insert(8, form("SHOWNKEPT"));
            t.p0x = "/Resources << /Font << /F1 21 0 R >> /XObject << /K 8 0 R >> >>";
            t.p0c = "BT /F1 12 Tf 20 100 Td (Hello) Tj ET /K Do";
        }
        "opt" | "dv" | "nref" => {
            (t.cat, t.p0x, t.p1x) = (RADIO, "/Annots [50 0 R]", "/Annots [51 0 R]");
            o.insert(54, tiny);
            let (field, w0, w1) = match v {
                "opt" => (
                    "/V /0 /DV /1 /Opt [(Healthy) (OPTSECRET)]",
                    "/AS /0 /AP << /N << /0 54 0 R /Off 54 0 R >> >>",
                    "/AS /Off /AP << /N << /1 54 0 R /Off 54 0 R >> >>",
                ),
                "dv" => (
                    "/V /Off /DV /DVSECRET",
                    "/AS /Off /AP << /N << /Healthy 54 0 R /Off 54 0 R >> >>",
                    "/AS /Off /AP << /N << /DVSECRET 54 0 R /Off 54 0 R >> >>",
                ),
                _ => {
                    o.insert(55, "<< /Healthy 54 0 R /Off 54 0 R >>".into());
                    (
                        "/V /Healthy",
                        "/AS /Healthy /AP << /N 55 0 R >>",
                        "/AS /Off /AP << /N << /Other 54 0 R /Off 54 0 R >> >>",
                    )
                }
            };
            o.insert(
                70,
                format!("<< /FT /Btn /Ff 49152 /T (r) {field} /Kids [50 0 R 51 0 R] >>"),
            );
            o.insert(50, format!("{W0} {w0} >>"));
            o.insert(51, format!("{W1} {w1} >>"));
        }
        _ => panic!("unknown probe {v}"),
    }
    assemble(dir, v, o, &t)
}
