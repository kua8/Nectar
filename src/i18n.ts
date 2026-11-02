import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import en from "./locales/en.json";

type Messages = Record<string, string>;

/**
 * Locale tables.
 *
 * Only English is bundled eagerly. Every window used to parse every locale at
 * startup even though one language is ever displayed, and this table is part of
 * the chunk all five windows preload. The rest are fetched when the language is
 * actually selected; until one arrives the UI falls back to English, which is
 * the same behaviour as a missing key.
 *
 * To add a language: drop `src/locales/<code>.json` next to the others, add a
 * loader to `LAZY_LOCALES`, and register it in `src-tauri/src/i18n.rs` so the
 * tray menu is translated too.
 */
export const LANGUAGES: Record<string, { name: string }> = {
  en: { name: "English" },
  tr: { name: "Türkçe" },
};

const LAZY_LOCALES: Record<string, () => Promise<{ default: Messages }>> = {
  tr: () => import("./locales/tr.json"),
};

const loaded: Record<string, Messages> = { en: en as Messages };

function loadIfNeeded(lang: string) {
  if (loaded[lang] || !LAZY_LOCALES[lang]) return;
  loaded[lang] = {};
  LAZY_LOCALES[lang]()
    .then((mod) => {
      loaded[lang] = mod.default as Messages;
      listeners.forEach((l) => l());
    })
    .catch(() => {
    });
}

const STORAGE_KEY = "nectar-language";
const listeners = new Set<() => void>();

function readPreference(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) || "system";
  } catch {
    return "system";
  }
}

function resolve(preference: string): string {
  if (preference in LANGUAGES) return preference;
  const system = (navigator.language || "en").toLowerCase().split("-")[0];
  return system in LANGUAGES ? system : "en";
}

export function getLanguagePreference(): string {
  return readPreference();
}

export function setLanguagePreference(value: string) {
  localStorage.setItem(STORAGE_KEY, value);
  invoke("save_setting", { key: STORAGE_KEY, value }).catch(console.error);
  loadIfNeeded(resolve(value));
  listeners.forEach((l) => l());
}

export function t(key: string, params?: Record<string, string | number>): string {
  const lang = resolve(readPreference());
  if (lang !== "en") loadIfNeeded(lang);
  const messages = loaded[lang];
  const text = messages?.[key] ?? (en as Messages)[key] ?? key;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, name) => (name in params ? String(params[name]) : m));
}

/** Locale for dates and times, so they match the chosen UI language. */
export function dateLocale(): string {
  return resolve(readPreference());
}

/** Weekday names starting on Sunday. */
export function weekdayNames(width: "short" | "narrow"): string[] {
  return Array.from({ length: 7 }, (_, i) => new Date(2023, 0, 1 + i).toLocaleDateString(dateLocale(), { weekday: width }));
}

export function timeAgo(ms: number | null, translate: (key: string, params?: Record<string, string | number>) => string): string {
  if (!ms) return translate("common.ago.never");
  const mins = Math.round((Date.now() - ms) / 60000);
  if (mins < 1) return translate("common.ago.now");
  if (mins < 60) return translate("common.ago.minutes", { count: mins });
  const hours = Math.round(mins / 60);
  return hours < 24
    ? translate("common.ago.hours", { count: hours })
    : translate("common.ago.days", { count: Math.round(hours / 24) });
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  loadIfNeeded(resolve(readPreference()));
  // Other windows change the setting through localStorage.
  const onStorage = (e: StorageEvent) => {
    if (e.key === STORAGE_KEY || e.key === null) {
      loadIfNeeded(resolve(readPreference()));
      listener();
    }
  };
  window.addEventListener("storage", onStorage);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", onStorage);
  };
}

/** Re-renders the calling component when the language changes; returns the translate function. */
export function useT() {
  useSyncExternalStore(subscribe, readPreference);
  return t;
}

export type TFunction = typeof t;
