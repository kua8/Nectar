import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Timer } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

const DELAYS = ["0", "150", "300", "600", "1000"];

export function EdgeDelay() {
  const t = useT();
  const [value, setValue] = useState(() => {
    const saved = localStorage.getItem("nectar-edge-delay");
    return saved && DELAYS.includes(saved) ? saved : "300";
  });

  const change = (next: string) => {
    setValue(next);
    localStorage.setItem("nectar-edge-delay", next);
    invoke("save_setting", { key: "nectar-edge-delay", value: next }).catch(() => { });
  };

  return (
    <SettingRow icon={Timer} label={t("overlays.edgeDelay")} desc={t("overlays.edgeDelay.desc")} divider={false}>
      <Dropdown
        value={value}
        onChange={change}
        options={DELAYS.map((ms) => ({ value: ms, label: ms === "0" ? t("overlays.edgeDelay.instant") : `${Number(ms) / 1000} s` }))}
      />
    </SettingRow>
  );
}
