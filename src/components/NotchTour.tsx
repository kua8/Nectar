import { useT } from "../i18n";

export interface TourStepDef {
  key: string;
  signal?: "panel" | "slider" | "cc" | "shelf" | "dock" | "mode";
  need?: number;
}

export const TOUR_STEPS: TourStepDef[] = [
  { key: "welcome" },
  { key: "panels", signal: "panel", need: 3 },
  { key: "sliders", signal: "slider", need: 2 },
  { key: "controlCenter", signal: "cc", need: 1 },
  { key: "shelf", signal: "shelf", need: 1 },
  { key: "dock", signal: "dock", need: 1 },
  { key: "modes", signal: "mode", need: 1 },
  { key: "customize" },
];

type ActionId = "control-center" | "settings";

interface Props {
  step: number;
  done: boolean;
  dockEnabled: boolean;
  onStep: (next: number) => void;
  onClose: () => void;
  onAction: (id: ActionId) => void;
}

const ACTIONS: Record<string, ActionId> = { controlCenter: "control-center", customize: "settings" };

export function NotchTour({ step, done, dockEnabled, onStep, onClose, onAction }: Props) {
  const t = useT();
  const current = TOUR_STEPS[step];
  const last = step === TOUR_STEPS.length - 1;
  const action = ACTIONS[current.key];
  const tryIt = !!current.signal && (current.key !== "dock" || dockEnabled);

  return (
    <div className="tour-bar" onClick={(e) => e.stopPropagation()}>
      <div className="tour-head">
        <span className="tour-count">{step + 1}/{TOUR_STEPS.length}</span>
        <span className="tour-title">{t(`tour.${current.key}.title`)}</span>
        <button className="tour-skip" onClick={onClose}>{t("tour.skip")}</button>
      </div>
      <p className="tour-text">{t(`tour.${current.key}.text`)}</p>
      <div className="tour-foot">
        <span className="tour-dots">
          {TOUR_STEPS.map((_, i) => (
            <i key={i} className={i === step ? "on" : ""} />
          ))}
        </span>
        {action && (
          <button className="tour-btn ghost" onClick={() => onAction(action)}>
            {t(`tour.${current.key}.action`)}
          </button>
        )}
        {step > 0 && (
          <button className="tour-btn ghost" onClick={() => onStep(step - 1)}>{t("common.back")}</button>
        )}
        <button
          className={`tour-btn${tryIt ? (done ? " ready" : " waiting") : ""}`}
          disabled={tryIt && !done}
          onClick={() => (last ? onClose() : onStep(step + 1))}
        >
          {last ? t("tour.done") : t("tour.next")}
        </button>
      </div>
    </div>
  );
}
