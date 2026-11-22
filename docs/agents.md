# AI agent status

Nectar can show what an AI agent on your computer is doing: a mascot and a status dot on the notch, and a panel when you hover the notch while the agent needs you.

Turn it on in Settings > Notch > AI agents, then restart Nectar. It listens on `127.0.0.1` only, on a random port, with a token that only `nectar.exe` reads from `agent.json` in the settings folder.

## Command line

```
nectar.exe agent <working|needs-input|done|error|clear> [--name NAME] [--project PROJECT]
```

`done` clears itself after 8 seconds. The command also tells Nectar which window the agent runs in, so the panel's Open button can bring it to the front.

## Claude Code

Nothing to set up. With the switch on, Nectar adds its hooks to `~/.claude/settings.json` when it starts, with the path to its own `nectar.exe`, and removes them when you turn the switch off and restart. Your other settings and hooks are not touched; the first time, the original file is saved next to it as `settings.json.nectar-backup`.

| Claude Code event | Notch |
| --- | --- |
| You send a prompt, a tool finishes | working |
| A permission question | needs input, with Allow and Deny in the panel |
| Claude waits for you (idle prompt) | needs input |
| Claude finishes | done |
| The session ends | cleared |

Answer a permission question in the notch panel, or ignore it and answer in the terminal. If Nectar is closed or nobody answers within two minutes, Claude asks in the terminal as usual.

## Connect your own agent

Any tool can drive the notch, not only Claude Code. Pick whichever of these fits.

### The easy way: the command

Call `nectar.exe` from a script, a hook or a wrapper. It finds the running Nectar, signs the request and exits.

```
nectar.exe agent working --name "My agent" --project "my-repo"
nectar.exe agent needs-input --name "My agent"
nectar.exe agent done --name "My agent"
```

Exit code is `0` when Nectar got it and `1` when it did not (Nectar closed, or the AI agents switch is off). It is safe to ignore the exit code.

### The direct way: HTTP

Read `agent.json` from `%APPDATA%\com.kua8.nectar\`. It holds the port and the token. Both change every time Nectar starts, so read the file for every run instead of saving the values.

```json
{ "port": 51234, "token": "..." }
```

Then send a `POST` to `http://127.0.0.1:<port>/activity` with the header `Authorization: Bearer <token>` and a JSON body:

| Field | Required | What it does |
| --- | --- | --- |
| `state` | yes | `working`, `tool`, `needs_input`, `done`, `error` or `clear` (also `thinking`, `waiting`, `completed`, `failed`, `idle`) |
| `name` | no | Agent name shown in the panel (40 characters) |
| `project` | no | Project or folder shown under the name (40 characters) |
| `detail` | no | One line of context, for example the command being run (600 characters) |
| `window` | no | A window handle (HWND, as a number). The panel's Open button brings it to the front |

A good request returns `204`. A wrong token returns `401`, an unknown state or bad JSON `400`, a body over 8 KB `413`.

PowerShell:

```powershell
$cfg = Get-Content "$env:APPDATA\com.kua8.nectar\agent.json" | ConvertFrom-Json
Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$($cfg.port)/activity" `
  -Headers @{ Authorization = "Bearer $($cfg.token)" } -ContentType "application/json" `
  -Body '{"state":"working","name":"My agent","project":"my-repo"}'
```

Python:

```python
import json, os, urllib.request
cfg = json.load(open(os.path.join(os.environ["APPDATA"], "com.kua8.nectar", "agent.json")))
req = urllib.request.Request(
    f"http://127.0.0.1:{cfg['port']}/activity",
    data=json.dumps({"state": "done", "name": "My agent"}).encode(),
    headers={"Authorization": f"Bearer {cfg['token']}", "Content-Type": "application/json"},
)
urllib.request.urlopen(req)
```

Node:

```js
const fs = require("fs"), path = require("path");
const cfg = JSON.parse(fs.readFileSync(path.join(process.env.APPDATA, "com.kua8.nectar", "agent.json")));
fetch(`http://127.0.0.1:${cfg.port}/activity`, {
  method: "POST",
  headers: { Authorization: `Bearer ${cfg.token}`, "Content-Type": "application/json" },
  body: JSON.stringify({ state: "working", name: "My agent" }),
});
```

### Asking the user

To get an Allow or Deny from the notch, `POST` to `/ask` with the same header. The request stays open until the user answers, so use a timeout of about two minutes.

```json
{ "name": "My agent", "project": "my-repo", "tool": "Bash", "detail": "npm install" }
```

The answer is `{"decision":"allow"}` or `{"decision":"deny"}`. If nobody answers in two minutes, or the connection closes, you get `204` and should ask in your own UI.

To ask a multiple choice question instead, add `"questions": [{ "question": "Which one?", "options": ["A", "B"] }]` (up to 4 questions with 2 to 4 options each). An allowed answer then carries `"choices": [0]`, the index picked for each question in order.

### Good to know

- Only programs on the same computer can connect, and only with the token. Treat `agent.json` like a password.
- The state is per computer, not per agent: the newest update wins. Send `clear` when your agent exits so the notch goes back to normal.
- `done` clears itself after 8 seconds.
- The mascot and the sounds follow the same states, so an agent you connect gets them for free: `working` and `tool` show the working cat, `needs_input` the question cat, `done` the happy cat, `error` the worried cat.
- Settings that control all of this are listed in [SETTINGS.md](../SETTINGS.md) (`nectar-agent-api`, `nectar-agent-sounds`).
