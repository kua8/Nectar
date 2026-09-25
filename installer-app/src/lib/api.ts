import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri, mockInitialState, mockRunInstall, mockRunUninstall, mockListen } from "./mock";

export interface InitialState {
  uninstallMode: boolean;
  defaultInstallDirUser: string;
  defaultInstallDirAllUsers: string;
  prefillInstallDir: string | null;
  prefillAllUsers: boolean;
  prefillDesktopShortcut: boolean;
  autoInstall: boolean;
  autoUninstall: boolean;
  appVersion: string;
  payloadPresent: boolean;
}

export interface ProgressPayload {
  stage: string;
  percent: number;
  message: string;
}

export const getInitialState = (): Promise<InitialState> =>
  isTauri() ? invoke<InitialState>("get_initial_state") : Promise.resolve(mockInitialState);

export const pickInstallDir = (current: string): Promise<string | null> =>
  isTauri()
    ? invoke<string | null>("pick_install_dir", { current })
    : Promise.resolve(window.prompt("Install location (preview only):", current));

export const needsElevation = (allUsers: boolean): Promise<boolean> =>
  isTauri() ? invoke<boolean>("needs_elevation", { allUsers }) : Promise.resolve(false);

export const relaunchElevated = (
  installDir: string,
  allUsers: boolean,
  desktopShortcut: boolean,
): Promise<void> =>
  isTauri()
    ? invoke<void>("relaunch_elevated", { installDir, allUsers, desktopShortcut })
    : Promise.resolve(console.log("[preview] relaunch elevated (no-op)"));

export const startInstall = (
  installDir: string,
  allUsers: boolean,
  desktopShortcut: boolean,
): Promise<string> =>
  isTauri()
    ? invoke<string>("start_install", { installDir, allUsers, desktopShortcut })
    : mockRunInstall();

export const launchApp = (exePath: string): Promise<void> =>
  isTauri() ? invoke<void>("launch_app", { exePath }) : Promise.resolve(console.log("[preview] launch", exePath));

export const openInstallFolder = (path: string): Promise<void> =>
  isTauri() ? invoke<void>("open_install_folder", { path }) : Promise.resolve(console.log("[preview] open folder", path));

export const startUninstall = (): Promise<void> =>
  isTauri() ? invoke<void>("start_uninstall") : mockRunUninstall();

export const openUrl = (url: string): Promise<void> =>
  isTauri() ? invoke<void>("open_url", { url }) : Promise.resolve(void window.open(url, "_blank"));

export const uiReady = (): Promise<void> =>
  isTauri() ? invoke<void>("ui_ready") : Promise.resolve();

export const minimizeWindow = (): Promise<void> =>
  isTauri() ? invoke<void>("window_minimize") : Promise.resolve(console.log("[preview] minimize (no-op)"));

export const closeWindow = (): Promise<void> =>
  isTauri() ? invoke<void>("window_close") : Promise.resolve(console.log("[preview] close (no-op)"));

export const onInstallProgress = (cb: (e: ProgressPayload) => void) =>
  isTauri()
    ? listen<ProgressPayload>("install-progress", (event) => cb(event.payload))
    : Promise.resolve(mockListen("install-progress", cb));

export const onUninstallProgress = (cb: (e: ProgressPayload) => void) =>
  isTauri()
    ? listen<ProgressPayload>("uninstall-progress", (event) => cb(event.payload))
    : Promise.resolve(mockListen("uninstall-progress", cb));
