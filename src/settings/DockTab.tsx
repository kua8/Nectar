import { Monitor, Eye, EyeOff, Circle, Search, Shuffle, MonitorCheck, CalendarDays } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import type { MonitorInfo, MonitorMode } from "./types";

interface DockTabProps {
  dockEnabled: boolean;
  toggleDock: () => void;
  dockMode: string;
  setDockModeValue: (mode: string) => void;
  dockPreviewEnabled: boolean;
  toggleDockPreview: () => void;
  dockSearchEnabled: boolean;
  toggleDockSearch: () => void;
  dockCalendarEnabled: boolean;
  toggleDockCalendar: () => void;
  dockIconOnly: boolean;
  toggleDockIconOnly: () => void;
  dockMixedReorder: boolean;
  toggleDockMixedReorder: () => void;
  monitors: MonitorInfo[];
  dockMonitorMode: MonitorMode;
  setDockMonitorModeValue: (mode: MonitorMode) => void;
  dockMonitorId: string;
  setDockMonitorIdValue: (id: string) => void;
  dockModeByMonitor: Record<string, string>;
  setDockModeForMonitor: (monitorId: string, mode: string | null) => void;
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
  dockCalendarEnabled,
  toggleDockCalendar,
  dockIconOnly,
  toggleDockIconOnly,
  dockMixedReorder,
  toggleDockMixedReorder,
  monitors,
  dockMonitorMode,
  setDockMonitorModeValue,
  dockMonitorId,
  setDockMonitorIdValue,
  dockModeByMonitor,
  setDockModeForMonitor,
}: DockTabProps) {
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

            {dockMonitorMode === "all" && monitors.length > 1 && monitors.map((m) => (
              <SettingRow key={m.id} icon={Monitor} label={`${m.label} Behavior`} desc={m.is_primary ? "Primary monitor" : undefined}>
                <Dropdown
                  value={dockModeByMonitor[m.id] || ""}
                  onChange={(v) => setDockModeForMonitor(m.id, v || null)}
                  options={[
                    { value: "", label: `Use default (${dockMode === "fixed" ? "Fixed" : dockMode === "smart" ? "Smart" : "Peek"})` },
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

            <SettingRow icon={Search} label="Search Icon" desc="Show a Windows Search shortcut next to Start">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockSearchEnabled} onChange={toggleDockSearch} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={CalendarDays} label="Calendar Icon" desc="Show a shortcut that opens the Calendar window">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockCalendarEnabled} onChange={toggleDockCalendar} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Circle} label="Icon Only" desc="Remove icon background and padding">
              <label className="toggle-switch">
                <input type="checkbox" checked={dockIconOnly} onChange={toggleDockIconOnly} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow
              icon={Shuffle}
              label="Mix Pinned & Running"
              desc={dockMixedReorder ? "Drag icons anywhere, pinned or not" : "Pinned and running icons stay in separate groups"}
              divider={false}
            >
              <label className="toggle-switch">
                <input type="checkbox" checked={dockMixedReorder} onChange={toggleDockMixedReorder} />
                <span className="slider"></span>
              </label>
            </SettingRow>
          </>
        )}
      </div>
    </>
  );
}
