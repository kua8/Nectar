import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CaldavState, CalEvent } from "../hooks/useCaldav";

type View = "day" | "week" | "month" | "agenda";

const VIEWS: { id: View; label: string }[] = [
  { id: "day", label: "Day" },
  { id: "week", label: "Week" },
  { id: "month", label: "Month" },
  { id: "agenda", label: "Agenda" },
];

const VIEW_KEY = "nectar-calendar-view";
const AGENDA_DAYS = 14;

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

function colorVar(e: CalEvent): React.CSSProperties {
  return { ["--c" as string]: e.color ?? "var(--nectar-text, #fff)" };
}

interface Props {
  caldav: CaldavState;
  hour12: boolean;
}

export function NotchCalendar({ caldav, hour12 }: Props) {
  const connected = caldav.connected;
  const today = startOfDay(new Date());
  const [view, setViewState] = useState<View>(readView);
  const [anchor, setAnchor] = useState<Date>(today);

  const time = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit", hour12 });

  const setView = (next: View) => {
    setViewState(next);
    remember(VIEW_KEY, next);
    invoke("save_setting", { key: VIEW_KEY, value: next }).catch(() => {});
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

  const activeView: View = connected ? view : "month";

  let title: string;
  if (activeView === "week") {
    const first = addDays(anchor, -anchor.getDay());
    const last = addDays(first, 6);
    const fmt = (d: Date) => d.toLocaleDateString("default", { month: "short", day: "numeric" });
    title = `${fmt(first)} – ${first.getMonth() === last.getMonth() ? last.getDate() : fmt(last)}`;
  } else if (activeView === "day") {
    title = anchor.toLocaleDateString("default", { weekday: "short", month: "short", day: "numeric" });
  } else if (activeView === "agenda") {
    title = "Upcoming";
  } else {
    title = anchor.toLocaleDateString("default", { month: "long", year: "numeric" });
  }

  const showToday = connected && activeView !== "agenda" && !sameDay(anchor, today);

  return (
    <div className={`calendar-container${connected ? " with-agenda" : ""}`}>
      <div className="calendar-header">
        <span className="month-year">{title}</span>
        {connected && (
          <span className="cal-nav-group">
            <button className="cal-open" onClick={() => invoke("open_calendar_window").catch(() => {})} title="Open full calendar" aria-label="Open full calendar">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M15 3h6v6" />
                <path d="M10 14L21 3" />
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
              </svg>
            </button>
            {showToday && (
              <button className="cal-today" onClick={() => setAnchor(today)}>
                Today
              </button>
            )}
            {activeView !== "agenda" && (
              <>
                <button className="cal-nav" onClick={() => shift(-1)} aria-label="Previous">
                  &#8249;
                </button>
                <button className="cal-nav" onClick={() => shift(1)} aria-label="Next">
                  &#8250;
                </button>
              </>
            )}
          </span>
        )}
      </div>

      {connected && (
        <div className="cal-views" role="tablist">
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
      )}

      {activeView === "month" && (
        <>
          <MonthGrid anchor={anchor} today={today} events={caldav.events} connected={connected} onPick={setAnchor} />
          {connected && (
            <SelectedDay day={anchor} events={caldav.events} time={time} onOpen={() => openDay(anchor)} />
          )}
        </>
      )}
      {connected && activeView === "week" && (
        <WeekList anchor={anchor} today={today} events={caldav.events} time={time} onOpen={openDay} />
      )}
      {connected && activeView === "day" && <DayList day={anchor} events={caldav.events} time={time} />}
      {connected && activeView === "agenda" && (
        <AgendaList today={today} events={caldav.events} time={time} onOpen={openDay} />
      )}
    </div>
  );
}

interface MonthProps {
  anchor: Date;
  today: Date;
  events: CalEvent[];
  connected: boolean;
  onPick: (day: Date) => void;
}

function MonthGrid({ anchor, today, events, connected, onPick }: MonthProps) {
  const year = anchor.getFullYear();
  const month = anchor.getMonth();
  const totalDays = new Date(year, month + 1, 0).getDate();
  const startDay = new Date(year, month, 1).getDay();

  const byDay = useMemo(() => {
    if (!connected) return [];
    return Array.from({ length: totalDays }, (_, i) => eventsOn(events, new Date(year, month, i + 1)));
  }, [connected, events, year, month, totalDays]);

  const cells = [];
  for (let i = 0; i < startDay; i++) {
    cells.push(<div key={`empty-${i}`} className="calendar-day empty" />);
  }
  for (let i = 1; i <= totalDays; i++) {
    const day = new Date(year, month, i);
    const list = byDay[i - 1] ?? [];
    const classes = ["calendar-day"];
    if (sameDay(day, today)) classes.push("today");
    if (list.length) classes.push("has-events", list.length >= 4 ? "lvl-3" : list.length >= 2 ? "lvl-2" : "lvl-1");
    if (connected && sameDay(day, anchor)) classes.push("selected");
    const colors = Array.from(new Set(list.map((e) => e.color ?? "currentColor"))).slice(0, 3);
    cells.push(
      <div key={i} className={classes.join(" ")} onClick={connected ? () => onPick(day) : undefined}>
        {i}
        {colors.length > 0 && (
          <span className="event-dots">
            {colors.map((c) => (
              <span key={c} className="event-dot" style={{ background: c }} />
            ))}
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="calendar-grid">
      {["S", "M", "T", "W", "T", "F", "S"].map((d, i) => (
        <div key={`${d}-${i}`} className="day-name">
          {d}
        </div>
      ))}
      {cells}
    </div>
  );
}

interface TimeProps {
  time: (ms: number) => string;
}

function SelectedDay({ day, events, time, onOpen }: TimeProps & { day: Date; events: CalEvent[]; onOpen: () => void }) {
  const list = eventsOn(events, day);
  const label = day.toLocaleDateString("default", { weekday: "short", day: "numeric" });
  return (
    <div className="calendar-agenda">
      <button className="agenda-title" onClick={onOpen}>
        {label} <span aria-hidden>&#8250;</span>
      </button>
      {list.length === 0 && <div className="agenda-empty">No events</div>}
      {list.slice(0, 3).map((e, i) => (
        <div key={`${e.uid}-${e.start}-${i}`} className="agenda-item" style={colorVar(e)} title={e.location ? `${e.title} · ${e.location}` : e.title}>
          <span className="agenda-dot" />
          <span className="agenda-time">{e.allDay ? "All day" : time(e.start)}</span>
          <span className="agenda-name">{e.title}</span>
        </div>
      ))}
      {list.length > 3 && (
        <button className="agenda-more" onClick={onOpen}>
          +{list.length - 3} more
        </button>
      )}
    </div>
  );
}

function Chip({ event, time }: TimeProps & { event: CalEvent }) {
  return (
    <span className="cal-chip" style={colorVar(event)} title={event.location ? `${event.title} · ${event.location}` : event.title}>
      <i>{event.allDay ? "All day" : time(event.start)}</i>
      <span>{event.title}</span>
    </span>
  );
}

function WeekList({ anchor, today, events, time, onOpen }: TimeProps & { anchor: Date; today: Date; events: CalEvent[]; onOpen: (d: Date) => void }) {
  const first = addDays(anchor, -anchor.getDay());
  return (
    <div className="cal-scroll">
      {Array.from({ length: 7 }, (_, i) => {
        const day = addDays(first, i);
        const list = eventsOn(events, day);
        return (
          <button key={i} className={`cal-week-row${sameDay(day, today) ? " today" : ""}`} onClick={() => onOpen(day)}>
            <span className="cal-week-label">
              <em>{day.toLocaleDateString("default", { weekday: "short" })}</em>
              <b>{day.getDate()}</b>
            </span>
            <span className="cal-week-events">
              {list.length === 0 && <span className="cal-none">Free</span>}
              {list.slice(0, 2).map((e, k) => (
                <Chip key={`${e.uid}-${e.start}-${k}`} event={e} time={time} />
              ))}
              {list.length > 2 && <span className="cal-more">+{list.length - 2} more</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}

function DayList({ day, events, time }: TimeProps & { day: Date; events: CalEvent[] }) {
  const list = eventsOn(events, day);
  const [nowMs] = useState(() => Date.now());
  if (list.length === 0) {
    return <div className="cal-empty">Nothing planned</div>;
  }
  return (
    <div className="cal-scroll">
      {list.map((e, i) => {
        const live = !e.allDay && e.start <= nowMs && e.end > nowMs;
        return (
          <div key={`${e.uid}-${e.start}-${i}`} className="cal-day-event" style={colorVar(e)}>
            <span className="cal-day-time">
              {e.allDay ? (
                <b>All day</b>
              ) : (
                <>
                  <b>{time(e.start)}</b>
                  <span>{time(e.end)}</span>
                </>
              )}
            </span>
            <span className="cal-day-body">
              <span className="cal-day-title">
                {e.title}
                {live && <em>Now</em>}
              </span>
              <span className="cal-day-meta">{[e.location, e.calendar].filter(Boolean).join(" · ")}</span>
            </span>
          </div>
        );
      })}
    </div>
  );
}

function AgendaList({ today, events, time, onOpen }: TimeProps & { today: Date; events: CalEvent[]; onOpen: (d: Date) => void }) {
  const groups = Array.from({ length: AGENDA_DAYS }, (_, i) => addDays(today, i))
    .map((day) => ({ day, list: eventsOn(events, day) }))
    .filter((g) => g.list.length > 0);

  if (groups.length === 0) {
    return <div className="cal-empty">Nothing in the next two weeks</div>;
  }

  const heading = (day: Date) => {
    const diff = Math.round((day.getTime() - today.getTime()) / 86_400_000);
    if (diff === 0) return "Today";
    if (diff === 1) return "Tomorrow";
    return day.toLocaleDateString("default", { weekday: "short", month: "short", day: "numeric" });
  };

  return (
    <div className="cal-scroll">
      {groups.map(({ day, list }) => (
        <div key={day.getTime()} className="cal-agenda-group">
          <button className="cal-agenda-heading" onClick={() => onOpen(day)}>
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
