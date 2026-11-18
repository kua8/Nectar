import { dateLocale } from "./i18n";

export type NotchDateFormat = "weekday" | "date";
export type NotchDatePosition = "left" | "right";

export const parseNotchDateFormat = (v: unknown): NotchDateFormat => (v === "date" ? "date" : "weekday");
export const parseNotchDatePosition = (v: unknown): NotchDatePosition => (v === "right" ? "right" : "left");

/** "Per 19 Kas" / "Thu Nov 19", or just "19 Kas" / "Nov 19". Weekday comes first in every locale. */
export function formatNotchDate(now: Date, format: NotchDateFormat): string {
  const locale = dateLocale();
  const day = now.toLocaleDateString(locale, { day: "numeric", month: "short" });
  if (format === "date") return day;
  return `${now.toLocaleDateString(locale, { weekday: "short" })} ${day}`;
}
