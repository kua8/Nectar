import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeftRight } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useT } from "../i18n";

type Side = "left" | "right";

const read = (key: string, fallback: Side): Side => {
  const saved = localStorage.getItem(key);
  return saved === "left" || saved === "right" ? saved : fallback;
};

const save = (key: string, value: Side) => {
  localStorage.setItem(key, value);
  invoke("save_setting", { key, value }).catch(() => { });
};

export function EdgeSides() {
  const t = useT();
  const [volume, setVolume] = useState<Side>(() => read("nectar-volume-side", "left"));
  const [brightness, setBrightness] = useState<Side>(() => read("nectar-brightness-side", "right"));
  const opposite = (side: Side): Side => (side === "left" ? "right" : "left");
  const options = [
    { value: "left", label: t("overlays.side.left") },
    { value: "right", label: t("overlays.side.right") },
  ];

  const changeVolume = (value: string) => {
    const next = value as Side;
    setVolume(next);
    save("nectar-volume-side", next);
    if (brightness === next) {
      setBrightness(opposite(next));
      save("nectar-brightness-side", opposite(next));
    }
  };

  const changeBrightness = (value: string) => {
    const next = value as Side;
    setBrightness(next);
    save("nectar-brightness-side", next);
    if (volume === next) {
      setVolume(opposite(next));
      save("nectar-volume-side", opposite(next));
    }
  };

  return (
    <>
      <SettingRow icon={ArrowLeftRight} label={t("overlays.volumeSide")} desc={t("overlays.side.desc")}>
        <Dropdown value={volume} onChange={changeVolume} options={options} />
      </SettingRow>
      <SettingRow icon={ArrowLeftRight} label={t("overlays.brightnessSide")} desc={t("overlays.side.desc")}>
        <Dropdown value={brightness} onChange={changeBrightness} options={options} />
      </SettingRow>
    </>
  );
}
