//! Drawn sizes of the images on every page, for downsampling to a dpi.
use super::{PdfiumError, count_of, page_of};
use pdfium_render::prelude::{PdfDocument, PdfMatrix, PdfPageObject, PdfPageObjectsCommon};
use std::collections::HashMap;

/// Image pixel size `(w, h)` -> the largest drawn `(width, height)` in points
/// along the image's own axes, over every use of an image of that pixel size
/// (page content and nested form XObjects; annotation appearances are not
/// scanned).
pub(crate) type Extents = HashMap<(u32, u32), (f32, f32)>;

/// Row-vector affine matrix `[a, b, c, d, e, f]`.
type M = [f32; 6];

fn of(m: PdfMatrix) -> M {
    [m.a(), m.b(), m.c(), m.d(), m.e(), m.f()]
}

/// `inner` (object in its form's space) followed by `outer` (form to page).
fn then(i: M, o: M) -> M {
    [
        i[0] * o[0] + i[1] * o[2],
        i[0] * o[1] + i[1] * o[3],
        i[2] * o[0] + i[3] * o[2],
        i[2] * o[1] + i[3] * o[3],
        i[4] * o[0] + i[5] * o[2] + o[4],
        i[4] * o[1] + i[5] * o[3] + o[5],
    ]
}

fn visit(obj: &PdfPageObject<'_>, outer: M, map: &mut Extents) {
    let Ok(m) = obj.matrix() else { return };
    let m = then(of(m), outer);
    if let Some(img) = obj.as_image_object() {
        // ponytail: an image pdfium cannot size is left as it is.
        let (Ok(w), Ok(h)) = (img.width(), img.height()) else {
            return;
        };
        let (Ok(w), Ok(h)) = (u32::try_from(w), u32::try_from(h)) else {
            return;
        };
        let drawn = (m[0].hypot(m[1]), m[2].hypot(m[3]));
        let e = map.entry((w, h)).or_insert((0.0, 0.0));
        *e = (e.0.max(drawn.0), e.1.max(drawn.1));
    } else if let Some(form) = obj.as_x_object_form_object() {
        for child in form.iter() {
            visit(&child, m, map);
        }
    }
}

/// [`Extents`] of every page of `doc`.
///
/// # Errors
/// `Damaged` when a page cannot be loaded.
pub(crate) fn image_extents(doc: &PdfDocument<'_>) -> Result<Extents, PdfiumError> {
    let mut map = Extents::new();
    for i in 0..count_of(doc) {
        let page = page_of(doc, i)?;
        for obj in page.objects().iter() {
            visit(&obj, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut map);
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::then;

    #[test]
    fn then_scales_then_translates() {
        // Unit image scaled to 100x50, inside a form scaled 2x and moved.
        let img = [100.0, 0.0, 0.0, 50.0, 10.0, 0.0];
        let form = [2.0, 0.0, 0.0, 2.0, 5.0, 7.0];
        assert_eq!(then(img, form), [200.0, 0.0, 0.0, 100.0, 25.0, 7.0]);
        // 90 degree form rotation keeps the image's axis lengths.
        let m = then(img, [0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
        assert_eq!((m[0].hypot(m[1]), m[2].hypot(m[3])), (100.0, 50.0));
    }
}
