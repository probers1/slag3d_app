import { X, Plus, Trash2 } from 'lucide-react';
import React, { useState, useEffect } from 'react';
import { TagItem } from '../types';
import { apiFetch } from '../utils/api';

interface TagsModalProps {
  onClose: () => void;
  onAddTag: (name: string) => void;
  onDeleteTag: (id: string) => void;
}

function TagIcon({ className }: { className?: string }) {
  return (
    <svg 
      xmlns="http://www.w3.org/2000/svg" 
      viewBox="0 0 24 24" 
      fill="none" 
      stroke="currentColor" 
      strokeWidth="2" 
      strokeLinecap="round" 
      strokeLinejoin="round"
      className={className}
    >
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

export const TagsModal: React.FC<TagsModalProps> = ({
  onClose,
  onAddTag,
  onDeleteTag,
}) => {
  const [newTagName, setNewTagName] = useState('');
  const [tags, setTags] = useState<TagItem[]>([]);

  // Fetch tags when modal opens
  useEffect(() => {
    if (tags.length === 0) {
      fetchTags();
    }
  }, []);

  async function fetchTags() {
    try {
      const res = await apiFetch('/api/tags');
      if (res.ok) {
        const data = await res.json();
        setTags(data.tags || []);
      }
    } catch (err) {
      console.error('Error fetching tags:', err);
    }
  }

  // Handle adding a new tag
  async function handleAddTag() {
    if (!newTagName.trim()) return;
    
    try {
      const res = await apiFetch('/api/tags', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newTagName }),
      });

      if (res.ok) {
        setNewTagName('');
        fetchTags();  // Refresh the list
      } else {
        const data = await res.json();
        alert(data.error || 'Failed to add tag');
      }
    } catch (err) {
      console.error('Add tag error:', err);
    }
  }

  // Handle deleting a tag
  async function handleDeleteTag(id: string, e?: any) {
    if (!e) return;
    e.stopPropagation();
    
    try {
      const res = await apiFetch(`/api/tags/${id}`, { method: 'DELETE' });
      if (res.ok) {
        fetchTags();  // Refresh the list
      } else {
        const data = await res.json();
        alert(data.error || 'Failed to delete tag');
      }
    } catch (err) {
      console.error('Delete tag error:', err);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div 
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />
      
      {/* Modal Panel */}
      <div className="relative w-full max-w-md mx-4 bg-[#1a1a23] border border-[#353540] rounded-xl shadow-2xl animate-in fade-in zoom-in duration-200">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[#2e2e3a]">
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <TagIcon className="w-5 h-5 text-blue-400" />
            Tags
          </h2>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg hover:bg-[#2e2e3a] transition-colors text-neutral-400 hover:text-white"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="p-5">
          {/* Add Tag Input */}
          <div className="flex items-center gap-2 mb-6">
            <input
              type="text"
              placeholder="New tag name..."
              value={newTagName}
              onChange={(e) => setNewTagName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleAddTag();
              }}
              className="flex-1 bg-[#121214] border border-[#2e2e3c] rounded-lg px-3 py-2 text-xs text-white placeholder-neutral-500 focus:outline-none focus:border-blue-500 transition-colors"
            />
            <button
              onClick={handleAddTag}
              disabled={!newTagName.trim()}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-blue-600 text-white text-xs font-semibold hover:bg-blue-500 active:scale-98 transition-all disabled:opacity-40 disabled:hover:bg-blue-600"
            >
              <Plus className="w-3 h-3" />
              Add
            </button>
          </div>

          {/* Tag List */}
          {tags.length === 0 ? (
            <div className="text-center py-8">
              <TagIcon className="w-8 h-8 text-neutral-600 mx-auto mb-2" />
              <p className="text-sm text-neutral-500 font-medium">No tags yet</p>
              <p className="text-xs text-neutral-600 mt-1">Add your first tag to organize models!</p>
            </div>
          ) : (
            <div className="space-y-2 max-h-[340px] overflow-y-auto pr-1 custom-scrollbar">
              {tags.map((tag) => (
                <div
                  key={tag.id}
                  onClick={() => handleDeleteTag(tag.id)}
                  className="group flex items-center justify-between px-3 py-2.5 rounded-lg bg-[#1e1e26]/60 hover:bg-[#22222c] border border-transparent hover:border-[#3a3a44] transition-all cursor-pointer"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-neutral-200">#{tag.name}</span>
                    <span className="text-xs text-neutral-500 font-mono bg-[#1a1a23] px-1.5 py-0.5 rounded">
                      {tag.count}
                    </span>
                  </div>
                  <button
                    onClick={(e) => handleDeleteTag(tag.id, e)}
                    className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-[#3a3a44] transition-all text-neutral-400 hover:text-white"
                    title={`Delete #${tag.name}`}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Footer */}
          <div className="mt-6 pt-4 border-t border-[#2e2e3a] flex justify-end">
            <button
              onClick={onClose}
              className="px-4 py-2 rounded-lg bg-[#1e1e26] text-neutral-300 text-xs font-semibold hover:bg-[#282834] transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

