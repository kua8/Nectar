import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, ChevronLeft, CloudCog, Link2, Plus, RefreshCw, Timer, Trash2, Unplug } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { Dropdown } from "./Dropdown";
import { useCaldav } from "../hooks/useCaldav";
import { timeAgo, useT, type TFunction } from "../i18n";

const PROVIDER_LABELS: Record<string, string> = {
  google: "Google Calendar",
  icloud: "iCloud",
  outlook: "Outlook",
  proton: "Proton Calendar",
  fastmail: "Fastmail",
  nextcloud: "Nextcloud",
};

type Provider = "nextcloud" | "icloud" | "other";

interface Guide {
  id: string;
  label: string;
  steps: string[];
  note: string;
}

const GUIDE_STEP_COUNTS: Record<string, number> = { google: 3, icloud: 4, outlook: 4, proton: 4, nextcloud: 3, other: 2 };
const GUIDE_BRANDS: Record<string, string> = { google: "Google", icloud: "iCloud", outlook: "Outlook", proton: "Proton", nextcloud: "Nextcloud" };

function buildGuides(t: TFunction): Guide[] {
  return Object.keys(GUIDE_STEP_COUNTS).map((id) => ({
    id,
    label: GUIDE_BRANDS[id] ?? t(`calsync.guide.${id}.label`),
    steps: Array.from({ length: GUIDE_STEP_COUNTS[id] }, (_, i) => t(`calsync.guide.${id}.step${i + 1}`)),
    note: t(`calsync.guide.${id}.note`),
  }));
}

const ICLOUD_URL = "https://caldav.icloud.com";

const ACCOUNT_PROVIDERS: Provider[] = ["nextcloud", "icloud", "other"];
const ACCOUNT_BRANDS: Partial<Record<Provider, string>> = { nextcloud: "Nextcloud", icloud: "iCloud" };

interface Detection {
  provider: string;
  label: string;
  problem: string | null;
}

