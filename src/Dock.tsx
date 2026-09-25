import { useState, useEffect, useLayoutEffect, useMemo, useRef, memo } from 'react';
import { motion, AnimatePresence, Reorder } from 'framer-motion';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './Dock.css';
import { initTheme } from './theme';
import { useSettingsSync } from './hooks/useSettingsSync';

interface AppInfo {
  name: string;
  path: string;
  icon: string | null;
  is_running: boolean;
  is_pinned?: boolean;
  hwnd?: number;
  executable?: string;
  all_hwnds?: [number, string][];
}

const isNectarWindow = (app: { name: string; path: string }) =>
  app.path.toLowerCase().includes('nectar.exe') || app.name.toLowerCase() === 'nectar';

// Stable module-level constants so object references never change between renders,
// preventing Framer Motion from re-triggering animations on every re-render.
const ITEM_ENTRY_TRANSITION = {
  opacity: { duration: 0.15, delay: 0.15 },
  scale: { type: 'spring' as const, stiffness: 400, damping: 25, delay: 0.15 }
};
const ITEM_INITIAL = { opacity: 0, scale: 0 };
const ITEM_ANIMATE = { opacity: 1, scale: 1 };
const ITEM_EXIT = { opacity: 0, scale: 0 };

function SearchIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="11" cy="11" r="7" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
  );
}

