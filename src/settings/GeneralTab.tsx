import { Power, Download, Clock, BatteryWarning, RefreshCw, LogOut } from "lucide-react";
import { SettingRow } from "./SettingRow";

interface GeneralTabProps {
  autostart: boolean;
  toggleAutostart: () => void;
  timeFormat24h: boolean;
  toggleTimeFormat24h: () => void;
  showUpdateIndicator: boolean;
  toggleUpdateIndicator: () => void;
  lowBatteryThreshold: number;
  handleThresholdChange: (val: number) => void;
  restartNectar: () => void;
  quitNectar: () => void;
}

export function GeneralTab({
  autostart,
  toggleAutostart,
  timeFormat24h,
  toggleTimeFormat24h,
  showUpdateIndicator,
  toggleUpdateIndicator,
  lowBatteryThreshold,
  handleThresholdChange,
  restartNectar,
  quitNectar,
}: GeneralTabProps) {
  return (
    <>
      <div className="setting-group-label">System</div>
      <div className="setting-group">
        <SettingRow icon={Power} label="Launch at Login" desc="Open Nectar automatically">
          <label className="toggle-switch">
            <input type="checkbox" checked={autostart} onChange={toggleAutostart} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={Download} label="Update Indicator" desc="Show green dot when update available">
          <label className="toggle-switch">
            <input type="checkbox" checked={showUpdateIndicator} onChange={toggleUpdateIndicator} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={Clock} label="24-Hour Time" desc="Use 24-hour clock format">
          <label className="toggle-switch">
            <input type="checkbox" checked={timeFormat24h} onChange={toggleTimeFormat24h} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow
          icon={BatteryWarning}
          label="Low Battery Alert"
          desc={`Trigger at ${lowBatteryThreshold}%`}
          divider={false}
        >
          <input
            type="range"
            min="5"
            max="50"
            step="5"
            value={lowBatteryThreshold}
            onChange={(e) => handleThresholdChange(parseInt(e.target.value))}
            className="settings-slider"
          />
        </SettingRow>
      </div>

      <div className="setting-group-label">App</div>
      <div className="setting-group">
        <SettingRow icon={RefreshCw} label="Restart Nectar" desc="Reinitialize all components" action onClick={restartNectar} />
        <SettingRow icon={LogOut} label="Quit Nectar" desc="Exit application completely" action danger onClick={quitNectar} divider={false} />
      </div>
    </>
  );
}
