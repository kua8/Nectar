import { useEffect, useState } from "react";
import type { Stopwatch } from "../hooks/useStopwatch";
import { formatStopwatch, parseDuration } from "../clockFormat";

type Tab = "timer" | "stopwatch";

const TAB_KEY = "nectar-clock-tab";
const PRESETS = [5, 15, 25, 50];

export function StopwatchReadout({ base, startedAt, precise }: { base: number; startedAt: number | null; precise: boolean }) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (startedAt === null) return;
    const id = setInterval(() => setNow(Date.now()), precise ? 50 : 250);
    return () => clearInterval(id);
  }, [startedAt, precise]);

  const ms = base + (startedAt === null ? 0 : Math.max(0, Math.max(now, startedAt) - startedAt));
  return <>{formatStopwatch(ms, precise)}</>;
}

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    return;
  }
}

function readTab(): Tab {
  try {
    return localStorage.getItem(TAB_KEY) === "stopwatch" ? "stopwatch" : "timer";
  } catch {
    return "timer";
  }
}

interface Props {
  timerEnabled: boolean;
  stopwatchEnabled: boolean;
  solo: boolean;
  timerSeconds: number;
  isTimerRunning: boolean;
  formatTimerTime: (seconds: number) => string;
  onStartTimer: (seconds: number) => void;
  onToggleTimer: () => void;
  onResetTimer: () => void;
  stopwatch: Stopwatch;
}

export function NotchClock({
  timerEnabled,
  stopwatchEnabled,
  solo,
  timerSeconds,
  isTimerRunning,
  formatTimerTime,
  onStartTimer,
  onToggleTimer,
  onResetTimer,
  stopwatch,
}: Props) {
  const [savedTab, setSavedTab] = useState<Tab>(readTab);
  const [custom, setCustom] = useState("");
  const [invalid, setInvalid] = useState(false);

  const tab: Tab = savedTab === "stopwatch" && stopwatchEnabled ? "stopwatch" : timerEnabled ? "timer" : "stopwatch";

  const pickTab = (next: Tab) => {
    setSavedTab(next);
    remember(TAB_KEY, next);
  };

  const submitCustom = (e: React.FormEvent) => {
    e.preventDefault();
    const seconds = parseDuration(custom);
    if (seconds === null) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    setCustom("");
    onStartTimer(seconds);
  };

  const recentLaps = stopwatch.laps.slice(-3).map((total, i, list) => {
    const index = stopwatch.laps.length - list.length + i;
    const previous = index === 0 ? 0 : stopwatch.laps[index - 1];
    return { number: index + 1, split: total - previous };
  });

  return (
    <div className={`timer-column${solo ? " solo" : ""}`}>
      <div className="timer-section-new">
        {timerEnabled && stopwatchEnabled && (
          <div className="clock-tabs" role="tablist">
            <button role="tab" aria-selected={tab === "timer"} className={tab === "timer" ? "active" : ""} onClick={() => pickTab("timer")}>
              Timer
            </button>
            <button
              role="tab"
              aria-selected={tab === "stopwatch"}
              className={tab === "stopwatch" ? "active" : ""}
              onClick={() => pickTab("stopwatch")}
            >
              Stopwatch
            </button>
          </div>
        )}

        {tab === "timer" ? (
          <>
            <div className="timer-display-large">
              <span className="timer-time-large">{formatTimerTime(timerSeconds)}</span>
            </div>

            <div className="timer-controls-new">
              <button onClick={onToggleTimer} className="timer-btn primary">
                {isTimerRunning ? "Pause" : "Start"}
              </button>
              <button onClick={onResetTimer} className="timer-btn secondary">
                Reset
              </button>
            </div>

            <div className="timer-presets-new">
              {PRESETS.map((mins) => (
                <button key={mins} onClick={() => onStartTimer(mins * 60)} className="preset-btn-small">
                  {mins}m
                </button>
              ))}
            </div>

            <form className={`timer-custom${invalid ? " invalid" : ""}`} onSubmit={submitCustom}>
              <input
                value={custom}
                onChange={(e) => {
                  setCustom(e.target.value);
                  setInvalid(false);
                }}
                placeholder="Custom: 12, 1:30 or 0:45:00"
                spellCheck={false}
                aria-label="Custom timer length"
              />
              <button type="submit" disabled={!custom.trim()}>
                Go
              </button>
            </form>
          </>
        ) : (
          <>
            <div className="timer-display-large">
              <span className="timer-time-large timer-time-precise">
                <StopwatchReadout base={stopwatch.base} startedAt={stopwatch.startedAt} precise />
              </span>
            </div>

            <div className="timer-controls-new">
              <button onClick={stopwatch.toggle} className="timer-btn primary">
                {stopwatch.running ? "Pause" : "Start"}
              </button>
              <button onClick={stopwatch.lap} className="timer-btn secondary" disabled={!stopwatch.running && stopwatch.base === 0}>
                Lap
              </button>
              <button onClick={stopwatch.reset} className="timer-btn secondary">
                Reset
              </button>
            </div>

            <div className="sw-laps">
              {recentLaps.length === 0 && <span className="sw-hint">Laps show up here</span>}
              {recentLaps.map((lap) => (
                <div key={lap.number} className="sw-lap">
                  <span>Lap {lap.number}</span>
                  <span>{formatStopwatch(lap.split, true)}</span>
                </div>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
