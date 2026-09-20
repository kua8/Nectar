import { Monitor, Eye, EyeOff, Circle, Search, Shuffle } from "lucide-react";
import { SettingRow } from "./SettingRow";

interface DockTabProps {
  dockEnabled: boolean;
  toggleDock: () => void;
  dockMode: string;
  setDockModeValue: (mode: string) => void;
  dockPreviewEnabled: boolean;
  toggleDockPreview: () => void;
  dockSearchEnabled: boolean;
  toggleDockSearch: () => void;
  dockIconOnly: boolean;
  toggleDockIconOnly: () => void;
  dockMixedReorder: boolean;
  toggleDockMixedReorder: () => void;
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
  dockIconOnly,
  toggleDockIconOnly,
  dockMixedReorder,
  toggleDockMixedReorder,
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
              <select
                className="settings-select"
                value={dockMode}
                onChange={(e) => setDockModeValue(e.target.value)}
              >
                <option value="fixed">Fixed</option>
                <option value="smart">Smart</option>
                <option value="peek">Peek</option>
              </select>
            </SettingRow>

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
