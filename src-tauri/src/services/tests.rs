use crate::services::storage::{is_3d_file, is_allowed_extension};
use crate::services::archive::is_safe_path;
use std::path::PathBuf;

#[test]
fn test_is_allowed_3d_extension() {
    assert!(is_allowed_extension("box.stl"));
    assert!(is_allowed_extension("component.3MF"));
    assert!(is_allowed_extension("statue.obj"));
    assert!(!is_allowed_extension("script.sh"));
    assert!(!is_allowed_extension("image.png"));
}

#[test]
fn test_is_allowed_extension_case_insensitive() {
    assert!(is_allowed_extension("UPPER.STL"));
    assert!(is_allowed_extension("Mixed.3MF"));
    assert!(is_allowed_extension("lower.obj"));
    assert!(!is_allowed_extension("UPPER.PNG"));
}

/// Regression tests for the path-traversal fix in is_safe_path.
/// A raw string starts_with() on uncanonicalized paths is trivially bypassed
/// ("models_evil/file.stl" matches a "models" prefix, and
/// "models/../../vault.db" matches too), so containment must be checked on
/// canonical forms after rejecting ".." components.
#[test]
fn test_pdf_is_allowed_but_not_a_3d_file() {
    assert!(is_allowed_extension("manual.pdf"));
    assert!(is_allowed_extension("MANUAL.PDF"));
    assert!(!is_3d_file("manual.pdf"));
    assert!(is_3d_file("part.STL"));
}

#[test]
fn test_is_safe_path_rejects_dotdot_components() {
    let base = PathBuf::from("/tmp/slag3d/uploads/models/abc123");
    std::fs::create_dir_all(&base).unwrap();
    let _ = std::fs::create_dir_all("/tmp/slag3d/uploads/models");

    // Direct traversal: climbs above the model dir.
    assert!(!is_safe_path(&base, &PathBuf::from("/tmp/slag3d/uploads/models/abc123/../../../vault.db")));

    // Traversal hidden after an allowed extension: "models_evil/file.stl"
    // must NOT match a "models" prefix.
    assert!(!is_safe_path(&base, &PathBuf::from("/tmp/slag3d/uploads/models_evil/file.stl")));

    // A legitimate file inside the model dir is allowed.
    assert!(is_safe_path(&base, &base.join("part.stl")));

    let _ = std::fs::remove_dir_all("/tmp/slag3d");
}

#[test]
fn test_is_safe_path_allows_legitimate_file_in_subdir() {
    let base = PathBuf::from("/tmp/slag3d2/uploads/models/abc123");
    std::fs::create_dir_all(&base).unwrap();

    assert!(is_safe_path(&base, &base.join("sub/part.stl")));

    let _ = std::fs::remove_dir_all("/tmp/slag3d2");
}

/// Regression: the ".." check must be component-by-component, not a substring
/// match, otherwise a legitimate file like "foo..bar.stl" gets rejected.
#[test]
fn test_is_safe_path_allows_double_dot_filename() {
    let base = PathBuf::from("/tmp/slag3d3/uploads/models/abc123");
    std::fs::create_dir_all(&base).unwrap();

    assert!(is_safe_path(&base, &base.join("foo..bar.stl")));

    let _ = std::fs::remove_dir_all("/tmp/slag3d3");
}
