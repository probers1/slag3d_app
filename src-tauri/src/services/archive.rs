use std::path::Path;
use std::fs;
use crate::services::path_util::path_starts_with;
use crate::services::storage::{is_allowed_extension, ExtractedFile, get_model_dir, get_unique_filename};

#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    InvalidPath(String),
    UnsupportedFormat,
    NoValidFiles,
    TooLarge,
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
            ArchiveError::TooLarge => write!(f, "Archive contains too many entries or exceeds the size limit"),
            ArchiveError::ZipError(e) => write!(f, "ZIP error: {}", e),
            ArchiveError::SevenZError(e) => write!(f, "7z error: {}", e),
        }
    }
}

impl std::error::Error for ArchiveError {}

/// Maximum number of entries extracted from a single uploaded archive.
const MAX_ARCHIVE_ENTRIES: usize = 10_000;

/// Maximum number of bytes extracted from a single uploaded archive.
const MAX_EXTRACTED_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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
            // Never silently fall back to the base: that would make an
            // uncanonicalizable path look contained and bypass the check.
            let mut canonical = match ancestor.canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            for component in tail.iter().rev() {
                canonical.push(component);
            }
            canonical
        }
    };

    // Component-wise containment on canonical forms handles Windows \\?\
    // prefixes, mixed separators and sibling-directory prefix trickery.
    path_starts_with(&path_canonical, &base_canonical)
}

