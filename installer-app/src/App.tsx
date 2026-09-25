import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Minus, X, FolderOpen, CheckCircle2, AlertTriangle, MessageSquareText } from "lucide-react";
import {
  type InitialState,
  type ProgressPayload,
  getInitialState,
  pickInstallDir,
  needsElevation,
  relaunchElevated,
  startInstall,
  launchApp,
  openInstallFolder,
  startUninstall,
  openUrl,
  minimizeWindow,
  closeWindow,
  onInstallProgress,
  onUninstallProgress,
} from "./lib/api";

type Screen =
  | "loading"
  | "welcome"
  | "options"
  | "installing"
  | "finish"
  | "uninstall-confirm"
  | "uninstalling"
  | "uninstall-done"
  | "error";

const FEEDBACK_REASONS = [
  "Found a bug",
  "Missing a feature I need",
  "Switching to something else",
  "Other",
];

const screenVariants = {
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  exit: { opacity: 0, y: -8 },
};

function TitleBar({ title }: { title: string }) {
  return (
    <div className="title-bar" data-tauri-drag-region>
      <span className="title-text" data-tauri-drag-region>{title}</span>
      <div className="title-bar-buttons">
        <button className="chrome-btn" onClick={() => minimizeWindow()} title="Minimize">
          <Minus size={13} strokeWidth={1.75} />
        </button>
        <button className="chrome-btn chrome-btn-close" onClick={() => closeWindow()} title="Close">
          <X size={13} strokeWidth={1.75} />
        </button>
      </div>
    </div>
  );
}

function Background() {
  return (
    <div className="bg-blobs" aria-hidden>
      <div className="blob blob-a" />
    </div>
  );
}

function Logo({ size = 64, idle = false }: { size?: number; idle?: boolean }) {
  return (
    <motion.div
      className={`logo-glow ${idle ? "logo-idle" : ""}`}
      style={{ width: size, height: size }}
      initial={{ opacity: 0, scale: 0.7 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ type: "spring", stiffness: 220, damping: 18 }}
    >
      <img src="/nectar.svg" alt="Nectar" width={size * 0.55} height={size * 0.55} />
    </motion.div>
  );
}

function ProgressBar({ percent }: { percent: number }) {
  return (
    <div className="progress-track">
      <motion.div
        className="progress-fill"
        animate={{ width: `${Math.round(percent * 100)}%` }}
        transition={{ type: "spring", stiffness: 120, damping: 20 }}
      >
        <div className="progress-shimmer" />
      </motion.div>
    </div>
  );
}

function PrimaryButton(props: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const { className = "", ...rest } = props;
  return (
    <motion.button
      whileHover={{ scale: 1.025 }}
      whileTap={{ scale: 0.97 }}
      className={`btn btn-primary ${className}`}
      {...(rest as any)}
    />
  );
}

function SecondaryButton(props: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const { className = "", ...rest } = props;
  return (
    <motion.button
      whileHover={{ scale: 1.025 }}
      whileTap={{ scale: 0.97 }}
      className={`btn btn-secondary ${className}`}
      {...(rest as any)}
    />
  );
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <label className="toggle-row">
      <span>{label}</span>
      <span
        className={`toggle-track ${checked ? "on" : ""}`}
        role="switch"
        aria-checked={checked}
        tabIndex={0}
        onClick={() => onChange(!checked)}
        onKeyDown={(e) => {
          if (e.key === " " || e.key === "Enter") {
            e.preventDefault();
            onChange(!checked);
          }
        }}
      >
        <motion.span
          className="toggle-thumb"
          animate={{ x: checked ? 16 : 0 }}
          transition={{ type: "spring", stiffness: 500, damping: 32 }}
        />
      </span>
    </label>
  );
}

