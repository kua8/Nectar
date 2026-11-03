import { motion } from "framer-motion";
import { useT } from "../i18n";

export type AgentState = "working" | "tool" | "needs_input" | "done" | "error";

export interface AgentActivity {
  state: AgentState;
  name?: string;
  project?: string;
  window?: number;
  detail?: string;
  ask?: { id: number; tool?: string; detail?: string };
  cancelled?: number;
}

const MASCOT: Record<AgentState, string> = {
  working: "working",
  tool: "working",
  needs_input: "needs-input",
  done: "done",
  error: "error",
};

export function useAgentLabel(activity: AgentActivity) {
  const t = useT();
  const { state, name, project } = activity;
  const status = t(`agent.${state}`);
  return `${name ?? t("agent.default")} · ${project ? `${status} · ${project}` : status}`;
}

export function AgentMascot({ activity }: { activity: AgentActivity }) {
  return (
    <motion.img
      className="agent-mascot"
      key={activity.state}
      src={`/mascot/${MASCOT[activity.state]}.svg`}
      title={useAgentLabel(activity)}
      alt=""
      draggable={false}
      initial={{ scale: 0.6, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      transition={{ type: "spring", stiffness: 520, damping: 22 }}
    />
  );
}

export function AgentDot({ activity }: { activity: AgentActivity }) {
  return <span className={`agent-dot ${activity.state}`} title={useAgentLabel(activity)} />;
}

export function AgentLeft({ activity, dot }: { activity: AgentActivity; dot: boolean }) {
  return (
    <>
      <AgentMascot activity={activity} />
      {dot && <AgentDot activity={activity} />}
    </>
  );
}

export function AgentPanel({ activity, onOpen, onDismiss, onAnswer }: { activity: AgentActivity; onOpen: () => void; onDismiss: () => void; onAnswer: (allow: boolean) => void }) {
  const t = useT();
  const { ask } = activity;
  return (
    <>
      <div className="agent-panel-head">
        <img className="agent-panel-mascot" src={`/mascot/${MASCOT[activity.state]}.svg`} alt="" draggable={false} />
        <div className="agent-panel-text">
          <b>{activity.name ?? t("agent.default")}</b>
          <span>{activity.project ? `${t(`agent.${activity.state}`)} · ${activity.project}` : t(`agent.${activity.state}`)}</span>
        </div>
        <span className={`agent-dot ${activity.state}`} />
      </div>
      {ask && (
        <div className="agent-ask">
          {ask.tool && <b>{ask.tool}</b>}
          {ask.detail && <code>{ask.detail}</code>}
        </div>
      )}
      {!ask && activity.detail && (
        <div className="agent-ask">
          <span className="agent-ask-text">{activity.detail}</span>
        </div>
      )}
      <div className="agent-panel-actions">
        {ask ? (
          <>
            <button className="agent-btn primary" onClick={() => onAnswer(true)}>{t("agent.allow")}</button>
            <button className="agent-btn" onClick={() => onAnswer(false)}>{t("agent.deny")}</button>
          </>
        ) : null}
        <button className={`agent-btn${ask ? "" : " primary"}`} onClick={onOpen} disabled={!activity.window}>{t("agent.open")}</button>
        {!ask && <button className="agent-btn" onClick={onDismiss}>{t("agent.dismiss")}</button>}
      </div>
    </>
  );
}
