# Pulse — Codex Implementation Brief

Build **Pulse** (*Processes, Units, Logs, States & Events*), a reliable, **read-only** Rust/Ratatui/Crossterm terminal app for systemd services. Work in `~/Projects/Pulse`; proposed repo/executable: `pulse` (not availability-cleared). NixOS-first; target MVP `0.1.0`.

Codex loads applicable global and repository `AGENTS.md` instructions automatically. Follow their engineering standards and **full pre-commit gate**; don't paste or reread them on every task. This brief supplies product scope and milestones.

## Outcome

A usable keyboard-driven TUI that lists **system/user `.service` units**, shows status/scope, supports search and filters, inspects unit properties and bounded recent journal logs, refreshes without blocking input, displays errors/staleness honestly, and cleans up terminal state. **No service modification, privilege escalation, or global NixOS changes.**

Use typed domain models and an isolated, mockable `systemctl`/`journalctl` subprocess backend (explicit argument arrays, no shell interpolation). Provide relevant tests, a suitable Nix dev shell, `.gitignore`, GitHub Actions CI, an accurate README, and a short `docs/PROGRESS.md`.

## Milestones

1. **Foundation:** Inspect existing files and Rust/Nix/systemd tools; implement Cargo layout, minimal safe TUI lifecycle, error handling, basic tests, dev shell if appropriate, README, and CI.
2. **Discovery:** System and user service enumeration; typed states; robust parsing; fixtures for missing, malformed, empty, and failed outputs.
3. **Navigation:** Service list, shortcuts/help, search/filter, empty states, resizing, and tested selection behavior.
4. **Details/logs:** Metadata and bounded journal view; permission/unavailable states, timeout/cancellation, no guessed diagnoses.
5. **Refresh/reliability:** Nonblocking updates, freshness/error indication, selection retention, failure recovery, regression tests.
6. **Release readiness:** Full automated gate, Nix dev-shell validation, relevant real TUI smoke test, README/shortcut review, and a brief `0.1.0` readiness report. **Do not tag or publish without approval.**

Keep MVP limited to local services and observation. Subprocesses are simpler to test now; leave room for future D-Bus. Do not add service controls, remote hosts, other unit types, or a general process monitor without approval.

## Execution instructions

- Begin with **Milestone 1**. Inspect existing content before overwriting anything. Identify only genuine blockers (such as tool availability, consequential dependencies, or a license decision); use sensible defaults otherwise.
- Briefly state architecture/risks/acceptance tests, then **implement and validate** the milestone rather than stopping at a proposal. Continue until it passes or a real blocker arises.
- Run targeted checks during development; run all mandatory project checks **before each intended commit**, against the staged content. Make coherent, accurate Conventional Commits only when verified. Failed or unavailable required checks mean **no commit**; report the blocker.
- After a milestone, update the short progress note and report only: delivered behavior, actual checks/results, gaps, next milestone, and commit ID if any. Use the note to restart a fresh session; consult only relevant future brief sections.
- Local commits are allowed under the gate. Ask before remote creation/push, licensing, or release publication.
