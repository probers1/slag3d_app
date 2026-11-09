use std::path::{Path, PathBuf};
use std::fs;

#[derive(Debug, Clone)]
pub struct ExtractedFile {
    pub filename: String,
    pub destination_path: PathBuf,
    pub size: u64,
}

pub fn is_allowed_extension(filename: &str) -> bool {
    let allowed = ["stl", "3mf", "obj"];
    if let Some(ext) = Path::new(filename).extension().and_then(|e| e.to_str()) {
        allowed.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

pub fn get_app_data_dir() -> PathBuf {
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
