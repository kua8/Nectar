import { useEffect, useState } from "react";
import type { ChangeEvent, CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Eye, EyeOff, Circle, Search, Shuffle, MonitorCheck, CalendarDays, Maximize2, Keyboard, Sparkles, RotateCcw, LayoutGrid, Trash2, HardDrive, TextSearch } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import type { MonitorInfo, MonitorMode } from "./types";
import { usePerMonitor } from "./usePerMonitor";

const SEARCH_PROVIDERS = [
  { value: "windows", label: "Windows Search", desc: "Opens Windows Search (Win+S)" },
  { value: "cmdpal", label: "PowerToys Command Palette", desc: "Needs PowerToys with Command Palette turned on" },
  { value: "ptrun", label: "PowerToys Run", desc: "Needs PowerToys with PowerToys Run turned on" },
  { value: "raycast", label: "Raycast", desc: "Presses Raycast's shortcut; needs Raycast for Windows running" },
  { value: "shortcut", label: "Other Launcher", desc: "Presses your launcher's shortcut (Flow Launcher, Listary…)" },
];

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
      aria-label="Launcher shortcut"
      title={normalized === null ? "Use modifiers and one key, like Alt+Space or Ctrl+Shift+K" : undefined}
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
  dockTaskView: boolean;
  toggleDockTaskView: () => void;
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
  dockTaskView,
  toggleDockTaskView,
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
      <div className="setting-group-label">Dock</div>
      <div className="setting-group">
        <SettingRow icon={Monitor} label="Nectar Dock" desc="Replace Windows taskbar">
          <label className="toggle-switch">
            <input type="checkbox" checked={dockEnabled} onChange={toggleDock} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {dockEnabled && (
          <>
            {!showPerMonitor && (
              <SettingRow icon={dockMode === "fixed" ? EyeOff : Eye} label="Behavior" desc="Choose how the dock appears">
                <Dropdown
                  value={dockMode}
                  onChange={setDockModeValue}
                  options={[
                    { value: "fixed", label: "Fixed" },
                    { value: "smart", label: "Smart" },
                    { value: "peek", label: "Peek" },
                  ]}
                />
              </SettingRow>
            )}

            <SettingRow icon={MonitorCheck} label="Show On" desc="Choose which monitor(s) display the dock">
              <Dropdown
                value={dockMonitorMode}
                onChange={(v) => setDockMonitorModeValue(v as MonitorMode)}
                options={[
                  { value: "primary", label: "Primary Monitor" },
                  { value: "all", label: "All Monitors" },
                  { value: "specific", label: "Specific Monitor" },
                ]}
              />
            </SettingRow>

            {dockMonitorMode === "specific" && (
              <SettingRow icon={Monitor} label="Monitor" desc="Which display shows the dock">
                <Dropdown
                  value={dockMonitorId}
                  onChange={setDockMonitorIdValue}
                  placeholder={monitors.length === 0 ? "Loading..." : undefined}
                  options={monitors.map((m) => ({
                    value: m.id,
                    label: `${m.label}${m.is_primary ? " (Primary)" : ""}`,
                  }))}
                />
              </SettingRow>
            )}

            {multiMonitor && (
              <SettingRow icon={Monitor} label="Per-Monitor Behavior" desc="Set a different behavior for each monitor">
                <label className="toggle-switch">
                  <input type="checkbox" checked={perMonitor.enabled} onChange={perMonitor.toggle} />
                  <span className="slider"></span>
                </label>
              </SettingRow>
            )}

            {showPerMonitor && monitors.map((m) => (
              <SettingRow key={m.id} icon={Monitor} label={`${m.label} Behavior`} desc={m.is_primary ? "Primary monitor" : undefined}>
                <Dropdown
                  value={dockModeByMonitor[m.id] || dockMode}
                  onChange={(v) => setDockModeForMonitor(m.id, v)}
                  options={[
                    { value: "fixed", label: "Fixed" },
                    { value: "smart", label: "Smart" },
                    { value: "peek", label: "Peek" },
                  ]}
                />
              </SettingRow>
            ))}

            <SettingRow icon={Eye} label="Show App Previews" desc="Show window thumbnails on hover">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockPreviewEnabled} onChange={toggleDockPreview} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Search} label="Search Icon" desc="Show a search shortcut next to Start">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockSearchEnabled} onChange={toggleDockSearch} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {dockSearchEnabled && (
              <SettingRow
                icon={TextSearch}
                label="Search With"
                desc={(SEARCH_PROVIDERS.find((p) => p.value === dockSearchProvider) ?? SEARCH_PROVIDERS[0]).desc}
              >
                <Dropdown
                  value={dockSearchProvider}
                  onChange={handleDockSearchProviderChange}
                  options={SEARCH_PROVIDERS.map(({ value, label }) => ({ value, label }))}
                />
              </SettingRow>
            )}

            {dockSearchEnabled && (dockSearchProvider === "raycast" || dockSearchProvider === "shortcut") && (
              <SettingRow
                icon={Keyboard}
                label="Launcher Shortcut"
                desc={dockSearchProvider === "raycast" ? "Change this if you changed Raycast's hotkey" : "The shortcut that opens your launcher"}
              >
                <ShortcutField
                  value={dockSearchShortcut}
                  placeholder={dockSearchProvider === "raycast" ? "Alt+Space" : "e.g. Alt+Space"}
                  onCommit={handleDockSearchShortcutChange}
                />
              </SettingRow>
            )}

            <SettingRow icon={CalendarDays} label="Calendar Icon" desc="Show a shortcut that opens the Calendar window">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockCalendarEnabled} onChange={toggleDockCalendar} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {!legacyMode && (
              <>
                <SettingRow icon={LayoutGrid} label="Task View Icon" desc="Show a Task View button next to Start">
                  <label className="toggle-switch">
                    <input type="checkbox" checked={dockTaskView} onChange={toggleDockTaskView} />
                    <span className="slider"></span>
                  </label>
                </SettingRow>

                <SettingRow icon={Trash2} label="Recycle Bin" desc="Show the Recycle Bin in the dock">
                  <label className="toggle-switch">
                    <input type="checkbox" checked={dockRecycle} onChange={toggleDockRecycle} />
                    <span className="slider"></span>
                  </label>
                </SettingRow>

                {drives.map((d) => (
                  <SettingRow
                    key={d.letter}
                    icon={HardDrive}
                    label={`Drive ${d.letter}:`}
                    desc={d.label ? `${d.label}${d.removable ? " (removable)" : ""}` : d.removable ? "Removable drive" : "Local disk"}
                  >
                    <label className="toggle-switch">
                      <input type="checkbox" checked={dockDrives.includes(d.letter)} onChange={() => toggleDockDrive(d.letter)} />
                      <span className="slider"></span>
                    </label>
                  </SettingRow>
                ))}
              </>
            )}

            <SettingRow icon={Circle} label="Icon Only" desc="Remove icon background and padding">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockIconOnly} onChange={toggleDockIconOnly} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Keyboard} label="Win+Number Shortcuts" desc="Open pinned apps with Win+1 through Win+9">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockWinNumberEnabled} onChange={toggleDockWinNumber} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {dockMode === "fixed" && (
              <SettingRow icon={Maximize2} label="Adaptive Mode" desc="Stretch to full width when a window is maximized">
                <label className="toggle-switch">
                  <input type="checkbox" checked={dockAdaptive} onChange={toggleDockAdaptive} />
                  <span className="slider"></span>
                </label>
              </SettingRow>
            )}

            <SettingRow
              icon={Shuffle}
              label="Mix Pinned & Running"
              desc={dockMixedReorder ? "Drag icons anywhere, pinned or not" : "Pinned and running icons stay in separate groups"}
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
                  <span className="setting-label">Start Menu Icon</span>
                  <span className="setting-desc">Choose the dock start button icon</span>
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
                  title="Custom icon"
                >
                  {startIcon.startsWith("custom:") ? (
                    <img
                      src={startIcon.replace("custom:", "")}
                      alt="Custom"
                      style={{ width: "100%", height: "100%", objectFit: "contain", borderRadius: "8px" }}
                      draggable={false}
                    />
                  ) : (
                    <span style={{ fontSize: "20px" }}>+</span>
                  )}
                </div>
                {startIcon !== "default" && (
                  <div onClick={() => handleStartIconChange("default")} style={startIconTileStyle(false)} title="Reset to default">
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
          </>
        )}
      </div>
    </>
  );
}
