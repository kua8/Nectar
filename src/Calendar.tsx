import { StrictMode, useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Effect, EffectState } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { Check, ChevronLeft, ChevronRight, RefreshCw, Settings as SettingsIcon } from "lucide-react";
import { WindowControls } from "./components/WindowControls";
import { initTheme } from "./theme";
import { useCaldav, type CalEvent } from "./hooks/useCaldav";
import "./Calendar.css";

type View = "day" | "week" | "month" | "agenda";

const VIEWS: { id: View; label: string }[] = [
  { id: "day", label: "Day" },
  { id: "week", label: "Week" },
  { id: "month", label: "Month" },
  { id: "agenda", label: "Agenda" },
];

const VIEW_KEY = "nectar-calendar-window-view";
const HIDDEN_KEY = "nectar-calendar-hidden";
const appWindow = getCurrentWebviewWindow();

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const sameDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

function eventsOn(events: CalEvent[], day: Date): CalEvent[] {
  const from = day.getTime();
  const to = addDays(day, 1).getTime();
  return events
    .filter((e) => e.start < to && e.end > from)
    .sort((a, b) => Number(b.allDay) - Number(a.allDay) || a.start - b.start);
}

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    return;
  }
}

function readView(): View {
  try {
    const saved = localStorage.getItem(VIEW_KEY);
    if (saved === "day" || saved === "week" || saved === "month" || saved === "agenda") return saved;
  } catch {
    return "month";
  }
  return "month";
}

function readHidden(): Set<string> {
  try {
    const parsed = JSON.parse(localStorage.getItem(HIDDEN_KEY) ?? "[]");
    return new Set(Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : []);
  } catch {
    return new Set();
  }
}

