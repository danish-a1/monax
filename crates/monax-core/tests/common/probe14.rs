//! gen14 redaction probes (page 0 kept, page 1 redacted): over-deep and
//! long input structure (a group nested past qpdf's limit, a chain of
//! indirect /OCGs arrays, a field kids DAG) and dictionaries of plain data
//! a kept page draws with (a shared /BS, a /CIDSystemInfo direct or
//! indirect). Page 0 draws "Hello" and a bar in layer `oc1` (object 73),
//! off by default.
use super::probe12::write;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn st(d: &str, data: &str) -> String {
    format!(
        "<< {d} /Length {} >>\nstream\n{data}\nendstream",
        data.len()
    )
}

/// A non-embedded Japan1 CID font (21, 22, 24) with `/CIDSystemInfo` `info`.
fn cid(o: &mut BTreeMap<usize, String>, info: &str) {
    o.insert(21, "<< /Type /Font /Subtype /Type0 /BaseFont /KozMinPro-Regular /Encoding /Identity-H /DescendantFonts [22 0 R] >>".into());
    o.insert(22, format!("<< /Type /Font /Subtype /CIDFontType0 /BaseFont /KozMinPro-Regular /CIDSystemInfo {info} /FontDescriptor 24 0 R /DW 1000 >>"));
    o.insert(24, "<< /Type /FontDescriptor /FontName /KozMinPro-Regular /Flags 6 /FontBBox [0 -200 1000 900] /ItalicAngle 0 /Ascent 880 /Descent -120 /CapHeight 700 /StemV 80 >>".into());
}

/// `<v>.pdf` in `dir` for gen14 probe `v` (`name:n` for a size).
pub fn probe14_pdf(dir: &Path, v: &str) -> PathBuf {
    let (v, n) = v
        .split_once(':')
        .map_or((v, 0), |(a, b)| (a, b.parse().unwrap()));
    let font = "/Font << /F1 21 0 R >>";
    let mut o: BTreeMap<usize, String> = BTreeMap::new();
    let mut cat = "/OCProperties << /OCGs [73 0 R] /D << /OFF [73 0 R] >> >>".to_string();
    let mut p0x = format!("/Resources << {font} /Properties << /oc1 73 0 R >> >>");
    let mut p1x = format!("/Resources << {font} >>");
    let mut p0c = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC";
    let mut p1c = "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET";
    o.insert(
        21,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    );
    o.insert(73, "<< /Type /OCG /Name (L) >>".into());
    let cid_text = (
        "BT /F1 40 Tf 5 100 Td <0022002300240025> Tj ET",
        "BT /F1 12 Tf 20 150 Td <0022> Tj ET (PAGESECRET) Tj",
    );
    match v {
        // /OCGs: a chain of `n` indirect arrays ending at 73.
        "ocgchain" => {
            for i in 0..n {
                let next = if i + 1 < n {
                    format!("[{} 0 R]", 1001 + i)
                } else {
                    "[73 0 R]".into()
                };
                o.insert(1000 + i, next);
            }
            cat = "/OCProperties << /OCGs 1000 0 R /D << /OFF [73 0 R] >> >>".into();
        }
        // Field kids DAG: each node lists the next twice (`fieldchain`:
        // once, a healthy tree). `widgetdag`: the DAG's root is a widget
        // listed only in page 1's /Annots, /Fields empty (rv8 mkw.py).
        "fielddag" | "fieldchain" | "widgetdag" => {
            for i in 0..n {
                let k = format!("{} 0 R", 1001 + i);
                let kids = if v == "fieldchain" {
                    k
                } else {
                    format!("{k} {k}")
                };
                o.insert(1000 + i, format!("<< /T (f{i}) /Kids [{kids}] >>"));
            }
            o.insert(
                1000 + n,
                "<< /Type /Annot /Subtype /Widget /FT /Tx /T (w) /Rect [0 0 50 20] /P 4 0 R >>"
                    .into(),
            );
            cat = "/AcroForm << /Fields [1000 0 R] >>".into();
            p1x += &format!(" /Annots [{} 0 R]", 1000 + n);
            if v == "widgetdag" {
                let w = "/Type /Annot /Subtype /Widget /Rect [0 0 50 50] /P 4 0 R";
                o.insert(1000, format!("<< {w} /T (f0) /Kids [1001 0 R 1001 0 R] >>"));
                cat = "/AcroForm << /Fields [] >>".into();
                p1x = format!("/Resources << {font} >> /Annots [1000 0 R]");
            }
        }
        // Group 73 nested `n` deep, off by default: pdfium hides it.
        "deepocgoff" => {
            let deep = format!("{}1{}", "<< /a ".repeat(n), " >>".repeat(n));
            o.insert(73, format!("<< /Type /OCG /Name (L) /Junk {deep} >>"));
        }
        "cidsys" | "cidreg" => {
            if v == "cidsys" {
                cid(&mut o, "23 0 R");
                o.insert(
                    23,
                    "<< /Registry (Adobe) /Ordering (Japan1) /Supplement 4 >>".into(),
                );
            } else {
                cid(
                    &mut o,
                    "<< /Registry (Adobe) /Ordering 23 0 R /Supplement 4 >>",
                );
                o.insert(23, "(Japan1)".into());
            }
            cat.clear();
            p0x = format!("/Resources << {font} >>");
            (p0c, p1c) = cid_text;
        }
        // Kept and redacted squares share a /BS, no /AP.
        "bs" => {
            o.insert(50, "<< /W 12 /S /S >>".into());
            let sq =
                "/Type /Annot /Subtype /Square /Rect [20 20 180 180] /C [0 0 0] /F 4 /BS 50 0 R";
            o.insert(60, format!("<< {sq} >>"));
            o.insert(61, format!("<< {sq} /Contents (PAGESECRET) >>"));
            cat.clear();
            p0x += " /Annots [60 0 R]";
            p1x += " /Annots [61 0 R]";
        }
        _ => panic!("unknown probe {v}"),
    }
    o.insert(1, format!("<< /Type /Catalog /Pages 2 0 R {cat} >>"));
    o.insert(
        2,
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200] >>".into(),
    );
    o.insert(
        3,
        format!("<< /Type /Page /Parent 2 0 R {p0x} /Contents 5 0 R >>"),
    );
    o.insert(
        4,
        format!("<< /Type /Page /Parent 2 0 R {p1x} /Contents 8 0 R >>"),
    );
    o.insert(5, st("", p0c));
    o.insert(8, st("", p1c));
    let last = *o.keys().last().unwrap();
    let objs: Vec<String> = (1..=last)
        .map(|i| o.remove(&i).unwrap_or_else(|| "null".into()))
        .collect();
    let name = if n > 0 { format!("{v}_{n}") } else { v.into() };
    write(dir, &format!("{name}.pdf"), &objs, true, None)
}
