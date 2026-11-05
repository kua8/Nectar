import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Keyboard } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { ShortcutRecorder } from "./ShortcutRecorder";
import { useT } from "../i18n";

export function NotchHotkey() {
  const t = useT();
  const [value, setValue] = useState(() => localStorage.getItem("nectar-notch-hotkey") || "");

  const commit = (next: string) => {
    setValue(next);
    localStorage.setItem("nectar-notch-hotkey", next);
    invoke("save_setting", { key: "nectar-notch-hotkey", value: next }).catch(() => { });
  };

  return (
    <SettingRow icon={Keyboard} label={t("notch.hotkey")} desc={t("notch.hotkey.desc")}>
      <ShortcutRecorder value={value} onCommit={commit} />
    </SettingRow>
  );
}
