import { useEffect, useRef, useState } from "react";
import { Palette } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useT } from "../i18n";

const SEEN = "nectar-agent-intro-seen";

const COLORS = [
  { color: "#60a5fa", key: "working" },
  { color: "#ff9f43", key: "input" },
  { color: "#4ade80", key: "done" },
  { color: "#ff5f5f", key: "error" },
];

function seenBefore() {
  try {
    return localStorage.getItem(SEEN) === "true";
  } catch {
    return false;
  }
}

export function AgentIntro({ enabled }: { enabled: boolean }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const previous = useRef(enabled);

  useEffect(() => {
    if (enabled && !previous.current && !seenBefore()) setOpen(true);
    if (!enabled) setOpen(false);
    previous.current = enabled;
  }, [enabled]);

  const toggle = () => {
    setOpen(!open);
    try { localStorage.setItem(SEEN, "true"); } catch { }
  };

  if (!enabled) return null;
  return (
    <>
      <SettingRow icon={Palette} label={t("agent.intro.title")} desc={t("agent.intro.desc")} divider={!open}>
        <button className="agent-sound-btn" onClick={toggle}>{open ? t("agent.intro.hide") : t("agent.intro.show")}</button>
      </SettingRow>
      {open && (
        <div className="agent-legend">
          {COLORS.map(({ color, key }) => (
            <div className="agent-legend-item" key={key}>
              <span className="agent-legend-dot" style={{ background: color, boxShadow: `0 0 8px ${color}` }} />
              <div>
                <b>{t(`agent.intro.${key}.title`)}</b>
                <span>{t(`agent.intro.${key}.desc`)}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
