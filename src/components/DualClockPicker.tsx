import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { isValidZone, searchZones, zoneOffsetLabel, type ZoneOption } from "../dualClock";

interface Props {
  onSelect: (option: { label: string; zone: string }) => void;
}

const MAX_RESULTS = 6;

export function DualClockPicker({ onSelect }: Props) {
  const [query, setQuery] = useState("");
  const [cities, setCities] = useState<ZoneOption[]>([]);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2) {
      setCities([]);
      return;
    }
    let cancelled = false;
    const timeout = setTimeout(async () => {
      try {
        const res = await fetch(
          `https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(q)}&count=5&language=en&format=json`
        );
        const data = await res.json();
        if (cancelled) return;
        setCities(
          (data.results ?? [])
            .filter((r: any) => typeof r.timezone === "string" && isValidZone(r.timezone))
            .map((r: any) => ({ label: r.name, zone: r.timezone, detail: r.country || "" }))
        );
      } catch {
        if (!cancelled) setCities([]);
      }
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, [query]);

  const zones = searchZones(query).filter((z) => !cities.some((c) => c.zone === z.zone && c.label === z.label));
  const results = [...cities, ...zones].slice(0, MAX_RESULTS);

  const pick = (option: ZoneOption) => {
    onSelect({ label: option.label, zone: option.zone });
    setQuery("");
    setOpen(false);
  };

  return (
    <div className="manual-city-input">
      <div className="city-input-row">
        <input
          type="text"
          placeholder="Search city or time zone..."
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && results.length > 0) {
              e.preventDefault();
              pick(results[0]);
            }
            if (e.key === "Escape") {
              setOpen(false);
            }
          }}
          onBlur={() => setTimeout(() => setOpen(false), 150)}
          spellCheck={false}
          aria-label="Second clock time zone"
        />
        {query && (
          <button
            className="city-clear-btn"
            onMouseDown={(e) => {
              e.preventDefault();
              setQuery("");
            }}
            title="Clear search"
          >
            <X size={10} strokeWidth={2.5} />
          </button>
        )}
      </div>
      {open && results.length > 0 && (
        <div className="city-dropdown">
          {results.map((option) => (
            <button
              key={`${option.label}-${option.zone}-${option.detail}`}
              className="city-dropdown-item"
              onMouseDown={(e) => {
                e.preventDefault();
                pick(option);
              }}
            >
              <span className="city-dropdown-name">{option.label}</span>
              <span className="city-dropdown-country">
                {[option.detail, zoneOffsetLabel(option.zone)].filter(Boolean).join(" · ")}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
