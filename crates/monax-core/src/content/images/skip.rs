//! Reasons images stay above the dpi, and the notes that count them.
use std::collections::BTreeMap;

/// Why an image drawn above the dpi was left as it is (note order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Skip {
    ColorSpace,
    BitDepth,
    Decode,
    Masked,
    Filter,
    NotSmaller,
    /// Not drawn in page content, so its dpi is unknown.
    NotDrawn,
}

impl Skip {
    fn why(self) -> &'static str {
        match self {
            Self::ColorSpace => "color space not gray or RGB",
            Self::BitDepth => "not 8 bits per component",
            Self::Decode => "has a /Decode array",
            Self::Masked => "soft mask with /Matte or color-key mask",
            Self::Filter => "compression qpdf cannot decode (JPX, JBIG2, CCITT)",
            Self::NotSmaller => "a resampled JPEG would not be smaller",
            Self::NotDrawn => "only drawn outside page content (patterns, annotations) or unused",
        }
    }
}

/// One note per reason: "2 images left above 72 dpi: <why>".
pub(super) fn notes(skipped: &BTreeMap<Skip, usize>, dpi: u32) -> Vec<String> {
    let plural = |n: usize| if n == 1 { "image" } else { "images" };
    (skipped.iter())
        .map(|(s, &n)| match s {
            Skip::NotDrawn => format!(
                "{n} {} not checked against {dpi} dpi: {}",
                plural(n),
                s.why()
            ),
            _ => format!("{n} {} left above {dpi} dpi: {}", plural(n), s.why()),
        })
        .collect()
}
