// src/components/ModelViewerModal.tsx
import React, {
  useEffect,
  useRef,
  useState,
  useCallback,
  useMemo,
} from 'react';
import * as THREE from 'three';
import { ErrorBoundary } from './ErrorBoundary';
import { PdfViewerModal } from './PdfViewerModal';
import {
  X,
  Camera,
  Heart,
  Download,
  FolderPlus,
  RefreshCw,
  Box,
  Scale,
  Check,
  FileCode,
  FileText,
  Edit2,
  Trash2,
  Eye,
  Star,
  Upload,
} from 'lucide-react';
import { ModelFile, CollectionItem } from '../types';
import { Dropdown } from './Dropdown';
import { invoke } from '@tauri-apps/api/core';
import { apiFetch, apiSaveThumbnail, fetchFileAsBlobUrl, apiAddModelFiles } from '../utils/api';
import { useModelViewerScene, MeshStats } from '../hooks/useModelViewerScene';

const is3DFile = (file: ModelFile) =>
  ['stl', '3mf', 'obj'].includes(file.original_name.split('.').pop()?.toLowerCase() || '');

interface ModelDetail {
  id: string;
  user_id: string;
  title: string;
  description: string;
  thumbnail_path: string | null;
  thumbnail_status: 'pending' | 'processing' | 'completed' | 'failed';
  created_at: string;
  updated_at: string;
  owner_username: string;
  is_favorited: boolean;
  tags: string[];
  files: ModelFile[];
  collections: { id: string; name: string }[];
}

interface ModelViewerModalProps {
  modelId: string;
  collections: CollectionItem[];
  currentUserId?: string;
  isAdmin?: boolean;
  onClose: () => void;
  onModelUpdated: (newThumbnailPath?: string | null) => void;
}

