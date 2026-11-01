import { useEffect, useState } from "react";
import type { ChangeEvent, CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Eye, EyeOff, Circle, Search, Shuffle, MonitorCheck, CalendarDays, Maximize2, Keyboard, Sparkles, RotateCcw, LayoutGrid, Trash2, HardDrive, ChevronUp, TextSearch } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { SettingSection } from "./SettingSection";
import { Dropdown } from "./Dropdown";
import type { MonitorInfo, MonitorMode } from "./types";
import { usePerMonitor } from "./usePerMonitor";
import { useT } from "../i18n";

const SEARCH_PROVIDERS = ["windows", "cmdpal", "ptrun", "raycast", "shortcut"];

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

function ShortcutField({ value, placeholder, onCommit }: { value: string; placeholder: string; onCommit: (shortcut: string) => void }) {
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
      aria-label={t("dock.searchShortcut")}
      title={normalized === null ? t("dock.searchShortcut.invalid") : undefined}
    />
  );
}

const START_ICON_PRESETS = [
  { key: "default", src: "/nectar.png", label: "Nectar" },
  { key: "windows", src: "/windows.png", label: "Windows" },
];

const startIconTileStyle = (active: boolean): CSSProperties => ({
  width: "48px",
  height: "48px",
  borderRadius: "12px",
  border: active ? "2px solid var(--nectar-accent, #007aff)" : "2px solid rgba(255,255,255,0.1)",
  background: active ? "rgba(0,122,255,0.15)" : "rgba(255,255,255,0.05)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "pointer",
  transition: "all 0.15s ease",
  padding: "6px",
  color: "rgba(255,255,255,0.5)",
});

interface DriveInfo {
  letter: string;
  label: string;
  removable: boolean;
}

interface DockTabProps {
  dockEnabled: boolean;
  toggleDock: () => void;
  dockMode: string;
  setDockModeValue: (mode: string) => void;
  dockPreviewEnabled: boolean;
  toggleDockPreview: () => void;
  dockSearchEnabled: boolean;
  toggleDockSearch: () => void;
  dockSearchProvider: string;
  handleDockSearchProviderChange: (provider: string) => void;
  dockSearchShortcut: string;
  handleDockSearchShortcutChange: (shortcut: string) => void;
  dockCalendarEnabled: boolean;
  toggleDockCalendar: () => void;
  legacyMode: boolean;
  dockTray: boolean;
  toggleDockTray: () => void;
  dockTaskView: boolean;
  toggleDockTaskView: () => void;
  dockShowDesktop: boolean;
  toggleDockShowDesktop: () => void;
  dockRecycle: boolean;
  toggleDockRecycle: () => void;
  dockDrives: string[];
  toggleDockDrive: (letter: string) => void;
  dockIconOnly: boolean;
  toggleDockIconOnly: () => void;
  dockAdaptive: boolean;
  toggleDockAdaptive: () => void;
  startIcon: string;
  handleStartIconChange: (icon: string) => void;
  dockWinNumberEnabled: boolean;
  toggleDockWinNumber: () => void;
  dockMixedReorder: boolean;
  toggleDockMixedReorder: () => void;
  monitors: MonitorInfo[];
  dockMonitorMode: MonitorMode;
  setDockMonitorModeValue: (mode: MonitorMode) => void;
  dockMonitorId: string;
  setDockMonitorIdValue: (id: string) => void;
  dockModeByMonitor: Record<string, string>;
  setDockModeForMonitor: (monitorId: string, mode: string | null) => void;
  replaceDockModes: (next: Record<string, string>) => void;
}

