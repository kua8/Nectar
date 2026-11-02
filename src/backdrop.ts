import { invoke } from "@tauri-apps/api/core";

// The glass styles show a pre-blurred copy of the desktop wallpaper behind the notch and dock,
// lined up with where they sit on screen (the windows are transparent, so nothing real can be
// blurred). Where an app window is behind them, they still show the wallpaper.

interface WallpaperBackdrop {
  image: string;
  imageWidth: number;
  imageHeight: number;
  fit: string;
  monitorWidth: number;
  monitorHeight: number;
  monitorScale: number;
  windowX: number;
  windowY: number;
  windowWidth: number;
  windowHeight: number;
}

// Blur of the 640px copy per glass style; on screen it is about three times larger.
const STYLE_BLUR: Record<string, number> = { frosted: 9, liquid: 3 };

const VARS = ["--backdrop-image", "--backdrop-size", "--backdrop-x", "--backdrop-top", "--backdrop-bottom"];

let enabled = false;
let generation = 0;

// Only the notch and dock draw a backdrop; other windows skip loading the wallpaper.
export function enableBackdrop() {
  enabled = true;
}

export function usesBackdrop(style: string | null) {
  return !!style && Object.prototype.hasOwnProperty.call(STYLE_BLUR, style);
}

// Where Windows draws the wallpaper on the monitor, in logical pixels.
function wallpaperRect(d: WallpaperBackdrop) {
  const mw = d.monitorWidth;
  const mh = d.monitorHeight;
  let w = mw;
  let h = mh;
  if (d.fit === "center") {
    w = d.imageWidth / d.monitorScale;
    h = d.imageHeight / d.monitorScale;
  } else if (d.fit !== "stretch") {
    // fill (and tile/span, approximated) covers the screen; fit letterboxes
    const ratio = d.fit === "fit" ? Math.min(mw / d.imageWidth, mh / d.imageHeight) : Math.max(mw / d.imageWidth, mh / d.imageHeight);
    w = d.imageWidth * ratio;
    h = d.imageHeight * ratio;
  }
  return { x: (mw - w) / 2, y: (mh - h) / 2, w, h };
}

function clearBackdrop(root: HTMLElement) {
  root.classList.remove("has-backdrop");
  for (const name of VARS) root.style.removeProperty(name);
}

export async function applyBackdrop(root: HTMLElement, style: string) {
  const current = ++generation;
  if (!enabled || !usesBackdrop(style)) {
    clearBackdrop(root);
    return;
  }
  const d = await invoke<WallpaperBackdrop | null>("get_wallpaper_backdrop", { blur: STYLE_BLUR[style] }).catch(() => null);
  if (current !== generation) return;
  if (!d) {
    clearBackdrop(root);
    return;
  }

  const r = wallpaperRect(d);
  // The notch is zoomed and the dock scaled by the UI scale, so lengths are given before scaling.
  const scale = parseFloat(localStorage.getItem("nectar-scale") || "1") || 1;
  const px = (v: number) => `${(v / scale).toFixed(2)}px`;
  // Both are centred horizontally in their window, so the image is lined up from the centre.
  const dx = r.x + r.w / 2 - (d.windowX + d.windowWidth / 2);

  root.style.setProperty("--backdrop-image", `url("${d.image}")`);
  root.style.setProperty("--backdrop-size", `${px(r.w)} ${px(r.h)}`);
  root.style.setProperty("--backdrop-x", `calc(50% + ${px(dx)})`);
  // The notch hangs from the top of its window; the dock sits on the bottom of its window.
  root.style.setProperty("--backdrop-top", px(r.y - d.windowY));
  root.style.setProperty("--backdrop-bottom", `calc(100% + ${px(r.y + r.h - d.windowY - d.windowHeight)})`);
  root.classList.add("has-backdrop");
}
