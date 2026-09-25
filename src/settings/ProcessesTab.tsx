import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, RefreshCw } from "lucide-react";

interface ProcessInfo {
  pid: number;
  name: string;
  exe_path: string | null;
  memory_kb: number;
}

function ProcessIcon({ path }: { path: string | null }) {
  const [icon, setIcon] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!path) return;
    invoke<string | null>("get_app_icon", { path, name: null, hwnd: null })
      .then((data) => { if (!cancelled) setIcon(data); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [path]);

  return (
    <div className="process-icon">
      {icon ? <img src={icon} alt="" /> : <div className="process-icon-fallback" />}
    </div>
  );
}

export function ProcessesTab() {
  const [processes, setProcesses] = useState<ProcessInfo[]>([]);
  const [endingPid, setEndingPid] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const refreshTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const refresh = () => {
    invoke<ProcessInfo[]>("get_running_processes")
      .then((list) => setProcesses(list))
      .catch((e) => console.error("Failed to list processes:", e))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refresh();
    refreshTimer.current = setInterval(refresh, 5000);
    return () => {
      if (refreshTimer.current) clearInterval(refreshTimer.current);
    };
  }, []);

  const handleEnd = async (proc: ProcessInfo) => {
    if (endingPid !== null) return;
    setEndingPid(proc.pid);
    try {
      await invoke("kill_process", { pid: proc.pid, name: proc.name });
      setProcesses((prev) => prev.filter((p) => p.pid !== proc.pid));
    } catch (e) {
      console.error("Failed to end process:", e);
    } finally {
      setEndingPid(null);
    }
  };

  return (
    <>
      <div className="setting-group-label">
        Background Tasks
        <button className="processes-refresh-btn" onClick={refresh} title="Refresh">
          <RefreshCw size={12} strokeWidth={1.75} />
        </button>
      </div>
      <div className="setting-group processes-list">
        {loading && <div className="processes-empty">Loading...</div>}
        {!loading && processes.length === 0 && <div className="processes-empty">No processes found</div>}
        {!loading && processes.map((proc) => (
          <div className="process-row" key={proc.pid}>
            <ProcessIcon path={proc.exe_path} />
            <div className="process-info">
              <span className="process-name">{proc.name}</span>
              <span className="process-desc">{(proc.memory_kb / 1024).toFixed(1)} MB</span>
            </div>
            <button
              className="process-end-btn"
              disabled={endingPid === proc.pid}
              onClick={() => handleEnd(proc)}
              title="End task"
            >
              <X size={13} strokeWidth={1.75} />
            </button>
          </div>
        ))}
      </div>
    </>
  );
}
