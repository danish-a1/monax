//! gen12 redaction probes (page 0 kept, page 1 redacted): hostile syntax
//! around layer states (#-escaped names, comments, `endobj` in strings,
//! deep nesting, huge arrays, an unterminated string, a broken xref, no
//! `endobj`), string-typed groups, plain data pointing into page 1's
//! private dictionaries, and many groups with long intents. Page 0 draws
//! "Hello" and a bar in layer `oc1` (object 73).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every gen12 probe, with its size parameters.
pub const GEN12: &str = "vshash vscomment vsendobj basehash basecomment intenthash deepcat:1000 \
    deepocg:1000 parens:1000 deepcomment:50000 typestr typestrref basebig unterm plainrescue \
    plainrescue2 badxref noendobj many:200";

fn st(d: &str, data: &str) -> String {
    format!(
        "<< {d} /Length {} >>\nstream\n{data}\nendstream",
        data.len()
    )
}

/// `objs` (objects 1..) as `name` in `dir`; without `endobj` after non-streams when
/// `!endobj`, and the xref offsets of objects `swap` exchanged.
pub fn write(
    dir: &Path,
    name: &str,
    objs: &[String],
    endobj: bool,
    swap: Option<(usize, usize)>,
) -> PathBuf {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\n", i + 1).bytes());
        if endobj || o.contains("stream") {
            out.extend(b"endobj\n");
        }
    }
    if let Some((a, b)) = swap {
        offs.swap(a - 1, b - 1);
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    let n = objs.len() + 1;
    out.extend(format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").bytes());
    let p = dir.join(name);
    std::fs::write(&p, out).unwrap();
    p
}

/// `<v>.pdf` in `dir` for gen12 probe `v` (`name:n` for a size).
pub fn probe12_pdf(dir: &Path, v: &str) -> PathBuf {
    let (v, n) = v
        .split_once(':')
        .map_or((v, 0), |(a, b)| (a, b.parse().unwrap()));
    let font = "/Font << /F1 21 0 R >>";
    let res = |props: &str| format!("/Resources << {font} /Properties << {props} >> >>");
    let mut o: BTreeMap<usize, String> = BTreeMap::new();
    let mut cat = "/OCProperties << /OCGs [73 0 R] /D << /OFF [73 0 R] >> >>".to_string();
    let (mut p0x, mut p1x) = (res("/oc1 73 0 R"), format!("/Resources << {font} >>"));
    let on = "/OCProperties << /OCGs [73 0 R] /D << >> >>";
    let (mut endobj, mut swap) = (true, None);
    let vs_off = "/Usage << /View << /ViewState /OFF >> >>";
    let g = |rest: &str| format!("<< /Type /OCG /Name (L) {rest} >>");
    let mut g73 = None;
    match v {
        "vshash" => g73 = Some(g("/Usage << /View << /ViewState /O#46F >> >>")),
        "vscomment" => g73 = Some(g("/Usage << /View << % a b\n/ViewState /OFF >> >>")),
        "vsendobj" => g73 = Some(format!("<< /Type /OCG /Name (endobj) {vs_off} >>")),
        "basehash" => cat = "/OCProperties << /OCGs [73 0 R] /D << /BaseState /O#46F >> >>".into(),
        "basecomment" => {
            cat = "/OCProperties << /OCGs [73 0 R] /D << % x\n/BaseState /OFF >> >>".into()
        }
        "intenthash" => g73 = Some(g("/Intent /Vi#65w")),
        "deepcat" => cat += &format!(" /Junk {}{}", "[".repeat(n), "]".repeat(n)),
        "deepocg" => {
            g73 = Some(g(&format!(
                "/Junk {}1{} {vs_off}",
                "<< /a ".repeat(n),
                " >>".repeat(n)
            )))
        }
        "parens" => {
            g73 = Some(format!(
                "<< /Type /OCG /Name ({}{}) {vs_off} >>",
                "(".repeat(n),
                ")".repeat(n)
            ))
        }
        "deepcomment" => g73 = Some(g(&format!("% {}\n{vs_off}", "[".repeat(n)))),
        "typestr" => g73 = Some("<< /Type (OCG) /Name (L) /Priv 8 0 R >>".into()),
        "typestrref" => {
            g73 = Some("<< /Type 95 0 R /Name (L) >>".into());
            o.insert(95, "(OCG)".into());
        }
        "basebig" => {
            let junk = " 1".repeat(600_000);
            cat = format!(
                "/OCProperties << /OCGs [73 0 R] /D << /Junk [{junk}] /BaseState /OFF >> >>"
            );
        }
        "unterm" => g73 = Some(format!("<< /Type /OCG /Name (L) {vs_off} /X (abc")),
        // Plain data of a kept font / annotation pointing at page 1's
        // private dictionaries.
        "plainrescue" => {
            o.insert(40, "<< /A << /Private (PIECESECRET) >> >>".into());
            p1x += " /PieceInfo 40 0 R";
            o.insert(
                21,
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Name 40 0 R >>".into(),
            );
        }
        "plainrescue2" => {
            o.insert(40, "<< /Private (PIECESECRET) >>".into());
            p1x += " /PieceInfo << /X 40 0 R >>";
            o.insert(87, "<< /Type /Annot /Subtype /Square /Rect [10 10 190 190] /C [0 0 0] /F 4 /Contents 40 0 R >>".into());
            p0x += " /Annots [87 0 R]";
        }
        "badxref" => {
            g73 = Some(g(vs_off));
            o.insert(74, "<< /Type /OCG /Name (M) >>".into());
            (cat, swap) = (on.into(), Some((73, 74)));
        }
        "noendobj" => {
            g73 = Some(g(vs_off));
            (cat, endobj) = (on.into(), false);
        }
        // `n` groups with 50-long intents, no endobj, a 200 KB tail.
        "many" => {
            let ids: Vec<usize> = (100..100 + n).collect();
            for &j in &ids {
                o.insert(
                    j,
                    format!(
                        "<< /Type /OCG /Name (L) /Intent [{}] >>",
                        " /View".repeat(50)
                    ),
                );
            }
            let props: Vec<String> = ids.iter().map(|j| format!("/g{j} {j} 0 R")).collect();
            let refs: Vec<String> = ids.iter().map(|j| format!("{j} 0 R")).collect();
            p0x = res(&format!("/oc1 73 0 R {}", props.join(" ")));
            cat = format!(
                "/OCProperties << /OCGs [73 0 R {}] /D << /OFF [73 0 R] >> >>",
                refs.join(" ")
            );
            o.insert(100 + n, st("", &"x".repeat(200_000)));
            endobj = false;
        }
        _ => panic!("unknown probe {v}"),
    }
    // As gen12: these are on by default (only their /Usage may hide them).
    let on_by_default = "vshash vscomment vsendobj deepocg parens deepcomment unterm";
    if on_by_default.split(' ').any(|x| x == v) {
        cat = on.into();
    }
    o.insert(73, g73.unwrap_or_else(|| g("")));
    o.insert(
        21,
        o.get(&21)
            .cloned()
            .unwrap_or_else(|| "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into()),
    );
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
    o.insert(
        5,
        st(
            "",
            "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC",
        ),
    );
    o.insert(8, st("", "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET"));
    let last = *o.keys().last().unwrap();
    let objs: Vec<String> = (1..=last)
        .map(|i| o.remove(&i).unwrap_or_else(|| "null".into()))
        .collect();
    write(dir, &format!("{v}.pdf"), &objs, endobj, swap)
}
