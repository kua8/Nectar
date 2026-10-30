import type { ReactNode } from "react";
import { Sparkles, SlidersHorizontal, PanelBottom, Settings2 } from "lucide-react";

interface Step {
  icon: ReactNode;
  title: string;
  text: string;
  action?: { label: string; id: "control-center" | "settings" };
}

const STEPS: Step[] = [
  {
    icon: <Sparkles size={18} />,
    title: "Welcome to Nectar",
    text: "Hover the notch to open it. Scroll over it to switch between music, status, calendar and the Control Center.",
  },
  {
    icon: <SlidersHorizontal size={18} />,
    title: "Control Center",
    text: "Wi-Fi, Bluetooth, volume and brightness live here. Click › on the Wi-Fi or Bluetooth tile to see networks and devices.",
    action: { label: "Open it", id: "control-center" },
  },
  {
    icon: <PanelBottom size={18} />,
    title: "The dock",
    text: "Drag icons to reorder, right-click for options, middle-click to open a new window. Add Task View, drives or the Recycle Bin in Settings > Dock.",
  },
  {
    icon: <Settings2 size={18} />,
    title: "Make it yours",
    text: "Themes, notch and dock modes and per-monitor options are in Settings. Prefer the classic look? Turn on Legacy Mode in Settings > General.",
    action: { label: "Open Settings", id: "settings" },
  },
];

interface Props {
  step: number;
  onStep: (next: number) => void;
  onClose: () => void;
  onAction: (id: "control-center" | "settings") => void;
}

export function NotchTour({ step, onStep, onClose, onAction }: Props) {
  const current = STEPS[step];
  const last = step === STEPS.length - 1;

  return (
    <div className="tour-content" onClick={(e) => e.stopPropagation()}>
      <div className="tour-head">
        <span className="tour-icon">{current.icon}</span>
        <span className="tour-title">{current.title}</span>
        <button className="tour-skip" onClick={onClose}>Skip</button>
      </div>
      <p className="tour-text">{current.text}</p>
      <div className="tour-foot">
        <span className="tour-dots">
          {STEPS.map((_, i) => (
            <i key={i} className={i === step ? "on" : ""} />
          ))}
        </span>
        {current.action && (
          <button className="tour-btn ghost" onClick={() => onAction(current.action!.id)}>
            {current.action.label}
          </button>
        )}
        {step > 0 && (
          <button className="tour-btn ghost" onClick={() => onStep(step - 1)}>Back</button>
        )}
        <button className="tour-btn" onClick={() => (last ? onClose() : onStep(step + 1))}>
          {last ? "Done" : "Next"}
        </button>
      </div>
    </div>
  );
}
