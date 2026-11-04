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

Enter opens selected-service properties and a bounded recent journal view. Each
pane independently shows loading, empty, permission-denied, unavailable, or error
results, preserving access warnings. Properties include identity, description,
load/active/substate, fragment path, unit-file state, main PID, exit code/status,
result, and active-enter timestamp. Empty values and unreported fields are distinct.

Discovery runs on startup and on manual refresh. Inspection is fetched when opened,
when switching services, and on manual refresh. There is no periodic refresh.
The terminal is restored on normal exit, I/O errors, and unwinding panics.
Interactive stdin and stdout are required. No service controls or privilege
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
| `r` from the list | Refresh both system and user discovery |
| `r` while inspecting | Refresh properties and journal for this service |
| `?` | Open help; `?` or `Esc` closes it |
| `Enter`, `l` from the list | Inspect properties / open journal |
| `Tab` while inspecting | Switch Properties / Journal |
| `Up`/`Down`, `k`/`j`, paging, `Home`/`End` while inspecting | Scroll the current pane |
| `Left`/`Right` while inspecting | Pan long property/log lines |
| `[`/`]` while inspecting | Inspect previous/next visible service |
| `Esc` while inspecting | Return to the list with selection preserved |
| `q` or `Esc` from the list | Quit (`q` also quits from help/inspection) |
| `Ctrl-C` | Quit in any mode |

Search and filters compose. Search input is limited to 256 bytes; `q`, `?`, and `r`
are literal text during editing. To clear an accepted search, press `/`, `Ctrl-U`,
then `Enter`. Status summaries abbreviate long warnings/errors; the diagnostic
example reports their full bounded text. Terminal resizing preserves selection;
below 60 columns or 16 rows, a resize prompt replaces the list.

During refresh, the last successful snapshot stays visible and is marked `STALE`.
Each scope and inspection pane shows the age of its own last successful retrieval.
A failed refresh preserves that data, its original timestamp, and the latest error;
press `r` to retry. Successful empty results replace old data. Initial failures
without a successful snapshot show errors without fabricated empty results.
Snapshot age is always visible; `STALE` specifically marks pending or failed
refreshes, rather than applying an arbitrary age threshold.

Refresh retains search, filters, selected scope/unit identity, the inspection tab,
and scroll/pan offsets where possible. Removed or filtered-out services fall back
to the first visible row; offsets clamp when results shrink. Repeated refreshes
cancel superseded work and replace the pending request; late results cannot
overwrite the current request, even for the same service. Refreshing inspection
does not also refresh the underlying service list.

Inspection retrieves the last 100 available journal entries in chronological order;
`End` jumps to the last displayed line. It does not follow or upload logs. Commands
retain the five-second timeout and are capped at 512 KiB of combined output per
inspection command. Display is capped at 2,000 journal lines and 4,096 input
characters per property/log line before control-character escaping, with visible
truncation markers. Exceeding the subprocess byte limit reports an error.
Invalid UTF-8 also reports an error. Warnings about
restricted journal visibility remain visible even with empty or partial results;
an empty result means no entries were returned, not proof that none exist.

Forced termination such as SIGKILL cannot run cleanup. Compatibility with other
systemd Linux distributions is unverified.

## Development

State/input handling lives in `src/app.rs`, rendering in `src/ui.rs`, and terminal
setup/restoration in `src/terminal.rs`. Typed service models, listing parsing,
subprocess execution, and the systemd adapter live in separate modules. The
adapter accepts a mockable `Runner` and preserves independent scope results.
`src/worker.rs` and `src/inspection_worker.rs` run synchronous adapters off the UI
thread, sharing `latest_worker.rs`: one running request, one replaceable pending
request, and two bounded results per worker. Request IDs protect discovery;
scope/name/request IDs also protect inspection, including reopening a service.
Closing inspection cancels its request. Cancellation remains active even with a
full result queue. Workers join after terminal restoration. List scroll state stays
in `ui::View`; selection/filter and inspection state stay in `App` and
`inspection.rs`. `freshness.rs` tracks monotonic retrieval age and refresh errors
independently of the retained data.

The default runner limits each command to five seconds and 4 MiB of combined
stdout/stderr, supports cancellation, and kills/reaps its direct child on errors.
Nonblocking pipe reads use `rustix`, already present in the dependency graph, to
avoid deadlocks and uncancellable reader threads. It passes explicit arguments,
disables pager/color/hyperlinks, and fixes the locale. Invalid UTF-8 and malformed
listings fail explicitly; unknown state tokens retain their value. Terminal
controls and bidirectional formatting in descriptions/diagnostics are escaped.
Inspection values and journal messages receive the same treatment. Property reads
use explicit `systemctl show` arguments with `--` before the validated service
name. The journal adapter uses scope-specific `--unit=` / `--user-unit=` arguments
and disables the pager without suppressing privilege warnings. No shell is used.

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
property/journal fixtures, unsafe-name rejection, request cancellation/late results,
refresh failure/recovery, selection retention, and actual subprocess limits.
The ignored `subprocess_fixture` is
a child-process helper invoked by the runner tests, not a skipped behavior test.
CI runs the same four Cargo checks on pushes and PRs.
Manually verify help, resizing, each exit shortcut, and restored terminal settings
in an interactive terminal after building. See `docs/PROGRESS.md` for actual
validation status; declared tests are not evidence that they have passed.

## Roadmap and limitations

Next: Milestone 6 release-readiness validation and review. Remote hosts, other unit
types, service controls, and general process monitoring remain out of scope.
No license has been selected and no release is published.
