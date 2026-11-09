use crate::services::storage::is_allowed_extension;

#[test]
fn test_is_allowed_3d_extension() {
    assert!(is_allowed_extension("box.stl"));
    assert!(is_allowed_extension("component.3MF"));
    assert!(is_allowed_extension("statue.obj"));
    assert!(!is_allowed_extension("script.sh"));
    assert!(!is_allowed_extension("image.png"));
}
