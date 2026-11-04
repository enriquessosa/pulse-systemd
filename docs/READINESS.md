# 0.1.0 readiness review

Milestone 6 is complete for owner review on the verified NixOS x86_64 setup.
This is a source/build readiness assessment, not approval to publish a release.

## Audit outcome

- The adapters issue only `systemctl list-units`, `systemctl show`, and bounded
  `journalctl` reads. Unit validation, explicit arguments, output escaping, limits,
  and scope/request identity checks remain intact; no shell or service controls.
- Existing UI/state, models, parsing, adapters, and execution boundaries were kept.
  The three direct Rust dependencies and Cargo/Nix lockfiles were unchanged.
- Fixed terminal cleanup: it runs once, and only an active session's UI-thread
  panic restores the terminal. Worker panics no longer disable live UI input;
  panics after the session no longer attempt another restoration.
- Added five real-PTY lifecycle regressions and wired them into CI. Python is a
  development/test tool in the pinned shell, not an application dependency.
- README now distinguishes implemented features, setup, controls, bounds, access
  limits, verified platforms, and outstanding release decisions.

## Verification

| Check | Actual result |
| --- | --- |
| Full fmt/Clippy/test/build gate | Passed with native Rust/Cargo 1.95.0 and pinned 1.99.0 |
| Rust behavior tests | 46 passed; fixture-only ignored tests are explicitly invoked by their harnesses |
| Terminal lifecycle PTYs | Success, I/O error, UI panic, worker panic, after-session panic passed |
| Nix | Both Linux shell definitions checked; x86_64 shell entered with Rust/Cargo 1.99.0 and Python 3.14.8 |
| Optimized build | `cargo build --locked --release` passed in pinned shell |
| Optimized binary, live PTYs | System/user discovery, properties, journal, refresh, navigation, resizing, terminal cleanup passed |
| Optimized binary, controlled PTYs | Empty/partial/denied/malformed/missing-tool/slow/timeout/overflow, stale recovery, overlap and child cleanup passed |
| Workflow lint | actionlint passed using locked nixpkgs |

Live checks used NixOS/systemd 260. Controlled service-UI smoke checks used
stand-ins and disposable local PTY harnesses; they did not change real units.
The checked-in `tests/terminal_pty.py` reproduces the new lifecycle regressions.

CI is configured for the full locked Rust gate, Nix flake/shell validation, and
terminal PTY checks. Its Nix setup follows the
[installer's flake documentation](https://github.com/cachix/install-nix-action#usage-with-flakes).
After Milestone 6, the [public source repository](https://github.com/enriquessosa/pulse-systemd)
was published. The [first hosted CI run](https://github.com/enriquessosa/pulse-systemd/actions/runs/41936356123)
passed both `check` and `nix` jobs at commit `799312f`, including Cargo, Nix, and
terminal PTY checks. This verifies that run, not future commits or release readiness.

## Known limits and remaining decisions

- Manager-known services only; unloaded installed service files are excluded.
  Refresh is manual; access and output bounds are documented in the README.
- Other systemd distributions, aarch64 runtime, and older Rust/systemd versions
  remain unverified. Nix shell evaluation is not proof of runtime compatibility.
- Unexpected internal worker termination may require restarting Pulse. Force-kill
  or an inaccessible terminal can prevent cleanup. No general process monitor,
  other unit types, remote hosts, or service controls are planned for this MVP.
- Following Milestone 6, the owner authorized MIT adoption: Copyright (c) 2026
  Carlos Enrique Jimenez Sosa. See [LICENSE](../LICENSE) and
  [third-party notices](../THIRD_PARTY_NOTICES.md). `publish = false` remains set.
- Binary-distribution licensing is not fully cleared. Verify screenshot ownership,
  incomplete upstream attribution, native/runtime libraries, and any bundled Nix
  closure before release; project MIT adoption does not relicense third-party code.
- Public source publication as `enriquessosa/pulse-systemd` is complete; MIT is
  adopted and the first hosted CI run passed. Version 0.1.0, tags, packages, and
  binary distributions remain unpublished and require separate owner approval.
- Historically, no remote, push, tag, license selection, or publication occurred
  during Milestone 6; MIT adoption and source publication followed that milestone.
