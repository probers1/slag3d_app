# SLAG3D — OpenCode notes

Tauri v2 desktop app: React 19 + Vite (`src/`) and Rust (`src-tauri/`), single local user, no auth. No other instruction files; `.opencode/opencode.json` only loads a plugin.

## Read the right sources

- `README.md`, `MIGRATION_GUIDE.md` and `SECURITY_GUIDE.md` describe the **current Tauri app**. `AUDIT_REPORT.md` and `docs/superpowers/*` are explicitly marked historical (old conversion audit/plan/spec) — don't follow their checklists or schema notes. As always, `package.json`, `src-tauri/tauri.conf.json` and the code win if docs and code disagree.
- Current DB schema is **v6** (hard deletes; legacy `is_deleted`/`deleted_at` dropped on startup).

## Commands

- `npm run build:client` → `dist/` (`npm run build` is an alias). `npm run typecheck` runs `tsc --noEmit`. No frontend test or lint script exists.
- `cd src-tauri && cargo check` and `cargo test` (19 unit tests: migrations, extension allowlist, path traversal, archive/backup round-trips, rollback). Run both after Rust changes.
- `npm run tauri dev` starts Vite automatically: `beforeDevCommand` runs `npm run dev:client` (`vite --port 3000`) from the repo root, matching `devUrl`. `DISABLE_HMR=true` still disables HMR and watching.
- `npm run build:desktop` = `sync-version && tauri build`. `scripts/sync-version.js` rewrites committed `package.json`, `tauri.conf.json` and `src-tauri/Cargo.toml` versions from the latest `v*` git tag, so expect dirty files afterward (Cargo refreshes `Cargo.lock` on the next build). CI runs `npm run tauri build` directly, using committed versions.
- `.github/workflows/ci.yml` runs typecheck, frontend build, `cargo check` and `cargo test` on pushes to `main` and on pull requests.
- Bundle targets in `tauri.conf.json` are `appimage`, `nsis`, `dmg` (each used only on its own platform); `.deb`/`.msi` are not built. CI builds a universal macOS DMG (`npm run tauri build -- --target universal-apple-darwin`). Output: `src-tauri/target/<target>/release/bundle/`.

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
