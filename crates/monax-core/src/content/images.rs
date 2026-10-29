//! Image downsampling for [`super::compress`]: picks the image XObjects drawn
//! above the target dpi, decodes them with qpdf, shrinks and JPEG-encodes
//! them, and returns qpdf JSON updates that replace their stream data. The
//! page content streams are not touched.
//!
//! Only 8-bit gray or RGB images (Device, Cal or ICCBased) whose filters qpdf
//! can decode are resampled. Images left above the dpi are counted per
//! [`Skip`] reason and reported as notes; soft masks themselves are never
//! resampled (their parent may be, as a mask may differ in size, unless it
//! has /Matte, which requires matching sizes).
use super::PagesError;
use super::jpeg::{encode, shrink};
use super::qjson::Doc;
use crate::output::private_file;
use crate::pages::qpdf_out;
use crate::pdfium::Extents;
use serde_json::{Map, Value, json};
use skip::{Skip, notes};
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::path::Path;

/// Color channels of a re-encodable color space: 1 (gray) or 3 (RGB).
fn channels(doc: &Doc, cs: &Value) -> Option<usize> {
    let cs = doc.get(cs);
    let (family, icc) = match cs.as_array() {
        Some(a) => (a.first()?.as_str()?, a.get(1)),
        None => (cs.as_str()?, None),
    };
    match family {
        "/DeviceGray" | "/CalGray" => Some(1),
        "/DeviceRGB" | "/CalRGB" => Some(3),
        "/ICCBased" => match doc.get(icc?)["/N"].as_u64()? {
            1 => Some(1),
            3 => Some(3),
            _ => None,
        },
        _ => None,
    }
}

/// Pixel size for `drawn` points at `dpi`, at least 1 and never above `px`.
fn target(px: (u32, u32), drawn: (f32, f32), dpi: u32) -> (u32, u32) {
    let fit = |pts: f32, max: u32| ((pts / 72.0 * dpi as f32).ceil() as u32).clamp(1, max);
    (fit(drawn.0, px.0), fit(drawn.1, px.1))
}

/// Source size, target size and channels of an image to resample.
struct Plan {
    from: (u32, u32),
    to: (u32, u32),
    n: usize,
}

/// `Ok(None)` when the image is drawn at or below `dpi` (or unreadable).
fn plan(doc: &Doc, d: &Map<String, Value>, ext: &Extents, dpi: u32) -> Result<Option<Plan>, Skip> {
    let num = |k: &str| u32::try_from(d.get(k)?.as_u64()?).ok();
    let (Some(w), Some(h)) = (num("/Width"), num("/Height")) else {
        return Ok(None);
    };
    let drawn = *ext.get(&(w, h)).ok_or(Skip::NotDrawn)?;
    let to = target((w, h), drawn, dpi);
    if to == (w, h) {
        return Ok(None);
    }
    let stencil = d.get("/ImageMask") == Some(&Value::Bool(true));
    if stencil || num("/BitsPerComponent") != Some(8) {
        return Err(Skip::BitDepth);
    }
    let matte = d
        .get("/SMask")
        .is_some_and(|m| doc.get(m).get("/Matte").is_some());
    if matte || d.contains_key("/Mask") {
        return Err(Skip::Masked);
    }
    if d.contains_key("/Decode") {
        return Err(Skip::Decode);
    }
    let n = d
        .get("/ColorSpace")
        .and_then(|cs| channels(doc, cs))
        .ok_or(Skip::ColorSpace)?;
    Ok(Some(Plan {
        from: (w, h),
        to,
        n,
    }))
}

