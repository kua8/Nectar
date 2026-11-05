import { useEffect, useState } from "react";
import { useT } from "../i18n";

const SHORTCUT_MODIFIERS: Record<string, string> = {
  ctrl: "Ctrl",
  control: "Ctrl",
  alt: "Alt",
  shift: "Shift",
  win: "Win",
  windows: "Win",
  meta: "Win",
  super: "Win",
};
const NAMED_KEYS = ["Space", "Enter", "Tab", "Esc", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "PageDown", "Up", "Down", "Left", "Right"];

// Same grammar the dock's Rust side presses (launchers.rs): modifiers, then one key.
// Returns the tidied shortcut, "" for empty, or null if it can't be pressed.
function normalizeShortcut(text: string): string | null {
  const parts = text.split("+").map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return "";
  const mods = new Set<string>();
  let key: string | null = null;
  for (const part of parts) {
    const mod = SHORTCUT_MODIFIERS[part.toLowerCase()];
    if (mod) {
      mods.add(mod);
      continue;
    }
    if (key) return null;
    if (/^[a-z0-9]$/i.test(part) || /^f([1-9]|1\d|2[0-4])$/i.test(part)) key = part.toUpperCase();
    else if (part.length === 1 && "`-=,./;'[]\\".includes(part)) key = part;
    else key = NAMED_KEYS.find((k) => k.toLowerCase() === part.toLowerCase()) ?? null;
    if (!key) return null;
  }
  if (!key) return null;
  return [...["Win", "Ctrl", "Alt", "Shift"].filter((m) => mods.has(m)), key].join("+");
}

export function ShortcutField({ value, placeholder, onCommit, label = "dock.searchShortcut" }: { value: string; placeholder: string; onCommit: (shortcut: string) => void; label?: string }) {
  const t = useT();
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const normalized = normalizeShortcut(draft);
  const commit = () => {
    if (normalized === null) return;
    setDraft(normalized);
    if (normalized !== value) onCommit(normalized);
  };
  return (
    <input
      className={`shortcut-input${normalized === null ? " invalid" : ""}`}
      value={draft}
      placeholder={placeholder}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
      spellCheck={false}
      aria-label={t(label)}
      title={normalized === null ? t("dock.searchShortcut.invalid") : undefined}
    />
  );
}
