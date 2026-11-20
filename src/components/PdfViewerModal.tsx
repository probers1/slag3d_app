import React, { useEffect, useState } from 'react';
import { X, Download, FileText } from 'lucide-react';
import { ModelFile } from '../types';
import { fetchFileAsBlobUrl } from '../utils/api';

interface PdfViewerModalProps {
  file: ModelFile;
  onClose: () => void;
  onDownload: (file: ModelFile) => void;
}

// Uses the WebView's built-in PDF viewer: a blob URL with an application/pdf
// MIME type renders with native zoom, paging and search, without a dependency.
export const PdfViewerModal: React.FC<PdfViewerModalProps> = ({
  file,
  onClose,
  onDownload,
}) => {
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let createdUrl: string | null = null;
    setBlobUrl(null);
    setError(null);

    fetchFileAsBlobUrl(file.file_path)
      .then((url) => {
        createdUrl = url;
        if (active) setBlobUrl(url);
        else URL.revokeObjectURL(url);
      })
      .catch((e) => {
        if (active) setError(e.message || 'Failed to load PDF');
      });

    return () => {
      active = false;
      if (createdUrl) URL.revokeObjectURL(createdUrl);
    };
  }, [file.file_path]);

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/80 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-5xl h-full max-h-[90vh] flex flex-col bg-[#1b1b24] border border-[#35354a] rounded-xl shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex items-center justify-between gap-3 px-4 py-3 border-b border-[#2e2e3c] shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <FileText className="w-4 h-4 text-amber-400 shrink-0" />
            <span className="text-sm font-semibold text-white truncate" title={file.original_name}>
              {file.original_name}
            </span>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <button
              onClick={() => onDownload(file)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-neutral-300 hover:text-white bg-[#252530] hover:bg-[#303040] rounded-lg transition-colors"
              title="Download PDF"
            >
              <Download className="w-3.5 h-3.5" />
              Download
            </button>
            <button
              onClick={onClose}
              className="p-1.5 text-neutral-400 hover:text-white hover:bg-white/10 rounded-lg transition-colors"
              title="Close PDF viewer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </header>

        <div className="flex-1 min-h-0 bg-white">
          {error ? (
            <div className="h-full flex items-center justify-center p-6 text-center text-sm text-red-600">
              {error}
            </div>
          ) : blobUrl ? (
            <iframe
              src={blobUrl}
              title={file.original_name}
              className="w-full h-full border-0"
            />
          ) : (
            <div className="h-full flex items-center justify-center text-sm text-neutral-500">
              Loading PDF...
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
