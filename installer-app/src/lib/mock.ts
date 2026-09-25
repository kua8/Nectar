import type { InitialState, ProgressPayload } from "./api";

export const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const previewUninstallMode =
  typeof window !== "undefined" && new URLSearchParams(window.location.search).get("uninstall") === "1";

export const mockInitialState: InitialState = {
  uninstallMode: previewUninstallMode,
  defaultInstallDirUser: String.raw`C:\Users\Preview\AppData\Local\Programs\Nectar`,
  defaultInstallDirAllUsers: String.raw`C:\Program Files\Nectar`,
  prefillInstallDir: null,
  prefillAllUsers: false,
  prefillDesktopShortcut: true,
  autoInstall: false,
  autoUninstall: false,
  appVersion: "1.0.0",
  payloadPresent: true,
};

const progressTarget = new EventTarget();

function runProgress(
  channel: string,
  stages: { stage: string; message: string }[],
): Promise<void> {
  return new Promise((resolve) => {
    let i = 0;
    const step = () => {
      if (i >= stages.length) {
        resolve();
        return;
      }
      const s = stages[i];
      const payload: ProgressPayload = { stage: s.stage, percent: (i + 1) / stages.length, message: s.message };
      progressTarget.dispatchEvent(new CustomEvent(channel, { detail: payload }));
      i++;
      setTimeout(step, 450);
    };
    step();
  });
}

export function mockRunInstall(): Promise<string> {
  return runProgress("install-progress", [
    { stage: "extracting", message: "Copying files..." },
    { stage: "extracting", message: "Copying files..." },
    { stage: "shortcuts", message: "Creating shortcuts..." },
    { stage: "registry", message: "Registering with Windows..." },
    { stage: "done", message: "Done" },
  ]).then(() => String.raw`C:\Users\Preview\AppData\Local\Programs\Nectar\nectar.exe`);
}

export function mockRunUninstall(): Promise<void> {
  return runProgress("uninstall-progress", [
    { stage: "removing", message: "Removing shortcuts..." },
    { stage: "removing", message: "Removing registry entries..." },
    { stage: "done", message: "Done" },
  ]);
}

export function mockListen(channel: string, cb: (e: ProgressPayload) => void) {
  const handler = (e: Event) => cb((e as CustomEvent<ProgressPayload>).detail);
  progressTarget.addEventListener(channel, handler);
  return () => progressTarget.removeEventListener(channel, handler);
}
