//! gen7 redaction probes on the [`super::probe`] skeleton (page 0 kept,
//! page 1 redacted): layer properties that reference page 1 and its
//! objects, name removed layers, or branch into a reference bomb; kept
//! layers behind an OCMD or off by base state; /CO naming page 1's fields;
//! a direct kept link to page 1; an unreferenced object.
use super::probe::{Two, assemble};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const OC1: &str = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R >> >>";
const OC1_76: &str = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 76 0 R >> >>";
const OC2: &str = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc2 75 0 R >> >>";
const IN_OC1: &str = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC";
const IN_OC2: &str = "/OC /oc2 BDC BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET EMC";
const NOTE: &str = "<< /Type /Annot /Subtype /Text /Rect [0 0 10 10] /Contents (ANNOTSECRET) >>";

/// A text widget `t<n>` on page object `p` holding `val`, then `extra`.
fn widget(n: u32, p: u32, val: &str, extra: &str) -> String {
    format!(
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (t{n}) /V ({val}) /Rect [0 0 50 20] /P {p} 0 R {extra} >>"
    )
}

/// `<v>.pdf` in `dir` for gen7 probe `v` (each arm says what it holds).
/// Page 0 draws in layer 73 (directly or through OCMD 76) where a layer
/// is involved; page 1 in layer 75.
pub fn probe7_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let mut t = Two {
        cat: "",
        p0x: "",
        p1x: "",
        p0c: "BT /F1 12 Tf 20 100 Td (Hello) Tj ET",
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    let ocg = |o: &mut BTreeMap<u32, String>, n: u32, name: &str| {
        o.insert(n, format!("<< /Type /OCG /Name ({name}) >>"));
    };
    let calc = "/AA << /C << /S /JavaScript /JS (CALCSECRET) >> >>";
    match v {
        // /D references page 1's only annotation and page 1 itself.
        "ocpinline" => {
            t.cat =
                "/OCProperties << /OCGs [73 0 R] /D << /Order [73 0 R] /X 30 0 R /Y 4 0 R >> >>";
            (t.p0x, t.p0c) = (OC1, IN_OC1);
            t.p1x = "/Annots [30 0 R] /Foo (PAGEDICTSECRET)";
            o.insert(30, NOTE.into());
        }
        // /D and /Order branch into a self-referencing array.
        "ocpbomb" => {
            t.cat = "/OCProperties << /OCGs [73 0 R] /D << /Order [73 0 R 9 0 R] /ON 9 0 R /X 9 0 R >> >>";
            (t.p0x, t.p0c) = (OC1, IN_OC1);
            o.insert(9, format!("[{}]", "9 0 R ".repeat(8)));
        }
        // /Order labels and nests page 1's layer; /Configs names it.
        "ocplabel" => {
            t.cat = "/OCProperties << /OCGs [73 0 R 75 0 R] /D << /Name (CFG) \
                /Order [73 0 R (LABELSECRET) [75 0 R]] /OFF [75 0 R] /RBGroups [[73 0 R 75 0 R]] >> \
                /Configs [<< /Name (CFGSECRET) /Order [75 0 R] >>] >>";
            ocg(&mut o, 73, "L");
            ocg(&mut o, 75, "OCGNAMESECRET");
            (t.p0x, t.p0c, t.p1x, t.p1c) = (OC1, IN_OC1, OC2, IN_OC2);
        }
        // Kept layer 73's /Usage names page 1's note, a private key its
        // /PieceInfo: both go with the layer's other keys (R106).
        "ocgusage" => {
            t.cat = "/OCProperties << /OCGs [73 0 R] /D << /Order [73 0 R] >> >>";
            o.insert(
                73,
                "<< /Type /OCG /Name (L) /Usage << /CreatorInfo << /Creator (c) \
                /Subtype /Artwork >> /X 30 0 R >> /Priv 31 0 R >>"
                    .into(),
            );
            (t.p0x, t.p0c) = (OC1, IN_OC1);
            t.p1x = "/Annots [30 0 R] /PieceInfo << /App << /LastModified (D:2020) /Private 31 0 R >> >>";
            o.insert(30, NOTE.into());
            o.insert(31, "<< /Data (PIECESECRET) >>".into());
        }
        // Page 0 draws through OCMD 76 on layer 73; page 1's layer 75 is
        // off. `ocmdoff`: the base state is off, so 73 is hidden too.
        "ocmd" | "ocmdoff" => {
            t.cat = if v == "ocmd" {
                "/OCProperties << /OCGs [73 0 R 75 0 R] /D << /Order [73 0 R 75 0 R] /OFF [75 0 R] >> >>"
            } else {
                "/OCProperties << /OCGs [73 0 R 75 0 R] /D << /BaseState /OFF /ON [75 0 R] >> >>"
            };
            ocg(&mut o, 73, "L");
            ocg(&mut o, 75, "R");
            o.insert(76, "<< /Type /OCMD /OCGs [73 0 R] /P /AllOn >>".into());
            (t.p0x, t.p0c, t.p1x, t.p1c) = (OC1_76, IN_OC1, OC2, IN_OC2);
        }
        // /CO names a field whose only widget is on page 1.
        "coredfield" => {
            t.cat = "/AcroForm << /Fields [71 0 R 72 0 R] /CO [72 0 R 71 0 R] \
                /DA (/F1 0 Tf 0 g) /DR << /Font << /F1 21 0 R >> >> >>";
            (t.p0x, t.p1x) = ("/Annots [71 0 R]", "/Annots [72 0 R]");
            o.insert(71, widget(1, 3, "KEPTVALUE", ""));
            o.insert(72, widget(2, 4, "FIELDSECRET", calc));
        }
        // An indirect /CO names page 1's field under a kept parent.
        "coind" => {
            t.cat = "/AcroForm << /Fields [70 0 R] /CO 60 0 R >>";
            o.insert(60, "[72 0 R]".into());
            o.insert(70, "<< /T (p) /Kids [71 0 R 72 0 R] >>".into());
            o.insert(71, widget(1, 3, "KEPTVALUE", "/Parent 70 0 R"));
            let x = format!("/Parent 70 0 R {calc}");
            o.insert(72, widget(2, 4, "FIELDSECRET", &x));
            (t.p0x, t.p1x) = ("/Annots [71 0 R]", "/Annots [72 0 R]");
        }
        // Page 0's direct annotation links to page 1.
        "directlink" => {
            t.p0x = "/Annots [<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] /Dest [4 0 R /Fit] /P 4 0 R >>]";
            t.p1x = "/Foo (PAGEDICTSECRET)";
        }
        // An object nothing references.
        "objstm" => {
            t.p1x = "/Foo (PAGEDICTSECRET)";
            o.insert(90, "<< /Junk (OBJSTMSECRET) >>".into());
        }
        _ => panic!("unknown probe {v}"),
    }
    if t.cat.contains("73 0 R") {
        o.entry(73)
            .or_insert_with(|| "<< /Type /OCG /Name (L) >>".into());
    }
    assemble(dir, v, o, &t)
}
