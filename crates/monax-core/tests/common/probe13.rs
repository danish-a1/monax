//! gen13 redaction probes: both pages draw through one shared resource
//! dictionary with every kind of drawing resource (a soft-masked
//! ExtGState with a transfer function, a shading pattern, images with a
//! soft mask and an image mask whose /Decode is indirect, a Type3 font with
//! its own resources and an indirect /Encoding and /Widths, Separation and
//! Indexed colour spaces, a transparency group form with an indirect /CS),
//! an annotation appearance and a page group; `diff`: a shared font whose
//! /Encoding holds an indirect /Differences. Page 1 adds PAGESECRET.
use super::scan::write_pdf;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn st(d: &str, data: &[u8]) -> Vec<u8> {
    let head = format!("<< {d} /Length {} >>\nstream\n", data.len());
    [head.as_bytes(), data, b"\nendstream"].concat()
}

/// `<v>.pdf` in `dir` for gen13 probe `v`: `drawall` or `diff`.
pub fn probe13_pdf(dir: &Path, v: &str) -> PathBuf {
    let mut o: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut s = |k: u32, x: &str| drop(o.insert(k, x.as_bytes().to_vec()));
    let form = "/Type /XObject /Subtype /Form";
    let img = "/Type /XObject /Subtype /Image /Width 2 /Height 2";
    let (page, draw);
    if v == "diff" {
        s(30, "<< /Font << /F1 120 0 R >> >>");
        s(
            120,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding 121 0 R >>",
        );
        s(
            121,
            "<< /Type /Encoding /BaseEncoding /WinAnsiEncoding /Differences 122 0 R >>",
        );
        s(122, "[65 /W /W /W /W /W]");
        (page, draw) = ("", "BT /F1 40 Tf 5 100 Td (ABCDE) Tj ET".to_string());
    } else {
        s(
            30,
            "<< /ExtGState << /GS1 31 0 R >> /Pattern << /P1 50 0 R >> \
            /XObject << /Im1 60 0 R /Im2 62 0 R /Fm 90 0 R >> /Font << /T3 70 0 R >> \
            /ColorSpace << /CS1 [/Separation /Spot /DeviceRGB 81 0 R] /CS2 [/Indexed /DeviceRGB 1 82 0 R] >> >>",
        );
        s(
            31,
            "<< /Type /ExtGState /SMask << /S /Luminosity /G 40 0 R /TR 41 0 R >> >>",
        );
        s(
            41,
            "<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>",
        );
        s(50, "<< /PatternType 2 /Shading 51 0 R >>");
        s(
            51,
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 200 0] /Function 52 0 R /Extend [true true] >>",
        );
        s(
            52,
            "<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>",
        );
        s(64, "[1 0]");
        s(
            70,
            "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] /FontMatrix [0.001 0 0 0.001 0 0] \
            /CharProcs << /a 71 0 R >> /Encoding 73 0 R /FirstChar 97 /LastChar 97 /Widths 74 0 R /Resources 72 0 R >>",
        );
        s(72, "<< /XObject << /Sub 75 0 R >> >>");
        s(73, "<< /Type /Encoding /Differences 76 0 R >>");
        s(74, "[1000]");
        s(76, "[97 /a]");
        s(
            81,
            "<< /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [1 0 0] /N 1 >>",
        );
        s(91, "/DeviceRGB");
        s(92, "<< /CA 0.5 /ca 0.5 /BM /Multiply >>");
        s(
            100,
            "<< /Type /Annot /Subtype /Square /Rect [150 0 200 50] /F 4 /AP << /N 101 0 R /D 102 0 R /R 102 0 R >> >>",
        );
        s(
            103,
            "<< /Type /Annot /Subtype /Square /Rect [150 0 200 50] /F 4 /AP << /N 101 0 R /D 102 0 R >> >>",
        );
        s(110, "<< /S /Transparency /CS 111 0 R >>");
        s(111, "/DeviceRGB");
        let gray =
            format!("{form} /BBox [0 0 200 200] /Group << /S /Transparency /CS /DeviceGray >>");
        o.insert(40, st(&gray, b"1 g 0 0 100 200 re f"));
        let (smask, mask) = (
            format!("{img} /SMask 61 0 R"),
            format!("{img} /Mask 63 0 R"),
        );
        o.insert(
            60,
            st(
                &format!("{smask} /ColorSpace /DeviceGray /BitsPerComponent 8"),
                &[0, 80, 160, 240],
            ),
        );
        o.insert(
            61,
            st(
                &format!("{img} /ColorSpace /DeviceGray /BitsPerComponent 8"),
                &[255, 0, 255, 0],
            ),
        );
        o.insert(
            62,
            st(
                &format!("{mask} /ColorSpace /DeviceRGB /BitsPerComponent 8"),
                &[0, 200, 0].repeat(4),
            ),
        );
        o.insert(
            63,
            st(
                &format!("{img} /ImageMask true /BitsPerComponent 1 /Decode 64 0 R"),
                &[0x40, 0x80],
            ),
        );
        o.insert(71, st("", b"1000 0 0 0 1000 1000 d1 /Sub Do"));
        o.insert(
            75,
            st(
                &format!("{form} /BBox [0 0 1000 1000]"),
                b"0 0 1 rg 0 0 800 800 re f",
            ),
        );
        o.insert(82, st("", &[0, 255, 0, 255, 0, 255]));
        let fm = format!(
            "{form} /BBox [0 0 200 200] /Group << /S /Transparency /CS 91 0 R /I true >> \
            /Resources << /ExtGState << /G2 92 0 R >> >>"
        );
        o.insert(90, st(&fm, b"/G2 gs 0 1 1 rg 120 120 60 60 re f"));
        o.insert(
            101,
            st(
                &format!("{form} /BBox [0 0 50 50] /Resources << /XObject << /I 60 0 R >> >>"),
                b"q 50 0 0 50 0 0 cm /I Do Q",
            ),
        );
        o.insert(
            102,
            st(&format!("{form} /BBox [0 0 50 50]"), b"0 g 0 0 50 50 re f"),
        );
        page = " /Group 110 0 R";
        draw = "q /GS1 gs 0 g 0 0 200 200 re f Q q /Pattern cs /P1 scn 0 150 200 50 re f Q \
            q 40 0 0 40 0 100 cm /Im1 Do Q q 40 0 0 40 50 100 cm /Im2 Do Q /Fm Do BT /T3 30 Tf 100 60 Td (a) Tj ET \
            q /CS1 cs 0.8 scn 0 60 30 30 re f /CS2 cs 1 sc 40 60 20 20 re f Q"
            .to_string();
    }
    let annots = |a: u32| {
        if v == "diff" {
            String::new()
        } else {
            format!(" /Annots [{a} 0 R]")
        }
    };
    let mut s = |k: u32, x: String| drop(o.insert(k, x.into_bytes()));
    s(1, "<< /Type /Catalog /Pages 2 0 R >>".into());
    s(
        2,
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 200 200] >>".into(),
    );
    s(
        3,
        format!(
            "<< /Type /Page /Parent 2 0 R /Resources 30 0 R /Contents 5 0 R{}{page} >>",
            annots(100)
        ),
    );
    s(
        4,
        format!(
            "<< /Type /Page /Parent 2 0 R /Resources 30 0 R /Contents 8 0 R{}{page} >>",
            annots(103)
        ),
    );
    let f = if v == "diff" { "F1" } else { "T3" };
    o.insert(5, st("", draw.as_bytes()));
    o.insert(
        8,
        st(
            "",
            format!("{draw} BT /{f} 12 Tf 0 0 Td (PAGESECRET) Tj ET").as_bytes(),
        ),
    );
    let n = *o.keys().last().unwrap();
    let objs: Vec<Vec<u8>> = (1..=n)
        .map(|i| o.remove(&i).unwrap_or_else(|| b"null".to_vec()))
        .collect();
    write_pdf(dir, &format!("{v}.pdf"), &objs)
}
