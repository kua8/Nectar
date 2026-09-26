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
} from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useCaldav } from "../hooks/useCaldav";
import { StatusWidgetConfig } from "../components/StatusWidgetConfig";
import type { WidgetConfig, MonitorInfo, MonitorMode } from "./types";

interface NotchTabProps {
  notchMode: string;
  setNotchModeValue: (mode: string) => void;
  monitors: MonitorInfo[];
  notchMonitorMode: MonitorMode;
  setNotchMonitorModeValue: (mode: MonitorMode) => void;
  notchMonitorId: string;
  setNotchMonitorIdValue: (id: string) => void;
  calendarEnabled: boolean;
  toggleCalendar: () => void;
  timerEnabled: boolean;
  toggleTimerEnabled: () => void;
  stopwatchEnabled: boolean;
  toggleStopwatchEnabled: () => void;
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
  calendarEnabled,
  toggleCalendar,
  timerEnabled,
  toggleTimerEnabled,
  stopwatchEnabled,
  toggleStopwatchEnabled,
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
  const caldav = useCaldav();
  const sources = (caldav.account ? 1 : 0) + caldav.links.length;
  return (
    <>
      <div className="setting-group-label">Notch</div>
      <div className="setting-group">
        <SettingRow icon={PanelTop} label="Notch Behavior" desc="Choose how the notch appears">
          <select
            className="settings-select"
            value={notchMode}
            onChange={(e) => setNotchModeValue(e.target.value)}
          >
            <option value="fixed">Fixed</option>
            <option value="smart">Smart</option>
            <option value="peek">Peek</option>
          </select>
        </SettingRow>

        <SettingRow icon={MonitorCheck} label="Show On" desc="Choose which monitor(s) display the notch">
          <select
            className="settings-select"
            value={notchMonitorMode}
            onChange={(e) => setNotchMonitorModeValue(e.target.value as MonitorMode)}
          >
            <option value="primary">Primary Monitor</option>
            <option value="all">All Monitors</option>
            <option value="specific">Specific Monitor</option>
          </select>
        </SettingRow>

        {notchMonitorMode === "specific" && (
          <SettingRow icon={Monitor} label="Monitor" desc="Which display shows the notch">
            <select
              className="settings-select"
              value={notchMonitorId}
              onChange={(e) => setNotchMonitorIdValue(e.target.value)}
            >
              {monitors.length === 0 && <option value="">Loading...</option>}
              {monitors.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}{m.is_primary ? " (Primary)" : ""}
                </option>
              ))}
            </select>
          </SettingRow>
        )}

        <SettingRow icon={Calendar} label="Calendar" desc="Day, week, month and agenda views in the notch">
          <label className="toggle-switch">
            <input type="checkbox" checked={calendarEnabled} onChange={toggleCalendar} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {calendarEnabled && (
          <SettingRow
            icon={CloudCog}
            label="Calendar Sync"
            desc={sources ? `${sources} source${sources === 1 ? "" : "s"} connected` : "Show your real events from Google, iCloud, Nextcloud and more"}
            action
            onClick={onOpenCalendarSync}
          >
            <ChevronRight size={14} strokeWidth={1.5} />
          </SettingRow>
        )}

        <SettingRow icon={Hourglass} label="Timer" desc="Countdown with presets and a custom time">
          <label className="toggle-switch">
            <input type="checkbox" checked={timerEnabled} onChange={toggleTimerEnabled} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={Timer} label="Stopwatch" desc="Count up with laps">
          <label className="toggle-switch">
            <input type="checkbox" checked={stopwatchEnabled} onChange={toggleStopwatchEnabled} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={Music} label="Music Mode" desc="Interactive live music widget">
          <label className="toggle-switch">
            <input type="checkbox" checked={musicModeEnabled} onChange={toggleMusicMode} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {musicModeEnabled && (
          <>
            <SettingRow icon={Minimize2} label="Compact Mode" desc="Show visualizer & artwork when collapsed">
              <label className="toggle-switch">
                <input type="checkbox" checked={musicCompactNotch} onChange={toggleMusicCompactNotch} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={LayoutList} label="Media Layout" desc="Choose expanded player style">
              <div className="unit-toggle-minimal wide">
                <span className={mediaLayout === "classic" ? "active" : ""} onClick={() => toggleMediaLayout("classic")}>
                  Classic
                </span>
                <span className={mediaLayout === "compact" ? "active" : ""} onClick={() => toggleMediaLayout("compact")}>
                  Compact
                </span>
              </div>
            </SettingRow>

            <SettingRow icon={Sparkles} label="Ambient Glow" desc="Colored glow behind expanded album art">
              <label className="toggle-switch">
                <input type="checkbox" checked={mediaAmbienceEnabled} onChange={toggleAmbience} />
                <span className="slider"></span>
              </label>
            </SettingRow>

            <SettingRow icon={Circle} label="Compact Glow" desc="Glow around collapsed thumbnail" divider={false}>
              <label className="toggle-switch">
                <input type="checkbox" checked={mediaCompactGlowEnabled} onChange={toggleCompactGlow} />
                <span className="slider"></span>
              </label>
            </SettingRow>
          </>
        )}
      </div>

      <div className="setting-group-label">Weather</div>
      <div className="setting-group">
        <SettingRow icon={CloudSun} label="Weather Status" desc={cityName || "Auto-detect location"}>
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
                placeholder="Search city..."
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
                  title="Clear city"
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

      <div className="setting-group-label">Widgets</div>
      <div className="setting-group">
        <StatusWidgetConfig value={statusWidgets} onChange={handleWidgetsChange} hasBattery={hasBattery} />
      </div>
    </>
  );
}
