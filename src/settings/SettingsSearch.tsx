import { useEffect, useMemo, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { dateLocale, t as translate, useT } from "../i18n";
import { loadCatalogs, runSearch } from "./searchEngine";
import { TAB_KEYS } from "./searchIndex";
import type { SearchEntry } from "./searchIndex";

type Catalogs = Awaited<ReturnType<typeof loadCatalogs>>;

export function useSearchCatalogs(): Catalogs | null {
  const [cats, setCats] = useState<Catalogs | null>(null);
  useEffect(() => {
    let live = true;
    loadCatalogs().then((c) => live && setCats(c));
    return () => {
      live = false;
    };
  }, []);
  return cats;
}

export function SettingsSearchBox({ query, setQuery, onEnter }: { query: string; setQuery: (q: string) => void; onEnter: () => void }) {
  const t = useT();
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        ref.current?.focus();
        ref.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="settings-search">
      <Search size={13} strokeWidth={1.75} />
      <input
        ref={ref}
        value={query}
        placeholder={t("settings.search")}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") setQuery("");
          if (e.key === "Enter") onEnter();
        }}
        spellCheck={false}
        autoComplete="off"
      />
      {query && (
        <button className="settings-search-clear" onClick={() => setQuery("")} aria-label={t("shortcut.clear")}>
          <X size={12} strokeWidth={2} />
        </button>
      )}
    </div>
  );
}

export function useSearchHits(query: string): SearchEntry[] {
  const cats = useSearchCatalogs();
  useT();
  return useMemo(() => {
    if (!cats || !query.trim()) return [];
    return runSearch(cats, dateLocale(), query).map((h) => h.entry);
  }, [cats, query, dateLocale()]);
}

export function SearchResults({ hits, query, onPick }: { hits: SearchEntry[]; query: string; onPick: (entry: SearchEntry) => void }) {
  const t = useT();
  if (hits.length === 0) {
    return <div className="settings-search-empty">{t("settings.search.empty", { query: query.trim() })}</div>;
  }
  return (
    <div className="settings-search-results">
      {hits.map((entry) => {
        const descKey = `${entry.key}.desc`;
        const desc = translate(descKey);
        const isTab = entry.key.startsWith("tabs.");
        return (
          <button key={entry.key} className="settings-search-result" onClick={() => onPick(entry)}>
            <span className="settings-search-result-text">
              <span className="settings-search-result-label">{t(entry.key)}</span>
              {!isTab && desc !== descKey && <span className="settings-search-result-desc">{desc}</span>}
            </span>
            {!isTab && <span className="settings-search-result-tab">{t(TAB_KEYS[entry.tab] ?? "tabs.general")}</span>}
          </button>
        );
      })}
    </div>
  );
}
