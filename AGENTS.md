# Pulse — Project Rules

**Pulse** (*Processes, Units, Logs, States & Events*) is a **read-only** systemd service observatory. The acronym is descriptive, **not** a promise to monitor all processes.

- **Directory:** `~/Projects/Pulse` (case-sensitive)
- **Proposed GitHub repo / executable:** `pulse` (availability unverified)
- **Stack:** Rust, Ratatui, Crossterm; NixOS-first; other systemd Linux distributions only when verified
- **MVP target:** `0.1.0`. Ask before selecting a license, creating/pushing a remote, tagging, or publishing.

## MVP acceptance criteria

1. Inspect **system and user** `.service` units; show name, description, load/active/substate, and manager scope.
2. Keyboard-first navigation, search/filter, help, resize handling, and selection persistence across refresh where practical.
3. Service properties and bounded recent journal entries, with accurate empty, unavailable, error, and permission-denied states.
4. Nonblocking refresh; expose freshness/errors and mark stale data clearly. Restore terminal state on exit and recoverable errors.
5. **Read-only always:** no start/stop/restart/enable, `sudo`, privilege escalation, system rebuild, configuration changes, or journal-data uploads.

Other unit types, remote servers, service controls, process monitoring, and advanced NixOS insights are out of scope.

## Design

- Separate UI/state, typed models, parsing, systemd/journal adapters, and subprocess execution. Use mockable interfaces for tests.
- Start with `systemctl`/`journalctl` using explicit `Command` arguments—never shell interpolation. Confirm supported flags/output locally; do not assume JSON.
- Treat names/logs/output as untrusted; handle malformed output, encoding, missing utilities/user manager, denied access, timeouts, cancellation, and nonzero exits.
- Bound logs and subprocesses. Keep dependencies small and backend replaceable (D-Bus later); prefer a reproducible Nix dev shell where useful.

## Quality gate

**Before every commit**, run successfully against the content to be committed:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --all-targets --all-features
```

Also run changed fixture/integration tests, check the Nix dev shell when modified, and manually smoke-test affected TUI behavior when feasible. If a required check cannot be completed, disclose it and seek direction **before committing**. Configure GitHub Actions for automated checks on push/PR. Follow the global staged-content rules.

## Documentation and context

- Keep `README.md` accurate: implemented features, setup, shortcuts, usage, development/testing, limitations, roadmap; no invented screenshots/compatibility.
- Keep `docs/PROGRESS.md` to a **brief milestone handoff** (done, key decisions, next, blockers); consult it when resuming, not on trivial edits.
- `IMPLEMENTATION.md` is the phased task brief, not an auto-loaded instruction. Read the relevant milestone only when needed.
