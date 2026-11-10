import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MousePointer2 } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

export function EdgeArea({ settingKey }: { settingKey: "nectar-notch-edge-area" | "nectar-dock-edge-area" }) {
  const t = useT();
  const [value, setValue] = useState(() => (localStorage.getItem(settingKey) === "narrow" ? "narrow" : "full"));

  const change = (next: string) => {
    setValue(next);
    localStorage.setItem(settingKey, next);
    invoke("save_setting", { key: settingKey, value: next }).catch(() => { });
  };

  return (
    <SettingRow icon={MousePointer2} label={t("edgeArea.label")} desc={t(settingKey === "nectar-notch-edge-area" ? "edgeArea.notch.desc" : "edgeArea.dock.desc")}>
      <Dropdown
        value={value}
        onChange={change}
        options={[
          { value: "full", label: t("edgeArea.full") },
          { value: "narrow", label: t("edgeArea.narrow") },
        ]}
      />
    </SettingRow>
  );
}
