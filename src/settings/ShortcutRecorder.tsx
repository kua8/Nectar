import { useState } from "react";
import type { KeyboardEvent } from "react";
import { X } from "lucide-react";
import { useT } from "../i18n";

const NAMED: Record<string, string> = {
  Space: "Space", Enter: "Enter", Tab: "Tab", Backspace: "Backspace", Delete: "Delete", Insert: "Insert",
  Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown",
  ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
  Backquote: "`", Minus: "-", Equal: "=", Comma: ",", Period: ".", Slash: "/",
  Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]", Backslash: "\\",
};

function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  return NAMED[code] ?? null;
}

const MODIFIER_CODES = ["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"];

function modifiersOf(e: KeyboardEvent): string[] {
  return [e.metaKey && "Win", e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
}

export function ShortcutRecorder({ value, onCommit }: { value: string; onCommit: (shortcut: string) => void }) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  const [held, setHeld] = useState<string[]>([]);

  const stop = () => {
    setRecording(false);
    setHeld([]);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.code === "Escape") {
      stop();
      return;
    }
    if (MODIFIER_CODES.includes(e.code)) {
      setHeld(modifiersOf(e));
      return;
    }
    const key = keyName(e.code);
    const mods = modifiersOf(e);
    const isFunctionKey = /^F\d+$/.test(key ?? "");
    if (!key || (mods.length === 0 && !isFunctionKey)) return;
    stop();
    onCommit([...mods, key].join("+"));
  };

  return (
    <div className="shortcut-recorder">
      <button
        type="button"
        className={`shortcut-input shortcut-record${recording ? " recording" : ""}`}
        onClick={() => setRecording(true)}
        onKeyDown={onKeyDown}
        onKeyUp={(e) => recording && setHeld(modifiersOf(e))}
        onBlur={stop}
      >
        {recording ? (held.length ? `${held.join("+")}+…` : t("shortcut.recording")) : value || t("shortcut.none")}
      </button>
      {value && !recording && (
        <button type="button" className="shortcut-clear" onClick={() => onCommit("")} aria-label={t("shortcut.clear")}>
          <X size={12} />
        </button>
      )}
    </div>
  );
}
