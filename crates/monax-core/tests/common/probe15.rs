//! gen15 redaction probes (page 0 kept, page 1 redacted): page 0 draws
//! "Hello" and a bar through an OCMD (object 74, not listed in /OCGs) over
//! layer 73, off by default: pdfium hides them. qpdf warns reading most of them
//! (R112): the OCMD nested past its limit (`ocmddeep`), the same behind
//! `n` objects that each warn (`ocmdflood`: the nesting line is pushed out
//! of the kept stderr), or holding a parse error (`junkdict`, `junkhex`,
//! `junkarr`). The others read clean: `ocmdplain` (control) and /OCGs
//! with a dangling (`ocgdangling`), explicit (`ocgnull`) or object
//! (`ocgnullobj`) null entry, which pdfium ignores too.
use super::probe12::write;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every gen15 probe qpdf warns on, with its size parameters.
pub const WARNED: &str = "ocmddeep ocmdflood:2000 junkdict junkhex junkarr";
/// Every gen15 probe qpdf reads clean.
pub const CLEAN: &str = "ocmdplain ocgdangling ocgnull ocgnullobj";

fn st(data: &str) -> String {
    format!("<< /Length {} >>\nstream\n{data}\nendstream", data.len())
}

/// `<v>.pdf` in `dir` for gen15 probe `v` (`name:n` for a size).
pub fn probe15_pdf(dir: &Path, v: &str) -> PathBuf {
    let (v, n) = v
        .split_once(':')
        .map_or((v, 0), |(a, b)| (a, b.parse().unwrap()));
    let mut o: BTreeMap<usize, String> = BTreeMap::new();
    let deep = format!("{}1{}", "<< /a ".repeat(1000), " >>".repeat(1000));
    let ocmd = |x: &str| format!("<< /Type /OCMD /OCGs [73 0 R] /P /AllOn {x} >>");
    let mut ocgs = "73 0 R";
    match v {
        "ocmddeep" => drop(o.insert(74, ocmd(&format!("/Junk {deep}")))),
        "ocmdflood" => {
            o.insert(74, ocmd(&format!("/Junk {deep}")));
            for i in 0..n {
                o.insert(2000 + i, "<< /A 1 /B >>".into());
            }
        }
        "junkdict" => drop(o.insert(74, ocmd("/X <<"))),
        "junkhex" => drop(o.insert(74, ocmd("/X <"))),
        "junkarr" => drop(o.insert(74, ocmd("/X [["))),
        "ocmdplain" | "ocgdangling" | "ocgnull" | "ocgnullobj" => {
            o.insert(74, ocmd(""));
            ocgs = match v {
                "ocgdangling" => "73 0 R 99999 0 R",
                "ocgnull" => "73 0 R null",
                "ocgnullobj" => "73 0 R 6 0 R", // object 6 is `null`
                _ => ocgs,
            };
        }
        _ => panic!("unknown probe {v}"),
    }
    let font = "/Font << /F1 21 0 R >>";
    let cat = format!("/OCProperties << /OCGs [{ocgs}] /D << /OFF [73 0 R] >> >>");
    o.insert(1, format!("<< /Type /Catalog /Pages 2 0 R {cat} >>"));
    o.insert(
        2,
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200] >>".into(),
    );
    o.insert(3, format!("<< /Type /Page /Parent 2 0 R /Resources << {font} /Properties << /oc1 74 0 R >> >> /Contents 5 0 R >>"));
    o.insert(
        4,
        format!("<< /Type /Page /Parent 2 0 R /Resources << {font} >> /Contents 8 0 R >>"),
    );
    o.insert(
        5,
        st("/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC"),
    );
    o.insert(8, st("BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET"));
    o.insert(
        21,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    );
    o.insert(73, "<< /Type /OCG /Name (L) >>".into());
    let last = *o.keys().last().unwrap();
    let objs: Vec<String> = (1..=last)
        .map(|i| o.remove(&i).unwrap_or_else(|| "null".into()))
        .collect();
    let name = if n > 0 { format!("{v}_{n}") } else { v.into() };
    write(dir, &format!("{name}.pdf"), &objs, true, None)
}
