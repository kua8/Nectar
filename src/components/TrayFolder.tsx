import { useEffect, useRef, useState } from "react";
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

  // Only scan while the dock is on screen and the feature is wanted; the scan walks every
  // top-level window, so it should not run for a hidden dock.
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
    // that happens means the pulse starts with the flash instead of up to a poll interval later.
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

/** Sends a click to a tray icon's real location. */
const clickIcon = (icon: TrayIcon, right: boolean) =>
  invoke("tray_click", { key: icon.key, right }).catch(() => {});

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
    // The popover animates in, so measure after the first frame and again once it settles.
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
 * The dock's notification area: a folder-shaped button the size of a dock icon, showing the
 * first four tray apps as a grid. Hovering opens the full list; clicking an icon opens that app.
 */
export function TrayFolder({
  icons,
  open,
  onOpenChange,
  onHoverChange,
}: {
  icons: TrayIcon[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onHoverChange: (hovered: boolean) => void;
}) {
  const t = useT();
  const images = useTrayIconImages(icons);
  const [hovered, setHovered] = useState(false);
  const closeTimer = useRef<number | undefined>(undefined);

  const show = hovered || open;
  const popoverRef = useMenuHitArea(show);

  // The native notification area opens as soon as the pointer is over it, so this does too. A
  // short close delay is the only concession: crossing the corner of the folder on the way to
  // something else should not make the list appear and vanish twice.
  const enter = () => {
    window.clearTimeout(closeTimer.current);
    setHovered(true);
    onOpenChange(true);
    onHoverChange(true);
  };
  const leave = () => {
    window.clearTimeout(closeTimer.current);
    closeTimer.current = window.setTimeout(() => {
      setHovered(false);
      onOpenChange(false);
      onHoverChange(false);
    }, 160);
  };

  useEffect(() => () => window.clearTimeout(closeTimer.current), []);

  const slots = icons.slice(0, GRID_SLOTS);

  return (
    <div
      className="tray-folder-wrapper"
      onMouseEnter={enter}
      onMouseLeave={leave}
    >
      <div className="tray-folder" role="button" aria-label={t("dockmenu.hiddenIcons")}>
        <div className="tray-folder-grid">
          {Array.from({ length: GRID_SLOTS }).map((_, i) => {
            const icon = slots[i];
            return (
              <div
                key={icon?.key ?? `empty-${i}`}
                className={`tray-folder-cell${icon?.attention ? " attention" : ""}`}
              >
                {icon ? (
                  <img
                    src={images[icon.key] || undefined}
                    alt=""
                    draggable={false}
                    onError={(e) => { (e.currentTarget as HTMLImageElement).style.visibility = "hidden"; }}
                  />
                ) : null}
              </div>
            );
          })}
        </div>
        {/* A small chevron marks this as something that opens, not an app to launch. */}
        <span className="tray-folder-chevron" aria-hidden>
          <svg viewBox="0 0 8 8" width="8" height="8">
            <path d="M2 3 L4 5 L6 3" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </span>
      </div>

      <AnimatePresence>
        {show && icons.length > 0 && (
          <motion.div
            ref={popoverRef}
            className="tray-popover"
            initial={{ opacity: 0, y: 6, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 4, scale: 0.97 }}
            transition={{ type: "spring", stiffness: 480, damping: 34 }}
            onMouseEnter={enter}
            onMouseLeave={leave}
          >
            <div className="tray-popover-grid">
              {icons.map((icon) => (
                <button
                  key={icon.key}
                  className={`tray-item${icon.attention ? " attention" : ""}`}
                  title={icon.name || icon.app}
                  onClick={() => clickIcon(icon, false)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    clickIcon(icon, true);
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

