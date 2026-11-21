//! Full-vault backup and restore.
//!
//! A backup is a ZIP containing:
//!   manifest.json  - format marker and metadata
//!   vault.db       - consistent SQLite snapshot (via VACUUM INTO, so WAL data
//!                    is included even when the live DB has uncheckpointed pages)
//!   uploads/...    - model files and thumbnails (transient `tmp/` is skipped)
//!
//! Restore copies the backed-up database into the *live* connection with
//! SQLite's online backup API instead of swapping DB files under a running
//! process. The app keeps one process-wide connection (db/mod.rs), so closing
//! and reopening it from a command is not possible.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::services::archive::is_safe_path;

pub const BACKUP_FORMAT: &str = "slag3d-backup";
pub const BACKUP_FORMAT_VERSION: u32 = 1;
pub const MANIFEST_NAME: &str = "manifest.json";
pub const DB_NAME: &str = "vault.db";
const MAX_RESTORE_BYTES: u64 = 10 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format: String,
    pub format_version: u32,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub model_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub path: String,
    pub size: u64,
    pub created_at: String,
    pub model_count: u64,
}

#[derive(Debug)]
pub enum BackupError {
    Io(io::Error),
    Sql(rusqlite::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    Invalid(String),
}

impl From<io::Error> for BackupError {
    fn from(e: io::Error) -> Self {
        BackupError::Io(e)
    }
}
impl From<rusqlite::Error> for BackupError {
    fn from(e: rusqlite::Error) -> Self {
        BackupError::Sql(e)
    }
}
impl From<zip::result::ZipError> for BackupError {
    fn from(e: zip::result::ZipError) -> Self {
        BackupError::Zip(e)
    }
}
impl From<serde_json::Error> for BackupError {
    fn from(e: serde_json::Error) -> Self {
        BackupError::Json(e)
    }
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupError::Io(e) => write!(f, "IO error: {}", e),
            BackupError::Sql(e) => write!(f, "Database error: {}", e),
            BackupError::Zip(e) => write!(f, "Archive error: {}", e),
            BackupError::Json(e) => write!(f, "Manifest error: {}", e),
            BackupError::Invalid(e) => write!(f, "Invalid backup: {}", e),
        }
    }
}

impl std::error::Error for BackupError {}

pub fn model_count(conn: &Connection) -> Result<u64, BackupError> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM models", [], |r| r.get(0))?;
    Ok(n.max(0) as u64)
}

pub fn schema_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT version FROM schema_versions WHERE id = 1",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Quote a path for use as a SQL string literal. VACUUM INTO's argument does
/// not reliably accept bound parameters across SQLite versions.
fn sql_string(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

fn collect_upload_files(
    dir: &Path,
    rel: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), BackupError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        // uploads/tmp holds in-flight extraction files, not user data.
        if rel.as_os_str().is_empty() && name == "tmp" && kind.is_dir() {
            continue;
        }
        // Skip symlinks rather than following them outside the vault.
        if !kind.is_file() && !kind.is_dir() {
            continue;
        }
        let child_rel = rel.join(&name);
        if kind.is_dir() {
            collect_upload_files(&entry.path(), &child_rel, out)?;
        } else {
            out.push((
                child_rel.to_string_lossy().replace('\\', "/"),
                entry.path(),
            ));
        }
    }
    Ok(())
}

/// Write a consistent, self-contained copy of the live DB to `snapshot_path`
/// without disturbing the connection or requiring a WAL checkpoint.
pub fn snapshot_database(live_db: &Connection, snapshot_path: &Path) -> Result<(), BackupError> {
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent)?;
    }
    live_db.execute_batch(&format!("VACUUM INTO {}", sql_string(snapshot_path)))?;
    Ok(())
}

