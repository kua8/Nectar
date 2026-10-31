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
