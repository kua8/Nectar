import { Palette, Droplet, Contrast, Droplets, Sun, Square, Maximize2, Layers, SwatchBook, Highlighter } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { THEME_PRESETS, hexToHsl, hslToHex, type ThemePreset } from "../theme";

const STYLE_OPTIONS = [
  { value: "solid", label: "Solid" },
  { value: "frosted", label: "Frosted Glass" },
  { value: "liquid", label: "Liquid Glass" },
  { value: "material", label: "Material You" },
];

function presetBackground(preset: ThemePreset): string {
  return hslToHex(hexToHsl(preset.color).h, preset.saturation * 100, preset.brightness * 100);
}

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
  themeStyle: string;
  handleThemeStyleChange: (style: string) => void;
  themeAccent: string;
  handleAccentChange: (accent: string) => void;
  themeAccentColor: string;
  handleAccentColorChange: (color: string) => void;
  applyThemePreset: (preset: ThemePreset) => void;
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
  themeStyle,
  handleThemeStyleChange,
  themeAccent,
  handleAccentChange,
  themeAccentColor,
  handleAccentColorChange,
  applyThemePreset,
}: AppearanceTabProps) {
  const isMaterial = themeStyle === "material";
  const showCustomColor = themeMode === "custom";
  // Material You builds the whole palette from the accent, so these don't apply there.
  const showAdvancedSliders = !isMaterial && (themeMode === "custom" || themeMode === "adaptive");
  const isActivePreset = (preset: ThemePreset) =>
    themeMode === "custom" &&
    themeColor.toLowerCase() === preset.color.toLowerCase() &&
    Math.abs(themeSaturation - preset.saturation) < 0.01 &&
    Math.abs(themeBrightness - preset.brightness) < 0.01;

  return (
    <>
      <div className="setting-group-label">Theme</div>
      <div className="setting-group">
        <SettingRow icon={Layers} label="Style" desc="How the notch, dock and overlays are drawn" divider={false} />
        <div className="style-tiles" role="radiogroup" aria-label="Style">
          {STYLE_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              role="radio"
              aria-checked={themeStyle === opt.value}
              className={`style-tile${themeStyle === opt.value ? " active" : ""}`}
              onClick={() => handleThemeStyleChange(opt.value)}
            >
              <span className={`style-tile-preview style-preview-${opt.value}`}>
                <span className="style-tile-pill" />
              </span>
              <span className="style-tile-label">{opt.label}</span>
            </button>
          ))}
        </div>
        <div className="setting-divider" />

        <SettingRow icon={Palette} label="Theme Mode" desc="Configure visual styling">
          <Dropdown
            value={themeMode}
            onChange={handleThemeModeChange}
            options={[
              { value: "auto", label: "Auto (Match Windows)" },
              { value: "dark", label: "Dark (Translucent)" },
              { value: "light", label: "Light (Translucent)" },
              { value: "custom", label: "Custom Color" },
              { value: "adaptive", label: "Adaptive Accent" },
            ]}
          />
        </SettingRow>

        <SettingRow icon={SwatchBook} label="Presets" desc="One-click color themes">
          <div className="preset-swatches">
            {THEME_PRESETS.map((preset) => (
              <button
                key={preset.name}
                type="button"
                className={`preset-swatch${isActivePreset(preset) ? " active" : ""}`}
                style={{ background: presetBackground(preset) }}
                onClick={() => applyThemePreset(preset)}
                title={preset.name}
                aria-label={`${preset.name} preset`}
              >
                <span style={{ background: preset.color }} />
              </button>
            ))}
          </div>
        </SettingRow>

        <SettingRow
          icon={Highlighter}
          label="Accent Color"
          desc={isMaterial ? "Seed for the Material You palette" : "Toggles, active buttons, sliders and today"}
        >
          {/* Material You always needs a seed, so Monochrome reads as the Windows accent there */}
          <Dropdown
            value={isMaterial && themeAccent === "mono" ? "system" : themeAccent}
            onChange={handleAccentChange}
            options={[
              ...(isMaterial ? [] : [{ value: "mono", label: "Monochrome" }]),
              { value: "system", label: "Windows Accent" },
              { value: "custom", label: "Custom" },
            ]}
          />
        </SettingRow>

        {themeAccent === "custom" && (
          <SettingRow icon={Droplet} label="Custom Accent" desc="Pick the highlight color">
            <div className="color-picker-row">
              <input
                type="color"
                value={themeAccentColor}
                onChange={(e) => handleAccentColorChange(e.target.value)}
                className="color-picker-input"
              />
              <span className="color-picker-label">{themeAccentColor.toUpperCase()}</span>
            </div>
          </SettingRow>
        )}

        {showCustomColor && (
          <SettingRow icon={Droplet} label="Custom Theme Color" desc="Choose layout background color">
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
          label="Background Opacity"
          desc={`Adjust theme transparency (${Math.round(themeOpacity * 100)}%)`}
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
              label="Color Saturation"
              desc={`Adjust theme color vibrancy (${Math.round(themeSaturation * 100)}%)`}
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
              label="Background Brightness"
              desc={`Adjust background lightness (${Math.round(themeBrightness * 100)}%)`}
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
      </div>

      <div className="setting-group-label">Display</div>
      <div className="setting-group">
        <SettingRow icon={Square} label="Screen Corners" desc="Rounded top edges">
          <label className="toggle-switch">
            <input type="checkbox" checked={cornersEnabled} onChange={toggleCorners} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow
          icon={Maximize2}
          label="UI & Font Scale"
          desc={`Adjust desktop size (${Math.round(scale * 100)}%)`}
          divider={false}
        >
          <div className="scale-button-container">
            <button
              onClick={() => handleScaleChange(Math.max(0.8, parseFloat((scale - 0.1).toFixed(1))))}
              disabled={scale <= 0.8}
              className="scale-adjust-btn"
              title="Decrease Scale"
            >
              —
            </button>
            <span className="scale-display-value">{Math.round(scale * 100)}%</span>
            <button
              onClick={() => handleScaleChange(Math.min(1.3, parseFloat((scale + 0.1).toFixed(1))))}
              disabled={scale >= 1.3}
              className="scale-adjust-btn"
              title="Increase Scale"
            >
              +
            </button>
          </div>
        </SettingRow>
      </div>
    </>
  );
}
