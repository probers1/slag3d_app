use std::path::{Path, PathBuf};
use std::fs;

#[derive(Debug, Clone)]
pub struct ExtractedFile {
    pub filename: String,
    pub destination_path: PathBuf,
    pub size: u64,
}

pub fn is_3d_file(filename: &str) -> bool {
    matches!(
        Path::new(filename).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str(),
        "stl" | "3mf" | "obj"
    )
}

pub fn is_allowed_extension(filename: &str) -> bool {
    is_3d_file(filename)
        || Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .eq_ignore_ascii_case("pdf")
}

pub fn get_app_data_dir() -> PathBuf {
    // Allow runtime override (set by Tauri init from app.path().app_data_dir())
    if let Ok(dir) = std::env::var("SLAG3D_APP_DATA_DIR") {
        return PathBuf::from(dir);
    }
    // In Tauri, PathResolver is accessible via app.handle() in commands
    // For utility functions, use home directory as fallback for tests
    // This will be properly initialized in commands using PathResolver
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".").canonicalize().unwrap_or_else(|_| PathBuf::from(".")))
        .join("slag3d")
}

pub fn get_uploads_dir() -> PathBuf {
    get_app_data_dir().join("uploads")
}

pub fn get_models_dir() -> PathBuf {
    get_uploads_dir().join("models")
}

pub fn get_thumbnails_dir() -> PathBuf {
    get_uploads_dir().join("thumbnails")
}

pub fn get_tmp_dir() -> PathBuf {
    get_uploads_dir().join("tmp")
}

/// Scratch space for backup export/import. Deliberately outside `uploads/`:
/// during a restore the whole `uploads/` tree is swapped out, and nothing in
/// the work dir should ever be served by the `slag3d-asset://` protocol.
pub fn get_backup_work_dir() -> PathBuf {
    get_app_data_dir().join(".backup-work")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [
        get_uploads_dir(),
        get_models_dir(),
        get_thumbnails_dir(),
        get_tmp_dir(),
    ] {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

pub fn get_model_dir(model_id: &str) -> PathBuf {
    get_models_dir().join(model_id)
}

pub fn ensure_model_dir(model_id: &str) -> std::io::Result<PathBuf> {
    let dir = get_model_dir(model_id);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Generates a unique filename within `dir` to prevent name collisions
/// (e.g. if an archive contains files with the same name in different subdirectories).
pub fn get_unique_filename<P: AsRef<Path>>(dir: P, base_name: &str) -> (String, PathBuf) {
    let dir = dir.as_ref();
    let path = Path::new(base_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let mut candidate_name = if ext.is_empty() {
        stem.to_string()
    } else {
        format!("{}.{}", stem, ext)
    };
    let mut candidate_path = dir.join(&candidate_name);
    let mut counter = 1;

    while candidate_path.exists() {
        candidate_name = if ext.is_empty() {
            format!("{}_{}", stem, counter)
        } else {
            format!("{}_{}.{}", stem, counter, ext)
        };
        candidate_path = dir.join(&candidate_name);
        counter += 1;
    }

    (candidate_name, candidate_path)
}
