import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface CalEvent {
  uid: string;
  calendar: string;
  color: string | null;
  title: string;
  location: string | null;
  allDay: boolean;
  start: number;
  end: number;
}

export interface AccountView {
  calendars: number;
  events: number;
  syncedAt: number | null;
  error: string | null;
}

export interface LinkView {
  id: string;
  name: string;
  provider: string;
  events: number;
  syncedAt: number | null;
  error: string | null;
}

export interface CaldavState {
  connected: boolean;
  syncedAt: number | null;
  account: AccountView | null;
  links: LinkView[];
  calendars: { name: string; color: string | null }[];
  events: CalEvent[];
}

const EMPTY: CaldavState = { connected: false, syncedAt: null, account: null, links: [], calendars: [], events: [] };

export function useCaldav(): CaldavState {
  const [state, setState] = useState<CaldavState>(EMPTY);

  useEffect(() => {
    let alive = true;
    invoke<CaldavState>("caldav_get_cache")
      .then((s) => alive && setState(s))
      .catch(() => {});
    const unlisten = listen<CaldavState>("caldav-updated", (e) => setState(e.payload));
    return () => {
      alive = false;
      unlisten.then((fn) => fn());
    };
  }, []);

  return state;
}
