export const DEFAULT_PANEL_ORDER = ["command-center", "status", "music", "calendar", "shelf"] as const;

export type PanelId = (typeof DEFAULT_PANEL_ORDER)[number];

export function parsePanelOrder(raw: string | null | undefined): PanelId[] | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return null;
    const known = parsed.filter((id): id is PanelId => (DEFAULT_PANEL_ORDER as readonly string[]).includes(id));
    const unique = Array.from(new Set(known));
    return [...unique, ...DEFAULT_PANEL_ORDER.filter((id) => !unique.includes(id))];
  } catch {
    return null;
  }
}

export const CC_TILES = ["network", "dock-mode", "bluetooth", "notch-mode"] as const;
export const CC_BUTTONS = ["dnd", "battery-saver", "tray", "notifications", "settings", "restart"] as const;

export type CcLayout = { tiles?: string[]; buttons?: string[] };

function complete(saved: unknown, defaults: readonly string[]): string[] | undefined {
  if (!Array.isArray(saved)) return undefined;
  const known = Array.from(new Set(saved.filter((id): id is string => defaults.includes(id as string))));
  return [...known, ...defaults.filter((id) => !known.includes(id))];
}

export function parseCcLayout(raw: string | null | undefined): CcLayout {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    return { tiles: complete(parsed?.tiles, CC_TILES), buttons: complete(parsed?.buttons, CC_BUTTONS) };
  } catch {
    return {};
  }
}
