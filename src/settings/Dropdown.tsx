import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
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

const GAP = 6;
const EDGE = 8;
const MAX_HEIGHT = 240;

// The menu is drawn in <body> so no section, card or scroll area can clip it or paint over it, and
// it opens upward when there is more room above the button.
export function Dropdown({ value, options, onChange, placeholder, align = "right" }: DropdownProps) {
  const [open, setOpen] = useState(false);
  const [style, setStyle] = useState<CSSProperties | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const place = useCallback(() => {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const below = window.innerHeight - rect.bottom - GAP - EDGE;
    const above = rect.top - GAP - EDGE;
    const wanted = Math.min(MAX_HEIGHT, menuRef.current?.scrollHeight ?? options.length * 30 + 8);
    const up = below < wanted && above > below;
    const room = up ? above : below;
    setStyle({
      top: up ? "auto" : rect.bottom + GAP,
      bottom: up ? window.innerHeight - rect.top + GAP : "auto",
      left: align === "left" ? rect.left : "auto",
      right: align === "right" ? window.innerWidth - rect.right : "auto",
      minWidth: rect.width,
      maxHeight: Math.max(96, Math.min(MAX_HEIGHT, room)),
    });
  }, [align, options.length]);

  useLayoutEffect(() => {
    if (open) place();
    else setStyle(null);
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      const target = e.target as Node;
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    const handleScroll = (e: Event) => {
      if (menuRef.current?.contains(e.target as Node)) return;
      place();
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", handleScroll, true);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", handleScroll, true);
    };
  }, [open, place]);

  const selected = options.find((o) => o.value === value);

  return (
    <div className="dropdown">
      <button
        ref={triggerRef}
        type="button"
        className={`dropdown-trigger ${open ? "open" : ""}`}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="dropdown-trigger-label">{selected?.label ?? placeholder ?? value}</span>
        <ChevronDown size={12} strokeWidth={2.5} className="dropdown-chevron" />
      </button>
      {open &&
        createPortal(
          <div ref={menuRef} className="dropdown-menu dropdown-menu--portal" style={style ?? { visibility: "hidden" }}>
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
          </div>,
          document.body
        )}
    </div>
  );
}