/// Write a complete backup to `destination`. `work_dir` is scratch space that
/// must be on the same filesystem as the DB and is removed by the caller.
pub fn write_backup_archive(
    live_db: &Connection,
    uploads_dir: &Path,
    work_dir: &Path,
    destination: &Path,
    app_version: &str,
) -> Result<BackupInfo, BackupError> {
    fs::create_dir_all(work_dir)?;
    let snapshot = work_dir.join(format!("snapshot-{}.db", Uuid::new_v4()));
    snapshot_database(live_db, &snapshot)?;

    let manifest = BackupManifest {
        format: BACKUP_FORMAT.to_string(),
        format_version: BACKUP_FORMAT_VERSION,
        created_at: Utc::now().to_rfc3339(),
        app_version: app_version.to_string(),
        schema_version: schema_version(live_db),
        model_count: model_count(live_db)?,
    };

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(destination)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6));

    zip.start_file(DB_NAME, options)?;
    zip.write_all(&fs::read(&snapshot)?)?;

    zip.start_file(MANIFEST_NAME, options)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    // Keep the folder in the archive even when the vault has no files yet.
    zip.add_directory("uploads/", options)?;

    if uploads_dir.is_dir() {
        let mut files = Vec::new();
        collect_upload_files(uploads_dir, Path::new(""), &mut files)?;
        for (rel, path) in files {
            zip.start_file(format!("uploads/{}", rel), options)?;
            let mut input = File::open(&path)?;
            io::copy(&mut input, &mut zip)?;
        }
    }

    zip.finish()?;
    let _ = fs::remove_file(&snapshot);

    Ok(BackupInfo {
        path: destination.to_string_lossy().into_owned(),
        size: fs::metadata(destination)?.len(),
        created_at: manifest.created_at,
        model_count: manifest.model_count,
    })
}

/// Accept only relative, normal path components inside the ZIP. Backslashes,
/// colons (Windows drive prefixes) and `..` are rejected outright.
fn safe_entry_name(name: &str) -> bool {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') || name.contains(':') {
        return false;
    }
    Path::new(name)
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Extract a backup into a fresh work directory and validate its manifest.
pub fn extract_backup_archive(
    archive_path: &Path,
    work_dir: &Path,
) -> Result<BackupManifest, BackupError> {
    fs::create_dir_all(work_dir)?;
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut extracted = 0u64;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if !safe_entry_name(&name) {
            return Err(BackupError::Invalid(format!("unsafe path in archive: {}", name)));
        }
        let target = work_dir.join(&name);
        if !is_safe_path(work_dir, &target) {
            return Err(BackupError::Invalid(format!("unsafe path in archive: {}", name)));
        }
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&target)?;
        extracted += io::copy(&mut entry, &mut output)?;
        if extracted > MAX_RESTORE_BYTES {
            return Err(BackupError::Invalid("backup exceeds the 10 GB size limit".to_string()));
        }
    }

    let manifest_path = work_dir.join(MANIFEST_NAME);
    if !manifest_path.is_file() {
        return Err(BackupError::Invalid("manifest.json is missing".to_string()));
    }
    let manifest: BackupManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.format != BACKUP_FORMAT || manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err(BackupError::Invalid(format!(
            "unsupported format {} version {}",
            manifest.format, manifest.format_version
        )));
    }
    if !work_dir.join(DB_NAME).is_file() {
        return Err(BackupError::Invalid("vault.db is missing".to_string()));
    }
    Ok(manifest)
}

/// Verify the extracted database before touching the live vault.
pub fn validate_backup_db(db_path: &Path) -> Result<(i64, u64), BackupError> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let check: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(BackupError::Invalid(format!(
            "database integrity check failed: {}",
            check
        )));
    }
    Ok((schema_version(&conn), model_count(&conn)?))
}

