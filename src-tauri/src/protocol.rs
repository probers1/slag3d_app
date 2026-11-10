use std::fs;
use tauri::{Manager, Runtime, UriSchemeContext};
use tauri::http::{Request, Response, StatusCode};
use mime_guess::from_path;

use crate::services::storage::get_uploads_dir;

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

    // Parse the URI to extract the path
    // Expected formats:
    // slag3d-asset://localhost/uploads/models/{id}/{file}
    // slag3d-asset://uploads/models/{id}/{file}
    // slag3d-asset://localhost/uploads/thumbnails/{file}
    // slag3d-asset://uploads/thumbnails/{file}

    let path = match extract_path_from_uri(&uri) {
        Some(p) => p,
        None => return not_found(),
    };

    // Use the same app_data_dir as the DB and thumbnails (com.slag3d.app)
    let app_data_dir = match ctx.app_handle().path().app_data_dir() {
        Ok(p) => p,
        Err(_) => return not_found(),
    };
    let uploads_dir = app_data_dir.join("uploads");

    // Security: ensure the path is within uploads directory
    let requested_path = uploads_dir.join(&path);

    // Canonicalize to prevent directory traversal
    let canonical_uploads = match fs::canonicalize(&uploads_dir) {
        Ok(p) => p,
        Err(_) => return not_found(),
    };

    let canonical_requested = match fs::canonicalize(&requested_path) {
        Ok(p) => p,
        Err(_) => return not_found(),
    };

    // Ensure the requested path is within uploads directory
    if !canonical_requested.starts_with(&canonical_uploads) {
        return not_found();
    }

    // Check if file exists
    if !canonical_requested.is_file() {
        return not_found();
    }

    // Read file
    let content = match fs::read(&canonical_requested) {
        Ok(c) => c,
        Err(_) => return not_found(),
    };

    // Determine MIME type
    let mime_type = from_path(&canonical_requested)
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

fn extract_path_from_uri(uri: &str) -> Option<String> {
    // Remove the scheme
    let without_scheme = uri.strip_prefix("slag3d-asset://")?;
    
    // Handle optional "localhost" prefix
    let path_part = without_scheme
        .strip_prefix("localhost/")
        .unwrap_or(without_scheme);
    
    // URL decode the path
    let decoded = urlencoding::decode(path_part).ok()?;
    
    // Ensure path starts with uploads/
    if !decoded.starts_with("uploads/") {
        return None;
    }
    
    // Return the relative path from uploads/
    Some(decoded["uploads/".len()..].to_string())
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