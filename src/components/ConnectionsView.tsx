import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, Lock, X, Bluetooth, Check, Speaker } from "lucide-react";

interface WifiNetwork {
  ssid: string;
  signal: number;
  secured: boolean;
  connected: boolean;
  known: boolean;
}

interface BluetoothDeviceInfo {
  id: string;
  name: string;
  connected: boolean;
}

interface OutputDevice {
  id: string;
  name: string;
  is_default: boolean;
}

interface Props {
  kind: "wifi" | "bluetooth" | "audio";
  onBack: () => void;
  onOpenWindowsSettings: () => void;
}

const REFRESH_MS = 6000;

function SignalBars({ signal }: { signal: number }) {
  const level = signal >= 75 ? 4 : signal >= 50 ? 3 : signal >= 25 ? 2 : 1;
  return (
    <span className="cv-signal" aria-label={`Signal ${signal}%`}>
      {[1, 2, 3, 4].map((bar) => (
        <i key={bar} className={bar <= level ? "on" : ""} style={{ height: 3 + bar * 2.5 }} />
      ))}
    </span>
  );
}

export function ConnectionsView({ kind, onBack, onOpenWindowsSettings }: Props) {
  const [networks, setNetworks] = useState<WifiNetwork[] | null>(null);
  const [devices, setDevices] = useState<BluetoothDeviceInfo[] | null>(null);
  const [outputs, setOutputs] = useState<OutputDevice[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [passwordFor, setPasswordFor] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [confirmRemove, setConfirmRemove] = useState<string | null>(null);

  useEffect(() => {
    if (passwordFor === null) return;
    invoke("set_window_keyboard_focus", { enabled: true }).catch(() => {});
    return () => {
      invoke("set_window_keyboard_focus", { enabled: false }).catch(() => {});
    };
  }, [passwordFor]);

  const refresh = useCallback(async () => {
    try {
      if (kind === "wifi") setNetworks(await invoke<WifiNetwork[]>("wifi_scan"));
      else if (kind === "audio") setOutputs(await invoke<OutputDevice[]>("get_audio_devices"));
      else setDevices(await invoke<BluetoothDeviceInfo[]>("bluetooth_devices"));
      setError("");
    } catch (e) {
      setError(String(e));
      if (kind === "wifi") setNetworks((prev) => prev ?? []);
      else if (kind === "audio") setOutputs((prev) => prev ?? []);
      else setDevices((prev) => prev ?? []);
    }
  }, [kind]);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  const run = async (label: string, action: () => Promise<unknown>) => {
    setBusy(label);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("");
      setTimeout(() => void refresh(), 1200);
    }
  };

  const joinNetwork = (network: WifiNetwork) => {
    if (network.connected) return;
    if (network.secured && !network.known) {
      setPasswordFor(network.ssid);
      setPassword("");
      return;
    }
    void run(network.ssid, () => invoke("wifi_connect", { ssid: network.ssid, password: null }));
  };

  const submitPassword = (e: React.FormEvent) => {
    e.preventDefault();
    if (!passwordFor || !password) return;
    const ssid = passwordFor;
    const key = password;
    setPasswordFor(null);
    setPassword("");
    void run(ssid, () => invoke("wifi_connect", { ssid, password: key }));
  };

  const list = kind === "wifi" ? networks : kind === "audio" ? outputs : devices;
  const loading = list === null;
  const empty = list?.length === 0;
  const title = kind === "wifi" ? "Wi-Fi networks" : kind === "audio" ? "Output devices" : "Bluetooth devices";
  const emptyText = kind === "wifi" ? "No networks found" : kind === "audio" ? "No output devices" : "No paired devices";

  return (
    <div className="cv-root" onClick={(e) => e.stopPropagation()}>
      <div className="cv-header">
        <button className="cv-back" onClick={onBack} aria-label="Back">
          <ChevronLeft size={16} />
        </button>
        <span className="cv-title">{title}</span>
        <button className="cv-link" onClick={onOpenWindowsSettings}>Windows settings</button>
      </div>

      <div className="cv-list">
        {loading && <div className="cv-hint">{kind === "wifi" ? "Scanning…" : "Loading…"}</div>}
        {!loading && empty && !error && <div className="cv-hint">{emptyText}</div>}

        {kind === "audio" && outputs?.map((o) => (
          <div
            key={o.id}
            className={`cv-row${o.is_default ? " connected" : ""}${busy === o.id ? " busy" : ""}`}
            onClick={() => { if (!o.is_default) void run(o.id, () => invoke("set_default_audio_device", { id: o.id })); }}
          >
            <Speaker size={13} className="cv-bt-icon" />
            <span className="cv-name">{o.name}</span>
            {o.is_default ? <Check size={14} /> : <span className="cv-status">{busy === o.id ? "Switching…" : ""}</span>}
          </div>
        ))}

        {kind === "wifi" && networks?.map((n) => (
          <div key={n.ssid}>
            <div
              className={`cv-row${n.connected ? " connected" : ""}${busy === n.ssid ? " busy" : ""}`}
              onClick={() => joinNetwork(n)}
            >
              <SignalBars signal={n.signal} />
              <span className="cv-name">{n.ssid}</span>
              {n.secured && <Lock size={11} className="cv-lock" />}
              <span className="cv-status">{busy === n.ssid ? "Connecting…" : n.connected ? "Connected" : n.known ? "Saved" : ""}</span>
              {n.connected && (
                <button className="cv-action" onClick={(e) => { e.stopPropagation(); void run(n.ssid, () => invoke("wifi_disconnect")); }}>
                  Disconnect
                </button>
              )}
              {n.known && !n.connected && (
                <button
                  className="cv-icon-btn"
                  title="Forget network"
                  onClick={(e) => { e.stopPropagation(); void run(n.ssid, () => invoke("wifi_forget", { ssid: n.ssid })); }}
                >
                  <X size={12} />
                </button>
              )}
            </div>
            {passwordFor === n.ssid && (
              <form className="cv-password" onSubmit={submitPassword}>
                <input
                  type="password"
                  autoFocus
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Network password"
                  spellCheck={false}
                />
                <button type="submit" disabled={!password}>Join</button>
              </form>
            )}
          </div>
        ))}

        {kind === "bluetooth" && devices?.map((d) => (
          <div key={d.id} className={`cv-row${d.connected ? " connected" : ""}`}>
            <Bluetooth size={13} className="cv-bt-icon" />
            <span className="cv-name">{d.name}</span>
            <span className="cv-status">{d.connected ? "Connected" : "Paired"}</span>
            {confirmRemove === d.id ? (
              <button
                className="cv-action danger"
                onClick={() => { setConfirmRemove(null); void run(d.id, () => invoke("bluetooth_unpair", { id: d.id })); }}
              >
                Remove?
              </button>
            ) : (
              <button className="cv-icon-btn" title="Remove device" onClick={() => setConfirmRemove(d.id)}>
                <X size={12} />
              </button>
            )}
          </div>
        ))}
      </div>

      {error && <div className="cv-error">{error}</div>}
    </div>
  );
}

