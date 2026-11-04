# Milestone handoff

- Milestone 3 delivered: service table with explicit scope/load/active/substate,
  selected-row summary, clamped/page navigation, search, scope/active-state filters,
  help, and resize handling. Selection follows (scope, name); hidden rows fall
  back to the first visible row. Search editing supports accept/cancel/clear.
- Discovery runs on one worker with a two-result channel; each scope reports
  loading, ready/empty, warnings, or failure independently. Exit cancels work,
  restores the terminal before joining, and preserves subprocess bounds/cleanup.
- Validation: fmt, Clippy, all 27 behavior tests, and build passed. Nix flake checks
  passed for both Linux definitions. The ignored subprocess_fixture is a child
  helper invoked by the runner tests, not an omitted behavior check.
- Real PTY checks passed against both local NixOS managers for navigation, search,
  scope, help, resizing, and cleanup. Controlled PTYs also passed normal/empty/
  partial/denied/missing-tool/slow/timeout cases; slow exit reaped the child.
- Limits: startup snapshot only (no refresh), manager-known units only, minimum
  list size 60x16, abbreviated status diagnostics. Other distributions and aarch64
  runtime remain unverified. No service mutations, push, or publication.
- Next: Milestone 4 service properties and bounded journal view, reusing the typed
  models and cancellable runner while keeping all I/O off the UI thread.
