import React from 'react';
import { Upload } from 'lucide-react';

interface NavbarProps {
  activeTab: 'all' | 'mine' | 'favorites' | 'collections';
  onTabChange: (tab: 'all' | 'mine' | 'favorites' | 'collections') => void;
  onOpenUpload: () => void;
}

export const Navbar: React.FC<NavbarProps> = ({
  activeTab,
  onTabChange,
  onOpenUpload,
}) => {
  return (
    <header className="sticky top-0 z-40 bg-[#16161a]/95 backdrop-blur-md border-b border-[#262630] px-4 lg:px-8 py-3.5">
      <div className="max-w-7xl mx-auto flex items-center justify-between gap-6">
        <div className="flex items-center gap-3 shrink-0">
          <button
            onClick={() => onTabChange('all')}
            className="flex items-center gap-2.5 text-left group focus:outline-none"
          >
            <div className="w-9 h-9 rounded-lg bg-blue-600/20 border border-blue-500/40 flex items-center justify-center text-blue-400 group-hover:bg-blue-600/30 group-hover:border-blue-400 transition-all overflow-hidden">
              <img src="/favicon.png" alt="SLAG3D" className="w-5 h-5" />
            </div>
            <div>
              <span className="text-xl font-bold tracking-tight text-white font-['Plus_Jakarta_Sans',sans-serif]">
                SLAG<span className="text-blue-500">3D</span>
              </span>
              <span className="hidden sm:inline-block ml-2 text-[10px] tracking-wider uppercase text-neutral-400 font-mono">
                Asset Vault
              </span>
            </div>
          </button>
        </div>

        <nav className="hidden md:flex items-center gap-1.5 p-1 bg-[#1c1c23] border border-[#2d2d38] rounded-xl text-sm font-medium">
          <button
            onClick={() => onTabChange('all')}
            className={`px-3.5 py-1.5 rounded-lg transition-all whitespace-nowrap shrink-0 ${
              activeTab === 'all'
                ? 'bg-blue-600 text-white shadow-sm font-semibold'
                : 'text-neutral-400 hover:text-white hover:bg-white/5'
            }`}
          >
            All Models
          </button>
          <button
            onClick={() => onTabChange('mine')}
            className={`px-3.5 py-1.5 rounded-lg transition-all whitespace-nowrap shrink-0 ${
              activeTab === 'mine'
                ? 'bg-blue-600 text-white shadow-sm font-semibold'
                : 'text-neutral-400 hover:text-white hover:bg-white/5'
            }`}
          >
            My Models
          </button>
          <button
            onClick={() => onTabChange('favorites')}
            className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg transition-all whitespace-nowrap shrink-0 ${
              activeTab === 'favorites'
                ? 'bg-blue-600 text-white shadow-sm font-semibold'
                : 'text-neutral-400 hover:text-white hover:bg-white/5'
            }`}
          >
            Favorites
          </button>
        </nav>

        <div className="flex items-center gap-2.5 shrink-0">
          <button
            onClick={onOpenUpload}
            className="flex items-center gap-1.5 px-3.5 py-2 text-xs font-semibold text-white bg-blue-600 rounded-lg hover:bg-blue-500 shadow-md shadow-blue-600/20 active:scale-98 transition-all whitespace-nowrap shrink-0"
          >
            <Upload className="w-3.5 h-3.5" />
            Upload Model
          </button>
        </div>
      </div>
    </header>
  );
};
