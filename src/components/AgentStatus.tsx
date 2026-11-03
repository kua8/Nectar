import { motion } from "framer-motion";
import { useT } from "../i18n";

export type AgentState = "working" | "needs_input" | "done" | "error";

export interface AgentActivity {
  state: AgentState;
  name?: string;
  project?: string;
}

const MASCOT: Record<AgentState, string> = {
  working: "working",
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
