//! gen8 redaction probes on the [`super::probe`] skeleton (page 0 kept,
//! page 1 redacted): layer states, membership and group dictionaries that
//! must keep page 0 showing or hiding exactly what it did. Page 0 draws
//! "Hello" and a bar in layer `oc1`; page 1, when layered, in 75.
use super::probe::{Two, assemble};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every gen8 probe.
pub const GEN8: [&str; 12] = [
    "ocmdred",
    "ocmdoff",
    "usage",
    "ordernest",
    "baseoff",
    "notinocgs",
    "notinocgs2",
    "ocgsind",
    "ocmdonly",
    "dupe",
    "intent",
    "asprint",
];

const P1: &str = "/Resources << /Font << /F1 21 0 R >> /Properties << /oc2 75 0 R >> >>";
const P1C: &str = "/OC /oc2 BDC BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET EMC";
const IN: &str = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET 0 0 200 20 re f EMC";

/// Page 0's resources, drawing `oc1` as object `n`.
fn oc1(n: u32) -> String {
    format!("/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 {n} 0 R >> >>")
}

/// `<v>.pdf` in `dir` for gen8 probe `v` (each arm says what it holds).
pub fn probe8_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, String> = BTreeMap::new();
    let mut t = Two {
        cat: "",
        p0x: "",
        p1x: "",
        p0c: IN,
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    let mut ins = |k: u32, s: &str| drop(o.insert(k, s.to_string()));
    let (l, r) = ("<< /Type /OCG /Name (L) >>", "<< /Type /OCG /Name (R) >>");
    let (p0x, layered1, cat): (String, bool, &str) = match v {
        // OCMD AnyOn over kept 73 (off) and page 1's 75 (on), also by /VE.
        "ocmdred" => {
            ins(73, l);
            ins(75, r);
            ins(
                76,
                "<< /Type /OCMD /OCGs [73 0 R 75 0 R] /P /AnyOn /VE [/Or 73 0 R 75 0 R] >>",
            );
            let c = "/OCProperties << /OCGs [73 0 R 75 0 R] /D << /OFF [73 0 R] /ON [75 0 R] >> >>";
            (oc1(76), true, c)
        }
        // OCMD AllOn [73 75]; 75 off, listed only in /D.
        "ocmdoff" => {
            ins(73, l);
            ins(75, r);
            ins(76, "<< /Type /OCMD /OCGs [73 0 R 75 0 R] /P /AllOn >>");
            (
                oc1(76),
                false,
                "/OCProperties << /OCGs [73 0 R] /D << /OFF [75 0 R] >> >>",
            )
        }
        // The kept group has /Usage text and a private key.
        "usage" => {
            ins(
                73,
                "<< /Type /OCG /Name (L) /Usage << /CreatorInfo << /Creator (USAGESECRET) \
                /Subtype /Artwork >> >> /Priv (PRIVSECRET) /Intent [/View /Design] >>",
            );
            (
                oc1(73),
                false,
                "/OCProperties << /OCGs [73 0 R] /D << /Order [73 0 R] >> >>",
            )
        }
        // Nested indirect /Order arrays (a cycle) with labels; kept 73, 74
        // (off) and page 1's 75.
        "ordernest" => {
            ins(60, "[(SUBSECRET) 74 0 R [75 0 R 61 0 R]]");
            ins(61, "[73 0 R 74 0 R 60 0 R]");
            ins(73, "<< /Type /OCG /Name (A) >>");
            ins(74, "<< /Type /OCG /Name (B) >>");
            ins(75, "<< /Type /OCG /Name (NAME75SECRET) >>");
            t.p0c = "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET EMC /OC /oc3 BDC 0 0 200 20 re f EMC";
            let p =
                "/Resources << /Font << /F1 21 0 R >> /Properties << /oc1 73 0 R /oc3 74 0 R >> >>";
            let c = "/OCProperties << /OCGs [73 0 R 74 0 R 75 0 R] \
                /D << /Order [(LBLSECRET) 73 0 R 60 0 R] /OFF [74 0 R] >> >>";
            (p.into(), true, c)
        }
        // Base state off; /ON lists kept 73 and page 1's 75.
        "baseoff" => {
            ins(73, l);
            ins(75, r);
            let c = "/OCProperties << /OCGs [73 0 R 75 0 R] /D << /BaseState /OFF /ON [75 0 R 73 0 R] >> >>";
            (oc1(73), true, c)
        }
        // Page 0 draws in 74, missing from /OCGs; /D lists it off, or
        // the base state is off.
        "notinocgs" | "notinocgs2" => {
            ins(73, l);
            ins(74, "<< /Type /OCG /Name (Stray) >>");
            let c = match v {
                "notinocgs" => "/OCProperties << /OCGs [73 0 R] /D << /OFF [74 0 R] >> >>",
                _ => "/OCProperties << /OCGs [73 0 R] /D << /BaseState /OFF >> >>",
            };
            (oc1(74), false, c)
        }
        // Everything indirect; kept 73 off.
        "ocgsind" => {
            ins(62, "<< /OCGs 63 0 R /D 64 0 R >>");
            ins(63, "[73 0 R 75 0 R]");
            ins(64, "<< /OFF 65 0 R /BaseState 66 0 R >>");
            ins(65, "[73 0 R]");
            ins(66, "/ON");
            ins(73, l);
            ins(75, r);
            t.p1x = P1;
            (oc1(73), false, "/OCProperties 62 0 R")
        }
        // Page 0's OCMD names only 75 (off), which page 1 also uses.
        "ocmdonly" => {
            ins(75, r);
            ins(76, "<< /Type /OCMD /OCGs 75 0 R >>");
            t.p1x = P1;
            (
                oc1(76),
                false,
                "/OCProperties << /OCGs [75 0 R] /D << /OFF [75 0 R] >> >>",
            )
        }
        // 73 twice in /OCGs, in both /ON and /OFF.
        "dupe" => {
            ins(73, l);
            let c = "/OCProperties << /OCGs [73 0 R 73 0 R] /D << /ON [73 0 R] /OFF [73 0 R] >> >>";
            (oc1(73), false, c)
        }
        // Off, with a /Design intent (pdfium then always shows it).
        "intent" => {
            ins(73, "<< /Type /OCG /Name (L) /Intent /Design >>");
            let c = "/OCProperties << /OCGs [73 0 R] /D << /Intent /Design /OFF [73 0 R] >> >>";
            (oc1(73), false, c)
        }
        // On by /D, off by /Usage /View (pdfium applies it, /AS or not).
        "asprint" => {
            ins(
                73,
                "<< /Type /OCG /Name (L) /Usage << /View << /ViewState /OFF >> >> >>",
            );
            let c = "/OCProperties << /OCGs [73 0 R] /D << /AS [<< /Event /View /Category [/View] \
                /OCGs [73 0 R] >>] >> >>";
            (oc1(73), false, c)
        }
        _ => panic!("unknown probe {v}"),
    };
    t.p0x = &p0x;
    t.cat = cat;
    if layered1 {
        (t.p1x, t.p1c) = (P1, P1C);
    }
    assemble(dir, v, o, &t)
}