export function DockTab({
  dockEnabled,
  toggleDock,
  dockMode,
  setDockModeValue,
  dockPreviewEnabled,
  toggleDockPreview,
  dockSearchEnabled,
  toggleDockSearch,
  dockSearchProvider,
  handleDockSearchProviderChange,
  dockSearchShortcut,
  handleDockSearchShortcutChange,
  dockCalendarEnabled,
  toggleDockCalendar,
  legacyMode,
  dockTray,
  toggleDockTray,
  dockTaskView,
  toggleDockTaskView,
  dockShowDesktop,
  toggleDockShowDesktop,
  dockRecycle,
  toggleDockRecycle,
  dockDrives,
  toggleDockDrive,
  dockIconOnly,
  toggleDockIconOnly,
  dockAdaptive,
  toggleDockAdaptive,
  startIcon,
  handleStartIconChange,
  dockWinNumberEnabled,
  toggleDockWinNumber,
  dockMixedReorder,
  toggleDockMixedReorder,
  monitors,
  dockMonitorMode,
  setDockMonitorModeValue,
  dockMonitorId,
  setDockMonitorIdValue,
  dockModeByMonitor,
  setDockModeForMonitor,
  replaceDockModes,
}: DockTabProps) {
  const t = useT();
  const perMonitor = usePerMonitor(dockMode, dockModeByMonitor, replaceDockModes, monitors);
  const multiMonitor = dockMonitorMode === "all" && monitors.length > 1;
  const showPerMonitor = multiMonitor && perMonitor.enabled;
  const [drives, setDrives] = useState<DriveInfo[]>([]);

  useEffect(() => {
    invoke<DriveInfo[]>("get_drives").then(setDrives).catch(() => {});
  }, []);

  const handleStartIconUpload = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      handleStartIconChange(`custom:${reader.result as string}`);
    };
    reader.readAsDataURL(file);
    e.target.value = "";
  };

  return (
    <>
      <SettingSection id="dock-main" title={t("dock.group")} defaultOpen>
        <SettingRow icon={Monitor} label={t("dock.enable")} desc={t("dock.enable.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={dockEnabled} onChange={toggleDock} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {dockEnabled && (
          <>
            {!showPerMonitor && (
              <SettingRow icon={dockMode === "fixed" ? EyeOff : Eye} label={t("dock.behavior")} desc={t("dock.behavior.desc")}>
                <Dropdown
                  value={dockMode}
                  onChange={setDockModeValue}
                  options={[
                    { value: "fixed", label: t("common.fixed") },
                    { value: "smart", label: t("common.smart") },
                    { value: "peek", label: t("common.peek") },
                  ]}
                />
              </SettingRow>
            )}

            <SettingRow icon={MonitorCheck} label={t("common.showOn")} desc={t("dock.showOn.desc")}>
              <Dropdown
                value={dockMonitorMode}
                onChange={(v) => setDockMonitorModeValue(v as MonitorMode)}
                options={[
                  { value: "primary", label: t("common.monitors.primary") },
                  { value: "all", label: t("common.monitors.all") },
                  { value: "specific", label: t("common.monitors.specific") },
                ]}
              />
            </SettingRow>

            {dockMonitorMode === "specific" && (
              <SettingRow icon={Monitor} label={t("common.monitor")} desc={t("dock.monitor.desc")}>
                <Dropdown
                  value={dockMonitorId}
                  onChange={setDockMonitorIdValue}
                  placeholder={monitors.length === 0 ? t("common.loading") : undefined}
                  options={monitors.map((m) => ({
                    value: m.id,
                    label: `${m.label}${m.is_primary ? ` (${t("common.primary")})` : ""}`,
                  }))}
                />
              </SettingRow>
            )}

            {multiMonitor && (
              <SettingRow icon={Monitor} label={t("common.perMonitor")} desc={t("common.perMonitor.desc")}>
                <label className="toggle-switch">
                  <input type="checkbox" checked={perMonitor.enabled} onChange={perMonitor.toggle} />
                  <span className="slider"></span>
                </label>
              </SettingRow>
            )}

            {showPerMonitor && monitors.map((m) => (
              <SettingRow key={m.id} icon={Monitor} label={t("common.monitorBehavior", { name: m.label })} desc={m.is_primary ? t("common.primaryMonitor") : undefined}>
                <Dropdown
                  value={dockModeByMonitor[m.id] || dockMode}
                  onChange={(v) => setDockModeForMonitor(m.id, v)}
                  options={[
                    { value: "fixed", label: t("common.fixed") },
                    { value: "smart", label: t("common.smart") },
                    { value: "peek", label: t("common.peek") },
                  ]}
                />
              </SettingRow>
            ))}

          </>
        )}
      </SettingSection>

      {dockEnabled && (
        <>
          <SettingSection id="dock-shortcuts" title={t("dock.section.shortcuts")}>
            <SettingRow icon={Eye} label={t("dock.previews")} desc={t("dock.previews.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={dockPreviewEnabled} onChange={toggleDockPreview} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Search} label={t("dock.search")} desc={t("dock.search.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={dockSearchEnabled} onChange={toggleDockSearch} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {dockSearchEnabled && (
              <SettingRow
                icon={TextSearch}
                label={t("dock.searchWith")}
                desc={t(`dock.searchWith.${SEARCH_PROVIDERS.includes(dockSearchProvider) ? dockSearchProvider : "windows"}.desc`)}
              >
                <Dropdown
                  value={dockSearchProvider}
                  onChange={handleDockSearchProviderChange}
                  options={SEARCH_PROVIDERS.map((value) => ({ value, label: t(`dock.searchWith.${value}`) }))}
                />
              </SettingRow>
            )}

            {dockSearchEnabled && (dockSearchProvider === "raycast" || dockSearchProvider === "shortcut") && (
              <SettingRow
                icon={Keyboard}
                label={t("dock.searchShortcut")}
                desc={t(dockSearchProvider === "raycast" ? "dock.searchShortcut.descRaycast" : "dock.searchShortcut.desc")}
              >
                <ShortcutField
                  value={dockSearchShortcut}
                  placeholder={dockSearchProvider === "raycast" ? "Alt+Space" : t("dock.searchShortcut.placeholder")}
                  onCommit={handleDockSearchShortcutChange}
                />
              </SettingRow>
            )}

            <SettingRow icon={CalendarDays} label={t("dock.calendar")} desc={t("dock.calendar.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={dockCalendarEnabled} onChange={toggleDockCalendar} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {!legacyMode && (
              <>
                <SettingRow icon={ChevronUp} label={t("dock.tray")} desc={t("dock.tray.desc")}>
                  <label className="toggle-switch">
                    <input type="checkbox" checked={dockTray} onChange={toggleDockTray} />
                    <span className="slider"></span>
                  </label>
                </SettingRow>

                <SettingRow icon={LayoutGrid} label={t("dock.taskView")} desc={t("dock.taskView.desc")}>
                  <label className="toggle-switch">
                    <input type="checkbox" checked={dockTaskView} onChange={toggleDockTaskView} />
                    <span className="slider"></span>
                  </label>
                </SettingRow>

                <SettingRow icon={Monitor} label={t("dock.showDesktop")} desc={t("dock.showDesktop.desc")}>
                  <label className="toggle-switch">
                    <input type="checkbox" checked={dockShowDesktop} onChange={toggleDockShowDesktop} />
                    <span className="slider"></span>
                  </label>
                </SettingRow>

                <SettingRow icon={Trash2} label={t("dock.recycleBin")} desc={t("dock.recycleBin.desc")}>
                  <label className="toggle-switch">
                    <input type="checkbox" checked={dockRecycle} onChange={toggleDockRecycle} />
                    <span className="slider"></span>
                  </label>
                </SettingRow>

                {drives.map((d) => (
                  <SettingRow
                    key={d.letter}
                    icon={HardDrive}
                    label={t("dock.drive", { letter: d.letter })}
                    desc={d.label ? `${d.label}${d.removable ? ` (${t("dock.drive.removableSuffix")})` : ""}` : d.removable ? t("dock.drive.removable") : t("dock.drive.local")}
                  >
                    <label className="toggle-switch">
                      <input type="checkbox" checked={dockDrives.includes(d.letter)} onChange={() => toggleDockDrive(d.letter)} />
                      <span className="slider"></span>
                    </label>
                  </SettingRow>
                ))}
              </>
            )}

          </SettingSection>

          <SettingSection id="dock-style" title={t("dock.section.style")}>
            <SettingRow icon={Circle} label={t("dock.iconOnly")} desc={t("dock.iconOnly.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={dockIconOnly} onChange={toggleDockIconOnly} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Keyboard} label={t("dock.winNumber")} desc={t("dock.winNumber.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={dockWinNumberEnabled} onChange={toggleDockWinNumber} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {dockMode === "fixed" && (
              <SettingRow icon={Maximize2} label={t("dock.adaptive")} desc={t("dock.adaptive.desc")}>
                <label className="toggle-switch">
                  <input type="checkbox" checked={dockAdaptive} onChange={toggleDockAdaptive} />
                  <span className="slider"></span>
                </label>
              </SettingRow>
            )}

            <SettingRow
              icon={Shuffle}
              label={t("dock.mixed")}
              desc={dockMixedReorder ? t("dock.mixed.on") : t("dock.mixed.off")}
            >
              <label className="toggle-switch">
                <input type="checkbox" checked={dockMixedReorder} onChange={toggleDockMixedReorder} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <div className="setting-item" style={{ flexDirection: "column", alignItems: "flex-start", gap: "10px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "10px", width: "100%" }}>
                <div className="setting-icon-bg">
                  <Sparkles size={14} strokeWidth={1.5} />
                </div>
                <div className="setting-info">
                  <span className="setting-label">{t("dock.startIcon")}</span>
                  <span className="setting-desc">{t("dock.startIcon.desc")}</span>
                </div>
              </div>
              <div style={{ display: "flex", gap: "8px", flexWrap: "wrap", paddingLeft: "34px" }}>
                {START_ICON_PRESETS.map((icon) => (
                  <div
                    key={icon.key}
                    onClick={() => handleStartIconChange(icon.key)}
                    style={startIconTileStyle(startIcon === icon.key)}
                    title={icon.label}
                  >
                    <img src={icon.src} alt={icon.label} style={{ width: "100%", height: "100%", objectFit: "contain" }} draggable={false} />
                  </div>
                ))}
                <div
                  onClick={() => document.getElementById("start-icon-file-input")?.click()}
                  style={startIconTileStyle(startIcon.startsWith("custom:"))}
                  title={t("dock.startIcon.custom")}
                >
                  {startIcon.startsWith("custom:") ? (
                    <img
                      src={startIcon.replace("custom:", "")}
                      alt={t("dock.startIcon.customAlt")}
                      style={{ width: "100%", height: "100%", objectFit: "contain", borderRadius: "8px" }}
                      draggable={false}
                    />
                  ) : (
                    <span style={{ fontSize: "20px" }}>+</span>
                  )}
                </div>
                {startIcon !== "default" && (
                  <div onClick={() => handleStartIconChange("default")} style={startIconTileStyle(false)} title={t("dock.startIcon.reset")}>
                    <RotateCcw size={18} strokeWidth={1.5} />
                  </div>
                )}
              </div>
              <input
                id="start-icon-file-input"
                type="file"
                accept=".png,.ico,.jpg,.jpeg,.svg,.bmp"
                style={{ display: "none" }}
                onChange={handleStartIconUpload}
              />
            </div>
          </SettingSection>
        </>
      )}
    </>
  );
}
