import { useState } from "react";

export interface Stopwatch {
  base: number;
  startedAt: number | null;
  running: boolean;
  laps: number[];
  toggle: () => void;
  reset: () => void;
  lap: () => void;
}

export function useStopwatch(): Stopwatch {
  const [base, setBase] = useState(0);
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [laps, setLaps] = useState<number[]>([]);

  const elapsed = () => base + (startedAt === null ? 0 : Date.now() - startedAt);

  return {
    base,
    startedAt,
    running: startedAt !== null,
    laps,
    toggle: () => {
      if (startedAt === null) {
        setStartedAt(Date.now());
      } else {
        setBase(elapsed());
        setStartedAt(null);
      }
    },
    reset: () => {
      setBase(0);
      setStartedAt(null);
      setLaps([]);
    },
    lap: () => {
      if (startedAt === null && base === 0) return;
      setLaps((prev) => [...prev, elapsed()]);
    },
  };
}
