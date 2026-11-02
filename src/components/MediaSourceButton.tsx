import { invoke } from "@tauri-apps/api/core";

export interface MediaSession {
  id: string;
  name: string;
  title: string;
  is_playing: boolean;
  active: boolean;
}

export function MediaSourceButton({ sessions }: { sessions: MediaSession[] }) {
  if (sessions.length < 2) return null;
  const index = Math.max(0, sessions.findIndex((s) => s.active));
  const current = sessions[index];
  const next = sessions[(index + 1) % sessions.length];
  return (
    <button
      className="media-source-btn"
      title={`${current.name} (${index + 1}/${sessions.length}) → ${next.name}`}
      aria-label={`Switch audio source to ${next.name}`}
      onClick={(e) => {
        e.stopPropagation();
        invoke("select_media_session", { id: next.id }).catch(() => {});
      }}
    >
      {current.is_playing && <span className="media-source-dot" />}
      <span className="media-source-name">{current.name}</span>
    </button>
  );
}
