import React, { useState } from 'react';
import {
  AlertTriangle,
  Archive,
  CheckCircle,
  Download,
  Loader2,
  RotateCcw,
  X,
} from 'lucide-react';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { ErrorBoundary } from './ErrorBoundary';
import { apiCreateBackup, apiImportBackup } from '../utils/api';
import { BackupInfo } from '../types';

interface BackupModalProps {
  onClose: () => void;
  onRestored: () => void;
}

const BACKUP_FILTER = [{ name: 'SLAG3D Backup', extensions: ['zip'] }];

function asPath(value: unknown): string | null {
  if (!value) return null;
  if (typeof value === 'string') return value;
  const enumValue = value as { Path?: string; Url?: string };
  return enumValue.Path ?? enumValue.Url ?? String(value);
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

export const BackupModal: React.FC<BackupModalProps> = ({ onClose, onRestored }) => {
  const [busy, setBusy] = useState<'export' | 'import' | null>(null);
  const [result, setResult] = useState<BackupInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pendingRestore, setPendingRestore] = useState<string | null>(null);

  const handleCreate = async () => {
    setError(null);
    setResult(null);
    try {
      const picked = await saveDialog({
        defaultPath: `slag3d-backup-${new Date().toISOString().slice(0, 10)}.zip`,
        filters: BACKUP_FILTER,
      });
      const destination = asPath(picked);
      if (!destination) return;

      setBusy('export');
      const info = await apiCreateBackup(destination);
      setResult(info);
    } catch (err) {
      setError(typeof err === 'string' ? err : (err as Error)?.message || 'Backup failed');
    } finally {
      setBusy(null);
    }
  };

  const handlePickRestore = async () => {
    setError(null);
    setResult(null);
    try {
      const picked = await openDialog({ multiple: false, filters: BACKUP_FILTER });
      setPendingRestore(asPath(picked));
    } catch (err) {
      setError(typeof err === 'string' ? err : (err as Error)?.message || 'Could not open file picker');
    }
  };

  const handleRestore = async () => {
    if (!pendingRestore) return;
    setError(null);
    setBusy('import');
    try {
      const info = await apiImportBackup(pendingRestore);
      setResult(info);
      // Let the success message paint before the view reloads with restored data.
      window.setTimeout(onRestored, 700);
    } catch (err) {
      setError(typeof err === 'string' ? err : (err as Error)?.message || 'Restore failed');
    } finally {
      setBusy(null);
    }
  };

  return (
    <ErrorBoundary>
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
        <div className="bg-[#18181e] border border-[#2d2d38] rounded-2xl w-full max-w-xl max-h-[90vh] overflow-y-auto flex flex-col shadow-2xl">
          <div className="flex items-center justify-between px-6 py-4 border-b border-[#262632]">
            <div className="flex items-center gap-2">
              <Archive className="w-5 h-5 text-blue-400" />
              <h2 className="text-base font-bold text-white">Backup &amp; Restore</h2>
            </div>
            <button
              onClick={onClose}
              className="p-1.5 text-neutral-400 hover:text-white rounded-lg hover:bg-white/5 transition-colors"
              aria-label="Close"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <div className="p-6 flex flex-col gap-5">
            {error && (
              <div className="flex items-start gap-3 p-4 bg-red-950/30 border border-red-800/50 rounded-xl text-xs text-red-200">
                <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5 text-red-400" />
                <span className="break-words">{error}</span>
              </div>
            )}
            {result && (
              <div className="flex items-start gap-3 p-4 bg-emerald-950/30 border border-emerald-800/50 rounded-xl text-xs text-emerald-200">
                <CheckCircle className="w-4 h-4 shrink-0 mt-0.5 text-emerald-400" />
                <div className="min-w-0">
                  <p className="font-semibold">
                    {result.model_count} model{result.model_count === 1 ? '' : 's'} · {formatSize(result.size)}
                  </p>
                  <p className="truncate text-emerald-300/80 mt-0.5" title={result.path}>{result.path}</p>
                </div>
              </div>
            )}

            <section className="border border-[#2c2c38] bg-[#131317]/60 rounded-xl p-4 flex flex-col gap-3">
              <div className="flex items-start gap-3">
                <Download className="w-4 h-4 text-blue-400 mt-0.5 shrink-0" />
                <div>
                  <h3 className="text-sm font-semibold text-neutral-100">Create a backup</h3>
                  <p className="text-xs text-neutral-500 mt-1">
                    Saves a single .zip with the database, all models, attachments and thumbnails.
                  </p>
                </div>
              </div>
              <button
                onClick={handleCreate}
                disabled={busy !== null}
                className="self-start flex items-center gap-2 px-3.5 py-2 text-xs font-semibold text-white bg-blue-600 rounded-lg hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {busy === 'export' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
                {busy === 'export' ? 'Backing up…' : 'Choose location…'}
              </button>
            </section>

            <section className="border border-[#2c2c38] bg-[#131317]/60 rounded-xl p-4 flex flex-col gap-3">
              <div className="flex items-start gap-3">
                <RotateCcw className="w-4 h-4 text-amber-400 mt-0.5 shrink-0" />
                <div>
                  <h3 className="text-sm font-semibold text-neutral-100">Restore from a backup</h3>
                  <p className="text-xs text-neutral-500 mt-1">
                    Replaces the current vault with the selected backup.
                  </p>
                </div>
              </div>
              <button
                onClick={handlePickRestore}
                disabled={busy !== null}
                className="self-start flex items-center gap-2 px-3.5 py-2 text-xs font-semibold text-neutral-200 bg-[#22222c] border border-[#343444] rounded-lg hover:bg-[#2b2b37] disabled:opacity-50 transition-colors"
              >
                <Archive className="w-3.5 h-3.5" />
                Select backup file…
              </button>

              {pendingRestore && (
                <div className="flex flex-col gap-3 p-3 bg-amber-950/20 border border-amber-800/40 rounded-lg">
                  <p className="text-xs text-amber-200 truncate" title={pendingRestore}>{pendingRestore}</p>
                  <p className="text-xs text-amber-200/90">
                    <strong>This will delete all models, collections, tags and files currently in the vault.</strong> The app will reload when the restore completes.
                  </p>
                  <div className="flex gap-2">
                    <button
                      onClick={handleRestore}
                      disabled={busy !== null}
                      className="flex items-center gap-2 px-3.5 py-2 text-xs font-semibold text-white bg-amber-600 rounded-lg hover:bg-amber-500 disabled:opacity-50 transition-colors"
                    >
                      {busy === 'import' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <RotateCcw className="w-3.5 h-3.5" />}
                      {busy === 'import' ? 'Restoring…' : 'Restore and replace vault'}
                    </button>
                    <button
                      onClick={() => setPendingRestore(null)}
                      disabled={busy !== null}
                      className="px-3 py-2 text-xs font-semibold text-neutral-400 hover:text-white disabled:opacity-50"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              )}
            </section>
          </div>
        </div>
      </div>
    </ErrorBoundary>
  );
};
