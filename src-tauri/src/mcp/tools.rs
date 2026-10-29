//! MCP tools. Thin: deserialize a `monax-core` input, submit its `run` to the
//! shared registry, return `{ job_id }`. Schemas derive from the same structs.
//! Adding an op is one `job!` line in [`TOOLS`]; its description is the input
//! struct's doc comment (the schema's `description`).
use super::OpenHook;
use super::args::{JobArgs, NoArgs, OpenArgs};
use crate::commands::start;
use monax_core::annotate::AnnotateInput;
use monax_core::content::{
    BatesInput, CompressInput, DeleteAnnotationsInput, FlattenInput, HeaderFooterInput,
    LinearizeInput, PageNumbersInput, RepairInput, SetMetadataInput, WatermarkInput,
};
use monax_core::convert::{ImagesToPdfInput, OfficeToPdfInput, PdfToImagesInput};
use monax_core::forms::{FillFormInput, ListFieldsInput, SignImageInput, SignTextInput};
use monax_core::pages::{
    AlternateMixInput, CropInput, DeleteInput, ExtractInput, InsertBlankInput, MergeInput,
    NupInput, OrganizeInput, ReorderInput, RotateInput, SplitBookmarksInput, SplitHalfInput,
    SplitPagesInput, SplitSizeInput, SplitTextInput,
};
use monax_core::read::{PageInfoInput, ReadTextInput, RenderPageInput};
use monax_core::security::{
    DecryptInput, EncryptInput, InspectSecurityInput, RedactInput, UnlockInput,
};
use monax_core::{Registry, ocr::OcrInput, pdfium::ExtractImagesInput};
use rmcp::handler::server::common::schema_for_type;
use rmcp::model::JsonObject;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;

/// One rmcp service instance per request (stateless); clones share the registry.
#[derive(Clone)]
pub(super) struct Monax {
    pub reg: Registry,
    pub open: OpenHook,
}

pub(super) type Call = fn(&Monax, Value) -> Result<Value, String>;

pub(super) struct Spec {
    pub name: &'static str,
    pub about: fn() -> String,
    pub schema: fn() -> Arc<JsonObject>,
    pub call: Call,
}

fn parse<T: DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| format!("invalid arguments: {e}"))
}

