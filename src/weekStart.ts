import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { dateLocale } from "./i18n";
import { useSettingsSync } from "./hooks/useSettingsSync";

export const WEEK_START_KEY = "nectar-week-start";

export function resolveWeekStart(raw: string | null | undefined): number {
  if (raw === "monday") return 1;
  if (raw === "sunday") return 0;
  try {
    const locale = new Intl.Locale(dateLocale()) as Intl.Locale & { getWeekInfo?: () => { firstDay: number }; weekInfo?: { firstDay: number } };
    const first = (locale.getWeekInfo?.() ?? locale.weekInfo)?.firstDay;
    if (first) return first % 7;
  } catch {}
  return 0;
}

export function daysSinceWeekStart(day: Date, weekStart: number): number {
  return (day.getDay() - weekStart + 7) % 7;
}

export function useWeekStart(): number {
  const [value, setValue] = useState(() => resolveWeekStart(localStorage.getItem(WEEK_START_KEY)));
  useEffect(() => {
    invoke<Record<string, unknown>>("load_settings")
      .then((s) => {
        const raw = s?.[WEEK_START_KEY];
        if (raw != null) setValue(resolveWeekStart(String(raw)));
      })
      .catch(() => {});
  }, []);
  useSettingsSync({ [WEEK_START_KEY]: (v) => setValue(resolveWeekStart(String(v))) });
  return value;
}
