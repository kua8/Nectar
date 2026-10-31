import {
  PanelTop,
  Calendar,
  Music,
  Minimize2,
  LayoutList,
  Sparkles,
  Circle,
  CloudSun,
  X,
  Monitor,
  MonitorCheck,
  CloudCog,
  ChevronRight,
  Hourglass,
  Timer,
  BellRing,
  Globe,
  Volume2,
} from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useCaldav } from "../hooks/useCaldav";
import { StatusWidgetConfig } from "../components/StatusWidgetConfig";
import { DualClockPicker } from "../components/DualClockPicker";
import { zoneOffsetLabel } from "../dualClock";
import type { WidgetConfig, MonitorInfo, MonitorMode } from "./types";
import { usePerMonitor } from "./usePerMonitor";
import { useT } from "../i18n";

interface NotchTabProps {
  notchMode: string;
  setNotchModeValue: (mode: string) => void;
  monitors: MonitorInfo[];
  notchMonitorMode: MonitorMode;
  setNotchMonitorModeValue: (mode: MonitorMode) => void;
  notchMonitorId: string;
  setNotchMonitorIdValue: (id: string) => void;
  notchModeByMonitor: Record<string, string>;
  setNotchModeForMonitor: (monitorId: string, mode: string | null) => void;
  replaceNotchModes: (next: Record<string, string>) => void;
  calendarEnabled: boolean;
  toggleCalendar: () => void;
  timerEnabled: boolean;
  toggleTimerEnabled: () => void;
  timerSoundEnabled: boolean;
  toggleTimerSound: () => void;
  notchTimerProgress: boolean;
  toggleNotchTimerProgress: () => void;
  stopwatchEnabled: boolean;
  toggleStopwatchEnabled: () => void;
  legacyMode: boolean;
  dualClockEnabled: boolean;
  toggleDualClock: () => void;
  dualClockZone: string;
  dualClockLabel: string;
  selectDualClockZone: (option: { label: string; zone: string }) => void;
  musicModeEnabled: boolean;
  toggleMusicMode: () => void;
  musicCompactNotch: boolean;
  toggleMusicCompactNotch: () => void;
  mediaLayout: "classic" | "compact";
  toggleMediaLayout: (layout: "classic" | "compact") => void;
  mediaAmbienceEnabled: boolean;
  toggleAmbience: () => void;
  mediaCompactGlowEnabled: boolean;
  toggleCompactGlow: () => void;
  notchScrollVolume: boolean;
  toggleNotchScrollVolume: () => void;
  shelfPersist: boolean;
  toggleShelfPersist: () => void;
  rememberMode: boolean;
  toggleRememberMode: () => void;
  weatherEnabled: boolean;
  toggleWeather: () => void;
  tempUnitFahrenheit: boolean;
  toggleTempUnit: () => void;
  cityName: string;
  setCityName: (name: string) => void;
  citySearchResults: Array<{ name: string; country: string; latitude: number; longitude: number }>;
  showCityDropdown: boolean;
  setShowCityDropdown: (show: boolean) => void;
  selectCity: (city: { name: string; country: string; latitude: number; longitude: number }) => void;
  handleCityClear: () => void;
  statusWidgets: WidgetConfig;
  handleWidgetsChange: (config: WidgetConfig) => void;
  hasBattery: boolean;
  onOpenCalendarSync: () => void;
}