export const ModelViewerModal: React.FC<ModelViewerModalProps> = ({
  modelId,
  collections,
  currentUserId,
  isAdmin,
  onClose,
  onModelUpdated,
}) => {
  const mountRef = useRef<HTMLDivElement>(null);

  // Model data state
  const [model, setModel] = useState<ModelDetail | null>(null);
  const [selectedFileId, setSelectedFileId] = useState<string | null>(null);
  const [pdfPreview, setPdfPreview] = useState<ModelFile | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMesh, setLoadingMesh] = useState(false);
  const [meshError, setMeshError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saveToast, setSaveToast] = useState<string | null>(null);

  // 3D Canvas states
  const [wireframe, setWireframe] = useState(false);
  const [isCapturingThumb, setIsCapturingThumb] = useState(false);
  const [meshStats, setMeshStats] = useState<MeshStats>({
    widthMm: 0,
    depthMm: 0,
    heightMm: 0,
    volumeCm3: 0,
    weightGrams: 0,
  });

  const [filamentPriceKg, setFilamentPriceKg] = useState<number>(20.0); // $20/kg default

  // Derive filament cost dynamically to avoid state loop re-renders
  const estimatedCostUsd = useMemo(() => {
    if (meshStats.weightGrams <= 0) return 0;
    return Number(((meshStats.weightGrams / 1000) * filamentPriceKg).toFixed(2));
  }, [meshStats.weightGrams, filamentPriceKg]);

  // Editing states
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState('');
  const [editDesc, setEditDesc] = useState('');
  const [editTags, setEditTags] = useState('');
  const [selectedCollectionToAdd, setSelectedCollectionToAdd] = useState('');

  // WebKitGTK drops window.confirm (Tauri installs no script-dialog handler), so
  // destructive actions use an in-app two-step confirm instead.
  const [confirmDeleteModel, setConfirmDeleteModel] = useState(false);
  const [confirmDeleteFileId, setConfirmDeleteFileId] = useState<string | null>(null);

  // Clear file delete confirmation when clicking a different file's trash button
  const clearFileConfirm = useCallback(() => {
    setConfirmDeleteFileId(null);
  }, []);

  const {
    sceneRef,
    cameraRef,
    rendererRef,
    controlsRef,
    activeMeshRef,
    loadActiveMesh,
  } = useModelViewerScene(mountRef);

  // Fetch model details
  const fetchModel = async () => {
    try {
      setLoading(true);
      const res = await apiFetch(`/api/models/${modelId}`);
      if (!res.ok) throw new Error('Failed to load model details');
      const data = await res.json();
      setModel(data.model);
      setEditTitle(data.model.title);
      setEditDesc(data.model.description || '');
      setEditTags(data.model.tags ? data.model.tags.join(', ') : '');

      if (data.model.files && data.model.files.length > 0) {
        setSelectedFileId((prev) => {
          if (prev && data.model.files.some((f: ModelFile) => f.id === prev)) {
            return prev;
          }
          const primary = data.model.files.find((f: ModelFile) => f.is_primary_3d === 1 && is3DFile(f))
            || data.model.files.find((f: ModelFile) => is3DFile(f));
          return primary ? primary.id : null;
        });
      }
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchModel();
  }, [modelId]);

  // When model or selectedFileId changes, load that file into the Three.js scene
  useEffect(() => {
    if (!model || !model.files || model.files.length === 0) return;
    const activeFile = model.files.find((f) => f.id === selectedFileId)
      || model.files.find((f) => f.is_primary_3d === 1);
    if (activeFile && is3DFile(activeFile)) {
      void loadActiveMesh(activeFile, {
        wireframe,
        setLoadingMesh,
        setMeshError,
        setMeshStats,
      });
    }
  }, [model, selectedFileId, loadActiveMesh, wireframe]);

  // Toggle wireframe mode dynamically
  const toggleWireframe = () => {
    setWireframe((prev) => {
      const next = !prev;
      if (activeMeshRef.current) {
        activeMeshRef.current.traverse((child) => {
          if ((child as THREE.Mesh).isMesh && (child as THREE.Mesh).material) {
            const mat = (child as THREE.Mesh).material as THREE.MeshStandardMaterial;
            mat.wireframe = next;
          }
        });
      }
      return next;
    });
  };

  // Reset Camera View
  const resetCamera = () => {
    if (!controlsRef.current || !activeMeshRef.current || !cameraRef.current) return;
    const box = new THREE.Box3().setFromObject(activeMeshRef.current);
    const size = box.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z) || 50;
    const fov = 45 * (Math.PI / 180);
    const cameraZ = Math.abs(maxDim / 2 / Math.tan(fov / 2)) * 1.5;

    cameraRef.current.position.set(cameraZ * 0.7, cameraZ * 0.5, cameraZ * 0.7);
    cameraRef.current.lookAt(0, size.y / 4, 0);
    controlsRef.current.target.set(0, size.y / 4, 0);
    controlsRef.current.update();
  };

  // "Save as Thumbnail" custom angle snapshot capture
  const handleSaveThumbnail = async () => {
    if (!rendererRef.current || !sceneRef.current || !cameraRef.current || !model) return;

    setIsCapturingThumb(true);
    setSaveToast(null);

    try {
      rendererRef.current.render(sceneRef.current, cameraRef.current);
      const dataUrl = rendererRef.current.domElement.toDataURL('image/png');

      const { thumbnail_path: newThumbPath } = await apiSaveThumbnail(model.id, dataUrl);

      setSaveToast('Custom thumbnail angle saved successfully!');
      onModelUpdated(newThumbPath);
      setTimeout(() => setSaveToast(null), 3000);
    } catch (err: any) {
      alert(err.message || 'Error saving thumbnail snapshot');
    } finally {
      setIsCapturingThumb(false);
    }
  };

  // Favorite toggle
  const handleToggleFavorite = async () => {
    if (!model) return;
    try {
      const res = await apiFetch(`/api/models/${model.id}/favorite`, { method: 'POST' });
      if (res.ok) {
        const data = await res.json();
        setModel((prev) => (prev ? { ...prev, is_favorited: data.is_favorited } : null));
        onModelUpdated();
      }
    } catch (err) {
      console.error('Favorite error:', err);
    }
  };

  // Add to Collection
  const handleAddToCollection = async () => {
    if (!model || !selectedCollectionToAdd) return;
    try {
      const res = await apiFetch(`/api/collections/${selectedCollectionToAdd}/models`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model_id: model.id }),
      });

      if (res.ok) {
        setSaveToast('Model added to collection');
        fetchModel();
        onModelUpdated();
        setTimeout(() => setSaveToast(null), 2500);
      }
    } catch (err: any) {
      alert(err.message);
    }
  };

  // Remove from Collection
  const handleRemoveFromCollection = async (collectionId: string) => {
    if (!model) return;
    try {
      const res = await apiFetch(`/api/collections/${collectionId}/models/${model.id}`, {
        method: 'DELETE',
      });

      if (res.ok) {
        setSaveToast('Model removed from collection');
        fetchModel();
        onModelUpdated();
        setTimeout(() => setSaveToast(null), 2500);
      }
    } catch (err: any) {
      alert(err.message || 'Error removing from collection');
    }
  };

  // Set as primary 3D file
  const handleSetPrimaryFile = async (fileId: string) => {
    if (!model) return;
    try {
      const res = await apiFetch(`/api/models/${model.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ primary_file_id: fileId }),
      });

      if (res.ok) {
        fetchModel();
        onModelUpdated();
      }
    } catch (err: any) {
      alert(err.message);
    }
  };

  // Save metadata edit
  const handleSaveMetadata = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!model) return;
    try {
      const res = await apiFetch(`/api/models/${model.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: editTitle,
          description: editDesc,
          tags: editTags,
        }),
      });

      if (res.ok) {
        setIsEditing(false);
        setSaveToast('Model details updated');
        fetchModel();
        onModelUpdated();
        setTimeout(() => setSaveToast(null), 2500);
      }
    } catch (err: any) {
      alert(err.message);
    }
  };

  // Upload supplementary files.
  //
  // In the Tauri desktop build there is no multipart upload — the web
  // `<input type="file">` cannot hand raw disk paths to Rust. We use the
  // native Tauri dialog plugin instead, then route the selected paths through
  // the `add_model_files` IPC command (see `apiAddModelFiles`).
  const handleFileUpload = async () => {
    try {
      const res = await apiFetch(`/api/models/${modelId}/files`, {
        method: 'POST',
      });

      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || 'Upload failed');
      }

      await fetchModel();
      onModelUpdated();
      setSaveToast('Files added successfully');
      setTimeout(() => setSaveToast(null), 2500);
    } catch (err: any) {
      alert(err.message || 'Error uploading files');
    }
  };

  // Delete model
  const handleDeleteModel = async () => {
    if (!model) return;

    try {
      const res = await apiFetch(`/api/models/${model.id}`, { method: 'DELETE' });
      if (res.ok) {
        onModelUpdated();
        onClose();
      }
    } catch (err: any) {
      alert(err.message);
    }
  };

  // Delete a single file from this model
  const handleDeleteFile = async (fileId: string) => {
    if (!model) return;

    try {
      const res = await apiFetch(`/api/models/${model.id}/files/${fileId}`, { method: 'DELETE' });
      if (res.ok) {
        if (selectedFileId === fileId) setSelectedFileId(null);
        await fetchModel();
        onModelUpdated();
      }
    } catch (err: any) {
      alert(err.message || 'Error deleting file');
    }
  };

  // Download a file to disk.
  //
  // A blob-URL anchor click does NOT trigger a download inside a Tauri WebView
  // (on Linux AppImage it silently does nothing — the button looked dead).
  // Instead, open the native save dialog and stream the file bytes to the path
  // the user picks, via the `save_downloaded_file` Rust command.
  const handleDownloadFile = async (file: ModelFile) => {
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const filePath = await save({
        defaultPath: file.original_name,
        filters: [
          { name: file.original_name.split('.').pop()?.toUpperCase() || 'File', extensions: [file.original_name.split('.').pop() || '*'] },
        ],
      });
      if (!filePath) return; // user cancelled the dialog

      // `save()` returns a FilePath enum (Path | Url), not a plain string.
      const destPath = typeof filePath === 'string'
        ? filePath
        : (filePath as any).Path ?? (filePath as any).Url ?? String(filePath);

      await invoke('save_downloaded_file', {
        sourcePath: file.file_path,
        destPath,
      });
      setSaveToast(`Saved ${file.original_name}`);
      setTimeout(() => setSaveToast(null), 2500);
    } catch (err: any) {
      // Tauri invoke rejections are often raw strings, not Error objects,
      // so `err.message` is undefined and the fallback would hide the real cause.
      const detail = (typeof err === 'string' ? err : (err?.message || err?.toString?.() || String(err)));
      alert(`Error downloading file: ${detail}`);
    }
  };

  // No user context means the single-user desktop build, where every model is
  // owned locally. The web server still enforces ownership on every mutation.
  const isOwnerOrAdmin =
    model && (currentUserId === undefined || model.user_id === currentUserId || isAdmin);
  const viewableFiles = model?.files.filter(is3DFile) || [];
  const currentActiveFile = model?.files.find((f) => f.id === selectedFileId && is3DFile(f))
    || viewableFiles.find((f) => f.is_primary_3d === 1)
    || viewableFiles[0];

  return (
    <ErrorBoundary>
      <div className="fixed inset-0 z-50 flex items-center justify-center p-2 sm:p-4 bg-black/85 backdrop-blur-md">
        <div className="bg-[#16161b] border border-[#2b2b36] rounded-2xl w-full max-w-6xl max-h-[95vh] overflow-hidden flex flex-col shadow-2xl">
          {/* Modal Top Header */}
          <div className="flex items-center justify-between px-6 py-3.5 border-b border-[#252530] bg-[#1a1a22]">
            <div className="flex items-center gap-3 truncate">
              <div className="w-8 h-8 rounded-lg bg-blue-600/20 text-blue-400 flex items-center justify-center shrink-0">
                <Box className="w-4 h-4" />
              </div>
              <div className="truncate">
                <h2 className="text-base font-bold text-white truncate font-['Plus_Jakarta_Sans',sans-serif]">
                  {model?.title || 'Loading Model...'}
                </h2>
                <div className="flex items-center gap-2 text-xs text-neutral-400">
                  <span>by {model?.owner_username}</span>
                  <span aria-hidden="true">·</span>
                  <span>
                    {model?.files.length || 0} {model?.files.length === 1 ? 'file' : 'files'}
                  </span>
                  {currentActiveFile && (
                    <>
                      <span aria-hidden="true">·</span>
                      <span className="text-blue-400 font-mono truncate">
                        Viewing: {currentActiveFile.original_name}
                      </span>
                    </>
                  )}
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2 shrink-0">
              {model && (
                <button
                  onClick={handleToggleFavorite}
                  className={`p-2 rounded-lg border transition-all ${
                    model.is_favorited
                      ? 'bg-red-500/20 text-red-400 border-red-500/40 hover:bg-red-500/30'
                      : 'bg-[#22222a] text-neutral-400 border-[#30303c] hover:text-white'
                  }`}
                  title={model.is_favorited ? 'Favorited' : 'Favorite'}
                >
                  <Heart className={`w-4 h-4 ${model.is_favorited ? 'fill-current text-red-500' : ''}`} />
                </button>
              )}

              <button
                onClick={onClose}
                className="p-2 text-neutral-400 hover:text-white rounded-lg hover:bg-white/5 transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
          </div>

          {/* Modal Body */}
          <div className="flex-1 grid grid-cols-1 lg:grid-cols-12 overflow-y-auto">
            {/* Left Column: 3D Viewport Stage */}
            <div className="lg:col-span-7 bg-[#121214] flex flex-col relative border-b lg:border-b-0 lg:border-r border-[#252530] min-h-[400px] lg:min-h-[560px]">
              <div ref={mountRef} className="w-full h-full flex-1 cursor-grab active:cursor-grabbing" />

              {/* Active File Switcher Overlay (3D files only) */}
              {viewableFiles.length > 1 && (
                <div className="absolute top-4 left-4 z-20 flex items-center gap-2 px-3 py-1.5 bg-[#181822]/90 backdrop-blur-md border border-[#2e2e40] rounded-xl text-xs shadow-xl">
                  <span className="text-[11px] font-semibold text-neutral-400 flex items-center gap-1.5 shrink-0">
                    <Box className="w-3.5 h-3.5 text-blue-400" />
                    Viewing:
                  </span>
                  <Dropdown
                    value={currentActiveFile?.id || ''}
                    onChange={setSelectedFileId}
                    options={viewableFiles.map((file, idx) => ({
                      value: file.id,
                      label: `${idx + 1}. ${file.original_name}${file.is_primary_3d === 1 ? ' ★ [Primary]' : ''}`,
                    }))}
                    className="bg-[#121216] border border-[#35354a] rounded-lg px-2.5 py-1 text-white text-xs outline-none focus:border-blue-500 font-medium max-w-[210px]"
                  />
                  <span className="text-[10px] text-neutral-500 font-mono shrink-0">
                    {viewableFiles.findIndex((f) => f.id === currentActiveFile?.id) + 1}/{viewableFiles.length}
                  </span>
                </div>
              )}

              {loadingMesh && (
                <div className="absolute top-4 right-4 z-20 px-3 py-1.5 bg-[#181822]/90 backdrop-blur-md border border-[#2e2e40] rounded-xl text-xs text-blue-400 flex items-center gap-2 shadow-lg">
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                  <span>Loading 3D asset...</span>
                </div>
              )}

              {meshError && (
                <div className="absolute top-4 right-4 z-30 max-w-[min(28rem,calc(100%-2rem))] px-3 py-2.5 bg-red-950/90 backdrop-blur-md border border-red-500/40 rounded-xl text-xs text-red-200 shadow-lg">
                  <div className="font-semibold text-red-100 mb-1">3D preview unavailable</div>
                  <div className="font-mono text-[11px] leading-relaxed break-words whitespace-pre-wrap">{meshError}</div>
                </div>
              )}

              {/* Viewport Controls */}
              <div className="absolute bottom-4 left-1/2 -translate-x-1/2 flex items-center gap-2 px-3 py-2 bg-[#1b1b24]/90 backdrop-blur-md border border-[#2e2e3d] rounded-xl shadow-xl text-xs z-10">
                <button
                  onClick={toggleWireframe}
                  className={`px-3 py-1.5 rounded-lg font-medium transition-colors ${
                    wireframe ? 'bg-blue-600 text-white' : 'text-neutral-300 hover:bg-[#282834]'
                  }`}
                >
                  {wireframe ? 'Wireframe: ON' : 'Solid Mesh'}
                </button>

                <button
                  onClick={resetCamera}
                  className="px-3 py-1.5 text-neutral-300 hover:bg-[#282834] rounded-lg font-medium transition-colors"
                >
                  Reset Camera
                </button>

                <div className="h-4 w-px bg-[#303040]" />

                <button
                  onClick={handleSaveThumbnail}
                  disabled={isCapturingThumb}
                  className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white font-semibold rounded-lg shadow-sm transition-all"
                  title="Capture this camera angle as the model's 512x512 thumbnail"
                >
                  <Camera className="w-3.5 h-3.5" />
                  <span>{isCapturingThumb ? 'Saving Angle...' : 'Save as Thumbnail'}</span>
                </button>
              </div>

              {saveToast && (
                <div className="absolute top-16 left-1/2 -translate-x-1/2 px-4 py-2 bg-emerald-950/90 border border-emerald-700/60 text-emerald-200 rounded-xl text-xs font-semibold shadow-xl flex items-center gap-2 z-30">
                  <Check className="w-4 h-4 text-emerald-400" />
                  <span>{saveToast}</span>
                </div>
              )}
            </div>

            {/* Right Column: Analytics & Management */}
            <div className="lg:col-span-5 p-6 flex flex-col gap-6 overflow-y-auto bg-[#18181f]">
              {/* Slicing Analytics */}
              <div className="bg-[#1f1f28] border border-[#2e2e3c] rounded-xl p-4 flex flex-col gap-3">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold text-white flex items-center gap-2">
                    <Scale className="w-4 h-4 text-blue-400" />
                    Slicing & Mesh Analytics
                  </h3>
                  <span className="text-[10px] text-neutral-400 font-mono">PLA @ 1.24 g/cm³</span>
                </div>

                {currentActiveFile && (
                  <div className="text-[11px] text-blue-300 bg-blue-950/40 border border-blue-900/50 rounded-lg px-2.5 py-1 flex items-center gap-1.5 truncate">
                    <Box className="w-3 h-3 text-blue-400 shrink-0" />
                    <span className="truncate">Analyzing: <strong>{currentActiveFile.original_name}</strong></span>
                  </div>
                )}

                <div className="grid grid-cols-2 sm:grid-cols-3 gap-2 text-xs">
                  <div className="p-2.5 bg-[#141418] rounded-lg border border-[#2a2a35] flex flex-col">
                    <span className="text-[10px] text-neutral-400">Dimensions (XYZ)</span>
                    <span className="font-mono font-bold text-white mt-1 tabular-nums">
                      {meshStats.widthMm} × {meshStats.depthMm} × {meshStats.heightMm}{' '}
                      <span className="text-[10px] font-normal text-neutral-400">mm</span>
                    </span>
                  </div>

                  <div className="p-2.5 bg-[#141418] rounded-lg border border-[#2a2a35] flex flex-col">
                    <span className="text-[10px] text-neutral-400">Watertight Volume</span>
                    <span className="font-mono font-bold text-white mt-1 tabular-nums">
                      {meshStats.volumeCm3}{' '}
                      <span className="text-[10px] font-normal text-neutral-400">cm³</span>
                    </span>
                  </div>

                  <div className="p-2.5 bg-[#141418] rounded-lg border border-[#2a2a35] flex flex-col">
                    <span className="text-[10px] text-neutral-400">Est. PLA Weight</span>
                    <span className="font-mono font-bold text-emerald-400 mt-1 tabular-nums">
                      ~{meshStats.weightGrams}{' '}
                      <span className="text-[10px] font-normal text-neutral-400">g</span>
                    </span>
                  </div>

                  <div className="col-span-2 sm:col-span-3 p-2.5 bg-[#141418] rounded-lg border border-[#2a2a35] flex items-center justify-between">
                    <div>
                      <span className="text-[10px] text-neutral-400 block">Estimated Filament Cost</span>
                      <span className="font-mono font-bold text-amber-400 text-sm mt-0.5 tabular-nums">
                        ${estimatedCostUsd.toFixed(2)}{' '}
                        <span className="text-[10px] font-normal text-neutral-400">USD</span>
                      </span>
                    </div>

                    <div className="flex items-center gap-1.5 text-xs">
                      <span className="text-[10px] text-neutral-500">Rate:</span>
                      <div className="flex items-center bg-[#202028] border border-[#353545] rounded px-1.5 py-0.5">
                        <span className="text-neutral-400 text-xs">$</span>
                        <input
                          type="number"
                          min="1"
                          max="200"
                          step="1"
                          value={filamentPriceKg}
                          onChange={(e) => setFilamentPriceKg(Number(e.target.value))}
                          className="w-10 bg-transparent text-white text-xs font-mono outline-none text-right pl-0.5"
                        />
                        <span className="text-neutral-400 text-[10px]">/kg</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Files List */}
              <div className="bg-[#1f1f28] border border-[#2e2e3c] rounded-xl p-4 flex flex-col gap-3">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold text-white flex items-center gap-2">
                    <FileCode className="w-4 h-4 text-blue-400" />
                    Model Files ({model?.files.length || 0})
                  </h3>
                  <span className="text-[11px] text-neutral-400">Click a 3D file to view · PDFs are downloadable attachments</span>
                </div>

                <div className="flex flex-col gap-2 max-h-56 overflow-y-auto pr-1">
                  {model?.files.map((file) => {
                    const isPdf = !is3DFile(file);
                    const isCurrent = !isPdf && file.id === currentActiveFile?.id;
                    return (
                      <div
                        key={file.id}
                        onClick={() => {
                          clearFileConfirm();
                          if (isPdf) setPdfPreview(file);
                          else setSelectedFileId(file.id);
                        }}
                        className={`flex items-center justify-between p-2.5 rounded-lg text-xs transition-all border cursor-pointer ${
                          isPdf
                            ? 'bg-[#141418] border-[#2a2a35] hover:bg-[#1a1a22] hover:border-[#38384a]'
                            : isCurrent
                              ? 'bg-blue-950/30 border-blue-500/60 shadow-sm shadow-blue-950/30'
                              : 'bg-[#141418] hover:bg-[#1a1a22] border-[#2a2a35] hover:border-[#38384a]'
                        }`}
                      >
                        <div className="truncate mr-2">
                          <div className="flex items-center gap-1.5 truncate">
                            {isPdf && <FileText className="w-3.5 h-3.5 text-amber-400 shrink-0" />}
                            <span
                              className={`font-semibold truncate ${
                                isCurrent ? 'text-blue-300' : 'text-neutral-200'
                              }`}
                            >
                              {file.original_name}
                            </span>

                            {isPdf && (
                              <span className="flex items-center gap-1 px-1.5 py-0.2 bg-amber-500/20 text-amber-300 border border-amber-500/40 rounded text-[9px] font-mono shrink-0">
                                <FileText className="w-2.5 h-2.5" />
                                ATTACHMENT
                              </span>
                            )}

                            {file.is_primary_3d === 1 && (
                              <span className="flex items-center gap-1 px-1.5 py-0.2 bg-amber-500/20 text-amber-300 border border-amber-500/40 rounded text-[9px] font-mono shrink-0">
                                <Star className="w-2.5 h-2.5 fill-current" />
                                PRIMARY
                              </span>
                            )}

                            {isCurrent && (
                              <span className="flex items-center gap-1 px-1.5 py-0.2 bg-blue-600/30 text-blue-400 border border-blue-500/40 rounded text-[9px] font-mono shrink-0">
                                <Eye className="w-2.5 h-2.5" />
                                VIEWING
                              </span>
                            )}
                          </div>
                          <span className="text-[10px] text-neutral-500 font-mono">
                            {(file.file_size / 1024 / 1024).toFixed(2)} MB · {file.mime_type}
                          </span>
                        </div>

                        <div className="flex items-center gap-1.5 shrink-0" onClick={(e) => e.stopPropagation()}>
                          {(isPdf || !isCurrent) && (
                            <button
                              onClick={() => {
                                if (isPdf) setPdfPreview(file);
                                else setSelectedFileId(file.id);
                              }}
                              className="flex items-center gap-1 px-2 py-1 text-[10px] text-blue-400 hover:text-white bg-blue-950/50 hover:bg-blue-600 border border-blue-800/50 rounded transition-colors"
                              title={isPdf ? 'Open PDF in viewer' : 'Load this file into the 3D viewport'}
                            >
                              {isPdf ? <FileText className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                              <span>View</span>
                            </button>
                          )}

                          {!isPdf && isOwnerOrAdmin && file.is_primary_3d !== 1 && (
                            <button
                              onClick={() => handleSetPrimaryFile(file.id)}
                              className="px-2 py-1 text-[10px] text-neutral-400 hover:text-white bg-[#22222c] hover:bg-[#2c2c38] rounded transition-colors"
                              title="Set as primary 3D file for thumbnail and default view"
                            >
                              Make Primary
                            </button>
                          )}

                          <button
                            type="button"
                            onClick={() => handleDownloadFile(file)}
                            className="p-1.5 text-neutral-400 hover:text-white hover:bg-white/10 rounded transition-colors"
                            title="Download file"
                          >
                            <Download className="w-3.5 h-3.5" />
                          </button>

                          {isOwnerOrAdmin && (
                            <button
                              onClick={() =>
                                confirmDeleteFileId === file.id
                                  ? handleDeleteFile(file.id)
                                  : setConfirmDeleteFileId(file.id)
                              }
                              className={`flex items-center gap-1 px-1.5 py-1 text-[10px] rounded transition-colors ${
                                confirmDeleteFileId === file.id
                                  ? 'text-white bg-red-600 hover:bg-red-500'
                                  : 'text-neutral-400 hover:text-red-400 hover:bg-red-950/40'
                              }`}
                              title={
                                confirmDeleteFileId === file.id
                                  ? 'Click again to delete this file from disk'
                                  : 'Delete this file'
                              }
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                              {confirmDeleteFileId === file.id && <span>Confirm</span>}
                            </button>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Collections Block */}
              <div className="bg-[#1f1f28] border border-[#2e2e3c] rounded-xl p-4 flex flex-col gap-2.5">
                <h3 className="text-xs font-bold text-white flex items-center gap-2">
                  <FolderPlus className="w-4 h-4 text-blue-400" />
                  Add to Folder / Collection
                </h3>

                {model?.collections && model.collections.length > 0 && (
                  <div className="flex flex-wrap gap-1.5 mb-1">
                    <span className="text-[11px] text-neutral-400 self-center">Assigned to:</span>
                    {model.collections.map((c) => (
                      <span
                        key={c.id}
                        className="inline-flex items-center gap-1 px-2 py-0.5 bg-blue-950/60 border border-blue-800/60 text-blue-300 rounded text-[11px]"
                      >
                        <span>{c.name}</span>
                        {isOwnerOrAdmin && (
                          <button
                            type="button"
                            onClick={() => handleRemoveFromCollection(c.id)}
                            className="text-blue-400 hover:text-red-400 transition-colors p-0.5"
                            title={`Remove from ${c.name}`}
                          >
                            <X className="w-2.5 h-2.5" />
                          </button>
                        )}
                      </span>
                    ))}
                  </div>
                )}

                <div className="flex items-center gap-2">
                <div className="flex-1">
                  <Dropdown
                    value={selectedCollectionToAdd}
                    onChange={setSelectedCollectionToAdd}
                    options={[
                      { value: '', label: 'Select Folder...' },
                      ...collections.map((c) => ({
                        value: c.id,
                        label: c.parent_folder_id ? `└── ${c.name}` : c.name,
                      })),
                    ]}
                    className="w-full bg-[#141418] border border-[#30303e] rounded-lg px-2.5 py-1.5 text-xs text-white outline-none"
                  />
                </div>
                  <button
                    onClick={handleAddToCollection}
                    disabled={!selectedCollectionToAdd}
                    className="px-3 py-1.5 text-xs font-semibold text-white bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-lg transition-colors whitespace-nowrap"
                  >
                    Assign
                  </button>
                </div>
              </div>

              {/* Description & Tags */}
              <div className="bg-[#1f1f28] border border-[#2e2e3c] rounded-xl p-4 flex flex-col gap-2">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-neutral-200">Description & Tags</span>
                  {isOwnerOrAdmin && !isEditing && (
                    <button
                      onClick={() => setIsEditing(true)}
                      className="flex items-center gap-1 text-[11px] text-blue-400 hover:underline"
                    >
                      <Edit2 className="w-3 h-3" />
                      Edit
                    </button>
                  )}
                </div>

                {isEditing ? (
                  <form onSubmit={handleSaveMetadata} className="flex flex-col gap-2 mt-1">
                    <input
                      type="text"
                      value={editTitle}
                      onChange={(e) => setEditTitle(e.target.value)}
                      required
                      className="bg-[#121214] border border-[#353545] rounded px-2.5 py-1.5 text-xs text-white"
                    />
                    <textarea
                      rows={2}
                      value={editDesc}
                      onChange={(e) => setEditDesc(e.target.value)}
                      placeholder="Description"
                      className="bg-[#121214] border border-[#353545] rounded px-2.5 py-1.5 text-xs text-white"
                    />
                    <input
                      type="text"
                      value={editTags}
                      onChange={(e) => setEditTags(e.target.value)}
                      placeholder="Tags (comma separated)"
                      className="bg-[#121214] border border-[#353545] rounded px-2.5 py-1.5 text-xs text-white"
                    />
                    <div className="flex items-center justify-end gap-2 mt-1">
                      <button
                        type="button"
                        onClick={() => setIsEditing(false)}
                        className="px-2.5 py-1 text-xs text-neutral-400"
                      >
                        Cancel
                      </button>
                      <button
                        type="submit"
                        className="px-3 py-1 bg-blue-600 hover:bg-blue-500 text-white rounded text-xs font-semibold"
                      >
                        Save Changes
                      </button>
                    </div>
                  </form>
                ) : (
                  <>
                    <p className="text-xs text-neutral-300 leading-relaxed">
                      {model?.description || 'No description provided.'}
                    </p>
                    {model?.tags && model.tags.length > 0 && (
                      <div className="flex flex-wrap gap-1 mt-1">
                        {model.tags.map((t) => (
                          <span
                            key={t}
                            className="px-2 py-0.5 bg-[#252530] text-neutral-300 text-[10px] rounded font-medium"
                          >
                            #{t}
                          </span>
                        ))}
                      </div>
                    )}
                  </>
                )}
              </div>

              {/* Add Files Block */}
              <div className="bg-[#1f1f28] border border-[#2e2e3c] rounded-xl p-4 flex flex-col gap-2.5">
                <h3 className="text-xs font-bold text-white flex items-center gap-2">
                  <FolderPlus className="w-4 h-4 text-blue-400" /> Add Additional Files
                </h3>

                <button
                  onClick={handleFileUpload}
                  disabled={loading}
                  className="flex items-center gap-1.5 px-5 py-2 text-xs font-medium text-white bg-blue-600 hover:bg-blue-500 rounded-lg justify-center transition-colors"
                >
                  <Upload className="w-4 h-4" /> Add Files
                </button>
              </div>

              {/* Danger Zone */}
              {isOwnerOrAdmin && (
                <div className="pt-2 flex justify-end">
                  <button
                    onClick={() =>
                      confirmDeleteModel ? handleDeleteModel() : setConfirmDeleteModel(true)
                    }
                    className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors ${
                      confirmDeleteModel
                        ? 'text-white bg-red-600 hover:bg-red-500 border-red-500'
                        : 'text-red-400 hover:text-red-300 bg-red-950/30 hover:bg-red-950/60 border-red-900/50'
                    }`}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    {confirmDeleteModel ? 'Confirm Delete' : 'Delete'}
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {pdfPreview && (
        <PdfViewerModal
          file={pdfPreview}
          onClose={() => setPdfPreview(null)}
          onDownload={handleDownloadFile}
        />
      )}
    </ErrorBoundary>
  );
};

export default ModelViewerModal;