pub fn extract_zip(archive_path: &Path, model_id: &str) -> Result<Vec<ExtractedFile>, ArchiveError> {
    let model_dir = get_model_dir(model_id);
    fs::create_dir_all(&model_dir)?;

    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    if archive.len() as usize > MAX_ARCHIVE_ENTRIES {
        return Err(ArchiveError::TooLarge);
    }

    let mut extracted_files = Vec::new();
    let mut total_extracted: u64 = 0;

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

        // Reject path traversal
        if !is_safe_path(&model_dir, &dest_path) {
            continue;
        }

        // Refuse archives that would extract past the size/entry budget.
        if file_entry.size() > MAX_EXTRACTED_ARCHIVE_BYTES || total_extracted + file_entry.size() > MAX_EXTRACTED_ARCHIVE_BYTES {
            return Err(ArchiveError::TooLarge);
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
        total_extracted += size;
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
    let mut extracted_count: usize = 0;
    let mut total_extracted: u64 = 0;

    // sevenz-rust decodes every file in a folder from one shared stream. If a
    // skipped entry isn't drained, the next file in that folder starts at the
    // wrong offset. Returning `false` from the callback aborts the whole
    // iteration, so skipped entries must be drained and then return `true`.
    fn drain_entry(reader: &mut dyn std::io::Read) {
        let mut sink = std::io::sink();
        let _ = std::io::copy(reader, &mut sink);
    }

    sevenz_rust::decompress_with_extract_fn(
        file,
        &model_dir,
        |entry, reader, _dest_path| {
            let file_name = entry.name().to_string();

            // Skip directories (some archives mark dirs explicitly, others end with /)
            if entry.is_directory() || file_name.ends_with('/') || file_name.ends_with('\\') {
                drain_entry(reader);
                return Ok(true);
            }

            // Reject path traversal outright. Entry names are attacker-controlled;
            // checking this before file_name() extraction prevents an entry like
            // "evil/../payload.stl" from passing is_allowed_extension on its
            // final component and then being flattened into the model dir.
            // Check component-by-component (not substring) so a legitimate file
            // like "foo..bar.stl" isn't rejected.
            if file_name.split('/').any(|c| c == "..") || file_name.split('\\').any(|c| c == "..") {
                drain_entry(reader);
                return Ok(true);
            }

            // Only extract allowed extensions (.stl, .3mf, .obj, .pdf)
            if !is_allowed_extension(&file_name) {
                drain_entry(reader);
                return Ok(true);
            }

            // Flatten file structure into model_dir to avoid deep subfolder hierarchy path issues in public_path.
            // If files from different subfolders share the same filename, resolve to a unique name.
            let base_filename = match Path::new(&file_name).file_name() {
                Some(name) => name,
                None => {
                    drain_entry(reader);
                    return Ok(true);
                }
            };
            let (safe_filename, safe_dest) = get_unique_filename(&model_dir, &base_filename.to_string_lossy());

            // Security: prevent path traversal
            if !is_safe_path(&model_dir, &safe_dest) {
                drain_entry(reader);
                return Ok(true);
            }

            // Refuse archives that would extract past the size/entry budget.
            extracted_count += 1;
            if extracted_count > MAX_ARCHIVE_ENTRIES {
                return Err(sevenz_rust::Error::Other("archive entry limit exceeded".into()));
            }
            if entry.size() > MAX_EXTRACTED_ARCHIVE_BYTES || total_extracted + entry.size() > MAX_EXTRACTED_ARCHIVE_BYTES {
                return Err(sevenz_rust::Error::Other("archive size limit exceeded".into()));
            }

            // Stream file data directly to disk without buffering the entire file into RAM
            let mut output_file = match fs::File::create(&safe_dest) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[SLAG3D 7Z] Failed to create {}: {}", safe_dest.display(), e);
                    drain_entry(reader);
                    return Ok(true);
                }
            };

            match std::io::copy(reader, &mut output_file) {
                Ok(size) => {
                    total_extracted += size;
                    filtered_extracted.push(ExtractedFile {
                        filename: safe_filename,
                        destination_path: safe_dest.clone(),
                        size,
                    });
                }
                Err(e) => {
                    eprintln!("[SLAG3D 7Z] Failed to write {}: {}", safe_dest.display(), e);
                    drain_entry(reader);
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

#[cfg(test)]
mod tests {
    use super::*;
    use sevenz_rust::{SevenZArchiveEntry, SevenZWriter};
    use std::path::PathBuf;
    use std::io::Write;
    use std::sync::{Mutex, MutexGuard};

    // extract_zip/extract_7z resolve their destination through the
    // SLAG3D_APP_DATA_DIR env var, and tests in this module run in parallel.
    // Serialize the ones that touch it.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap()
    }

    #[test]
    fn extract_zip_rejects_archive_over_entry_limit() {
        let _guard = env_lock();
        let root = std::env::temp_dir().join(format!(
            "slag3d-zip-limit-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        std::env::set_var("SLAG3D_APP_DATA_DIR", &root);

        let archive = root.join("big.zip");
        {
            let file = fs::File::create(&archive).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            for i in 0..=10_000 {
                zip.start_file(format!("file_{}.stl", i), zip::write::FileOptions::default())
                    .unwrap();
                zip.write_all(b"solid x\nendsolid x\n").unwrap();
            }
            zip.finish().unwrap();
        }

        let result = extract_zip(&archive, "test-model");
        std::env::remove_var("SLAG3D_APP_DATA_DIR");

        assert!(matches!(result, Err(ArchiveError::TooLarge)));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn extract_7z_skips_disallowed_entries_and_keeps_later_models() {
        let _guard = env_lock();
        let root = std::env::temp_dir().join(format!(
            "slag3d-7z-skip-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        // extract_7z resolves its destination through get_model_dir(), which
        // reads this env var. No other test in this crate touches it.
        std::env::set_var("SLAG3D_APP_DATA_DIR", &root);

        let archive = root.join("mixed.7z");
        {
            let mut writer = SevenZWriter::create(&archive).unwrap();
            let entries: [(&str, &[u8]); 4] = [
                ("readme.txt", b"not a model"),
                ("first.stl", b"solid first\nendsolid first\n"),
                ("__MACOSX/junk.bin", b"junk"),
                ("second.obj", b"v 0 0 0\n"),
            ];
            for (name, data) in entries {
                let mut entry = SevenZArchiveEntry::default();
                entry.name = name.to_string();
                entry.has_stream = true;
                writer.push_archive_entry::<&[u8]>(entry, Some(data)).unwrap();
            }
            writer.finish().unwrap();
        }

        let result = extract_7z(&archive, "test-model");
        // Restore the env var before asserting so a panic doesn't leak it.
        std::env::remove_var("SLAG3D_APP_DATA_DIR");

        let files = result.unwrap();
        assert_eq!(files.len(), 2, "expected both valid model files to extract");
        let model_dir: PathBuf = root.join("uploads/models/test-model");
        assert_eq!(fs::read(model_dir.join("first.stl")).unwrap(), b"solid first\nendsolid first\n");
        assert_eq!(fs::read(model_dir.join("second.obj")).unwrap(), b"v 0 0 0\n");
        assert!(!model_dir.join("readme.txt").exists());
        assert!(!model_dir.join("junk.bin").exists());

        fs::remove_dir_all(&root).unwrap();
    }
}
