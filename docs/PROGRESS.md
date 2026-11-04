# Milestone handoff

- Milestone 4 delivered: Enter opens properties; l opens recent journal; Tab
  switches panes; Esc returns to the list; arrows/paging scroll and Left/Right pan;
  [/] inspect adjacent visible services. Existing list navigation remains intact.
- Validated .service keys, explicit scoped arguments, separate systemd/journal
  adapters, strict property parsing, escaped untrusted values, and independent
  loading/empty/permission-denied/unavailable/error states. Empty and unreported
  properties differ; journal access warnings remain visible.
- One inspection worker replaces pending requests and cancels superseded work;
  scope/name/generation reject stale results. Results are bounded; workers join
  after terminal restoration. Limits: 100 journal entries, 512 KiB per inspection
  command, five seconds, 2,000 display lines, 4,096 input chars per displayed line.
- Validation: full fmt/Clippy/test/build gate passed (38 behavior tests); Nix flake
  checks passed for both Linux definitions. The ignored subprocess_fixture is
  invoked by runner tests. Live PTYs read system/user properties and logs;
  controlled PTYs passed empty/denied/partial/malformed/missing/slow/timeout/overflow
  states, rapid selection changes, scrolling, resize, and cleanup/child reaping.
- Limits: independent snapshots without refresh; manager-known units only;
  NixOS/systemd 260 verified, other distributions/aarch64 runtime unverified.
  No service mutations, push, or publication.
- Next: Milestone 5 refresh, freshness/staleness, selection retention, and recovery.
