import { Download, RefreshCw, FileDown, Upload, RotateCcw } from "lucide-react";
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
}: AboutTabProps) {
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

      <div className="about-footer">
        <p>Made with ❤️ by kua8</p>
      </div>
    </div>
  );
}
