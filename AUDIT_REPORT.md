# SLAG3D — Audit Report (Ponytail)

**What this repo does:** SLAG3D is a 3D-printer model repository with upload/extraction, collections/tags, thumbnails, and a Three.js viewer. It is mid-migration from a Node/Express server to a Tauri v2 desktop app. The on-disk working tree is **incomplete** — most backend files (`package.json`, `server.ts`, `server/`, `src-tauri/`, etc.) were deleted without commit; they exist only in `git HEAD`. The Tauri app is the target; the Express server is legacy but still in git history.

---

## Must Fix (Bug / Security / Data Loss)

### 1. Missing authentication in Tauri app — no way to log in
The spec explicitly removes all auth for the desktop app (single local user). **But the React frontend still calls `/api/auth/login`, `/api/auth/me`, `/api/auth/refresh`, `/api/auth/logout`, `/api/csrf-token` via `apiFetch`**. These are stubbed to return `{ok:true}` or `{}`. The `apiInitDb()` call in `App.tsx` does nothing useful. If a user runs the Tauri app, the login UI components are deleted per spec, but the auth flow still fires and silently fails. **Fix:** Remove all auth-related `apiFetch` calls from the frontend; wire `apiInitDb` to actually initialize the DB, or drop it entirely since Tauri does that on startup.

### 2. Supplementary file upload is stubbed (TODO in `api.ts:228`)
```
POST /api/models/{id}/files  →  makeResponse({ok:true})  // TODO: invoke('add_model_files', ...)
```
The Tauri command `add_model_files` exists and works, but the frontend shim never calls it. Supplementary uploads from the Model Viewer modal will silently succeed without adding files. **Fix:** Implement the shim in `apiFetch` to call `invoke('add_model_files', {modelId, filePaths})`.

