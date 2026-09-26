import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, RefreshCw, FileDown, Upload, RotateCcw, Trash2, ClipboardCopy, FolderOpen, Crosshair } from "lucide-react";
import { SettingRow } from "./SettingRow";

interface AboutTabProps {
  appVersion: string;
  autoUpdate: boolean;
  toggleAutoUpdate: () => void;
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
  const [diagStatus, setDiagStatus] = useState<"idle" | "working" | "copied" | "error">("idle");
  const [hitboxLogging, setHitboxLogging] = useState(false);

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

  const toggleHitboxLogging = () => {
    const next = !hitboxLogging;
    setHitboxLogging(next);
    invoke("set_hitbox_logging", { enabled: next }).catch(() => setHitboxLogging(!next));
  };

  const getDiagLabel = () => {
    if (diagStatus === "working") return "Collecting...";
    if (diagStatus === "copied") return "Copied to clipboard!";
    if (diagStatus === "error") return "Couldn't copy — open the log folder instead";
    return "Copy Diagnostics";
  };

  const getUpdateLabel = () => {
    switch (updateStatus) {
      case "checking":
        return "Checking...";
      case "available":
        return `Update Available (v${updateVersion})`;
      case "uptodate":
        return "Nectar is up to date";
      case "downloading":
        return "Downloading Update...";
      case "installing":
        return "Installing...";
      case "error":
        return "No updates found";
      default:
        return "Check for Updates";
    }
  };

  const getUpdateDesc = () =>
    updateStatus === "available" ? "Click to install and restart" : `Currently running v${appVersion}`;

  const getPillLabel = () => {
    switch (updateStatus) {
      case "checking":
        return "Checking...";
      case "available":
        return `Update to v${updateVersion}`;
      case "uptodate":
        return "Up to date";
      case "downloading":
        return "Downloading...";
      case "installing":
        return "Installing...";
      case "error":
        return "Retry check";
      default:
        return "Check for updates";
    }
  };

  const pillBusy = ["checking", "downloading", "installing"].includes(updateStatus);

  const getExportLabel = () => {
    if (exportStatus === "exporting") return "Exporting...";
    if (exportStatus === "success") return "Exported!";
    return "Export Settings";
  };

  const getImportLabel = () => {
    if (importStatus === "importing") return "Importing...";
    if (importStatus === "success") return "Imported!";
    return "Import Settings";
  };

  const getResetLabel = () => {
    if (resetStatus === "resetting") return "Resetting...";
    if (resetStatus === "confirm") return "Click Again to Confirm";
    return "Reset to Defaults";
  };

  const getResetDesc = () =>
    resetStatus === "confirm" ? "This will erase all settings — cannot be undone" : "Restore all settings to defaults";

  const getUninstallLabel = () => {
    if (uninstallStatus === "uninstalling") return "Uninstalling...";
    if (uninstallStatus === "confirm") return "Click Again to Confirm";
    if (uninstallStatus === "error") return "Couldn't Start Uninstaller";
    return "Uninstall Nectar";
  };

  const getUninstallDesc = () => {
    if (uninstallStatus === "confirm") return "This will remove Nectar from your computer";
    if (uninstallStatus === "error") return uninstallError || "Try again, or use Windows Settings > Apps";
    return "Doesn't require Windows Settings to be open";
  };


  return (
    <div className="about-tab-container">
      <div className="about-header">
        <img src="/nectar.png" className="about-logo" alt="Nectar Logo" />
        <h1 className="about-title">Nectar</h1>
        <p className="about-version">Version {appVersion || "—"}</p>
        <button
          className={`update-pill update-pill--${updateStatus || "idle"}`}
          disabled={pillBusy}
          onClick={() => (updateStatus === "available" ? installUpdate() : checkForUpdates())}
        >
          {getPillLabel()}
        </button>
      </div>

      <div className="setting-group-label">Software Updates</div>
      <div className="setting-group">
        <SettingRow icon={Download} label="Auto Update" desc="Update automatically on startup">
          <label className="toggle-switch">
            <input type="checkbox" checked={autoUpdate} onChange={toggleAutoUpdate} />
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
        Data
      </div>
      <div className="setting-group">
        <SettingRow icon={FileDown} label={getExportLabel()} desc="Save settings to a file" action onClick={handleExportSettings} />
        <SettingRow icon={Upload} label={getImportLabel()} desc="Load settings from a file" action onClick={handleImportSettings} />
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
        Diagnostics
      </div>
      <div className="setting-group">
        <SettingRow
          icon={ClipboardCopy}
          label={getDiagLabel()}
          desc="System info and recent log, to paste into a bug report"
          action
          onClick={copyDiagnostics}
        />
        <SettingRow icon={FolderOpen} label="Open Log Folder" desc="nectar.log and diagnostics.txt" action onClick={() => invoke("open_log_folder").catch(() => {})} />
        <SettingRow
          icon={Crosshair}
          label="Log Dock and Notch Clicks"
          desc="Turn on, move over the dock and notch for a few seconds, then copy diagnostics. Resets on restart"
          divider={false}
        >
          <label className="toggle-switch">
            <input type="checkbox" checked={hitboxLogging} onChange={toggleHitboxLogging} />
            <span className="slider"></span>
          </label>
        </SettingRow>
      </div>

      <div className="setting-group-label setting-group-label--spaced">
        Danger Zone
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
        <p>Made with ❤️ by kua8</p>
      </div>
    </div>
  );
}
