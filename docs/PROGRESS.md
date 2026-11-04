# Milestone handoff

- Milestone 2 implemented: typed scope/service states, strict listing parser,
  independent system/user results, mockable systemd adapter, and diagnostic
  `cargo run --example discover`. TUI listing/search remain Milestone 3.
- Runner uses explicit arguments, fixed locale, no pager/color/hyperlinks, a
  five-second/4 MiB combined-output bound, cancellation, and direct-child cleanup.
  rustix was already transitive; its direct use enables safe nonblocking pipes.
- Validation passed: Nix flake checks for both Linux definitions and the full
  pinned-shell fmt/Clippy/test/build gate (16 tests). The ignored subprocess_fixture
  is invoked by runner tests. PTY preview/help/resize/q/Esc/Ctrl-C/cleanup passed.
- Live NixOS/systemd 260 check passed: 119 system and 37 user services, verified
  scope counts/columns. Sandboxed bus access fails explicitly; supported escalated
  read-only checks reached both managers. No unit or system configuration changes.
- Limits: discovery lists units in manager memory, not all installed unit files;
  real missing-user-manager/denied-access variants are fixture-tested. Other
  distributions and aarch64 runtime remain unverified. No push/publication.
- Next: Milestone 3 navigation and search, using a
  worker for synchronous discovery so the UI event loop never waits on systemctl.
