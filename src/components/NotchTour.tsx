import type { ReactNode } from "react";
import { Sparkles, SlidersHorizontal, PanelBottom, Settings2 } from "lucide-react";
import { useT } from "../i18n";

interface Step {
  icon: ReactNode;
  key: string;
  action?: { id: "control-center" | "settings" };
}

const STEPS: Step[] = [
  {
    icon: <Sparkles size={18} />,
    key: "welcome",
  },
  {
    icon: <SlidersHorizontal size={18} />,
    key: "controlCenter",
    action: { id: "control-center" },
  },
  {
    icon: <PanelBottom size={18} />,
    key: "dock",
  },
  {
    icon: <Settings2 size={18} />,
    key: "customize",
    action: { id: "settings" },
  },
];

interface Props {
  step: number;
  onStep: (next: number) => void;
  onClose: () => void;
  onAction: (id: "control-center" | "settings") => void;
}

export function NotchTour({ step, onStep, onClose, onAction }: Props) {
  const t = useT();
  const current = STEPS[step];
  const last = step === STEPS.length - 1;

  return (
    <div className="tour-content" onClick={(e) => e.stopPropagation()}>
      <div className="tour-head">
        <span className="tour-icon">{current.icon}</span>
        <span className="tour-title">{t(`tour.${current.key}.title`)}</span>
        <button className="tour-skip" onClick={onClose}>{t("tour.skip")}</button>
      </div>
      <p className="tour-text">{t(`tour.${current.key}.text`)}</p>
      <div className="tour-foot">
        <span className="tour-dots">
          {STEPS.map((_, i) => (
            <i key={i} className={i === step ? "on" : ""} />
          ))}
        </span>
        {current.action && (
          <button className="tour-btn ghost" onClick={() => onAction(current.action!.id)}>
            {t(`tour.${current.key}.action`)}
          </button>
        )}
        {step > 0 && (
          <button className="tour-btn ghost" onClick={() => onStep(step - 1)}>{t("common.back")}</button>
        )}
        <button className="tour-btn" onClick={() => (last ? onClose() : onStep(step + 1))}>
          {last ? t("tour.done") : t("tour.next")}
        </button>
      </div>
    </div>
  );
}
