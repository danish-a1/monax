//! PDF to images test helpers: running the op as a registry job, reading
//! the PNGs it writes.
use super::ocr::job;
use monax_core::convert::{ImageFormat, pdf_to_images};
use monax_core::pages::PagesError;
use monax_core::{ProgressSink, Registry};
use std::path::{Path, PathBuf};

pub type R = Result<Vec<PathBuf>, PagesError>;

/// [`pdf_to_images`] into `input`'s directory on a registry job.
pub fn to_images(input: &Path, dpi: u32, format: ImageFormat, pages: Option<&[usize]>) -> R {
    let (i, p) = (input.to_path_buf(), pages.map(<[usize]>::to_vec));
    let dir = input.parent().unwrap().to_path_buf();
    job(move |s| pdf_to_images(&i, &dir, dpi, format, 85, p.as_deref(), s)).0
}

/// Width, height and straight RGBA of a PNG file.
pub fn decode(p: &Path) -> (u32, u32, Vec<u8>) {
    let mut r = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(p).unwrap()))
        .read_info()
        .unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

pub fn invalid(r: R) -> String {
    match r {
        Err(PagesError::InvalidInput(m)) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

/// TIFF payload of an APP1 Exif segment whose IFD0 holds only Orientation
/// `o`, little-endian (`II`) or big-endian (`MM`).
pub fn exif(o: u16, le: bool) -> Vec<u8> {
    let u16b = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let u32b = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut v = b"Exif\0\0".to_vec();
    v.extend(if le { b"II" } else { b"MM" });
    v.extend(u16b(42));
    v.extend(u32b(8)); // IFD0 right after the header
    v.extend(u16b(1)); // one entry
    v.extend(u16b(0x0112));
    v.extend(u16b(3)); // SHORT
    v.extend(u32b(1));
    v.extend(u16b(o));
    v.extend([0, 0]);
    v.extend(u32b(0)); // no next IFD
    v
}

/// A 32 x 16 px baseline JPEG `dir/name`, red in its top-left 16 x 8
/// quadrant and blue elsewhere, with APP1 `app1` (none when empty).
pub fn quadrant_jpeg(dir: &Path, name: &str, app1: Vec<u8>) -> PathBuf {
    let px: Vec<u8> = (0..16u32)
        .flat_map(|y| (0..32u32).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            if x < 16 && y < 8 {
                [255, 0, 0]
            } else {
                [0, 0, 255]
            }
        })
        .collect();
    let mut bytes = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut bytes, 95);
    if !app1.is_empty() {
        enc.add_app_segment(1, app1).unwrap();
    }
    enc.encode(&px, 32, 16, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

/// The sink of a finished job of its own: never cancelled.
pub fn idle() -> ProgressSink {
    let reg = Registry::new();
    let (tx, rx) = std::sync::mpsc::channel();
    reg.submit(move |s| {
        tx.send(s).unwrap();
        Ok::<_, String>(vec![])
    });
    rx.recv().unwrap()
}

/// The sink of a job of its own that is cancelled.
pub fn cancelled() -> ProgressSink {
    let reg = Registry::new();
    let (tx, rx) = std::sync::mpsc::channel();
    let (go, wait) = std::sync::mpsc::channel::<()>();
    let id = reg.submit(move |s| {
        tx.send(s).unwrap();
        let _ = wait.recv();
        Ok::<_, String>(vec![])
    });
    let sink = rx.recv().unwrap();
    reg.cancel(id);
    drop(go);
    sink
}
