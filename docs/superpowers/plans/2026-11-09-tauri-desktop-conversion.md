# SLAG3D Tauri v2 Desktop Conversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert SLAG3D into a standalone, cross-platform Tauri v2 desktop app (Linux, Windows, macOS) with 100% core asset management functionality preserved and authentication removed.

**Architecture:** Rust backend handling SQLite database operations (`rusqlite`), native zip/7z archive extractions, file storage, and custom protocol streaming (`slag3d-asset://`). React 19 + Three.js + Tailwind CSS frontend running in Tauri WebView with client-side WebGL thumbnail rendering and OS file dialogs via Tauri plugins.

**Tech Stack:** Tauri v2, Rust (1.93+), React 19, TypeScript 5+, Vite 6+, Tailwind CSS 4+, Three.js, SQLite (`rusqlite`).

**Spec:** `docs/superpowers/specs/2026-11-09-tauri-conversion-design.md`

## Global Constraints
- Target platforms: Linux (.AppImage, .deb), Windows (.msi), macOS (.dmg).
- Fixed local user ID: `00000000-0000-0000-0000-000000000000`.
- Allowed 3D file formats: `.stl`, `.3mf`, `.obj`.
- Allowed archive formats: `.zip`, `.7z`.
- Local App Storage Location: OS standard application data directory (`app_data_dir/slag3d`).
- No Node.js / Express / Puppeteer dependencies in desktop build.

## Review Focus
- **SQLite Database Initialization & Migrations**: Ensure `vault.db` auto-creates tables and migrates legacy schemas without data loss.
- **Archive Extraction Safety**: Reject malicious or invalid archive entries during zip/7z extraction.
- **Custom Asset Protocol Streaming**: Stream large binary 3D model files (.stl up to 300MB) smoothly over `slag3d-asset://` without buffer truncation.
- **WebGL Client Thumbnail Generation**: Render 512x512 thumbnails in WebGL canvas and save PNG files directly to Rust storage.
- **Native OS Dialog File Import**: Support selecting multiple files or archives natively via OS file dialogs without browser memory limits.

---

### Task 1: Tauri v2 Project Setup & Configuration

**Files:**
- Modify: `package.json`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Modify: `vite.config.ts`

**Interfaces:**
- Consumes: Vite frontend build output (`dist/`).
- Produces: Runnable Tauri application bundle configuration and Rust entrypoint.

- [ ] **Step 1: Update `package.json` with Tauri v2 CLI dependencies**

Add `@tauri-apps/cli` and `@tauri-apps/api` and `@tauri-apps/plugin-dialog` to `package.json`.

- [ ] **Step 2: Create `src-tauri/Cargo.toml`**

Configure Rust package `slag3d` with dependencies: `tauri` (v2), `tauri-plugin-dialog`, `rusqlite` (bundled feature enabled), `serde`, `serde_json`, `uuid` (v4, v4 feature enabled), `zip`, `sevenz-rust`, `tokio`, `image`, `mime_guess`.

- [ ] **Step 3: Create `src-tauri/tauri.conf.json`**

Set app identifier `com.slag3d.app`, title `SLAG3D - 3D Vault`, window dimensions (1280x800), asset protocol handler `slag3d-asset`, and frontend build target (`../dist`).

- [ ] **Step 4: Create `src-tauri/build.rs`, `src/main.rs`, and `src/lib.rs`**

Set up standard Tauri v2 builder invocation in `lib.rs` and `main.rs`.

- [ ] **Step 5: Verify Cargo compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS with 0 errors.

- [ ] **Step 6: Commit**

```bash
git add package.json vite.config.ts src-tauri/
git commit -m "feat(tauri): scaffold Tauri v2 Rust project and build configuration"
```

---

### Task 2: SQLite Database Manager & Migrations (`src-tauri/src/db/`)

**Files:**
- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/schema.rs`
- Test: `src-tauri/src/db/tests.rs`

**Interfaces:**
- Consumes: `vault.db` path in application storage directory.
- Produces: `DbState` struct wrapping thread-safe `Mutex<Connection>` or connection pool, exposing CRUD methods for models, files, tags, and collections.

- [ ] **Step 1: Write failing Rust database test**

Create `src-tauri/src/db/tests.rs` testing database schema creation and migration v1..v5:

```rust
#[test]
fn test_db_init_and_migrations() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    let version: i32 = conn.query_row("SELECT version FROM schema_versions WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(version, 5);
}
```

- [ ] **Step 2: Run Rust DB test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_db_init_and_migrations`
Expected: FAIL (unresolved `init_schema`).

- [ ] **Step 3: Implement `init_schema(&Connection) -> Result<()>` in `src-tauri/src/db/schema.rs`**

