# Milestone handoff

- Milestone 5 delivered: r refreshes both managers from the list, or both panes
  for the inspected service. Manual only; the brief specifies no periodic refresh.
- Last successful data stays visible as STALE during pending/failed refreshes,
  with independent monotonic retrieval ages and errors per scope/pane. Success
  clears errors; successful empty results replace old snapshots. No-data failures
  remain errors. Search/filters, scoped selection, tabs, and offsets are retained
  where valid; removed rows and shrinking results clamp safely.
- Discovery and inspection share latest_worker.rs: one running/one replaceable
  pending request and two results per worker, cancellable under backpressure.
  Request generations reject obsolete results; inspection also checks scope/name.
  Existing read-only arguments, timeout/output limits, terminal restoration, and
  child reaping are preserved. No new dependencies.
- Validation: fmt, Clippy, 46 behavior tests, and build passed; Nix flake checks
  passed for both Linux definitions. The ignored subprocess_fixture is invoked by
  runner tests. Live PTYs verified discovery/inspection refresh and navigation.
  Controlled PTYs verified stale snapshots, failures/recovery, overlap, timeout,
  empty results, no unsolicited fetching, and cleanup with both workers busy.
- Limits: manager-known units only; NixOS/systemd 260 verified. Other distributions
  and aarch64 runtime remain unverified. No service changes, push, or publication.
- Next: Milestone 6 release-readiness checks and documentation/shortcut review.