function ago(ms: number | null): string {
  if (!ms) return "never";
  const mins = Math.round((Date.now() - ms) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  return hours < 24 ? `${hours} h ago` : `${Math.round(hours / 24)} d ago`;
}

function colorStyle(e: CalEvent): React.CSSProperties {
  return { ["--c" as string]: e.color ?? "var(--nectar-text, #fff)" };
}

function CalendarWindow() {
  const state = useCaldav();
  const today = startOfDay(new Date());
  const [view, setViewState] = useState<View>(readView);
  const [anchor, setAnchor] = useState<Date>(today);
  const [hidden, setHidden] = useState<Set<string>>(readHidden);
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const hour12 = localStorage.getItem("nectar-time-format-24h") !== "true";

  useEffect(() => initTheme(), []);

  useEffect(() => {
    appWindow.setEffects({ effects: [Effect.Acrylic], state: EffectState.Active, color: [10, 10, 15, 20] }).catch(() => {});
    const preventContext = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", preventContext);
    return () => document.removeEventListener("contextmenu", preventContext);
  }, []);

  const time = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit", hour12 });

  const setView = (next: View) => {
    setViewState(next);
    remember(VIEW_KEY, next);
  };

  const toggleCalendar = (name: string) => {
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      remember(HIDDEN_KEY, JSON.stringify([...next]));
      return next;
    });
  };

  const events = useMemo(() => state.events.filter((e) => !hidden.has(e.calendar)), [state.events, hidden]);

  const sync = async () => {
    setSyncing(true);
    setSyncError(null);
    try {
      await invoke("caldav_sync_now");
    } catch (e) {
      setSyncError(String(e));
    } finally {
      setSyncing(false);
    }
  };

  const manageSources = async () => {
    await invoke("open_settings_window");
    await emit("settings-navigate", "calendar-sync");
  };

  const shift = (dir: number) => {
    if (view === "month") {
      const next = new Date(anchor.getFullYear(), anchor.getMonth() + dir, 1);
      setAnchor(next.getFullYear() === today.getFullYear() && next.getMonth() === today.getMonth() ? today : next);
    } else if (view === "week") {
      setAnchor(addDays(anchor, 7 * dir));
    } else {
      setAnchor(addDays(anchor, dir));
    }
  };

  const openDay = (day: Date) => {
    setAnchor(day);
    setView("day");
  };

  let title: string;
  if (view === "week") {
    const first = addDays(anchor, -anchor.getDay());
    const last = addDays(first, 6);
    const fmt = (d: Date) => d.toLocaleDateString("default", { month: "short", day: "numeric" });
    title = `${fmt(first)} – ${first.getMonth() === last.getMonth() ? last.getDate() : fmt(last)}`;
  } else if (view === "day") {
    title = anchor.toLocaleDateString("default", { weekday: "long", month: "long", day: "numeric" });
  } else if (view === "agenda") {
    title = "Upcoming";
  } else {
    title = anchor.toLocaleDateString("default", { month: "long", year: "numeric" });
  }

  const selectedEvents = eventsOn(events, anchor);

  return (
    <div className="cw">
      <div className="cw-titlebar" data-tauri-drag-region>
        <WindowControls canZoom />
        <span className="cw-title-text" data-tauri-drag-region>
          Calendar
        </span>
        <span className="cw-title-spacer" />
      </div>

      <div className="cw-body">
        <aside className="cw-sidebar">
          <div className="cw-label">Calendars</div>
          <ul className="cw-sources">
            {state.calendars.map((c) => {
              const off = hidden.has(c.name);
              const color = c.color ?? "var(--nectar-text, #fff)";
              return (
                <li key={c.name}>
                  <button onClick={() => toggleCalendar(c.name)} className={off ? "off" : ""}>
                    <span className="cw-check" style={{ borderColor: color, background: off ? "transparent" : color }}>
                      {!off && <Check size={10} strokeWidth={3} />}
                    </span>
                    <span className="cw-source-name">{c.name}</span>
                  </button>
                </li>
              );
            })}
            {state.calendars.length === 0 && <li className="cw-muted">No calendars yet</li>}
          </ul>

          <div className="cw-sidebar-foot">
            <div className="cw-sync-row">
              <span>Synced {syncing ? "now…" : ago(state.syncedAt)}</span>
              <button onClick={sync} disabled={syncing || !state.connected}>
                <RefreshCw size={11} className={syncing ? "spin" : ""} /> Sync
              </button>
            </div>
            {syncError && <div className="cw-error">{syncError}</div>}
            <button className="cw-manage" onClick={manageSources}>
              <SettingsIcon size={12} /> Manage sources
            </button>
          </div>
        </aside>

        <main className="cw-main">
          {!state.connected ? (
            <div className="cw-empty-state">
              <div className="cw-empty-title">No calendar connected</div>
              <div className="cw-empty-text">
                Add a Google, iCloud or Outlook calendar link, or connect Nextcloud, to see your events here.
              </div>
              <button className="cw-primary" onClick={manageSources}>
                Set up Calendar Sync
              </button>
            </div>
          ) : (
            <>
              <div className="cw-toolbar">
                <div className="cw-heading">{title}</div>
                <div className="cw-controls">
                  <div className="cw-segment" role="tablist">
                    {VIEWS.map((v) => (
                      <button
                        key={v.id}
                        role="tab"
                        aria-selected={view === v.id}
                        className={view === v.id ? "active" : ""}
                        onClick={() => setView(v.id)}
                      >
                        {v.label}
                      </button>
                    ))}
                  </div>
                  {view !== "agenda" && (
                    <div className="cw-nav">
                      <button className="cw-today" onClick={() => setAnchor(today)}>
                        Today
                      </button>
                      <button aria-label="Previous" onClick={() => shift(-1)}>
                        <ChevronLeft size={15} />
                      </button>
                      <button aria-label="Next" onClick={() => shift(1)}>
                        <ChevronRight size={15} />
                      </button>
                    </div>
                  )}
                </div>
              </div>

              {view === "month" && (
                <div className="cw-selected-strip">
                  <span className="cw-selected-date">
                    {anchor.toLocaleDateString("default", { weekday: "long", month: "long", day: "numeric" })}
                  </span>
                  <span className="cw-selected-count">
                    {selectedEvents.length === 0 ? "No events" : `${selectedEvents.length} event${selectedEvents.length === 1 ? "" : "s"}`}
                  </span>
                  <button className="cw-primary small" onClick={() => openDay(anchor)}>
                    Open day &#8250;
                  </button>
                </div>
              )}

              <div className="cw-content">
                {view === "month" && <MonthView anchor={anchor} today={today} events={events} time={time} onSelect={setAnchor} onOpen={openDay} />}
                {view === "week" && <WeekView anchor={anchor} today={today} events={events} time={time} onOpen={openDay} />}
                {view === "day" && <DayView day={anchor} events={events} time={time} />}
                {view === "agenda" && <AgendaView today={today} events={events} time={time} onOpen={openDay} />}
              </div>
            </>
          )}
        </main>
      </div>
    </div>
  );
}

