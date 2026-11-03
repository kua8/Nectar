// Which dock items want attention: windows that flashed their taskbar button (reported by the
// backend) and unread counts that apps put in their window titles.

export interface AttentionApp {
  name: string;
  executable?: string;
  hwnd?: number;
  all_hwnds?: [number, string][];
}

// "(3) Inbox - Gmail" and "(12+) WhatsApp" are common. "Telegram (5)" only counts when it is the
// app's own name, so a window for "notes (2).txt" or "Downloads (2)" doesn't get a badge.
export function unreadCount(title: string, names: string[]): number {
  const lead = title.match(/^\s*\((\d{1,5})\+?\)\s/);
  if (lead) return parseInt(lead[1], 10);
  const tail = title.match(/^\s*(.+?)\s+\((\d{1,5})\+?\)\s*$/);
  if (tail && names.some((n) => n && n.toLowerCase() === tail[1].toLowerCase())) return parseInt(tail[2], 10);
  return 0;
}

export function appAttention(app: AttentionApp, flashing: Set<number>) {
  const windows: [number, string][] = app.all_hwnds ?? (app.hwnd ? [[app.hwnd, ""]] : []);
  const names = [app.name.trim(), (app.executable || "").replace(/\.exe$/i, "")];
  let count = 0;
  let flash = false;
  for (const [hwnd, title] of windows) {
    if (flashing.has(hwnd)) flash = true;
    // Several windows of one app usually show the same total, so take the largest.
    count = Math.max(count, unreadCount(title || "", names));
  }
  return { flashing: flash, count };
}
