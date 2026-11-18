use std::path::Path;
use std::fs;
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

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "IO error: {}", e),
            ArchiveError::InvalidPath(e) => write!(f, "Invalid path: {}", e),
            ArchiveError::UnsupportedFormat => write!(f, "Unsupported archive format"),
            ArchiveError::NoValidFiles => write!(f, "No valid files in archive"),
            ArchiveError::ZipError(e) => write!(f, "ZIP error: {}", e),
            ArchiveError::SevenZError(e) => write!(f, "7z error: {}", e),
        }
    }
}

impl std::error::Error for ArchiveError {}

pub fn is_safe_path(base: &Path, path: &Path) -> bool {
    // Try canonicalized comparison first (works when both paths exist).
    if let (Ok(normalized), Ok(base_canonical)) = (
        path.canonicalize(),
        base.canonicalize(),
    ) {
        return normalized.starts_with(&base_canonical);
    }

    // Fallback: normalize path separators and do a string prefix check.
    // On Windows, canonicalize() may prepend a \\?\ long-path prefix to
    // existing directories while the non-existent file path keeps its
    // plain form, so a starts_with() on the raw canonicalized values
    // would fail even for perfectly safe paths.
    let base_norm = base.to_string_lossy().replace('\\', "/");
    let path_norm = path.to_string_lossy().replace('\\', "/");
    if path_norm.starts_with(&base_norm) {
        return true;
    }

    // Reject any path that climbs above the base via .. components.
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            return false;
        }
    }
    true
}

pub fn extract_zip(archive_path: &Path, model_id: &str) -> Result<Vec<ExtractedFile>, ArchiveError> {
    let model_dir = get_model_dir(model_id);
    fs::create_dir_all(&model_dir)?;

    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let mut extracted_files = Vec::new();

    for i in 0..archive.len() {
        let mut file_entry = match archive.by_index(i) {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("[SLAG3D ZIP] Skipping entry {}: {}", i, e);
                continue;
            }
        };
        let file_name = file_entry.name().to_string();

        // Skip directories
        if file_entry.is_dir() || file_name.ends_with('/') || file_name.ends_with('\\') {
            continue;
        }

        // Only extract allowed extensions (.stl, .3mf, .obj)
        if !is_allowed_extension(&file_name) {
            continue;
        }

        // Flatten file structure into model_dir to avoid deep subfolder hierarchy path issues in public_path
        let base_filename = match Path::new(&file_name).file_name() {
            Some(name) => name,
            None => continue,
        };
        let dest_path = model_dir.join(base_filename);
        let safe_filename = base_filename.to_string_lossy().to_string();

        // Security: prevent path traversal
        if !is_safe_path(&model_dir, &dest_path) {
            continue;
        }

        let mut output_file = match fs::File::create(&dest_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[SLAG3D ZIP] Failed to create {}: {}", dest_path.display(), e);
                continue;
            }
        };
        if let Err(e) = std::io::copy(&mut file_entry, &mut output_file) {
            eprintln!("[SLAG3D ZIP] Failed to write {}: {}", dest_path.display(), e);
            continue;
        }

        let size = fs::metadata(&dest_path)?.len();
        extracted_files.push(ExtractedFile {
            filename: safe_filename,
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

            // Skip directories (some archives mark dirs explicitly, others end with /)
            if entry.is_directory() || file_name.ends_with('/') || file_name.ends_with('\\') {
                return Ok(true);
            }

            // Only extract allowed extensions (.stl, .3mf, .obj)
            if !is_allowed_extension(&file_name) {
                return Ok(false);
            }

            // Flatten file structure into model_dir to avoid deep subfolder hierarchy path issues in public_path
            let base_filename = match Path::new(&file_name).file_name() {
                Some(name) => name,
                None => return Ok(false),
            };
            let safe_filename = base_filename.to_string_lossy().to_string();
            let safe_dest = model_dir.join(&safe_filename);

            // Security: prevent path traversal
            if !is_safe_path(&model_dir, &safe_dest) {
                return Ok(false);
            }

            // Extract file data
            let mut file_data = Vec::new();
            if reader.read_to_end(&mut file_data).is_ok() {
                if let Err(e) = fs::write(&safe_dest, &file_data) {
                    eprintln!("[SLAG3D 7Z] Failed to write {}: {}", safe_dest.display(), e);
                    return Ok(true);
                }
                let size = file_data.len() as u64;
                filtered_extracted.push(ExtractedFile {
                    filename: safe_filename,
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
