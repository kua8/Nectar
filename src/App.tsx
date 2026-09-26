import { motion, AnimatePresence, useAnimation } from "framer-motion";
import { useEffect, useState, useCallback, useRef, memo } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UpdateCheckResult } from "./updater";
import "./App.css";
import { initTheme } from "./theme";
import { PlayIcon, PauseIcon, SkipBackIcon, SkipForwardIcon, VolumeLowIcon, VolumeHighIcon, MusicNoteIcon, AudioOutputIcon } from "./icons";
import { CompactMediaPlayer } from "./CompactMediaPlayer";
import { useWeather } from "./hooks/useWeather";
import { useSettingsSync } from "./hooks/useSettingsSync";
import { useCaldav } from "./hooks/useCaldav";
import { NotchCalendar } from "./components/NotchCalendar";
import { NotchClock, StopwatchReadout } from "./components/NotchClock";
import { useStopwatch } from "./hooks/useStopwatch";
import type { WidgetConfig } from "./components/StatusWidgetConfig";
import {
  Cpu,
  MemoryStick,
  HardDrive,
  ArrowUpDown,
} from "lucide-react";

// Simple SVG icons
function WifiIcon({ connected }: { connected: boolean }) {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" opacity={connected ? 1 : 0.4}>
      <path d="M5 12.55a11 11 0 0 1 14.08 0" />
      <path d="M1.42 9a16 16 0 0 1 21.16 0" />
      <path d="M8.53 16.11a6 6 0 0 1 6.95 0" />
      <line x1="12" y1="20" x2="12.01" y2="20" />
    </svg>
  );
}

function TrayIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="6" height="6" rx="1" />
      <rect x="15" y="3" width="6" height="6" rx="1" />
      <rect x="15" y="15" width="6" height="6" rx="1" />
      <rect x="3" y="15" width="6" height="6" rx="1" />
    </svg>
  );
}

function BatteryIcon({ charging, level, threshold = 20 }: { charging: boolean; level: number; threshold?: number }) {
  const percentage = Math.min(Math.max(level, 0), 100);

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      position: 'relative',
      height: '14px',
      justifyContent: 'center'
    }}>
      <svg width="20" height="10" viewBox="0 0 20 10" fill="none">
        {/* Battery Shell - Centered at 9px within 20px width, ignoring the tip's offset */}
        <rect
          x="2" y="0.75" width="14" height="8.5" rx="2.4"
          stroke="currentColor" strokeOpacity={0.35} strokeWidth="1.1"
        />
        {/* Battery Tip */}
        <path
          d="M17.5 3.5V6.5"
          stroke="currentColor" strokeOpacity={0.35} strokeWidth="1.2" strokeLinecap="round"
        />
        {/* Fill */}
        <rect
          x="3.8" y="2.5"
          width={Math.max(0.5, (percentage / 100) * 10.4)}
          height="5" rx="1"
          fill={charging ? "#32D74B" : (percentage <= threshold ? "#FF453A" : "white")}
        />
      </svg>
      {/* Charging Bolt - Centered on the battery body */}
      {charging && (
        <div style={{
          position: 'absolute',
          top: '50%',
          left: '9px',
          transform: 'translate(-50%, -50%)',
          color: 'white',
          filter: 'drop-shadow(0px 0px 1.5px rgba(0,0,0,0.8))'
        }}>
          <svg width="7" height="10" viewBox="0 0 8 12" fill="currentColor">
            <path d="M4.5 0L0 7H3.5L2.5 12L8 5H4.5L5.5 0H4.5Z" />
          </svg>
        </div>
      )}
    </div>
  );
}

function GreenDownArrowIcon() {
  return (
    <div style={{
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '18px',
      height: '18px',
      borderRadius: '50%',
      background: 'rgba(50, 215, 75, 0.15)',
      border: '1px solid rgba(50, 215, 75, 0.35)',
      boxShadow: '0 0 8px rgba(50, 215, 75, 0.25)',
      flexShrink: 0,
      verticalAlign: 'middle'
    }}>
      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="#32D74B" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
        <line x1="12" y1="4" x2="12" y2="16"></line>
        <polyline points="18 10 12 16 6 10"></polyline>
        <line x1="6" y1="20" x2="18" y2="20"></line>
      </svg>
    </div>
  );
}

