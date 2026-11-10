import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type SoundState = "needs_input" | "done" | "error";

const cache = new Map<SoundState, string>();

const STATES: SoundState[] = ["needs_input", "done", "error"];
const warm = () => STATES.forEach((state) => { sourceFor(state); });

listen("agent-sound-changed", () => { cache.clear(); warm(); }).catch(() => { });
setTimeout(warm, 0);

async function sourceFor(state: SoundState) {
  const cached = cache.get(state);
  if (cached) return cached;
  const custom = await invoke<string | null>("agent_sound_data", { state }).catch(() => null);
  const source = custom ?? `/sounds/agent-${state}.wav`;
  cache.set(state, source);
  return source;
}

export const soundKey = (state: SoundState) => `nectar-agent-sound-${state}`;

export function stateSoundOn(state: SoundState) {
  try {
    const value = localStorage.getItem(soundKey(state));
    return value === null ? state !== "done" : value === "true";
  } catch {
    return state !== "done";
  }
}

export async function playAgentSound(state: SoundState, force = false) {
  try {
    if (!force && (localStorage.getItem("nectar-agent-sounds") === "false" || !stateSoundOn(state))) return;
    const audio = new Audio(await sourceFor(state));
    audio.volume = 0.6;
    await audio.play();
  } catch {
  }
}
