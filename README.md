# Pulse

Pulse (Processes, Units, Logs, States & Events) is a read-only systemd service
observatory in development. Target MVP: 0.1.0. The acronym does not imply general
process monitoring.

The TUI preview provides help, resize handling, and terminal restoration on normal
exit, I/O errors, and unwinding panics. It requires interactive stdin and stdout.
The discovery backend lists system and user `.service` units with descriptions,
typed load/active/substates, and manager scope. TUI service navigation is next;
the current screen does not query either manager. Properties and logs are not yet
implemented. No service controls or privilege escalation are provided by Pulse.

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

This read-only diagnostic reports both scopes separately and exits unsuccessfully
if either fails. It uses `systemctl list-units --all --type=service`, which lists
units currently known to each manager, including inactive and not-found entries.
Installed service files that are not loaded into a manager are not enumerated.
It retains warnings and failure diagnostics rather than treating errors as empty
results. Access depends on the current user's manager and bus permissions; Pulse
does not attempt escalation.

| Key | Action |
| --- | --- |
| `?` | Toggle help |
| `q`, `Esc`, `Ctrl-C` | Quit |

Terminal resizing redraws the screen. Forced termination such as SIGKILL cannot
run cleanup. Compatibility with other systemd Linux distributions is unverified.

## Development

State/input handling lives in `src/app.rs`, rendering in `src/ui.rs`, and terminal
setup/restoration in `src/terminal.rs`. Typed service models, listing parsing,
subprocess execution, and the systemd adapter live in separate modules. The
adapter accepts a mockable `Runner` and preserves independent scope results.
Discovery is synchronous and must run on a worker when integrated into the TUI.

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

Tests cover UI basics, parsing fixtures, scope isolation, failure/empty states,
and actual subprocess limits/cancellation. The ignored `subprocess_fixture` is
a child-process helper invoked by the runner tests, not a skipped behavior test.
CI runs the same four Cargo checks on pushes and PRs.
Manually verify help, resizing, each exit shortcut, and restored terminal settings
in an interactive terminal after building. See `docs/PROGRESS.md` for actual
validation status; declared tests are not evidence that they have passed.

## Roadmap and limitations

Next: TUI service navigation/search, then properties and bounded journal entries,
nonblocking refresh with honest error and freshness states, and release validation.
Pulse does not currently implement these
features. Remote hosts, other unit types, service controls, and general process
monitoring are out of scope. No license has been selected and no release is published.
