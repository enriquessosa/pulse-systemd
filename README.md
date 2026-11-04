# Pulse

Pulse (Processes, Units, Logs, States & Events) is a read-only systemd service
observatory in development. Target MVP: 0.1.0. The acronym does not imply general
process monitoring.

The foundation preview provides a Ratatui/Crossterm terminal screen, help, resize
handling, and terminal restoration on normal exit, I/O errors, and unwinding
panics. It requires interactive stdin and stdout. It does not yet query systemd
or display services, properties, or logs. No service controls or privilege
escalation are provided.

## Setup and usage

Use a current stable Rust toolchain supporting edition 2024, or run `nix develop`
to enter the development shell. `flake.lock` pins nixpkgs and its Cargo, rustc,
rustfmt, and Clippy tools for x86_64 and aarch64 Linux. No system configuration
changes are needed. Initial setup requires network access for Nix inputs and
Cargo dependencies.

```sh
cargo run
```

| Key | Action |
| --- | --- |
| `?` | Toggle help |
| `q`, `Esc`, `Ctrl-C` | Quit |

Terminal resizing redraws the screen. Forced termination such as SIGKILL cannot
run cleanup. Compatibility with other systemd Linux distributions is unverified.

## Development

State/input handling lives in `src/app.rs`, rendering in `src/ui.rs`, and terminal
setup/restoration in `src/terminal.rs`. Future typed models, parsing, subprocess
execution, and systemd/journal adapters should remain separate and mockable.

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

Tests cover help/exit events, ignored key releases, tiny terminal rendering, and
noninteractive startup errors. CI runs the same four checks on pushes and PRs.
Manually verify help, resizing, each exit shortcut, and restored terminal settings
in an interactive terminal after building. See `docs/PROGRESS.md` for actual
validation status; declared tests are not evidence that they have passed.

## Roadmap and limitations

Next: system/user `.service` discovery and robust parsing, then navigation/search,
properties and bounded journal entries, nonblocking refresh with honest error and
freshness states, and release validation. Pulse does not currently implement these
features. Remote hosts, other unit types, service controls, and general process
monitoring are out of scope. No license has been selected and no release is published.
