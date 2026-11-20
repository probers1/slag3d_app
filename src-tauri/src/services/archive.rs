use std::path::Path;
use std::fs;
use crate::services::storage::{is_allowed_extension, ExtractedFile, get_model_dir, get_unique_filename};

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
    // Reject any path that climbs above the base via .. components. Do this
    // FIRST, before any string-prefix check: a raw starts_with() on
    // uncanonicalized paths is trivially bypassed ("models_evil/file.stl"
    // matches a "models" prefix, and "models/../../vault.db" matches too).
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            return false;
        }
    }

    // Canonicalize the base (it always exists — the model dir is created
    // before extraction). For the target path, canonicalize what we can:
    // if the file already exists, canonicalize it directly; otherwise
    // canonicalize the nearest existing ancestor and re-join the remainder.
    let base_canonical = match base.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let path_canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Path doesn't exist yet (we're about to create it). Walk up to
            // the nearest existing ancestor, canonicalize that, then append
            // the remaining components.
            let mut ancestor = path.to_path_buf();
            let mut tail = Vec::new();
            while !ancestor.exists() {
                match ancestor.file_name() {
                    Some(name) => {
                        tail.push(name.to_os_string());
                        ancestor.pop();
                    }
                    None => return false,
                }
            }
            let mut canonical = ancestor.canonicalize().unwrap_or_else(|_| base_canonical.clone());
            for component in tail.iter().rev() {
                canonical.push(component);
            }
            canonical
        }
    };

    // Containment check on canonical forms (handles Windows \\?\ prefixes and
    // mixed separators via the helper below).
    path_starts_with(&path_canonical, &base_canonical)
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

        // Reject path traversal outright. Entry names are attacker-controlled;
        // checking this before file_name() extraction prevents an entry like
        // "evil/../payload.stl" from passing is_allowed_extension on its
        // final component and then being flattened into the model dir.
        // Check component-by-component (not substring) so a legitimate file
        // like "foo..bar.stl" isn't rejected.
        if file_name.split('/').any(|c| c == "..") || file_name.split('\\').any(|c| c == "..") {
            continue;
        }

        // Only extract allowed extensions (.stl, .3mf, .obj, .pdf)
        if !is_allowed_extension(&file_name) {
            continue;
        }

        // Flatten file structure into model_dir to avoid deep subfolder hierarchy path issues in public_path.
        // If files from different subfolders share the same filename, resolve to a unique name.
        let base_filename = match Path::new(&file_name).file_name() {
            Some(name) => name,
            None => continue,
        };
        let (safe_filename, dest_path) = get_unique_filename(&model_dir, &base_filename.to_string_lossy());

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

            // Reject path traversal outright. Entry names are attacker-controlled;
            // checking this before file_name() extraction prevents an entry like
            // "evil/../payload.stl" from passing is_allowed_extension on its
            // final component and then being flattened into the model dir.
            // Check component-by-component (not substring) so a legitimate file
            // like "foo..bar.stl" isn't rejected.
            if file_name.split('/').any(|c| c == "..") || file_name.split('\\').any(|c| c == "..") {
                return Ok(false);
            }

            // Only extract allowed extensions (.stl, .3mf, .obj, .pdf)
            if !is_allowed_extension(&file_name) {
                return Ok(false);
            }

            // Flatten file structure into model_dir to avoid deep subfolder hierarchy path issues in public_path.
            // If files from different subfolders share the same filename, resolve to a unique name.
            let base_filename = match Path::new(&file_name).file_name() {
                Some(name) => name,
                None => return Ok(false),
            };
            let (safe_filename, safe_dest) = get_unique_filename(&model_dir, &base_filename.to_string_lossy());

            // Security: prevent path traversal
            if !is_safe_path(&model_dir, &safe_dest) {
                return Ok(false);
            }

            // Stream file data directly to disk without buffering the entire file into RAM
            let mut output_file = match fs::File::create(&safe_dest) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[SLAG3D 7Z] Failed to create {}: {}", safe_dest.display(), e);
                    return Ok(true);
                }
            };

            match std::io::copy(reader, &mut output_file) {
                Ok(size) => {
                    filtered_extracted.push(ExtractedFile {
                        filename: safe_filename,
                        destination_path: safe_dest.clone(),
                        size,
                    });
                }
                Err(e) => {
                    eprintln!("[SLAG3D 7Z] Failed to write {}: {}", safe_dest.display(), e);
                }
            }

            Ok(true)
        }
    )?;

    if filtered_extracted.is_empty() {
        return Err(ArchiveError::NoValidFiles);
    }

    Ok(filtered_extracted)
}