export function NotchTab({
  notchMode,
  setNotchModeValue,
  monitors,
  notchMonitorMode,
  setNotchMonitorModeValue,
  notchMonitorId,
  setNotchMonitorIdValue,
  notchModeByMonitor,
  setNotchModeForMonitor,
  replaceNotchModes,
  calendarEnabled,
  toggleCalendar,
  timerEnabled,
  toggleTimerEnabled,
  timerSoundEnabled,
  toggleTimerSound,
  notchTimerProgress,
  toggleNotchTimerProgress,
  stopwatchEnabled,
  toggleStopwatchEnabled,
  legacyMode,
  dualClockEnabled,
  toggleDualClock,
  dualClockZone,
  dualClockLabel,
  selectDualClockZone,
  musicModeEnabled,
  toggleMusicMode,
  musicCompactNotch,
  toggleMusicCompactNotch,
  mediaLayout,
  toggleMediaLayout,
  mediaAmbienceEnabled,
  toggleAmbience,
  mediaCompactGlowEnabled,
  toggleCompactGlow,
  notchScrollVolume,
  toggleNotchScrollVolume,
  shelfPersist,
  toggleShelfPersist,
  rememberMode,
  toggleRememberMode,
  weatherEnabled,
  toggleWeather,
  tempUnitFahrenheit,
  toggleTempUnit,
  cityName,
  setCityName,
  citySearchResults,
  showCityDropdown,
  setShowCityDropdown,
  selectCity,
  handleCityClear,
  statusWidgets,
  handleWidgetsChange,
  hasBattery,
  onOpenCalendarSync,
}: NotchTabProps) {
  const t = useT();
  const caldav = useCaldav();
  const sources = (caldav.account ? 1 : 0) + caldav.links.length;
  const perMonitor = usePerMonitor(notchMode, notchModeByMonitor, replaceNotchModes, monitors);
  const multiMonitor = notchMonitorMode === "all" && monitors.length > 1;
  const showPerMonitor = multiMonitor && perMonitor.enabled;
  return (
    <>
      <div className="setting-group-label">{t("notch.group")}</div>
      <div className="setting-group">
        {!showPerMonitor && (
          <SettingRow icon={PanelTop} label={t("notch.behavior")} desc={t("notch.behavior.desc")}>
            <Dropdown
              value={notchMode}
              onChange={setNotchModeValue}
              options={[
                { value: "fixed", label: t("common.fixed") },
                { value: "smart", label: t("common.smart") },
                { value: "peek", label: t("common.peek") },
              ]}
            />
          </SettingRow>
        )}

        <SettingRow icon={MonitorCheck} label={t("common.showOn")} desc={t("notch.showOn.desc")}>
          <Dropdown
            value={notchMonitorMode}
            onChange={(v) => setNotchMonitorModeValue(v as MonitorMode)}
            options={[
              { value: "primary", label: t("common.monitors.primary") },
              { value: "all", label: t("common.monitors.all") },
              { value: "specific", label: t("common.monitors.specific") },
            ]}
          />
        </SettingRow>

        {notchMonitorMode === "specific" && (
          <SettingRow icon={Monitor} label={t("common.monitor")} desc={t("notch.monitor.desc")}>
            <Dropdown
              value={notchMonitorId}
              onChange={setNotchMonitorIdValue}
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
              value={notchModeByMonitor[m.id] || notchMode}
              onChange={(v) => setNotchModeForMonitor(m.id, v)}
              options={[
                { value: "fixed", label: t("common.fixed") },
                { value: "smart", label: t("common.smart") },
                { value: "peek", label: t("common.peek") },
              ]}
            />
          </SettingRow>
        ))}

        <SettingRow icon={Calendar} label={t("notch.calendar")} desc={t("notch.calendar.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={calendarEnabled} onChange={toggleCalendar} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {calendarEnabled && (
          <SettingRow
            icon={CloudCog}
            label={t("notch.calendarSync")}
            desc={sources ? t(sources === 1 ? "notch.calendarSync.connectedOne" : "notch.calendarSync.connectedMany", { count: sources }) : t("notch.calendarSync.desc")}
            action
            onClick={onOpenCalendarSync}
          >
            <ChevronRight size={14} strokeWidth={1.5} />
          </SettingRow>
        )}

        <SettingRow icon={Hourglass} label={t("notch.timer")} desc={t("notch.timer.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={timerEnabled} onChange={toggleTimerEnabled} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {timerEnabled && (
          <SettingRow icon={BellRing} label={t("notch.timerSound")} desc={t("notch.timerSound.desc")}>
            <label className="toggle-switch">
              <input type="checkbox" checked={timerSoundEnabled} onChange={toggleTimerSound} />
              <span className="slider"></span>
            </label>
          </SettingRow>
        )}

        {timerEnabled && (
          <SettingRow icon={Timer} label={t("notch.timerProgress")} desc={t("notch.timerProgress.desc")}>
            <label className="toggle-switch">
              <input type="checkbox" checked={notchTimerProgress} onChange={toggleNotchTimerProgress} />
              <span className="slider"></span>
            </label>
          </SettingRow>
        )}

        <SettingRow icon={Timer} label={t("notch.stopwatch")} desc={t("notch.stopwatch.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={stopwatchEnabled} onChange={toggleStopwatchEnabled} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {!legacyMode && (
          <>
            <SettingRow
              icon={Globe}
              label={t("notch.dualClock")}
              desc={dualClockEnabled ? [dualClockLabel, zoneOffsetLabel(dualClockZone)].filter(Boolean).join(" · ") : t("notch.dualClock.desc")}
            >
              <label className="toggle-switch">
                <input type="checkbox" checked={dualClockEnabled} onChange={toggleDualClock} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            {dualClockEnabled && (
              <>
                <DualClockPicker onSelect={selectDualClockZone} />
                <div className="setting-divider" />
              </>
            )}
          </>
        )}

        <SettingRow icon={Music} label={t("notch.musicMode")} desc={t("notch.musicMode.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={musicModeEnabled} onChange={toggleMusicMode} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {musicModeEnabled && (
          <>
            <SettingRow icon={Minimize2} label={t("notch.compactMode")} desc={t("notch.compactMode.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={musicCompactNotch} onChange={toggleMusicCompactNotch} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={LayoutList} label={t("notch.mediaLayout")} desc={t("notch.mediaLayout.desc")}>
              <div className="unit-toggle-minimal wide">
                <span className={mediaLayout === "classic" ? "active" : ""} onClick={() => toggleMediaLayout("classic")}>
                  {t("notch.mediaLayout.classic")}
                </span>
                <span className={mediaLayout === "compact" ? "active" : ""} onClick={() => toggleMediaLayout("compact")}>
                  {t("notch.mediaLayout.compact")}
                </span>
              </div>
            </SettingRow>

            <SettingRow icon={Sparkles} label={t("notch.ambientGlow")} desc={t("notch.ambientGlow.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={mediaAmbienceEnabled} onChange={toggleAmbience} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Circle} label={t("notch.compactGlow")} desc={t("notch.compactGlow.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={mediaCompactGlowEnabled} onChange={toggleCompactGlow} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Volume2} label={t("notch.scrollVolume")} desc={t("notch.scrollVolume.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={notchScrollVolume} onChange={toggleNotchScrollVolume} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Circle} label={t("notch.shelfPersist")} desc={t("notch.shelfPersist.desc")}>
              <label className="toggle-switch">
                <input type="checkbox" checked={shelfPersist} onChange={toggleShelfPersist} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Circle} label={t("notch.rememberMode")} desc={t("notch.rememberMode.desc")} divider={false}>
              <label className="toggle-switch">
                <input type="checkbox" checked={rememberMode} onChange={toggleRememberMode} />
                <span className="slider"></span>
              </label>
            </SettingRow>
          </>
        )}
      </div>

      <div className="setting-group-label">{t("notch.weather.group")}</div>
      <div className="setting-group">
        <SettingRow icon={CloudSun} label={t("notch.weather")} desc={cityName || t("notch.weather.auto")}>
          <div className="weather-controls">
            <div className="unit-toggle-minimal" onClick={toggleTempUnit}>
              <span className={!tempUnitFahrenheit ? "active" : ""}>C</span>
              <span className={tempUnitFahrenheit ? "active" : ""}>F</span>
            </div>
            <label className="toggle-switch">
              <input type="checkbox" checked={weatherEnabled} onChange={toggleWeather} />
              <span className="slider"></span>
            </label>
          </div>
        </SettingRow>

        {weatherEnabled && (
          <div className="manual-city-input">
            <div className="city-input-row">
              <input
                type="text"
                placeholder={t("notch.weather.search")}
                value={cityName}
                onChange={(e) => setCityName(e.target.value)}
                onFocus={() => citySearchResults.length > 0 && setShowCityDropdown(true)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && citySearchResults.length > 0) {
                    e.preventDefault();
                    selectCity(citySearchResults[0]);
                  }
                  if (e.key === "Escape") {
                    setShowCityDropdown(false);
                  }
                }}
                onBlur={() => setTimeout(() => setShowCityDropdown(false), 150)}
              />
              {cityName && (
                <button
                  className="city-clear-btn"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    handleCityClear();
                  }}
                  title={t("notch.weather.clear")}
                >
                  <X size={10} strokeWidth={2.5} />
                </button>
              )}
            </div>
            {showCityDropdown && citySearchResults.length > 0 && (
              <div className="city-dropdown">
                {citySearchResults.map((city) => (
                  <button
                    key={`${city.name}-${city.latitude}`}
                    className="city-dropdown-item"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      selectCity(city);
                    }}
                  >
                    <span className="city-dropdown-name">{city.name}</span>
                    <span className="city-dropdown-country">{city.country}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      <div className="setting-group-label">{t("notch.widgets.group")}</div>
      <div className="setting-group">
        <StatusWidgetConfig value={statusWidgets} onChange={handleWidgetsChange} hasBattery={hasBattery} />
      </div>
    </>
  );
}
