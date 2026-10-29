mod commands;
pub mod mcp;
use commands::*;

/// Registers state and every command; shared by `run()` and the e2e tests.
pub fn configure<R: tauri::Runtime>(b: tauri::Builder<R>) -> tauri::Builder<R> {
    b.manage(monax_core::Registry::new())
        .manage(mcp::McpState::default())
        .invoke_handler(tauri::generate_handler![
            pages_extract,
            pages_delete,
            pages_rotate,
            pages_reorder,
            pages_merge,
            pages_organize,
            pages_alternate_mix,
            pages_split_pages,
            pages_split_size,
            pages_split_text,
            pages_split_bookmarks,
            pages_nup,
            pages_crop,
            pages_insert_blank,
            pages_split_half,
            pages_extract_images,
            content_page_numbers,
            content_header_footer,
            content_watermark,
            content_bates,
            content_compress,
            content_set_metadata,
            content_flatten,
            content_delete_annotations,
            content_repair,
            content_linearize,
            security_encrypt,
            security_decrypt,
            security_unlock,
            security_inspect,
            security_redact,
            forms_fill,
            forms_sign_image,
            forms_sign_text,
            annotate_save,
            convert_images_to_pdf,
            convert_pdf_to_images,
            convert_office_to_pdf,
            office_available,
            read_text,
            forms_list_fields,
            ocr_run,
            ocr_languages,
            pages_info,
            pages_thumbnail,
            job_status,
            job_cancel,
            mcp_info,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            mcp::launch(app.handle(), mcp::default_dir());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
