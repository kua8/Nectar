import { Power, Download, Clock, History, BatteryWarning, RefreshCw, LogOut, Languages } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { LANGUAGES, getLanguagePreference, setLanguagePreference, useT } from "../i18n";

interface GeneralTabProps {
  autostart: boolean;
  toggleAutostart: () => void;
  timeFormat24h: boolean;
  toggleTimeFormat24h: () => void;
  legacyMode: boolean;
  toggleLegacyMode: () => void;
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
  legacyMode,
  toggleLegacyMode,
  showUpdateIndicator,
  toggleUpdateIndicator,
  lowBatteryThreshold,
  handleThresholdChange,
  restartNectar,
  quitNectar,
}: GeneralTabProps) {
  const t = useT();
  const languageOptions = [
    { value: "system", label: t("general.language.system") },
    ...Object.entries(LANGUAGES).map(([value, { name }]) => ({ value, label: name })),
  ];
  return (
    <>
      <div className="setting-group-label">{t("general.system")}</div>
      <div className="setting-group">
        <SettingRow icon={Power} label={t("general.launchAtLogin")} desc={t("general.launchAtLogin.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={autostart} onChange={toggleAutostart} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={Download} label={t("general.updateIndicator")} desc={t("general.updateIndicator.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={showUpdateIndicator} onChange={toggleUpdateIndicator} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={Languages} label={t("general.language")} desc={t("general.language.desc")}>
          <Dropdown value={getLanguagePreference()} options={languageOptions} onChange={setLanguagePreference} />
        </SettingRow>

        <SettingRow icon={Clock} label={t("general.time24h")} desc={t("general.time24h.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={timeFormat24h} onChange={toggleTimeFormat24h} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow
          icon={History}
          label={t("general.legacyMode")}
          desc={t("general.legacyMode.desc")}
        >
          <label className="toggle-switch">
            <input type="checkbox" checked={legacyMode} onChange={toggleLegacyMode} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow
          icon={BatteryWarning}
          label={t("general.lowBattery")}
          desc={t("general.lowBattery.desc", { percent: lowBatteryThreshold })}
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

      <div className="setting-group-label">{t("general.app")}</div>
      <div className="setting-group">
        <SettingRow icon={RefreshCw} label={t("general.restart")} desc={t("general.restart.desc")} action onClick={restartNectar} />
        <SettingRow icon={LogOut} label={t("general.quit")} desc={t("general.quit.desc")} action danger onClick={quitNectar} divider={false} />
      </div>
    </>
  );
}
