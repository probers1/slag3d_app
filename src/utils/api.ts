import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { appDataDir } from '@tauri-apps/api/path';
import { BackupInfo } from '../types';

// Detect if running inside Tauri at runtime
const isTauri = typeof window !== 'undefined' && ('__TAURI__' in window || '__TAURI_INTERNALS__' in window);

/**
 * Convert a public /uploads/thumbnails/... path to the correct asset URL for the platform.
 * On Linux/macOS: returns slag3d-asset://localhost/uploads/thumbnails/...
 * On Windows: returns http://slag3d-asset.localhost/uploads/thumbnails/... (WebView2 workaround)
 * 
 * NOTE: Only thumbnail paths are converted. Model file paths (/uploads/models/...)
 * are NOT converted because they are loaded via fetchFileAsBlobUrl -> blob URL
 * which uses the read_file Rust command that expects the original path format.
 * 
 * We construct the URL manually instead of using convertFileSrc because that
 * function prepends the origin to the full absolute path, but our custom
 * protocol handler expects the path relative to the uploads directory.
 */
async function toAssetUrl(path: string): Promise<string> {
  if (!isTauri) return path;
  // Only convert thumbnail paths, not model file paths
  if (!path.startsWith('/uploads/thumbnails/')) return path;

  // Extract the relative path from /uploads/ (e.g., /thumbnails/model.png -> thumbnails/model.png)
  const relativePath = path.slice('/uploads/'.length);
  
  // Build the platform-specific origin (matches Tauri's protocol::origin logic)
  // On Windows/Android: http://{scheme}.localhost (or https if use_https_scheme)
  // On Linux/macOS: {scheme}://localhost
  const isWindows = navigator.userAgent.includes('Windows');
  const isAndroid = navigator.userAgent.includes('Android');
  const useHttps = false; // We don't use https scheme
  
  let origin: string;
  if (isWindows || isAndroid) {
    origin = `http://slag3d-asset.localhost`;
  } else {
    origin = `slag3d-asset://localhost`;
  }
  
  // Construct URL with relative path from uploads directory
  return `${origin}/uploads/${relativePath}`;
}

async function mapAssetPaths(value: any): Promise<any> {
  if (typeof value === 'string') {
    // Convert thumbnail paths to custom protocol URLs in Tauri mode
    return toAssetUrl(value);
  }
  if (Array.isArray(value)) {
    return Promise.all(value.map(mapAssetPaths));
  }
  if (value && typeof value === 'object') {
    const out: any = {};
    for (const k in value) {
      out[k] = await mapAssetPaths(value[k]);
    }
    return out;
  }
  return value;
}

export async function apiGetModels(params: {
  page: number;
  limit: number;
  search?: string;
  tag?: string | null;
  collectionId?: string | null;
  filter?: string;
}) {
  const data = await invoke<any>('get_models', {
    page: params.page,
    limit: params.limit,
    search: params.search || null,
    tag: params.tag || null,
    collectionId: params.collectionId || null,
    filter: params.filter || null,
  });
  return await mapAssetPaths(data);
}

export async function apiGetModelDetail(id: string) {
  const data = await invoke<any>('get_model_detail', { id });
  return await mapAssetPaths(data);
}

export async function apiImportModels(filePaths: string[], title?: string, description?: string, tags?: string, collectionId?: string) {
  const data = await invoke<any>('import_models', {
    filePaths,
    title: title || null,
    description: description || null,
    tags: tags || null,
    collectionId: collectionId || null,
  });
  return await mapAssetPaths(data);
}

export async function apiUpdateModel(id: string, payload: { title?: string; description?: string; tags?: string; primaryFileId?: string }) {
  await invoke('update_model', {
    id,
    title: payload.title || null,
    description: payload.description || null,
    tags: payload.tags || null,
    primaryFileId: payload.primaryFileId || null,
  });
}

export async function apiDeleteModel(id: string) {
  await invoke('delete_model', { id });
}

export async function apiDeleteModelFile(modelId: string, fileId: string) {
  await invoke('delete_model_file', { modelId, fileId });
}

export async function apiToggleFavorite(id: string) {
  const isFavorited = await invoke<boolean>('toggle_favorite', { id });
  return { is_favorited: isFavorited };
}

export async function apiSaveThumbnail(id: string, imageBase64: string) {
  // imageBase64 is a data URL, strip prefix if present
  const base64 = imageBase64.startsWith('data:') ? imageBase64.split(',')[1] : imageBase64;
  const path = await invoke<string>('save_thumbnail', { id, imageBase64: base64 });
  return { thumbnail_path: await mapAssetPaths(path) };
}

export async function apiGetCollections() {
  const data = await invoke<any[]>('get_collections');
  return await mapAssetPaths(data);
}

export async function apiCreateCollection(name: string, description: string, parentId: string | null) {
  return await invoke<any>('create_collection', {
    name,
    description,
    parentFolderId: parentId,
  });
}