/// `groups.pdf` in `dir` (gen9): `n` groups, all listed in /OCGs and, with
/// `lists`, in /D /ON, /OFF and /Order; page 0 draws through one OCMD
/// naming them all.
pub fn groups_pdf(dir: &Path, n: u32, lists: bool) -> PathBuf {
    let refs: Vec<String> = (0..n).map(|i| format!("{} 0 R", 100 + i)).collect();
    let refs = refs.join(" ");
    let mut o: BTreeMap<u32, String> = (0..n)
        .map(|i| (100 + i, format!("<< /Type /OCG /Name (g{i}) >>")))
        .collect();
    o.insert(6, format!("<< /Type /OCMD /OCGs [{refs}] /P /AnyOn >>"));
    let d = match lists {
        true => format!("/ON [{refs}] /OFF [{refs}] /Order [{refs}]"),
        false => String::new(),
    };
    let cat = format!("/OCProperties << /OCGs [{refs}] /D << {d} >> >>");
    let p0x = oc1(6);
    let t = Two {
        cat: &cat,
        p0x: &p0x,
        p1x: "",
        p0c: "/OC /oc1 BDC BT /F1 12 Tf 20 100 Td (Hello) Tj ET EMC",
        p1c: "BT /F1 12 Tf 20 150 Td (PAGESECRET) Tj ET",
    };
    assemble(dir, "groups", o, &t)
}
