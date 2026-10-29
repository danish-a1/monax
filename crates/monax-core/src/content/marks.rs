//! Free text marks: header/footer and watermark.
use super::overlay::{Stamp, stamp};
use super::text::expand;
use super::{Anchor, PagesError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Left, center and right text of one header or footer row; empty = none.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Row {
    /// Left-aligned text.
    pub left: String,
    /// Centered text.
    pub center: String,
    /// Right-aligned text.
    pub right: String,
}

/// Options of [`header_footer`]. Sizes and margins in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct HfOpts {
    /// Text along the top edge.
    pub header: Row,
    /// Text along the bottom edge.
    pub footer: Row,
    /// Font size in points.
    pub size: f32,
    /// Points from the displayed page edges.
    pub margin: f32,
    /// 0-based pages to mark; `None` = all.
    pub pages: Option<Vec<usize>>,
}

impl Default for HfOpts {
    fn default() -> Self {
        Self {
            header: Row::default(),
            footer: Row::default(),
            size: 10.0,
            margin: 24.0,
            pages: None,
        }
    }
}

/// Options of [`watermark`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct WatermarkOpts {
    /// Watermark text; `%[page]` and `%[pages]` expand.
    pub text: String,
    /// 0.0 (invisible) ..= 1.0 (opaque).
    pub opacity: f32,
    /// Degrees counter-clockwise as displayed.
    pub angle: f32,
    /// Font size in points.
    pub size: f32,
    /// RGB fill color.
    pub color: [u8; 3],
    /// Anchor on the page as displayed.
    pub position: Anchor,
    /// Points from the displayed page edges.
    pub margin: f32,
    /// 0-based pages to mark; `None` = all.
    pub pages: Option<Vec<usize>>,
}

impl Default for WatermarkOpts {
    fn default() -> Self {
        Self {
            text: String::new(),
            opacity: 0.3,
            angle: 45.0,
            size: 48.0,
            color: [0, 0, 0],
            position: Anchor::Center,
            margin: 0.0,
            pages: None,
        }
    }
}

/// Stamps header and footer text onto the selected pages: `header` along
/// the top edge, `footer` along the bottom, each at left, center and right.
/// `%[page]` is the 1-based page number, `%[pages]` the page count.
///
/// # Errors
/// `InvalidInput` (bad paths, every slot empty, bad size or margin,
/// unsupported characters, text that does not fit), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Pdfium`, `Io`.
pub fn header_footer(input: &Path, out: &Path, opts: HfOpts) -> Result<PathBuf, PagesError> {
    use Anchor::*;
    let (h, f) = (&opts.header, &opts.footer);
    let slots = [
        (&h.left, TopLeft),
        (&h.center, TopCenter),
        (&h.right, TopRight),
        (&f.left, BottomLeft),
        (&f.center, BottomCenter),
        (&f.right, BottomRight),
    ];
    if slots.iter().all(|(t, _)| t.is_empty()) {
        return Err(PagesError::InvalidInput(
            "header and footer are all empty".into(),
        ));
    }
    stamp(input, out, opts.pages.as_deref(), |_, i, count| {
        let full = slots.iter().filter(|(t, _)| !t.is_empty());
        Ok(full
            .map(|(t, at)| {
                let t = expand(t, i as u64 + 1, count);
                Stamp::plain(t, *at, opts.margin, opts.size)
            })
            .collect())
    })
}

/// Stamps `text` onto the selected pages, drawn over the page content at
/// `opacity` (there is no underlay mode), rotated by `angle`.
/// `%[page]` is the 1-based page number, `%[pages]` the page count.
///
/// # Errors
/// As [`header_footer`]; `InvalidInput` for empty text, `opacity` outside
/// 0..=1 or a non-finite `angle`.
pub fn watermark(input: &Path, out: &Path, opts: WatermarkOpts) -> Result<PathBuf, PagesError> {
    let o = &opts;
    stamp(input, out, o.pages.as_deref(), |_, i, count| {
        Ok(vec![Stamp {
            text: expand(&o.text, i as u64 + 1, count),
            at: o.position,
            margin: o.margin,
            size: o.size,
            color: o.color,
            opacity: o.opacity,
            angle: o.angle,
        }])
    })
}
