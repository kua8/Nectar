import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClipboardCopy, FolderOpen, Crosshair, Image as ImageIcon } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useT } from "../i18n";

export function DiagnosticsTab() {
  const t = useT();
  const [diagStatus, setDiagStatus] = useState<"idle" | "working" | "copied" | "error">("idle");
  const [hitboxLogging, setHitboxLogging] = useState(false);
  const [iconStatus, setIconStatus] = useState<"idle" | "working" | "done" | "error">("idle");

  useEffect(() => {
    invoke<boolean>("get_hitbox_logging").then(setHitboxLogging).catch(() => {});
  }, []);

  const copyDiagnostics = async () => {
    setDiagStatus("working");
    try {
      const report = await invoke<string>("get_diagnostics");
      await navigator.clipboard.writeText(report);
      setDiagStatus("copied");
    } catch {
      setDiagStatus("error");
    }
    setTimeout(() => setDiagStatus("idle"), 2500);
  };

  const refreshIconCache = async () => {
    setIconStatus("working");
    try {
      await invoke("clear_icon_cache");
      setIconStatus("done");
    } catch {
      setIconStatus("error");
    }
    setTimeout(() => setIconStatus("idle"), 2500);
  };

  const toggleHitboxLogging = () => {
    const next = !hitboxLogging;
    setHitboxLogging(next);
    invoke("set_hitbox_logging", { enabled: next }).catch(() => setHitboxLogging(!next));
  };

  const diagLabel =
    diagStatus === "working" ? t("about.diagnostics.working")
    : diagStatus === "copied" ? t("about.diagnostics.copied")
    : diagStatus === "error" ? t("about.diagnostics.error")
    : t("about.diagnostics.copy");

  const iconLabel =
    iconStatus === "working" ? t("about.icons.working")
    : iconStatus === "done" ? t("about.icons.done")
    : iconStatus === "error" ? t("about.icons.error")
    : t("about.icons");

  return (
    <div>
      <div className="setting-group-label">{t("about.diagnostics.group")}</div>
      <div className="setting-group">
        <SettingRow icon={ClipboardCopy} label={diagLabel} desc={t("about.diagnostics.desc")} action onClick={copyDiagnostics} />
        <SettingRow icon={FolderOpen} label={t("about.logs")} desc={t("about.logs.desc")} action onClick={() => invoke("open_log_folder").catch(() => {})} />
        <SettingRow icon={Crosshair} label={t("about.hitbox")} desc={t("about.hitbox.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={hitboxLogging} onChange={toggleHitboxLogging} />
            <span className="slider"></span>
          </label>
        </SettingRow>
        <SettingRow icon={ImageIcon} label={iconLabel} desc={t("about.icons.desc")} action divider={false} onClick={refreshIconCache} />
      </div>
    </div>
  );
}
