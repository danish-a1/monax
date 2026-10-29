//! Generated image PDFs ("scans"): written at test time, never committed.
use std::path::{Path, PathBuf};

/// One image XObject: `px` x `px` pixels of `bpp` bytes each, drawn `pts`
/// x `pts` points (`px * 72 / pts` dpi). `dict` holds its /ColorSpace,
/// /BitsPerComponent and any extra entries; `smask`, when set, adds a gray
/// soft mask of the same size with these extra entries.
#[derive(Clone, Copy)]
pub struct Img {
    pub px: usize,
    pub pts: u32,
    pub bpp: usize,
    pub dict: &'static str,
    pub smask: Option<&'static str>,
}

/// An 8-bit RGB image.
pub fn rgb(px: usize, pts: u32) -> Img {
    Img {
        px,
        pts,
        bpp: 3,
        dict: "/ColorSpace /DeviceRGB /BitsPerComponent 8",
        smask: None,
    }
}

/// A gradient: byte `c` of pixel `(x, y)` varies with x, y and c.
fn pixels(px: usize, bpp: usize) -> Vec<u8> {
    (0..px * px)
        .flat_map(|i| (0..bpp).map(move |c| ((i % px + i / px * c) * 255 / (px * (c + 1))) as u8))
        .collect()
}

/// `name` in `dir`: one 200-point-high page drawing `imgs` side by side.
/// With `indirect`, every image's /Length is an indirect object.
pub fn images_pdf(dir: &Path, name: &str, imgs: &[Img], indirect: bool) -> PathBuf {
    let mut objs: Vec<Vec<u8>> = vec![vec![]; 4]; // catalog, pages, page, content
    let (mut content, mut names, mut x) = (String::new(), String::new(), 20);
    for (k, im) in imgs.iter().enumerate() {
        let data = pixels(im.px, im.bpp);
        let mut extra = String::from(im.dict);
        if let Some(m) = im.smask {
            let mask = pixels(im.px, 1);
            let d = format!(
                "<< /Type /XObject /Subtype /Image /Width {0} /Height {0} /ColorSpace /DeviceGray \
                 /BitsPerComponent 8 {m} /Length {1} >>\nstream\n",
                im.px,
                mask.len()
            );
            objs.push([d.as_bytes(), &mask, b"\nendstream"].concat());
            extra += &format!(" /SMask {} 0 R", objs.len());
        }
        let len = if indirect {
            objs.push(data.len().to_string().into_bytes());
            format!("{} 0 R", objs.len())
        } else {
            data.len().to_string()
        };
        let d = format!(
            "<< /Type /XObject /Subtype /Image /Width {0} /Height {0} {extra} /Length {len} >>\nstream\n",
            im.px
        );
        objs.push([d.as_bytes(), &data, b"\nendstream"].concat());
        names += &format!(" /Im{k} {} 0 R", objs.len());
        content += &format!("q {0} 0 0 {0} {x} 20 cm /Im{k} Do Q ", im.pts);
        x += im.pts + 10;
    }
    objs[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    objs[1] = b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec();
    objs[2] = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} 200] \
         /Resources << /XObject <<{names} >> >> /Contents 4 0 R >>",
        x + 10
    )
    .into_bytes();
    objs[3] = format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    )
    .into_bytes();
    write_pdf(dir, name, &objs)
}

/// `name` in `dir`: a PDF of `objs` (object `k + 1` is `objs[k]`; object 1
/// the catalog) with a correct xref table.
pub fn write_pdf(dir: &Path, name: &str, objs: &[Vec<u8>]) -> PathBuf {
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(pdf.len());
        pdf.extend(
            format!("{} 0 obj\n", i + 1)
                .bytes()
                .chain(o.iter().copied()),
        );
        pdf.extend(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        pdf.extend(format!("{o:010} 00000 n \n").bytes());
    }
    let n = objs.len() + 1;
    pdf.extend(format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").bytes());
    let p = dir.join(name);
    std::fs::write(&p, pdf).unwrap();
    p
}

/// One RGB `px` x `px` image drawn `pts` x `pts` points, as `scan.pdf`.
pub fn scan_pdf(dir: &Path, px: usize, pts: u32) -> PathBuf {
    images_pdf(dir, "scan.pdf", &[rgb(px, pts)], false)
}