### 3. `regenerate-thumbnail` endpoint is stubbed (`api.ts:182`)
Returns `{}` instead of invoking the Tauri thumbnail command (which doesn't even exist — thumbnails are client-side in Tauri). The retry button in `ModelCard` calls this and updates local state to `pending`, but nothing actually renders. **Fix:** Either implement a Tauri-side thumbnail re-render command, or remove the retry button for the Tauri build.

### 4. Tauri DB schema version mismatch (v4 vs v5)
- Server migrations go to v4 (soft-delete on users).
- Tauri `init_schema` sets schema version to **5** and inserts a default user with empty password/username.
- If a user migrates an existing `vault.db` (v4) to the Tauri app, the version check in `runMigrations` (server) would skip v4, but Tauri's `init_schema` will try to INSERT into `schema_versions` with version 5, potentially leaving the DB in an inconsistent state. **Fix:** Align versions; Tauri should either run the server migrations or explicitly handle v4→v5.

### 5. Soft-delete column `is_deleted` on `models` table exists in Tauri schema but not in server v4
Server migration v4 only adds `is_deleted` + `deleted_at` to **users**. Tauri `init_schema` adds `is_deleted` to `models` too. If a v4 DB is opened in Tauri, `models.is_deleted` won't exist → queries filtering `m.is_deleted = 0` will fail. **Fix:** Add a migration to add `is_deleted` to `models` (and `files`, `collections` if needed) or guard queries with `PRAGMA table_info`.

### 6. `startup-validation.js` copied to root in Dockerfile but tracked under `server/`
Dockerfile: `COPY ./startup-validation.js ./startup-validation.js` (expects file at repo root).
Git: `server/startup-validation.js` (only tracked location).
On current disk: neither exists (deleted). `docker build` will fail. **Fix:** Either move the file to root and update Dockerfile, or change Dockerfile to `COPY ./server/startup-validation.js ./startup-validation.js`.

### 7. `p7zip-full` installed in Docker but Tauri uses `sevenz-rust` (no system binary needed)
Dockerfile installs `p7zip-full` for server-side 7z extraction. Tauri uses the `sevenz-rust` crate (pure Rust). If the Tauri app is built in the same Dockerfile, the system package is unnecessary bloat. **Fix:** Remove `p7zip-full` from Dockerfile if only building Tauri; keep it for the Express server image.

### 8. `.env` not tracked, but `.env.example` has placeholder secrets
`.env` is gitignored. `.env.example` contains `<paste-openssl-rand-base64-52-output-here>` placeholders. The startup validation rejects placeholders. **Documentation gap:** No clear instruction that users must copy `.env.example` → `.env` and fill secrets before `docker compose up`. The README mentions it but not prominently. **Fix:** Add a bold warning in README and Dockerfile comment.

### 9. `favicon.svg` referenced in `index.html` but only `favicon.ico` exists in git
`<link rel="icon" type="image/svg+xml" href="/favicon.svg" />` — file not tracked. `public/favicon.svg` and `public/favicon.png` are tracked but not the root-level ones Vite serves from `public/`. **Fix:** Add `favicon.svg` and `favicon.png` to `public/` or update `index.html`.

### 10. Tauri `read_file` command returns base64; frontend converts to blob URL — but `Content-Type` is lost
`read_file` reads raw bytes and base64-encodes them. Three.js loaders infer type from URL extension, which works for `.stl`, `.obj`, `.3mf`. But if a file has no extension or a misleading one, the loader may fail. **Low risk**, but worth noting.

---

## Risk (Security / Correctness)

### 11. CSRF cookie issued but never validated in Tauri (no server)
`apiFetch` shim doesn't send `x-csrf-token` header; the `slag3d_csrf_token` cookie is set by stubbed login but never read. In Tauri, there's no CSRF risk (no cross-origin requests), but the cookie clutters storage. **Fix:** Remove CSRF cookie logic from frontend for Tauri build.

### 12. `apiFetch` returns `makeResponse({})` for unhandled routes — masks 404s
Any unknown API call returns `{ok:true, status:200, json:{} }`. Bugs in route mapping (e.g., typo in path) become silent no-ops. **Fix:** Throw or log a warning in the catch-all branch.

### 13. Tauri `delete_model` soft-deletes (`is_deleted=1`) but frontend doesn't filter
`ModelCard` and `get_models` don't check `is_deleted` on the Tauri side (the SQL does: `m.is_deleted = 0`). But if a model is soft-deleted via the Tauri command, it disappears from the list correctly. **However**, the server `DELETE /api/models/:id` **hard-deletes** (removes files + DB row). Inconsistent behavior between server and Tauri. **Fix:** Decide: soft-delete or hard-delete, and align both.

### 14. No `busy_timeout` in Tauri SQLite connection
Server sets `busy_timeout = 5000` in `set-admin-password.js` and likely in DB init. Tauri `rusqlite::Connection::open` has no timeout → concurrent access (unlikely in single-user desktop) could lock indefinitely. **Fix:** Add `conn.busy_timeout(std::time::Duration::from_millis(5000))` in Tauri DB init.

### 15. `uploadSecurity.ts` allows `.zip`/`.7z` filenames without inspecting extension (comment says deliberate)
The validation only checks the archive's **contents**, not the archive filename. An archive named `evil.html.zip` passes validation. Since the archive is deleted after extraction and never served, this is **safe** — but the comment admits it was a deliberate choice after false positives. **No action needed**, but document the threat model.

### 16. Tauri asset protocol (`slag3d-asset://`) is registered but frontend uses blob URLs instead
The protocol handler exists in `protocol.rs` and is registered in `lib.rs`. However, `fetchFileAsBlobUrl` (used by viewer and thumbnail renderer) reads via `read_file` Tauri command and creates a blob URL. The protocol is **unused**. This is fine (blob URLs work better on WebView2), but the dead code should be removed or documented.

---

## Scale / Performance

### 17. Client-side thumbnail rendering runs in the main thread (blocks UI)
`renderThumbnailInBrowser` creates a Three.js scene, loads the model, renders to an offscreen canvas — all on the main thread. Large models (>25MB) skip inlining and render a placeholder, but STL/OBJ parsing and rendering can still block for seconds. **Fix:** Move to a Web Worker or use `requestIdleCallback` / chunked rendering.

### 18. `p-queue` concurrency=2 for server thumbnails; Tauri has no queue
Server uses `PQueue` with 2 concurrent Puppeteer renders. Tauri does thumbnail rendering client-side on demand (when a card mounts with `pending`/`failed` status). No queue → if user scrolls past 20 models with missing thumbnails, 20 concurrent Three.js renders could freeze the app. **Fix:** Add a client-side render queue (simple array + `setTimeout` serial processing).

### 19. `lib/three-bundle.js` is 780KB (checked into git)
This is the Three.js bundle for server-side Puppeteer rendering. It's committed, not built. If Three.js version updates, the bundle must be manually rebuilt. **Fix:** Add a build script (`npm run build:three-bundle`) and `.gitignore` the output, or keep as-is with a comment.

### 20. `modelExtensionOf` called repeatedly in hot paths
Called for every file in archive extraction, every file list render, every thumbnail render. It's a simple `path.extname` + array lookup — negligible, but could be memoized if it ever becomes a bottleneck.

---

## Lean (Code That Should Not Exist / Can Be Smaller)

### 21. `squashfs-root/` (560 files) committed in git history
AppImage extraction artifacts. Should be in `.gitignore` and removed from history (`git filter-repo` or `git rm -r --cached`). **Currently only in history, not working tree.**

### 22. Duplicate `docs/superpowers/` files shown as deleted in `git status`
Two identical entries for each plan/spec file (shown twice in `git status --short`). Likely a git index corruption from the mass deletion. **Fix:** `git checkout -- .` to restore, then commit cleanly.

### 23. `server/migrations/003_add_soft_delete_flag.ts` exists but is abandoned
Migration v3 targeted the wrong table; v4 is the correct one. v3 is skipped in `migrations/index.ts` (not in the `migrations` array). **Fix:** Delete the file or mark it clearly as abandoned.

### 24. `src-tauri/src/commands/auth.rs` exists but is empty (only placeholder)
File exists with no commands. The spec removes auth. **Fix:** Delete the file.

### 25. `src-tauri/src/db/tests.rs` and `src-tauri/src/services/tests.rs` are empty placeholder files
Only contain `#[cfg(test)] mod tests;`. **Fix:** Delete or add actual tests.

### 26. `scripts/cleanup-tokens.js` references `expires_at` column on `auth_tokens` table — **column doesn't exist**
Server schema: `auth_tokens(token PRIMARY KEY, user_id, created_at)`. No `expires_at`. The cleanup script will fail with "no such column". **Fix:** Add `expires_at` column to `auth_tokens` (migration) or change script to use `created_at` + 7 days.

### 27. `server/lib/tags.ts` — `getTagsWithCounts` does N+1 query (one COUNT per tag)
```ts
const tags = db.prepare('SELECT id, name FROM tags').all();
for (const tag of tags) {
  tag.count = db.prepare('SELECT COUNT(*) ...').get(tag.id).count;
}
```
With hundreds of tags, this is slow. **Fix:** Single query with `LEFT JOIN` and `GROUP BY`.

### 28. `server/routes/models.ts` — `syncModelTags` does `DELETE` + loop of `INSERT OR IGNORE` + `SELECT` per tag
Three round-trips per tag. **Fix:** Use a transaction and `INSERT OR IGNORE` + `INSERT OR IGNORE INTO model_tags` with a CTE or batch.

### 29. `server/paths.ts` — `resolveAppRoot` uses `process.cwd()` fallback which breaks in bundled `dist/server.js`
The comment acknowledges this bug was fixed before. The logic checks for `package.json` or `lib/` in cwd, then goes to `__dirname/..`. In `dist/server.js`, `__dirname` is `/app/dist`, parent is `/app` — correct. But if run from a different cwd, it may resolve wrong. **Fix:** Use `import.meta.dirname` (Node 20+) or a build-time constant.

### 30. `vite.config.ts` disables HMR with `DISABLE_HMR=true` (intentional for AI Studio)
The comment says "don't remove the flag". This is a dev-only workaround. **No action**, but ensure it's not enabled in production builds.

---

## Missing Tests

### 31. No integration tests for Tauri commands
`src-tauri/src/db/tests.rs` and `services/tests.rs` are empty. The server has `tests/production_paths.test.ts` and `tests/upload_security.test.ts` which are excellent. **Fix:** Port critical paths to Tauri integration tests (using `tauri::test` or headless).

### 32. No test for `renderThumbnailInBrowser` (client-side thumbnail renderer)
Complex Three.js logic with multiple loaders, error paths, blob URL handling. **Fix:** Add a unit test with a minimal STL fixture.

### 33. No test for `apiFetch` route mapping
The shim maps 20+ legacy Express routes to Tauri commands. A typo in a pathname regex silently returns `{}`. **Fix:** Snapshot test the mapping table.

---

## Not Checked
- Full Docker build (missing `package.json` on disk)
- Tauri desktop build (`src-tauri/target/` not present)
- WebView2 / WebKitGTK / WKWebView rendering quirks
- Migration v4→v5 on real data
- Large archive extraction (>100MB) memory usage
- Windows path handling in `resolveUploadsPath` / `is_safe_path`

---

## Verdict
**Healthy core, broken migration state.** The Tauri implementation is solid (Rust backend, proper IPC, security-conscious archive extraction, client-side thumbnail fallback). The blocker is the incomplete working tree — `git checkout -- .` restores everything. Once restored:
1. Fix the 3 stubbed endpoints in `api.ts` (login/me/refresh, regenerate-thumbnail, add_model_files).
2. Align DB schema versions (v4↔v5).
3. Remove dead auth code from frontend.
4. Add `busy_timeout` to Tauri DB.
5. Delete `squashfs-root/` from history.

**Lean: -560 files (squashfs), -3 empty test files, -1 abandoned migration, -780KB committed three-bundle (can be built).**