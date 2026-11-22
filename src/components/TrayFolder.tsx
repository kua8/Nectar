import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import { useT } from "../i18n";

/** One icon in the notification area, as reported by `tray_list`. */
export interface TrayIcon {
  /** Stable identity across scans: notification-area window handle plus icon id. */
  key: string;
  pid: number;
  /** Executable stem, e.g. "Weixin". */
  app: string;
  /** Friendly name from the executable, when it has one. */
  name: string;
  /** Full path to the owning executable, used to resolve its icon. */
  path: string;
  left: number;
  top: number;
  width: number;
  height: number;
  /** The app wants attention: a window of it flashed. Pulses until opened. */
  attention: boolean;
}

/** How many icons the collapsed folder shows as a 2x2 grid. */
const GRID_SLOTS = 4;

/** Icons are rescanned while the folder is open; tray apps add and remove icons at will. */
const REFRESH_MS = 3000;

/**
 * Icons for the notification area.
 *
 * Windows 11's taskbar no longer exposes its tray icon list, so this asks the Rust side to scan
 * for notification-area windows and their icon rectangles. Clicking an icon clicks its real
 * pixels, because that is the only way a tray icon receives a click.
 */
export function useTrayIcons(enabled: boolean) {
  const [icons, setIcons] = useState<TrayIcon[]>([]);
  const [open, setOpen] = useState(false);

  const refresh = useRef(() => {});

  refresh.current = () => {
    invoke<TrayIcon[]>("tray_list")
      .then((list) => setIcons(Array.isArray(list) ? list : []))
      .catch(() => setIcons([]));
  };

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    const tick = () => {
      if (cancelled) return;
      refresh.current();
    };
    tick();
    const timer = window.setInterval(tick, open ? REFRESH_MS : REFRESH_MS * 4);

    // A window flashing is the only "look at me" signal most apps send. Rescanning as soon as
    // that happens means the pulse starts with the flash, not a poll interval later.
    const unlistenAttention = listen("app-attention", tick);
    const unlistenWindows = listen("windows-changed", tick);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
      unlistenAttention.then((fn) => fn()).catch(() => {});
      unlistenWindows.then((fn) => fn()).catch(() => {});
    };
  }, [enabled, open]);

  return { icons, open, setOpen, refresh: () => refresh.current() };
}

/**
 * A click on a notification-area icon, with the meaning the real shell gives it:
 *
 *   left          open the app / its menu — what the icon does on a normal click
 *   right         the context menu, where "Safely Remove Hardware" lives
 *   double left   the shell's alternate action (usually the same as left)
 *   middle        the shell's third action, rarely used
 */
const clickIcon = (icon: TrayIcon, button: "left" | "right" | "middle", double = false) =>
  invoke("tray_click", { key: icon.key, button, double }).catch(() => {});

/**
 * The dock is a click-through window: the backend decides per cursor position whether the dock
 * takes the click. A popover drawn above the dock sits outside the dock's own rectangle, so the
 * backend would keep passing clicks through to whatever is underneath. `set_menu_open` tells it
 * a popover is up and where — the same mechanism the dock's context menu uses.
 */
function useMenuHitArea(open: boolean) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      invoke("set_menu_open", { open: false, rect: null }).catch(() => {});
      return;
    }
    const report = () => {
      const el = ref.current;
      if (!el) return;
      const r = el.getBoundingClientRect();
      invoke("set_menu_open", {
        open: true,
        rect: { x: r.left, y: r.top, width: r.width, height: r.height },
      }).catch(() => {});
    };
    const first = requestAnimationFrame(report);
    const second = window.setTimeout(report, 220);
    return () => {
      cancelAnimationFrame(first);
      window.clearTimeout(second);
      invoke("set_menu_open", { open: false, rect: null }).catch(() => {});
    };
  }, [open]);

  return ref;
}

/**
 * Whether the pointer is over the folder or its popover.
 *
 * `onMouseLeave` is not enough: the popover is drawn above the folder with a gap between them,
 * and moving across that gap fires a leave on one and an enter on the other. Closing is driven
 * by where the pointer actually is instead, so the gap does not dismiss the list.
 */
function usePointerWithin(ref: React.RefObject<HTMLElement | null>, active: boolean, onOutside: () => void) {
  useEffect(() => {
    if (!active) return;
    let timer = 0;
    const check = (e: PointerEvent) => {
      const el = ref.current;
      if (!el) return;
      const r = el.getBoundingClientRect();
      // A margin covers the gap between the folder and the popover and small cursor jitter.
      const pad = 12;
      const inside =
        e.clientX >= r.left - pad && e.clientX <= r.right + pad &&
        e.clientY >= r.top - pad && e.clientY <= r.bottom + pad;
      if (inside) {
        window.clearTimeout(timer);
        timer = 0;
      } else if (!timer) {
        // Leave a moment to come back, the way the native tray waits before closing.
        timer = window.setTimeout(() => {
          timer = 0;
          onOutside();
        }, 220);
      }
    };
    window.addEventListener("pointermove", check);
    return () => {
      window.removeEventListener("pointermove", check);
      window.clearTimeout(timer);
    };
  }, [ref, active, onOutside]);
}

