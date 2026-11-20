use std::path::Path;
use std::fs;

/// Resolve a user-supplied path (which may be a `slag3d-asset://` URI, a
/// Windows `http://slag3d-asset.localhost/` URI, or a bare `/uploads/...`
/// path) to a canonical `PathBuf` within the uploads directory.
///
/// Returns `Ok(canonical_path)` if the path is valid and contained within
/// `uploads_dir`. Returns `Err` if the path escapes the uploads tree, doesn't
/// exist, or fails to canonicalize.
///
/// This is the single source of truth for path resolution used by:
/// - `protocol.rs::handle_asset_request`
/// - `commands::models::read_file`
/// - `commands::models::save_downloaded_file`
///
/// Using one function prevents drift like the `http(s)://slag3d-asset.localhost/`
/// prefix gap that previously existed.
pub fn resolve_upload_relative_path(
    input_path: &str,
    uploads_dir: &Path,
) -> Result<std::path::PathBuf, String> {
    // Strip all known scheme prefixes. Order matters: strip the longer
    // Windows HTTP variants first so we don't leave a `localhost/` remnant
    // that would be incorrectly joined.
    let relative = input_path
        .strip_prefix("http://slag3d-asset.localhost/")
        .or_else(|| input_path.strip_prefix("https://slag3d-asset.localhost/"))
        .or_else(|| input_path.strip_prefix("slag3d-asset://"))
        .or_else(|| input_path.strip_prefix("slag3d-asset:"))
        .unwrap_or(input_path);

    // Strip the `localhost/` authority that Linux Tauri sometimes includes.
    let relative = relative.strip_prefix("localhost/").unwrap_or(relative);

    // Remove leading slash so we join relative to uploads_dir.
    let relative = relative.trim_start_matches('/');

    // Drop the leading `uploads/` so we join directly under uploads_dir.
    let relative = relative.strip_prefix("uploads/").unwrap_or(relative);

    // Strip query parameters and fragments (cache-busting like `?v=123`).
    // This must happen in the shared resolver: the custom asset protocol receives
    // `<img src>` URLs with cache-bust params appended, and read_file may too.
    // Do it before URL decoding so percent-encoded characters in the actual
    // filename are preserved.
    let path_only = relative
        .split(|c| c == '?' || c == '#')
        .next()
        .unwrap_or(relative);

    // URL decode the path
    let decoded = urlencoding::decode(path_only).map_err(|e| e.to_string())?;

    // Build the absolute path under uploads_dir.
    let requested_path = uploads_dir.join(decoded.as_ref());

    // Security: resolve to canonical form and verify containment.
    // A raw string starts_with() on uncanonicalized paths is trivially bypassed
    // ("uploads/../../vault.db" passes a prefix check against "uploads").
    // canonicalize() collapses ".." components so the containment check is real.
    //
    // On Windows, canonicalize() may prepend a \\?\ long-path prefix to the
    // existing uploads_dir but not to the (possibly non-existent) file path,
    // so compare using the normalized string form (see path_starts_with).
    let canonical_uploads = fs::canonicalize(uploads_dir).map_err(|e| e.to_string())?;
    let canonical_file = fs::canonicalize(&requested_path).map_err(|e| e.to_string())?;

    if !path_starts_with(&canonical_file, &canonical_uploads) {
        return Err("Access denied".to_string());
    }

    Ok(canonical_file)
}

