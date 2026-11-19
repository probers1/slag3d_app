# Security Implementation Guide — SLAG3D Desktop (Tauri v2)

> This guide covers security for the **Tauri v2 desktop application only**. The legacy web server authentication model (JWT, sessions, CSRF) has been removed.

---

## Architecture Overview

The desktop app runs as a **single-user local application** with:
- No network server / no open ports
- No authentication (local user context only)
- Native OS WebView (WebKitGTK / WebView2 / WKWebView)
- Rust backend with SQLite (`rusqlite`), native archive extraction, custom asset protocol

---

## Security Controls

### 1. Custom Asset Protocol (`slag3d-asset://`)

**Purpose**: Safely serve local 3D model files and thumbnails to the WebView without CORS or HTTP overhead.

**Implementation** (`src-tauri/src/protocol.rs`):
- Path traversal protection: resolves and validates paths against allowed base directories (`uploads/models/`, `uploads/thumbnails/`)
- MIME type validation: only serves known 3D model and image types
- Range request support for large file streaming (up to 300MB)
- No directory listing capability

**Validation checklist**:
- [ ] Protocol rejects `../` traversal attempts
- [ ] Only registered asset directories are accessible
- [ ] Unknown file extensions return 404, not served as octet-stream

---

### 2. Archive Extraction Safety

**Purpose**: Extract user-provided `.zip` and `.7z` archives without path traversal or resource exhaustion.

**Implementation** (`src-tauri/src/services/archive.rs`):
- Uses `zip` and `sevenz-rust` crates (pure Rust, no shell execution)
- Validates each entry path: rejects absolute paths, `..` components, symlinks
- Enforces per-entry size limits and total extraction size limits
- Extracts to isolated temporary directory (`uploads/tmp/<uuid>/`)
- Only allows known 3D extensions (`.stl`, `.3mf`, `.obj`) to be promoted to permanent storage

**Validation checklist**:
- [ ] Zip slip (path traversal) attempts are blocked
- [ ] Symlink entries are rejected
- [ ] Extraction aborts on total size > 500MB
- [ ] Temporary directory cleaned up on success and failure

---

### 3. SQLite Database Security

**Implementation** (`src-tauri/src/db/`):
- WAL mode enabled for concurrency safety
- `busy_timeout(5000ms)` prevents lock contention
- Single fixed user ID (`00000000-0000-0000-0000-000000000000`) — no multi-user isolation needed
- Migrations run at startup (`schema.rs`); schema version tracked in `schema_versions` table
- Parameterized queries everywhere (no string interpolation)

**Database location** (OS standard app data dir):
- Linux: `~/.local/share/slag3d/vault.db`
- Windows: `%APPDATA%\slag3d\vault.db`
- macOS: `~/Library/Application Support/slag3d/vault.db`

**Validation checklist**:
- [ ] All queries use parameter binding (`?` placeholders)
- [ ] No dynamic SQL construction from user input
- [ ] Migration v5 handles legacy `user_id` columns safely

---

### 4. WebView Content Security Policy

Configured in `src-tauri/tauri.conf.json`:

```json
"security": {
  "csp": "default-src 'self' slag3d-asset:; connect-src 'self' slag3d-asset: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: slag3d-asset:; font-src 'self' data:; worker-src 'self' blob:;"
}
```

**Key points**:
- `slag3d-asset:` protocol explicitly allowed for assets
- No `http:` or `https:` in `default-src` — app makes no network requests by default
- `'unsafe-eval'` required for Three.js shader compilation (mitigated by no external scripts)
- No `frame-ancestors` needed (not embeddable)

---

### 5. File System Permissions

Tauri capability configuration (`src-tauri/capabilities/default.json`):
- `fs:allow-read` / `fs:allow-write` scoped to `$APPDATA/slag3d/**`
- `dialog:allow-open` for native file pickers (user-initiated only)
- No arbitrary file system access

---

### 6. Native IPC Command Surface

All backend operations exposed via Tauri commands (`invoke()`):
- Input validation at command boundary (serde deserialization + explicit checks)
- No raw SQL or shell command execution
- File operations use `std::fs` with validated paths
- Error messages sanitized (no internal paths in responses)

---

## Threat Model & Mitigations

| Threat | Mitigation |
|--------|------------|
| Malicious archive (zip slip, symlinks) | Pure Rust extractors + path validation + size limits |
| Large file DoS (memory exhaustion) | Streaming via `slag3d-asset://` protocol, no full-file buffering |
| Path traversal via model ID | Model IDs are UUIDs; file paths constructed via `storage.rs` helpers |
| WebView XSS | CSP restricts sources; no user HTML rendering; Three.js loads trusted assets only |
| Database corruption | WAL mode + busy_timeout; single-writer (local app) |
| Unauthorized file read | Protocol handler validates against allowlisted directories only |

---

## Security Checklist for Releases

```bash
# 1. Verify CSP in tauri.conf.json has no http:/https: in default-src
grep -A5 '"csp"' src-tauri/tauri.conf.json

# 2. Confirm no shell commands in Rust code
grep -r "std::process::Command" src-tauri/src/

# 3. Verify all SQL uses parameter binding
grep -r "format!\|concat!" src-tauri/src/db/ || echo "No string formatting in SQL ✓"

# 4. Check archive extraction limits are enforced
grep -A10 "MAX_" src-tauri/src/services/archive.rs

# 5. Build with audit
cd src-tauri && cargo audit
```

---

## No Longer Applicable (Removed)

| Feature | Reason |
|---------|--------|
| JWT / Session secrets | No authentication in desktop app |
| CSRF tokens | No state-changing HTTP endpoints |
| Rate limiting | No network exposure |
| Password hashing / rotation | No user accounts |
| HTTPS / HSTS headers | No HTTP server |
| Multi-user isolation | Single local user context |
| Admin panel / user management | Not applicable |

---

## Reporting Security Issues

If you discover a security issue in the desktop app, please report it via GitHub Security Advisories (private disclosure) rather than public issues.

---

**Last Updated**: Nov 2026 — Tauri v2 Desktop Architecture