function DayNumber({ day, today, muted }: { day: Date; today: Date; muted?: boolean }) {
  return <span className={`cw-daynum${sameDay(day, today) ? " today" : ""}${muted ? " muted" : ""}`}>{day.getDate()}</span>;
}

function Chip({ event, time, stacked }: { event: CalEvent; time?: (ms: number) => string; stacked?: boolean }) {
  return (
    <div className={`cw-chip${stacked ? " stacked" : ""}`} style={colorStyle(event)}>
      {time && <i>{event.allDay ? "All day" : time(event.start)}</i>}
      <span>{event.title}</span>
    </div>
  );
}

interface MonthProps {
  anchor: Date;
  today: Date;
  events: CalEvent[];
  time: (ms: number) => string;
  onSelect: (d: Date) => void;
  onOpen: (d: Date) => void;
}

interface HoverCard {
  day: Date;
  left: number;
  top: number;
}

const CARD_WIDTH = 236;

function MonthView({ anchor, today, events, time, onSelect, onOpen }: MonthProps) {
  const first = new Date(anchor.getFullYear(), anchor.getMonth(), 1);
  const gridStart = addDays(first, -first.getDay());
  const days = Array.from({ length: 42 }, (_, i) => addDays(gridStart, i));

  const wrapRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const [rowHeight, setRowHeight] = useState(90);
  const [hover, setHover] = useState<HoverCard | null>(null);

  useEffect(() => {
    const el = gridRef.current;
    if (!el) return;
    const measure = () => setRowHeight(el.clientHeight / 6);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const capacity = Math.max(0, Math.floor((rowHeight - 30 + 2) / 21));

  const showCard = (e: React.MouseEvent<HTMLDivElement>, day: Date, count: number) => {
    const wrapEl = wrapRef.current;
    if (count === 0 || !wrapEl) {
      setHover(null);
      return;
    }
    const wrap = wrapEl.getBoundingClientRect();
    const cell = e.currentTarget.getBoundingClientRect();
    const estimate = 40 + Math.min(count, 6) * 38 + (count > 6 ? 18 : 0);
    const cellTop = cell.top - wrap.top;
    const cellBottom = cell.bottom - wrap.top;
    const left = Math.min(Math.max(cell.left - wrap.left + cell.width / 2 - CARD_WIDTH / 2, 6), wrapEl.clientWidth - CARD_WIDTH - 6);
    const below = cellBottom + 6 + estimate <= wrapEl.clientHeight;
    setHover({ day, left, top: below ? cellBottom + 6 : Math.max(6, cellTop - estimate - 6) });
  };

  const hoverList = hover ? eventsOn(events, hover.day) : [];

  return (
    <div ref={wrapRef} className="cw-month">
      <div className="cw-dow">
        {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].map((d) => (
          <div key={d}>{d}</div>
        ))}
      </div>
      <div ref={gridRef} className="cw-grid">
        {days.map((day) => {
          const list = eventsOn(events, day);
          const inMonth = day.getMonth() === anchor.getMonth();
          const classes = ["cw-cell"];
          if (sameDay(day, anchor)) classes.push("selected");
          if (sameDay(day, today)) classes.push("today");
          if (!inMonth) classes.push("outside");

          let shown: CalEvent[] = list;
          let extra = 0;
          let compact = false;
          if (list.length > capacity) {
            if (capacity >= 2) {
              shown = list.slice(0, capacity - 1);
              extra = list.length - shown.length;
            } else {
              shown = [];
              compact = true;
            }
          }
          const colors = Array.from(new Set(list.map((e) => e.color ?? "currentColor"))).slice(0, 4);

          return (
            <div
              key={day.getTime()}
              className={classes.join(" ")}
              onClick={() => onSelect(day)}
              onDoubleClick={() => onOpen(day)}
              onMouseEnter={(e) => showCard(e, day, list.length)}
              onMouseLeave={() => setHover(null)}
            >
              <DayNumber day={day} today={today} muted={!inMonth} />
              <div className="cw-cell-events">
                {shown.map((e, i) => (
                  <Chip key={`${e.uid}-${e.start}-${i}`} event={e} />
                ))}
                {extra > 0 && <span className="cw-more">+{extra} more</span>}
                {compact && (
                  <span className="cw-compact">
                    {colors.map((color) => (
                      <span key={color} className="cw-dot" style={{ background: color }} />
                    ))}
                    <span>{list.length}</span>
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {hover && (
        <div className="cw-hover" style={{ left: hover.left, top: hover.top, width: CARD_WIDTH }}>
          <div className="cw-hover-title">
            {hover.day.toLocaleDateString("default", { weekday: "long", month: "short", day: "numeric" })}
          </div>
          {hoverList.slice(0, 6).map((e, i) => (
            <div key={`${e.uid}-${e.start}-${i}`} className="cw-hover-row" style={colorStyle(e)}>
              <span className="cw-hover-name">{e.title}</span>
              <span className="cw-hover-meta">
                {e.allDay ? "All day" : `${time(e.start)} – ${time(e.end)}`}
                {e.location ? ` · ${e.location}` : ""}
              </span>
            </div>
          ))}
          {hoverList.length > 6 && <div className="cw-hover-more">+{hoverList.length - 6} more</div>}
        </div>
      )}
    </div>
  );
}

function WeekView({ anchor, today, events, time, onOpen }: { anchor: Date; today: Date; events: CalEvent[]; time: (ms: number) => string; onOpen: (d: Date) => void }) {
  const first = addDays(anchor, -anchor.getDay());
  return (
    <div className="cw-week">
      {Array.from({ length: 7 }, (_, i) => {
        const day = addDays(first, i);
        const list = eventsOn(events, day);
        return (
          <div key={i} className={`cw-week-col${sameDay(day, today) ? " today" : ""}`}>
            <button className="cw-week-head" onClick={() => onOpen(day)}>
              <em>{day.toLocaleDateString("default", { weekday: "short" })}</em>
              <DayNumber day={day} today={today} />
            </button>
            <div className="cw-week-events">
              {list.map((e, k) => (
                <Chip key={`${e.uid}-${e.start}-${k}`} event={e} time={time} stacked />
              ))}
              {list.length === 0 && <span className="cw-muted center">Free</span>}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function DayView({ day, events, time }: { day: Date; events: CalEvent[]; time: (ms: number) => string }) {
  const list = eventsOn(events, day);
  const [nowMs] = useState(() => Date.now());
  if (list.length === 0) return <div className="cw-empty-day">Nothing planned</div>;
  return (
    <div className="cw-list">
      {list.map((e, i) => {
        const live = !e.allDay && e.start <= nowMs && e.end > nowMs;
        return (
          <div key={`${e.uid}-${e.start}-${i}`} className="cw-day-event" style={colorStyle(e)}>
            <span className="cw-day-time">
              {e.allDay ? (
                <b>All day</b>
              ) : (
                <>
                  <b>{time(e.start)}</b>
                  <span>{time(e.end)}</span>
                </>
              )}
            </span>
            <span className="cw-day-body">
              <span className="cw-day-title">
                {e.title}
                {live && <em>Now</em>}
              </span>
              <span className="cw-day-meta">{[e.location, e.calendar].filter(Boolean).join(" · ")}</span>
            </span>
          </div>
        );
      })}
    </div>
  );
}

function AgendaView({ today, events, time, onOpen }: { today: Date; events: CalEvent[]; time: (ms: number) => string; onOpen: (d: Date) => void }) {
  const groups = Array.from({ length: 30 }, (_, i) => addDays(today, i))
    .map((day) => ({ day, list: eventsOn(events, day) }))
    .filter((g) => g.list.length > 0);

  if (groups.length === 0) return <div className="cw-empty-day">Nothing in the next 30 days</div>;

  const heading = (day: Date) => {
    const diff = Math.round((day.getTime() - today.getTime()) / 86_400_000);
    if (diff === 0) return "Today";
    if (diff === 1) return "Tomorrow";
    return day.toLocaleDateString("default", { weekday: "long", month: "short", day: "numeric" });
  };

  return (
    <div className="cw-list">
      {groups.map(({ day, list }) => (
        <div key={day.getTime()} className="cw-agenda-group">
          <button className="cw-agenda-heading" onClick={() => onOpen(day)}>
            {heading(day)}
          </button>
          {list.map((e, i) => (
            <Chip key={`${e.uid}-${e.start}-${i}`} event={e} time={time} />
          ))}
        </div>
      ))}
    </div>
  );
}

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <CalendarWindow />
  </StrictMode>,
);
