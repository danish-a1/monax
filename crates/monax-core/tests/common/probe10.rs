//! gen10 redaction probes on the [`super::probe`] skeleton (page 0 kept,
//! page 1 redacted): group /Usage view states, /Name and /Intent forms
//! pdfium reads, and kept non-groups (a font, resources, an annotation)
//! listed in /OCGs. Page 0 draws "Hello" and a bar in layer `oc1`.
use super::probe::{Two, assemble};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every gen10 probe (`typeless` is ours: a group without /Type).
pub const GEN10: [&str; 12] = [
    "vson",
    "vsfoo",
    "nameref",
    "namedict",
    "intentref",
    "intentdes",
    "intentempty",
    "intentstr",
    "listfont",
    "listres",
    "listannot",
    "typeless",
];

const IN: &str = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC";
const OC1: &str = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R >> >>";
const OFF: &str = "/OCProperties << /OCGs [73 0 R] /D << /OFF [73 0 R] >> >>";

/// `<v>.pdf` in `dir` for gen10 probe `v` (each arm says what it holds).
pub fn probe10_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let mut t = Two {
        cat: OFF,
        p0x: OC1,
        p1x: "",
        p0c: IN,
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    let mut ins = |k: u32, s: &str| drop(o.insert(k, s.to_string()));
    let l = "<< /Type /OCG /Name (L) >>";
    match v {
        // Off by /D, but /Usage /View /ViewState (pdfium shows it).
        "vson" => ins(
            73,
            "<< /Type /OCG /Name (L) /Usage << /View << /ViewState /ON >> >> >>",
        ),
        "vsfoo" => ins(
            73,
            "<< /Type /OCG /Name (L) /Usage << /View << /ViewState /Foo >> >> >>",
        ),
        // /Name an indirect string.
        "nameref" => {
            ins(73, "<< /Type /OCG /Name 80 0 R >>");
            ins(80, "(NAMEREFKEPT)");
        }
        // /Name a dictionary or an array carrying text.
        "namedict" => {
            ins(
                73,
                "<< /Type /OCG /Name << /T (NAMEDICTSECRET) /R 81 0 R >> >>",
            );
            ins(81, "<< /X (NAMEDEEPSECRET) >>");
            ins(74, "<< /Type /OCG /Name [(NAMEARRSECRET) 82 0 R] >>");
            ins(82, "(NAMEARRREFSECRET)");
            t.cat = "/OCProperties << /OCGs [73 0 R 74 0 R] /D << /OFF [73 0 R] >> >>";
            t.p0x =
                "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R /oc3 74 0 R >> >>";
            t.p0c = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET EMC /OC /oc3 BDC 0 0 200 20 re f EMC";
        }
        // /Intent through references, with a text string.
        "intentref" => {
            ins(
                73,
                "<< /Type /OCG /Name (L) /Intent [84 0 R 83 0 R (INTENTSECRET)] >>",
            );
            ins(83, "/Design");
            ins(84, "/View");
        }
        // /Intent an indirect array naming /Design only (pdfium shows it).
        "intentdes" => {
            ins(73, "<< /Type /OCG /Name (L) /Intent 85 0 R >>");
            ins(85, "[83 0 R]");
            ins(83, "/Design");
        }
        "intentempty" => ins(73, "<< /Type /OCG /Name (L) /Intent [] >>"),
        "intentstr" => ins(73, "<< /Type /OCG /Name (L) /Intent (Design) >>"),
        // Page 0's font listed in /OCGs.
        "listfont" => {
            ins(73, l);
            t.cat = "/OCProperties << /OCGs [73 0 R 21 0 R] /D << >> >>";
        }
        // Page 0's indirect /Resources listed in /OCGs.
        "listres" => {
            ins(73, l);
            ins(
                86,
                "<< /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R >> >>",
            );
            t.cat = "/OCProperties << /OCGs [73 0 R 86 0 R] >>";
            t.p0x = "/Resources 86 0 R";
        }
        // Page 0's annotation listed in /OCGs.
        "listannot" => {
            ins(73, l);
            ins(
                87,
                "<< /Type /Annot /Subtype /Square /Rect [10 10 190 190] /C [0 0 0] \
                /IC [0 0 0] /BS << /W 5 >> /F 4 >>",
            );
            t.cat = "/OCProperties << /OCGs [73 0 R 87 0 R] >>";
            t.p0x = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R >> >> /Annots [87 0 R]";
        }
        // A group without /Type, off, with a private key.
        "typeless" => ins(73, "<< /Name (L) /Priv (TYPELESSSECRET) >>"),
        _ => panic!("unknown probe {v}"),
    }
    assemble(dir, v, o, &t)
}
