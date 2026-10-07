import { useEffect, useState } from "react";
import type { MonitorInfo } from "./types";

// Switches between one behavior for every monitor and one per monitor.
// Turning it on starts each monitor at the current global mode, turning it off clears the overrides.
export function usePerMonitor(
  globalMode: string,
  modeByMonitor: Record<string, string>,
  replaceModes: (next: Record<string, string>) => void,
  monitors: MonitorInfo[],
) {
  const [enabled, setEnabled] = useState(() => Object.keys(modeByMonitor).length > 0);

  // The overrides load after the first render.
  useEffect(() => {
    if (Object.keys(modeByMonitor).length > 0) setEnabled(true);
  }, [modeByMonitor]);

  // One save for the whole map, saving per monitor echoed back half-cleared states and flickered.
  const toggle = () => {
    if (enabled) {
      setEnabled(false);
      replaceModes({});
    } else {
      setEnabled(true);
      replaceModes(Object.fromEntries(monitors.map((m) => [m.id, modeByMonitor[m.id] || globalMode])));
    }
  };

  return { enabled, toggle };
}
