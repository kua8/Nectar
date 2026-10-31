import { t } from "./i18n";

export const DEFAULT_DUAL_CLOCK_ZONE = "UTC";

export interface ZoneOption {
  label: string;
  zone: string;
  detail: string;
}

const MINUTE = 60_000;
const DAY = 24 * 60 * MINUTE;

const formatters = new Map<string, Intl.DateTimeFormat>();

function formatter(key: string, make: () => Intl.DateTimeFormat): Intl.DateTimeFormat {
  let f = formatters.get(key);
  if (!f) {
    f = make();
    formatters.set(key, f);
  }
  return f;
}

export function isValidZone(zone: string): boolean {
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: zone });
    return true;
  } catch {
    return false;
  }
}

// Wall-clock time in a zone, expressed as if it were UTC, so two zones can be subtracted.
function wallClock(date: Date, zone?: string): number {
  if (!zone) return Date.UTC(date.getFullYear(), date.getMonth(), date.getDate(), date.getHours(), date.getMinutes());
  const parts = formatter(`wall|${zone}`, () =>
    new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      hourCycle: "h23",
      year: "numeric",
      month: "numeric",
      day: "numeric",
      hour: "numeric",
      minute: "numeric",
    }),
  ).formatToParts(date);
  const get = (type: string) => Number(parts.find((p) => p.type === type)?.value);
  return Date.UTC(get("year"), get("month") - 1, get("day"), get("hour"), get("minute"));
}

export function formatZoneTime(date: Date, zone: string, hour12: boolean): string {
  try {
    return formatter(`time|${zone}|${hour12}`, () =>
      new Intl.DateTimeFormat([], { timeZone: zone, hour: "2-digit", minute: "2-digit", hour12 }),
    ).format(date);
  } catch {
    return "";
  }
}

// -1, 0 or 1: whether the zone is on yesterday, today or tomorrow compared with here.
export function zoneDayShift(date: Date, zone: string): number {
  try {
    return Math.floor(wallClock(date, zone) / DAY) - Math.floor(wallClock(date) / DAY);
  } catch {
    return 0;
  }
}

function zoneOffsetMinutes(date: Date, zone?: string): number {
  return Math.round((wallClock(date, zone) - wallClock(date, "UTC")) / MINUTE);
}

function formatHours(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `${h}:${String(m).padStart(2, "0")}` : String(h);
}

export function zoneOffsetLabel(zone: string, date = new Date()): string {
  try {
    const offset = zoneOffsetMinutes(date, zone);
    if (offset === 0) return "UTC";
    return `UTC${offset > 0 ? "+" : "−"}${formatHours(Math.abs(offset))}`;
  } catch {
    return "";
  }
}

export function zoneDifferenceLabel(zone: string, date = new Date()): string {
  try {
    const diff = zoneOffsetMinutes(date, zone) - zoneOffsetMinutes(date);
    if (diff === 0) return t("dualClock.sameTime");
    const abs = Math.abs(diff);
    const amount = `${Math.floor(abs / 60)}h${abs % 60 ? ` ${abs % 60}m` : ""}`;
    return t(diff > 0 ? "dualClock.ahead" : "dualClock.behind", { amount });
  } catch {
    return "";
  }
}

// "America/Argentina/Buenos_Aires" -> "Buenos Aires"
export function zoneCityName(zone: string): string {
  return zone.split("/").pop()!.replace(/_/g, " ");
}

let zoneList: string[] | null = null;

function allZones(): string[] {
  if (!zoneList) {
    let zones: string[] = [];
    try {
      zones = (Intl as unknown as { supportedValuesOf: (key: string) => string[] }).supportedValuesOf("timeZone");
    } catch {
      zones = [];
    }
    zoneList = zones.includes("UTC") ? zones : ["UTC", ...zones];
  }
  return zoneList;
}

// Offline matches against the IANA zone list, so the picker still works without a network.
export function searchZones(query: string, limit = 6): ZoneOption[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return allZones()
    .filter((zone) => zone.toLowerCase().replace(/_/g, " ").includes(q))
    .sort((a, b) => Number(!zoneCityName(a).toLowerCase().startsWith(q)) - Number(!zoneCityName(b).toLowerCase().startsWith(q)))
    .slice(0, limit)
    .map((zone) => ({ label: zoneCityName(zone), zone, detail: zone.includes("/") ? zone.split("/")[0] : "" }));
}

const NOTCH_FONT = '"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
// Keep in step with .time-flip-container, .time-secondary and .time-secondary-label in App.css.
const CLOCK_MIN_WIDTH = 60;
const SECONDARY_CHROME = 18;
const SECONDARY_GAP = 4;
const LABEL_MAX_WIDTH = 64;

let measureCtx: CanvasRenderingContext2D | null | undefined;

function textWidth(text: string, font: string, letterSpacing: number): number {
  if (measureCtx === undefined) measureCtx = document.createElement("canvas").getContext("2d");
  if (!measureCtx) return text.length * 7;
  measureCtx.font = font;
  return measureCtx.measureText(text).width + text.length * letterSpacing;
}

export function dayShiftLabel(shift: number): string {
  return shift > 0 ? `+${shift}` : `−${-shift}`;
}

// The notch width is animated explicitly, so it has to know how much room the second clock takes.
export function measureDualClock(time: string, label: string, otherTime: string, shift: number) {
  const main = Math.ceil(textWidth(time, `500 13px ${NOTCH_FONT}`, 0.3));
  let secondary =
    SECONDARY_CHROME +
    Math.min(textWidth(label.toUpperCase(), `600 9px ${NOTCH_FONT}`, 0.4), LABEL_MAX_WIDTH) +
    SECONDARY_GAP +
    textWidth(otherTime, `500 13px ${NOTCH_FONT}`, 0.3);
  if (shift) secondary += SECONDARY_GAP + textWidth(dayShiftLabel(shift), `600 9px ${NOTCH_FONT}`, 0);
  return { main, extra: Math.max(0, Math.ceil(main + secondary - CLOCK_MIN_WIDTH)) };
}
