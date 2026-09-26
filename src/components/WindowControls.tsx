import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import "./WindowControls.css";

interface Props {
  canMinimize?: boolean;
  canZoom?: boolean;
}

export function WindowControls({ canMinimize = true, canZoom = false }: Props) {
  const [focused, setFocused] = useState(true);

  useEffect(() => {
    const unlisten = getCurrentWebviewWindow().onFocusChanged(({ payload }) => setFocused(payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const win = getCurrentWebviewWindow();

  return (
    <div className={`wc${focused ? "" : " blurred"}`}>
      <button className="wc-btn close" aria-label="Close" onClick={() => win.hide().catch(() => {})} />
      <button className="wc-btn minimize" aria-label="Minimize" disabled={!canMinimize} onClick={() => win.minimize().catch(() => {})} />
      <button className="wc-btn zoom" aria-label="Zoom" disabled={!canZoom} onClick={() => win.toggleMaximize().catch(() => {})} />
    </div>
  );
}
