import { useEffect, useRef, useState } from "react";
import { ChevronDown, Check } from "lucide-react";

export interface DropdownOption {
  value: string;
  label: string;
}

interface DropdownProps {
  value: string;
  options: DropdownOption[];
  onChange: (value: string) => void;
  placeholder?: string;
  align?: "left" | "right";
}

export function Dropdown({ value, options, onChange, placeholder, align = "right" }: DropdownProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  const selected = options.find((o) => o.value === value);

  return (
    <div className="dropdown" ref={ref}>
      <button
        type="button"
        className={`dropdown-trigger ${open ? "open" : ""}`}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="dropdown-trigger-label">{selected?.label ?? placeholder ?? value}</span>
        <ChevronDown size={12} strokeWidth={2.5} className="dropdown-chevron" />
      </button>
      {open && (
        <div className={`dropdown-menu dropdown-menu--${align}`}>
          {options.map((opt) => (
            <button
              type="button"
              key={opt.value}
              className={`dropdown-item ${opt.value === value ? "selected" : ""}`}
              onClick={() => {
                onChange(opt.value);
                setOpen(false);
              }}
            >
              <span>{opt.label}</span>
              {opt.value === value && <Check size={12} strokeWidth={2.5} />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
