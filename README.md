# Pulse

Pulse (Processes, Units, Logs, States & Events) is a read-only systemd service
observatory in development. Target MVP: 0.1.0. The acronym does not imply general
process monitoring.

The TUI lists system and user `.service` units with descriptions, typed
load/active/substates, and manager scope. It supports keyboard navigation,
case-insensitive search, scope and active-state filters, help, and resizing.
Discovery runs on a worker so input remains responsive, including while a manager
is unavailable. Each scope reports loading, successful empty results, warnings,
or errors independently. Selection follows the scope and unit name when results
arrive or visible rows change; when hidden, it falls back to the first visible row.

The data is a startup snapshot; refresh, properties, and journal views are not yet
implemented. The terminal is restored on normal exit, I/O errors, and unwinding
panics. Interactive stdin and stdout are required. No service controls or privilege
escalation are provided by Pulse.

## Setup and usage

Use a current stable Rust toolchain supporting edition 2024, or run `nix develop`
to enter the development shell. `flake.lock` pins nixpkgs and its Cargo, rustc,
rustfmt, and Clippy tools for x86_64 and aarch64 Linux. No system configuration
changes are needed. Initial setup requires network access for Nix inputs and
Cargo dependencies.

```sh
cargo run
```

To exercise discovery without the TUI:

```sh
cargo run --example discover
```

The TUI and diagnostic use the same discovery backend. The diagnostic reports both
scopes separately and exits unsuccessfully if either fails. Discovery uses
`systemctl list-units --all --type=service`, which lists
units currently known to each manager, including inactive and not-found entries.
Installed service files that are not loaded into a manager are not enumerated.
It retains warnings and failure diagnostics rather than treating errors as empty
results. Access depends on the current user's manager and bus permissions; Pulse
does not attempt escalation.

| Key | Action |
| --- | --- |
| `Up`/`Down`, `k`/`j` | Move selection (clamps at either end) |
| `PageUp`/`PageDown`, `Home`/`End` | Move a visible page, or first/last service |
| `/` | Edit search over unit names and descriptions |
| `Enter` / `Esc` while searching | Accept / cancel and restore the previous search and selection |
| `Backspace`, `Ctrl-U` while searching | Delete a character / clear search |
| `Tab` | Cycle scope: all, system, user |
| `f` | Cycle active-state filter: all, active, failed, inactive |
| `?` | Open help; `?` or `Esc` closes it |
| `q` or `Esc` while browsing | Quit (`q` also quits from help) |
| `Ctrl-C` | Quit in any mode |

Search and filters compose. Search input is limited to 256 bytes; `q` and `?`
are literal text during editing. To clear an accepted search, press `/`, `Ctrl-U`,
then `Enter`. Status summaries abbreviate long warnings/errors; the diagnostic
example reports their full bounded text. Terminal resizing preserves selection;
below 60 columns or 16 rows, a resize prompt replaces the list.

Forced termination such as SIGKILL cannot run cleanup. Compatibility with other
systemd Linux distributions is unverified.

## Development

State/input handling lives in `src/app.rs`, rendering in `src/ui.rs`, and terminal
setup/restoration in `src/terminal.rs`. Typed service models, listing parsing,
subprocess execution, and the systemd adapter live in separate modules. The
adapter accepts a mockable `Runner` and preserves independent scope results.
`src/worker.rs` runs synchronous discovery off the UI thread, delivering each scope
through a bounded channel. On exit it cancels and joins after terminal restoration.
UI scroll state stays in `ui::View`; selection/filter state stays in `App`.

The default runner limits each command to five seconds and 4 MiB of combined
stdout/stderr, supports cancellation, and kills/reaps its direct child on errors.
Nonblocking pipe reads use `rustix`, already present in the dependency graph, to
avoid deadlocks and uncancellable reader threads. It passes explicit arguments,
disables pager/color/hyperlinks, and fixes the locale. Invalid UTF-8 and malformed
listings fail explicitly; unknown state tokens retain their value. Terminal
controls and bidirectional formatting in descriptions/diagnostics are escaped.

Required checks:

```sh
nix flake check
nix develop --command cargo --version
nix develop
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --all-targets --all-features
```

Tests cover navigation/search/filter transitions, identity-based selection,
resizing/scrolling, scope isolation, loading/error/empty states, worker cancellation,
parsing fixtures, and actual subprocess limits. The ignored `subprocess_fixture` is
a child-process helper invoked by the runner tests, not a skipped behavior test.
CI runs the same four Cargo checks on pushes and PRs.
Manually verify help, resizing, each exit shortcut, and restored terminal settings
in an interactive terminal after building. See `docs/PROGRESS.md` for actual
validation status; declared tests are not evidence that they have passed.

## Roadmap and limitations

Next: properties and bounded journal entries, then nonblocking refresh with honest
error and freshness states, and release validation. Pulse does not currently
implement these features. Remote hosts, other unit types, service controls, and
general process monitoring are out of scope. No license has been selected and no
release is published.
