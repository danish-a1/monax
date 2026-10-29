//! Annotation builders and qpdf readers of what `annotate::save` wrote.
use super::docs::{get, objects, qpdf};
use monax_core::annotate::{Annot, PageAnnot, Point, ShapeKind};
use monax_core::pages::Rect;
use serde_json::Value;
use std::path::Path;

pub fn rect(left: f32, bottom: f32, right: f32, top: f32) -> Rect {
    Rect {
        left,
        bottom,
        right,
        top,
    }
}

pub fn on(page: usize, annot: Annot) -> PageAnnot {
    PageAnnot { page, annot }
}

pub fn highlight(r: Rect) -> Annot {
    Annot::Highlight {
        rect: r,
        color: "#FFD700".into(),
    }
}

pub fn shape(kind: ShapeKind, r: Rect, width: f32) -> Annot {
    Annot::Shape {
        kind,
        rect: r,
        color: "#0000FF".into(),
        width,
    }
}

pub fn ink(strokes: &[&[(f32, f32)]], width: f32) -> Annot {
    let strokes = strokes
        .iter()
        .map(|s| s.iter().map(|&(x, y)| Point { x, y }).collect())
        .collect();
    Annot::Ink {
        strokes,
        width,
        color: "#FF0000".into(),
    }
}

pub fn free_text(r: Rect, text: &str, font_size: f32) -> Annot {
    Annot::FreeText {
        rect: r,
        text: text.into(),
        font_size,
        color: "#000000".into(),
    }
}

/// The annotation dictionaries on each page, in /Annots order.
pub fn dicts(p: &Path) -> Vec<Vec<Value>> {
    let objs = objects(p);
    let pages: Value = serde_json::from_str(&qpdf(&[
        "--json=2",
        "--json-key=pages",
        p.to_str().unwrap(),
    ]))
    .unwrap();
    let pages = pages["pages"].as_array().unwrap();
    pages
        .iter()
        .map(|pg| {
            let page = get(&objs, &pg["object"]);
            let list = get(&objs, &page["/Annots"]).as_array().cloned();
            let list = list.unwrap_or_default();
            list.iter().map(|a| get(&objs, a).clone()).collect()
        })
        .collect()
}

/// Whether `/AP /N` of `annot` (in `p`) references a form XObject stream.
pub fn has_normal_appearance(p: &Path, annot: &Value) -> bool {
    let objs = objects(p);
    let n = &get(&objs, &annot["/AP"])["/N"];
    let stream = n.as_str().and_then(|r| objs.get(&format!("obj:{r}")));
    stream.is_some_and(|s| s["stream"]["dict"]["/Subtype"] == "/Form")
}

/// The decoded data of the stream `/AP /N` of `annot` references.
pub fn appearance(p: &Path, annot: &Value) -> String {
    let n = annot["/AP"]["/N"].as_str().unwrap();
    let id = format!("--show-object={}", n.split(' ').next().unwrap());
    let path = p.to_str().unwrap();
    qpdf(&[&id, "--filtered-stream-data", path])
}

/// `annot`'s numeric array `key` as f32s.
pub fn nums(annot: &Value, key: &str) -> Vec<f32> {
    let a = annot[key].as_array().unwrap();
    a.iter().map(|v| v.as_f64().unwrap() as f32).collect()
}
