# SLAG3D — OpenCode notes

Tauri v2 desktop app: React 19 + Vite (`src/`) and Rust (`src-tauri/`), single local user, no auth. No other instruction files; `.opencode/opencode.json` only loads a plugin.

## Read the right sources

- `README.md`, `AUDIT_REPORT.md`, `MIGRATION_GUIDE.md` and `docs/superpowers/*` describe the **retired Express/Puppeteer web server** (`npm run dev/start`, JWT, soft deletes, schema v4, Puppeteer thumbnails). Those scripts and deps do not exist. Trust `package.json`, `src-tauri/tauri.conf.json`, and the code instead. README's native prerequisite packages are still valid.
- Current DB schema is **v6** (hard deletes; legacy `is_deleted`/`deleted_at` dropped on startup), not README's v4.

## Commands

- `npm run build:client` → `dist/` (`npm run build` is an alias). This is the only frontend verification script: no lint, typecheck, or test script exists for `src/`.
- `cd src-tauri && cargo check` and `cargo test` (14 unit tests: migrations, extension allowlist, path traversal, backup round-trip). Run both after Rust changes.
- `npm run tauri dev` does **not** start Vite: `tauri.conf.json` has no `beforeDevCommand`, and `devUrl` is `http://localhost:3000` while Vite defaults to 5173. Start Vite on port 3000 (`npx vite --port 3000`) alongside it, or deliberately change the config.
- `npm run build:desktop` = `sync-version && tauri build`. `scripts/sync-version.js` rewrites committed `package.json` and `tauri.conf.json` versions from the latest `v*` git tag (not `Cargo.toml`), so expect dirty files afterward. CI runs `npm run tauri build` directly, using committed versions.
- Bundle targets are only `appimage` and `nsis` in `tauri.conf.json`; README's `.deb`/`.msi`/`.dmg` claims are aspirational. Output: `src-tauri/target/release/bundle/`.

## Architecture that affects edits

- Components call `apiFetch('/api/...')` from `src/utils/api.ts`: a shim mapping legacy Express REST paths to Tauri `invoke()` commands. Add routes there and register commands in `src-tauri/src/lib.rs`; there is no HTTP server. Unhandled routes deliberately warn and return 404.
- Thumbnails are client-side: `ModelCard` → `src/utils/renderQueue.ts` (serial, deduped, capped) → `clientThumbnailRenderer.ts` (Three.js/WebGL) → `invoke('save_thumbnail')`. `/api/models/{id}/regenerate-thumbnail` is an intentional no-op that flips status to pending.
- `read_file` returns raw IPC bytes; `fetchFileAsBlobUrl` handles ArrayBuffer, JSON byte array, and base64 because all three occur across Tauri transports. Keep all branches.
- Downloads use a native save dialog plus `invoke('save_downloaded_file', { sourcePath, destPath })` — anchor-click downloads silently fail on Linux AppImage. PDF attachments render in `PdfViewerModal` via a blob with `application/pdf` MIME.
- All asset path resolution (custom protocol, `read_file`, `save_downloaded_file`) goes through `services/path_util.rs::resolve_upload_relative_path` — scheme variants, URL decoding, canonical containment. Put path/security fixes there; archive extraction has its own `is_safe_path` in `services/archive.rs`.
- Backup/restore: Navbar → `BackupModal` → `invoke('create_backup' | 'import_backup')` (`commands/backup.rs`, `services/backup.rs`). The ZIP holds `manifest.json`, a `vault.db` snapshot made with `VACUUM INTO`, and `uploads/` (no `tmp/`). Restore validates the archive, copies the DB into the existing live connection via SQLite's online backup API, migrates, and swaps `uploads/`; scratch space is `<app_data>/.backup-work`.
- `slag3d-asset://` serves files under `<app_data>/uploads`; Windows/WebView2 uses `http://slag3d-asset.localhost/...`. `toAssetUrl` in `api.ts` rewrites only thumbnail paths; model files go through `read_file` → blob URL.
- DB: one shared `Mutex<Connection>` with WAL + 5s busy_timeout; migrations run on every startup in `db/schema.rs`; `DEFAULT_USER_ID` stands in for the single user. Get paths from `services/storage.rs` helpers (`get_app_data_dir`, `get_uploads_dir`, `get_models_dir`, `get_thumbnails_dir`, `get_tmp_dir`, `get_model_dir`, `ensure_model_dir`, `get_unique_filename`) and `SLAG3D_APP_DATA_DIR` — never hardcode them.

## Gotchas

- `vite.config.ts`: `DISABLE_HMR=true` disables both HMR and file watching (AI Studio workaround) — leave that logic intact.
- `main.rs` sets `GTK_THEME=Adwaita:dark` unless already set, for native GTK select popups.
- The release workflow builds on `v*` tags across Linux/Windows/macOS; the Windows cross-build needs `RUSTFLAGS=-C link-arg=-fuse-ld=lld`.
