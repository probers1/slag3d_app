use std::path::{Path, PathBuf};
use std::fs;
use std::io::Read;
use crate::services::storage::{is_allowed_extension, ExtractedFile, get_model_dir};

#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    InvalidPath(String),
    UnsupportedFormat,
    NoValidFiles,
    ZipError(zip::result::ZipError),
    SevenZError(sevenz_rust::Error),
}

impl From<std::io::Error> for ArchiveError {
    fn from(err: std::io::Error) -> Self {
        ArchiveError::Io(err)
    }
}

impl From<zip::result::ZipError> for ArchiveError {
    fn from(err: zip::result::ZipError) -> Self {
        ArchiveError::ZipError(err)
    }
}

impl From<sevenz_rust::Error> for ArchiveError {
    fn from(err: sevenz_rust::Error) -> Self {
        ArchiveError::SevenZError(err)
    }
}

pub fn is_safe_path(base: &Path, path: &Path) -> bool {
    let normalized = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Ok(base_canonical) = base.canonicalize() {
        normalized.starts_with(&base_canonical)
    } else {
        // Base doesn't exist yet, check relative path
        if path.is_absolute() {
            return false;
        }
        let mut current = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => return false,
                std::path::Component::CurDir => continue,
                _ => current.push(component),
            }
        }
        !current.to_string_lossy().contains("..")
    }
}

pub fn extract_zip(archive_path: &Path, model_id: &str) -> Result<Vec<ExtractedFile>, ArchiveError> {
    let model_dir = get_model_dir(model_id);
    fs::create_dir_all(&model_dir)?;

    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    
    let mut extracted_files = Vec::new();
    
    for i in 0..archive.len() {
        let mut file_entry = archive.by_index(i)?;
        let file_name = file_entry.name().to_string();
        
        // Skip directories
        if file_entry.is_dir() {
            continue;
        }
        
        // Security: prevent path traversal
        let dest_path = model_dir.join(&file_name);
        if !is_safe_path(&model_dir, &dest_path) {
            continue;
        }
        
        // Only extract allowed extensions
        if !is_allowed_extension(&file_name) {
            continue;
        }
        
        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let mut output_file = fs::File::create(&dest_path)?;
        std::io::copy(&mut file_entry, &mut output_file)?;
        
        let size = fs::metadata(&dest_path)?.len();
        extracted_files.push(ExtractedFile {
            filename: PathBuf::from(&file_name).file_name().unwrap().to_string_lossy().to_string(),
            destination_path: dest_path,
            size,
        });
    }
    
    if extracted_files.is_empty() {
        return Err(ArchiveError::NoValidFiles);
    }
    
    Ok(extracted_files)
}

pub fn extract_7z(archive_path: &Path, model_id: &str) -> Result<Vec<ExtractedFile>, ArchiveError> {
    let model_dir = get_model_dir(model_id);
    fs::create_dir_all(&model_dir)?;
    
    let file = fs::File::open(archive_path)?;
    
    let mut filtered_extracted: Vec<ExtractedFile> = Vec::new();
    
    sevenz_rust::decompress_with_extract_fn(
        file,
        &model_dir,
        |entry, reader, _dest_path| {
            let file_name = entry.name().to_string();
            
            // Skip directories
            if entry.is_directory() {
                return Ok(true);
            }
            
            // Only extract allowed extensions
            if !is_allowed_extension(&file_name) {
                return Ok(false);
            }
            
            // Security: prevent path traversal
            let safe_dest = model_dir.join(file_name.trim_start_matches('/'));
            if !is_safe_path(&model_dir, &safe_dest) {
                return Ok(false);
            }
            
            // Create parent directories
            if let Some(parent) = safe_dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            
            // Extract file data
            let mut file_data = Vec::new();
            if reader.read_to_end(&mut file_data).is_ok() {
                let _ = fs::write(&safe_dest, &file_data);
                let size = file_data.len() as u64;
                filtered_extracted.push(ExtractedFile {
                    filename: PathBuf::from(&file_name).file_name().unwrap().to_string_lossy().to_string(),
                    destination_path: safe_dest.clone(),
                    size,
                });
            }
            
            Ok(true)
        }
    )?;
    
    if filtered_extracted.is_empty() {
        return Err(ArchiveError::NoValidFiles);
    }
    
    Ok(filtered_extracted)
}
