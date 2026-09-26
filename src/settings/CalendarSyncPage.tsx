import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, ChevronLeft, CloudCog, Link2, Plus, RefreshCw, Timer, Trash2, Unplug } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useCaldav } from "../hooks/useCaldav";

function timeAgo(ms: number | null): string {
  if (!ms) return "never";
  const mins = Math.round((Date.now() - ms) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  return hours < 24 ? `${hours} h ago` : `${Math.round(hours / 24)} d ago`;
}

const PROVIDER_LABELS: Record<string, string> = {
  google: "Google Calendar",
  icloud: "iCloud",
  outlook: "Outlook",
  proton: "Proton Calendar",
  fastmail: "Fastmail",
  nextcloud: "Nextcloud",
  other: "Calendar link",
};

interface Guide {
  id: string;
  label: string;
  steps: string[];
  note: string;
}

const GUIDES: Guide[] = [
  {
    id: "google",
    label: "Google",
    steps: [
      "Open Google Calendar on the web (calendar.google.com).",
      "Hover the calendar in the left list, click the three-dot menu, then Settings and sharing.",
      "Scroll to Integrate calendar and copy the Secret address in iCal format.",
    ],
    note: "Anyone with this address can read the calendar, so keep it private. Repeat for each calendar you want. If the option is missing, your Google Workspace admin may have turned it off.",
  },
  {
    id: "icloud",
    label: "iCloud",
    steps: [
      "Open Calendar on icloud.com and sign in.",
      "Click the second button from the bottom right of the window.",
      "Click the info (i) icon next to the calendar you want.",
      "Tick Public Calendar, then copy the link that appears.",
    ],
    note: "The link starts with webcal://. Paste it as it is. Only that one calendar becomes readable to anyone holding the link, and unticking Public Calendar stops sharing it.",
  },
  {
    id: "outlook",
    label: "Outlook",
    steps: [
      "In Outlook on the web, open Calendar and click the gear icon for Settings.",
      "Go to Calendar, then Shared calendars.",
      "Under Publish a calendar, pick the calendar and choose Can view all details.",
      "Click Publish. Two links appear: copy the ICS link (it ends in .ics), not the HTML one.",
    ],
    note: "Work or school accounts often have publishing turned off by an admin. Without Can view all details, events would show without their titles.",
  },
  {
    id: "proton",
    label: "Proton",
    steps: [
      "Open Proton Calendar on the web (calendar.proton.me).",
      "Go to Settings, All settings, Calendars, and choose the calendar.",
      "Under Share with anyone, click Create link and pick Full view.",
      "Click Copy link.",
    ],
    note: "Limited view only shows busy blocks with no titles. Anyone with the link can see the calendar, and you can delete the link from its Actions menu to cut off access.",
  },
  {
    id: "nextcloud",
    label: "Nextcloud",
    steps: [
      "Open the Calendar app in Nextcloud.",
      "Click the share icon next to a calendar, then the + next to Share link.",
      "Copy the link and paste it here. Nectar converts it to the calendar feed for you.",
    ],
    note: "To read your own private calendars without a public link, use the CalDAV account option further down instead.",
  },
  {
    id: "other",
    label: "Other",
    steps: [
      "Find the calendar's iCal or ICS subscription address in its sharing or export settings.",
      "Paste any link that ends in .ics or starts with webcal://.",
    ],
    note: "This covers holidays, sports fixtures, school timetables, Fastmail and more. If the link opens a web page instead of downloading a file, it isn't the right one.",
  },
];

type Provider = "nextcloud" | "icloud" | "other";

const ICLOUD_URL = "https://caldav.icloud.com";

const ACCOUNT_PROVIDERS: Record<Provider, { label: string; urlPlaceholder: string; userPlaceholder: string; hint: string }> = {
  nextcloud: {
    label: "Nextcloud",
    urlPlaceholder: "Server address, e.g. cloud.example.com",
    userPlaceholder: "Username",
    hint: "Create an app password in Nextcloud under Settings › Security › Devices & sessions.",
  },
  icloud: {
    label: "iCloud",
    urlPlaceholder: "",
    userPlaceholder: "Apple ID email",
    hint: "Apple doesn't allow your real password here. Create an app-specific password at account.apple.com under Sign-In and Security › App-Specific Passwords.",
  },
  other: {
    label: "Other CalDAV",
    urlPlaceholder: "CalDAV server address",
    userPlaceholder: "Username",
    hint: "Fastmail, Synology, Radicale, Baikal and most other CalDAV servers work. Use an app password if the service offers one.",
  },
};

interface Detection {
  provider: string;
  label: string;
  problem: string | null;
}

export function CalendarSyncPage({ onBack }: { onBack: () => void }) {
  const state = useCaldav();
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
    ? `${sources} source${sources === 1 ? "" : "s"} · ${state.events.length} events · synced ${timeAgo(state.syncedAt)}`
    : "Nothing connected yet";

  const account = ACCOUNT_PROVIDERS[provider];
  const activeGuide =
    GUIDES.find((g) => g.id === (guideId ?? detection?.provider)) ?? GUIDES[GUIDES.length - 1];

  return (
    <>
      <div className="caldav-page-header">
        <button className="caldav-back" onClick={onBack}>
          <ChevronLeft size={14} /> Notch
        </button>
        <div className="caldav-actions">
          {sources > 0 && (
            <button className="caldav-btn" onClick={() => invoke("open_calendar_window").catch(() => {})}>
              <CalendarDays size={11} /> Open calendar
            </button>
          )}
          {sources > 0 && (
            <button className="caldav-btn" disabled={busy !== null} onClick={() => run("sync", "caldav_sync_now")}>
              <RefreshCw size={11} /> {busy === "sync" ? "Syncing..." : "Sync now"}
            </button>
          )}
        </div>
      </div>

      <div className="setting-group-label">Calendar Sync</div>
      <div className="setting-group">
        <SettingRow
          icon={Timer}
          label="Sync Interval"
          desc={status}
          divider={false}
        >
          <select className="settings-select" value={interval} onChange={(e) => changeInterval(e.target.value)}>
            <option value="5">5 minutes</option>
            <option value="15">15 minutes</option>
            <option value="30">30 minutes</option>
            <option value="60">Hourly</option>
          </select>
        </SettingRow>
      </div>

      {message && <div className="caldav-error caldav-error--page">{message}</div>}

      <div className="setting-group-label setting-group-label--spaced">Calendar links</div>
      <div className="setting-group">
        <div className="caldav-intro">
          Paste a link from Google, iCloud, Outlook, Proton and more. No password or sign-in needed, and links are only
          ever read. They are stored in Windows Credential Manager.
        </div>

        {state.links.map((link) => (
          <div key={link.id}>
            <SettingRow
              icon={Link2}
              label={link.name}
              desc={`${PROVIDER_LABELS[link.provider] ?? "Calendar link"} · ${link.events} events · synced ${timeAgo(link.syncedAt)}`}
            >
              <button
                className="caldav-btn danger"
                disabled={busy !== null}
                onClick={() => run(`rm-${link.id}`, "ics_remove_link", { id: link.id })}
              >
                <Trash2 size={11} /> Remove
              </button>
            </SettingRow>
            {link.error && <div className="caldav-error">{link.error}</div>}
          </div>
        ))}

        <div className="caldav-form">
          <input
            type="text"
            placeholder="Paste a calendar link (https:// or webcal://)"
            value={linkUrl}
            onChange={(e) => setLinkUrl(e.target.value)}
            spellCheck={false}
          />
          {detection && (
            <div className={`caldav-detect ${detection.problem ? "bad" : "good"}`}>
              {detection.problem ?? `Recognized: ${detection.label}`}
            </div>
          )}
          <input
            type="text"
            placeholder="Name (optional)"
            value={linkName}
            onChange={(e) => setLinkName(e.target.value)}
            autoComplete="off"
          />
          <div className="caldav-form-footer">
            <button className="caldav-link-btn" onClick={() => setShowGuide((v) => !v)}>
              {showGuide ? "Hide" : "Where do I find my link?"}
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
              <Plus size={11} /> {busy === "add" ? "Adding..." : "Add link"}
            </button>
          </div>
        </div>

        {showGuide && (
          <div className="caldav-guide">
            <div className="caldav-chips">
              {GUIDES.map((g) => (
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

      <div className="setting-group-label setting-group-label--spaced">Calendar account</div>
      <div className="setting-group">
        <SettingRow
          icon={CloudCog}
          label="CalDAV account"
          desc={
            state.account
              ? `${state.account.calendars} calendar${state.account.calendars === 1 ? "" : "s"} · ${state.account.events} events · synced ${timeAgo(state.account.syncedAt)}`
              : "Also reads private calendars. Nextcloud, iCloud and other CalDAV servers."
          }
          divider={!state.account}
        >
          {state.account && (
            <button className="caldav-btn danger" disabled={busy !== null} onClick={() => run("disconnect", "caldav_disconnect")}>
              <Unplug size={11} /> Disconnect
            </button>
          )}
        </SettingRow>
        {state.account?.error && <div className="caldav-error">{state.account.error}</div>}

        {!state.account && (
          <div className="caldav-form">
            <div className="unit-toggle-minimal wide caldav-providers">
              {(Object.keys(ACCOUNT_PROVIDERS) as Provider[]).map((key) => (
                <span key={key} className={provider === key ? "active" : ""} onClick={() => pickProvider(key)}>
                  {ACCOUNT_PROVIDERS[key].label}
                </span>
              ))}
            </div>
            {provider !== "icloud" && (
              <input
                type="text"
                placeholder={account.urlPlaceholder}
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                spellCheck={false}
              />
            )}
            <input
              type="text"
              placeholder={account.userPlaceholder}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
            <input
              type="password"
              placeholder="App password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="off"
            />
            <div className="caldav-form-footer">
              <span className="caldav-hint">
                {account.hint} The password is stored in Windows Credential Manager, never in settings.json.
              </span>
              <button
                className="caldav-btn primary"
                disabled={busy !== null || !url.trim() || !username.trim() || !password}
                onClick={() => run("connect", "caldav_connect", { url, username, password }, () => setPassword(""))}
              >
                <Link2 size={11} /> {busy === "connect" ? "Connecting..." : "Connect"}
              </button>
            </div>
          </div>
        )}
      </div>
    </>
  );
}
