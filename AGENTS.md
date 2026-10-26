# AGENTS.md

Project-level guidance for AI coding agents working in this repository.

## Project Overview

`quickpick` is a Rust project (edition 2024). The development environment is managed with [devenv](https://devenv.sh/) and [direnv](https://direnv.net/), backed by Nix. Rust tooling is pinned via the `rust-overlay` Nix input and uses the **stable** channel.

## Development Environment

The dev shell provides:
- `rustc`, `cargo`, `clippy`, `rustfmt`, `rust-analyzer`
- `evcxr` (Rust REPL)

To enter the dev shell:
```sh
devenv shell   # or: direnv allow  (if direnv is configured)
```

Note that the rust commands (`cargo`, `rustc`, etc.) are provided by the dev shell and may not be available outside of it.

All commands below assume you are inside the dev shell.

## Build

```sh
cargo build
```

## Run

```sh
cargo run
```

## Test

```sh
cargo test
```

## Lint & Format

```sh
cargo clippy -- -D warnings   # lint; treat warnings as errors
cargo fmt --check             # check formatting without modifying files
cargo fmt                     # auto-format
```

## Verification Checklist

Before submitting changes, ensure the following all pass:

1. `cargo build` succeeds with no errors
2. `cargo test` passes
3. `cargo clippy -- -D warnings` produces no warnings or errors
4. `cargo fmt --check` reports no formatting issues

## Conventions

- Rust edition: **2024**
- Add dependencies with `cargo add <crate>` rather than editing `Cargo.toml` by hand.
- `Cargo.lock` is **gitignored** (library crate convention). Do not commit it.
- Environment variables are loaded from `.env` (gitignored). Do not commit secrets.
