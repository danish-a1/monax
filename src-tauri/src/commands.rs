//! Thin Tauri wrappers over `monax-core`. No PDF logic lives here: op
//! commands submit the core call to the job registry and return its id.
use crate::mcp::{McpState, McpStatus};
use monax_core::annotate::AnnotateInput;
use monax_core::content::{
    BatesInput, CompressInput, DeleteAnnotationsInput, FlattenInput, HeaderFooterInput,
    LinearizeInput, PageNumbersInput, RepairInput, SetMetadataInput, WatermarkInput,
};
use monax_core::convert::{ImagesToPdfInput, OfficeToPdfInput, PdfToImagesInput};
use monax_core::forms::{FillFormInput, SignImageInput, SignTextInput};
use monax_core::ocr::OcrInput;
use monax_core::pages::{
    AlternateMixInput, CropInput, DeleteInput, ExtractInput, InsertBlankInput, MergeInput,
    NupInput, OrganizeInput, ReorderInput, RotateInput, SplitBookmarksInput, SplitHalfInput,
    SplitPagesInput, SplitSizeInput, SplitTextInput,
};
use monax_core::pdfium::ExtractImagesInput;
use monax_core::read::ReadTextInput;
use monax_core::security::{
    DecryptInput, EncryptInput, InspectSecurityInput, RedactInput, UnlockInput,
};
use monax_core::{Job, JobId, ProgressSink, Registry};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

mod query;
pub use query::*;

/// Result of every op command.
#[derive(Serialize)]
pub struct Started {
    job_id: JobId,
}

/// What an op's `run` returns: one written file, several, or a file plus
/// notes for the user (which go to the job's `notes`).
pub(crate) trait Outputs {
    fn publish(self, sink: &ProgressSink) -> Vec<PathBuf>;
}

impl Outputs for PathBuf {
    fn publish(self, _: &ProgressSink) -> Vec<PathBuf> {
        vec![self]
    }
}

impl Outputs for Vec<PathBuf> {
    fn publish(self, _: &ProgressSink) -> Vec<PathBuf> {
        self
    }
}

impl<T: Outputs> Outputs for (T, Vec<String>) {
    fn publish(self, sink: &ProgressSink) -> Vec<PathBuf> {
        for n in self.1 {
            sink.note(n);
        }
        self.0.publish(sink)
    }
}

/// Submits an op to the registry; `run` gets the job's sink (progress,
/// cancel). Shared with the MCP tools.
pub(crate) fn start<F, O, E>(reg: &Registry, run: F) -> Started
where
    F: FnOnce(&ProgressSink) -> Result<O, E> + Send + 'static,
    O: Outputs,
    E: std::fmt::Display,
{
    let job_id = reg.submit(move |sink| run(&sink).map(|o| o.publish(&sink)));
    Started { job_id }
}

/// One `#[tauri::command]` per op: takes the core input struct and submits
/// its `run` as a job.
macro_rules! op_commands {
    ($($cmd:ident: $input:ty),* $(,)?) => {$(
        #[tauri::command]
        pub fn $cmd(reg: State<Registry>, input: $input) -> Started {
            start(&reg, move |_| input.run())
        }
    )*};
}

op_commands! {
    pages_extract: ExtractInput,
    pages_delete: DeleteInput,
    pages_rotate: RotateInput,
    pages_reorder: ReorderInput,
    pages_merge: MergeInput,
    pages_organize: OrganizeInput,
    pages_alternate_mix: AlternateMixInput,
    pages_split_pages: SplitPagesInput,
    pages_split_size: SplitSizeInput,
    pages_split_text: SplitTextInput,
    pages_split_bookmarks: SplitBookmarksInput,
    pages_nup: NupInput,
    pages_crop: CropInput,
    pages_insert_blank: InsertBlankInput,
    pages_split_half: SplitHalfInput,
    pages_extract_images: ExtractImagesInput,
    content_page_numbers: PageNumbersInput,
    content_header_footer: HeaderFooterInput,
    content_watermark: WatermarkInput,
    content_bates: BatesInput,
    content_compress: CompressInput,
    content_set_metadata: SetMetadataInput,
    content_flatten: FlattenInput,
    content_delete_annotations: DeleteAnnotationsInput,
    content_repair: RepairInput,
    content_linearize: LinearizeInput,
    security_encrypt: EncryptInput,
    security_decrypt: DecryptInput,
    security_unlock: UnlockInput,
    security_inspect: InspectSecurityInput,
    security_redact: RedactInput,
    forms_fill: FillFormInput,
    forms_sign_image: SignImageInput,
    forms_sign_text: SignTextInput,
    annotate_save: AnnotateInput,
    read_text: ReadTextInput,
}

/// OCR: reports progress per page and cancels its tesseract run.
#[tauri::command]
pub fn ocr_run(reg: State<Registry>, input: OcrInput) -> Started {
    start(&reg, move |sink| input.run(sink))
}

/// PDF to images: reports progress per page and stops on cancel.
#[tauri::command]
pub fn convert_pdf_to_images(reg: State<Registry>, input: PdfToImagesInput) -> Started {
    start(&reg, move |sink| input.run(sink))
}

/// Images to PDF: reports progress per image and stops on cancel.
#[tauri::command]
pub fn convert_images_to_pdf(reg: State<Registry>, input: ImagesToPdfInput) -> Started {
    start(&reg, move |sink| input.run(sink))
}

/// Office to PDF with the bundled WordCraft: cancel kills it.
#[tauri::command]
pub fn convert_office_to_pdf(reg: State<Registry>, input: OfficeToPdfInput) -> Started {
    start(&reg, move |sink| input.run(sink))
}

/// Installed OCR language codes, for the language picker.
#[tauri::command]
pub fn ocr_languages() -> Vec<String> {
    monax_core::ocr::languages()
}

/// Snapshot of a job; `None` (null) for an unknown or pruned id.
#[tauri::command(rename_all = "snake_case")]
pub fn job_status(reg: State<Registry>, job_id: JobId) -> Option<Job> {
    reg.status(job_id)
}

/// MCP server status for the Settings panel (includes the token).
#[tauri::command]
pub fn mcp_info(state: State<McpState>) -> McpStatus {
    state.get()
}

#[tauri::command(rename_all = "snake_case")]
pub fn job_cancel(reg: State<Registry>, job_id: JobId) {
    reg.cancel(job_id);
}