/// Compare two paths for prefix containment, normalizing Windows \\?\ long-path
/// prefixes so a canonicalized existing directory and a plain file path
/// compare correctly.
fn path_starts_with(path: &Path, base: &Path) -> bool {
    let p = path.to_string_lossy();
    let p = p.strip_prefix(r"\\?\").unwrap_or(&p);
    let b = base.to_string_lossy();
    let b = b.strip_prefix(r"\\?\").unwrap_or(&b);
    p.replace('\\', "/").starts_with(&b.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_slag3d_asset_uri() {
        let base = PathBuf::from("/tmp/slag3d_path_test/uploads");
        fs::create_dir_all(&base).unwrap();
        let model_dir = base.join("models/abc123");
        fs::create_dir_all(&model_dir).unwrap();
        let file = model_dir.join("part.stl");
        fs::write(&file, b"test").unwrap();

        // Linux custom scheme
        let resolved = resolve_upload_relative_path(
            "slag3d-asset://localhost/uploads/models/abc123/part.stl",
            &base,
        ).unwrap();
        assert!(resolved.exists());
        assert_eq!(resolved, fs::canonicalize(&file).unwrap());

        // Windows HTTP scheme (no trailing slash variant)
        let resolved = resolve_upload_relative_path(
            "http://slag3d-asset.localhost/uploads/models/abc123/part.stl",
            &base,
        ).unwrap();
        assert!(resolved.exists());

        // Windows HTTPS scheme
        let resolved = resolve_upload_relative_path(
            "https://slag3d-asset.localhost/uploads/models/abc123/part.stl",
            &base,
        ).unwrap();
        assert!(resolved.exists());

        // Bare public path
        let resolved = resolve_upload_relative_path(
            "/uploads/models/abc123/part.stl",
            &base,
        ).unwrap();
        assert!(resolved.exists());

        // Relative public path
        let resolved = resolve_upload_relative_path(
            "uploads/models/abc123/part.stl",
            &base,
        ).unwrap();
        assert!(resolved.exists());

        let _ = fs::remove_dir_all("/tmp/slag3d_path_test");
    }

    #[test]
    fn test_resolve_strips_cache_busting_query() {
        let uploads_dir = PathBuf::from("/tmp/slag3d_path_test_query/uploads");
        let thumbnails_dir = uploads_dir.join("thumbnails");
        fs::create_dir_all(&thumbnails_dir).unwrap();
        let thumb = thumbnails_dir.join("abc.png");
        fs::write(&thumb, b"png").unwrap();

        let expected = fs::canonicalize(&thumb).unwrap();

        let resolved = resolve_upload_relative_path(
            "slag3d-asset://localhost/uploads/thumbnails/abc.png?v=1732100000000",
            &uploads_dir,
        ).unwrap();
        assert_eq!(resolved, expected);

        let resolved = resolve_upload_relative_path(
            "http://slag3d-asset.localhost/uploads/thumbnails/abc.png?v=2",
            &uploads_dir,
        ).unwrap();
        assert_eq!(resolved, expected);

        let _ = fs::remove_dir_all("/tmp/slag3d_path_test_query");
    }

    #[test]
    fn test_resolve_rejects_traversal() {
        let base = PathBuf::from("/tmp/slag3d_path_test2/uploads");
        fs::create_dir_all(&base).unwrap();

        // Direct traversal
        let err = resolve_upload_relative_path(
            "slag3d-asset://localhost/uploads/../../vault.db",
            &base,
        );
        assert!(err.is_err());

        // Prefix bypass (models_evil)
        let err = resolve_upload_relative_path(
            "uploads/models_evil/file.stl",
            &base,
        );
        assert!(err.is_err());

        // URL-encoded traversal
        let err = resolve_upload_relative_path(
            "uploads/%2e%2e/%2e%2e/vault.db",
            &base,
        );
        assert!(err.is_err());

        let _ = fs::remove_dir_all("/tmp/slag3d_path_test2");
    }

    #[test]
    fn test_resolve_allows_double_dot_filename() {
        // The `base` parameter to resolve_upload_relative_path is the uploads_dir,
        // not the model dir.
        let uploads_dir = PathBuf::from("/tmp/slag3d_path_test3/uploads");
        let model_dir = uploads_dir.join("models/abc123");
        fs::create_dir_all(&model_dir).unwrap();
        let file = model_dir.join("foo..bar.stl");
        fs::write(&file, b"test").unwrap();

        let resolved = resolve_upload_relative_path(
            "uploads/models/abc123/foo..bar.stl",
            &uploads_dir,
        ).unwrap();
        assert!(resolved.exists());

        let _ = fs::remove_dir_all("/tmp/slag3d_path_test3");
    }
}