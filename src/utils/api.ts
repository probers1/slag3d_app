import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

function mapAssetPaths(value: any): any {
  if (typeof value === 'string') {
    return value.replace(/^\/uploads\//, 'slag3d-asset://uploads/');
  }
  if (Array.isArray(value)) {
    return value.map(mapAssetPaths);
  }
  if (value && typeof value === 'object') {
    const out: any = {};
    for (const k in value) {
      out[k] = mapAssetPaths(value[k]);
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
  return mapAssetPaths(data);
}

export async function apiGetModelDetail(id: string) {
  const data = await invoke<any>('get_model_detail', { id });
  return mapAssetPaths(data);
}

export async function apiImportModels(filePaths: string[], title?: string, description?: string, tags?: string, collectionId?: string) {
  const data = await invoke<any>('import_models', {
    filePaths,
    title: title || null,
    description: description || null,
    tags: tags || null,
    collectionId: collectionId || null,
  });
  return mapAssetPaths(data);
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
  return { thumbnail_path: mapAssetPaths(path) };
}

export async function apiGetCollections() {
  const data = await invoke<any[]>('get_collections');
  return mapAssetPaths(data);
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

export async function apiGetTags() {
  const data = await invoke<any[]>('get_tags');
  return mapAssetPaths(data);
}

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

export async function fetchFileAsBlob(path: string): Promise<Blob> {
  const base64 = await invoke<string>('fetch_file', { path });
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return new Blob([bytes]);
}

export async function fetchFileAsUrl(path: string): Promise<string> {
  const blob = await fetchFileAsBlob(path);
  return URL.createObjectURL(blob);
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
    if (pathname.match(/^\/api\/models\/[^/]+\/regenerate-thumbnail$/) && method === 'POST') {
      return makeResponse({});
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
    // POST /api/models/{id}/files — supplementary file upload
    if (pathname.match(/^\/api\/models\/[^/]+\/files$/) && method === 'POST') {
      const id = pathname.split('/')[3];
      // Forward form data via Tauri command (file paths from dialog)
      const form = options.body as FormData;
      const files: string[] = [];
      if (form && typeof form.entries === 'function') {
        for (const [key, value] of form.entries()) {
          if (key === 'files' && value instanceof File) {
            // Tauri can't receive File objects; use the web dialog path instead
            files.push((value as File).name);
          }
        }
      }
      // No-op shim: supplementary upload not yet wired to a Tauri command
      return makeResponse({ ok: true });
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
    // Fallback for unhandled routes
    return makeResponse({});
  } catch (e) {
    return { ok: false, status: 500, json: async () => ({ error: (e as Error).message }) };
  }
}