import { Volume2, Sun, ArrowLeftToLine, ArrowRightToLine } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { SettingSection } from "./SettingSection";
import { useT } from "../i18n";

interface OverlaysTabProps {
  volumeOverlayEnabled: boolean;
  toggleVolumeOverlay: () => void;
  volumeEdgeEnabled: boolean;
  toggleVolumeEdge: () => void;
  brightnessOverlayEnabled: boolean;
  toggleBrightnessOverlay: () => void;
  brightnessEdgeEnabled: boolean;
  toggleBrightnessEdge: () => void;
}

export function OverlaysTab({
  volumeOverlayEnabled,
  toggleVolumeOverlay,
  volumeEdgeEnabled,
  toggleVolumeEdge,
  brightnessOverlayEnabled,
  toggleBrightnessOverlay,
  brightnessEdgeEnabled,
  toggleBrightnessEdge,
}: OverlaysTabProps) {
  const t = useT();
  return (
    <>
      <SettingSection id="overlays-main" title={t("overlays.group")} defaultOpen>
        <SettingRow icon={Volume2} label={t("overlays.volume")} desc={t("overlays.volume.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={volumeOverlayEnabled} onChange={toggleVolumeOverlay} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {volumeOverlayEnabled && (
          <SettingRow icon={ArrowLeftToLine} label={t("overlays.edgeHover")} desc={t("overlays.edgeHover.left")}>
            <label className="toggle-switch">
              <input type="checkbox" checked={volumeEdgeEnabled} onChange={toggleVolumeEdge} />
              <span className="slider"></span>
            </label>
          </SettingRow>
        )}

        <SettingRow icon={Sun} label={t("overlays.brightness")} desc={t("overlays.brightness.desc")}>
          <label className="toggle-switch">
            <input type="checkbox" checked={brightnessOverlayEnabled} onChange={toggleBrightnessOverlay} />
            <span className="slider"></span>
          </label>
        </SettingRow>

        {brightnessOverlayEnabled && (
          <SettingRow icon={ArrowRightToLine} label={t("overlays.edgeHover")} desc={t("overlays.edgeHover.right")} divider={false}>
            <label className="toggle-switch">
              <input type="checkbox" checked={brightnessEdgeEnabled} onChange={toggleBrightnessEdge} />
              <span className="slider"></span>
            </label>
          </SettingRow>
        )}
      </SettingSection>
    </>
  );
}