Create SQLite table definitions (`models`, `files`, `tags`, `model_tags`, `favorites`, `collections`, `collection_models`, `schema_versions`). Apply migration v5 to default `user_id` to `'00000000-0000-0000-0000-000000000000'`. Enable `PRAGMA journal_mode=WAL` and `PRAGMA foreign_keys=ON`.

- [ ] **Step 4: Implement DB query helper functions in `src-tauri/src/db/mod.rs`**

Implement model listing (with pagination, title/desc search, tag filter, collection filter, favorite filter), model detail fetch, model insertion/update/deletion, collection CRUD, and tag fetching.

- [ ] **Step 5: Run Rust DB tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS (all DB tests passing).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/
git commit -m "feat(tauri): implement SQLite schema manager and migrations v1..v5 in Rust"
```

---

### Task 3: Storage Directory Manager & Archive Extraction Service (`src-tauri/src/services/`)

**Files:**
- Create: `src-tauri/src/services/storage.rs`
- Create: `src-tauri/src/services/archive.rs`
- Test: `src-tauri/src/services/tests.rs`

**Interfaces:**
- Consumes: User filesystem file paths (`.zip`, `.7z`, `.stl`, `.3mf`, `.obj`).
- Produces: Structured `ExtractedFile` structs with safe destination paths inside `app_data_dir/slag3d/uploads/models/<model_id>/`.

- [ ] **Step 1: Write failing Rust unit test for archive security validation**

Create `src-tauri/src/services/tests.rs` testing file extension allowlisting:

```rust
#[test]
fn test_is_allowed_3d_extension() {
    assert!(is_allowed_extension("box.stl"));
    assert!(is_allowed_extension("component.3MF"));
    assert!(is_allowed_extension("statue.obj"));
    assert!(!is_allowed_extension("script.sh"));
    assert!(!is_allowed_extension("image.png"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_is_allowed_3d_extension`
Expected: FAIL.

- [ ] **Step 3: Implement `storage.rs` and `archive.rs`**

- `storage.rs`: Get or initialize `UPLOADS_DIR`, `MODELS_DIR`, `THUMBNAILS_DIR`, `TMP_DIR` using `tauri::path::PathResolver`.
- `archive.rs`: Write `extract_zip` (using `zip` crate) and `extract_7z` (using `sevenz-rust` crate) to extract files matching `.stl`, `.3mf`, `.obj` into the model's dedicated directory.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/
git commit -m "feat(tauri): implement storage manager and zip/7z extraction service"
```

---

### Task 4: Custom Asset Protocol Handler (`src-tauri/src/protocol.rs`)

**Files:**
- Create: `src-tauri/src/protocol.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: URI requests starting with `slag3d-asset://localhost/...` or `slag3d-asset://...` from WebView.
- Produces: Binary HTTP Response stream with content-type (`application/sla`, `model/3mf`, `model/obj`, `image/png`).

- [ ] **Step 1: Implement `register_asset_protocol` in `src-tauri/src/protocol.rs`**

Use Tauri v2's `register_uri_scheme_protocol("slag3d-asset", ...)` to decode public paths (e.g. `/uploads/models/{id}/{file}`) into absolute filesystem paths in `UPLOADS_DIR` and return byte stream responses with MIME types.

- [ ] **Step 2: Register protocol in `lib.rs` builder**

Connect `register_asset_protocol` inside `tauri::Builder::default()`.

- [ ] **Step 3: Cargo compilation check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/protocol.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): add custom slag3d-asset URI protocol handler"
```

---

### Task 5: Tauri IPC Commands Implementation (`src-tauri/src/commands/`)

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/models.rs`
- Create: `src-tauri/src/commands/collections.rs`
- Create: `src-tauri/src/commands/tags.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: React frontend `invoke()` calls.
- Produces: JSON serializable responses matching SLAG3D data types (`ModelItem`, `CollectionItem`, `TagItem`).

- [ ] **Step 1: Implement `src-tauri/src/commands/models.rs`**

Add Tauri command functions annotated with `#[tauri::command]`:
- `get_models(page: u32, limit: u32, search: Option<String>, tag: Option<String>, collection_id: Option<String>, filter: Option<String>) -> Result<PaginatedModels, String>`
- `get_model_detail(id: String) -> Result<ModelDetail, String>`
- `import_models(file_paths: Vec<String>, title: Option<String>, description: Option<String>, tags: Option<String>, collection_id: Option<String>) -> Result<ModelDetail, String>`
- `update_model(id: String, title: Option<String>, description: Option<String>, tags: Option<String>, primary_file_id: Option<String>) -> Result<(), String>`
- `delete_model(id: String) -> Result<(), String>`
- `toggle_favorite(id: String) -> Result<bool, String>`
- `save_thumbnail(id: String, image_base64: String) -> Result<String, String>`

- [ ] **Step 2: Implement `src-tauri/src/commands/collections.rs` and `tags.rs`**

- `get_collections() -> Result<Vec<CollectionItem>, String>`
- `create_collection(name: String, description: Option<String>, parent_folder_id: Option<String>) -> Result<CollectionItem, String>`
- `rename_collection(id: String, name: String) -> Result<(), String>`
- `delete_collection(id: String) -> Result<(), String>`
- `add_model_to_collection(collection_id: String, model_id: String) -> Result<(), String>`
- `remove_model_from_collection(collection_id: String, model_id: String) -> Result<(), String>`
- `get_tags() -> Result<Vec<TagItem>, String>`

- [ ] **Step 3: Register commands in `lib.rs`**

Add all commands to `.invoke_handler(tauri::generate_handler![...])` in `lib.rs`.

- [ ] **Step 4: Cargo test and check**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs
git commit -m "feat(tauri): implement all IPC backend commands for models, collections, and tags"
```

---

### Task 6: Frontend Refactoring - Remove Auth & Adapt API Bridge

**Files:**
- Modify: `src/utils/api.ts`
- Modify: `src/types.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/Navbar.tsx`
- Modify: `src/components/UploadModal.tsx`
- Modify: `src/components/ModelViewerModal.tsx`
- Modify: `src/utils/clientThumbnailRenderer.ts`
- Delete: `src/components/LoginModal.tsx`
- Delete: `src/components/AdminModal.tsx`
- Delete: `src/components/ChangePasswordModal.tsx`

**Interfaces:**
- Consumes: `@tauri-apps/api/core` `invoke()` and `@tauri-apps/plugin-dialog`.
- Produces: Auth-free React UI connected to Rust Tauri IPC backend.

- [ ] **Step 1: Rewrite `src/utils/api.ts` to bridge to Tauri IPC**

Update `apiFetch` or create direct typed Tauri command helpers (`apiGetModels`, `apiImportModels`, etc.) using `invoke()`. Map filesystem URL responses (`/uploads/...`) to `slag3d-asset://uploads/...`.

- [ ] **Step 2: Delete auth modal components**

Delete `src/components/LoginModal.tsx`, `src/components/AdminModal.tsx`, `src/components/ChangePasswordModal.tsx`.

- [ ] **Step 3: Simplify `Navbar.tsx` and `App.tsx`**

Remove auth checking state (`currentUser`, `checkAuth`, `isLoginOpen`, `isAdminOpen`, `isPasswordModalOpen`). Set default user context as active automatically. Simplify navigation bar to show title, upload button, and model filter tabs.

- [ ] **Step 4: Update `UploadModal.tsx` for native OS file selection**

Replace HTML `<input type="file">` with `@tauri-apps/plugin-dialog` `open()` call, allowing multi-file selection of `.stl`, `.3mf`, `.obj`, `.zip`, `.7z`. Pass selected file paths directly to Rust `import_models` command.

- [ ] **Step 5: Update `clientThumbnailRenderer.ts` and `ModelViewerModal.tsx`**

Ensure browser WebGL thumbnail renderer sends base64 PNG data to Rust command `save_thumbnail` via `invoke('save_thumbnail', { id, imageBase64 })`.

- [ ] **Step 6: Run TypeScript typecheck**

Run: `npm run typecheck`
Expected: PASS with 0 errors.

- [ ] **Step 7: Commit**

```bash
git add src/
git rm src/components/LoginModal.tsx src/components/AdminModal.tsx src/components/ChangePasswordModal.tsx
git commit -m "refactor(frontend): adapt API client to Tauri IPC, remove auth components, add native file dialogs"
```

---

### Task 7: Full Application Build & Desktop Packaging Verification

**Files:**
- Modify: `package.json` (add scripts: `"tauri": "tauri"`, `"build:desktop": "npm run build && tauri build"`)
- Test: Full build pipeline

**Interfaces:**
- Consumes: Complete project source code.
- Produces: Compiled standalone executable and desktop installers.

- [ ] **Step 1: Run full frontend build**

Run: `npm run build`
Expected: Dist directory successfully outputted with Vite bundle.

- [ ] **Step 2: Run Rust cargo tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All backend unit tests PASS.

- [ ] **Step 3: Run Tauri compilation check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS with 0 warnings or errors.

- [ ] **Step 4: Commit build configuration**

```bash
git add package.json
git commit -m "chore: configure Tauri desktop build scripts and finalize packaging readiness"
```
