import { useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";

type Props = {
  id: string;
  title: ReactNode;
  defaultOpen?: boolean;
  children: ReactNode;
};

const KEY = "nectar-settings-section-";

export function SettingSection({ id, title, defaultOpen = false, children }: Props) {
  const [open, setOpen] = useState(() => {
    try {
      const saved = localStorage.getItem(KEY + id);
      return saved === null ? defaultOpen : saved === "1";
    } catch {
      return defaultOpen;
    }
  });

  const toggle = () => {
    setOpen((prev) => {
      try { localStorage.setItem(KEY + id, prev ? "0" : "1"); } catch { /* storage unavailable */ }
      return !prev;
    });
  };

  return (
    <div className={`setting-section${open ? "" : " collapsed"}`}>
      <button className="setting-group-label setting-section-header" onClick={toggle} aria-expanded={open}>
        <span>{title}</span>
        <ChevronDown size={14} strokeWidth={1.75} className="setting-section-chevron" />
      </button>
      {open && <div className="setting-group">{children}</div>}
    </div>
  );
}
