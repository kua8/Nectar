import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Timer } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

const DELAYS = [
  { value: "800", label: "0.8 s" },
  { value: "2000", label: "2 s" },
  { value: "5000", label: "5 s" },
  { value: "15000", label: "15 s" },
  { value: "60000", label: "1 min" },
  { value: "300000", label: "5 min" },
  { value: "900000", label: "15 min" },
  { value: "3600000", label: "1 h" },
  { value: "10800000", label: "3 h" },
];

export function HideDelay() {
  const t = useT();
  const [value, setValue] = useState(() => {
    const saved = localStorage.getItem("nectar-dock-hide-delay");
    return saved && DELAYS.some((d) => d.value === saved) ? saved : "800";
  });

  const change = (next: string) => {
    setValue(next);
    localStorage.setItem("nectar-dock-hide-delay", next);
    invoke("save_setting", { key: "nectar-dock-hide-delay", value: next }).catch(() => { });
  };

  return (
    <SettingRow icon={Timer} label={t("dock.hideDelay")} desc={t("dock.hideDelay.desc")}>
      <Dropdown value={value} onChange={change} options={DELAYS} />
    </SettingRow>
  );
}
