import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { startDrag } from "@crabnebula/tauri-plugin-drag";
import { useT } from "../i18n";

export type ShelfItem = {
  id: string;
  kind: "file" | "text";
  path?: string;
  name: string;
  text?: string;
  thumb?: string | null;
};

const STORAGE_KEY = "nectar-shelf-items";
const MAX_ITEMS = 10;

const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

function readSaved(): ShelfItem[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
    return Array.isArray(parsed) ? parsed.map((i: ShelfItem) => ({ ...i, thumb: null })) : [];
  } catch {
    return [];
  }
}

type ShelfCallbacks = {
  onDragEnter: () => void;
  onDragLeave: () => void;
  onDrop: () => void;
};

export function useShelf(windowLabel: string, enabled: boolean, persist: boolean, cb: ShelfCallbacks) {
  const [items, setItems] = useState<ShelfItem[]>(() => (persist ? readSaved() : []));
  const cbRef = useRef(cb);
  cbRef.current = cb;

  useEffect(() => {
    if (!persist) {
      try { localStorage.removeItem(STORAGE_KEY); } catch { /* storage unavailable */ }
      return;
    }
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(items.map(({ thumb, ...rest }) => rest))); } catch { /* storage unavailable */ }
  }, [items, persist]);

  // Forget saved files that were moved or deleted since the last run.
  useEffect(() => {
    if (!persist) return;
    const files = items.filter((i) => i.kind === "file" && i.path).map((i) => i.path as string);
    if (files.length === 0) return;
    invoke<string[]>("shelf_prune", { paths: files }).then((alive) => {
      const keep = new Set(alive);
      setItems((prev) => prev.filter((i) => i.kind !== "file" || keep.has(i.path as string)));
    }).catch(() => { });
    // Only once at startup.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const loadThumb = useCallback((id: string, path: string) => {
    invoke<string | null>("shelf_thumbnail", { path }).then((thumb) => {
      if (thumb) setItems((prev) => prev.map((i) => (i.id === id ? { ...i, thumb } : i)));
    }).catch(() => { });
  }, []);

  const add = useCallback((paths?: string[], text?: string) => {
    const fresh: ShelfItem[] = [];
    if (paths) {
      for (const path of paths) {
        fresh.push({ id: `f-${Date.now()}-${fresh.length}-${path}`, kind: "file", path, name: baseName(path) });
      }
    } else if (text) {
      fresh.push({ id: `t-${Date.now()}`, kind: "text", name: text.trim().slice(0, 40), text });
    }
    if (fresh.length === 0) return;
    setItems((prev) => {
      const taken = new Set(fresh.filter((i) => i.path).map((i) => i.path));
      const rest = prev.filter((i) => !(i.path && taken.has(i.path)));
      return [...fresh, ...rest].slice(0, MAX_ITEMS);
    });
    fresh.forEach((i) => { if (i.path) loadThumb(i.id, i.path); });
  }, [loadThumb]);

  useEffect(() => {
    if (!enabled || !windowLabel) return;
    const isNotch = windowLabel === "main" || windowLabel.startsWith("main-");
    if (!isNotch) return;
    invoke("setup_shelf_drop").catch(() => { });

    const unlistens: Promise<() => void>[] = [
      listen<{ label: string }>("shelf-drag-enter", (e) => {
        if (e.payload.label === windowLabel) cbRef.current.onDragEnter();
      }),
      listen<{ label: string }>("shelf-drag-leave", (e) => {
        if (e.payload.label === windowLabel) cbRef.current.onDragLeave();
      }),
      listen<{ label: string; paths?: string[]; text?: string }>("shelf-drag-drop", (e) => {
        add(e.payload.paths, e.payload.text);
        if (e.payload.label === windowLabel) cbRef.current.onDrop();
      }),
    ];
    return () => { unlistens.forEach((u) => u.then((fn) => fn())); };
  }, [enabled, windowLabel, add]);

  const remove = useCallback((id: string) => setItems((prev) => prev.filter((i) => i.id !== id)), []);
  const clear = useCallback(() => setItems([]), []);

  return { items, remove, clear };
}

type Props = {
  items: ShelfItem[];
  onRemove: (id: string) => void;
  onClear: () => void;
  onDragOutEnd: () => void;
};

const DRAG_THRESHOLD = 6;

export function NotchShelf({ items, onRemove, onClear, onDragOutEnd }: Props) {
  const t = useT();
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const press = useRef<{ x: number; y: number; id: string; dragging: boolean } | null>(null);
  const dragIcon = useRef<string | null>(null);

  useEffect(() => {
    invoke<string | null>("shelf_drag_icon").then((icon) => { dragIcon.current = icon; }).catch(() => { });
  }, []);

  const beginDrag = async (item: ShelfItem) => {
    if (item.kind !== "file" || !item.path) return;
    const icon = item.thumb ?? dragIcon.current;
    if (!icon) return;
    try {
      await startDrag({ item: [item.path], icon, mode: "copy" });
    } catch { /* drag was cancelled or refused */ }
    onDragOutEnd();
  };

  const activate = (item: ShelfItem) => {
    if (item.kind === "file" && item.path) {
      invoke("shelf_open", { path: item.path }).catch(() => { });
    } else if (item.text) {
      navigator.clipboard.writeText(item.text).then(() => {
        setCopiedId(item.id);
        setTimeout(() => setCopiedId((cur) => (cur === item.id ? null : cur)), 1200);
      }).catch(() => { });
    }
  };

  return (
    <div className="shelf-panel" onClick={(e) => e.stopPropagation()}>
      {items.length === 0 ? (
        <div className="shelf-empty">{t("shelf.empty")}</div>
      ) : (
        <>
          <div className="shelf-row">
            {items.map((item) => (
              <div
                key={item.id}
                className={`shelf-item ${item.kind}`}
                title={item.kind === "text" ? item.text : item.path}
                onMouseDown={(e) => {
                  if (e.button !== 0) return;
                  press.current = { x: e.clientX, y: e.clientY, id: item.id, dragging: false };
                }}
                onMouseMove={(e) => {
                  const p = press.current;
                  if (!p || p.id !== item.id || p.dragging || e.buttons !== 1) return;
                  if (Math.hypot(e.clientX - p.x, e.clientY - p.y) > DRAG_THRESHOLD) {
                    p.dragging = true;
                    beginDrag(item);
                  }
                }}
                onMouseUp={() => {
                  const p = press.current;
                  press.current = null;
                  if (p && p.id === item.id && !p.dragging) activate(item);
                }}
                onMouseLeave={() => { if (press.current?.id === item.id) press.current = null; }}
              >
                <div className="shelf-thumb">
                  {item.thumb ? <img src={item.thumb} alt="" draggable={false} /> : <span className="shelf-glyph">{item.kind === "text" ? "T" : "▤"}</span>}
                </div>
                <span className="shelf-name">{copiedId === item.id ? t("shelf.copied") : item.name}</span>
                <button
                  className="shelf-remove"
                  title={t("shelf.remove")}
                  onMouseDown={(e) => e.stopPropagation()}
                  onMouseUp={(e) => e.stopPropagation()}
                  onClick={(e) => { e.stopPropagation(); onRemove(item.id); }}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
          <button className="shelf-clear" onClick={onClear}>{t("shelf.clear")}</button>
        </>
      )}
    </div>
  );
}