export async function apiRenameCollection(id: string, name: string) {
  await invoke('rename_collection', { id, name });
}

export async function apiDeleteCollection(id: string) {
  await invoke('delete_collection', { id });
}

export async function apiAddModelToCollection(collectionId: string, modelId: string) {
  await invoke('add_model_to_collection', { collectionId, modelId });
}

export async function apiRemoveModelFromCollection(collectionId: string, modelId: string) {
  await invoke('remove_model_from_collection', { collectionId, modelId });
}

export async function apiGetTags() {
  const data = await invoke<any[]>('get_tags');
  return await mapAssetPaths(data);
}

export const MODEL_FILE_FILTERS = ['stl', '3mf', 'obj', 'zip', '7z', 'pdf'];

export async function pickFiles(multiple = true, filters?: string[]) {
  const res = await openDialog({
    multiple,
    directory: false,
    filters: filters?.map(ext => ({ name: ext, extensions: [ext] })) || [],
  });
  if (!res) return [];
  return Array.isArray(res) ? res : [res];
}

export async function apiInitDb() {
  await invoke('init_db');
}

export async function apiCreateBackup(destination: string) {
  return await invoke<BackupInfo>('create_backup', { destination });
}

export async function apiImportBackup(archivePath: string) {
  return await invoke<BackupInfo>('import_backup', { archivePath });
}

/**
 * Supplementary file upload (mirrors `POST /api/models/{id}/files`).
 *
 * The Express server accepted a multipart FormData body; in Tauri we cannot
 * ship `File` objects over IPC, so we open the native OS dialog here and pass
 * the selected paths straight to the `add_model_files` Rust command.
 */
export async function apiAddModelFiles(modelId: string) {
  const files = await pickFiles(true, MODEL_FILE_FILTERS);
  if (files.length === 0) {
    return { ok: true, added: [] };
  }
  const added = await invoke<any[]>('add_model_files', { modelId, filePaths: files });
  return { ok: true, added };
}

// Read an uploaded asset file via Tauri and return a blob URL.
// Caller must revoke the URL when done (use try/finally).
//
// `read_file` returns a `tauri::ipc::Response` with the raw file bytes. How
// those bytes reach JavaScript depends on which IPC transport Tauri uses:
//
//  - custom `ipc:` protocol  -> ArrayBuffer (fast path)
//  - postMessage fallback    -> JSON array of byte numbers, because Rust
//                               serializes `Vec<u8>` as a JSON array when the
//                               custom protocol is blocked by CSP
//  - older backend builds    -> base64 string
//
// All three must be handled or the 3D viewer silently renders an empty scene.
export async function fetchFileAsBlobUrl(path: string): Promise<string> {
  const data: unknown = await invoke('read_file', { path });

  let bytes: Uint8Array;
  if (typeof data === 'string') {
    const binaryString = atob(data);
    bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }
  } else if (data instanceof ArrayBuffer) {
    bytes = new Uint8Array(data);
  } else if (ArrayBuffer.isView(data)) {
    bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  } else if (Array.isArray(data)) {
    bytes = Uint8Array.from(data as number[]);
  } else {
    throw new Error(`Unexpected read_file response type: ${typeof data}`);
  }

  // Copy into a plain ArrayBuffer: Uint8Array<ArrayBufferLike> is not assignable
  // to BlobPart when its buffer may be a SharedArrayBuffer under the current
  // TypeScript DOM types.
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);

  // The native WebView PDF viewer only engages when the blob advertises the
  // PDF MIME type. Other files (3D models) keep the default octet-stream type.
  const mimeType = path.toLowerCase().endsWith('.pdf') ? 'application/pdf' : undefined;
  return URL.createObjectURL(new Blob([buffer], mimeType ? { type: mimeType } : undefined));
}

