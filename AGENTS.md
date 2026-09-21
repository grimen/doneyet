# doneyet — agent guide (read first)

**What:** realtime CI/CD pipeline watcher CLI in Rust. GitHub Actions today;
the ports are built for more providers and more frontends later.

**Repo:** `/home/jonas/Dev/doneyet` — cargo workspace, six crates, zero
external services needed for tests (wiremock everywhere).

## Session rules

1. **The gate is `just check`** = `fmt --all --check` + `clippy --all-targets
   -D warnings` + `nextest run` + `cargo deny check`. The local machine is the
   referee; nothing is done until this is green.
2. **TDD is mandatory.** Write the failing test first; derive expectations
   from ports, GitHub REST/webhook docs, or fixture shapes — never mirror the
   implementation. Renderer goldens are regenerated with
   `UPDATE_SNAPSHOTS=1 cargo nextest run -p doneyet-ux`, never hand-edited.
3. **Architecture is law:**
   - `doneyet-core` has **no I/O deps** (no tokio, no reqwest, no
     serde_json; serde + jiff + async-trait are fine)
   - providers implement the capability traits (`RunSource`,
     `LogSource`, `AnnotationSource`; `PipelineProvider` is the blanket
     aggregate) and every new provider must pass `doneyet-contract`
   - renderers implement `Renderer`; decorate, don't branch (see
     `TeeRenderer`) — the engine must not know what it is talking to
   - naming avoids "CI" prefixes: workflows are CD too; "pipeline" and
     "run" are the neutral vocabulary
4. **Dependency direction:** `cli → app / ux / github → core`, never the
   reverse. New deps go in the root `[workspace.dependencies]` and, if a new
   license appears, `deny.toml`'s allowlist — `cargo deny check` will tell you.
5. **Failure semantics are contracts:** a dead `PushSource` degrades to
   polling (never kills a watch), renderer failure is fatal, provider
   auth/transport failure aborts with exit 4.
6. **Exit codes are public API:** 0 pass, 1 fail, 2 cancelled/timed-out,
   130 ctrl-c, 4 error.
7. **Style:** no code comments unless asked; match the existing voice;
   rustfmt defaults are canonical (no rustfmt.toml on purpose).

## Dev loop

```
nix develop          # or: direnv allow
just check
cargo run -p doneyet-cli -- runs acme/api --api-base http://…
```

Tokens for manual runs: `--token`, `DONEYET_TOKEN`, `GH_TOKEN`,
`GITHUB_TOKEN`, or `gh auth token`. `--api-base` points the provider at any
GitHub-shaped API (wiremock in tests, GHES in prod).

## Fast pointers

- Ports (all traits): `crates/doneyet-core/src/ports.rs`
- Provider conformance suite: `crates/doneyet-contract` (+ fixtures in
  `crates/doneyet-contract/fixtures/`)
- Watch engine (poll/push/cancel loop): `crates/doneyet-app/src/engine.rs`
- Rendering, themes, goldens, recording: `crates/doneyet-ux`
  (snapshots in `crates/doneyet-ux/tests/snapshots/`)
- User-facing behavior: `README.md` (usage, theming, recording, webhooks)
