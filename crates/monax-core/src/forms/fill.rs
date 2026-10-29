//! fill (R131): qpdf writes the new values, pdfium draws them, and the
//! output is checked to hold an appearance for every filled widget.
use super::FieldValue;
use super::node::Node;
use super::set::Plan;
use super::tree::nodes;
use crate::content::qjson::Doc;
use crate::output::{check_out, private_dir, private_file, write_output};
use crate::pages::{PagesError, open, qpdf_op};
use crate::pdfium::{fields, page_of, render_page, save, with_checked};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Note of a fill whose input had XFA form data (R131).
pub(crate) const XFA_NOTE: &str =
    "XFA form data was removed; the form now uses its standard fields";

/// [`fill_with_notes`] without the notes.
///
/// # Errors
/// As [`fill_with_notes`].
pub fn fill(input: &Path, out: &Path, values: &[FieldValue]) -> Result<PathBuf, PagesError> {
    fill_with_notes(input, out, values).map(|(p, _)| p)
}

/// Writes `input` to `out` with each named field set to its value (see
/// [`FieldValue`]) and drawn: text and choice fields get a new appearance
/// from pdfium, checkboxes and radios show the chosen state. Fields not
/// named keep their values. An XFA form's /XFA is dropped, so its standard
/// fields are what every reader shows (the note says so). Returns the path
/// written and the notes.
///
/// # Errors
/// `InvalidInput` (bad paths, no values, a name given twice, an unknown
/// name with the valid ones, a value the field does not accept and why),
/// `FieldNotDrawn` (pdfium drew no appearance for a value; nothing is
/// written), `EncryptedInput`, `Damaged`, `Pdfium` (`DamagedRepairFirst`:
/// qpdf warns or fails reading it; the field check), `Qpdf`, `Sidecar`, `Io`.
pub fn fill_with_notes(
    input: &Path,
    out: &Path,
    values: &[FieldValue],
) -> Result<(PathBuf, Vec<String>), PagesError> {
    open(input)?;
    check_out(out, &[input])?;
    if values.is_empty() {
        return Err(PagesError::InvalidInput("no field values given".into()));
    }
    let dir = private_dir()?; // document data
    // A file qpdf must repair to read is refused: the output would hold
    // qpdf's repair, which may differ from what the input shows.
    let doc = Doc::read_unrepaired(input, dir.path())?;
    let all = nodes(&doc);
    let mut plan = Plan::new(&doc);
    let mut given = HashSet::new();
    for v in values {
        if !given.insert(v.name.as_str()) {
            let why = format!("field {:?} is given twice", v.name);
            return Err(PagesError::InvalidInput(why));
        }
        let hits: Vec<&Node> = all.iter().filter(|n| n.field.name == v.name).collect();
        if hits.is_empty() {
            return Err(unknown(&v.name, &all));
        }
        for n in hits {
            plan.set(n, &v.value)?;
        }
    }
    let notes: Vec<String> = match plan.drop_xfa() {
        true => vec![XFA_NOTE.into()],
        false => Vec::new(),
    };
    let pages = std::mem::take(&mut plan.pages);
    let staged = dir.path().join("staged.pdf");
    private_file(&staged)?;
    let update = doc.update_arg(plan.updates(), dir.path())?;
    qpdf_op(&[input.into(), update, staged.clone().into()])?;
    let ok = fields::check(&staged, None)?;
    // Drawing a page has pdfium write the appearance of every widget on
    // it that has none (the filled ones) into the document.
    let bytes = with_checked(&ok, None, |_, d| {
        for &i in &pages {
            render_page(&page_of(d, i)?, 1, 1, &ok)?;
        }
        save(d)
    })?;
    let path = write_output(out, &[input], |tmp| {
        std::fs::write(tmp, &bytes)?;
        drawn(&Doc::read(tmp, dir.path())?, &given)
    })?;
    Ok((path, notes))
}

/// `InvalidInput` naming `name` and the valid names (the first 50).
pub(super) fn unknown(name: &str, all: &[Node]) -> PagesError {
    let names: Vec<&str> = all.iter().map(|n| n.field.name.as_str()).collect();
    let valid = match names.len() {
        0 => "the document has no form fields".to_string(),
        n if n > 50 => format!(
            "valid fields: {}, ... and {} more",
            names[..50].join(", "),
            n - 50
        ),
        _ => format!("valid fields: {}", names.join(", ")),
    };
    PagesError::InvalidInput(format!("unknown field {name:?}; {valid}"))
}

/// Fails unless every widget on a page of the fields `names` in `doc` has
/// a normal appearance, holding its /AS state for a checkbox or radio.
pub(super) fn drawn(doc: &Doc, names: &HashSet<&str>) -> Result<(), PagesError> {
    for n in nodes(doc)
        .iter()
        .filter(|n| names.contains(n.field.name.as_str()))
    {
        for w in n.widgets.iter().filter(|w| w.page.is_some()) {
            let r = Value::from(w.r.as_str());
            let d = doc.get(&r);
            let at = &doc.get(&d["/AP"])["/N"];
            let normal = doc.get(at);
            let ok = match d["/AS"].as_str() {
                Some(s) if s != "/Off" => !doc.is_stream(at) && normal.get(s).is_some(),
                _ => !normal.is_null(),
            };
            if !ok {
                return Err(PagesError::FieldNotDrawn(n.field.name.clone()));
            }
        }
    }
    Ok(())
}