/// The update replacing image `key` ("obj:N G R") with its resampled JPEG.
fn resample(
    doc: &Doc,
    work: &Path,
    key: &str,
    d: &Map<String, Value>,
    p: Plan,
    quality: u8,
    dir: &Path,
) -> Result<Result<Value, Skip>, PagesError> {
    let id = key.split([':', ' ']).nth(1).unwrap_or_default();
    let mut prefix = OsString::from("--json-stream-prefix=");
    prefix.push(dir.join("raw"));
    let args: [OsString; 7] = [
        "--json=2".into(),
        "--json-key=qpdf".into(),
        format!("--json-object={id}").into(),
        "--json-stream-data=file".into(),
        prefix,
        "--decode-level=all".into(),
        work.into(),
    ];
    let v: Value = serde_json::from_str(&qpdf_out(&args)?)
        .map_err(|e| PagesError::Qpdf(format!("unreadable qpdf JSON: {e}")))?;
    let s = &v["qpdf"][1][key]["stream"];
    let Some(file) = s["datafile"].as_str() else {
        return Ok(Err(Skip::Filter));
    };
    let raw = std::fs::read(file)?;
    std::fs::remove_file(file)?;
    let (w, h) = (p.from.0 as usize, p.from.1 as usize);
    if s["dict"].get("/Filter").is_some() || raw.len() != w * h * p.n {
        return Ok(Err(Skip::Filter));
    }
    let px = shrink(&raw, p.from, p.n, p.to);
    // `to` is below `from`, so it fits JPEG's limit whenever encoding can work.
    let Some(jpg) = encode(&px, p.to, p.n, quality) else {
        return Ok(Err(Skip::Filter));
    };
    // An unreadable /Length (direct or indirect) means "replace".
    let old = d.get("/Length").and_then(|l| doc.get(l).as_u64());
    if old.is_some_and(|old| jpg.len() as u64 >= old) {
        return Ok(Err(Skip::NotSmaller));
    }
    let path = dir.join(format!("img-{id}.jpg"));
    private_file(&path)?;
    std::fs::write(&path, jpg)?;
    let mut d = d.clone();
    for k in ["/Filter", "/DecodeParms", "/Length"] {
        d.remove(k);
    }
    d.insert("/Width".into(), p.to.0.into());
    d.insert("/Height".into(), p.to.1.into());
    d.insert("/Filter".into(), "/DCTDecode".into());
    Ok(Ok(json!({ "stream": { "dict": d, "datafile": path } })))
}

/// qpdf JSON updates resampling every image of `doc` (read from `work`)
/// drawn above `dpi` (see the module docs) to JPEG at `quality`, and the
/// notes for the images left above it; scratch files go to `dir`.
///
/// # Errors
/// `Qpdf`, `Sidecar`, `Io`.
pub(super) fn downsample(
    doc: &Doc,
    work: &Path,
    ext: &Extents,
    dpi: u32,
    quality: u8,
    dir: &Path,
) -> Result<(Map<String, Value>, Vec<String>), PagesError> {
    let images: Vec<(&String, &Map<String, Value>)> = (doc.objects.iter())
        .filter_map(|(k, o)| Some((k, o["stream"]["dict"].as_object()?)))
        .filter(|(_, d)| d.get("/Subtype") == Some(&json!("/Image")))
        .collect();
    let masks: HashSet<&str> = (images.iter())
        .flat_map(|(_, d)| ["/SMask", "/Mask"].map(|k| d.get(k).and_then(Value::as_str)))
        .flatten()
        .collect();
    let (mut updates, mut skipped) = (Map::new(), BTreeMap::new());
    for (key, d) in images {
        if masks.contains(key.trim_start_matches("obj:")) {
            continue;
        }
        let done = match plan(doc, d, ext, dpi) {
            Ok(None) => continue,
            Ok(Some(p)) => resample(doc, work, key, d, p, quality, dir)?,
            Err(s) => Err(s),
        };
        match done {
            Ok(u) => drop(updates.insert(key.clone(), u)),
            Err(s) => *skipped.entry(s).or_insert(0) += 1,
        }
    }
    Ok((updates, notes(&skipped, dpi)))
}

mod skip;
#[cfg(test)]
mod tests;
