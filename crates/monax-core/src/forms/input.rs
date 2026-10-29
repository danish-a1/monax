//! The serde inputs of [`super::list_fields`], [`super::fill`],
//! [`super::sign_image`] and [`super::sign_text`] (GUI commands and MCP tools).
use super::{FieldValue, Placement, SIGN_NOTE};
use crate::pages::{PagesError, input, write_bytes};
use crate::read::dest;
use crate::security::Unlocked;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Writes a .json listing a PDF's form fields: `{"fields": [{name, kind,
/// page, rect, value, options, read_only}, ...]}`. `kind` is text,
/// checkbox, radio, combo, list, button or signature; `page` is 0-based;
/// `rect` is `{left, bottom, right, top}` in PDF user space points (origin
/// bottom-left, y-up, unrotated MediaBox); `options` are the values a
/// combo, list, checkbox or radio accepts.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListFieldsInput {
    /// Absolute path of the PDF to inspect.
    pub input: PathBuf,
    /// Absolute output path of the `.json`; defaults beside the input.
    #[serde(default)]
    pub out: Option<PathBuf>,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed.
    #[serde(default)]
    pub password: Option<crate::Secret>,
}

impl ListFieldsInput {
    /// Writes `{"fields": [...]}` (see [`super::Field`]); returns the path
    /// written and the notes of opening the input.
    ///
    /// # Errors
    /// As [`super::list_fields`], plus `WrongPassword`.
    pub fn run(self) -> Result<(PathBuf, Vec<String>), PagesError> {
        let out = dest(&self.out, &self.input, "fields", "json");
        let ((path, mut notes), opened) =
            Unlocked::single(&self.input, self.password, Some(&out), |input| {
                let (fields, notes) = super::list_fields_noted(input)?;
                let json = serde_json::json!({ "fields": fields });
                let written = write_bytes::<PagesError>(&out, input, json.to_string().as_bytes())?;
                Ok((written, notes))
            })?;
        notes.extend(opened);
        Ok((path, notes))
    }
}

input!(/// Fills a PDF form's fields and writes a new PDF; each value is drawn
    /// into its field, so it shows in every viewer. Field names and
    /// allowed values are those `pdf_list_fields` reports. Fields not given
    /// keep their values. An XFA form keeps only its standard fields.
    FillFormInput { /// Absolute path of the PDF form.
        input: PathBuf, /// Values to set, one per field name.
        values: Vec<FieldValue> }
    "filled" -> notes, |s, out| &s.input => super::fill_with_notes(&s.input, &out, &s.values));
input!(/// Places a signature image (PNG or JPEG) on one page of a PDF and
    /// writes a new PDF: fitted and centred in `placement.rect`, aspect kept,
    /// PNG transparency kept, upright as the page is displayed. Note: this
    /// places a signature image or text; it is not a digital (cryptographic)
    /// signature.
    SignImageInput { /// Absolute path of the source PDF.
        input: PathBuf, /// Absolute path of the signature image: PNG or JPEG.
        image: PathBuf, /// Page, rect, turn and opacity of the signature.
        placement: Placement }
    "signed" -> notes, |s, out| &s.input => super::sign_image(&s.input, &out, &s.placement, &s.image)
        .map(|p| (p, vec![SIGN_NOTE.into()])));
input!(/// Places a line of signature text (Helvetica, WinAnsi characters) on
    /// one page of a PDF and writes a new PDF: centred in `placement.rect`,
    /// shrunk to fit (never below 4 pt), upright as the page is displayed.
    /// Note: this places a signature image or text; it is not a digital
    /// (cryptographic) signature.
    SignTextInput { /// Absolute path of the source PDF.
        input: PathBuf, /// The signature text, one line.
        text: String, /// Font size in points (> 0); shrunk if the text does not fit.
        font_size: f32, /// Page, rect, turn and opacity of the signature.
        placement: Placement }
    "signed" -> notes, |s, out| &s.input => super::sign_text(&s.input, &out, &s.placement, &s.text, s.font_size)
        .map(|p| (p, vec![SIGN_NOTE.into()])));
