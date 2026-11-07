import en from "../locales/en.json";
import { KEYWORDS, SEARCH_ENTRIES, TAB_ENTRIES, TAB_KEYS, type SearchEntry } from "./searchIndex";

type Messages = Record<string, string>;

const LOADERS: Record<string, () => Promise<{ default: Messages }>> = {
  tr: () => import("../locales/tr.json"),
  de: () => import("../locales/de.json"),
  ar: () => import("../locales/ar.json"),
  es: () => import("../locales/es.json"),
  fr: () => import("../locales/fr.json"),
};

let catalogs: Promise<Record<string, Messages>> | null = null;

export function loadCatalogs(): Promise<Record<string, Messages>> {
  if (!catalogs) {
    catalogs = Promise.all(Object.entries(LOADERS).map(async ([lang, load]) => [lang, (await load()).default] as const)).then(
      (pairs) => ({ en: en as Messages, ...Object.fromEntries(pairs) })
    );
  }
  return catalogs;
}

export function normalize(text: string): string {
  return text
    .toLowerCase()
    .replace(/ß/g, "ss")
    .replace(/ı/g, "i")
    .replace(/[œ]/g, "oe")
    .replace(/[æ]/g, "ae")
    .normalize("NFD")
    .replace(/[̀-ًͯ-ٰٟ]/g, "")
    .replace(/[أإآ]/g, "ا")
    .replace(/ى/g, "ي")
    .replace(/ة/g, "ه")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

function distance(a: string, b: string, max: number): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const row = [i];
    let best = row[0];
    for (let j = 1; j <= b.length; j++) {
      row[j] = Math.min(prev[j] + 1, row[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (row[j] < best) best = row[j];
    }
    if (best > max) return max + 1;
    prev = row;
  }
  return prev[b.length];
}

function tokenScore(token: string, text: string): number {
  if (!text) return 0;
  const words = text.split(" ");
  let best = 0;
  for (const word of words) {
    if (word === token) best = Math.max(best, 1);
    else if (word.startsWith(token)) best = Math.max(best, 0.85);
    else if (token.length >= 3 && word.includes(token)) best = Math.max(best, 0.55);
    else if (token.length >= 4 && word.length >= 4) {
      const allowed = token.length >= 8 ? 2 : 1;
      const d = distance(token, word.slice(0, token.length + 1), allowed);
      if (d <= allowed) best = Math.max(best, 0.5 - d * 0.1);
    }
  }
  return best;
}

export interface SearchHit {
  entry: SearchEntry;
  score: number;
}

interface Prepared {
  entry: SearchEntry;
  fields: { text: string; weight: number }[];
}

let prepared: { lang: string; items: Prepared[] } | null = null;

function prepare(cats: Record<string, Messages>, lang: string): Prepared[] {
  if (prepared && prepared.lang === lang) return prepared.items;
  const items = [...SEARCH_ENTRIES, ...TAB_ENTRIES].map((entry) => {
    const fields: { text: string; weight: number }[] = [];
    for (const [code, messages] of Object.entries(cats)) {
      const langWeight = code === lang ? 1 : code === "en" ? 0.85 : 0.75;
      const label = messages[entry.key];
      const desc = messages[`${entry.key}.desc`];
      if (label) fields.push({ text: normalize(label), weight: 3 * langWeight });
      if (desc) fields.push({ text: normalize(desc), weight: 1 * langWeight });
      const tabLabel = messages[TAB_KEYS[entry.tab]];
      if (tabLabel && !entry.key.startsWith("tabs.")) fields.push({ text: normalize(tabLabel), weight: 0.5 * langWeight });
    }
    const extra = KEYWORDS[entry.key];
    if (extra) fields.push({ text: normalize(extra), weight: 1.4 });
    return { entry, fields };
  });
  prepared = { lang, items };
  return items;
}

export function runSearch(cats: Record<string, Messages>, lang: string, query: string): SearchHit[] {
  const tokens = normalize(query).split(" ").filter(Boolean);
  if (tokens.length === 0) return [];
  const hits: SearchHit[] = [];
  for (const { entry, fields } of prepare(cats, lang)) {
    let total = 0;
    let all = true;
    for (const token of tokens) {
      let best = 0;
      for (const f of fields) {
        const s = tokenScore(token, f.text) * f.weight;
        if (s > best) best = s;
      }
      if (best === 0) {
        all = false;
        break;
      }
      total += best;
    }
    if (all) hits.push({ entry, score: total / tokens.length });
  }
  hits.sort((a, b) => b.score - a.score);
  return hits.slice(0, 10);
}