function buildFeedbackUrl(reason: string, comment: string, appVersion: string): string {
  const title = `Uninstall feedback: ${reason}`;
  const bodyLines = [
    `**Reason:** ${reason}`,
    "",
    comment.trim() ? comment.trim() : "_(no additional comments)_",
    "",
    `---`,
    `Nectar version: ${appVersion || "unknown"}`,
  ];
  const params = new URLSearchParams({ title, body: bodyLines.join("\n") });
  return `https://github.com/kua8/Nectar/issues/new?${params.toString()}`;
}

export default function App() {
  const [screen, setScreen] = useState<Screen>("loading");
  const [state, setState] = useState<InitialState | null>(null);
  const [installDir, setInstallDir] = useState("");
  const [allUsers, setAllUsers] = useState(false);
  const [desktopShortcut, setDesktopShortcut] = useState(true);
  const [dirTouched, setDirTouched] = useState(false);
  const [progress, setProgress] = useState<ProgressPayload>({ stage: "", percent: 0, message: "" });
  const [exePath, setExePath] = useState("");
  const [launchNow, setLaunchNow] = useState(true);
  const [errorMsg, setErrorMsg] = useState("");
  const [feedbackReason, setFeedbackReason] = useState<string | null>(null);
  const [feedbackComment, setFeedbackComment] = useState("");
  const [feedbackSent, setFeedbackSent] = useState(false);
  const autoStarted = useRef(false);

  useEffect(() => {
    getInitialState().then((s) => {
      setState(s);
      if (s.uninstallMode) {
        if (s.autoUninstall) {
          handleConfirmUninstall();
        } else {
          setScreen("uninstall-confirm");
        }
        return;
      }
      const dir = s.prefillInstallDir ?? s.defaultInstallDirUser;
      setInstallDir(dir);
      setAllUsers(s.prefillAllUsers);
      setDesktopShortcut(s.prefillDesktopShortcut);
      if (s.autoInstall && !autoStarted.current) {
        autoStarted.current = true;
        setScreen("installing");
        runInstall(dir, s.prefillAllUsers, s.prefillDesktopShortcut);
      } else {
        setScreen("welcome");
      }
    }).catch((e) => {
      setErrorMsg(String(e));
      setScreen("error");
    });
  }, []);

  const handleToggleAllUsers = (next: boolean) => {
    setAllUsers(next);
    if (!dirTouched && state) {
      setInstallDir(next ? state.defaultInstallDirAllUsers : state.defaultInstallDirUser);
    }
  };

  const handleBrowse = async () => {
    const picked = await pickInstallDir(installDir).catch(() => null);
    if (picked) {
      setInstallDir(picked);
      setDirTouched(true);
    }
  };

  const runInstall = async (dir: string, forAllUsers: boolean, withDesktopShortcut: boolean) => {
    const unlisten = await onInstallProgress((p) => setProgress(p));
    try {
      const exe = await startInstall(dir, forAllUsers, withDesktopShortcut);
      setExePath(exe);
      setScreen("finish");
    } catch (err) {
      setErrorMsg(String(err));
      setScreen("error");
    } finally {
      unlisten();
    }
  };

  const handleInstallClick = async () => {
    if (allUsers) {
      const elevationNeeded = await needsElevation(true).catch(() => false);
      if (elevationNeeded) {
        try {
          await relaunchElevated(installDir, allUsers, desktopShortcut);
        } catch (err) {
          setErrorMsg("Admin permission was not granted, so an all-users install can't continue.");
          setScreen("error");
        }
        return;
      }
    }
    setScreen("installing");
    setProgress({ stage: "starting", percent: 0, message: "Starting..." });
    runInstall(installDir, allUsers, desktopShortcut);
  };

  const handleFinish = async () => {
    if (launchNow && exePath) {
      await launchApp(exePath).catch(() => {});
    }
    closeWindow();
  };

  const handleConfirmUninstall = async () => {
    setScreen("uninstalling");
    const unlisten = await onUninstallProgress((p) => setProgress(p));
    try {
      await startUninstall();
      setScreen("uninstall-done");
    } catch (err) {
      setErrorMsg(String(err));
      setScreen("error");
    } finally {
      unlisten();
    }
  };

  const handleSendFeedback = async () => {
    if (!feedbackReason) return;
    const url = buildFeedbackUrl(feedbackReason, feedbackComment, state?.appVersion ?? "");
    await openUrl(url).catch(() => {});
    setFeedbackSent(true);
  };

  return (
    <div className="installer-window">
      <Background />
      <TitleBar title={screen.startsWith("uninstall") ? "Uninstall Nectar" : "Nectar Setup"} />
      <div className="content">
        <AnimatePresence mode="wait">
          {screen === "loading" && (
            <motion.div key="loading" className="centered muted" {...screenVariants}>
              Loading...
            </motion.div>
          )}

          {screen === "welcome" && (
            <motion.div key="welcome" className="screen welcome-screen" {...screenVariants}>
              <Logo size={92} idle />
              <h1>Welcome to Nectar</h1>
              <p className="muted">
                This will install Nectar {state?.appVersion ? `v${state.appVersion}` : ""} on your
                computer.
              </p>
              <div className="spacer" />
              <div className="actions">
                <SecondaryButton onClick={() => closeWindow()}>Cancel</SecondaryButton>
                <PrimaryButton onClick={() => setScreen("options")}>Continue</PrimaryButton>
              </div>
            </motion.div>
          )}

          {screen === "options" && (
            <motion.div key="options" className="screen" {...screenVariants}>
              <h2>Install options</h2>

              <label className="field-label">Install location</label>
              <div className="path-row">
                <input
                  className="path-input"
                  value={installDir}
                  onChange={(e) => { setInstallDir(e.target.value); setDirTouched(true); }}
                />
                <button className="btn btn-icon" onClick={handleBrowse} title="Browse">
                  <FolderOpen size={16} strokeWidth={1.75} />
                </button>
              </div>

              <div className="radio-group">
                <label className={`radio-option ${!allUsers ? "selected" : ""}`}>
                  <input type="radio" checked={!allUsers} onChange={() => handleToggleAllUsers(false)} />
                  <div>
                    <div className="radio-title">Just me</div>
                    <div className="radio-sub muted">Installs for your account only, no admin needed</div>
                  </div>
                </label>
                <label className={`radio-option ${allUsers ? "selected" : ""}`}>
                  <input type="radio" checked={allUsers} onChange={() => handleToggleAllUsers(true)} />
                  <div>
                    <div className="radio-title">All users</div>
                    <div className="radio-sub muted">Available to everyone on this PC, requires admin</div>
                  </div>
                </label>
              </div>

              <Toggle
                checked={desktopShortcut}
                onChange={setDesktopShortcut}
                label="Create a desktop shortcut"
              />

              <div className="spacer" />
              <div className="actions">
                <SecondaryButton onClick={() => setScreen("welcome")}>Back</SecondaryButton>
                <PrimaryButton onClick={handleInstallClick}>Install</PrimaryButton>
              </div>
            </motion.div>
          )}

          {screen === "installing" && (
            <motion.div key="installing" className="screen centered-screen" {...screenVariants}>
              <Logo size={64} />
              <div className="spacer-sm" />
              <ProgressBar percent={progress.percent} />
              <p className="muted progress-message">{progress.message || "Installing..."}</p>
            </motion.div>
          )}

          {screen === "finish" && (
            <motion.div key="finish" className="screen welcome-screen" {...screenVariants}>
              <motion.div
                className="success-icon"
                initial={{ scale: 0.5, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ type: "spring", stiffness: 260, damping: 16 }}
              >
                <CheckCircle2 size={56} strokeWidth={1.5} />
              </motion.div>
              <h1>Nectar is installed</h1>
              <p className="muted">Setup finished successfully.</p>
              <Toggle checked={launchNow} onChange={setLaunchNow} label="Launch Nectar now" />
              <div className="spacer" />
              <div className="actions">
                <SecondaryButton onClick={() => openInstallFolder(installDir)}>
                  Open folder
                </SecondaryButton>
                <PrimaryButton onClick={handleFinish}>
                  {launchNow ? "Launch Nectar" : "Finish"}
                </PrimaryButton>
              </div>
            </motion.div>
          )}

          {screen === "uninstall-confirm" && (
            <motion.div key="uninstall-confirm" className="screen welcome-screen" {...screenVariants}>
              <Logo size={92} idle />
              <h1>Uninstall Nectar</h1>
              <p className="muted">This will remove Nectar and its shortcuts from this computer.</p>
              <div className="spacer" />
              <div className="actions">
                <SecondaryButton onClick={() => closeWindow()}>Cancel</SecondaryButton>
                <PrimaryButton onClick={handleConfirmUninstall}>Uninstall</PrimaryButton>
              </div>
            </motion.div>
          )}

          {screen === "uninstalling" && (
            <motion.div key="uninstalling" className="screen centered-screen" {...screenVariants}>
              <Logo size={64} />
              <div className="spacer-sm" />
              <ProgressBar percent={progress.percent} />
              <p className="muted progress-message">{progress.message || "Uninstalling..."}</p>
            </motion.div>
          )}

          {screen === "uninstall-done" && (
            <motion.div key="uninstall-done" className="screen feedback-screen" {...screenVariants}>
              <motion.div
                className="success-icon"
                initial={{ scale: 0.5, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ type: "spring", stiffness: 260, damping: 16 }}
              >
                <CheckCircle2 size={48} strokeWidth={1.5} />
              </motion.div>
              <h1>Nectar has been uninstalled</h1>

              {!feedbackSent ? (
                <>
                  <p className="muted feedback-prompt">
                    <MessageSquareText size={14} strokeWidth={1.75} /> Mind telling us why? Totally optional.
                  </p>
                  <div className="reason-chips">
                    {FEEDBACK_REASONS.map((r) => (
                      <button
                        key={r}
                        className={`chip ${feedbackReason === r ? "chip-selected" : ""}`}
                        onClick={() => setFeedbackReason(r)}
                      >
                        {r}
                      </button>
                    ))}
                  </div>
                  <div className={`feedback-textarea-reveal ${feedbackReason ? "open" : ""}`}>
                    <div className="feedback-textarea-wrap">
                      <textarea
                        className="feedback-textarea"
                        placeholder="Anything else you'd like to add? (optional)"
                        value={feedbackComment}
                        onChange={(e) => setFeedbackComment(e.target.value)}
                      />
                    </div>
                  </div>
                  <p className="muted feedback-note">
                    Clicking "Send feedback" opens a pre-filled GitHub issue in your browser — you'll see
                    exactly what's submitted before it sends. Nothing is sent automatically.
                  </p>
                  <div className="spacer" />
                  <div className="actions">
                    <SecondaryButton onClick={() => closeWindow()}>Skip</SecondaryButton>
                    <PrimaryButton disabled={!feedbackReason} onClick={handleSendFeedback}>
                      Send feedback
                    </PrimaryButton>
                  </div>
                </>
              ) : (
                <>
                  <p className="muted">Thanks — your browser should have opened to finish sending it.</p>
                  <div className="spacer" />
                  <div className="actions">
                    <PrimaryButton onClick={() => closeWindow()}>Close</PrimaryButton>
                  </div>
                </>
              )}
            </motion.div>
          )}

          {screen === "error" && (
            <motion.div key="error" className="screen welcome-screen" {...screenVariants}>
              <div className="error-icon"><AlertTriangle size={56} strokeWidth={1.5} /></div>
              <h1>Something went wrong</h1>
              <p className="muted">{errorMsg}</p>
              <div className="spacer" />
              <div className="actions">
                <PrimaryButton onClick={() => closeWindow()}>Close</PrimaryButton>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
