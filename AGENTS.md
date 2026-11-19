# SLAG3D Developer Guide — Tauri v2 Desktop App

## Quick Reference

### Commands
```bash
npm run tauri dev         # Development mode (Vite + Tauri with hot reload)
npm run build:desktop     # Production build — native installers
npm run build:client      # Frontend only build (Vite)
npm run build             # Alias for build:client
```

### Database
- **Location**: OS app data directory
  - Linux: `~/.local/share/slag3d/vault.db`
  - Windows: `%APPDATA%\slag3d\vault.db`
  - macOS: `~/Library/Application Support/slag3d/vault.db`
- **Schema Version**: 5 (includes legacy user_id migration)
- **Engine**: SQLite via `rusqlite` (Rust), WAL mode
- **Migrations**: Auto-run on startup (`src-tauri/src/db/schema.rs`)

---

## File System Structure

```
slag3D_app/
├── src-tauri/                 # Rust Desktop Application Backend
│   ├── Cargo.toml
│   ├── tauri.conf.json        # Tauri v2 configuration
│   ├── build.rs
│   ├── icons/                 # Platform application icons
│   └── src/
│       ├── main.rs            # Entry point
│       ├── lib.rs             # Tauri setup & command registration
│       ├── db/
│       │   ├── mod.rs         # SQLite initialization & WAL mode
│       │   └── schema.rs      # Schema definition & migrations v1..v5
│       ├── commands/          # IPC commands invoked by React
│       │   ├── models.rs
│       │   ├── collections.rs
│       │   ├── tags.rs
│       │   └── app.rs         # App info, storage location commands
│       ├── protocol.rs        # Custom slag3d-asset:// protocol
│       └── services/
│           ├── archive.rs     # Zip / 7z extraction logic
│           └── storage.rs     # Local directory manager
├── src/                       # React 19 Frontend
│   ├── App.tsx
│   ├── components/
│   │   ├── Navbar.tsx
│   │   ├── ModelCard.tsx
│   │   ├── ModelViewerModal.tsx
│   │   ├── CollectionsDrawer.tsx
│   │   ├── UploadModal.tsx
│   │   └── ErrorBoundary.tsx
│   ├── utils/
│   │   ├── api.ts             # Tauri IPC bridge wrapper
│   │   └── clientThumbnailRenderer.ts
│   └── types.ts
├── package.json               # Frontend dependencies & Tauri CLI
├── vite.config.ts             # Vite configuration for Tauri
└── docs/
    └── superpowers/specs/2026-11-09-tauri-conversion-design.md
```

---

## Storage Paths

- **Never** hardcode paths — use `src-tauri/src/services/storage.rs` helpers:
  - `get_app_data_dir()` → OS standard app data directory
  - `get_uploads_dir()` → `<app_data>/uploads`
  - `get_models_dir()` → `<app_data>/uploads/models`
  - `get_thumbnails_dir()` → `<app_data>/uploads/thumbnails`
  - `get_tmp_dir()` → `<app_data>/uploads/tmp`
  - `get_model_dir(model_id)` → `<app_data>/uploads/models/<id>`
- Tauri `PathResolver` provides the base directory at runtime
- `SLAG3D_APP_DATA_DIR` env var set at startup for Rust utilities

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `SLAG3D_APP_DATA_DIR` | Set automatically by Tauri at runtime |
| `GTK_THEME` | Set to `Adwaita:dark` on Linux for native dark theme |

**No secrets required** — desktop app has no authentication, no JWT, no sessions.

---

## Thumbnail Engine

### Flow (Client-Side WebGL)
1. User imports model via native file dialog
2. Frontend loads file in Three.js offscreen canvas
3. Renders 512×512 PNG via `clientThumbnailRenderer.ts`
4. Sends base64 PNG to Rust via `invoke('save_thumbnail', { id, base64 })`
5. Rust saves to `<app_data>/uploads/thumbnails/<id>.png`

### No Puppeteer / No Headless Chromium / No Job Queue

---

## Architecture

| Layer | Technology |
|-------|------------|
| Frontend | React 19, TypeScript, Tailwind CSS 4, Three.js |
| Backend | Rust (Tauri v2), `rusqlite`, `zip`, `sevenz-rust`, `tokio` |
| Database | SQLite (WAL mode), single local user |
| IPC | Tauri commands (`invoke()`) + custom `slag3d-asset://` protocol |
| File Dialogs | `@tauri-apps/plugin-dialog` (native OS pickers) |

---

## IPC Commands (Rust → Frontend)

| Command | Description |
|---------|-------------|
| `get_models` | Paginated search & filtering |
| `get_model_detail` | Full model details, files, tags, collections |
| `import_models` | Import files/archives from native dialog |
| `update_model` | Update metadata (title, description, tags) |
| `delete_model` | Remove model + files |
| `delete_model_file` | Remove individual file from model |
| `toggle_favorite` | Toggle favorite status |
| `save_thumbnail` | Save base64 PNG from WebGL render |
| `add_model_files` | Add additional files to existing model |
| `read_file` | Read file content via `slag3d-asset://` |
| `get_collections` | Hierarchy of collections/folders |
| `create_collection` | Create new collection/folder |
| `rename_collection` | Rename collection |
| `delete_collection` | Delete collection |
| `add_model_to_collection` | Assign model to collection |
| `remove_model_from_collection` | Unassign model from collection |
| `get_tags` | List tags with usage counts |
| `init_db` | Re-initialize database (dev) |

---

## Custom Asset Protocol

`slag3d-asset://` — Serves local files to WebView securely:
- `slag3d-asset://models/<model_id>/<filename>` → 3D model files
- `slag3d-asset://thumbnails/<model_id>.png` → Thumbnail images
- Path traversal blocked, MIME types validated, range requests supported

---

## Development

### Prerequisites
- Node.js 20+
- Rust 1.93+ (`rustup update`)
- Linux: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
- Windows: MinGW-w64 or Visual Studio Build Tools
- macOS: Xcode Command Line Tools

### Workflow
```bash
npm install                    # Install frontend deps
npm run tauri dev             # Start dev (Vite HMR + Rust hot reload)
# Edit src/ → UI updates instantly
# Edit src-tauri/src/ → Rust recompiles, app reloads
```

### Building
```bash
npm run build:desktop         # Creates native installers in src-tauri/target/release/bundle/
# Linux:   .AppImage, .deb
# Windows: .msi, .exe (NSIS)
# macOS:   .dmg, .app
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `tauri dev` fails on Linux | Install WebKitGTK deps (see Prerequisites) |
| Rust compile errors | `cd src-tauri && cargo check` for details |
| Database locked | Ensure no other instance running; WAL handles concurrency |
| Thumbnails not generating | Check WebGL support in WebView; fallback renders wireframe |
| Large file import slow | Uses streaming; check `slag3d-asset://` protocol handler |
| App won't start on Windows | Install WebView2 Runtime (usually pre-installed on Win10+) |

---

## Notes for Working with OpenCode

- **Rust backend**: Edit `src-tauri/src/**/*.rs` → `cargo check` in `src-tauri/` to verify
- **Frontend**: Edit `src/**/*.tsx` → HMR updates in `tauri dev`
- **Typescript**: Use `@/*` path aliases pointing to `src/`
- **Three.js**: Bundled via Vite; check `src/utils/clientThumbnailRenderer.ts` for rendering logic
- **Database changes**: Modify `src-tauri/src/db/schema.rs` migrations, bump version

---

**Last Updated**: Nov 2026 — Tauri v2 Desktop Architecture