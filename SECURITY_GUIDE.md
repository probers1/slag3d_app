# Security Implementation Guide — SLAG3D Desktop (Tauri v2)

> This guide describes the **current Tauri v2 desktop app**. The legacy Express server's authentication model (JWT, sessions, CSRF) is gone; see [README.md](./README.md) for setup and architecture.

## Architecture

The app is a single-user local application:

- No network server or listening ports
- No authentication, accounts or roles
- Native OS WebView (WebKitGTK / WebView2 / WKWebView)
- Rust backend: `rusqlite`, `zip` / `sevenz-rust`, native filesystem access via `std::fs`
- Frontend talks to Rust through Tauri `invoke()` commands; no HTTP API

## Path handling

All user-influenced paths go through `src-tauri/src/services/path_util.rs`:

- `resolve_upload_relative_path()` strips scheme variants (`slag3d-asset://`, the Windows `http(s)://slag3d-asset.localhost/` form, bare `/uploads/...`), decodes percent escapes, strips cache-busting query strings, canonicalizes, and verifies **component-wise containment** under `<app-data>/uploads`.
- `is_safe_id()` rejects model IDs containing separators, `.` or other non-alphanumeric path components before they are joined into `uploads/models/<id>` or `uploads/thumbnails/<id>.png`.
- Archive extraction uses `is_safe_path()` in `services/archive.rs` for the same containment guarantee on entries that do not exist yet.

Keep new file access on these helpers rather than adding ad-hoc `starts_with()` or `strip_prefix()` checks.

## Asset protocol

`slag3d-asset://` (registered in `protocol.rs`, with `http://slag3d-asset.localhost/` used on Windows/WebView2):

- Resolves every request through the shared path resolver; traversal and sibling-directory tricks return 404.
- Serves only regular files under `uploads/`; no directory listing.
- MIME type is inferred from the file extension (`mime_guess`); unknown types fall back to `application/octet-stream`.
- The handler reads the whole file into memory for the response — there is no range-request support. Large model files are normally loaded through `read_file` → blob URL in the frontend instead.

## File reads and downloads

- `read_file` validates the path and returns raw bytes over IPC; the frontend builds a blob URL (`fetchFileAsBlobUrl` handles ArrayBuffer, JSON byte arrays and base64 transports).
- Downloads use the native save dialog plus `save_downloaded_file`, which validates the source path and copies it to the user-chosen destination. Anchor-click downloads are not used (they fail silently on Linux AppImage).
- PDF attachments are rendered in `PdfViewerModal` from a blob with `application/pdf` MIME.

## Archive extraction

`services/archive.rs` uses pure-Rust `zip` and `sevenz-rust` crates (no shelling out to `7z`):

- Entry names are checked for traversal and containment before any file is written; files are flattened into `uploads/models/<id>/` with collision-safe names.
- Only `.stl`, `.3mf`, `.obj` and `.pdf` are kept. Disallowed 7z entries are drained and skipped so later valid files are still extracted.
- Model archives are extracted directly into the model directory; `uploads/tmp/` is only used for transient work and is excluded from backups.

There is currently **no size cap on model archive extraction** — extract untrusted archives with the same care you would any other user-supplied file. Database backup restores have a 10 GB extraction cap in `services/backup.rs`.

## Database

- Single shared connection with WAL mode and a 5-second `busy_timeout`.
- All queries use parameter binding; multi-step writes use transactions (`update_model`, imports, deletes).
- Schema v6 hard-deletes models; legacy soft-delete columns are dropped on startup.
- The fixed `DEFAULT_USER_ID` stands in for the single local user; there are no account or session tables.

## Backup & restore

- Backups are ZIPs containing `manifest.json`, a `VACUUM INTO` snapshot of the DB and `uploads/` (without `tmp/`).
- Exports are written to a temp file and renamed into place so a failed export cannot destroy the previous backup.
- Imports validate ZIP entry paths, check the DB integrity, and require `manifest.json` metadata to match the actual database. The version check uses the DB's schema version.
- Restore copies the DB into the live connection with SQLite's online backup API and swaps `uploads/`; on failure both halves are rolled back. Pre-restore files are only deleted after full success.

## WebView CSP and capabilities

The live CSP is in `src-tauri/tauri.conf.json` (`security.csp` / `devCsp`) — read that file rather than any copied snippet. It permits the asset scheme, blob URLs and localhost dev origins; `'unsafe-inline'` / `'unsafe-eval'` are required by the existing frontend and Three.js.

`src-tauri/capabilities/default.json` grants `core:default`, `dialog:default`, `dialog:allow-open` and `dialog:allow-save` to the main window. File access is performed by Rust commands, not the Tauri `fs` plugin.

## Threat model

| Threat | Mitigation |
|--------|------------|
| Zip slip / traversal in archives or asset URLs | Canonical component-wise containment checks; disallowed entries skipped |
| Malicious model ID used as a path | `is_safe_id()` validation before path joins |
| Database corruption | WAL, busy timeout, transactions, startup migrations, backup integrity check |
| Tampered backup metadata | Manifest compared against the DB snapshot; newer schema rejected |
| Unauthorized file read | Asset protocol only serves contained paths under `uploads/` |
| WebView XSS | CSP restricts sources; no server or external scripts; user content is not rendered as HTML |

## Removed with the web server

JWT/session secrets, CSRF tokens, rate limiting, password hashing/rotation, HTTPS/HSTS headers, multi-user isolation, admin panel and user management.

## Reporting security issues

Report security issues through GitHub Security Advisories (private disclosure) rather than public issues.
