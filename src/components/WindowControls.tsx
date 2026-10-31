import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useT } from "../i18n";
import "./WindowControls.css";

export function WindowControls() {
  const t = useT();
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
      <button className="wc-btn minimize" aria-label={t("window.minimize")} onClick={() => win.minimize().catch(() => {})} />
      <button className="wc-btn close" aria-label={t("window.close")} onClick={() => win.hide().catch(() => {})} />
    </div>
  );
}
