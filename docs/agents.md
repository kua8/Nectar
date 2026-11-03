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
