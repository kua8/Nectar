import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Timer } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

const DELAYS = ["0", "100", "200", "300", "500", "800"];

export function PreviewDelay() {
  const t = useT();
  const [value, setValue] = useState(() => {
    const saved = localStorage.getItem("nectar-dock-preview-delay");
    return saved && DELAYS.includes(saved) ? saved : "300";
  });

  const change = (next: string) => {
    setValue(next);
    localStorage.setItem("nectar-dock-preview-delay", next);
    invoke("save_setting", { key: "nectar-dock-preview-delay", value: next }).catch(() => { });
  };

  return (
    <SettingRow icon={Timer} label={t("dock.previewDelay")} desc={t("dock.previewDelay.desc")}>
      <Dropdown
        value={value}
        onChange={change}
        options={DELAYS.map((ms) => ({ value: ms, label: ms === "0" ? t("overlays.edgeDelay.instant") : `${Number(ms) / 1000} s` }))}
      />
    </SettingRow>
  );
}
