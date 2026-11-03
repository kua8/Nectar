import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Bell, CircleCheck, CircleAlert, Volume2, Play, FolderOpen, RotateCcw } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { playAgentSound, type SoundState } from "../agentSounds";
import { useT } from "../i18n";

const ROWS: { state: SoundState; icon: typeof Bell }[] = [
  { state: "needs_input", icon: Bell },
  { state: "done", icon: CircleCheck },
  { state: "error", icon: CircleAlert },
];

export function AgentSounds() {
  const t = useT();
  const [enabled, setEnabled] = useState(() => localStorage.getItem("nectar-agent-sounds") !== "false");
  const [custom, setCustom] = useState<string[]>([]);
  const refresh = () => invoke<string[]>("agent_sound_custom").then(setCustom).catch(() => { });

  useEffect(() => {
    refresh();
    const unlisten = listen("agent-sound-changed", refresh);
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const toggle = () => {
    const next = !enabled;
    setEnabled(next);
    localStorage.setItem("nectar-agent-sounds", String(next));
    invoke("save_setting", { key: "nectar-agent-sounds", value: String(next) }).catch(() => { });
  };

  return (
    <>
      <SettingRow icon={Volume2} label={t("notch.agentSounds")} desc={t("notch.agentSounds.desc")}>
        <label className="toggle-switch">
          <input type="checkbox" checked={enabled} onChange={toggle} />
          <span className="slider"></span>
        </label>
      </SettingRow>
      {enabled && ROWS.map(({ state, icon }, i) => (
        <SettingRow key={state} icon={icon} label={t(`agent.${state}`)} desc={custom.includes(state) ? t("notch.agentSound.custom") : t("notch.agentSound.default")} divider={i < ROWS.length - 1}>
          <div className="agent-sound-actions">
            <button className="agent-sound-btn" onClick={() => playAgentSound(state, true)}><Play size={12} />{t("notch.agentSound.play")}</button>
            <button className="agent-sound-btn" onClick={() => invoke("pick_agent_sound", { state }).catch(() => { })}><FolderOpen size={12} />{t("notch.agentSound.choose")}</button>
            {custom.includes(state) && <button className="agent-sound-btn" onClick={() => invoke("reset_agent_sound", { state })} title={t("notch.agentSound.reset")}><RotateCcw size={12} /></button>}
          </div>
        </SettingRow>
      ))}
    </>
  );
}
