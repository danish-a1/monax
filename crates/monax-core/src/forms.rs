//! AcroForm fields: list them and fill them; place signatures (`sign`). qpdf reads the field tree
//! (JSON) and writes new values; pdfium, through the field check (R113),
//! then draws the appearance of every filled field into the file, so each
//! value shows in every viewer.
mod fill;
mod input;
mod node;
mod set;
mod sign;
mod text;
mod tree;

pub use fill::{fill, fill_with_notes};
pub use input::{FillFormInput, ListFieldsInput, SignImageInput, SignTextInput};
pub use sign::{Placement, SIGN_NOTE, sign_image, sign_text};

use crate::content::qjson::Doc;
use crate::output::private_dir;
use crate::pages::{PagesError, Rect, open};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// What a field holds and how it is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Checkbox,
    Radio,
    Combo,
    List,
    /// A push button: holds no value.
    Button,
    /// A signature field: not fillable.
    Signature,
}

/// One terminal form field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Field {
    /// Fully qualified name (`parent.child`).
    pub name: String,
    pub kind: FieldKind,
    /// 0-based page of its first widget; null when no widget is on a page.
    pub page: Option<usize>,
    /// Its first widget's rectangle (PDF user space points, origin
    /// bottom-left, y-up, unrotated MediaBox); null with `page`.
    pub rect: Option<Rect>,
    /// Current value: text, the chosen option's export value, or for a
    /// checkbox or radio its export value or `Off`.
    pub value: String,
    /// Allowed values: options of a combo or list, export values of a
    /// checkbox or radio (`Off` is always allowed for those).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// Read-only fields cannot be filled.
    pub read_only: bool,
}

/// A value to set on the field named `name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldValue {
    /// Fully qualified field name, as listed.
    pub name: String,
    /// Text; a combo or list option; for a checkbox its export value,
    /// `Off`, `true` or `false`; for a radio an export value or `Off`.
    pub value: String,
}

/// Every terminal field of `input`'s form in field-tree order (empty
/// without a form). Rects are PDF user space points, origin bottom-left,
/// y-up, relative to the unrotated MediaBox. The input is not modified.
///
/// # Errors
/// `InvalidInput` (bad path), `EncryptedInput`, `Damaged`, `Qpdf`,
/// `Sidecar`, `Io`.
pub fn list_fields(input: &Path) -> Result<Vec<Field>, PagesError> {
    list_fields_noted(input).map(|(f, _)| f)
}

/// [`list_fields`] plus a note per multi-select list box: `Field::value`
/// holds only the first of its selected values.
pub(crate) fn list_fields_noted(input: &Path) -> Result<(Vec<Field>, Vec<String>), PagesError> {
    open(input)?;
    let dir = private_dir()?; // document data
    let doc = Doc::read(input, dir.path())?;
    let nodes = tree::nodes(&doc);
    let notes = (nodes.iter().filter(|n| n.selected > 1))
        .map(|n| {
            let (name, k) = (&n.field.name, n.selected);
            format!("field \"{name}\" has {k} selected values; only the first is shown")
        })
        .collect();
    Ok((nodes.into_iter().map(|n| n.field).collect(), notes))
}

/// [`list_fields`] of `input` opened with `password` (a secret, user or
/// owner) when it is encrypted.
///
/// # Errors
/// As [`list_fields`], plus `WrongPassword`.
pub fn list_fields_opened(
    input: &Path,
    password: Option<crate::Secret>,
) -> Result<Vec<Field>, PagesError> {
    crate::security::Unlocked::single(input, password, None, list_fields).map(|(f, _)| f)
}

#[cfg(test)]
mod tests;
