use std::fs;
use tauri::{Manager, Runtime, UriSchemeContext};
use tauri::http::{Request, Response, StatusCode};
use mime_guess::from_path;
use crate::services::path_util::resolve_upload_relative_path;

pub fn register_asset_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol("slag3d-asset", |ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>| {
        handle_asset_request(&ctx, request)
    })
}

fn handle_asset_request<R: Runtime>(
    ctx: &UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let uri = request.uri().to_string();

    // Use the same app_data_dir as the DB and thumbnails (com.slag3d.app)
    let app_data_dir = match ctx.app_handle().path().app_data_dir() {
        Ok(p) => p,
        Err(_) => return not_found(),
    };
    let uploads_dir = app_data_dir.join("uploads");

    // Resolve the URI to a canonical path within uploads_dir using the shared
    // path_util. This handles all scheme variants, URL decoding, and canonical
    // containment checking in one place.
    let canonical_file = match resolve_upload_relative_path(&uri, &uploads_dir) {
        Ok(p) => p,
        Err(_) => return not_found(),
    };

    // Check if file exists
    if !canonical_file.is_file() {
        return not_found();
    }

    // Read file
    let content = match fs::read(&canonical_file) {
        Ok(c) => c,
        Err(_) => return not_found(),
    };

    // Determine MIME type
    let mime_type = from_path(&canonical_file)
        .first_or_octet_stream()
        .to_string();

    // Respond with file content
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime_type)
        .header("Content-Length", content.len().to_string())
        .body(content)
        .unwrap()
}

fn not_found() -> Response<Vec<u8>> {
    let body = b"Not Found".to_vec();
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/plain")
        .header("Content-Length", body.len().to_string())
        .body(body)
        .unwrap()
}