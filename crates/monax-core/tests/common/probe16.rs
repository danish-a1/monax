//! gen16 probes: qpdf reads them clean (exit 0) but other than pdfium.
//! Redaction (page 0 kept, page 1 redacted): page 0 draws "Hello" and a
//! bar through OCMD 74 over layer 73, off by default, so pdfium hides
//! them; qpdf reads 74 as null (it would show them): 74 defined as
//! `74 1 obj` but referenced `74 0 R` (`gen1`), referenced `74 1 R` but
//! defined at generation 0 (`gen1ref`), or marked free by an incremental
//! update that pdfium ignores (`incrfree`). Render: [`gen1_fields`] moves
//! every field of a fielddag probe to generation 1, which qpdf no longer
//! resolves from the generation-0 /Kids references, but pdfium does.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every gen16 redaction probe.
pub const GEN16: &str = "gen1 gen1ref incrfree";

fn st(data: &str) -> String {
    format!("<< /Length {} >>\nstream\n{data}\nendstream", data.len())
}

/// `<v>.pdf` in `dir` for gen16 redaction probe `v`; `v+form` adds an
/// empty form (`/AcroForm << /Fields [] >>`) to the catalog.
pub fn probe16_pdf(dir: &Path, v: &str) -> PathBuf {
    let name = v;
    let (v, form) = match v.strip_suffix("+form") {
        Some(v) => (v, "/AcroForm << /Fields [] >> "),
        None => (v, ""),
    };
    let font = "/Font << /F1 21 0 R >>";
    let r74 = if v == "gen1ref" { "74 1 R" } else { "74 0 R" };
    let mut o: BTreeMap<usize, String> = BTreeMap::new();
    let cat = format!("{form}/OCProperties << /OCGs [73 0 R] /D << /OFF [73 0 R] >> >>");
    o.insert(1, format!("<< /Type /Catalog /Pages 2 0 R {cat} >>"));
    o.insert(
        2,
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200] >>".into(),
    );
    o.insert(3, format!("<< /Type /Page /Parent 2 0 R /Resources << {font} /Properties << /oc1 {r74} >> >> /Contents 5 0 R >>"));
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
    o.insert(74, "<< /Type /OCMD /OCGs [73 0 R] /P /AllOn >>".into());
    let m = *o.keys().last().unwrap();
    let g = |i: usize| usize::from(v == "gen1" && i == 74);
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for i in 1..=m {
        offs.push(out.len());
        let body = o.get(&i).map_or("null", String::as_str);
        out.extend(format!("{i} {} obj\n{body}\nendobj\n", g(i)).bytes());
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", m + 1).bytes());
    for (k, off) in offs.iter().enumerate() {
        out.extend(format!("{off:010} {:05} n \n", g(k + 1)).bytes());
    }
    let n = m + 1;
    out.extend(format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").bytes());
    if v == "incrfree" {
        let x2 = out.len();
        out.extend(format!("xref\n0 1\n0000000000 65535 f \n74 1\n0000000000 00001 f \ntrailer\n<< /Size {n} /Root 1 0 R /Prev {x} >>\nstartxref\n{x2}\n%%EOF\n").bytes());
    }
    let p = dir.join(format!("{name}.pdf"));
    std::fs::write(&p, out).unwrap();
    p
}

/// `src` (a fielddag probe: fields are objects 1000 and up, written
/// `N 0 obj`; one xref section `0 N`, 20-byte entries) with every field at
/// generation 1.
pub fn gen1_fields(src: &Path) -> PathBuf {
    let b = std::fs::read(src).unwrap();
    let x = b.windows(6).rposition(|w| w == b"\nxref\n").unwrap() + 1;
    let (head, tail) = b.split_at(x);
    let mut tail = tail.to_vec();
    let text = String::from_utf8_lossy(&tail).into_owned();
    let line = text.lines().nth(1).unwrap(); // "0 N"
    let count: usize = line.split(' ').nth(1).unwrap().parse().unwrap();
    let rows = "xref\n".len() + line.len() + 1;
    let mut head = String::from_utf8(head.to_vec()).unwrap();
    for n in 1000..count {
        let e = rows + n * 20;
        if tail[e + 17] == b'n' {
            tail[e + 11..e + 16].copy_from_slice(b"00001");
            head = head.replacen(&format!("\n{n} 0 obj\n"), &format!("\n{n} 1 obj\n"), 1);
        }
    }
    let stem = src.file_stem().unwrap().to_string_lossy();
    let p = src.with_file_name(format!("{stem}-g1.pdf"));
    std::fs::write(&p, [head.as_bytes(), &tail].concat()).unwrap();
    p
}

/// `src` (a fielddag probe, as [`gen1_fields`]) with its fields but the
/// page's widget renamed to generation 1 (references left at 0), a
/// harmless generation-0 field 1000 written first, and no xref: both readers rebuild it, qpdf reads
/// `1000 0 R` as the harmless field and pdfium, by number, as the tree.
pub fn rebuilt_fields(src: &Path) -> PathBuf {
    let b = std::fs::read(src).unwrap();
    let x = b.windows(6).rposition(|w| w == b"\nxref\n").unwrap() + 1;
    let mut head = String::from_utf8(b[..x].to_vec()).unwrap();
    // The page's widget stays at generation 0: a null /Annots element is
    // refused on its own (R117), here and in the repaired file.
    for n in 1000..2000 {
        if head.contains(&format!("/Annots [{n} 0 R]")) {
            continue;
        }
        head = head.replacen(&format!("\n{n} 0 obj\n"), &format!("\n{n} 1 obj\n"), 1);
    }
    let i = head.find('\n').unwrap() + 1;
    head.insert_str(i, "1000 0 obj\n<< /T (b) >>\nendobj\n");
    head.push_str("trailer\n<< /Root 1 0 R /Size 2000 >>\nstartxref\n999999\n%%EOF\n");
    let stem = src.file_stem().unwrap().to_string_lossy();
    let p = src.with_file_name(format!("{stem}-rb.pdf"));
    std::fs::write(&p, head).unwrap();
    p
}
