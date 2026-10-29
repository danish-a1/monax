//! Read-only GUI queries answered at once (no job): form fields, page
//! geometry, page images, WordCraft presence. Errors are the core error's message.
use monax_core::Secret;
use monax_core::forms::Field;
use monax_core::pdfium::{self, PageInfo};
use std::path::PathBuf;
use tauri::ipc::Response;

/// The form fields of `path`, opened with `password` when given (a
/// secret), for the fill form panel; errors are the core error's message.
#[tauri::command]
pub async fn forms_list_fields(
    path: PathBuf,
    password: Option<Secret>,
) -> Result<Vec<Field>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        monax_core::forms::list_fields_opened(&path, password)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Page count and each page's boxes and /Rotate (PDF user space points) of
/// `path`, opened with `password` when given (a secret); errors are the
/// core error's message.
#[tauri::command]
pub async fn pages_info(path: PathBuf, password: Option<Secret>) -> Result<PageInfo, String> {
    tauri::async_runtime::spawn_blocking(move || pdfium::page_info(&path, password.as_ref()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Raw PNG bytes of one page as displayed, longest side `max_px`
/// (16..=4096: thumbnails and the viewer's device-pixel size); the document
/// opened with `password` when given (a secret).
#[tauri::command(rename_all = "snake_case")]
pub async fn pages_thumbnail(
    path: PathBuf,
    page: usize,
    max_px: u32,
    password: Option<Secret>,
) -> Result<Response, String> {
    let png = tauri::async_runtime::spawn_blocking(move || {
        pdfium::view_png(&path, page, max_px, password.as_ref())
    })
    .await
    .map_err(|e| e.to_string())?;
    png.map(Response::new).map_err(|e| e.to_string())
}

/// Whether the bundled WordCraft is present: the GUI greys out Office to PDF
/// without it.
#[tauri::command]
pub fn office_available() -> bool {
    monax_core::convert::office_available().is_some()
}