/**
 * The dock's notification area: a folder-shaped button the size of a dock icon, showing the
 * first four tray apps as a grid. Hovering opens the full list, and it stays open until the
 * pointer leaves both the folder and the list.
 */
export function TrayFolder({
  icons,
  open,
  onOpenChange,
}: {
  icons: TrayIcon[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const t = useT();
  const images = useTrayIconImages(icons);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const popoverRef = useMenuHitArea(open);
  const lastClick = useRef<{ key: string; at: number }>({ key: "", at: 0 });

  const close = useCallback(() => {
    onOpenChange(false);
  }, [onOpenChange]);

  usePointerWithin(wrapperRef, open, close);

  const slots = icons.slice(0, GRID_SLOTS);

  /**
   * Native double-click is a second click on the same icon inside the system interval. The
   * first click already went through, exactly as it would in the real tray.
   */
  const handleClick = (icon: TrayIcon, button: "left" | "right" | "middle") => {
    const now = Date.now();
    const isDouble =
      button === "left" && lastClick.current.key === icon.key && now - lastClick.current.at < 500;
    lastClick.current = { key: isDouble ? "" : icon.key, at: now };
    clickIcon(icon, button, isDouble);
  };

  return (
    <div ref={wrapperRef} className="tray-folder-wrapper">
      <div
        className="tray-folder"
        role="button"
        aria-label={t("dockmenu.hiddenIcons")}
        onMouseEnter={() => onOpenChange(true)}
      >
        <div className="tray-folder-grid">
          {Array.from({ length: GRID_SLOTS }).map((_, i) => {
            const icon = slots[i];
            if (!icon) {
              return <div key={`empty-${i}`} className="tray-folder-cell" />;
            }
            return (
              <div
                key={icon.key}
                className={`tray-folder-cell${icon.attention ? " attention" : ""}`}
                title={icon.name || icon.app}
                onClick={(e) => {
                  e.stopPropagation();
                  handleClick(icon, "left");
                }}
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  handleClick(icon, "right");
                }}
                onAuxClick={(e) => {
                  if (e.button !== 1) return;
                  e.stopPropagation();
                  handleClick(icon, "middle");
                }}
              >
                <img
                  src={images[icon.key] || undefined}
                  alt=""
                  draggable={false}
                  onError={(e) => { (e.currentTarget as HTMLImageElement).style.visibility = "hidden"; }}
                />
              </div>
            );
          })}
        </div>
        <span className="tray-folder-chevron" aria-hidden>
          <svg viewBox="0 0 8 8" width="8" height="8">
            <path d="M2 3 L4 5 L6 3" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </span>
      </div>

      <AnimatePresence>
        {open && icons.length > 0 && (
          <motion.div
            ref={popoverRef}
            className="tray-popover"
            initial={{ opacity: 0, y: 6, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 4, scale: 0.97 }}
            transition={{ type: "spring", stiffness: 480, damping: 34 }}
          >
            <div className="tray-popover-grid">
              {icons.map((icon) => (
                <button
                  key={icon.key}
                  className={`tray-item${icon.attention ? " attention" : ""}`}
                  title={icon.name || icon.app}
                  onClick={() => handleClick(icon, "left")}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    handleClick(icon, "right");
                  }}
                  onAuxClick={(e) => {
                    if (e.button !== 1) return;
                    handleClick(icon, "middle");
                  }}
                >
                  <img src={images[icon.key] || undefined} alt="" draggable={false} />
                  <span className="tray-item-name">{icon.name || icon.app}</span>
                </button>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/**
 * Resolves an icon image for each tray app.
 *
 * The tray bitmaps themselves live inside the shell's own UI and cannot be read, so this uses
 * the owning executable's icon — the same source the dock already uses for running apps.
 */
function useTrayIconImages(icons: TrayIcon[]) {
  const [images, setImages] = useState<Record<string, string>>({});

  useEffect(() => {
    let cancelled = false;
    const missing = icons.filter((icon) => icon.path && !images[icon.key]);
    if (missing.length === 0) return;

    Promise.all(
      missing.map(async (icon) => {
        try {
          const data = await invoke<string | null>("get_app_icon", {
            path: icon.path,
            name: icon.app || null,
            hwnd: null,
          });
          return [icon.key, data ?? ""] as const;
        } catch {
          return [icon.key, ""] as const;
        }
      }),
    ).then((entries) => {
      if (cancelled) return;
      setImages((prev) => {
        const next = { ...prev };
        for (const [key, data] of entries) if (data) next[key] = data;
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [icons, images]);

  return images;
}