const Dock = memo(function Dock() {
  useEffect(() => {
    return initTheme();
  }, []);

  const [pinnedApps, setPinnedApps] = useState<AppInfo[]>([]);
  const [activeApps, setActiveApps] = useState<AppInfo[]>([]);
  const iconsRef = useRef<Record<string, string>>({});
  const [, setIconsTick] = useState(0); 
  const [dockMode, setDockMode] = useState(() => {
    const raw = localStorage.getItem("nectar-dock-mode") || "fixed";
    if (raw === "auto-hide") return "smart";
    return raw;
  });
  const [dockPreviewEnabled, setDockPreviewEnabled] = useState(() => localStorage.getItem("nectar-dock-preview-enabled") !== "false");
  const [dockSearchEnabled, setDockSearchEnabled] = useState(() => localStorage.getItem("nectar-dock-search-enabled") !== "false");
  const [dockIconOnly, setDockIconOnly] = useState(() => localStorage.getItem("nectar-dock-icon-only") === "true");
  const [dockMixedReorder, setDockMixedReorder] = useState(() => localStorage.getItem("nectar-dock-mixed-reorder") === "true");
  const [mixedOrder, setMixedOrder] = useState<string[]>([]);
  const [previewData, setPreviewData] = useState<{ path: string, previews: { hwnd: number, title: string, image: string }[] } | null>(null);
  const [isDockHovered, setIsDockHovered] = useState(false);
  const [isEdgeHovered, setIsEdgeHovered] = useState(false);
  const [isOverlapped, setIsOverlapped] = useState(false);
  const [showAddPopup, setShowAddPopup] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number, y: number, app: AppInfo | null } | null>(null);
  const [contextMenuHeight, setContextMenuHeight] = useState(0);
  const [contextMenuWidth, setContextMenuWidth] = useState(0);
  const [activeSubmenu, setActiveSubmenu] = useState<string | null>(null);
  const [activeOrder, setActiveOrder] = useState<string[]>([]);
  const [isDragging, setIsDragging] = useState(false);
  const [hoveredApp, setHoveredApp] = useState<string | null>(null);
  const [pressedApp, setPressedApp] = useState<string | null>(null);
  const [isReady, setIsReady] = useState(false);
  const [isImpacted, setIsImpacted] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [startupAnimating, setStartupAnimating] = useState(false);
  const [customIcons, setCustomIcons] = useState<Record<string, string>>({});
  const [toast, setToast] = useState<string | null>(null);
  const iconPickerTargetRef = useRef<string | null>(null);
  const toastTimerRef = useRef<any>(null);
  const dockRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(() => parseFloat(localStorage.getItem("nectar-scale") || "1.0"));



  const isCurrentlyHovered = isDockHovered || isEdgeHovered;
  const [interactionState, setInteractionState] = useState<'active' | 'grace' | 'none'>('none');
  const isAnyInteraction = isCurrentlyHovered || !!contextMenu || showAddPopup;
  
  const previewTimerRef = useRef<any>(null);
  const isPreviewHoveredRef = useRef(false);
  const hoveredAppRef = useRef<string | null>(null);

  useEffect(() => {
    if (isAnyInteraction) {
      setInteractionState('active');
    } else if (interactionState !== 'none') {
      setInteractionState('grace');
      const timer = setTimeout(() => setInteractionState('none'), 800);
      return () => clearTimeout(timer);
    }
  }, [isAnyInteraction]);

  const isHidden = !startupAnimating && (
    (dockMode === 'smart' && isOverlapped && interactionState === 'none') ||
    (dockMode === 'peek' && interactionState === 'none')
  );

  useEffect(() => {
    let cleared = false;

    const checkVisibility = async (): Promise<boolean> => {
      try {
        const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
        const visible = await getCurrentWebviewWindow().isVisible();
        if (visible) {
          setStartupAnimating(true);
          setIsReady(true);
          setTimeout(() => setIsImpacted(true), 280);
          setTimeout(() => setIsExpanded(true), 350);
          setTimeout(() => setStartupAnimating(false), 1500);
          return true;
        }
      } catch (_) {}
      return false;
    };

    // Keep polling until visible — no time cap, since the dock can be
    // enabled at runtime from settings after any delay.
    const interval = setInterval(async () => {
      if (cleared) return;
      if (await checkVisibility()) {
        clearInterval(interval);
        cleared = true;
      }
    }, 200);

    // Also attempt immediately
    checkVisibility().then(ok => { if (ok) { clearInterval(interval); cleared = true; } });

    return () => { clearInterval(interval); cleared = true; };
  }, []);

  useEffect(() => {
    const updateRect = () => {
      if (dockRef.current) {
        const rect = dockRef.current.getBoundingClientRect();
        const hasPreview = !!previewData;
        invoke('update_dock_rect', {
          rect: {
            x: Math.round(rect.x) - (hasPreview ? 500 : 0),
            y: Math.round(rect.y) - (hasPreview ? 320 : 0),
            width: Math.round(rect.width) + (hasPreview ? 1000 : 0),
            height: Math.round(rect.height) + (hasPreview ? 320 : 0)
          }
        }).catch(() => {});
      }
    };

    updateRect();
    window.addEventListener('resize', updateRect);
    const observer = new ResizeObserver(updateRect);
    if (dockRef.current) observer.observe(dockRef.current);

    return () => {
      window.removeEventListener('resize', updateRect);
      observer.disconnect();
    };
  }, [pinnedApps, activeApps, isHidden, previewData, scale]);

  useEffect(() => {
    const init = async () => {
      const settings: any = await invoke('load_settings').catch(() => ({}));
      const getVal = (key: string, fallback: string | null = null) => {
        const val = settings[key];
        if (val !== undefined && val !== null) return String(val);
        const local = localStorage.getItem(key);
        if (local !== null) return local;
        return fallback;
      };
      
      const dMode = getVal("nectar-dock-mode", "fixed");
      if (dMode) {
        const mapped = dMode === "auto-hide" ? "smart" : dMode;
        setDockMode(mapped);
      }

      const preview = getVal("nectar-dock-preview-enabled", "true");
      setDockPreviewEnabled(preview === "true");

      const searchEnabled = getVal("nectar-dock-search-enabled", "true");
      setDockSearchEnabled(searchEnabled === "true");

      const iconOnly = getVal("nectar-dock-icon-only", "false");
      setDockIconOnly(iconOnly === "true");

      const mixedReorder = getVal("nectar-dock-mixed-reorder", "false");
      setDockMixedReorder(mixedReorder === "true");

      const scaleVal = getVal("nectar-scale");
      if (scaleVal !== null) setScale(parseFloat(scaleVal));

      const pinned = await invoke<AppInfo[]>('load_pinned_apps');
      setPinnedApps(pinned.map(a => ({ ...a, is_pinned: true })));
      pinned.forEach(app => fetchIcon(app.path));

      // Load custom icons
      try {
        const icons = await invoke<Record<string, string>>('get_custom_icons');
        setCustomIcons(icons);
      } catch (_) {}
    };
    init();

    const unlistenOverlap = listen<boolean>("dock-overlap", (event) => {
      setIsOverlapped(event.payload);
    });

    const unlistenEdgeHover = listen<boolean>("dock-edge-hover", (event) => {
      setIsEdgeHovered(event.payload);
    });

    return () => {
      unlistenOverlap.then(f => f());
      unlistenEdgeHover.then(f => f());
    };
  }, []);

  useSettingsSync(
    {
      "nectar-dock-mode": setDockMode,
      "nectar-dock-preview-enabled": setDockPreviewEnabled,
      "nectar-dock-search-enabled": setDockSearchEnabled,
      "nectar-dock-icon-only": setDockIconOnly,
      "nectar-dock-mixed-reorder": setDockMixedReorder,
      "nectar-scale": setScale,
    }
  );

  useEffect(() => {
    const poll = async () => {
      if (isDragging) return;
      const running = await invoke<AppInfo[]>('get_active_windows');
      setActiveApps(running);

      setActiveOrder(prev => {
        const newPaths = running.map(r => r.path);
        const existingPaths = prev.filter(p => newPaths.includes(p));
        const addedPaths = newPaths.filter(p => !prev.includes(p));
        return [...existingPaths, ...addedPaths];
      });

      running.forEach(app => fetchIcon(app.path, app.name, app.hwnd));
    };

    poll();

    const unlistenWindowChange = listen("windows-changed", () => {
      poll();
    });

    const unlistenSettingsReset = listen("settings-reset", () => {
      localStorage.clear();
      window.location.reload();
    });

    return () => {
      unlistenWindowChange.then(f => f());
      unlistenSettingsReset.then(f => f());
    };
  }, [isDragging]);

  const fetchIcon = async (path: string, name?: string, hwnd?: number, retryCount = 0) => {
    const isHost = path.toLowerCase().includes("msedge.exe") || path.toLowerCase().includes("chrome.exe") || path.toLowerCase().includes("applicationframehost.exe");
    const cacheKey = isHost && name ? `${path}:${name.toLowerCase()}` : (hwnd ? `${path}-${hwnd}` : path);
    
    if (iconsRef.current[cacheKey]) return;
    try {
      const icon = await invoke<string | null>('get_app_icon', { path, name: name || null, hwnd: hwnd || null });
      if (icon) {
        iconsRef.current[cacheKey] = icon;
        if (!isHost) iconsRef.current[path] = icon;
        setIconsTick(t => t + 1);
      } else if (retryCount < 3 && !hwnd) {
        setTimeout(() => fetchIcon(path, name, undefined, retryCount + 1), 3000 * (retryCount + 1));
      }
    } catch (e) {
      console.error(`Failed to fetch icon for ${path}:`, e);
      if (retryCount < 3 && !hwnd) {
        setTimeout(() => fetchIcon(path, name, undefined, retryCount + 1), 3000 * (retryCount + 1));
      }
    }
  };

  const handleClearIconCache = async () => {
    try {
      await invoke('clear_icon_cache');
      iconsRef.current = {};
      setIconsTick(t => t + 1);
      pinnedApps.forEach(app => fetchIcon(app.path));
    } catch (e) {
      console.error("Failed to clear icon cache:", e);
    }
  };

  const handleIconFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    const target = iconPickerTargetRef.current;
    if (!file || !target) return;

    const reader = new FileReader();
    reader.onload = async () => {
      const dataUri = reader.result as string;
      try {
        const newIcon = await invoke<string>('set_custom_icon', {
          cacheKey: target,
          iconData: dataUri
        });
        setCustomIcons(prev => ({ ...prev, [target]: newIcon }));
      } catch (err) {
        const msg = typeof err === 'string' ? err : 'Failed to set icon';
        if (toastTimerRef.current) clearTimeout(toastTimerRef.current);
        setToast(msg);
        toastTimerRef.current = setTimeout(() => setToast(null), 4000);
      }
    };
    reader.readAsDataURL(file);
    e.target.value = '';
  };

  const handleRemoveCustomIcon = async (app: AppInfo) => {
    try {
      await invoke('remove_custom_icon', { path: app.path, name: app.name || null });
      const isHost = app.path.toLowerCase().includes("msedge.exe") || app.path.toLowerCase().includes("chrome.exe") || app.path.toLowerCase().includes("applicationframehost.exe");
      const ck = isHost && app.name ? `${app.path}:${app.name.toLowerCase()}` : (app.hwnd ? `${app.path}-${app.hwnd}` : app.path);
      setCustomIcons(prev => {
        const next = { ...prev };
        delete next[ck];
        delete next[app.path];
        return next;
      });
      // Invalidate only this app's cached icon and re-fetch only this app
      delete iconsRef.current[ck];
      delete iconsRef.current[app.path];
      if (app.hwnd) delete iconsRef.current[`${app.path}-${app.hwnd}`];
      setIconsTick(t => t + 1);
      fetchIcon(app.path, app.name, app.hwnd);
    } catch (err) {
      console.error("Failed to remove custom icon:", err);
    }
  };

  const handleClosePreview = async (e: React.MouseEvent, hwnd: number) => {
    e.stopPropagation();
    try {
      await invoke('close_window', { hwnd });
      setPreviewData(prev => {
        if (!prev) return null;
        const remaining = prev.previews.filter(p => p.hwnd !== hwnd);
        if (remaining.length === 0) {
          setHoveredApp(null);
          return null;
        }
        return { ...prev, previews: remaining };
      });
    } catch (err) {
      console.error("Failed to close window:", err);
    }
  };

  const handleAppClick = async (app: AppInfo) => {
    try {
      if (app.path === 'start') {
        await invoke('open_app', { appName: 'start' });
      } else if (app.hwnd) {
        await invoke('focus_window', { hwnd: app.hwnd });
      } else {
        await invoke('open_app', { appName: app.path });
      }
    } catch (e) {
      console.error(`Failed to interact with ${app.name}:`, e);
    }
  };

  const togglePin = async (app: AppInfo) => {
    let newPinned;
    if (app.is_pinned) {
      newPinned = pinnedApps.filter(a => a.path !== app.path);
    } else {
      if (pinnedApps.find(a => a.path === app.path)) return;
      newPinned = [...pinnedApps, { ...app, is_pinned: true, is_running: false, hwnd: undefined }];
      fetchIcon(app.path, app.name); 
    }
    setPinnedApps(newPinned);
    await invoke('save_pinned_apps', { apps: newPinned });
    closeMenu();
  };

  const menuRef = useRef<HTMLDivElement>(null);
  const popupRef = useRef<HTMLDivElement>(null);



  const handleContextMenu = (e: React.MouseEvent, app: AppInfo | null) => {
    e.stopPropagation();
    e.preventDefault();
    setContextMenuHeight(0);
    setContextMenuWidth(0);
    const dockRect = dockRef.current?.getBoundingClientRect();
    const target = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setContextMenu({
      x: app ? target.left + target.width / 2 : (dockRect ? dockRect.left + dockRect.width / 2 : e.clientX),
      y: dockRect ? dockRect.top : e.clientY,
      app,
    });
  };

  const closeMenu = () => {
    setContextMenu(null);
    setActiveSubmenu(null);
    invoke('set_menu_open', { open: false, rect: null }).catch(() => {});
  };

  const closePopup = () => {
    setShowAddPopup(false);
    invoke('set_menu_open', { open: false, rect: null }).catch(() => {});
  };

  useLayoutEffect(() => {
    if (contextMenu && menuRef.current) {
      const measured = menuRef.current.offsetHeight;
      if (measured > 0 && measured !== contextMenuHeight) {
        setContextMenuHeight(measured);
      }
      const measuredWidth = menuRef.current.getBoundingClientRect().width;
      if (measuredWidth > 0 && Math.abs(measuredWidth - contextMenuWidth) > 0.5) {
        setContextMenuWidth(measuredWidth);
      }
    } else if (!contextMenu && (contextMenuHeight !== 0 || contextMenuWidth !== 0)) {
      setContextMenuHeight(0);
      setContextMenuWidth(0);
    }
  }, [contextMenu, contextMenuHeight, contextMenuWidth]);

  useEffect(() => {
    let open = false;
    let rect = null;

    if (contextMenu && menuRef.current) {
      const r = menuRef.current.getBoundingClientRect();
      rect = { 
        x: Math.round(r.x), 
        y: Math.round(r.y), 
        width: Math.round(r.width + (activeSubmenu ? 160 : 0)), 
        height: Math.round(r.height) 
      };
      open = true;
    } else if (showAddPopup && popupRef.current) {
      const r = popupRef.current.getBoundingClientRect();
      rect = { 
        x: Math.round(r.x), 
        y: Math.round(r.y), 
        width: Math.round(r.width), 
        height: Math.round(r.height) 
      };
      open = true;
    }

    invoke('set_menu_open', { open, rect }).catch(() => {});
  }, [contextMenu, showAddPopup, pinnedApps, activeApps, activeSubmenu, scale]);

  const dockItems = useMemo(() => {
    const getAppId = (p: string, executable?: string) => {
      if (!p) return "";
      const normalized = p.toLowerCase().replace(/\\/g, '/');
      if (executable) return `${normalized}:${executable.toLowerCase()}`;
      return normalized;
    };

    const runningMap = new Map();
    activeApps.forEach(a => {
      const id = getAppId(a.path, a.executable);
      if (!runningMap.has(id)) runningMap.set(id, a);
    });
    
    const matchedRunningKeys = new Set<string>();

    const findRunningApp = (p: AppInfo) => {
      // 1. Try exact match by getAppId
      const id = getAppId(p.path, p.executable);
      let running = runningMap.get(id);
      if (running) {
        matchedRunningKeys.add(getAppId(running.path, running.executable));
        return running;
      }

      // 2. Try match by path (without executable)
      const pathId = getAppId(p.path);
      running = runningMap.get(pathId);
      if (running) {
        matchedRunningKeys.add(getAppId(running.path, running.executable));
        return running;
      }

      // 3. Try fallback match by executable name if defined
      if (p.executable) {
        const targetExe = p.executable.toLowerCase();
        const found = activeApps.find(a => a.executable?.toLowerCase() === targetExe);
        if (found) {
          matchedRunningKeys.add(getAppId(found.path, found.executable));
          return found;
        }
      }

      // 4. Try fallback match by path's file name (e.g., if path is "msedge" and running app's executable is "msedge.exe")
      const pinFilename = p.path.split('/').pop()?.split('\\').pop()?.toLowerCase() || "";
      if (pinFilename) {
        const found = activeApps.find(a => {
          const runExe = a.executable?.toLowerCase() || a.path.split('/').pop()?.split('\\').pop()?.toLowerCase() || "";
          return runExe === pinFilename || runExe === `${pinFilename}.exe` || `${runExe}.exe` === pinFilename;
        });
        if (found) {
          matchedRunningKeys.add(getAppId(found.path, found.executable));
          return found;
        }
      }

      return undefined;
    };

    const pinned: AppInfo[] = [
      { name: 'Start', path: 'start', icon: null, is_running: false, is_pinned: true, hwnd: undefined, all_hwnds: undefined, executable: undefined },
      ...pinnedApps.map(p => {
        const running = findRunningApp(p);
        return { ...p, is_running: !!running, hwnd: running?.hwnd, all_hwnds: running?.all_hwnds };
      })
    ];

    const unpinned = activeOrder
      .map(path => activeApps.find(a => a.path.toLowerCase().replace(/\\/g, '/') === path.toLowerCase().replace(/\\/g, '/')))
      .filter((a): a is AppInfo => !!a && !matchedRunningKeys.has(getAppId(a.path, a.executable)));

    return [...pinned, ...unpinned];
  }, [pinnedApps, activeApps, activeOrder]);

  const startItem = useMemo(() => dockItems.find(i => i.path === 'start') as AppInfo, [dockItems]);
  const pinnedItems = useMemo(() => dockItems.filter(i => i.path !== 'start' && i.is_pinned), [dockItems]);
  const unpinnedItems = useMemo(() => dockItems.filter(i => !i.is_pinned), [dockItems]);

  const combinableItems = useMemo(() => dockItems.filter(i => i.path !== 'start'), [dockItems]);
  const mixedItems = useMemo(() => {
    if (!dockMixedReorder) return combinableItems;
    const known = combinableItems.filter(i => mixedOrder.includes(i.path));
    const unknown = combinableItems.filter(i => !mixedOrder.includes(i.path));
    known.sort((a, b) => mixedOrder.indexOf(a.path) - mixedOrder.indexOf(b.path));
    return [...known, ...unknown];
  }, [combinableItems, dockMixedReorder, mixedOrder]);

  const handleReorder = (newPaths: string[]) => {
    const oldPaths = pinnedApps.map(p => p.path);
    if (JSON.stringify(newPaths) !== JSON.stringify(oldPaths)) {
      const reordered = newPaths
        .map(path => pinnedApps.find(p => p.path === path))
        .filter((p): p is AppInfo => !!p);
      setPinnedApps(reordered);
    }
  };

  const handleDragEnd = () => {
    setIsDragging(false);
    setPressedApp(null);
    invoke('save_pinned_apps', { apps: pinnedApps }).catch(console.error);
  };

  const handleUnpinnedReorder = (newPaths: string[]) => {
    setActiveOrder(newPaths);
  };

  const handleUnpinnedDragEnd = () => {
    setIsDragging(false);
    setPressedApp(null);
  };

  const handleMixedReorder = (newPaths: string[]) => {
    setMixedOrder(newPaths);
    const pinnedPaths = new Set(pinnedApps.map(p => p.path));
    const newPinnedOrder = newPaths.filter(p => pinnedPaths.has(p));
    const oldPinnedOrder = pinnedApps.map(p => p.path);
    if (JSON.stringify(newPinnedOrder) !== JSON.stringify(oldPinnedOrder)) {
      const reordered = newPinnedOrder
        .map(path => pinnedApps.find(p => p.path === path))
        .filter((p): p is AppInfo => !!p);
      setPinnedApps(reordered);
    }
  };

  const handleMixedDragEnd = (app: AppInfo) => {
    setIsDragging(false);
    setPressedApp(null);
    if (app.is_pinned) {
      invoke('save_pinned_apps', { apps: pinnedApps }).catch(console.error);
    }
  };

  useEffect(() => {
    // Only report true dock-window hover, not the edge-hover from Rust,
    // to avoid a feedback loop that keeps the dock open.
    invoke('set_dock_hovered', { hovered: isDockHovered }).catch(() => {});
  }, [isDockHovered]);

  useEffect(() => {
    invoke('set_dock_dragging', { dragging: isDragging }).catch(() => {});
  }, [isDragging]);

  useEffect(() => {
    const handleBlur = () => {
      closeMenu();
      closePopup();
    };
    window.addEventListener('blur', handleBlur);
    return () => window.removeEventListener('blur', handleBlur);
  }, []);

  useEffect(() => {
    if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
    isPreviewHoveredRef.current = false;
    hoveredAppRef.current = hoveredApp;

    if (!hoveredApp) {
      const timer = setTimeout(() => setPreviewData(null), 100);
      return () => clearTimeout(timer);
    }

    if (hoveredApp && !isDragging) {
      const app = dockItems.find(a => a.path === hoveredApp);
      if (app && app.is_running) {
        const hwndsToCapture = app.all_hwnds || (app.hwnd ? [[app.hwnd, app.name]] : []);

        previewTimerRef.current = setTimeout(async () => {
          try {
            const results = await Promise.all(
              hwndsToCapture.map(async ([hwnd, title]) => {
                try {
                  const res = await invoke<[string, number] | null>("capture_window_thumbnail", { hwnd, maxWidth: 320, maxHeight: 200 });
                  if (res) {
                    const [image, lastFocused] = res;
                    return { hwnd, title, image, lastFocused };
                  }
                } catch {}
                return null;
              })
            );

            const captured = results
              .filter((r): r is { hwnd: number, title: string, image: string, lastFocused: number } => r !== null)
              .sort((a, b) => b.lastFocused - a.lastFocused)
              .map(({ hwnd, title, image }) => ({ hwnd, title, image }));

            const currentHovered = hoveredAppRef.current;
            if (captured.length > 0 && currentHovered === app.path) {
              setPreviewData({ path: app.path, previews: captured });
            } else if (currentHovered === app.path) {
              setPreviewData(null);
            }
          } catch (e) {
            console.error("Failed to capture thumbnails:", e);
            setPreviewData(null);
          }
        }, 300);
      } else {
        setPreviewData(null);
      }
    }

    return () => {
      if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
    };
  }, [hoveredApp, isDragging, dockItems]);

  const iconVariants = {
    idle: { y: 0, scale: 1 },
    hover: { y: -5, scale: 1.1 },
    drag: { y: -10, scale: 1.1, opacity: 0.8 },
    tap: { scale: 0.95 }
  };

  return (
    <div className={`dock-container ${isDragging ? 'dragging' : ''}`} onClick={closeMenu}>
      <div style={{ width: '100%', height: '100%', display: 'flex', justifyContent: 'center', alignItems: 'flex-end' }}>
        <motion.div
          ref={dockRef}
          layout
          className={`dock ${isExpanded && !isHidden ? 'dock-expanded' : ''} ${isImpacted && !isExpanded && !isHidden ? 'dock-impacted' : ''} ${dockIconOnly ? 'dock-icon-only' : ''}`}
          onMouseEnter={() => setIsDockHovered(true)}
          onMouseLeave={() => { setIsDockHovered(false); setHoveredApp(null); setPressedApp(null); }}
        initial={{ y: -800, opacity: 1, width: 34, height: 34, borderTopLeftRadius: 17, borderTopRightRadius: 17, borderBottomLeftRadius: 17, borderBottomRightRadius: 17 }}
        animate={{
          y: !isReady ? -800 : (isHidden ? 100 : 0),
          width: isExpanded && !isHidden ? 'auto' : 34,
          height: isExpanded && !isHidden ? 'auto' : 34,
          borderTopLeftRadius: (isImpacted || isExpanded) && !isHidden ? 18 : 17,
          borderTopRightRadius: (isImpacted || isExpanded) && !isHidden ? 18 : 17,
          borderBottomLeftRadius: (isImpacted || isExpanded) && !isHidden ? 0 : 17,
          borderBottomRightRadius: (isImpacted || isExpanded) && !isHidden ? 0 : 17,
          opacity: 1,
          scale: scale,
        }}
        transition={{
          y: { type: "spring", stiffness: 400, damping: 35, mass: 0.8 },
          width: { type: "spring", stiffness: 250, damping: 22, mass: 0.8 },
          height: { type: "spring", stiffness: 250, damping: 22, mass: 0.8 },
          layout: isDragging ? { duration: 0 } : { type: "spring", stiffness: 300, damping: 25 },
          borderTopLeftRadius: { type: "spring", stiffness: 500, damping: 30 },
          borderTopRightRadius: { type: "spring", stiffness: 500, damping: 30 },
          borderBottomLeftRadius: { type: "spring", stiffness: 500, damping: 30 },
          borderBottomRightRadius: { type: "spring", stiffness: 500, damping: 30 },
          opacity: { type: "tween", duration: 0.2 },
          scale: { duration: 0 },
        }}
        style={{ originX: 0.5, originY: 1, minWidth: 34 }}
        onContextMenu={(e) => handleContextMenu(e, null)}
      >
        <AnimatePresence>
          {isExpanded && (
            <motion.div
              key="dock-content"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="dock-reorder-container"
            >
              {startItem && (
                <motion.div
                  initial={{ opacity: 0, scale: 0 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ opacity: { duration: 0.15, delay: 0.15 }, scale: { type: "spring", stiffness: 400, damping: 25, delay: 0.15 } }}
                  className="dock-icon-wrapper"
                  onContextMenu={(e) => handleContextMenu(e, startItem)}
                  onMouseEnter={() => setHoveredApp(startItem.path)}
                  onMouseLeave={() => { setHoveredApp(null); setPressedApp(null); }}
                >
                  {(!dockPreviewEnabled || (dockPreviewEnabled && hoveredApp === startItem.path)) && (
                    <div className="tooltip">{startItem.name}</div>
                  )}
                  <motion.div 
                    className="dock-icon"
                    variants={iconVariants}
                    animate={pressedApp === startItem.path ? "tap" : (hoveredApp === startItem.path ? "hover" : "idle")}
                    onPointerDown={() => setPressedApp(startItem.path)}
                    onPointerUp={() => setPressedApp(null)}
                    onPointerCancel={() => setPressedApp(null)}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleAppClick(startItem);
                    }}
                  >
                    <img src="/nectar.png" alt="Nectar" className="nectar-icon-img" draggable={false} />
                  </motion.div>
                </motion.div>
              )}
              
              {dockSearchEnabled && (
                <motion.div
                  initial={{ opacity: 0, scale: 0 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ opacity: { duration: 0.15, delay: 0.18 }, scale: { type: "spring", stiffness: 400, damping: 25, delay: 0.18 } }}
                  className="dock-icon-wrapper"
                  onMouseEnter={() => setHoveredApp('search')}
                  onMouseLeave={() => { setHoveredApp(null); setPressedApp(null); }}
                >
                  {(!dockPreviewEnabled || (dockPreviewEnabled && hoveredApp === 'search')) && (
                    <div className="tooltip">Search</div>
                  )}
                  <motion.div
                    className="dock-icon"
                    variants={iconVariants}
                    animate={pressedApp === 'search' ? "tap" : (hoveredApp === 'search' ? "hover" : "idle")}
                    onPointerDown={() => setPressedApp('search')}
                    onPointerUp={() => setPressedApp(null)}
                    onPointerCancel={() => setPressedApp(null)}
                    onClick={(e) => {
                      e.stopPropagation();
                      invoke('open_app', { appName: 'search' }).catch((err) => console.error('Failed to open search:', err));
                    }}
                  >
                    <SearchIcon />
                  </motion.div>
                </motion.div>
              )}

              {!dockMixedReorder ? (
              <>
              <Reorder.Group
                as="div"
                axis="x"
                values={pinnedItems.map(i => i.path)}
                onReorder={handleReorder}
                className="dock-reorder-group"
              >
                {pinnedItems.map((app) => (
                  <Reorder.Item
                    as="div"
                    key={app.path}
                    value={app.path}
                    style={{ position: 'relative' }}
                    onDragStart={() => { setIsDragging(true); setHoveredApp(null); setPressedApp(null); }}
                    onDragEnd={handleDragEnd}
                    onContextMenu={(e) => handleContextMenu(e, app)}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!isDragging) handleAppClick(app);
                    }}
                  >
                    <motion.div
                      className="dock-icon-wrapper"
                      initial={ITEM_INITIAL}
                      animate={ITEM_ANIMATE}
                      exit={ITEM_EXIT}
                      transition={ITEM_ENTRY_TRANSITION}
                      onMouseEnter={() => setHoveredApp(app.path)}
                      onMouseLeave={() => { if (!isPreviewHoveredRef.current) { setHoveredApp(null); setPressedApp(null); } }}
                    >
                <AnimatePresence>
                  {dockPreviewEnabled && previewData && previewData.path === app.path && hoveredApp === app.path && (
                    <motion.div 
                      className={`preview-tooltip ${previewData.previews.length > 1 ? 'multi' : ''}`} 
                      initial={{opacity: 0, y: 10, scale: 0.95}} 
                      animate={{opacity: 1, y: 0, scale: 1}} 
                      exit={{opacity: 0, scale: 0.95}} 
                      transition={{duration: 0.15}}
                      onMouseEnter={() => { isPreviewHoveredRef.current = true; }}
                      onMouseLeave={() => { isPreviewHoveredRef.current = false; setHoveredApp(null); setPressedApp(null); }}
                    >
                      <div className="preview-items">
                        {previewData.previews.map((prev, idx) => (
                          <div key={prev.hwnd} className="preview-item" onClick={() => invoke('focus_window', { hwnd: prev.hwnd })}>
                            <img src={prev.image} alt={`Preview ${idx}`} />
                            <div className="preview-label">{prev.title || app.name}</div>
                            <button
                              className="preview-close-btn"
                              onClick={(e) => handleClosePreview(e, prev.hwnd)}
                              data-tooltip="Close Window"
                            >
                              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
                                <line x1="18" y1="6" x2="6" y2="18"></line>
                                <line x1="6" y1="6" x2="18" y2="18"></line>
                              </svg>
                            </button>
                          </div>
                        ))}
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
                
                {/* Fallback to text tooltip if previews are disabled, app isn't running, or preview failed to load */}
                {(!dockPreviewEnabled || (dockPreviewEnabled && hoveredApp === app.path && !previewData)) && (
                  <div className="tooltip">{app.name}</div>
                )}
                <motion.div 
                  className="dock-icon"
                  variants={iconVariants}
                  animate={pressedApp === app.path ? "tap" : (isDragging && !app.is_pinned ? "idle" : (hoveredApp === app.path && !isDragging ? "hover" : "idle"))}
                  whileDrag="drag"
                  onPointerDown={() => setPressedApp(app.path)}
                  onPointerUp={() => setPressedApp(null)}
                  onPointerCancel={() => setPressedApp(null)}
                >
                  {(() => {
                    const isHost = app.path.toLowerCase().includes("msedge.exe") || app.path.toLowerCase().includes("chrome.exe") || app.path.toLowerCase().includes("applicationframehost.exe");
                    const cacheKey = isHost ? `${app.path}:${app.name.toLowerCase()}` : (app.hwnd ? `${app.path}-${app.hwnd}` : app.path);
                    const icon = customIcons[cacheKey] || customIcons[app.path] || iconsRef.current[cacheKey] || iconsRef.current[app.path] || app.icon;
                    
                    const isNectarOrSettings = isNectarWindow(app);
                    
                    return (icon || isNectarOrSettings) ? (
                      <img 
                        src={isNectarOrSettings ? "/nectar.png" : (icon ?? undefined)} 
                        alt={app.name} 
                        className={isNectarOrSettings ? "nectar-icon-img" : ""} 
                        draggable={false} 
                      />
                    ) : (
                      <div className="fallback-icon">{app.name[0]}</div>
                    );
                  })()}
                </motion.div>
                  {app.is_running && <div className="active-indicator" />}
                    </motion.div>
                  </Reorder.Item>
                ))}
              </Reorder.Group>

              {pinnedItems.length > 0 && unpinnedItems.length > 0 && (
                <div className="dock-group-divider" />
              )}

              <Reorder.Group
                as="div"
                axis="x"
                values={unpinnedItems.map(i => i.path)}
                onReorder={handleUnpinnedReorder}
                className="dock-reorder-group"
              >
                {unpinnedItems.map((app) => (
                  <Reorder.Item
                    as="div"
                    key={app.path}
                    value={app.path}
                    style={{ position: 'relative' }}
                    onDragStart={() => { setIsDragging(true); setHoveredApp(null); setPressedApp(null); }}
                    onDragEnd={handleUnpinnedDragEnd}
                    onContextMenu={(e) => handleContextMenu(e, app)}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!isDragging) handleAppClick(app);
                    }}
                  >
                    <motion.div
                      className="dock-icon-wrapper"
                      initial={{ opacity: 0, scale: 0 }}
                      animate={{ opacity: 1, scale: 1 }}
                      exit={{ opacity: 0, scale: 0, transition: { duration: 0.12 } }}
                      transition={ITEM_ENTRY_TRANSITION}
                      onMouseEnter={() => setHoveredApp(app.path)}
                      onMouseLeave={() => { if (!isPreviewHoveredRef.current) { setHoveredApp(null); setPressedApp(null); } }}
                    >
                      <AnimatePresence>
                        {dockPreviewEnabled && previewData && previewData.path === app.path && hoveredApp === app.path && (
                          <motion.div
                            className={`preview-tooltip ${previewData.previews.length > 1 ? 'multi' : ''}`}
                            initial={{opacity: 0, y: 10, scale: 0.95}}
                            animate={{opacity: 1, y: 0, scale: 1}}
                            exit={{opacity: 0, scale: 0.95}}
                            transition={{duration: 0.15}}
                            onMouseEnter={() => { isPreviewHoveredRef.current = true; }}
                            onMouseLeave={() => { isPreviewHoveredRef.current = false; setHoveredApp(null); setPressedApp(null); }}
                          >
                            <div className="preview-items">
                              {previewData.previews.map((prev, idx) => (
                                <div key={prev.hwnd} className="preview-item" onClick={() => invoke('focus_window', { hwnd: prev.hwnd })}>
                                  <img src={prev.image} alt={`Preview ${idx}`} />
                                  <div className="preview-label">{prev.title || app.name}</div>
                                  <button
                                    className="preview-close-btn"
                                    onClick={(e) => handleClosePreview(e, prev.hwnd)}
                                    data-tooltip="Close Window"
                                  >
                                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
                                      <line x1="18" y1="6" x2="6" y2="18"></line>
                                      <line x1="6" y1="6" x2="18" y2="18"></line>
                                    </svg>
                                  </button>
                                </div>
                              ))}
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>
                      {(!dockPreviewEnabled || (dockPreviewEnabled && hoveredApp === app.path && !previewData)) && (
                        <div className="tooltip">{app.name}</div>
                      )}
                      <motion.div
                        className="dock-icon"
                        variants={iconVariants}
                        animate={pressedApp === app.path ? "tap" : (isDragging && app.is_pinned ? "idle" : (hoveredApp === app.path && !isDragging ? "hover" : "idle"))}
                        whileDrag="drag"
                        onPointerDown={() => setPressedApp(app.path)}
                        onPointerUp={() => setPressedApp(null)}
                        onPointerCancel={() => setPressedApp(null)}
                      >
                        {(() => {
                          const isHost = app.path.toLowerCase().includes("msedge.exe") || app.path.toLowerCase().includes("chrome.exe") || app.path.toLowerCase().includes("applicationframehost.exe");
                          const cacheKey = isHost ? `${app.path}:${app.name.toLowerCase()}` : (app.hwnd ? `${app.path}-${app.hwnd}` : app.path);
                        const icon = customIcons[cacheKey] || customIcons[app.path] || iconsRef.current[cacheKey] || iconsRef.current[app.path] || app.icon;
                          const isNectarOrSettings = isNectarWindow(app);
                          return (icon || isNectarOrSettings) ? (
                            <img src={isNectarOrSettings ? "/nectar.png" : (icon ?? undefined)} alt={app.name} className={isNectarOrSettings ? "nectar-icon-img" : ""} draggable={false} />
                          ) : (
                            <div className="fallback-icon">{app.name[0]}</div>
                          );
                        })()}
                      </motion.div>
                      {app.is_running && <div className="active-indicator" />}
                    </motion.div>
                  </Reorder.Item>
                ))}
              </Reorder.Group>
              </>
              ) : (
              <Reorder.Group
                as="div"
                axis="x"
                values={mixedItems.map(i => i.path)}
                onReorder={handleMixedReorder}
                className="dock-reorder-group"
              >
                {mixedItems.map((app) => (
                  <Reorder.Item
                    as="div"
                    key={app.path}
                    value={app.path}
                    style={{ position: 'relative' }}
                    onDragStart={() => { setIsDragging(true); setHoveredApp(null); setPressedApp(null); }}
                    onDragEnd={() => handleMixedDragEnd(app)}
                    onContextMenu={(e) => handleContextMenu(e, app)}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!isDragging) handleAppClick(app);
                    }}
                  >
                    <motion.div
                      className="dock-icon-wrapper"
                      initial={ITEM_INITIAL}
                      animate={ITEM_ANIMATE}
                      exit={ITEM_EXIT}
                      transition={ITEM_ENTRY_TRANSITION}
                      onMouseEnter={() => setHoveredApp(app.path)}
                      onMouseLeave={() => { if (!isPreviewHoveredRef.current) { setHoveredApp(null); setPressedApp(null); } }}
                    >
                      <AnimatePresence>
                        {dockPreviewEnabled && previewData && previewData.path === app.path && hoveredApp === app.path && (
                          <motion.div
                            className={`preview-tooltip ${previewData.previews.length > 1 ? 'multi' : ''}`}
                            initial={{opacity: 0, y: 10, scale: 0.95}}
                            animate={{opacity: 1, y: 0, scale: 1}}
                            exit={{opacity: 0, scale: 0.95}}
                            transition={{duration: 0.15}}
                            onMouseEnter={() => { isPreviewHoveredRef.current = true; }}
                            onMouseLeave={() => { isPreviewHoveredRef.current = false; setHoveredApp(null); setPressedApp(null); }}
                          >
                            <div className="preview-items">
                              {previewData.previews.map((prev, idx) => (
                                <div key={prev.hwnd} className="preview-item" onClick={() => invoke('focus_window', { hwnd: prev.hwnd })}>
                                  <img src={prev.image} alt={`Preview ${idx}`} />
                                  <div className="preview-label">{prev.title || app.name}</div>
                                  <button
                                    className="preview-close-btn"
                                    onClick={(e) => handleClosePreview(e, prev.hwnd)}
                                    data-tooltip="Close Window"
                                  >
                                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
                                      <line x1="18" y1="6" x2="6" y2="18"></line>
                                      <line x1="6" y1="6" x2="18" y2="18"></line>
                                    </svg>
                                  </button>
                                </div>
                              ))}
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>

                      {(!dockPreviewEnabled || (dockPreviewEnabled && hoveredApp === app.path && !previewData)) && (
                        <div className="tooltip">{app.name}</div>
                      )}
                      <motion.div
                        className="dock-icon"
                        variants={iconVariants}
                        animate={pressedApp === app.path ? "tap" : (hoveredApp === app.path && !isDragging ? "hover" : "idle")}
                        whileDrag="drag"
                        onPointerDown={() => setPressedApp(app.path)}
                        onPointerUp={() => setPressedApp(null)}
                        onPointerCancel={() => setPressedApp(null)}
                      >
                        {(() => {
                          const isHost = app.path.toLowerCase().includes("msedge.exe") || app.path.toLowerCase().includes("chrome.exe") || app.path.toLowerCase().includes("applicationframehost.exe");
                          const cacheKey = isHost ? `${app.path}:${app.name.toLowerCase()}` : (app.hwnd ? `${app.path}-${app.hwnd}` : app.path);
                          const icon = customIcons[cacheKey] || customIcons[app.path] || iconsRef.current[cacheKey] || iconsRef.current[app.path] || app.icon;
                          const isNectarOrSettings = isNectarWindow(app);
                          return (icon || isNectarOrSettings) ? (
                            <img src={isNectarOrSettings ? "/nectar.png" : (icon ?? undefined)} alt={app.name} className={isNectarOrSettings ? "nectar-icon-img" : ""} draggable={false} />
                          ) : (
                            <div className="fallback-icon">{app.name[0]}</div>
                          );
                        })()}
                      </motion.div>
                      {app.is_running && <div className="active-indicator" />}
                    </motion.div>
                  </Reorder.Item>
                ))}
              </Reorder.Group>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>

      {contextMenu && (
        <div
          style={{
            position: 'fixed',
            left: Math.min(
              Math.max(8, contextMenu.x - (contextMenuWidth || 160 * scale) / 2),
              Math.max(8, window.innerWidth - (contextMenuWidth || 160 * scale) - 8),
            ),
            top: Math.max(8, contextMenu.y - 10 - (contextMenuHeight || (contextMenu.app ? 200 : 100) * scale)),
            zIndex: 9999,
          }}
        >
        <div
          ref={menuRef}
          className="context-menu"
          style={{
            position: 'static',
            zoom: scale
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.app ? (
            <>
              <div className="menu-item" onClick={() => togglePin(contextMenu.app!)}>
                {contextMenu.app.is_pinned ? 'Unpin from Dock' : 'Pin to Dock'}
              </div>
              {contextMenu.app.is_pinned && contextMenu.app.path !== 'start' && (
                <>
                  <div className="menu-divider" />
                  <div className="menu-item" onClick={() => {
                    const isHost = contextMenu.app!.path.toLowerCase().includes("msedge.exe") || contextMenu.app!.path.toLowerCase().includes("chrome.exe") || contextMenu.app!.path.toLowerCase().includes("applicationframehost.exe");
                    const ck = isHost ? `${contextMenu.app!.path}:${contextMenu.app!.name.toLowerCase()}` : contextMenu.app!.path;
                    iconPickerTargetRef.current = ck;
                    closeMenu();
                    setTimeout(() => {
                      document.getElementById('icon-file-input')?.click();
                    }, 50);
                  }}>
                    Change Icon...
                  </div>
                  {(() => {
                    const isHost = contextMenu.app!.path.toLowerCase().includes("msedge.exe") || contextMenu.app!.path.toLowerCase().includes("chrome.exe") || contextMenu.app!.path.toLowerCase().includes("applicationframehost.exe");
                    const ck = isHost ? `${contextMenu.app!.path}:${contextMenu.app!.name.toLowerCase()}` : contextMenu.app!.path;
                    return customIcons[ck] ? (
                      <div className="menu-item" onClick={() => {
                        handleRemoveCustomIcon(contextMenu.app!);
                        closeMenu();
                      }}>
                        Reset Icon
                      </div>
                    ) : null;
                  })()}
                </>
              )}
              <div className="menu-divider" />
              <div className="menu-item" onClick={() => { setShowAddPopup(true); closeMenu(); }}>
                Add App to Dock...
              </div>
              <div 
                className="menu-item has-submenu"
                onMouseEnter={() => setActiveSubmenu('nectar')}
                onMouseLeave={() => setActiveSubmenu(null)}
              >
                Nectar Options
                <span className="submenu-arrow">▶</span>
                <div className="submenu">
                  <div className="menu-item" onClick={() => { invoke('open_settings_window'); closeMenu(); }}>Open Settings</div>
                  <div className="menu-item" onClick={() => invoke('restart_nectar')}>Restart Nectar</div>
                  <div className="menu-item" onClick={() => { handleClearIconCache(); closeMenu(); }}>Clear Icon Cache</div>
                  <div className="menu-divider" />
                  <div className="menu-item quit" onClick={() => invoke('quit_nectar')}>Quit Nectar</div>
                </div>
              </div>
              {contextMenu.app.is_running && (
                <>
                  <div className="menu-divider" />
                  <div className="menu-item quit" onClick={async () => {
                    if (contextMenu.app?.hwnd) {
                      await invoke('end_task', {
                        hwnd: contextMenu.app.hwnd,
                        allHwnds: contextMenu.app.all_hwnds?.map(([h]) => h) ?? null,
                      });
                      closeMenu();
                    }
                  }}>
                    Quit {contextMenu.app.name}
                  </div>
                </>
              )}
            </>
          ) : (
            <>
              <div className="menu-item" onClick={() => { setShowAddPopup(true); closeMenu(); }}>
                Add App to Dock...
              </div>
              <div 
                className="menu-item has-submenu"
                onMouseEnter={() => setActiveSubmenu('nectar')}
                onMouseLeave={() => setActiveSubmenu(null)}
              >
                Nectar Options
                <span className="submenu-arrow">▶</span>
                <div className="submenu">
                  <div className="menu-item" onClick={() => { invoke('open_settings_window'); closeMenu(); }}>Open Settings</div>
                  <div className="menu-item" onClick={() => invoke('restart_nectar')}>Restart Nectar</div>
                  <div className="menu-item" onClick={() => { handleClearIconCache(); closeMenu(); }}>Clear Icon Cache</div>
                  <div className="menu-divider" />
                  <div className="menu-item quit" onClick={() => invoke('quit_nectar')}>Quit Nectar</div>
                </div>
              </div>
            </>
          )}
        </div>
        </div>
      )}

      <input
        id="icon-file-input"
        type="file"
        accept=".png,.ico,.jpg,.jpeg,.svg,.bmp"
        style={{ display: 'none' }}
        onChange={handleIconFileSelect}
      />

      <AnimatePresence>
        {showAddPopup && (
          <AddAppPopup 
            containerRef={popupRef}
            onClose={closePopup} 
            onAdd={(app: AppInfo) => { togglePin(app); closePopup(); }}
            scale={scale}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {toast && (
          <motion.div
            className="dock-toast"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            transition={{ duration: 0.2 }}
            style={{ zoom: scale }}
          >
            {toast}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

function AddAppPopup({ onClose, onAdd, containerRef, scale }: {
  onClose: () => void,
  onAdd: (app: AppInfo) => void,
  containerRef: React.RefObject<HTMLDivElement | null>,
  scale: number
}) {
  const [apps, setApps] = useState<AppInfo[]>([]);
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const [listIcons, setListIcons] = useState<Record<string, string>>({});
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), 150);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Reset selection when search changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [debouncedSearch]);

  // Scroll selected item into view
  useEffect(() => {
    if (!listRef.current) return;
    const row = listRef.current.children[selectedIndex] as HTMLElement | undefined;
    if (row) row.scrollIntoView({ block: 'nearest' });
  }, [selectedIndex]);

  useEffect(() => {
    const load = async () => {
      try {
        const res = await invoke<AppInfo[]>('get_installed_apps');
        setApps(res.sort((a, b) => a.name.localeCompare(b.name)));
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  const filtered = useMemo(() => {
    const s = debouncedSearch.toLowerCase();
    if (!s) return apps.slice(0, 20);
    return apps.filter(a => a.name.toLowerCase().includes(s)).slice(0, 50);
  }, [apps, debouncedSearch]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex(i => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex(i => Math.max(i - 1, 0));
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (filtered[selectedIndex]) {
          onAdd(filtered[selectedIndex]);
        }
      }
    };
    const handleMouseDown = (e: MouseEvent) => {
      const popup = containerRef.current;
      if (popup && !popup.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleBlur = () => onClose();
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('blur', handleBlur);
    document.addEventListener('mousedown', handleMouseDown, true);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('blur', handleBlur);
      document.removeEventListener('mousedown', handleMouseDown, true);
    };
  }, [onClose, containerRef, filtered, selectedIndex, onAdd]);

  useEffect(() => {
    let active = true;
    const fetchVisibleIcons = async () => {
      let batch: Record<string, string> = {};
      let count = 0;
      for (const app of filtered) {
        if (!active) break;
        if (!listIcons[app.path]) {
          await new Promise(r => setTimeout(r, 20));
          try {
            const icon = await invoke<string | null>('get_app_icon', { path: app.path });
            if (icon && active) {
              batch[app.path] = icon;
              count++;
              if (count >= 6) {
                setListIcons(prev => ({ ...prev, ...batch }));
                batch = {};
                count = 0;
              }
            }
          } catch (err) {
            console.error(err);
          }
        }
      }
      if (active && count > 0) setListIcons(prev => ({ ...prev, ...batch }));
    };
    fetchVisibleIcons();
    return () => { active = false; };
  }, [filtered]);

  return (
    <div className="add-popup-anchor" style={{ zoom: scale }}>
      <motion.div
        ref={containerRef}
        className="add-app-popup"
        style={{ transformOrigin: "bottom center" }}
        initial={{ opacity: 0, scaleY: 0 }}
        animate={{ opacity: 1, scaleY: 1 }}
        exit={{ opacity: 0, scaleY: 0 }}
        transition={{
          opacity: { duration: 0.15 },
          scaleY: { type: "spring", stiffness: 500, damping: 30, mass: 0.8 },
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="popup-search-row">
          <svg className="popup-search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="8"/>
            <line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <input
            ref={inputRef}
            type="text"
            className="popup-search-input"
            placeholder="Search apps..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <div className="popup-apps-scroll" ref={listRef}>
          {loading ? (
            <div className="popup-loading">
              <div className="popup-spinner" />
            </div>
          ) : filtered.length > 0 ? (
            filtered.map((app, idx) => {
              const icon = listIcons[app.path];
              return (
                <div
                  key={app.path}
                  className={`popup-app-row${idx === selectedIndex ? ' selected' : ''}`}
                  onClick={() => onAdd(app)}
                  onMouseEnter={() => setSelectedIndex(idx)}
                >
                  <div className="popup-app-icon">
                    {icon ? (
                      <img src={icon} alt="" draggable={false} />
                    ) : (
                      <span className="popup-app-initial">{app.name[0]}</span>
                    )}
                  </div>
                  <span className="popup-app-name">{app.name}</span>
                  <span className="popup-app-pin">+</span>
                </div>
              );
            })
          ) : (
            <div className="popup-empty">No results</div>
          )}
        </div>
      </motion.div>
    </div>
  );
}

export default Dock;
