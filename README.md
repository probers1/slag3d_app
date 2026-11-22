# SLAG3D — 3D Vault

A local, single-user desktop app for organising 3D printing assets. Built with Tauri v2 (Rust + native WebView), React 19, Three.js and SQLite. There is no server, no login and no network dependency at runtime.

---

## Features

- **Model library** — import `.stl`, `.3mf`, `.obj` and PDF attachments; `.zip` / `.7z` archives are extracted automatically.
- **3D viewer** — orbit controls, wireframe toggle, multi-file inspection, bounding-box dimensions, volume and filament weight estimate.
- **PDF attachments** — opened inline with the OS WebView PDF viewer.
- **Organisation** — nested collections, tags, favourites, search and filters.
- **Thumbnails** — rendered client-side with Three.js/WebGL through a serial render queue; no Puppeteer or headless browser.
- **Backup & restore** — export/import the whole vault as one ZIP (database snapshot + uploads).
- **Native dialogs** — OS file pickers for imports, backup destination and downloads.

---

## Tech stack

| Layer | Technology |
|-------|------------|
| Desktop shell | Tauri v2 (Rust, native WebView) |
| Frontend | React 19, TypeScript, Tailwind CSS 4, Vite 8 |
| 3D | Three.js (`STLLoader`, `OBJLoader`, `3MFLoader`, `OrbitControls`) |
| Backend | Rust: `rusqlite`, `zip`, `sevenz-rust`, `tauri-plugin-dialog` |
| Database | SQLite (`vault.db`), WAL mode, schema v6 |

---

## Development

### Prerequisites

- Node.js 22+ and Rust (stable)
- Platform WebView/build dependencies:
  - **Linux (Ubuntu/Debian)**: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev build-essential curl wget file`
  - **Windows**: Visual Studio Build Tools or MinGW-w64
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)

### Install and run

```bash
npm ci
```

`npm run tauri dev` starts both processes: `beforeDevCommand` runs `npm run dev:client` (Vite on port 3000) and the native window loads `http://localhost:3000`. Use `npm run dev:client` on its own if you only need the frontend in a browser.

### Verification

```bash
npm run typecheck                    # TypeScript check
npm run build:client                 # frontend build (dist/)
cd src-tauri && cargo check && cargo test
```

CI (`.github/workflows/ci.yml`) runs these checks on pushes to `main` and on pull requests.

---

## Building releases

| Command | Result |
|---------|--------|
| `npm run build:client` | Frontend only, into `dist/` |
| `npm run tauri build` | Native bundle for the current target (used by CI) |
| `npm run build:desktop` | `sync-version` (from the latest `v*` git tag) then `tauri build` — this rewrites committed version files |

`build:desktop` derives the version from Git tags, so it leaves `package.json` and `tauri.conf.json` modified. Use `npm run tauri build` when you don't want that side effect.

| Platform | Bundle |
|----------|--------|
| Linux | AppImage (`.AppImage`) |
| Windows | NSIS installer (`.exe`) |
| macOS | Universal DMG (`.dmg`) |

Bundles land in `src-tauri/target/<target>/release/bundle/`. macOS must be built on macOS; the CI release workflow (`.github/workflows/release.yml`) builds all three platforms plus `production.tar.gz` on `v*` tags.

---

## Local storage

The app data directory is the OS application-data path (e.g. `~/.local/share/slag3d`, `%APPDATA%\slag3d`, `~/Library/Application Support/slag3d`). It can be overridden with `SLAG3D_APP_DATA_DIR`.

```
<app-data>/
├── vault.db                 SQLite database (schema v6)
├── uploads/
│   ├── models/<id>/         imported model files
│   ├── thumbnails/<id>.png  generated thumbnails
│   └── tmp/                 transient extraction files (not backed up)
└── .backup-work/            backup export/import scratch space
```

Migrations run on every startup in `src-tauri/src/db/schema.rs`. Schema v6 hard-deletes models; legacy `is_deleted` / `deleted_at` columns from the old web-server schema are dropped automatically. See [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md).

---

## Backup & restore

Use the **Backup** button in the top navigation:

- **Create backup** writes a ZIP containing `manifest.json`, a consistent `vault.db` snapshot (`VACUUM INTO`) and `uploads/`. The destination is written via a temporary file and renamed into place, so a failed export cannot truncate an existing backup.
- **Import backup** validates the archive and database, then replaces the current vault. This is a **replace, not a merge** — confirm before restoring. The DB is copied into the live connection with SQLite's online backup API and the uploads tree is swapped; if any step fails, both halves are rolled back.

---

## Documentation

- [AGENTS.md](./AGENTS.md) — developer notes and gotchas
- [SECURITY_GUIDE.md](./SECURITY_GUIDE.md) — current security model
- [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md) — database schema and migrations
- [AUDIT_REPORT.md](./AUDIT_REPORT.md) — historical audit snapshot (not current)
- `docs/superpowers/` — original conversion plan/spec (historical)
