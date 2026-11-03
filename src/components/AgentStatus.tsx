import { motion } from "framer-motion";
import { SurfaceShape } from "./SurfaceShape";
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

export function AgentStatus({ activity }: { activity: AgentActivity }) {
  const t = useT();
  const { state, name, project } = activity;
  return (
    <motion.div
      className={`agent-status ${state}`}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0, transition: { duration: 0.15 } }}
    >
      <motion.img
        className="agent-mascot"
        key={state}
        src={`/mascot/${MASCOT[state]}-sit.svg`}
        alt=""
        draggable={false}
        initial={{ y: -26, scale: 0.8, opacity: 0 }}
        animate={{ y: 0, scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 520, damping: 20 }}
      />
      <motion.div
        className="agent-card"
        initial={{ y: -8, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ type: "spring", stiffness: 420, damping: 32 }}
      >
        <SurfaceShape edge="top" flare={0} radius={14} edgeRadius={14} />
        <span className="agent-dot" />
        <span className="agent-text">
          <b>{name ?? t("agent.default")}</b>
          <span>{project ? `${t(`agent.${state}`)} · ${project}` : t(`agent.${state}`)}</span>
        </span>
      </motion.div>
    </motion.div>
  );
}
