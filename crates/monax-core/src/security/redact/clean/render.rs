//! R108: what renders a page - the only reach that keeps an object a
//! redacted page also reaches. Each dictionary is walked through the keys
//! its role draws with (an allow-list per role); private keys, /Usage,
//! /PieceInfo, actions and the like are never followed. Optional content
//! is found as pdfium finds it: /Properties values and /OC are groups when
//! typed /OCG or untyped and otherwise evaluated as OCMDs, whose /OCGs and /VE
//! members are groups whatever their type.
use super::obj;
use crate::content::qjson::Doc;
use serde_json::Value;
use std::collections::HashSet;

/// The role a value plays in drawing a page.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum K {
    Page,
    Res,
    /// A name -> object map of the given role.
    Map(&'static K),
    Font,
    Desc,
    XObj,
    Stream,
    Parms,
    Gs,
    SMask,
    Func,
    Cs,
    Pat,
    Sh,
    Grp,
    Annot,
    Ap,
    /// An appearance state: a form, or a state -> form map.
    ApState,
    Mk,
    Field,
    Oc,
    Member,
    /// Numbers, names, strings and arrays of them: a reference is followed
    /// only to one of those, never into a dictionary or a stream.
    Plain,
    /// A dictionary of plain data (/BS, /BE, /CIDSystemInfo): followed
    /// into a dictionary, never a stream, and walked through its plain keys.
    Data,
}

/// Keys whose values are plain data (see [`K::Plain`]).
const PLAIN: &str = "/Type /Subtype /Length /Filter /DL /Length1 /Length2 /Length3 /MediaBox /CropBox \
    /BleedBox /TrimBox /ArtBox /Rotate /UserUnit /ProcSet /Widths /FirstChar /LastChar \
    /FontBBox /FontMatrix /W /W2 /DW /DW2 /Registry /Ordering /Supplement /BaseFont /Name /Flags \
    /ItalicAngle /Ascent /Descent /CapHeight /StemV /StemH /XHeight /Leading /MaxWidth \
    /MissingWidth /AvgWidth /FontName /FontFamily /FontStretch /FontWeight /CharSet \
    /BBox /Matrix /Decode /Width /Height /BitsPerComponent /ImageMask /Interpolate \
    /Intent /SMaskInData /FormType /Matte /Predictor /Colors /Columns /EarlyChange /K \
    /EncodedByteAlign /Rows /EndOfBlock /BlackIs1 /ColorTransform /LW /LC /LJ /ML /D /RI \
    /OP /op /OPM /FL /SM /SA /BM /CA /ca /AIS /TK /BC /S /Domain /Range /Encode /C0 /C1 \
    /N /Bounds /Size /BitsPerSample /Order /FunctionType /HalftoneType /Frequency /Angle \
    /AccurateScreens /WhitePoint /BlackPoint /Gamma /Components /XStep /YStep /PaintType \
    /TilingType /PatternType /Background /AntiAlias /Coords /Extend /BitsPerCoordinate \
    /BitsPerFlag /VerticesPerRow /ShadingType /I /Rect /QuadPoints /Border /C /IC \
    /F /Vertices /InkList /L /LE /RD /Contents /DA /Q /DS /RC /T /NM /M /AS /CL /IT \
    /LL /LLE /LLO /Cap /CP /CO /Sy /Open /State /StateModel /FT /Ff /TU /TM /V /DV /Opt \
    /MaxLen /RV /TI /R /BG /AC /TP /IF /P /StructParents";

