import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyBackdrop, enableBackdrop, usesBackdrop } from "./backdrop";

// Convert a hex color string to rgba with a specified alpha
function hexToRgba(hex: string, alpha: number): string {
  hex = hex.replace('#', '');
  if (hex.length === 3) {
    hex = hex.split('').map(char => char + char).join('');
  }
  const r = parseInt(hex.substring(0, 2), 16) || 0;
  const g = parseInt(hex.substring(2, 4), 16) || 0;
  const b = parseInt(hex.substring(4, 6), 16) || 0;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export interface HSL {
  h: number;
  s: number;
  l: number;
}

// Convert Hex to HSL
export function hexToHsl(hex: string): HSL {
  hex = hex.replace('#', '');
  if (hex.length === 3) {
    hex = hex.split('').map(char => char + char).join('');
  }
  const r = (parseInt(hex.substring(0, 2), 16) || 0) / 255;
  const g = (parseInt(hex.substring(2, 4), 16) || 0) / 255;
  const b = (parseInt(hex.substring(4, 6), 16) || 0) / 255;

  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  let h = 0;
  let s = 0;
  const l = (max + min) / 2;

  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r:
        h = (g - b) / d + (g < b ? 6 : 0);
        break;
      case g:
        h = (b - r) / d + 2;
        break;
      case b:
        h = (r - g) / d + 4;
        break;
    }
    h /= 6;
  }

  return { h: Math.round(h * 360), s: Math.round(s * 100), l: Math.round(l * 100) };
}

