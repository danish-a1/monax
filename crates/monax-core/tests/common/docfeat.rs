//! Two-page PDFs (page 1 redacted) with one document-level feature each,
//! holding a `*SECRET` string: a redacted output is built from its pages
//! alone, so each feature is dropped and named in the job notes.
use super::probe::{Two, assemble};
use super::secret::stream;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// (probe, its secret, the label the drop note gives it).
pub const FEATURES: [(&str, &str, &str); 12] = [
    ("outlines", "OUTLINESECRET", "bookmarks"),
    ("names", "NAMEDSECRET", "named destinations"),
    ("dests", "DESTSECRET", "named destinations"),
    ("js", "JSDOCSECRET", "document JavaScript"),
    ("files", "ATTSECRET", "attachments"),
    ("labels", "LABELSECRET", "page labels"),
    ("xmp", "XMPSECRET", "metadata"),
    ("info", "INFOSECRET", "metadata"),
    ("open", "OPENSECRET", "the open action"),
    ("viewer", "VIEWSECRET", "viewer settings"),
    ("xfa", "XFASECRET", "XFA form data"),
    ("private", "PRIVSECRET", "/PieceInfo"),
];

/// `<v>.pdf` in `dir` for feature `v` of [`FEATURES`].
pub fn feature_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let st = |d: &str, data: &str| String::from_utf8(stream(d, data)).unwrap();
    let mut ins = |k: u32, s: &str| drop(o.insert(k, s.to_string()));
    let cat = match v {
        "outlines" => {
            ins(
                80,
                "<< /Type /Outlines /First 81 0 R /Last 81 0 R /Count 1 >>",
            );
            ins(
                81,
                "<< /Title (OUTLINESECRET) /Parent 80 0 R /Dest [4 0 R /Fit] >>",
            );
            "/Outlines 80 0 R"
        }
        "names" => {
            ins(82, "<< /Names [(NAMEDSECRET) [4 0 R /Fit]] >>");
            "/Names << /Dests 82 0 R >>"
        }
        "dests" => "/Dests << /DESTSECRET [4 0 R /Fit] >>",
        "js" => {
            "/Names << /JavaScript << /Names [(n) << /S /JavaScript /JS (JSDOCSECRET) >>] >> >>"
        }
        "files" => {
            ins(83, "<< /Type /Filespec /F (a.txt) /EF << /F 84 0 R >> >>");
            ins(84, &st("/Type /EmbeddedFile", "ATTSECRET"));
            "/Names << /EmbeddedFiles << /Names [(a.txt) 83 0 R] >> >>"
        }
        "labels" => "/PageLabels << /Nums [0 << /S /D >> 1 << /P (LABELSECRET) >>] >>",
        "xmp" => {
            ins(85, &st("/Type /Metadata /Subtype /XML", "<x>XMPSECRET</x>"));
            "/Metadata 85 0 R"
        }
        "info" => {
            ins(86, "<< /Title (INFOSECRET) >>");
            ""
        }
        "open" => "/OpenAction << /S /JavaScript /JS (OPENSECRET) >>",
        "viewer" => "/ViewerPreferences << /Direction /L2R /VIEWSECRET true >>",
        "xfa" => "/AcroForm << /Fields [] /XFA (XFASECRET) >>",
        "private" => "/PieceInfo << /X << /Private (PRIVSECRET) >> >>",
        _ => panic!("unknown feature {v}"),
    };
    let t = Two {
        cat,
        p0x: "",
        p1x: "",
        p0c: "BT /F1 12 Tf 20 100 Td (Hello) Tj ET",
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    let p = assemble(dir, v, o, &t);
    if v == "info" {
        // The trailer follows the xref table, so no offset moves.
        let pdf = std::fs::read(&p).unwrap();
        let s = String::from_utf8(pdf).unwrap();
        std::fs::write(
            &p,
            s.replace("/Root 1 0 R >>", "/Root 1 0 R /Info 86 0 R >>"),
        )
        .unwrap();
    }
    p
}
