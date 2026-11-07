import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SwatchBook } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useT } from "../i18n";

const KEY = "nectar-sync-windows-accent";

export function WindowsAccentSync({ color }: { color: string }) {
  const t = useT();
  const [on, setOn] = useState(() => localStorage.getItem(KEY) === "true");
  const [error, setError] = useState("");
  const first = useRef(true);

  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    if (!on) return;
    const timer = setTimeout(() => invoke("sync_windows_accent", { hex: color }).catch(() => { }), 400);
    return () => clearTimeout(timer);
  }, [color]);

  const toggle = () => {
    const next = !on;
    setError("");
    setOn(next);
    localStorage.setItem(KEY, String(next));
    invoke("save_setting", { key: KEY, value: String(next) }).catch(() => { });
    (next ? invoke("sync_windows_accent", { hex: color }) : invoke("restore_windows_accent")).catch((e) => {
      setError(String(e));
      setOn(!next);
      localStorage.setItem(KEY, String(!next));
      invoke("save_setting", { key: KEY, value: String(!next) }).catch(() => { });
    });
  };

  return (
    <SettingRow icon={SwatchBook} label={t("appearance.syncWindowsAccent")} desc={error || t("appearance.syncWindowsAccent.desc")}>
      <label className="toggle-switch">
        <input type="checkbox" checked={on} onChange={toggle} />
        <span className="slider"></span>
      </label>
    </SettingRow>
  );
}