fn to_json(v: impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

macro_rules! tool {
    ($name:literal, $about:expr, $args:ty, $call:expr) => {
        Spec {
            name: $name,
            about: || String::from($about),
            schema: schema_for_type::<$args>,
            call: $call,
        }
    };
}

const JOB_NOTE: &str = "Returns {job_id} at once; poll pdf_job_status until state.kind is \
    done (outputs lists the files written), failed (state.message is the reason) or cancelled.";

/// The input struct's doc comment (schema `description`) plus [`JOB_NOTE`].
fn describe<T: schemars::JsonSchema + std::any::Any>() -> String {
    let schema = schema_for_type::<T>();
    let doc = schema.get("description").and_then(Value::as_str);
    format!("{} {JOB_NOTE}", doc.unwrap_or_default().replace('\n', " "))
}

/// A tool that runs `<$input>::run` (or `$run`: input, job sink) as a job; returns `{ job_id }`.
macro_rules! job {
    ($name:literal, $input:ty) => {
        job!($name, $input, |input, _s| input.run())
    };
    ($name:literal, $input:ty, |$i:ident, $s:ident| $run:expr) => {
        Spec {
            name: $name,
            about: describe::<$input>,
            schema: schema_for_type::<$input>,
            call: |m, v| {
                let $i: $input = parse(v)?;
                to_json(start(&m.reg, move |$s| $run))
            },
        }
    };
}

pub(super) static TOOLS: &[Spec] = &[
    tool!(
        "pdf_capabilities",
        "Which sidecars are installed, with versions (null = missing): qpdf (page ops, \
        encryption, repair), tesseract (OCR), wordcraft (Office to PDF); ocr_languages: \
        the language codes pdf_ocr can read (empty without tesseract).",
        NoArgs,
        |_, v| parse::<NoArgs>(v).map(|_| capabilities())
    ),
    tool!(
        "pdf_job_status",
        "Snapshot of a job: {id, state: {kind, message?}, progress 0..1, notes?: [what the user \
        should know about the result, not errors], \
        outputs: [absolute paths]}. Jobs are kept 1h after they finish.",
        JobArgs,
        |m, v| {
            let a: JobArgs = parse(v)?;
            to_json(m.reg.status(a.job_id).ok_or("unknown job_id")?)
        }
    ),
    tool!(
        "pdf_job_cancel",
        "Requests cancellation of a running job; it ends cancelled with no outputs.",
        JobArgs,
        |m, v| {
            let a: JobArgs = parse(v)?;
            m.reg.status(a.job_id).ok_or("unknown job_id")?;
            m.reg.cancel(a.job_id);
            Ok(json!({ "cancel_requested": true }))
        }
    ),
    tool!(
        "pdf_open_in_app",
        "Shows a PDF in the Monax window so the user can see it.",
        OpenArgs,
        |m, v| {
            let a: OpenArgs = parse(v)?;
            monax_core::check_input_file(&a.path).map_err(|e| format!("invalid input: {e}"))?;
            (m.open)(&a.path);
            Ok(json!({ "opened": a.path }))
        }
    ),
    job!("pdf_page_info", PageInfoInput),
    job!("pdf_read_text", ReadTextInput),
    job!("pdf_render_page", RenderPageInput),
    job!("pdf_extract", ExtractInput),
    job!("pdf_delete", DeleteInput),
    job!("pdf_rotate", RotateInput),
    job!("pdf_reorder", ReorderInput),
    job!("pdf_organize", OrganizeInput),
    job!("pdf_merge", MergeInput),
    job!("pdf_alternate_mix", AlternateMixInput),
    job!("pdf_split_pages", SplitPagesInput),
    job!("pdf_split_size", SplitSizeInput),
    job!("pdf_split_text", SplitTextInput),
    job!("pdf_split_bookmarks", SplitBookmarksInput),
    job!("pdf_nup", NupInput),
    job!("pdf_crop", CropInput),
    job!("pdf_insert_blank", InsertBlankInput),
    job!("pdf_split_half", SplitHalfInput),
    job!("pdf_extract_images", ExtractImagesInput),
    job!("pdf_page_numbers", PageNumbersInput),
    job!("pdf_header_footer", HeaderFooterInput),
    job!("pdf_watermark", WatermarkInput),
    job!("pdf_bates", BatesInput),
    job!("pdf_compress", CompressInput),
    job!("pdf_set_metadata", SetMetadataInput),
    job!("pdf_flatten", FlattenInput),
    job!("pdf_delete_annotations", DeleteAnnotationsInput),
    job!("pdf_repair", RepairInput),
    job!("pdf_linearize", LinearizeInput),
    job!("pdf_encrypt", EncryptInput),
    job!("pdf_decrypt", DecryptInput),
    job!("pdf_unlock", UnlockInput),
    job!("pdf_inspect_security", InspectSecurityInput),
    job!("pdf_redact", RedactInput),
    job!("pdf_ocr", OcrInput, |input, s| input.run(s)),
    job!("pdf_list_fields", ListFieldsInput),
    job!("pdf_fill_forms", FillFormInput),
    job!("pdf_sign_image", SignImageInput),
    job!("pdf_sign_text", SignTextInput),
    job!("pdf_annotate", AnnotateInput),
    job!("pdf_images_to_pdf", ImagesToPdfInput, |input, s| input
        .run(s)),
    job!("pdf_to_images", PdfToImagesInput, |input, s| input.run(s)),
    job!("pdf_office_to_pdf", OfficeToPdfInput, |input, s| input
        .run(s)),
];

fn capabilities() -> Value {
    let c = monax_sidecar::Capabilities::detect();
    let v = |o: Option<monax_sidecar::Version>| o.map(|v| v.0);
    let langs = monax_core::ocr::languages();
    json!({ "qpdf": v(c.qpdf), "tesseract": v(c.tesseract), "wordcraft": v(c.wordcraft), "ocr_languages": langs })
}
