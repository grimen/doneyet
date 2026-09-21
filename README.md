# doneyet

> exits when it's done yet.

A realtime, colorful terminal watcher for CI/CD workflow runs — GitHub Actions
today, other providers tomorrow.

```
doneyet acme/api        # watch the latest run live (alias for `doneyet watch`)
doneyet runs acme/api   # list recent runs
doneyet run 2841        # inspect one run — failed jobs show their annotations
doneyet run 2841 --logs-failed [N]   # …and the last N log lines of failed jobs
```

Exit code equals the run conclusion: `0` pass, `1` fail, `2` cancelled/timeout,
`130` interrupted.

On a terminal, `watch` and `dash` read keys: `q` or `Esc` quits, `r` refreshes
now, `Ctrl-C` still interrupts. Piped or non-tty stdout stays in normal mode.

## Dashboard

`doneyet dash` live-updates the recent-runs table until you quit. It only
lists runs — no per-run job fetch — and exits `130`.

```
doneyet dash acme/api --limit 20 --interval 5
```

## Webhook mode (instant refresh)

Polling gets you 2–5s updates; webhooks get you milliseconds. Point a GitHub
webhook at a local listener and `doneyet` refetches immediately (ETag makes
each refetch cheap):

```
doneyet watch acme/api --webhook 127.0.0.1:4567 --webhook-secret "$SECRET"
```

- The listener validates `X-Hub-Signature-256` (HMAC-SHA256, constant-time);
  unsigned or wrongly signed deliveries are rejected with 401
- `workflow_run` and `workflow_job` events trigger a refresh; `ping` is
  acknowledged but ignored
- Since GitHub must reach your machine, expose the port through a tunnel:

```
smee.io     # npx smee-client --url https://smee.io/XXXX --target localhost:4567/webhook
ngrok       # ngrok http 4567  (use the forwarded URL in the webhook settings)
tailscale   # funnel serve on port 4567 also works
```

## Theming & rendering customization

Rendering is pure data: a `Theme` (serde-serializable, per-field defaults)
drives glyphs, colors, bar characters/width, step connector, title separator,
and the no-duration marker. The `Renderer` port, engine, and provider layers
never see it — themes live entirely in the widget/renderer layer, so a future
TUI/GUI frontend can adopt the same theme format.

```
doneyet watch acme/api --theme ascii    # 7-bit output for CI logs / dumb terms
doneyet runs  acme/api --theme default
```

`--theme` accepts a built-in name or a path to a JSON theme file, so a theme
can set only what it wants — everything else falls back to the built-in look:

```json
{ "bar": { "width": 5, "filled": "@", "empty": "-" },
  "glyphs": { "success": "✔", "failure": "✘" },
  "ink": { "success": { "color": "Green", "bold": true } } }
```

Save it anywhere and pass the path: `--theme ~/.config/doneyet/gruvbox.json`.

## Recording

`--record` taps the render stream: every frame and the final outcome are
mirrored to a JSONL file while the terminal keeps rendering normally. It's a
decorator on the `Renderer` port, so the engine can't tell the difference —
and a future TUI gets recording for free by wrapping the same way.

```
doneyet watch acme/api --record run.jsonl
```

Schema (versioned, one record per line):

```json
{"v":1,"kind":"render","world":{...},"events":[...]}
{"v":1,"kind":"finish","outcome":{...}}
```

Records are full domain snapshots (`World`, `DomainEvent`, `Outcome`), each
stamped with a wall-clock `ts`. If the recording file becomes unwritable
mid-run, recording is disabled with a warning; the watch keeps going.

Replay a recording offline — no API access, any theme, exit code preserved:

```
doneyet replay run.jsonl               # instant dump
doneyet replay run.jsonl --realtime    # at the recorded wall-clock pace
```

## Status

Milestone 6 complete — all planned MVP milestones shipped:

- `doneyet watch [OWNER/NAME] [-b branch] [--commit SHA] [--interval N]` —
  live inline redraw of the latest run; exits with the run's conclusion.
  While a run is in progress the header shows an ETA
  (`3m12s (~1m28s left)`) derived from the median duration of recent
  runs of the same workflow. `--logs` tails the last N lines of
  in-progress and failed jobs (default 20); a log fetch failure does not
  stop the watch. `--notify` fires a desktop notification (`notify-send`)
  when the run finishes
- `doneyet dash [OWNER/NAME] [--limit N] [--interval N]` — live recent-runs
  table until quit (`q` / `Esc` / `Ctrl-C`, exit `130`)
- `doneyet runs [OWNER/NAME] [--limit N]` — colorful recent-runs table
- `doneyet run <ID> [--repo OWNER/NAME]` — static single-run view + exit code
- bare `doneyet OWNER/NAME` is shorthand for `watch`
- repo defaults from the `origin` git remote; token from `--token` /
  `DONEYET_TOKEN` / `GH_TOKEN` / `GITHUB_TOKEN` / `gh auth token`
- `--api-base` for GitHub Enterprise, `--no-color`, `--width`
- exit codes: `0` pass · `1` fail · `2` cancelled/timeout · `130` ctrl-c · `4` error

Engine: ETag-polling watch loop with push-source preemption and cancellation,
paused-clock tested; renderer goldens; provider contract suite on wiremock;
webhook push source with HMAC validation (unit + end-to-end tested).

Roadmap: TUI/GUI frontends on the same `Renderer` port, more CI providers on
the `RunSource` capability traits — the `doneyet-contract` suite is provider-agnostic.
The live runs board is `doneyet dash`.

## Architecture

Hexagonal (ports & adapters):

| Crate              | Role                                                      |
|--------------------|-----------------------------------------------------------|
| `doneyet-core`     | domain model, diff engine, ports (`RunSource`/`LogSource`/`AnnotationSource`, `PipelineProvider`, `Renderer`, `PushSource`) — no I/O |
| `doneyet-github`   | GitHub Actions REST adapter (ETag polling)                |
| `doneyet-ux`     | rendering layer: glyphs, themes, frames, runs table, JSONL recording |
| `doneyet-app`      | watch engine (poll/push loop, exit codes)                 |
| `doneyet-contract` | provider conformance test suite (dev-only, provider-agnostic) |
| `doneyet-cli`      | binary `doneyet`, composition root                        |

## Development

```
nix develop   # or: direnv allow — rust, nextest, cargo-deny, just, typos, gh
just check    # fmt --check + clippy -D warnings + nextest + cargo-deny
just test     # nextest only
just typos    # spell-check (skips gracefully if typos is absent)
```

The nix flake provides the full toolchain; `nix fmt` formats nix files.
Dependabot keeps cargo deps and GitHub Actions fresh. Agent/contributor
conventions live in [AGENTS.md](AGENTS.md).
