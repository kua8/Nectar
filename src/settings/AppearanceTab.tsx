import { Palette, Droplet, Contrast, Droplets, Sun, Square, Maximize2 } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { SettingSection } from "./SettingSection";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

interface AppearanceTabProps {
  themeMode: string;
  handleThemeModeChange: (mode: string) => void;
  themeColor: string;
  handleThemeColorChange: (color: string) => void;
  themeOpacity: number;
  handleOpacityChange: (val: number) => void;
  themeSaturation: number;
  handleSaturationChange: (val: number) => void;
  themeBrightness: number;
  handleBrightnessChange: (val: number) => void;
  cornersEnabled: boolean;
  toggleCorners: () => void;
  scale: number;
  handleScaleChange: (val: number) => void;
}

export function AppearanceTab({
  themeMode,
  handleThemeModeChange,
  themeColor,
  handleThemeColorChange,
  themeOpacity,
  handleOpacityChange,
  themeSaturation,
  handleSaturationChange,
  themeBrightness,
  handleBrightnessChange,
  cornersEnabled,
  toggleCorners,
  scale,
  handleScaleChange,
}: AppearanceTabProps) {
  const t = useT();
  const showCustomColor = themeMode === "custom";
  const showAdvancedSliders = themeMode === "custom" || themeMode === "adaptive";

  return (
    <>
      <SettingSection id="appearance-theme" title={t("appearance.theme.group")} defaultOpen>
        <SettingRow icon={Palette} label={t("appearance.mode")} desc={t("appearance.mode.desc")}>
          <Dropdown
            value={themeMode}
            onChange={handleThemeModeChange}
            options={[
              { value: "dark", label: t("appearance.mode.dark") },
              { value: "light", label: t("appearance.mode.light") },
              { value: "custom", label: t("appearance.mode.custom") },
              { value: "adaptive", label: t("appearance.mode.adaptive") },
            ]}
          />
        </SettingRow>

        {showCustomColor && (
          <SettingRow icon={Droplet} label={t("appearance.color")} desc={t("appearance.color.desc")}>
            <div className="color-picker-row">
              <input
                type="color"
                value={themeColor}
                onChange={(e) => handleThemeColorChange(e.target.value)}
                className="color-picker-input"
              />
              <span className="color-picker-label">{themeColor.toUpperCase()}</span>
            </div>
          </SettingRow>
        )}

        <SettingRow
          icon={Contrast}
          label={t("appearance.opacity")}
          desc={t("appearance.opacity.desc", { percent: Math.round(themeOpacity * 100) })}
          divider={showAdvancedSliders}
        >
          <input
            type="range"
            min="0.1"
            max="1.0"
            step="0.05"
            value={themeOpacity}
            onChange={(e) => handleOpacityChange(parseFloat(e.target.value))}
            className="settings-slider"
          />
        </SettingRow>

        {showAdvancedSliders && (
          <>
            <SettingRow
              icon={Droplets}
              label={t("appearance.saturation")}
              desc={t("appearance.saturation.desc", { percent: Math.round(themeSaturation * 100) })}
            >
              <input
                type="range"
                min="0.0"
                max="1.0"
                step="0.02"
                value={themeSaturation}
                onChange={(e) => handleSaturationChange(parseFloat(e.target.value))}
                className="settings-slider"
              />
            </SettingRow>

            <SettingRow
              icon={Sun}
              label={t("appearance.brightness")}
              desc={t("appearance.brightness.desc", { percent: Math.round(themeBrightness * 100) })}
              divider={false}
            >
              <input
                type="range"
                min="0.0"
                max="1.0"
                step="0.02"
                value={themeBrightness}
                onChange={(e) => handleBrightnessChange(parseFloat(e.target.value))}
                className="settings-slider"
              />
            </SettingRow>
          </>
        )}
      </SettingSection>

      <SettingSection id="appearance-display" title={t("appearance.display.group")}>
        <SettingRow icon={Square} label={t("appearance.corners")} desc={t("appearance.corners.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={cornersEnabled} onChange={toggleCorners} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow
          icon={Maximize2}
          label={t("appearance.scale")}
          desc={t("appearance.scale.desc", { percent: Math.round(scale * 100) })}
          divider={false}
        >
          <div className="scale-button-container">
            <button
              onClick={() => handleScaleChange(Math.max(0.8, parseFloat((scale - 0.1).toFixed(1))))}
              disabled={scale <= 0.8}
              className="scale-adjust-btn"
              title={t("appearance.scale.decrease")}
            >
              —
            </button>
            <span className="scale-display-value">{Math.round(scale * 100)}%</span>
            <button
              onClick={() => handleScaleChange(Math.min(1.3, parseFloat((scale + 0.1).toFixed(1))))}
              disabled={scale >= 1.3}
              className="scale-adjust-btn"
              title={t("appearance.scale.increase")}
            >
              +
            </button>
          </div>
        </SettingRow>
      </SettingSection>
    </>
  );
}
