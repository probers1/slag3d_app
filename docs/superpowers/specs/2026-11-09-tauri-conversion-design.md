# SLAG3D Tauri v2 Desktop Conversion Design

## 1. Intent & Purpose
Convert SLAG3D from a Node.js/Express web server application into a standalone, cross-platform desktop application using **Tauri v2** (Rust backend + React/Three.js frontend). The app will run natively on Linux, Windows, and macOS as a local desktop tool for 3D printing asset management.

Since SLAG3D as a Tauri app runs locally on the user's personal machine, all authentication, session management, user roles, CSRF tokens, rate limiters, and admin panels are removed to simplify the user experience.

---

## 2. Goals & Non-Goals

### Goals
- **Cross-Platform Native Desktop App**: Package SLAG3D for Linux (.AppImage, .deb), Windows (.msi, exe installer), and macOS (.dmg, .app).
- **100% Core Feature Parity**:
  - Multi-file 3D model imports (.stl, .3mf, .obj, .zip, .7z).
  - WebGL 3D model viewer (Three.js with OrbitControls, mesh dimensions, bounding box volume calculations, slice layer count estimation, color customization, multi-file inspection).
  - Tags, nested folder/collections hierarchy, search, filter by tag/collection/favorites.
  - Automatic thumbnail generation via offscreen WebGL rendering inside the app window (replacing Node/Puppeteer server dependency).
  - Local SQLite database storage (`vault.db`) with automatic migrations.
- **Simplified Local Architecture**:
  - Zero authentication prompts (single default local user context).
  - Native OS file picker integration (`tauri-plugin-dialog`).
  - Native filesystem storage in OS standard app data directories (`~/.local/share/slag3d` on Linux, `%APPDATA%\slag3d` on Windows, `~/Library/Application Support/slag3d` on macOS).
  - Custom Tauri protocol / asset handler to serve local 3D files and thumbnails directly to the webview securely and efficiently without CORS or HTTP overhead.

### Non-Goals & Anti-Goals
- Multi-user server hosting or web deployment.
- Rewriting the frontend in a pure Rust GUI crate (e.g. egui/iced) — preserving the polished React + Three.js + Tailwind CSS frontend.
- Bundling Node.js, Express, or Headless Chromium / Puppeteer.

---

## 3. Constraints & Principles
- **Minimal memory footprint**: Tauri v2 uses native OS webviews (WebKitGTK on Linux, WebView2 on Windows, WKWebView on macOS) with ~40-80MB idle RAM usage compared to Electron's 400MB+.
- **Zero bloat**: Use standard Rust crates for SQLite (`rusqlite`), archive extraction (`zip`, `sevenz-rust`), and file operations.
- **Database Backwards Compatibility**: Support existing `vault.db` SQLite files. Database queries will transparently substitute a fixed local user ID (`00000000-0000-0000-0000-000000000000`) for legacy schema columns requiring `user_id`.

---

## 4. Approaches Considered

| Approach | Description | Verdict |
| text | text | text |
| **Option 1: Tauri v2 Desktop App** | Rust backend for SQLite/files/archives + existing React/Three.js frontend in Webview. | **CHOSEN** — Native app feel, OS file dialogs, tiny binary size, single installer per platform. |
| **Option 2: Axum Embedded Server** | Rust web server running on localhost, opening default web browser. | **REJECTED** — Lacks native app window feel; user requested standalone desktop app. |
| **Option 3: Pure Native Rust UI (egui/iced)** | Rewrite entire UI and 3D rendering engine in Rust using `wgpu`. | **REJECTED** — High complexity, total loss of Three.js loaders, shaders, and UI controls. |

---

## 5. System Architecture & Components

```
┌─────────────────────────────────────────────────────────────┐
│                       TAURI V2 APP                          │
├─────────────────────────────────────────────────────────────┤
│  Frontend (React 19 + Tailwind v4 + Three.js)               │
│  - Asset Gallery, Collections Tree, Search & Tag Filters     │
│  - 3D Model Interactive Viewer & WebGL Thumbnail Canvas     │
│  - Native File Upload Dialogs via Tauri API                 │
├─────────────────────────────────────────────────────────────┤
│  IPC Bridge (Tauri Commands / Custom Protocol)              │
│  - slag3d-asset:// (Fast local file streaming)              │
│  - IPC Commands (invoke("get_models"), invoke("import"))    │
├─────────────────────────────────────────────────────────────┤
│  Rust Backend                                               │
│  - SQLite Storage Engine (`rusqlite` + Migration v1..v5)    │
│  - File Storage Manager (App Data Dir)                      │
│  - Archive Processor (`zip` + `sevenz-rust`)                │
└─────────────────────────────────────────────────────────────┘
```

