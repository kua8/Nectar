const MAX_SECONDS = 99 * 3600;

export function formatStopwatch(ms: number, precise: boolean): string {
  const total = Math.floor(ms / 1000);
  const hours = Math.floor(total / 3600);
  const mins = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const base = hours > 0 ? `${hours}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}` : `${mins}:${String(secs).padStart(2, "0")}`;
  if (!precise) return base;
  return `${base}.${String(Math.floor((ms % 1000) / 10)).padStart(2, "0")}`;
}

export function parseDuration(text: string): number | null {
  const value = text.trim();
  let seconds: number;
  if (/^\d+$/.test(value)) {
    seconds = Number(value) * 60;
  } else if (/^\d+:\d{1,2}$/.test(value)) {
    const [m, s] = value.split(":").map(Number);
    if (s > 59) return null;
    seconds = m * 60 + s;
  } else if (/^\d+:\d{1,2}:\d{1,2}$/.test(value)) {
    const [h, m, s] = value.split(":").map(Number);
    if (m > 59 || s > 59) return null;
    seconds = h * 3600 + m * 60 + s;
  } else {
    return null;
  }
  return seconds > 0 && seconds <= MAX_SECONDS ? seconds : null;
}