export const Visualizer = memo(function Visualizer({ isPlaying, bars = 5, height = 20 }: { isPlaying: boolean; bars?: number; height?: number }) {
  const [audioData, setAudioData] = useState<number[]>(new Array(bars).fill(0.18));

  useEffect(() => {
    if (!isPlaying) {
      setAudioData(new Array(bars).fill(0.18));
      return;
    }

    const unlisten = listen<{ frequencies: number[] }>("audio-visualization", (event) => {
      // If we receive fewer frequencies than bars, repeat or interpolate
      // If more, slice
      let data = event.payload.frequencies;
      if (data.length > bars) data = data.slice(0, bars);
      while (data.length < bars) data.push(0.18);
      setAudioData(data);
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, [isPlaying, bars]);

  return (
    <div className="visualizer-horizontal" style={{ height: `${height}px`, width: `${bars * 6}px` }}>
      {audioData.map((value, i) => (
        <motion.div
          key={i}
          className="bar-horizontal"
          animate={{
            scaleY: isPlaying ? Math.max(0.2, value) : 0.1,
            opacity: isPlaying ? 0.95 : 0.5
          }}
          transition={{
            type: "spring",
            stiffness: 600,
            damping: 30,
            mass: 0.5
          }}
        />
      ))}
    </div>
  );
});

function UpdateActivity() {
  return (
    <div className="update-activity">
      <motion.div
        className="update-activity-glow"
        animate={{ opacity: [0.5, 1, 0.5], scale: [0.9, 1, 0.9] }}
        transition={{ duration: 1.6, repeat: Infinity, ease: "easeInOut" }}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#32D74B" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 5v11" />
          <path d="M6 12l6 6 6-6" />
        </svg>
      </motion.div>
    </div>
  );
}

interface MediaInfo {
  title: string;
  artist: string;
  is_playing: boolean;
  has_media: boolean;
  artwork?: string[];
  position_ms?: number;
  duration_ms?: number;
  seek_enabled?: boolean;
  position_updated_at?: number;
}

const MARQUEE_SPEED = 30; // px/s — constant for all titles
const MARQUEE_MIN_DURATION = 5; // floor so short titles don't flicker

const TitleMarquee = ({ title }: { title: string }) => {
  const textRef = useRef<HTMLSpanElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const [scrollDistance, setScrollDistance] = useState(0);
  const [scrollDuration, setScrollDuration] = useState(8);

  useEffect(() => {
    const check = () => {
      if (textRef.current && containerRef.current) {
        const textWidth = textRef.current.scrollWidth;
        const containerWidth = containerRef.current.clientWidth;
        const overflow = textWidth > containerWidth;
        setIsOverflowing(overflow);
        if (overflow) {
          const distance = textWidth - containerWidth;
          setScrollDistance(distance);
          setScrollDuration(Math.max(distance / MARQUEE_SPEED, MARQUEE_MIN_DURATION));
        }
      }
    };
    check();
    const observer = new ResizeObserver(check);
    if (containerRef.current) observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [title]);

  return (
    <div ref={containerRef} className="premium-title-wrap" style={{ width: '100%', overflow: 'hidden' }}>
      <span
        ref={textRef}
        className={`premium-title ${isOverflowing ? 'marquee' : ''}`}
        style={{
          '--scroll-distance': isOverflowing ? `-${scrollDistance}px` : undefined,
          '--scroll-duration': `${scrollDuration}s`,
        } as React.CSSProperties}
      >
        {title}
      </span>
    </div>
  );
};




function App() {
  useEffect(() => {
    return initTheme();
  }, []);

  const [time, setTime] = useState("");
  const [isHovered, setIsHovered] = useState(false);
  const [isReady, setIsReady] = useState(false);
  const [scale, setScale] = useState(() => parseFloat(localStorage.getItem("nectar-scale") || "1.0"));
  const [timeFormat24h, setTimeFormat24h] = useState(() => localStorage.getItem("nectar-time-format-24h") === "true");


  const [batteryLevel, setBatteryLevel] = useState(100);
  const [isCharging, setIsCharging] = useState(false);
  const [hasBattery, setHasBattery] = useState(true);
  const [showPowerPulse, setShowPowerPulse] = useState(false);
  const [showLowBatteryPulse, setShowLowBatteryPulse] = useState(false);
  const [lowBatteryThreshold, setLowBatteryThreshold] = useState(() => parseInt(localStorage.getItem("nectar-low-battery-threshold") || "20"));
  const prevChargingRef = useRef<boolean | null>(null);
  const powerPulseTimeoutRef = useRef<any>(null);
  const lowBatteryPulseShownRef = useRef<boolean>(false);

  const [eventPeek, setEventPeek] = useState(false);
  const eventPeekTimeoutRef = useRef<any>(null);
  const updatePulseTimerRef = useRef<any>(null);
  const triggerEventPeek = useCallback((duration = 3000) => {
    setEventPeek(true);
    if (eventPeekTimeoutRef.current) clearTimeout(eventPeekTimeoutRef.current);
    eventPeekTimeoutRef.current = setTimeout(() => setEventPeek(false), duration);
  }, []);

  const [notchMode, setNotchMode] = useState(() => {
    const raw = localStorage.getItem("nectar-notch-mode") || "fixed";
    if (raw === "auto-hide") return "smart";
    return raw;
  });


  useEffect(() => {
    if (hasBattery && isReady && prevChargingRef.current !== null && prevChargingRef.current !== isCharging) {
      setShowPowerPulse(true);
      if (notchMode === 'peek') triggerEventPeek(4000);
      if (powerPulseTimeoutRef.current) clearTimeout(powerPulseTimeoutRef.current);
      powerPulseTimeoutRef.current = setTimeout(() => {
        setShowPowerPulse(false);
      }, 4000);
    }
    prevChargingRef.current = isCharging;
  }, [isCharging, isReady, notchMode, triggerEventPeek, hasBattery]);

  useEffect(() => {
    // Trigger pulse when dropping below threshold while discharging
    if (hasBattery && isReady && batteryLevel <= lowBatteryThreshold && !isCharging && !lowBatteryPulseShownRef.current) {
      setShowLowBatteryPulse(true);
      if (notchMode === 'peek') triggerEventPeek(5000);
      lowBatteryPulseShownRef.current = true;
      setTimeout(() => setShowLowBatteryPulse(false), 5000);
    }

    // Reset the "shown" state if battery is charged or threshold is lowered
    if (isCharging || batteryLevel > lowBatteryThreshold) {
      lowBatteryPulseShownRef.current = false;
    }
  }, [batteryLevel, isCharging, lowBatteryThreshold, isReady, notchMode, triggerEventPeek, hasBattery]);

  // Weather state (managed by useWeather hook)

  // Media state
  const [isPlaying, setIsPlaying] = useState(false);
  const [mediaInfo, setMediaInfo] = useState<MediaInfo>({
    title: "",
    artist: "",
    is_playing: false,
    has_media: false
  });
  const [albumArtUrl, setAlbumArtUrl] = useState<string | null>(null);
  const [albumArtKey, setAlbumArtKey] = useState(0);
  const [volume, setVolume] = useState(0.5);
  const [wifiEnabled, setWifiEnabled] = useState(true);
  const [bluetoothEnabled, setBluetoothEnabled] = useState(true);
  const [batterySaverEnabled, setBatterySaverEnabled] = useState(false);
  const [currentBrightness, setCurrentBrightness] = useState(50);

  // System metrics for status widgets
  const [cpuUsage, setCpuUsage] = useState(0);
  const [ramUsage, setRamUsage] = useState(0);
  const [diskSpace, setDiskSpace] = useState(0);
  const [netUpSpeed, setNetUpSpeed] = useState(0);
  const [netDownSpeed, setNetDownSpeed] = useState(0);
  const [statusWidgets, setStatusWidgets] = useState<WidgetConfig>({ left: ["weather"], right: ["battery"] });

  const [windowLabel, setWindowLabel] = useState<string>("");
  useEffect(() => {
    setWindowLabel(getCurrentWebviewWindow().label);
  }, []);
  const isNotchWindow = windowLabel === 'main' || windowLabel.startsWith('main-');

  // Update state
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [showUpdateIndicator, setShowUpdateIndicator] = useState(() => localStorage.getItem("nectar-show-update-indicator") !== "false");
  const [showUpdatePulse, setShowUpdatePulse] = useState(false);

  useEffect(() => {
    if (windowLabel !== 'main') return;

    let unlisten: (() => void) | undefined;
    let disposed = false;

    listen<UpdateCheckResult>("update-available", (event) => {
      setUpdateAvailable(event.payload.available);
      if (!event.payload.available) {
        setShowUpdatePulse(false);
        if (updatePulseTimerRef.current) clearTimeout(updatePulseTimerRef.current);
        return;
      }
      setShowUpdatePulse(true);
      if (notchMode === 'peek') triggerEventPeek(6000);
      if (updatePulseTimerRef.current) clearTimeout(updatePulseTimerRef.current);
      updatePulseTimerRef.current = setTimeout(() => {
        setShowUpdatePulse(false);
      }, 6000);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });

    invoke<UpdateCheckResult>("get_update_state")
      .then((state) => setUpdateAvailable(state.available))
      .catch((e) => console.error("Failed to read update state:", e));

    return () => {
      disposed = true;
      unlisten?.();
      if (updatePulseTimerRef.current) clearTimeout(updatePulseTimerRef.current);
    };
  }, [windowLabel, notchMode, triggerEventPeek]);

  const [isVisible, setIsVisible] = useState(true);
  const [isImpacted, setIsImpacted] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [startupAnimating, setStartupAnimating] = useState(false);

  const [dockMode, setDockMode] = useState(() => {
    const raw = localStorage.getItem("nectar-dock-mode") || "fixed";
    if (raw === "auto-hide") return "smart";
    return raw;
  });
  const [dndActive, setDndActive] = useState(false);
  const [dockEnabled, setDockEnabled] = useState(() => localStorage.getItem("nectar-dock-enabled") !== "false");
  const [isNotchHovered, setIsNotchHovered] = useState(false);

  const [isEdgeHovered, setIsEdgeHovered] = useState(false);
  const [isOverlapped, setIsOverlapped] = useState(false);
  const [interactionState, setInteractionState] = useState<'active' | 'grace' | 'none'>('none');
  const nectarRef = useRef<HTMLDivElement>(null);
  const dockEnabledInitial = useRef(true);
  const dockModeInitial = useRef(true);
  const notchModeInitial = useRef(true);

  const isAnyInteraction = isHovered || isNotchHovered || isEdgeHovered;
  const isHidden = !startupAnimating && (
    (notchMode === 'smart' && isOverlapped && interactionState === 'none') ||
    (notchMode === 'peek' && interactionState === 'none' && !eventPeek)
  );

  useEffect(() => {
    if (isAnyInteraction) {
      setInteractionState('active');
    } else if (interactionState !== 'none') {
      setInteractionState('grace');
      const timer = setTimeout(() => setInteractionState('none'), 800);
      return () => clearTimeout(timer);
    }
  }, [isAnyInteraction]);

  useEffect(() => {
    if (isNotchWindow) {
      invoke('set_notch_hovered', { hovered: isNotchHovered }).catch(() => { });
    }
  }, [isNotchHovered, isNotchWindow]);

  useEffect(() => {
    const updateRect = () => {
      if (nectarRef.current && isNotchWindow) {
        const rect = nectarRef.current.getBoundingClientRect();
        invoke('update_notch_rect', {
          rect: {
            x: Math.round(rect.x),
            y: Math.round(rect.y),
            width: Math.round(rect.width),
            height: Math.round(rect.height)
          }
        }).catch(() => { });
      }
    };

    updateRect();
    window.addEventListener('resize', updateRect);
    const observer = new ResizeObserver(updateRect);
    if (nectarRef.current) observer.observe(nectarRef.current);

    return () => {
      window.removeEventListener('resize', updateRect);
      observer.disconnect();
    };
  }, [isExpanded, isHidden, windowLabel, scale]);

  useEffect(() => {
    if (!windowLabel) return;

    // Only animate the main top-bar
    if (windowLabel !== 'main') {
      setIsReady(true);
      setIsImpacted(true);
      setIsExpanded(true);
      return;
    }

    const proceedWithStartup = () => {
      const checkVisibility = async () => {
        try {
          const win = getCurrentWebviewWindow();
          const visible = await win.isVisible();
          if (visible) {
            setStartupAnimating(true);
            setIsReady(true);
            setTimeout(() => {
              setIsImpacted(true);
              setIsExpanded(true);
            }, 240);
            setTimeout(() => setStartupAnimating(false), 1500);
            return true;
          }
        } catch (e) { }
        return false;
      };

      const interval = setInterval(async () => {
        if (await checkVisibility()) clearInterval(interval);
      }, 100);

      checkVisibility();
      return interval;
    };

    const DOCK_SYNC_DELAY_MS = 400;
    const waitForSplash = () => {
      let started = false;
      let interval: any;
      let syncTimer: any;
      const begin = () => {
        if (started) return;
        started = true;
        syncTimer = setTimeout(() => { interval = proceedWithStartup(); }, DOCK_SYNC_DELAY_MS);
      };
      const unlistenSplash = listen("splash-done", () => {
        begin();
        unlistenSplash.then(fn => fn());
      });
      const safetyTimer = setTimeout(() => {
        begin();
        unlistenSplash.then(fn => fn());
      }, 6000);
      return () => {
        clearTimeout(safetyTimer);
        clearTimeout(syncTimer);
        if (interval) clearInterval(interval);
        unlistenSplash.then(fn => fn());
      };
    };

    return waitForSplash();
  }, [windowLabel]);

  // Settings state
  const [settingsWeatherEnabled, setSettingsWeatherEnabled] = useState(() => localStorage.getItem("nectar-weather-enabled") !== "false");
  const [settingsCalendarEnabled, setSettingsCalendarEnabled] = useState(() => localStorage.getItem("nectar-calendar-enabled") !== "false");
  const [settingsTimerEnabled, setSettingsTimerEnabled] = useState(
    () => (localStorage.getItem("nectar-timer-enabled") ?? localStorage.getItem("nectar-calendar-enabled")) !== "false",
  );
  const [settingsStopwatchEnabled, setSettingsStopwatchEnabled] = useState(
    () => (localStorage.getItem("nectar-stopwatch-enabled") ?? localStorage.getItem("nectar-calendar-enabled")) !== "false",
  );
  const showClockColumn = settingsTimerEnabled || settingsStopwatchEnabled;
  const productivityEnabled = settingsCalendarEnabled || showClockColumn;
  const [settingsMusicModeEnabled, setSettingsMusicModeEnabled] = useState(() => localStorage.getItem("nectar-music-mode-enabled") !== "false");
  const [settingsMusicCompactNotch, setSettingsMusicCompactNotch] = useState(() => localStorage.getItem("nectar-music-compact-notch") !== "false");
  const [settingsVisualizerEnabled, setSettingsVisualizerEnabled] = useState(() => localStorage.getItem("nectar-visualizer-enabled") !== "false");
  const [settingsAlbumArtEnabled, setSettingsAlbumArtEnabled] = useState(() => localStorage.getItem("nectar-media-album-art-enabled") !== "false");
  const [settingsAmbienceEnabled, setSettingsAmbienceEnabled] = useState(() => localStorage.getItem("nectar-media-ambience-enabled") !== "false");
  const [settingsCompactGlowEnabled, setSettingsCompactGlowEnabled] = useState(() => localStorage.getItem("nectar-media-compact-glow-enabled") !== "false");
  const [settingsCornersEnabled, setSettingsCornersEnabled] = useState(() => localStorage.getItem("nectar-corners-enabled") === "true");
  const [mediaLayout, setMediaLayout] = useState<'classic' | 'compact'>(() => (localStorage.getItem("nectar-media-layout") as 'classic' | 'compact') || 'classic');
  const [compactVolumeExpanded, setCompactVolumeExpanded] = useState(false);

  // Weather hook
  const { temperature, weatherCondition, weatherIcon: WeatherIcon, cityName, tempUnit } = useWeather(settingsWeatherEnabled);

  useEffect(() => {
    if (!windowLabel) return;

    invoke("load_settings").then((settings: any) => {
      const getVal = (key: string, fallback: string | null = null) => {
        const val = settings[key];
        if (val !== undefined && val !== null) return String(val);
        const local = localStorage.getItem(key);
        if (local !== null) return local;
        return fallback;
      };

      setSettingsWeatherEnabled(getVal("nectar-weather-enabled", "true") !== "false");
      setSettingsCalendarEnabled(getVal("nectar-calendar-enabled", "true") !== "false");
      setSettingsTimerEnabled((getVal("nectar-timer-enabled") ?? getVal("nectar-calendar-enabled", "true")) !== "false");
      setSettingsStopwatchEnabled((getVal("nectar-stopwatch-enabled") ?? getVal("nectar-calendar-enabled", "true")) !== "false");
      setSettingsMusicModeEnabled(getVal("nectar-music-mode-enabled", "true") !== "false");
      setSettingsMusicCompactNotch(getVal("nectar-music-compact-notch", "true") !== "false");
      const viz = getVal("nectar-media-visualizer-enabled") ?? getVal("nectar-visualizer-enabled", "true");
      setSettingsVisualizerEnabled(viz !== "false");
      setSettingsAlbumArtEnabled(getVal("nectar-media-album-art-enabled", "true") !== "false");
      setSettingsAmbienceEnabled(getVal("nectar-media-ambience-enabled", "true") !== "false");
      setSettingsCompactGlowEnabled(getVal("nectar-media-compact-glow-enabled", "true") !== "false");
      setSettingsCornersEnabled(getVal("nectar-corners-enabled", "false") === "true");
      setTimeFormat24h(getVal("nectar-time-format-24h") === "true");

      const thresholdStr = getVal("nectar-low-battery-threshold", "20");
      if (thresholdStr) setLowBatteryThreshold(parseInt(thresholdStr as string));

      const nMode = getVal("nectar-notch-mode", "fixed");
      if (nMode) {
        const mapped = nMode === "auto-hide" ? "smart" : nMode;
        setNotchMode(mapped);
      }

      if (windowLabel === 'main') {
        const firstRun = localStorage.getItem("nectar-first-run") === null;
        if (firstRun) {
          import("@tauri-apps/plugin-autostart").then(({ enable, isEnabled }) => {
            isEnabled().then(enabled => {
              if (!enabled) enable().catch(() => { });
            });
          });
          localStorage.setItem("nectar-first-run", "done");
        }
        const rawDockMode = getVal("nectar-dock-mode", "fixed") as string;
        const dockMode = rawDockMode === "auto-hide" ? "smart" : rawDockMode;
        const syncWindows = async () => {
          const dockEnabled = getVal("nectar-dock-enabled", "true") === "true";
          await Promise.all([
            dockEnabled ? invoke("init_dock", { mode: dockMode }) : Promise.resolve(),
            invoke("change_notch_mode", { mode: nMode }),
          ]);
          await invoke("sync_appbar");
        };

        const dockEnabled = getVal("nectar-dock-enabled", "true") === "true";
        const runDockInit = () => {
          // 1. Snappy initial sync
          setTimeout(syncWindows, 400);
          // 2. Safety-net dock retry
          if (dockEnabled) {
            setTimeout(() => invoke("init_dock", { mode: dockMode }).catch(() => {}), 1500);
          }
          // 3. Layout corrections
          setTimeout(() => invoke("sync_appbar"), 1000);
          setTimeout(() => invoke("sync_appbar"), 2500);
          setTimeout(() => invoke("sync_appbar"), 5000);
        };

        const runDockInitAfterSplash = () => {
          let dockStarted = false;
          const unlistenDock = listen("splash-done", () => {
            if (dockStarted) return;
            dockStarted = true;
            runDockInit();
            unlistenDock.then(fn => fn());
          });
          setTimeout(() => {
            if (dockStarted) return;
            dockStarted = true;
            runDockInit();
            unlistenDock.then(fn => fn());
          }, 6000);
        };

        runDockInitAfterSplash();
      }

      const scaleVal = getVal("nectar-scale");
      if (scaleVal !== null) setScale(parseFloat(scaleVal));

      const widgetsVal = getVal("nectar-status-widgets");
      if (widgetsVal) {
        try {
          const parsed = JSON.parse(widgetsVal);
          if (parsed && Array.isArray(parsed.left) && Array.isArray(parsed.right)) {
            setStatusWidgets(parsed);
          }
        } catch {}
      }
    }).catch(console.error);
  }, [windowLabel]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    // Disable context menu globally
    const preventContext = (e: MouseEvent) => e.preventDefault();
    document.addEventListener('contextmenu', preventContext);

    const unlistenVisibility = listen<boolean>("visibility-change", (event) => {
      setIsVisible(event.payload);
    });

    const unlistenNotchOverlap = listen<boolean>("notch-overlap", (event) => {
      setIsOverlapped(event.payload);
    });

    const unlistenNotchEdgeHover = listen<boolean>("notch-edge-hover", (event) => {
      setIsEdgeHovered(event.payload);
    });

    const unlistenSettingsReset = listen("settings-reset", () => {
      localStorage.clear();
      window.location.reload();
    });

    return () => {
      unlistenVisibility.then(f => f());
      unlistenNotchOverlap.then(f => f());
      unlistenNotchEdgeHover.then(f => f());
      unlistenSettingsReset.then(f => f());
      document.removeEventListener('contextmenu', preventContext);
    };
  }, [windowLabel]);

  // Settings sync — consolidated dispatch for all settings events
  useSettingsSync(
    {
      "nectar-weather-enabled": setSettingsWeatherEnabled,
      "nectar-calendar-enabled": setSettingsCalendarEnabled,
      "nectar-timer-enabled": setSettingsTimerEnabled,
      "nectar-stopwatch-enabled": setSettingsStopwatchEnabled,
      "nectar-music-mode-enabled": setSettingsMusicModeEnabled,
      "nectar-music-compact-notch": setSettingsMusicCompactNotch,
      "nectar-media-visualizer-enabled": setSettingsVisualizerEnabled,
      "nectar-visualizer-enabled": setSettingsVisualizerEnabled,
      "nectar-media-album-art-enabled": setSettingsAlbumArtEnabled,
      "nectar-media-ambience-enabled": setSettingsAmbienceEnabled,
      "nectar-media-compact-glow-enabled": setSettingsCompactGlowEnabled,
      "nectar-media-layout": setMediaLayout,
      "nectar-corners-enabled": setSettingsCornersEnabled,
      "nectar-scale": setScale,
      "nectar-low-battery-threshold": setLowBatteryThreshold,
      "nectar-dock-enabled": setDockEnabled,
      "nectar-dock-mode": setDockMode,
      "nectar-notch-mode": setNotchMode,
      "nectar-status-widgets": (value) => {
        try {
          const parsed = JSON.parse(value);
          if (parsed && Array.isArray(parsed.left) && Array.isArray(parsed.right)) {
            setStatusWidgets(parsed);
          }
        } catch {}
      },
      "nectar-show-update-indicator": (value) => setShowUpdateIndicator(String(value) === "true"),
      "nectar-time-format-24h": setTimeFormat24h,
    },
    [windowLabel]
  );

  // Side effects: dock-enabled (skip first render — startup code handles initial state)
  useEffect(() => {
    if (windowLabel !== 'main') return;
    if (dockEnabledInitial.current) { dockEnabledInitial.current = false; return; }
    if (dockEnabled) {
      invoke("init_dock", { mode: localStorage.getItem("nectar-dock-mode") || "fixed" });
    } else {
      invoke("toggle_dock", { enable: false });
    }
    setTimeout(() => invoke("sync_appbar"), 200);
  }, [dockEnabled, windowLabel]);

  // Side effects: dock-mode (skip first render)
  useEffect(() => {
    if (windowLabel !== 'main') return;
    if (dockModeInitial.current) { dockModeInitial.current = false; return; }
    invoke("change_dock_mode", { mode: dockMode });
    setTimeout(() => invoke("sync_appbar"), 200);
  }, [dockMode, windowLabel]);

  // Side effects: notch-mode (skip first render)
  useEffect(() => {
    if (windowLabel !== 'main') return;
    if (notchModeInitial.current) { notchModeInitial.current = false; return; }
    invoke("change_notch_mode", { mode: notchMode });
  }, [notchMode, windowLabel]);

  const [nectarMode, setNectarMode] = useState<'music' | 'calendar' | 'command-center' | 'status'>('status');
  const caldav = useCaldav();

  // Window height is now kept constant to prevent rendering layout lag and sharp corners

  const wheelAccumRef = useRef(0);
  const wheelIdleTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const WHEEL_STEP_THRESHOLD = 60;
  const WHEEL_IDLE_RESET_MS = 180;

  const handleWheel = (e: React.WheelEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.calendar-grid') || target.closest('.timer-column') || target.closest('.cal-scroll')) {
      return;
    }

    if (!isHovered) return;

    // Use absolute values to detect horizontal swipe gestures on trackpad
    const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
    if (Math.abs(delta) < 2) return; // Ignore near-zero noise

    wheelAccumRef.current += delta;
    if (wheelIdleTimeoutRef.current) clearTimeout(wheelIdleTimeoutRef.current);
    wheelIdleTimeoutRef.current = setTimeout(() => { wheelAccumRef.current = 0; }, WHEEL_IDLE_RESET_MS);

    if (Math.abs(wheelAccumRef.current) < WHEEL_STEP_THRESHOLD) return;
    const direction = wheelAccumRef.current > 0 ? 1 : -1;
    wheelAccumRef.current = 0;

    // Music shifts position based on playing state:
    // Playing: command-center → music → status → calendar (active, near command-center)
    // Paused:  command-center → status → music → calendar (secondary, after status)
    const musicBeforeStatus = isPlaying && mediaInfo.has_media && settingsMusicModeEnabled;
    const modes: ('command-center' | 'status' | 'music' | 'calendar')[] = musicBeforeStatus
      ? ['command-center', 'music', 'status', 'calendar']
      : ['command-center', 'status', 'music', 'calendar'];
    const availableModes = modes.filter(m => {
      if (m === 'music' && (!settingsMusicModeEnabled || !mediaInfo.has_media)) return false;
      if (m === 'calendar' && !productivityEnabled) return false;
      return true;
    });

    const currentIndex = availableModes.indexOf(nectarMode);
    if (currentIndex === -1) return;

    const nextIndex = (currentIndex + direction + availableModes.length) % availableModes.length;
    const nextMode = availableModes[nextIndex];
    manualMusicRef.current = nextMode === 'music';
    setNectarMode(nextMode);
  };



  // Timer state
  const [timerSeconds, setTimerSeconds] = useState(0);
  const [isTimerRunning, setIsTimerRunning] = useState(false);
  const [isCompactTimerVisible, setIsCompactTimerVisible] = useState(false);
  const [isTimerFinished, setIsTimerFinished] = useState(false);
  const timerIntervalRef = useRef<any>(null);

  const formatTimerTime = (totalSeconds: number) => {
    const mins = Math.floor(Math.abs(totalSeconds) / 60);
    const secs = Math.abs(totalSeconds) % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const startTimerSeconds = (seconds: number) => {
    setTimerSeconds(seconds);
    setIsTimerRunning(true);
    setIsTimerFinished(false);
  };

  const toggleTimer = () => setIsTimerRunning(!isTimerRunning);
  const resetTimer = () => {
    setIsTimerRunning(false);
    setTimerSeconds(0);
    setIsTimerFinished(false);
  };

  const stopwatch = useStopwatch();

  useEffect(() => {
    if (!settingsTimerEnabled) resetTimer();
  }, [settingsTimerEnabled]);

  useEffect(() => {
    if (!settingsStopwatchEnabled) stopwatch.reset();
  }, [settingsStopwatchEnabled]);

  useEffect(() => {
    if (isTimerRunning && timerSeconds > 0) {
      timerIntervalRef.current = setInterval(() => {
        setTimerSeconds(s => s - 1);
      }, 1000);
    } else if (isTimerRunning && timerSeconds === 0) {
      setIsTimerRunning(false);
      setIsTimerFinished(true);
    }
    return () => {
      if (timerIntervalRef.current) clearInterval(timerIntervalRef.current);
    };
  }, [isTimerRunning, timerSeconds === 0]);

  const lastTrackRef = useRef<string | null>(null);
  const lastPlayingRef = useRef<boolean>(false);
  const manualMusicRef = useRef<boolean>(false);

  // Auto-switch to music mode only when a *new* track starts while playing,
  // or when playback transitions from paused to playing.
  useEffect(() => {
    const isNewTrackWhilePlaying = mediaInfo.title !== lastTrackRef.current && isPlaying;
    const justStartedPlaying = isPlaying && !lastPlayingRef.current;

    // Peek notch on media events (play start or track change)
    if (notchMode === 'peek' && (isNewTrackWhilePlaying || justStartedPlaying)) {
      triggerEventPeek(3000);
    }

    // Only auto-switch if music mode is enabled
    if (settingsMusicModeEnabled && mediaInfo.has_media && isPlaying && nectarMode !== 'calendar' && (isNewTrackWhilePlaying || justStartedPlaying)) {
      // Switch if compact notch display is enabled OR we are hovered
      if (settingsMusicCompactNotch || isHovered) {
        manualMusicRef.current = false;
        setNectarMode('music');
      }
    }

    lastTrackRef.current = mediaInfo.title;
    lastPlayingRef.current = isPlaying;
  }, [mediaInfo.has_media, isPlaying, mediaInfo.title, settingsMusicModeEnabled, settingsMusicCompactNotch, isHovered, nectarMode, notchMode, triggerEventPeek]);

  // Auto-switch back from music if music stops for 5 seconds
  // Skip if user manually scrolled to music mode
  useEffect(() => {
    let timer: any;
    if (!isPlaying && nectarMode === 'music' && !manualMusicRef.current) {
      timer = setTimeout(() => {
        setNectarMode('status');
      }, 5000);
    }
    return () => clearTimeout(timer);
  }, [isPlaying, nectarMode]);

  useEffect(() => {
    if (!productivityEnabled && nectarMode === 'calendar') {
      setNectarMode('status');
    }
  }, [productivityEnabled, nectarMode]);

  useEffect(() => {
    if (!settingsMusicModeEnabled && nectarMode === 'music') {
      setNectarMode('status');
    }
  }, [settingsMusicModeEnabled, nectarMode]);

  useEffect(() => {
    if (!settingsMusicCompactNotch && nectarMode === 'music' && !isHovered) {
      setNectarMode('status');
    }
  }, [settingsMusicCompactNotch, nectarMode, isHovered]);

  useEffect(() => {
    if (settingsMusicModeEnabled && settingsMusicCompactNotch && mediaInfo.has_media && isPlaying && nectarMode === 'status' && !isHovered) {
      setNectarMode('music');
    }
  }, [settingsMusicModeEnabled, settingsMusicCompactNotch, mediaInfo.has_media, isPlaying, nectarMode, isHovered]);

  // Update time
  useEffect(() => {
    const updateTime = () => {
      const now = new Date();
      setTime(
        now.toLocaleTimeString([], {
          hour: "2-digit",
          minute: "2-digit",
          hour12: !timeFormat24h,
        })
      );
    };

    updateTime();
    const interval = setInterval(updateTime, 1000);

    // Toggle compact timer view every 5 seconds if running
    let timerToggleInterval: any;
    if ((isTimerRunning || stopwatch.running) && nectarMode !== 'calendar') {
      timerToggleInterval = setInterval(() => {
        setIsCompactTimerVisible(prev => !prev);
      }, 5000);
    } else {
      setIsCompactTimerVisible(false);
    }

    return () => {
      clearInterval(interval);
      if (timerToggleInterval) clearInterval(timerToggleInterval);
    };
  }, [isTimerRunning, stopwatch.running, nectarMode, timeFormat24h]);

  // Battery API
  useEffect(() => {
    let battery: any = null;

    const initBattery = async () => {
      try {
        battery = await (navigator as any).getBattery();

        const updateBattery = () => {
          setBatteryLevel(Math.round(battery.level * 100));
          setIsCharging(battery.charging);
        };

        updateBattery();

        battery.addEventListener("levelchange", updateBattery);
        battery.addEventListener("chargingchange", updateBattery);

        return () => {
          battery.removeEventListener("levelchange", updateBattery);
          battery.removeEventListener("chargingchange", updateBattery);
        };
      } catch (e) {
        // Battery API not supported
      }
    };

    initBattery();
  }, []);

  // Listen for Volume Changes
  useEffect(() => {
    const unlisten = listen<{ volume: number; is_muted: boolean }>("volume-change", (event) => {
      setVolume(event.payload.volume);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Load wifi/bluetooth/volume/brightness state on mount
  useEffect(() => {
    invoke<boolean>("get_wifi_state").then(setWifiEnabled).catch(() => { });
    invoke<boolean>("get_bluetooth_state").then(setBluetoothEnabled).catch(() => { });
    invoke<boolean>("get_battery_saver_state").then(setBatterySaverEnabled).catch(() => { });
    invoke<boolean>("has_battery").then(setHasBattery).catch(() => { });
    invoke<number>("get_volume").then(setVolume).catch(() => { });
    invoke<number>("get_brightness").then(setCurrentBrightness).catch(() => { });

    // Poll battery saver state every 5s (since we can't listen for changes)
    const interval = setInterval(() => {
      invoke<boolean>("get_battery_saver_state").then(setBatterySaverEnabled).catch(() => { });
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  // Poll system metrics for status widgets
  useEffect(() => {
    const fetchMetrics = () => {
      invoke<number>("get_cpu_usage").then(setCpuUsage).catch((e) => console.warn("CPU:", e));
      invoke<number>("get_ram_usage").then(setRamUsage).catch((e) => console.warn("RAM:", e));
      invoke<number>("get_disk_space").then(setDiskSpace).catch((e) => console.warn("Disk:", e));
      invoke<[number, number]>("get_network_speed").then(([up, down]) => {
        setNetUpSpeed(up);
        setNetDownSpeed(down);
      }).catch(() => {});
    };
    fetchMetrics();
    const interval = setInterval(fetchMetrics, 3000);
    return () => clearInterval(interval);
  }, []);

  // Listen for brightness changes
  useEffect(() => {
    const unlisten = listen<{ brightness: number }>("brightness-change", (event) => {
      setCurrentBrightness(event.payload.brightness);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Native Windows Media Controls - Listen for updates from background worker
  useEffect(() => {
    const unlisten = listen<MediaInfo>("media-update", (event) => {
      const info = event.payload;
      if (!info) return;

      setMediaInfo(prev => {
        // Find if artwork changed by checking the first element
        const prevArt = prev.artwork?.[0];
        const nextArt = info.artwork?.[0];
        const artChanged = prevArt !== nextArt;

        if (prev.title === info.title &&
          prev.artist === info.artist &&
          prev.is_playing === info.is_playing &&
          prev.has_media === info.has_media &&
          !artChanged &&
          prev.position_ms === info.position_ms &&
          prev.duration_ms === info.duration_ms) {
          return prev;
        }

        // Update playing state separately for the hook triggers
        setIsPlaying(info.is_playing);

        if (info.artwork && info.artwork.length > 0) {
          const newArt = info.artwork[0];
          setAlbumArtUrl(prev => {
            if (prev !== newArt) {
              setAlbumArtKey(k => k + 1);
              return newArt;
            }
            return prev;
          });
        } else {
          setAlbumArtUrl(null);
        }

        return info;
      });
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  // Media controls via Tauri commands
  /* Unused saveAndBroadcast removed to fix TS build error */
  const togglePlayPause = useCallback(async () => {
    try {
      await invoke("media_play_pause");
      setIsPlaying(!isPlaying);
    } catch (e) {
      console.error("Failed to toggle play/pause:", e);
    }
  }, [isPlaying]);


  const skipNext = useCallback(async () => {
    try {
      await invoke("media_next");
    } catch (e) {
      console.error("Failed to skip next:", e);
    }
  }, []);

  const skipPrevious = useCallback(async () => {
    try {
      await invoke("media_previous");
    } catch (e) {
      console.error("Failed to skip previous:", e);
    }
  }, []);

  // Slide-push animation for prev/next buttons
  const prevFront = useAnimation();
  const prevBack = useAnimation();
  const nextFront = useAnimation();
  const nextBack = useAnimation();

  const animatePrev = useCallback(async () => {
    prevFront.set({ x: 0 });
    prevBack.set({ x: 30 });
    prevFront.start({ x: -30, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
    await prevBack.start({ x: 0, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
    skipPrevious();
  }, [skipPrevious, prevFront, prevBack]);

  const animateNext = useCallback(async () => {
    nextFront.set({ x: 0 });
    nextBack.set({ x: -30 });
    nextFront.start({ x: 30, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
    await nextBack.start({ x: 0, transition: { duration: 0.18, ease: [0.4, 0, 0.2, 1] } });
    skipNext();
  }, [skipNext, nextFront, nextBack]);

  const lastVolumeCallRef = useRef(0);

  const handleVolumeChange = useCallback((newVol: number) => {
    setVolume(newVol);

    const now = Date.now();
    if (now - lastVolumeCallRef.current < 50) return;
    lastVolumeCallRef.current = now;

    invoke("set_volume", { volume: newVol }).catch(() => {});
  }, []);


  // Open WiFi settings
  const openWifiSettings = useCallback(async () => {
    try {
      await invoke("open_wifi_settings");
    } catch (e) {
      console.error("Failed to open WiFi settings:", e);
    }
  }, []);

  // WiFi toggle
  const toggleWifi = useCallback(async () => {
    const newState = !wifiEnabled;
    setWifiEnabled(newState);
    try {
      await invoke("set_wifi_state", { enabled: newState });
    } catch (e) {
      setWifiEnabled(!newState);
      console.error("Failed to toggle WiFi:", e);
    }
  }, [wifiEnabled]);

  // Bluetooth toggle
  const toggleBluetooth = useCallback(async () => {
    const newState = !bluetoothEnabled;
    setBluetoothEnabled(newState);
    try {
      await invoke("set_bluetooth_state", { enabled: newState });
    } catch (e) {
      setBluetoothEnabled(!newState);
      console.error("Failed to toggle Bluetooth:", e);
    }
  }, [bluetoothEnabled]);

  // Battery Saver - opens settings (no public API to toggle without admin)
  const openBatterySaverSettings = useCallback(async () => {
    try {
      await invoke("open_battery_saver_settings");
    } catch (e) {
      console.error("Failed to open Battery Saver settings:", e);
    }
  }, []);


  // Brightness change with throttling
  const lastBrightnessCallRef = useRef(0);

  const handleBrightnessChange = useCallback((newVal: number) => {
    setCurrentBrightness(newVal);

    const now = Date.now();
    if (now - lastBrightnessCallRef.current < 50) return;
    lastBrightnessCallRef.current = now;

    invoke("set_brightness", { brightness: newVal }).catch(() => {});
  }, []);

  const openSettingsWindow = useCallback(async () => {
    try {
      await invoke("open_settings_window");
    } catch (e) {
      console.error("Failed to open settings window:", e);
    }
  }, []);

  const handleWifiRightClick = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    openWifiSettings();
  }, [openWifiSettings]);

  const toggleDockModeSetting = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    const nextMode = dockMode === "fixed" ? "smart" : dockMode === "smart" ? "peek" : "fixed";
    setDockMode(nextMode);
    localStorage.setItem("nectar-dock-mode", nextMode);
    invoke("save_setting", { key: "nectar-dock-mode", value: nextMode }).catch(console.error);
    try {
      await invoke("change_dock_mode", { mode: nextMode });
    } catch (err) {
      console.error("Failed to change dock mode:", err);
    }
  }, [dockMode]);

  const toggleNotchModeSetting = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    const nextMode = notchMode === "fixed" ? "smart" : notchMode === "smart" ? "peek" : "fixed";
    setNotchMode(nextMode);
    localStorage.setItem("nectar-notch-mode", nextMode);
    invoke("save_setting", { key: "nectar-notch-mode", value: nextMode }).catch(console.error);
    try {
      await invoke("change_notch_mode", { mode: nextMode });
    } catch (err) {
      console.error("Failed to change notch mode:", err);
    }
  }, [notchMode]);

  const handleBluetoothRightClick = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    invoke("open_bluetooth_settings");
  }, []);



  const toggleCalendarMode = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isTimerFinished) {
      resetTimer();
      return;
    }
    if (!productivityEnabled) return;

    setNectarMode(prev => {
      if (prev === 'calendar') {
        // Return to music mode if media is present and playing and music mode is enabled, otherwise status
        return (settingsMusicModeEnabled && mediaInfo.has_media && isPlaying) ? 'music' : 'status';
      }
      return 'calendar';
    });
  };

  // Render a status widget by ID
  const renderStatusWidget = (id: string) => {
    switch (id) {
      case "weather":
        if (!settingsWeatherEnabled || temperature === null) return null;
        return (
          <div className="passive-feature" key="weather" data-tooltip={cityName ? `${weatherCondition} — ${cityName}` : weatherCondition}>
            <WeatherIcon size={12} strokeWidth={2.2} />
            <span className="label">{temperature}°{tempUnit === "fahrenheit" ? "F" : "C"}</span>
          </div>
        );
      case "battery":
        if (!hasBattery) return null;
        return (
          <div className="passive-feature" key="battery">
            <BatteryIcon charging={isCharging} level={batteryLevel} threshold={lowBatteryThreshold} />
            <span className="label">{batteryLevel}%</span>
          </div>
        );
      case "cpu":
        return (
          <div className="passive-feature" key="cpu" data-tooltip="CPU Usage">
            <Cpu size={12} strokeWidth={2} />
            <span className="label">{cpuUsage}%</span>
          </div>
        );
      case "ram":
        return (
          <div className="passive-feature" key="ram" data-tooltip="RAM Usage">
            <MemoryStick size={12} strokeWidth={2} />
            <span className="label">{Math.round(ramUsage)}%</span>
          </div>
        );
      case "disk":
        return (
          <div className="passive-feature" key="disk" data-tooltip="Free Disk Space">
            <HardDrive size={12} strokeWidth={2} />
            <span className="label">{diskSpace}GB</span>
          </div>
        );
      case "net":
        return (
          <div className="passive-feature" key="net" data-tooltip="Network Speed">
            <ArrowUpDown size={12} strokeWidth={2} />
            <span className="label">↑{formatBytes(netUpSpeed)} ↓{formatBytes(netDownSpeed)}</span>
          </div>
        );
      default:
        return null;
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
  };

  // Music mode shows any time we have media info (playing or paused) and music mode setting is enabled
  const isMusicMode = mediaInfo.has_media && nectarMode === 'music' && settingsMusicModeEnabled;

  // Calculate width dynamically based on enabled features
  const getDynamicWidth = () => {
    if (isCalendarMode) return settingsCalendarEnabled && showClockColumn ? 480 : settingsCalendarEnabled ? 310 : 270;
    if (nectarMode === 'command-center' && isHovered) {
      const totalWidgets = statusWidgets.left.length + statusWidgets.right.length;
      return Math.max(350, Math.min(200 + totalWidgets * 65, 460));
    }
    if (nectarMode === 'status' && isHovered) {
      const totalWidgets = statusWidgets.left.length + statusWidgets.right.length;
      return Math.min(200 + totalWidgets * 65, 460);
    }
    if (isMusicMode && isHovered) return mediaLayout === 'compact' ? 300 : 340;
    if ((showPowerPulse || showLowBatteryPulse || showUpdatePulse) && !isHovered) return 200;

    let w = 140;
    if (isMusicMode) {
      w = 140;
      if (settingsVisualizerEnabled && isPlaying) w += 30;
      if (settingsAlbumArtEnabled) w += 30;

      if (isHovered) {
        w += 60;
      }
    }

    return w;
  };

  const getDynamicHeight = () => {
    if (!isExpanded || isHidden) {
      return isImpacted ? 28.9 : 44.2;
    }
    if (nectarMode === 'calendar') return settingsCalendarEnabled ? (caldav.connected ? 350 : 310) : 262;
    if (nectarMode === 'command-center') return isHovered ? 230 : 36;
    if (nectarMode === 'status') return 36;
    if (isMusicMode && isHovered) {
      const hasProgressBar = (mediaInfo.duration_ms ?? 0) > 0;
      let h = mediaLayout === 'compact' ? (hasProgressBar ? 132 : 116) : 120;
      if (mediaLayout === 'compact') {
        if (compactVolumeExpanded) h += 36;
      }
      return h;
    }
    return 36;
  };

  const isCalendarMode = nectarMode === 'calendar';

  // Close compact media player expansions when notch is unhovered or mode changes
  useEffect(() => {
    if (mediaLayout === 'compact') {
      setCompactVolumeExpanded(false);
    }
  }, [isHovered, mediaLayout, nectarMode]);




  return (
    <div className="screen" style={{ overflow: 'hidden' }}>
      {/* Screen Corners (Top) */}
      <AnimatePresence>
        {isVisible && settingsCornersEnabled && (
          <>
            <motion.div
              className="screen-corner top-left"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, filter: "blur(0px)" }}
              exit={{ opacity: 0, filter: "blur(10px)" }}
            />
            <motion.div
              className="screen-corner top-right"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, filter: "blur(0px)" }}
              exit={{ opacity: 0, filter: "blur(10px)" }}
            />
          </>
        )}
      </AnimatePresence>

      <div style={{ zoom: scale, width: '100%', display: 'flex', justifyContent: 'center' }}>
        <motion.div
          ref={nectarRef}
          className={`nectar ${isHovered ? 'expanded' : ''} ${isImpacted ? 'is-impacted' : ''}`}
          onMouseEnter={() => setIsNotchHovered(true)}
        onMouseLeave={() => setIsNotchHovered(false)}
        onWheel={handleWheel}
        initial={{ y: 250, width: 30.6, height: 44.2, borderTopLeftRadius: 18, borderTopRightRadius: 18, borderBottomLeftRadius: 18, borderBottomRightRadius: 18, scaleX: 1, scaleY: 1, opacity: 0 }}
        animate={{
          y: !isReady ? 250 : (isHidden ? -100 : 0),
          width: !isReady ? 34 : (isExpanded && !isHidden ? getDynamicWidth() : (isImpacted ? 39.1 : 30.6)),
          height: !isReady ? 34 : getDynamicHeight(),
          opacity: isReady ? 1 : 0,
          scaleX: 1,
          scaleY: 1,
          borderTopLeftRadius: isImpacted ? 0 : 18,
          borderTopRightRadius: isImpacted ? 0 : 18,
          borderBottomLeftRadius: isCalendarMode ? 28 : 18,
          borderBottomRightRadius: isCalendarMode ? 28 : 18,
          filter: "blur(0px)",
          pointerEvents: 'auto'
        }}
        onClick={(e) => {
          e.stopPropagation();
        }}
        onHoverStart={() => {
          setIsHovered(true);
          setNectarMode(mediaInfo.has_media && isPlaying ? 'music' : 'status');
        }}
        onHoverEnd={() => {
          setIsHovered(false);
          const targetMode = mediaInfo.has_media && isPlaying && settingsMusicCompactNotch ? 'music' : 'status';
          if (nectarMode === 'music') {
            setNectarMode(targetMode);
          } else if (nectarMode === 'command-center' || nectarMode === 'calendar' || nectarMode === 'status') {
            setNectarMode(targetMode);
          }
        }}
        style={{ originY: 0 }}
        transition={{
          width: { type: "spring", stiffness: 400, damping: 31 },
          height: { type: "spring", stiffness: 450, damping: 29 },
          y: { type: "spring", stiffness: 550, damping: 45, mass: 0.8, restDelta: 0.001 },
          opacity: { duration: 0.2 },
          borderTopLeftRadius: { type: "spring", stiffness: 1000, damping: 40 },
          borderTopRightRadius: { type: "spring", stiffness: 1000, damping: 40 },
          borderBottomLeftRadius: { type: "spring", stiffness: 1000, damping: 40 },
          borderBottomRightRadius: { type: "spring", stiffness: 1000, damping: 40 },
          default: { type: "spring", stiffness: 500, damping: 30, mass: 1 }
        }}
      >

        <AnimatePresence>
          {isMusicMode && settingsAmbienceEnabled && albumArtUrl && isHovered && !isCalendarMode && (
            <motion.div
              className="notch-ambient-glow"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.15 }}
            >
              <AnimatePresence mode="wait">
                <motion.img
                  key={albumArtUrl}
                  src={albumArtUrl}
                  alt=""
                  draggable={false}
                  initial={{ opacity: 0, scale: 1.1 }}
                  animate={{ opacity: 1, scale: 1.8 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.3 }}
                />
              </AnimatePresence>
            </motion.div>
          )}
        </AnimatePresence>
        <AnimatePresence mode="wait">
          {isExpanded && (
            <motion.div
              key="nectar-content"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.1 }}
              style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', alignItems: 'center', position: 'relative', borderRadius: 'inherit' }}
            >
              {/* Faster Waiting Transition Area */}
              <AnimatePresence mode="wait">
                {isHovered && isMusicMode && !isCalendarMode ? (
                  <motion.div
                    key="expanded-music"
                    className="expanded-music-container"
                    initial={{ opacity: 0, scale: 0.98 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.98, transition: { duration: 0.1 } }}
                    transition={{ type: "spring", stiffness: 500, damping: 30 }}
                  >
                    {mediaLayout === 'compact' ? (
                      <CompactMediaPlayer
                        mediaInfo={mediaInfo}
                        albumArtUrl={albumArtUrl}
                        albumArtKey={albumArtKey}
                        isPlaying={isPlaying}
                        volume={volume}
                        volumeExpanded={compactVolumeExpanded}
                        onVolumeExpandedChange={setCompactVolumeExpanded}
                        onTogglePlayPause={togglePlayPause}
                        onVolumeChange={handleVolumeChange}
                        prevFront={prevFront}
                        prevBack={prevBack}
                        nextFront={nextFront}
                        nextBack={nextBack}
                        onAnimatePrev={animatePrev}
                        onAnimateNext={animateNext}
                        onLayoutChange={(layout) => {
                          setMediaLayout(layout);
                          localStorage.setItem("nectar-media-layout", layout);
                          window.dispatchEvent(new CustomEvent("settings-changed", { detail: { key: "media-layout", value: layout } }));
                        }}
                      />
                    ) : (
                    <div className="compact-premium-layout">
                      <div className="album-art-section">
                        <motion.div
                          className="premium-album-art"
                          whileHover={{ scale: 1.05 }}
                          whileTap={{ scale: 0.95 }}
                          onClick={(e) => {
                            e.stopPropagation();
                            setMediaLayout('compact');
                            localStorage.setItem("nectar-media-layout", "compact");
                            window.dispatchEvent(new CustomEvent("settings-changed", { detail: { key: "media-layout", value: "compact" } }));
                          }}
                          style={{ cursor: 'pointer' }}
                        >
                          <AnimatePresence mode="wait" initial={false}>
                            {albumArtUrl ? (
                              <motion.img
                                key={`art-${albumArtKey}`}
                                src={albumArtUrl}
                                alt="Art"
                                initial={{ rotateY: 90, opacity: 0 }}
                                animate={{ rotateY: 0, opacity: 1 }}
                                exit={{ rotateY: -90, opacity: 0 }}
                                transition={{ duration: 0.3, ease: "easeInOut" }}
                                style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                              />
                            ) : (
                              <motion.div
                                key="placeholder"
                                className="art-placeholder-mini"
                                initial={{ rotateY: 90, opacity: 0 }}
                                animate={{ rotateY: 0, opacity: 1 }}
                                exit={{ rotateY: -90, opacity: 0 }}
                                transition={{ duration: 0.3, ease: "easeInOut" }}
                              >
                                <MusicNoteIcon size={32} className="music-placeholder-svg" />
                              </motion.div>
                            )}
                          </AnimatePresence>
                        </motion.div>
                      </div>

                      <div className="metadata-controls-section-middle">
                        <div className="track-header-row">
                          <div className="track-info-middle">
                            <TitleMarquee title={mediaInfo.title} />
                            <span className="premium-artist">{mediaInfo.artist}</span>
                          </div>
                          <div className="header-visualizer-wrap">
                            <div className="header-visualizer">
                              <Visualizer isPlaying={isPlaying} bars={5} height={18} />
                            </div>
                            <motion.button
                              className="classic-audio-output-btn"
                              onClick={(e) => { e.stopPropagation(); invoke('open_sound_settings').catch(() => {}); }}
                              whileTap={{ scale: 0.9 }}
                              data-tooltip="Audio Output"
                            >
                              <AudioOutputIcon size={20} style={{ opacity: 0.5 }} />
                            </motion.button>
                          </div>
                        </div>

                        <div className="controls-row-sleek">
                          <motion.button
                            className="sleek-btn previous-btn"
                            onClick={(e) => { e.stopPropagation(); animatePrev(); }}
                            whileHover={{ scale: 1.2 }}
                            whileTap={{ scale: 0.9 }}
                            transition={{ type: "spring", stiffness: 400, damping: 25 }}
                          >
                            <div style={{ position: 'relative', width: 32, height: 16, overflow: 'hidden' }}>
                              <motion.div animate={prevBack} style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <SkipBackIcon size={32} />
                              </motion.div>
                              <motion.div animate={prevFront} style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <SkipBackIcon size={32} />
                              </motion.div>
                            </div>
                          </motion.button>

                          <motion.button
                            className="sleek-btn play-pause-btn-floating"
                            onClick={(e) => { e.stopPropagation(); togglePlayPause(); }}
                            whileHover={{ scale: 1.2 }}
                            whileTap={{ scale: 0.95 }}
                            transition={{ type: "spring", stiffness: 400, damping: 25 }}
                          >
                            <AnimatePresence mode="wait" initial={false}>
                              <motion.div
                                key={isPlaying ? "pause" : "play"}
                                initial={{ opacity: 0, scale: 0.8 }}
                                animate={{ opacity: 1, scale: 1 }}
                                exit={{ opacity: 0, scale: 0.8 }}
                                transition={{ duration: 0.15 }}
                                style={{ display: 'flex', alignItems: 'center', justifyContent: 'center' }}
                              >
                                {isPlaying ? <PauseIcon size={26} /> : <PlayIcon size={26} />}
                              </motion.div>
                            </AnimatePresence>
                          </motion.button>

                          <motion.button
                            className="sleek-btn next-btn"
                            onClick={(e) => { e.stopPropagation(); animateNext(); }}
                            whileHover={{ scale: 1.2 }}
                            whileTap={{ scale: 0.9 }}
                            transition={{ type: "spring", stiffness: 400, damping: 25 }}
                          >
                            <div style={{ position: 'relative', width: 32, height: 16, overflow: 'hidden' }}>
                              <motion.div animate={nextBack} style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <SkipForwardIcon size={32} />
                              </motion.div>
                              <motion.div animate={nextFront} style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <SkipForwardIcon size={32} />
                              </motion.div>
                            </div>
                          </motion.button>
                        </div>

                        <div className="volume-slider-container">
                          <VolumeLowIcon size={12} style={{ opacity: 0.5 }} />
                          <div className="slider-track-premium">
                            <input
                              type="range"
                              min="0"
                              max="1"
                              step="0.01"
                              value={volume}
                              onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
                              onPointerDown={(e) => e.stopPropagation()}
                              onClick={(e) => e.stopPropagation()}
                              className="premium-slider"
                            />
                            <div className="slider-progress-fill" style={{ width: `${volume * 100}%` }} />
                          </div>
                          <VolumeHighIcon size={14} style={{ opacity: 0.5 }} />
                        </div>
                      </div>

                    </div>
                    )}
                  </motion.div>
                ) : (
                  <motion.div
                    key="standard-view-group"
                    initial={{ opacity: 0, y: -5 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: 5, transition: { duration: 0.1 } }}
                    transition={{ duration: 0.2 }}
                    style={{ width: '100%' }}
                  >
                    <div className="main-row">
                      <AnimatePresence mode="wait">
                        {(showPowerPulse || showLowBatteryPulse || showUpdatePulse) && !isHovered ? (
                          showUpdatePulse ? (
                            <motion.div
                              key="update-pulse-view"
                              initial={{ opacity: 0, scale: 0.95, filter: "blur(4px)" }}
                              animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
                              exit={{ opacity: 0, scale: 1.05, filter: "blur(4px)" }}
                              className="power-pulse-content"
                            >
                              <GreenDownArrowIcon />
                              <span className="label" style={{ color: "#32D74B" }}>
                                Update Available
                              </span>
                            </motion.div>
                          ) : (
                            <motion.div
                              key="pulse-view"
                              initial={{ opacity: 0, scale: 0.95, filter: "blur(4px)" }}
                              animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
                              exit={{ opacity: 0, scale: 1.05, filter: "blur(4px)" }}
                              className="power-pulse-content"
                            >
                              {updateAvailable && <GreenDownArrowIcon />}
                              <BatteryIcon charging={isCharging} level={batteryLevel} threshold={lowBatteryThreshold} />
                              <span className="label" style={{ color: showLowBatteryPulse ? "#FF453A" : "inherit" }}>
                                {showLowBatteryPulse ? "Low Battery" : (isCharging ? "Charging" : "On Battery")} • {batteryLevel}%
                              </span>
                            </motion.div>
                          )
                        ) : (
                          <motion.div
                            key="standard-view"
                            className="main-row-inner"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            exit={{ opacity: 0 }}
                          >
                            {/* Left: update activity (takes priority), visualizer (music), or weather (command-center, calendar) */}
                            <div className="side-content left">
                              {updateAvailable && showUpdateIndicator ? (
                                <AnimatePresence>
                                  <motion.div
                                    key="update-activity"
                                    initial={{ scale: 0.8, opacity: 0 }}
                                    animate={{ scale: 1, opacity: 1 }}
                                    exit={{ scale: 0.8, opacity: 0 }}
                                  >
                                    <UpdateActivity />
                                  </motion.div>
                                </AnimatePresence>
                              ) : isMusicMode && settingsVisualizerEnabled ? (
                                <AnimatePresence>
                                  {settingsVisualizerEnabled && (
                                    <motion.div
                                      key="visualizer"
                                      initial={{ scale: 0.8, opacity: 0 }}
                                      animate={{ scale: 1, opacity: 1 }}
                                      exit={{ scale: 0.8, opacity: 0 }}
                                    >
                                      <Visualizer isPlaying={isPlaying} />
                                    </motion.div>
                                  )}
                                </AnimatePresence>
                              ) : (!isMusicMode && isHovered && statusWidgets.left.length > 0) ? (
                                <motion.div
                                  key="left-widgets"
                                  className="passive-features-group"
                                  initial={{ opacity: 0 }}
                                  animate={{ opacity: 1 }}
                                  exit={{ opacity: 0 }}
                                  transition={{ duration: 0.2 }}
                                >
                                  {statusWidgets.left.map(renderStatusWidget)}
                                </motion.div>
                              ) : null}
                            </div>

                            {/* Center - Time (always visible) */}
                            <div className="time-center">
                              <div className="time-flip-container" onClick={toggleCalendarMode}>
                                <AnimatePresence initial={false}>
                                  {isCompactTimerVisible || isTimerFinished ? (
                                    <motion.span
                                      key="timer"
                                      className={`time compact-timer ${isTimerFinished ? 'timer-finished' : ''}`}
                                      initial={{ rotateX: -90, opacity: 0 }}
                                      animate={{ rotateX: 0, opacity: 1 }}
                                      exit={{ rotateX: 90, opacity: 0 }}
                                      transition={{ type: "spring", stiffness: 600, damping: 30 }}
                                    >
                                      {isTimerRunning || isTimerFinished
                                        ? formatTimerTime(timerSeconds)
                                        : <StopwatchReadout base={stopwatch.base} startedAt={stopwatch.startedAt} precise={false} />}
                                    </motion.span>
                                  ) : (
                                    <motion.span
                                      key="clock"
                                      className="time"
                                      initial={{ rotateX: -90, opacity: 0 }}
                                      animate={{ rotateX: 0, opacity: 1 }}
                                      exit={{ rotateX: 90, opacity: 0 }}
                                      transition={{ type: "spring", stiffness: 600, damping: 30 }}
                                    >
                                      {time}
                                    </motion.span>
                                  )}
                                </AnimatePresence>
                              </div>
                              {updateAvailable && showUpdateIndicator && (
                                <div className="update-pill-indicator">
                                  <svg width="7" height="7" viewBox="0 0 24 24" fill="none" stroke="#0b0b0c" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round">
                                    <path d="M12 5v11" />
                                    <path d="M6 12l6 6 6-6" />
                                  </svg>
                                </div>
                              )}
                            </div>

                            {/* Right: album art (music) or battery (command-center, calendar) */}
                            <div className="side-content right">
                              {isMusicMode && settingsAlbumArtEnabled ? (
                                <AnimatePresence mode="wait">
                                  <motion.div
                                    key="album-art"
                                    className="album-art-glow-wrapper"
                                    initial={{ opacity: 0, scale: 0.8 }}
                                    animate={{ opacity: 1, scale: 1 }}
                                    exit={{ opacity: 0, y: -20, scale: 0.8, filter: "blur(8px)" }}
                                    transition={{ duration: 0.12 }}
                                  >
                                    {albumArtUrl && settingsCompactGlowEnabled && (
                                      <img
                                        src={albumArtUrl}
                                        alt=""
                                        className="album-art-glow-bg"
                                        draggable={false}
                                      />
                                    )}
                                    <button
                                      className={`album-art${isHovered ? ' album-art-large' : ''}${!isPlaying ? ' paused' : ''}`}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        togglePlayPause();
                                      }}
                                      onDoubleClick={(e) => {
                                        e.stopPropagation();
                                        skipNext();
                                      }}
                                      onContextMenu={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        skipPrevious();
                                      }}
                                    >
                                      <div className="album-art-inner">
                                        <AnimatePresence mode="wait" initial={false}>
                                          {albumArtUrl ? (
                                            <motion.img
                                              key={`compact-art-${albumArtKey}`}
                                              src={albumArtUrl}
                                              alt="Art"
                                              draggable={false}
                                              initial={{ rotateY: 90, opacity: 0 }}
                                              animate={{ rotateY: 0, opacity: 1 }}
                                              exit={{ rotateY: -90, opacity: 0 }}
                                              transition={{ duration: 0.25, ease: "easeInOut" }}
                                              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                                            />
                                          ) : (
                                            <motion.div
                                              key="compact-placeholder"
                                              className="album-art-placeholder"
                                              initial={{ rotateY: 90, opacity: 0 }}
                                              animate={{ rotateY: 0, opacity: 1 }}
                                              exit={{ rotateY: -90, opacity: 0 }}
                                              transition={{ duration: 0.25, ease: "easeInOut" }}
                                            >
                                              <MusicNoteIcon className="music-placeholder-svg-small" />
                                          </motion.div>
                                        )}
                                      </AnimatePresence>
                                      <div className="album-art-overlay">
                                        <div className="control-icon-small">
                                          {isPlaying ? <PauseIcon /> : <PlayIcon />}
                                        </div>
                                      </div>
                                    </div>
                                  </button>
                                  </motion.div>
                                </AnimatePresence>
                              ) : (!isMusicMode && isHovered && statusWidgets.right.length > 0) ? (
                                <motion.div
                                  key="right-widgets"
                                  className="passive-features-group"
                                  initial={{ opacity: 0 }}
                                  animate={{ opacity: 1 }}
                                  exit={{ opacity: 0 }}
                                  transition={{ duration: 0.2 }}
                                >
                                  {statusWidgets.right.map(renderStatusWidget)}
                                </motion.div>
                              ) : null}
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Command Center Panel */}
              <AnimatePresence>
                {nectarMode === 'command-center' && (
                  <motion.div
                    className="command-center-content-minimal"
                    onClick={e => e.stopPropagation()}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0, filter: "blur(4px)", transition: { duration: 0.1 } }}
                    transition={{ type: "spring", stiffness: 400, damping: 30 }}
                  >
                    {/* Pills Grid */}
                    <div className="cc-pills-grid">
                      {/* Wi-Fi Pill */}
                      <div
                        className={`cc-pill-tile ${wifiEnabled ? 'active' : ''}`}
                        onClick={(e) => { e.stopPropagation(); toggleWifi(); }}
                        onContextMenu={handleWifiRightClick}
                      >
                        <div className="cc-pill-icon-wrapper">
                          <WifiIcon connected={wifiEnabled} />
                        </div>
                        <div className="cc-pill-info">
                          <span className="cc-pill-title">Wi-Fi</span>
                          <span className="cc-pill-status">{wifiEnabled ? 'Connected' : 'Off'}</span>
                        </div>
                      </div>

                      {/* Dock Mode Pill */}
                      <div
                        className={`cc-pill-tile ${dockMode === 'fixed' ? 'active' : ''}`}
                        onClick={toggleDockModeSetting}
                      >
                        <div className="cc-pill-icon-wrapper">
                          <DockIcon />
                        </div>
                        <div className="cc-pill-info">
                          <span className="cc-pill-title">Dock Mode</span>
                          <span className="cc-pill-status">{dockMode === 'fixed' ? 'Fixed' : dockMode === 'smart' ? 'Smart' : 'Peek'}</span>
                        </div>
                      </div>

                      {/* Bluetooth Pill */}
                      <div
                        className={`cc-pill-tile ${bluetoothEnabled ? 'active' : ''}`}
                        onClick={(e) => { e.stopPropagation(); toggleBluetooth(); }}
                        onContextMenu={handleBluetoothRightClick}
                      >
                        <div className="cc-pill-icon-wrapper">
                          <BluetoothIcon />
                        </div>
                        <div className="cc-pill-info">
                          <span className="cc-pill-title">Bluetooth</span>
                          <span className="cc-pill-status">{bluetoothEnabled ? 'On' : 'Off'}</span>
                        </div>
                      </div>

                      {/* Notch Mode Pill */}
                      <div
                        className={`cc-pill-tile ${notchMode === 'fixed' ? 'active' : ''}`}
                        onClick={toggleNotchModeSetting}
                      >
                        <div className="cc-pill-icon-wrapper">
                          <NotchIcon />
                        </div>
                        <div className="cc-pill-info">
                          <span className="cc-pill-title">Notch Mode</span>
                          <span className="cc-pill-status">{notchMode === 'fixed' ? 'Fixed' : notchMode === 'smart' ? 'Smart' : 'Peek'}</span>
                        </div>
                      </div>
                    </div>

                    {/* Circular Actions Row */}
                    <div className="cc-circular-actions-row">
                      <button
                        className={`cc-circular-btn ${dndActive ? 'active' : ''}`}
                        onClick={(e) => { e.stopPropagation(); setDndActive(prev => !prev); }}
                      >
                        <MoonIcon />
                      </button>
                      <button
                        className={`cc-circular-btn ${batterySaverEnabled ? 'active' : ''}`}
                        onClick={(e) => { e.stopPropagation(); openBatterySaverSettings(); }}
                      >
                        <BatterySaverIcon />
                      </button>
                      <button
                        className="cc-circular-btn"
                        onClick={(e) => { e.stopPropagation(); invoke("open_tray_window"); }}
                      >
                        <TrayIcon />
                      </button>
                      <button
                        className="cc-circular-btn"
                        onClick={(e) => { e.stopPropagation(); invoke("open_notification_center"); }}
                      >
                        <BellIcon />
                      </button>
                      <button
                        className="cc-circular-btn"
                        onClick={(e) => { e.stopPropagation(); openSettingsWindow(); }}
                      >
                        <SettingsIcon />
                      </button>
                      <button
                        className="cc-circular-btn"
                        onClick={(e) => { e.stopPropagation(); invoke("restart_nectar"); }}
                      >
                        <ReloadIcon />
                      </button>
                    </div>

                    {/* Classic Sliders Area */}
                    <div className="cc-classic-sliders-area">
                      {/* Volume Slider */}
                      <div className="cc-classic-slider-row">
                        <div className="cc-classic-slider-label">
                          <VolumeLowIcon style={{ opacity: 0.5 }} />
                          <span>Volume</span>
                        </div>
                        <div className="cc-classic-slider-track">
                          <input
                            type="range"
                            min="0"
                            max="1"
                            step="0.01"
                            value={volume}
                            onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
                            onPointerDown={(e) => e.stopPropagation()}
                            onClick={(e) => e.stopPropagation()}
                            className="cc-classic-input"
                          />
                          <div className="cc-classic-fill" style={{ width: `${volume * 100}%` }} />
                        </div>
                        <span className="cc-classic-percentage">{Math.round(volume * 100)}%</span>
                      </div>

                      {/* Brightness Slider */}
                      <div className="cc-classic-slider-row">
                        <div className="cc-classic-slider-label">
                          <BrightnessLowIcon />
                          <span>Brightness</span>
                        </div>
                        <div className="cc-classic-slider-track">
                          <input
                            type="range"
                            min="0"
                            max="100"
                            step="1"
                            value={currentBrightness}
                            onChange={(e) => handleBrightnessChange(parseInt(e.target.value))}
                            onPointerDown={(e) => e.stopPropagation()}
                            onClick={(e) => e.stopPropagation()}
                            className="cc-classic-input"
                          />
                          <div className="cc-classic-fill" style={{ width: `${currentBrightness}%` }} />
                        </div>
                        <span className="cc-classic-percentage">{currentBrightness}%</span>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Calendar & Timer Split View */}
              <AnimatePresence>
                {productivityEnabled && isCalendarMode && (
                  <motion.div
                    className="calendar-timer-content split-view"
                    onClick={e => e.stopPropagation()} /* Block mode switches when clicking inside */
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0, filter: "blur(4px)", transition: { duration: 0.1 } }}
                    transition={{ type: "spring", stiffness: 400, damping: 30 }}
                  >
                    {settingsCalendarEnabled && (
                      <div className="calendar-column">
                        <NotchCalendar caldav={caldav} hour12={!timeFormat24h} />
                      </div>
                    )}

                    {showClockColumn && (
                      <NotchClock
                        timerEnabled={settingsTimerEnabled}
                        stopwatchEnabled={settingsStopwatchEnabled}
                        solo={!settingsCalendarEnabled}
                        timerSeconds={timerSeconds}
                        isTimerRunning={isTimerRunning}
                        formatTimerTime={formatTimerTime}
                        onStartTimer={startTimerSeconds}
                        onToggleTimer={toggleTimer}
                        onResetTimer={resetTimer}
                        stopwatch={stopwatch}
                      />
                    )}
                  </motion.div>
                )}
              </AnimatePresence>

            </motion.div>
          )}
        </AnimatePresence>

      </motion.div>
    </div>

    </div>
  );
}

function BluetoothIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6.5 6.5l11 11L12 23V1l5.5 5.5-11 11" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.9 }}>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

function BrightnessLowIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.5 }}>
      <circle cx="12" cy="12" r="5" fill="currentColor" />
    </svg>
  );
}

function MoonIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  );
}

function DockIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="14" width="20" height="8" rx="2" />
      <line x1="6" y1="18" x2="6.01" y2="18" strokeWidth="3.5" strokeLinecap="round" />
      <line x1="10" y1="18" x2="10.01" y2="18" strokeWidth="3.5" strokeLinecap="round" />
      <line x1="14" y1="18" x2="14.01" y2="18" strokeWidth="3.5" strokeLinecap="round" />
      <line x1="18" y1="18" x2="18.01" y2="18" strokeWidth="3.5" strokeLinecap="round" />
    </svg>
  );
}

function NotchIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 3h16a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
      <path d="M9 9v4a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2V9" />
    </svg>
  );
}

function BellIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9z" />
      <path d="M13.73 21a2 2 0 0 1-3.46 0" />
    </svg>
  );
}

function ReloadIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
    </svg>
  );
}

function BatterySaverIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="7" width="16" height="10" rx="2" />
      <path d="M22 11v2" />
      <path d="M6 12h4l2-3v6l-2-3H6" />
    </svg>
  );
}

export default App;
