import type { SettingsTab } from "./types";

export interface SearchEntry {
  key: string;
  tab: SettingsTab;
  page?: SettingsTab;
}

const group = (tab: SettingsTab, keys: string[]): SearchEntry[] => keys.map((key) => ({ key, tab }));

export const SEARCH_ENTRIES: SearchEntry[] = [
  ...group("general", [
    "general.launchAtLogin", "general.updateIndicator", "general.language", "general.weekStart", "general.time24h",
    "general.legacyMode", "general.ramSaver", "general.ramUltra", "general.lowBattery", "general.restart", "general.quit",
  ]),
  ...group("appearance", [
    "appearance.style", "appearance.mode", "appearance.presets", "appearance.accent", "appearance.accentColor",
    "appearance.color", "appearance.opacity", "appearance.saturation", "appearance.brightness", "appearance.corners",
    "appearance.scale", "appearance.glassWallpaper", "appearance.syncWindowsAccent",
  ]),
  ...group("notch", [
    "notch.behavior", "notch.calendar", "notch.calendarSync", "notch.timer", "notch.timerSound", "notch.timerProgress",
    "notch.stopwatch", "notch.dualClock", "notch.musicMode", "notch.compactMode", "notch.mediaLayout", "notch.artworkTheme",
    "notch.ambientGlow", "notch.compactGlow", "notch.panelControlCenter", "notch.panelShelf", "notch.rememberMode",
    "notch.panelOrder", "notch.agentApi", "notch.agentSounds", "notch.scrollVolume", "notch.shelfPersist", "notch.weather",
    "notch.hotkey", "notch.netWidget", "notch.edgeDelay", "notch.widgets.group", "notch.ccTiles", "notch.ccButtons", "notch.ccSliders",
  ]),
  ...group("dock", [
    "dock.enable", "dock.behavior", "dock.previews", "dock.previewDelay", "dock.hideDelay", "dock.attention", "dock.console",
    "dock.search", "dock.searchWith", "dock.searchShortcut", "dock.calendar", "dock.tray", "dock.taskView", "dock.showDesktop",
    "dock.recycleBin", "dock.iconOnly", "dock.indicator", "dock.winNumber", "dock.adaptive", "dock.mixed", "dock.startIcon",
    "dock.hidden.title", "dock.menu.title",
  ]),
  ...group("overlays", [
    "overlays.volume", "overlays.brightness", "overlays.edgeHover", "overlays.edgeDelay", "overlays.volumeSide", "overlays.brightnessSide",
  ]),
  ...group("about", [
    "about.autoUpdate", "about.prerelease", "about.update.check", "about.tour", "about.export", "about.import", "about.icons",
    "about.diagnostics.copy", "about.logs", "about.hitbox", "about.reset", "about.uninstall",
  ]),
];

export const TAB_KEYS: Record<string, string> = {
  general: "tabs.general",
  appearance: "tabs.appearance",
  notch: "tabs.notch",
  dock: "tabs.dock",
  overlays: "tabs.overlays",
  about: "tabs.about",
};

export const TAB_ENTRIES: SearchEntry[] = Object.keys(TAB_KEYS).map((tab) => ({ key: TAB_KEYS[tab], tab: tab as SettingsTab }));

export const KEYWORDS: Record<string, string> = {
  "general.launchAtLogin": "startup boot autostart start with windows run on startup login",
  "general.ramSaver": "memory ram performance lag cpu slow",
  "general.ramUltra": "memory ram gpu performance lag",
  "general.lowBattery": "power charge percent warning",
  "general.restart": "reload relaunch",
  "general.quit": "exit close stop",
  "general.legacyMode": "classic old simple",
  "appearance.syncWindowsAccent": "color colour theme explorer system",
  "appearance.mode": "dark light theme night",
  "appearance.style": "glass blur frosted liquid material theme",
  "appearance.scale": "size zoom bigger smaller font text",
  "dock.enable": "taskbar bar windows",
  "dock.menu.title": "context menu right click options quit restart task manager clear cache",
  "dock.indicator": "dots bar line tab running underline marker",
  "dock.previews": "thumbnail hover preview window peek",
  "dock.iconOnly": "background padding tiles",
  "dock.winNumber": "keyboard shortcut hotkey",
  "dock.attention": "badge unread notification flash",
  "notch.hotkey": "shortcut keyboard hide show toggle",
  "notch.agentApi": "ai claude codex agent status",
  "notch.musicMode": "spotify media player song cover",
  "notch.weather": "temperature forecast city location",
  "notch.timer": "countdown pomodoro alarm",
  "overlays.volume": "sound audio slider hud",
  "overlays.brightness": "screen light slider hud",
  "about.reset": "factory default restore erase",
  "about.uninstall": "remove delete",
  "about.export": "backup save file",
  "about.import": "restore load file backup",
  "about.autoUpdate": "upgrade version",
  "about.logs": "debug folder",
};
