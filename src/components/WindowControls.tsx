import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import "./WindowControls.css";

export function WindowControls() {
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
      <button className="wc-btn minimize" aria-label="Minimize" onClick={() => win.minimize().catch(() => {})} />
      <button className="wc-btn close" aria-label="Close" onClick={() => win.hide().catch(() => {})} />
    </div>
  );
}