/// Copy a backed-up database into the live connection. SQLite wraps this in a
/// transaction on the destination, so a failed restore leaves the live DB
/// unchanged.
pub fn restore_database(source_db: &Path, live_db: &mut Connection) -> Result<(), BackupError> {
    let source = Connection::open_with_flags(source_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let backup = rusqlite::backup::Backup::new(&source, live_db)?;
    backup.run_to_completion(100, Duration::from_millis(10), None)?;
    Ok(())
}

/// Swap `uploads/` for the restored copy, keeping the old tree until the new
/// one is in place. Both paths live under app data, so rename is atomic.
pub fn replace_uploads_dir(app_data: &Path, restored_uploads: &Path) -> Result<(), BackupError> {
    // Older hand-made archives may omit the directory entirely; treat that as
    // an empty vault rather than failing a valid DB restore.
    fs::create_dir_all(restored_uploads)?;
    let current = app_data.join("uploads");
    let staged = app_data.join(format!("uploads.pre-restore-{}", Uuid::new_v4()));
    let mut moved_old = false;

    if current.exists() {
        fs::rename(&current, &staged)?;
        moved_old = true;
    }
    if let Err(e) = fs::rename(restored_uploads, &current) {
        if moved_old && !current.exists() {
            let _ = fs::rename(&staged, &current);
        }
        return Err(BackupError::Io(e));
    }
    if moved_old {
        let _ = fs::remove_dir_all(&staged);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("slag3d-backup-test-{}-{}", tag, Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_db(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_versions (id INTEGER PRIMARY KEY, version INTEGER NOT NULL);
             INSERT INTO schema_versions (id, version) VALUES (1, 6);
             CREATE TABLE models (id TEXT PRIMARY KEY, title TEXT NOT NULL);
             INSERT INTO models (id, title) VALUES ('m1', 'Cube');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn backup_round_trip() {
        let root = temp_dir("roundtrip");
        let db_path = root.join("vault.db");
        let conn = make_db(&db_path);
        let uploads = root.join("uploads");
        fs::create_dir_all(uploads.join("models/m1")).unwrap();
        fs::write(uploads.join("models/m1/part.stl"), b"solid cube").unwrap();
        fs::create_dir_all(uploads.join("tmp")).unwrap();
        fs::write(uploads.join("tmp/junk"), b"skip me").unwrap();

        let destination = root.join("backup.zip");
        let info = write_backup_archive(
            &conn,
            &uploads,
            &root.join("work"),
            &destination,
            "test",
        )
        .unwrap();
        assert_eq!(info.model_count, 1);
        assert!(info.size > 0);

        let extracted = root.join("extracted");
        let manifest = extract_backup_archive(&destination, &extracted).unwrap();
        assert_eq!(manifest.schema_version, 6);
        assert_eq!(manifest.model_count, 1);
        assert!(extracted.join("vault.db").is_file());
        assert!(extracted.join("uploads/models/m1/part.stl").is_file());
        assert!(!extracted.join("uploads/tmp").exists());

        let (version, count) = validate_backup_db(&extracted.join("vault.db")).unwrap();
        assert_eq!((version, count), (6, 1));

        let mut live = make_db(&root.join("live.db"));
        restore_database(&extracted.join("vault.db"), &mut live).unwrap();
        assert_eq!(model_count(&live).unwrap(), 1);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn backup_with_no_files_restores_empty_uploads_dir() {
        let root = temp_dir("empty");
        let conn = make_db(&root.join("vault.db"));
        let destination = root.join("backup.zip");
        write_backup_archive(
            &conn,
            &root.join("no-uploads-here"),
            &root.join("work"),
            &destination,
            "test",
        )
        .unwrap();

        let extracted = root.join("out");
        extract_backup_archive(&destination, &extracted).unwrap();
        assert!(extracted.join("uploads").is_dir());
        replace_uploads_dir(&root, &extracted.join("uploads")).unwrap();
        assert!(root.join("uploads").is_dir());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_unsafe_zip_entries() {
        let root = temp_dir("traversal");
        let archive_path = root.join("evil.zip");
        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = FileOptions::default();
            zip.start_file("manifest.json", options).unwrap();
            zip.write_all(br#"{"format":"slag3d-backup","format_version":1,"created_at":"x","app_version":"x","schema_version":6,"model_count":0}"#).unwrap();
            zip.start_file("vault.db", options).unwrap();
            zip.write_all(b"db").unwrap();
            zip.start_file("../escaped.txt", options).unwrap();
            zip.write_all(b"bad").unwrap();
            zip.finish().unwrap();
        }
        let err = extract_backup_archive(&archive_path, &root.join("out")).unwrap_err();
        assert!(matches!(err, BackupError::Invalid(_)));
        fs::remove_dir_all(&root).unwrap();
    }
}
