import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Wifi } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

const MODES = ["speed", "status"];

export function NetWidgetMode() {
  const t = useT();
  const [value, setValue] = useState(() => {
    const saved = localStorage.getItem("nectar-net-widget-mode");
    return saved && MODES.includes(saved) ? saved : "speed";
  });

  const change = (next: string) => {
    setValue(next);
    localStorage.setItem("nectar-net-widget-mode", next);
    invoke("save_setting", { key: "nectar-net-widget-mode", value: next }).catch(() => { });
  };

  return (
    <SettingRow icon={Wifi} label={t("notch.netWidget")} desc={t("notch.netWidget.desc")}>
      <Dropdown
        value={value}
        onChange={change}
        options={MODES.map((m) => ({ value: m, label: t(`notch.netWidget.${m}`) }))}
      />
    </SettingRow>
  );
}