export async function apiFetch(url: string, options: RequestInit = {}) {
  const method = (options.method || 'GET').toUpperCase();
  const u = new URL(url, 'http://localhost');
  const pathname = u.pathname;
  const params = u.searchParams;

  const makeResponse = (data: any) => ({
    ok: true,
    status: 200,
    json: async () => data,
    text: async () => JSON.stringify(data),
  });

  try {
    if (pathname === '/api/collections' && method === 'GET') {
      const data = await apiGetCollections();
      return makeResponse({ collections: data });
    }
    if (pathname === '/api/tags' && method === 'GET') {
      const data = await apiGetTags();
      return makeResponse({ tags: data });
    }
    if (pathname === '/api/models' && method === 'GET') {
      const page = Number(params.get('page') || '1');
      const limit = Number(params.get('limit') || '12');
      const search = params.get('search') || undefined;
      const tag = params.get('tag') || null;
      const collection_id = params.get('collection_id') || null;
      const filter = params.get('filter') || 'all';
      const data = await apiGetModels({ page, limit, search, tag, collectionId: collection_id, filter });
      return makeResponse({ models: data.models, pagination: { total: data.total, page: data.page, limit: data.limit, totalPages: Math.ceil(data.total / data.limit) } });
    }
    if (pathname.match(/^\/api\/models\/[^/]+\/favorite$/) && method === 'POST') {
      const id = pathname.split('/')[3];
      const data = await apiToggleFavorite(id);
      return makeResponse(data);
    }
    // POST /api/models/{id}/regenerate-thumbnail — in Tauri, thumbnails are
    // rendered client-side (WebGL) on demand. The ModelCard retry button sets
    // thumbnail_status to 'pending', which triggers the auto-render useEffect.
    // This route is intentionally a no-op here; the real work happens in the
    // browser, not on a server-side queue.
    if (pathname.match(/^\/api\/models\/[^/]+\/regenerate-thumbnail$/) && method === 'POST') {
      return makeResponse({ ok: true });
    }
    // GET /api/models/{id} — model detail (viewer, edit)
    if (pathname.match(/^\/api\/models\/[^/]+$/) && method === 'GET') {
      const id = pathname.split('/')[3];
      const data = await apiGetModelDetail(id);
      return makeResponse({ model: data });
    }
    // PUT /api/models/{id} — update metadata / primary file
    if (pathname.match(/^\/api\/models\/[^/]+$/) && method === 'PUT') {
      const id = pathname.split('/')[3];
      const body = options.body ? JSON.parse(options.body as string) : {};
      await apiUpdateModel(id, {
        title: body.title,
        description: body.description,
        tags: body.tags,
        primaryFileId: body.primary_file_id,
      });
      return makeResponse({ ok: true });
    }
    // DELETE /api/models/{id}
    if (pathname.match(/^\/api\/models\/[^/]+$/) && method === 'DELETE') {
      const id = pathname.split('/')[3];
      await apiDeleteModel(id);
      return makeResponse({ ok: true });
    }
    // DELETE /api/models/{id}/files/{fileId}
    if (pathname.match(/^\/api\/models\/[^/]+\/files\/[^/]+$/) && method === 'DELETE') {
      const parts = pathname.split('/');
      await apiDeleteModelFile(parts[3], parts[5]);
      return makeResponse({ ok: true });
    }
    // POST /api/models/{id}/files — supplementary file upload.
    // The Express server accepted a multipart FormData body; in Tauri we cannot
    // ship File objects over IPC, so we open the native OS dialog here and pass
    // the selected paths straight to the `add_model_files` Rust command.
    if (pathname.match(/^\/api\/models\/[^/]+\/files$/) && method === 'POST') {
      const id = pathname.split('/')[3];
      const data = await apiAddModelFiles(id);
      return makeResponse(data);
    }
    if (pathname === '/api/collections' && method === 'POST') {
      const body = options.body ? JSON.parse(options.body as string) : {};
      await apiCreateCollection(body.name, body.description || '', body.parent_folder_id || null);
      return makeResponse({ ok: true });
    }
    if (pathname.match(/^\/api\/collections\/[^/]+$/) && method === 'PUT') {
      const id = pathname.split('/')[3];
      const body = options.body ? JSON.parse(options.body as string) : {};
      await apiRenameCollection(id, body.name);
      return makeResponse({ ok: true });
    }
    if (pathname.match(/^\/api\/collections\/[^/]+$/) && method === 'DELETE') {
      const id = pathname.split('/')[3];
      await apiDeleteCollection(id);
      return makeResponse({ ok: true });
    }
    if (pathname.match(/^\/api\/collections\/[^/]+\/models$/) && method === 'POST') {
      const collectionId = pathname.split('/')[3];
      const body = options.body ? JSON.parse(options.body as string) : {};
      await apiAddModelToCollection(collectionId, body.model_id);
      return makeResponse({ ok: true });
    }
    if (pathname.match(/^\/api\/collections\/[^/]+\/models\/[^/]+$/) && method === 'DELETE') {
      const parts = pathname.split('/');
      const collectionId = parts[3];
      const modelId = parts[5];
      await apiRemoveModelFromCollection(collectionId, modelId);
      return makeResponse({ ok: true });
    }
    // Fallback for unhandled routes.
    // Fail loudly instead of silently returning 200 — a typo in a pathname
    // regex above would otherwise become a silent no-op that's very hard to
    // debug (see audit issue #12).
    console.warn(`[apiFetch] Unhandled route: ${method} ${pathname}`);
    return { ok: false, status: 404, json: async () => ({ error: `Unhandled Tauri route: ${method} ${pathname}` }), text: async () => '' };
  } catch (e) {
    // Tauri command failures arrive either as a plain string or as the
    // structured AppError envelope ({ kind, message }); normalize to a message.
    const err = e as { kind?: string; message?: string } | string | Error;
    const message =
      typeof err === 'string'
        ? err
        : ((err as { message?: string })?.message || (err instanceof Error ? err.message : '') || 'Request failed');
    return { ok: false, status: 500, json: async () => ({ error: message }) };
  }
}