// Convert HSL back to Hex color string
export function hslToHex(h: number, s: number, l: number): string {
  s /= 100;
  l /= 100;
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs((h / 60) % 2 - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;

  if (0 <= h && h < 60) {
    r = c; g = x; b = 0;
  } else if (60 <= h && h < 120) {
    r = x; g = c; b = 0;
  } else if (120 <= h && h < 180) {
    r = 0; g = c; b = x;
  } else if (180 <= h && h < 240) {
    r = 0; g = x; b = c;
  } else if (240 <= h && h < 300) {
    r = x; g = 0; b = c;
  } else if (300 <= h && h < 360) {
    r = c; g = 0; b = x;
  }

  const toHex = (val: number) => {
    const hexVal = Math.round((val + m) * 255).toString(16);
    return hexVal.length === 1 ? '0' + hexVal : hexVal;
  };

  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

export interface ThemePreset {
  name: string;
  color: string;
  saturation: number;
  brightness: number;
}

// Background hue comes from `color`; the same color is used as the accent.
export const THEME_PRESETS: ThemePreset[] = [
  { name: "Midnight", color: "#5e8bff", saturation: 0.45, brightness: 0.13 },
  { name: "Ocean", color: "#2ec4d6", saturation: 0.6, brightness: 0.13 },
  { name: "Forest", color: "#34c77b", saturation: 0.35, brightness: 0.12 },
  { name: "Plum", color: "#c77dff", saturation: 0.3, brightness: 0.14 },
  { name: "Ember", color: "#ff7a45", saturation: 0.45, brightness: 0.13 },
  { name: "Latte", color: "#c0742d", saturation: 0.4, brightness: 0.88 },
  { name: "Mist", color: "#0a84ff", saturation: 0.35, brightness: 0.92 },
];

export const DEFAULT_ACCENT_COLOR = "#0a84ff";

export const THEME_STYLES = ["solid", "frosted", "liquid", "material"] as const;

// Background opacity each style is designed around; picking a style sets it, the slider can still change it.
export const STYLE_OPACITY: Record<string, number> = {
  solid: 0.8,
  frosted: 0.6,
  liquid: 0.45,
  material: 0.96,
};

const darkScheme = typeof window !== "undefined" && window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;

// "auto" follows the Windows app mode (WebView2 reports it as prefers-color-scheme).
function resolveMode(mode: string): string {
  if (mode !== "auto") return mode;
  return darkScheme && !darkScheme.matches ? "light" : "dark";
}

// Black or white, whichever reads better on the given color.
function contrastText(hex: string): string {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const channel = (i: number) => {
    const v = (parseInt(full.substring(i, i + 2), 16) || 0) / 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  const luminance = 0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4);
  return luminance > 0.4 ? "#000000" : "#ffffff";
}

function isHex(color: string | null): color is string {
  return !!color && /^#[0-9a-f]{3}([0-9a-f]{3})?$/i.test(color);
}

const hsla = (h: number, s: number, l: number, a = 1) => `hsla(${h}, ${s}%, ${l}%, ${a})`;

// Material You style tonal palette: tinted surfaces and a pastel (dark) or deep (light) primary.
function applyMaterialPalette(root: HTMLElement, seed: string, light: boolean, op: number, opExpanded: number, opPanel: number) {
  const { h, s } = hexToHsl(seed);
  const tint = Math.min(s, 60);
  const set = (name: string, value: string) => root.style.setProperty(name, value);
  if (light) {
    set("--nectar-bg", hsla(h, tint * 0.6, 95, op));
    set("--nectar-bg-expanded", hsla(h, tint * 0.55, 92, opExpanded));
    set("--nectar-bg-panel", hsla(h, tint * 0.55, 92, opPanel));
    set("--nectar-text", hsla(h, 18, 12));
    set("--nectar-text-muted", hsla(h, 12, 28, 0.75));
    set("--nectar-border", hsla(h, 15, 30, 0.16));
    set("--nectar-group-bg", hsla(h, 35, 45, 0.08));
    set("--nectar-highlight", hsla(h, Math.max(s, 45), 38));
    set("--nectar-on-highlight", "#ffffff");
  } else {
    set("--nectar-bg", hsla(h, tint * 0.35, 9, op));
    set("--nectar-bg-expanded", hsla(h, tint * 0.35, 12, opExpanded));
    set("--nectar-bg-panel", hsla(h, tint * 0.35, 12, opPanel));
    set("--nectar-text", hsla(h, 25, 92));
    set("--nectar-text-muted", hsla(h, 14, 78, 0.8));
    set("--nectar-border", hsla(h, 18, 70, 0.16));
    set("--nectar-group-bg", hsla(h, 35, 70, 0.08));
    set("--nectar-highlight", hsla(h, Math.max(s, 55), 78));
    set("--nectar-on-highlight", hsla(h, 55, 16));
  }
  set("--nectar-accent", seed);
}

let accentGeneration = 0;

// Highlights (active pills, toggles, sliders, today) are monochrome unless an accent is chosen.
// Material You always has one: it uses the accent, or the Windows accent, as its seed.
async function applyAccent(
  root: HTMLElement,
  surface: string,
  material: { op: number; opExpanded: number; opPanel: number; fallback: string } | null,
  legacy: boolean
) {
  const generation = ++accentGeneration;
  const accentMode = legacy ? "mono" : localStorage.getItem("nectar-theme-accent") || "mono";
  let color: string | null = null;
  if (accentMode === "custom") {
    color = localStorage.getItem("nectar-theme-accent-color") || DEFAULT_ACCENT_COLOR;
  } else if (accentMode === "system" || material) {
    color = await invoke<string>("get_system_accent_color").catch(() => null);
    if (generation !== accentGeneration) return;
  }
  if (material) {
    const light = root.classList.contains("light-mode");
    applyMaterialPalette(root, isHex(color) ? color : material.fallback, light, material.op, material.opExpanded, material.opPanel);
    return;
  }
  if (!isHex(color)) {
    // Monochrome: highlights use the text color, and text on them uses the opaque surface color
    // (the translucent background would turn it grey on glass).
    root.style.removeProperty("--nectar-highlight");
    root.style.setProperty("--nectar-on-highlight", surface);
    return;
  }
  root.style.setProperty("--nectar-accent", color);
  root.style.setProperty("--nectar-highlight", color);
  root.style.setProperty("--nectar-on-highlight", contrastText(color));
}

export async function applyTheme(
  mode: string,
  customColor: string,
  opacity?: number,
  saturation?: number,
  brightness?: number
) {
  const root = document.documentElement;
  mode = resolveMode(mode);

  // Legacy Mode keeps the classic look: no glass or Material You, and monochrome highlights.
  const legacy = localStorage.getItem("nectar-legacy-mode") === "true";
  const style = legacy ? "solid" : localStorage.getItem("nectar-theme-style") || "solid";
  root.classList.remove(...THEME_STYLES.map((s) => `style-${s}`));
  root.classList.add(`style-${(THEME_STYLES as readonly string[]).includes(style) ? style : "solid"}`);

  if (opacity === undefined || opacity === null) {
    const cachedOpacity = localStorage.getItem('nectar-theme-opacity');
    opacity = cachedOpacity !== null ? parseFloat(cachedOpacity) : 0.80;
  }

  if (saturation === undefined || saturation === null) {
    const cachedSaturation = localStorage.getItem('nectar-theme-saturation');
    saturation = cachedSaturation !== null ? parseFloat(cachedSaturation) : 0.50;
  }

  if (brightness === undefined || brightness === null) {
    const cachedBrightness = localStorage.getItem('nectar-theme-brightness');
    brightness = cachedBrightness !== null ? parseFloat(cachedBrightness) : 0.15;
  }

  const op = opacity;
  const opExpanded = Math.min(1.0, opacity + 0.12);
  // Large panels (calendar, command center) need a denser fill than the pill to stay readable on glass.
  const opPanel = Math.max(opExpanded, 0.82);
  let surface = '#000000';

  if (mode === 'light') {
    surface = '#f5f5f7';
    root.style.setProperty('--nectar-bg', `rgba(255, 255, 255, ${op})`);
    root.style.setProperty('--nectar-bg-expanded', `rgba(245, 245, 247, ${opExpanded})`);
    root.style.setProperty('--nectar-bg-panel', `rgba(245, 245, 247, ${opPanel})`);
    root.style.setProperty('--nectar-text', '#1c1c1e');
    root.style.setProperty('--nectar-text-muted', 'rgba(28, 28, 30, 0.65)');
    root.style.setProperty('--nectar-border', 'rgba(0, 0, 0, 0.12)');
    root.style.setProperty('--nectar-group-bg', 'rgba(0, 0, 0, 0.04)');
    root.style.setProperty('--nectar-accent', '#007aff');
    root.style.setProperty('--nectar-scrollbar-thumb', 'rgba(0, 0, 0, 0.15)');
    root.classList.add('light-mode');
    root.classList.remove('dark-mode');
    root.classList.add('theme-light');
    root.classList.remove('theme-dark', 'theme-custom', 'theme-adaptive');
  } else if (mode === 'custom') {
    const hsl = hexToHsl(customColor);
    hsl.s = saturation * 100;
    hsl.l = brightness * 100;
    const finalBgColor = hslToHex(hsl.h, hsl.s, hsl.l);
    const isLight = brightness > 0.55;
    surface = finalBgColor;

    root.style.setProperty('--nectar-bg', hexToRgba(finalBgColor, op));
    root.style.setProperty('--nectar-bg-expanded', hexToRgba(finalBgColor, opExpanded));
    root.style.setProperty('--nectar-bg-panel', hexToRgba(finalBgColor, opPanel));
    root.style.setProperty('--nectar-text', isLight ? '#1c1c1e' : '#ffffff');
    root.style.setProperty('--nectar-text-muted', isLight ? 'rgba(28, 28, 30, 0.65)' : 'rgba(255, 255, 255, 0.65)');
    root.style.setProperty('--nectar-border', isLight ? 'rgba(0, 0, 0, 0.12)' : 'rgba(255, 255, 255, 0.12)');
    root.style.setProperty('--nectar-group-bg', isLight ? 'rgba(0, 0, 0, 0.04)' : 'rgba(255, 255, 255, 0.04)');
    root.style.setProperty('--nectar-accent', customColor);
    root.style.setProperty('--nectar-scrollbar-thumb', isLight ? 'rgba(0, 0, 0, 0.15)' : 'rgba(255, 255, 255, 0.15)');
    root.classList.add('theme-custom');
    root.classList.remove('theme-light', 'theme-dark', 'theme-adaptive');
    if (isLight) {
      root.classList.add('light-mode');
      root.classList.remove('dark-mode');
    } else {
      root.classList.add('dark-mode');
      root.classList.remove('light-mode');
    }
  } else if (mode === 'adaptive') {
    try {
      const accentHex = await invoke<string>('get_system_accent_color');
      const hsl = hexToHsl(accentHex);
      hsl.s = saturation * 100;
      hsl.l = brightness * 100;
      const finalBgColor = hslToHex(hsl.h, hsl.s, hsl.l);
      const isLight = brightness > 0.55;
      surface = finalBgColor;

      root.style.setProperty('--nectar-bg', hexToRgba(finalBgColor, op));
      root.style.setProperty('--nectar-bg-expanded', hexToRgba(finalBgColor, opExpanded));
      root.style.setProperty('--nectar-bg-panel', hexToRgba(finalBgColor, opPanel));
      root.style.setProperty('--nectar-text', isLight ? '#1c1c1e' : '#ffffff');
      root.style.setProperty('--nectar-text-muted', isLight ? 'rgba(28, 28, 30, 0.65)' : 'rgba(255, 255, 255, 0.65)');
      root.style.setProperty('--nectar-border', isLight ? 'rgba(0, 0, 0, 0.12)' : 'rgba(255, 255, 255, 0.12)');
      root.style.setProperty('--nectar-group-bg', isLight ? 'rgba(0, 0, 0, 0.04)' : 'rgba(255, 255, 255, 0.04)');
      root.style.setProperty('--nectar-accent', accentHex);
      root.style.setProperty('--nectar-scrollbar-thumb', isLight ? 'rgba(0, 0, 0, 0.15)' : 'rgba(255, 255, 255, 0.15)');
      root.classList.add('theme-adaptive');
      root.classList.remove('theme-light', 'theme-dark', 'theme-custom');
      if (isLight) {
        root.classList.add('light-mode');
        root.classList.remove('dark-mode');
      } else {
        root.classList.add('dark-mode');
        root.classList.remove('light-mode');
      }
    } catch (e) {
      console.error("Failed to apply adaptive theme:", e);
      return applyTheme('dark', customColor, op, saturation, brightness);
    }
  } else {
    // default/dark
    root.style.setProperty('--nectar-bg', `rgba(0, 0, 0, ${op})`);
    root.style.setProperty('--nectar-bg-expanded', `rgba(0, 0, 0, ${opExpanded})`);
    root.style.setProperty('--nectar-bg-panel', `rgba(0, 0, 0, ${opPanel})`);
    root.style.setProperty('--nectar-text', '#ffffff');
    root.style.setProperty('--nectar-text-muted', 'rgba(255, 255, 255, 0.6)');
    root.style.setProperty('--nectar-border', 'rgba(255, 255, 255, 0.1)');
    root.style.setProperty('--nectar-group-bg', 'rgba(255, 255, 255, 0.04)');
    root.style.setProperty('--nectar-accent', '#007aff');
    root.style.setProperty('--nectar-scrollbar-thumb', 'rgba(255, 255, 255, 0.1)');
    root.classList.add('dark-mode');
    root.classList.remove('light-mode');
    root.classList.add('theme-dark');
    root.classList.remove('theme-light', 'theme-custom', 'theme-adaptive');
  }

  await Promise.all([
    applyAccent(
      root,
      surface,
      style === "material" ? { op, opExpanded, opPanel, fallback: isHex(customColor) ? customColor : DEFAULT_ACCENT_COLOR } : null,
      legacy
    ),
    applyBackdrop(root, style),
  ]);
}

// `backdrop`: this window draws the notch or dock, which show the wallpaper in the glass styles.
export function initTheme(options: { backdrop?: boolean } = {}) {
  if (options.backdrop) enableBackdrop();
  const syncMode = localStorage.getItem('nectar-theme-mode') || 'dark';
  const syncColor = localStorage.getItem('nectar-theme-color') || '#007aff';
  const syncOpacityVal = localStorage.getItem('nectar-theme-opacity');
  const syncOpacity = syncOpacityVal !== null ? parseFloat(syncOpacityVal) : 0.80;
  const syncSaturationVal = localStorage.getItem('nectar-theme-saturation');
  const syncSaturation = syncSaturationVal !== null ? parseFloat(syncSaturationVal) : 0.50;
  const syncBrightnessVal = localStorage.getItem('nectar-theme-brightness');
  const syncBrightness = syncBrightnessVal !== null ? parseFloat(syncBrightnessVal) : 0.15;

  // Fast synchronous draw using cached localStorage
  applyTheme(syncMode, syncColor, syncOpacity, syncSaturation, syncBrightness);

  // Sync settings.json in background to avoid flickers
  invoke<Record<string, any>>('load_settings').then((settings) => {
    const mode = settings['nectar-theme-mode'] ? String(settings['nectar-theme-mode']) : syncMode;
    const color = settings['nectar-theme-color'] ? String(settings['nectar-theme-color']) : syncColor;
    const opacityVal = settings['nectar-theme-opacity'] ? parseFloat(String(settings['nectar-theme-opacity'])) : syncOpacity;
    const saturationVal = settings['nectar-theme-saturation'] ? parseFloat(String(settings['nectar-theme-saturation'])) : syncSaturation;
    const brightnessVal = settings['nectar-theme-brightness'] ? parseFloat(String(settings['nectar-theme-brightness'])) : syncBrightness;

    let styleOrAccentChanged = false;
    for (const key of ['nectar-theme-accent', 'nectar-theme-accent-color', 'nectar-theme-style', 'nectar-legacy-mode']) {
      const val = settings[key];
      if (val !== undefined && val !== null && String(val) !== localStorage.getItem(key)) {
        localStorage.setItem(key, String(val));
        styleOrAccentChanged = true;
      }
    }

    if (
      mode !== syncMode ||
      color !== syncColor ||
      opacityVal !== syncOpacity ||
      saturationVal !== syncSaturation ||
      brightnessVal !== syncBrightness ||
      styleOrAccentChanged
    ) {
      localStorage.setItem('nectar-theme-mode', mode);
      localStorage.setItem('nectar-theme-color', color);
      localStorage.setItem('nectar-theme-opacity', String(opacityVal));
      localStorage.setItem('nectar-theme-saturation', String(saturationVal));
      localStorage.setItem('nectar-theme-brightness', String(brightnessVal));
      applyTheme(mode, color, opacityVal, saturationVal, brightnessVal);
    }
  }).catch(console.error);

  // Helper: read all theme values from localStorage and apply
  const applyThemeFromStorage = () => {
    const mode = localStorage.getItem('nectar-theme-mode') || 'dark';
    const color = localStorage.getItem('nectar-theme-color') || '#007aff';
    const opacity = parseFloat(localStorage.getItem('nectar-theme-opacity') || '0.80');
    const saturation = parseFloat(localStorage.getItem('nectar-theme-saturation') || '0.50');
    const brightness = parseFloat(localStorage.getItem('nectar-theme-brightness') || '0.15');
    applyTheme(mode, color, opacity, saturation, brightness);
  };

  const nectarThemeKeys = [
    'nectar-theme-mode',
    'nectar-theme-color',
    'nectar-theme-opacity',
    'nectar-theme-saturation',
    'nectar-theme-brightness',
    'nectar-theme-accent',
    'nectar-theme-accent-color',
    'nectar-theme-style',
    'nectar-legacy-mode',
    'nectar-scale',
  ];

  // Listen to setting changes broadcasted from settings window
  // (localStorage sync is handled by saveAndLocal in Settings.tsx; we just apply the theme)
  const settingsPromise = listen<{ key: string; value: any }>('settings-changed', (event) => {
    const { key } = event.payload;
    if (nectarThemeKeys.includes(key)) {
      applyThemeFromStorage();
    }
  });

  // Windows sends this for accent and other system setting changes
  const accentPromise = listen<string>('system-accent-changed', () => {
    const currentMode = localStorage.getItem('nectar-theme-mode') || 'dark';
    if (
      currentMode === 'adaptive' ||
      currentMode === 'auto' ||
      localStorage.getItem('nectar-theme-accent') === 'system' ||
      localStorage.getItem('nectar-theme-style') === 'material' ||
      usesBackdrop(localStorage.getItem('nectar-theme-style'))
    ) {
      applyThemeFromStorage();
    }
  });

  // The wallpaper backdrop is lined up with the window's place on its monitor
  const monitorsPromise = listen('monitors-changed', () => {
    if (usesBackdrop(localStorage.getItem('nectar-theme-style'))) applyThemeFromStorage();
  });

  // Follow Windows light/dark in auto mode
  const onSchemeChange = () => {
    if ((localStorage.getItem('nectar-theme-mode') || 'dark') === 'auto') applyThemeFromStorage();
  };
  darkScheme?.addEventListener('change', onSchemeChange);

  // Listen to external edits (e.g. settings.json edited outside the app)
  // localStorage sync is handled by useSettingsSync hook; we just apply the theme
  const externalPromise = listen<{ key: string; value: any }>('settings-external-changed', (event) => {
    const { key } = event.payload;
    if (nectarThemeKeys.includes(key)) {
      applyThemeFromStorage();
    }
  });

  return () => {
    settingsPromise.then(f => f());
    accentPromise.then(f => f());
    externalPromise.then(f => f());
    monitorsPromise.then(f => f());
    darkScheme?.removeEventListener('change', onSchemeChange);
  };
}