export function CalendarSyncPage({ onBack }: { onBack: () => void }) {
  const t = useT();
  const state = useCaldav();
  const guides = buildGuides(t);
  const [interval, setIntervalValue] = useState("15");
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const [linkUrl, setLinkUrl] = useState("");
  const [linkName, setLinkName] = useState("");
  const [detection, setDetection] = useState<Detection | null>(null);
  const [showGuide, setShowGuide] = useState(false);
  const [guideId, setGuideId] = useState<string | null>(null);

  const [provider, setProvider] = useState<Provider>("nextcloud");
  const [url, setUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  useEffect(() => {
    invoke<Record<string, string>>("load_settings")
      .then((s) => {
        const savedUrl = s["nectar-caldav-url"] ?? "";
        setUrl(savedUrl);
        if (savedUrl.includes("icloud.com")) setProvider("icloud");
        setUsername(s["nectar-caldav-username"] ?? "");
        setIntervalValue(s["nectar-caldav-interval"] ?? "15");
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!linkUrl.trim()) {
      setDetection(null);
      return;
    }
    let alive = true;
    invoke<Detection | null>("ics_detect", { url: linkUrl })
      .then((d) => alive && setDetection(d))
      .catch(() => alive && setDetection(null));
    return () => {
      alive = false;
    };
  }, [linkUrl]);

  const run = async (id: string, command: string, args?: Record<string, unknown>, onOk?: () => void) => {
    setBusy(id);
    setMessage(null);
    try {
      await invoke(command, args);
      onOk?.();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(null);
    }
  };

  const changeInterval = (minutes: string) => {
    setIntervalValue(minutes);
    invoke("save_setting", { key: "nectar-caldav-interval", value: minutes }).catch(() => {});
  };

  const pickProvider = (next: Provider) => {
    setProvider(next);
    setMessage(null);
    if (next === "icloud") setUrl(ICLOUD_URL);
    else if (url === ICLOUD_URL) setUrl("");
  };

  const sources = (state.account ? 1 : 0) + state.links.length;
  const status = sources
    ? t("calsync.status", {
        sources: t(sources === 1 ? "calsync.sourceOne" : "calsync.sourceMany", { count: sources }),
        events: state.events.length,
        ago: timeAgo(state.syncedAt, t),
      })
    : t("calsync.status.empty");

  const activeGuide =
    guides.find((g) => g.id === (guideId ?? detection?.provider)) ?? guides[guides.length - 1];

  return (
    <>
      <div className="caldav-page-header">
        <button className="caldav-back" onClick={onBack}>
          <ChevronLeft size={14} /> {t("notch.group")}
        </button>
        <div className="caldav-actions">
          {sources > 0 && (
            <button className="caldav-btn" onClick={() => invoke("open_calendar_window").catch(() => {})}>
              <CalendarDays size={11} /> {t("calsync.openCalendar")}
            </button>
          )}
          {sources > 0 && (
            <button className="caldav-btn" disabled={busy !== null} onClick={() => run("sync", "caldav_sync_now")}>
              <RefreshCw size={11} /> {busy === "sync" ? t("calsync.syncing") : t("calsync.syncNow")}
            </button>
          )}
        </div>
      </div>

      <div className="setting-group-label">{t("notch.calendarSync")}</div>
      <div className="setting-group">
        <SettingRow
          icon={Timer}
          label={t("calsync.interval")}
          desc={status}
          divider={false}
        >
          <Dropdown
            value={interval}
            onChange={changeInterval}
            options={[
              { value: "5", label: t("calsync.interval.minutes", { count: 5 }) },
              { value: "15", label: t("calsync.interval.minutes", { count: 15 }) },
              { value: "30", label: t("calsync.interval.minutes", { count: 30 }) },
              { value: "60", label: t("calsync.interval.hourly") },
            ]}
          />
        </SettingRow>
      </div>

      {message && <div className="caldav-error caldav-error--page">{message}</div>}

      <div className="setting-group-label setting-group-label--spaced">{t("calsync.links.group")}</div>
      <div className="setting-group">
        <div className="caldav-intro">
          {t("calsync.links.intro")}
        </div>

        {state.links.map((link) => (
          <div key={link.id}>
            <SettingRow
              icon={Link2}
              label={link.name}
              desc={t("calsync.link.desc", { provider: PROVIDER_LABELS[link.provider] ?? t("calsync.link.other"), events: link.events, ago: timeAgo(link.syncedAt, t) })}
            >
              <button
                className="caldav-btn danger"
                disabled={busy !== null}
                onClick={() => run(`rm-${link.id}`, "ics_remove_link", { id: link.id })}
              >
                <Trash2 size={11} /> {t("calsync.remove")}
              </button>
            </SettingRow>
            {link.error && <div className="caldav-error">{link.error}</div>}
          </div>
        ))}

        <div className="caldav-form">
          <input
            type="text"
            placeholder={t("calsync.link.placeholder")}
            value={linkUrl}
            onChange={(e) => setLinkUrl(e.target.value)}
            spellCheck={false}
          />
          {detection && (
            <div className={`caldav-detect ${detection.problem ? "bad" : "good"}`}>
              {detection.problem ?? t("calsync.link.recognized", { name: detection.label })}
            </div>
          )}
          <input
            type="text"
            placeholder={t("calsync.link.name")}
            value={linkName}
            onChange={(e) => setLinkName(e.target.value)}
            autoComplete="off"
          />
          <div className="caldav-form-footer">
            <button className="caldav-link-btn" onClick={() => setShowGuide((v) => !v)}>
              {showGuide ? t("calsync.guide.hide") : t("calsync.guide.show")}
            </button>
            <button
              className="caldav-btn primary"
              disabled={busy !== null || !linkUrl.trim() || !!detection?.problem}
              onClick={() =>
                run("add", "ics_add_link", { url: linkUrl, name: linkName }, () => {
                  setLinkUrl("");
                  setLinkName("");
                })
              }
            >
              <Plus size={11} /> {busy === "add" ? t("calsync.adding") : t("calsync.add")}
            </button>
          </div>
        </div>

        {showGuide && (
          <div className="caldav-guide">
            <div className="caldav-chips">
              {guides.map((g) => (
                <button key={g.id} className={g.id === activeGuide.id ? "active" : ""} onClick={() => setGuideId(g.id)}>
                  {g.label}
                </button>
              ))}
            </div>
            <ol>
              {activeGuide.steps.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ol>
            <p>{activeGuide.note}</p>
          </div>
        )}
      </div>

      <div className="setting-group-label setting-group-label--spaced">{t("calsync.account.group")}</div>
      <div className="setting-group">
        <SettingRow
          icon={CloudCog}
          label={t("calsync.account")}
          desc={
            state.account
              ? t("calsync.account.desc", {
                  calendars: t(state.account.calendars === 1 ? "calsync.calendarOne" : "calsync.calendarMany", { count: state.account.calendars }),
                  events: state.account.events,
                  ago: timeAgo(state.account.syncedAt, t),
                })
              : t("calsync.account.empty")
          }
          divider={!state.account}
        >
          {state.account && (
            <button className="caldav-btn danger" disabled={busy !== null} onClick={() => run("disconnect", "caldav_disconnect")}>
              <Unplug size={11} /> {t("calsync.disconnect")}
            </button>
          )}
        </SettingRow>
        {state.account?.error && <div className="caldav-error">{state.account.error}</div>}

        {!state.account && (
          <div className="caldav-form">
            <div className="unit-toggle-minimal wide caldav-providers">
              {ACCOUNT_PROVIDERS.map((key) => (
                <span key={key} className={provider === key ? "active" : ""} onClick={() => pickProvider(key)}>
                  {ACCOUNT_BRANDS[key] ?? t(`calsync.account.${key}.label`)}
                </span>
              ))}
            </div>
            {provider !== "icloud" && (
              <input
                type="text"
                placeholder={t(`calsync.account.${provider}.url`)}
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                spellCheck={false}
              />
            )}
            <input
              type="text"
              placeholder={t(`calsync.account.${provider}.user`)}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
            <input
              type="password"
              placeholder={t("calsync.account.password")}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="off"
            />
            <div className="caldav-form-footer">
              <span className="caldav-hint">
                {t(`calsync.account.${provider}.hint`)} {t("calsync.account.storage")}
              </span>
              <button
                className="caldav-btn primary"
                disabled={busy !== null || !url.trim() || !username.trim() || !password}
                onClick={() => run("connect", "caldav_connect", { url, username, password }, () => setPassword(""))}
              >
                <Link2 size={11} /> {busy === "connect" ? t("calsync.connecting") : t("calsync.connect")}
              </button>
            </div>
          </div>
        )}
      </div>
    </>
  );
}
