import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { Compass, Download, FlaskConical, RefreshCw, FileDown, Upload, RotateCcw, Trash2, ClipboardCopy, FolderOpen, Crosshair, Image as ImageIcon } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useT } from "../i18n";

interface AboutTabProps {
  appVersion: string;
  autoUpdate: boolean;
  toggleAutoUpdate: () => void;
  prereleaseChannel: boolean;
  togglePrereleaseChannel: () => void;
  updateStatus: string;
  updateVersion: string;
  checkForUpdates: () => void;
  installUpdate: () => void;
  exportStatus: string;
  importStatus: string;
  handleExportSettings: () => void;
  handleImportSettings: () => void;
  resetStatus: string;
  handleResetSettings: () => void;
  uninstallStatus: string;
  uninstallError?: string;
  handleUninstallNectar: () => void;
}

export function AboutTab({
  appVersion,
  autoUpdate,
  toggleAutoUpdate,
  prereleaseChannel,
  togglePrereleaseChannel,
  updateStatus,
  updateVersion,
  checkForUpdates,
  installUpdate,
  exportStatus,
  importStatus,
  handleExportSettings,
  handleImportSettings,
  resetStatus,
  handleResetSettings,
  uninstallStatus,
  uninstallError,
  handleUninstallNectar,
}: AboutTabProps) {
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

  const getIconCacheLabel = () => {
    if (iconStatus === "working") return t("about.icons.working");
    if (iconStatus === "done") return t("about.icons.done");
    if (iconStatus === "error") return t("about.icons.error");
    return t("about.icons");
  };

  const toggleHitboxLogging = () => {
    const next = !hitboxLogging;
    setHitboxLogging(next);
    invoke("set_hitbox_logging", { enabled: next }).catch(() => setHitboxLogging(!next));
  };

  const getDiagLabel = () => {
    if (diagStatus === "working") return t("about.diagnostics.working");
    if (diagStatus === "copied") return t("about.diagnostics.copied");
    if (diagStatus === "error") return t("about.diagnostics.error");
    return t("about.diagnostics.copy");
  };

  const getUpdateLabel = () => {
    switch (updateStatus) {
      case "checking":
        return t("about.update.checking");
      case "available":
        return t("about.update.available", { version: updateVersion });
      case "uptodate":
        return t("about.update.uptodate");
      case "downloading":
        return t("about.update.downloading");
      case "installing":
        return t("about.update.installing");
      case "error":
        return t("about.update.error");
      default:
        return t("about.update.check");
    }
  };

  const getUpdateDesc = () =>
    updateStatus === "available" ? t("about.update.available.desc") : t("about.update.current", { version: appVersion });

  const getPillLabel = () => {
    switch (updateStatus) {
      case "checking":
        return t("about.update.checking");
      case "available":
        return t("about.pill.update", { version: updateVersion });
      case "uptodate":
        return t("about.pill.uptodate");
      case "downloading":
        return t("about.pill.downloading");
      case "installing":
        return t("about.update.installing");
      case "error":
        return t("about.pill.error");
      default:
        return t("about.pill.check");
    }
  };

  const pillBusy = ["checking", "downloading", "installing"].includes(updateStatus);

  const getExportLabel = () => {
    if (exportStatus === "exporting") return t("about.export.working");
    if (exportStatus === "success") return t("about.export.done");
    return t("about.export");
  };

  const getImportLabel = () => {
    if (importStatus === "importing") return t("about.import.working");
    if (importStatus === "success") return t("about.import.done");
    return t("about.import");
  };

  const getResetLabel = () => {
    if (resetStatus === "resetting") return t("about.reset.working");
    if (resetStatus === "confirm") return t("about.confirmAgain");
    return t("about.reset");
  };

  const getResetDesc = () =>
    resetStatus === "confirm" ? t("about.reset.confirm.desc") : t("about.reset.desc");

  const getUninstallLabel = () => {
    if (uninstallStatus === "uninstalling") return t("about.uninstall.working");
    if (uninstallStatus === "confirm") return t("about.confirmAgain");
    if (uninstallStatus === "error") return t("about.uninstall.error");
    return t("about.uninstall");
  };

  const getUninstallDesc = () => {
    if (uninstallStatus === "confirm") return t("about.uninstall.confirm.desc");
    if (uninstallStatus === "error") return uninstallError || t("about.uninstall.error.desc");
    return t("about.uninstall.desc");
  };


  return (
    <div className="about-tab-container">
      <div className="about-header">
        <img src="/nectar.png" className="about-logo" alt="Nectar Logo" />
        <h1 className="about-title">Nectar</h1>
        <p className="about-version">{t("about.version", { version: appVersion || "—" })}</p>
        <button
          className={`update-pill update-pill--${updateStatus || "idle"}`}
          disabled={pillBusy}
          onClick={() => (updateStatus === "available" ? installUpdate() : checkForUpdates())}
        >
          {getPillLabel()}
        </button>
      </div>

      <div className="setting-group-label">{t("about.updates.group")}</div>
      <div className="setting-group">
        <SettingRow icon={Download} label={t("about.autoUpdate")} desc={t("about.autoUpdate.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={autoUpdate} onChange={toggleAutoUpdate} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow icon={FlaskConical} label={t("about.prerelease")} desc={t("about.prerelease.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={prereleaseChannel} onChange={togglePrereleaseChannel} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        <SettingRow
          icon={RefreshCw}
          label={getUpdateLabel()}
          desc={getUpdateDesc()}
          action
          divider={false}
          onClick={() => (updateStatus === "available" ? installUpdate() : checkForUpdates())}
        />
      </div>

      <div className="setting-group-label setting-group-label--spaced">
        {t("about.data.group")}
      </div>
      <div className="setting-group">
        <SettingRow icon={Compass} label={t("about.tour")} desc={t("about.tour.desc")} action onClick={() => emit("start-tour")} />
        <SettingRow icon={FileDown} label={getExportLabel()} desc={t("about.export.desc")} action onClick={handleExportSettings} />
        <SettingRow icon={Upload} label={getImportLabel()} desc={t("about.import.desc")} action onClick={handleImportSettings} />
        <SettingRow
          icon={ImageIcon}
          label={getIconCacheLabel()}
          desc={t("about.icons.desc")}
          action
          onClick={refreshIconCache}
        />
        <SettingRow
          icon={RotateCcw}
          label={getResetLabel()}
          desc={getResetDesc()}
          action
          divider={false}
          danger={resetStatus === "confirm"}
          onClick={handleResetSettings}
        />
      </div>

      <div className="setting-group-label setting-group-label--spaced">
        {t("about.diagnostics.group")}
      </div>
      <div className="setting-group">
        <SettingRow
          icon={ClipboardCopy}
          label={getDiagLabel()}
          desc={t("about.diagnostics.desc")}
          action
          onClick={copyDiagnostics}
        />
        <SettingRow icon={FolderOpen} label={t("about.logs")} desc={t("about.logs.desc")} action onClick={() => invoke("open_log_folder").catch(() => {})} />
        <SettingRow
          icon={Crosshair}
          label={t("about.hitbox")}
          desc={t("about.hitbox.desc")}
          divider={false}
        >
          <label className="toggle-switch">
            <input type="checkbox" checked={hitboxLogging} onChange={toggleHitboxLogging} />
            <span className="slider"></span>
          </label>
        </SettingRow>
      </div>

      <div className="setting-group-label setting-group-label--spaced">
        {t("about.danger.group")}
      </div>
      <div className="setting-group">
        <SettingRow
          icon={Trash2}
          label={getUninstallLabel()}
          desc={getUninstallDesc()}
          action
          divider={false}
          danger={uninstallStatus === "confirm" || uninstallStatus === "error"}
          onClick={handleUninstallNectar}
        />
      </div>

      <div className="about-footer">
        <p>{t("about.footer")}</p>
      </div>
    </div>
  );
}