/// The role of `key`'s value in a dictionary of role `k`, if it draws.
fn next(k: K, key: &str) -> Option<K> {
    use K::*;
    const FONTS: K = Font;
    const XOBJS: K = XObj;
    const GSS: K = Gs;
    const CSS: K = Cs;
    const PATS: K = Pat;
    const SHS: K = Sh;
    const OCS: K = Oc;
    const STREAMS: K = Stream;
    let r = match (k, key) {
        (Map(m), _) => *m,
        (_, "/DecodeParms") => Parms,
        (Page, "/Resources") | (XObj | Font | Pat, "/Resources") => Res,
        (Page, "/Contents") => Stream,
        (Page, "/Annots") => Annot,
        (Page | XObj, "/Group") => Grp,
        (Res, "/Font") => Map(&FONTS),
        (Res, "/XObject") => Map(&XOBJS),
        (Res, "/ExtGState") => Map(&GSS),
        (Res, "/ColorSpace") | (Cs, "/Colorants") => Map(&CSS),
        (Res, "/Pattern") => Map(&PATS),
        (Res, "/Shading") => Map(&SHS),
        (Res, "/Properties") => Map(&OCS),
        (Font, "/FontDescriptor") => Desc,
        (Font, "/DescendantFonts") | (Gs, "/Font") => Font,
        (Font, "/ToUnicode" | "/CIDToGIDMap" | "/Encoding") | (Stream, "/UseCMap") => Stream,
        (Stream, "/Differences" | "/BaseEncoding") => Plain, // an /Encoding dict
        (Font, "/CharProcs") => Map(&STREAMS),
        (Desc, "/FontFile" | "/FontFile2" | "/FontFile3" | "/CIDSet") => Stream,
        (Parms, "/JBIG2Globals") => Stream,
        (XObj | Annot, "/OC") => Oc,
        (XObj | Grp | Sh, "/ColorSpace") | (Grp, "/CS") | (Cs, "/Alternate" | "/Process") => Cs,
        (XObj, "/SMask" | "/Mask") | (SMask, "/G") | (Mk, "/I" | "/RI" | "/IX") => XObj,
        (Gs, "/SMask") => SMask,
        (Gs, "/TR" | "/TR2" | "/BG" | "/BG2" | "/UCR" | "/UCR2" | "/HT") => Func,
        (SMask, "/TR") | (Sh, "/Function") | (Func | Cs, "/Functions") => Func,
        (Func, "/SpotFunction" | "/TransferFunction") => Func,
        (Pat, "/Shading") => Sh,
        (Pat, "/ExtGState") => Gs,
        (Annot, "/AP") => Ap,
        (Ap, "/N" | "/R" | "/D") => ApState,
        (Annot, "/MK") => Mk,
        (Annot | Field, "/Parent" | "/Kids") => Field,
        (Oc, "/OCGs" | "/VE") => Member,
        (Annot, "/BS" | "/BE") | (Font | Stream, "/CIDSystemInfo") => Data,
        (Oc | Member | ApState, _) => return None,
        (_, key) if PLAIN.split_ascii_whitespace().any(|p| p == key) => Plain,
        _ => return None,
    };
    Some(r)
}

/// Whether pdfium takes optional content `d` for a group: a dictionary
/// typed "OCG" (a name or a string, through references) or untyped (any
/// other is evaluated as an OCMD).
fn is_ocg(doc: &Doc, d: &Value) -> bool {
    d.as_object().is_some_and(|m| {
        m.get("/Type")
            .is_none_or(|t| super::ocg::text(doc, t) == Some("OCG"))
    })
}

/// What `roots` (values and their roles) draw with, outside `stop`; and
/// the groups among them (see the module docs).
pub(super) fn render(
    doc: &Doc,
    roots: Vec<(&Value, K)>,
    stop: &HashSet<String>,
) -> (HashSet<String>, HashSet<String>) {
    let (mut drawn, mut groups, mut seen) = (HashSet::new(), HashSet::new(), HashSet::new());
    let mut todo = roots;
    while let Some((v, k)) = todo.pop() {
        match v {
            Value::String(r) if r.ends_with(" R") => {
                let stream = doc
                    .objects
                    .get(&format!("obj:{r}"))
                    .is_some_and(|o| o["value"].is_null());
                let into_dict = k == K::Plain && obj(doc, r).is_object(); // R110
                let into_stream = k == K::Data && stream;
                if stop.contains(r) || into_dict || into_stream || !seen.insert((r.clone(), k)) {
                    continue;
                }
                drawn.insert(r.clone());
                let t = obj(doc, r);
                match k {
                    K::Member if t.is_object() => drop(groups.insert(r.clone())),
                    K::Oc if is_ocg(doc, t) => {
                        groups.insert(r.clone());
                    }
                    K::ApState if stream => todo.push((t, K::XObj)),
                    _ => todo.push((t, k)),
                }
            }
            Value::Array(a) => todo.extend(a.iter().map(|x| (x, k))),
            Value::Object(m) => {
                let k = if k == K::ApState { K::Map(&K::XObj) } else { k };
                if k == K::Oc && is_ocg(doc, v) {
                    continue; // a direct group: nothing to rewrite
                }
                for (key, x) in m {
                    if let Some(k2) = next(k, key) {
                        todo.push((x, k2));
                    }
                }
            }
            _ => {}
        }
    }
    (drawn, groups)
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
