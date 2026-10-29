//! gen3 redaction probes on the [`super::probe`] skeleton (page 0 kept,
//! page 1 redacted): kept references to page 1 and to the catalog, an
//! attachment, and what kept pages share with page 1.
use super::probe::{Two, assemble};
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `<v>.pdf` in `dir` for probe `v`: `pref`, `catref`, `describers`, `attach`,
/// `sharedform`, `apshared` (each arm says what it holds).
pub fn probe3_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    let form =
        "/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 21 0 R >> >>";
    let (p0c, p1c);
    let mut t = Two {
        cat: "",
        p0x: "",
        p1x: "",
        p0c: "BT /F1 12 Tf 20 100 Td (Hello) Tj ET",
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    match v {
        // A kept annotation's /P and the catalog name page 1 itself.
        "pref" => {
            (t.p0x, t.cat) = ("/Annots [6 0 R]", "/Foo 4 0 R");
            o.insert(
                6,
                "<< /Type /Annot /Subtype /Square /Rect [0 0 10 10] /P 4 0 R >>".into(),
            );
        }
        // A kept annotation names the catalog (outlines, private data).
        "catref" => {
            (t.p0x, t.cat) = (
                "/Annots [6 0 R]",
                "/Outlines 80 0 R /PieceInfo << /X << /Private (CATSECRET) >> >>",
            );
            o.insert(
                6,
                "<< /Type /Annot /Subtype /Square /Rect [0 0 10 10] /Foo 1 0 R >>".into(),
            );
            o.insert(
                80,
                "<< /Type /Outlines /First 81 0 R /Last 81 0 R /Count 1 >>".into(),
            );
            o.insert(
                81,
                "<< /Title (OUTLINESECRET) /Parent 80 0 R /Dest [4 0 R /Fit] >>".into(),
            );
        }
        // A kept annotation names what describes page 1: a struct element
        // (its /Pg), a bookmark (its /Dest), an action (its /D).
        "describers" => {
            t.p0x = "/Annots [6 0 R]";
            o.insert(
                6,
                "<< /Type /Annot /Subtype /Square /Rect [0 0 10 10] /Foo [32 0 R 81 0 R 82 0 R] >>"
                    .into(),
            );
            o.insert(
                32,
                "<< /Type /StructElem /S /P /Pg 4 0 R /Alt (ALTSECRET) /K 0 >>".into(),
            );
            o.insert(81, "<< /Title (OUTLINESECRET) /Dest [4 0 R /Fit] >>".into());
            o.insert(
                82,
                "<< /S /GoTo /D [4 0 R /Fit] /Next << /S /JavaScript /JS (ACTSECRET) >> >>".into(),
            );
        }
        "attach" => {
            t.p1x = "/Annots [52 0 R]";
            o.insert(52, "<< /Type /Annot /Subtype /FileAttachment /Rect [40 40 50 50] /FS 55 0 R /Contents (ATTNOTE) >>".into());
            o.insert(
                55,
                "<< /Type /Filespec /F (att.txt) /EF << /F 56 0 R >> >>".into(),
            );
            o.insert(56, st("/Type /EmbeddedFile", "ATTSECRET"));
        }
        // Both pages draw form 22 (kept); page 0 draws page 1's note
        // appearance (kept, the note goes).
        "sharedform" => {
            o.insert(22, st(form, "BT /F1 12 Tf 20 50 Td (SHAREDTEXT) Tj ET"));
            let r = "/Resources << /Font << /F1 21 0 R >> /XObject << /Sh 22 0 R >> >>";
            (p0c, p1c) = (format!("{} /Sh Do", t.p0c), format!("{} /Sh Do", t.p1c));
            (t.p0x, t.p1x, t.p0c, t.p1c) = (r, r, &p0c, &p1c);
        }
        "apshared" => {
            o.insert(53, st(form, "BT /F1 12 Tf 2 5 Td (APTEXT) Tj ET"));
            p0c = format!("{} /Ap Do", t.p0c);
            t.p0c = &p0c;
            t.p0x = "/Resources << /Font << /F1 21 0 R >> /XObject << /Ap 53 0 R >> >>";
            t.p1x = "/Annots [52 0 R]";
            o.insert(
                52,
                "<< /Type /Annot /Subtype /FreeText /Rect [40 40 150 60] /Contents (FTSECRET) \
                /DA (/F1 12 Tf) /AP << /N 53 0 R >> >>"
                    .into(),
            );
        }
        _ => panic!("unknown probe {v}"),
    }
    assemble(dir, v, o, &t)
}
