//! gen17 render probes: a widget a page's /Annots reaches in a way the
//! field check must not skip (R117). One page; /Fields is empty, so pdfium
//! loads the widget (object 1000) from the page alone.
//! - `gen1elem:n` / `gen1list:n`: 1000 roots an `n`-level /Kids DAG (each
//!   node lists the next twice); the /Annots element (`1000 1 R`) or the
//!   /Annots array itself (`6 1 R`) is referenced at generation 1 though
//!   written at 0: qpdf reads null (exit 0), pdfium resolves by number.
//! - `gen1kids:n`: /Fields lists 1000, whose /Kids is `6 1 R` (written
//!   at 0) holding the DAG's next level twice; /Annots is empty.
//! - `indsub:n`: the DAG root's /Subtype is `5 0 R`, an indirect /Widget.
//! - `many:n`: 1000 has `n` leaf widget kids and /Annots lists it `n` times.
//! - `parent:n`: /Annots lists `n` widgets whose /Parent is 1000, the foot
//!   of an `n`-field /Parent chain (no /Kids): pdfium joins the chain's /T
//!   into each widget's full name.
use super::probe12::write;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `<v>_<n>.pdf` in `dir` for gen17 probe `v:n`.
pub fn probe17_pdf(dir: &Path, v: &str) -> PathBuf {
    let (v, n) = v.split_once(':').unwrap();
    let n: usize = n.parse().unwrap();
    let mut o: BTreeMap<usize, String> = BTreeMap::new();
    let w = "/Type /Annot /Rect [0 0 50 50] /P 3 0 R";
    let sub = if v == "indsub" { "5 0 R" } else { "/Widget" };
    let annots = match v {
        "gen1elem" => "[1000 1 R]".to_string(),
        "gen1list" => "6 1 R".into(),
        "many" => format!("[{}]", "1000 0 R ".repeat(n)),
        "gen1kids" => "[]".into(),
        "parent" => format!(
            "[{}]",
            (0..n)
                .map(|i| format!("{} 0 R ", 1000 + n + i))
                .collect::<String>()
        ),
        _ => "[1000 0 R]".into(),
    };
    if v == "many" {
        let kids: String = (0..n).map(|i| format!("{} 0 R ", 1001 + i)).collect();
        o.insert(
            1000,
            format!("<< /Subtype /Widget {w} /T (f) /Kids [{kids}] >>"),
        );
        for i in 0..n {
            let leaf = format!("<< /Subtype /Widget {w} /T (k{i}) /FT /Tx /Parent 1000 0 R >>");
            o.insert(1001 + i, leaf);
        }
    } else if v == "parent" {
        for i in 0..n {
            let up = if i + 1 < n {
                format!("/Parent {} 0 R", 1001 + i)
            } else {
                String::new()
            };
            o.insert(1000 + i, format!("<< /T (p{i}) {up} >>"));
            let leaf = format!("<< /Subtype /Widget {w} /FT /Tx /T (w{i}) /Parent 1000 0 R >>");
            o.insert(1000 + n + i, leaf);
        }
    } else {
        for i in 0..n {
            let k = format!("{} 0 R", 1001 + i);
            o.insert(1000 + i, format!("<< /T (f{i}) /Kids [{k} {k}] >>"));
        }
        o.insert(
            1000 + n,
            format!("<< /Subtype /Widget {w} /FT /Tx /T (w) >>"),
        );
        let kids = if v == "gen1kids" {
            "6 1 R"
        } else {
            "[1001 0 R 1001 0 R]"
        };
        o.insert(
            1000,
            format!("<< /Subtype {sub} {w} /T (f0) /Kids {kids} >>"),
        );
    }
    let fields = if v == "gen1kids" { "[1000 0 R]" } else { "[]" };
    o.insert(
        1,
        format!("<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields {fields} >> >>"),
    );
    o.insert(
        2,
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 200] >>".into(),
    );
    o.insert(
        3,
        format!("<< /Type /Page /Parent 2 0 R /Annots {annots} /Contents 4 0 R >>"),
    );
    o.insert(
        4,
        "<< /Length 14 >>\nstream\n0 0 50 50 re f\nendstream".into(),
    );
    o.insert(5, "/Widget".into());
    let six = if v == "gen1kids" {
        "[1001 0 R 1001 0 R]"
    } else {
        "[1000 0 R]"
    };
    o.insert(6, six.into());
    let last = *o.keys().last().unwrap();
    let objs: Vec<String> = (1..=last)
        .map(|i| o.remove(&i).unwrap_or_else(|| "null".into()))
        .collect();
    write(dir, &format!("{v}_{n}.pdf"), &objs, true, None)
}
