import React, { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown } from 'lucide-react';

export interface DropdownOption {
  value: string;
  label: string;
}

interface DropdownProps {
  value: string;
  options: DropdownOption[];
  onChange: (value: string) => void;
  className?: string;
  panelClassName?: string;
  disabled?: boolean;
}

/**
 * In-page replacement for <select>. Native popups are drawn by GTK on Linux and
 * ignore Tailwind classes, so they rendered light against the dark UI.
 */
export const Dropdown: React.FC<DropdownProps> = ({
  value,
  options,
  onChange,
  className = '',
  panelClassName = '',
  disabled,
}) => {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className={`flex items-center justify-between gap-2 text-left disabled:opacity-50 ${className}`}
      >
        <span className="truncate">{selected?.label ?? ''}</span>
        <ChevronDown className={`w-3.5 h-3.5 shrink-0 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>

      {open && (
        <div
          role="listbox"
          className={`absolute left-0 right-0 z-50 mt-1 max-h-60 overflow-y-auto bg-[#18181e] border border-[#343444] rounded-lg shadow-2xl py-1 ${panelClassName}`}
        >
          {options.map((option) => (
            <button
              key={option.value}
              type="button"
              role="option"
              aria-selected={option.value === value}
              onClick={() => {
                onChange(option.value);
                setOpen(false);
              }}
              className={`w-full flex items-center justify-between gap-2 px-3 py-1.5 text-left text-xs hover:bg-[#23232c] ${
                option.value === value ? 'text-white bg-blue-600/15' : 'text-neutral-300'
              }`}
            >
              <span className="truncate">{option.label}</span>
              {option.value === value && <Check className="w-3 h-3 shrink-0 text-blue-400" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};