### 5.1 Rust Backend (`src-tauri/`)

#### Core Modules:
1. **`db` (`src-tauri/src/db/mod.rs`)**:
   - Manages SQLite connection pool and WAL mode.
   - Runs schema migrations on launch (v1 through v5).
   - Migration v5: Removes legacy auth constraints or sets default `user_id = '00000000-0000-0000-0000-000000000000'`.
2. **`models` (`src-tauri/src/commands/models.rs`)**:
   - `get_models(page, limit, search, tag, collection_id, filter)`: Paginated search and filtering.
   - `get_model_detail(id)`: Full details, file list, tags, collections.
   - `import_models(file_paths, title, description, tags, collection_id)`: Imports files/archives into local storage and DB.
   - `update_model(id, title, description, tags, primary_file_id)`: Updates metadata.
   - `delete_model(id)`: Removes DB records and deletes files from disk.
   - `toggle_favorite(id)`: Toggles model favorite status.
   - `save_thumbnail(id, base64_png)`: Saves WebGL rendered thumbnail PNG.
3. **`collections` (`src-tauri/src/commands/collections.rs`)**:
   - `get_collections()`: Hierarchy of collections and folders.
   - `create_collection(name, description, parent_id)`.
   - `rename_collection(id, name)`.
   - `delete_collection(id)`.
   - `add_model_to_collection(collection_id, model_id)`.
   - `remove_model_from_collection(collection_id, model_id)`.
4. **`tags` (`src-tauri/src/commands/tags.rs`)**:
   - `get_tags()`: List tags with usage counts.
5. **`archive` (`src-tauri/src/services/archive.rs`)**:
   - Native Rust extraction for `.zip` and `.7z` archives to temporary directory.
   - Filters entries against allowed 3D model extensions (`.stl`, `.3mf`, `.obj`).
6. **`protocol` (`src-tauri/src/protocol.rs`)**:
   - Custom `slag3d-asset://` protocol to safely serve local files (`/uploads/models/...` and `/uploads/thumbnails/...`) to the Webview.

---

### 5.2 Frontend Changes (`src/`)

1. **Remove Auth System**:
   - Delete `src/components/LoginModal.tsx`, `src/components/AdminModal.tsx`, `src/components/ChangePasswordModal.tsx`.
   - Simplify `Navbar.tsx`: remove login status, admin modal trigger, change password button, logout button. Keep upload button, app logo, and main navigation tabs (All Models, Favorites, Collections).
2. **API Client Adaptation (`src/utils/api.ts`)**:
   - Replace Express HTTP `fetch()` calls with Tauri IPC `invoke()` calls, or route through local Tauri command wrapper.
   - Replace `/uploads/...` URL references with `slag3d-asset://...` or Tauri asset protocol URLs.
3. **Thumbnail Engine**:
   - All thumbnails generated on-demand via WebGL in the frontend (`clientThumbnailRenderer.ts`) upon file import, then sent to Rust via `invoke('save_thumbnail', { id, base64 })`.
4. **Native File Upload (`src/components/UploadModal.tsx`)**:
   - Replace web HTML file input with Tauri's native OS file open dialog (`@tauri-apps/plugin-dialog`), allowing selection of large 3D files/archives directly from disk without copying through browser memory buffers.

---

## 6. Project Directory Layout

```
slag3D_app/
├── src-tauri/                 # Rust Desktop Application Backend
│   ├── Cargo.toml
│   ├── tauri.conf.json        # Tauri v2 configuration
│   ├── build.rs
│   ├── icons/                 # Platform application icons (.icns, .ico, .png)
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

## 7. Migration & Compatibility Strategy

When launching the Tauri app, it locates or creates `vault.db` and the `uploads/` directory inside the OS user application storage directory:
- Linux: `~/.local/share/slag3d`
- Windows: `%APPDATA%\slag3d`
- macOS: `~/Library/Application Support/slag3d`

If an existing Node.js SLAG3D database (`vault.db`) is present, Tauri will automatically run **Migration v5**, which:
1. Hardcodes or updates any `user_id` foreign keys to default user `00000000-0000-0000-0000-000000000000`.
2. Ensures all orphaned models remain accessible in the local library.

---

## 8. Development & Build Pipeline

1. **Development Mode**: `npm run tauri dev`
   - Launches Vite dev server for React frontend.
   - Compiles and runs Rust Tauri backend with hot reloading.
2. **Production Build**: `npm run tauri build`
   - Bundles optimized React SPA.
   - Compiles native executable with embedded frontend assets.
   - Generates installer packages:
     - Linux: `.AppImage`, `.deb`
     - Windows: `.msi`, `.exe` (NSIS)
     - macOS: `.dmg`, `.app`
