use std::fs;
use std::path::Path;
use tauri::{Manager, Runtime, UriSchemeContext};
use tauri::http::{Request, Response, StatusCode};
use mime_guess::from_path;

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

    // Security: ensure the path is within uploads directory.
    // The path was already validated by extract_path_from_uri to start
    // with "uploads/" and not contain ".." components. We do a string-
    // based containment check instead of canonicalize()+starts_with()
    // because on Windows canonicalize() prepends a \\?\ long-path prefix
    // to existing directories but not to non-existent file paths, which
    // breaks the prefix comparison.
    let requested_path = uploads_dir.join(&path);

    // Verify the resolved path stays within the uploads directory using
    // normalized string comparison (handles Windows \\?\ prefixes and
    // mixed separators).
    if !path_within_dir(&requested_path, &uploads_dir) {
        return not_found();
    }

    // Check if file exists
    if !requested_path.is_file() {
        return not_found();
    }

    // Read file
    let content = match fs::read(&requested_path) {
        Ok(c) => c,
        Err(_) => return not_found(),
    };

    // Determine MIME type
    let mime_type = from_path(&requested_path)
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

    // Strip query parameters (cache-busting like ?v=123)
    let path_only = path_part.split('?').next().unwrap_or(path_part);

    // URL decode the path
    let decoded = urlencoding::decode(path_only).ok()?;

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

/// Check that `path` is contained within `dir`, using normalized string
/// comparison instead of canonicalize()+starts_with(). On Windows,
/// canonicalize() prepends a \\?\ long-path prefix to existing directories
/// but not to non-existent file paths, which breaks the prefix comparison.
/// The path is already validated by extract_path_from_uri to start with
/// "uploads/" and not contain ".." components.
fn path_within_dir(path: &Path, dir: &Path) -> bool {
    let p = path.to_string_lossy();
    let d = dir.to_string_lossy();
    // Strip Windows \\?\ long-path prefix before normalizing separators,
    // otherwise the prefix becomes //?/ after the replace and won't match.
    let p = p.strip_prefix(r"\\?\").unwrap_or(&p);
    let d = d.strip_prefix(r"\\?\").unwrap_or(&d);
    p.replace('\\', "/").starts_with(&d.replace('\\', "/"))
}