# AGENTS.md — opaq-cli

Guidance for AI coding agents working in this repo.

## Purpose

`opaq` is the CLI client for [opaq-server](https://github.com/dtrce/opaq-server), a self-hosted config/secret store. Single binary, no embedded server. Talks to the server over HTTPS using a bearer API key.

Published on crates.io as `opaq`. `cargo install opaq` is the user-facing install path.

## Layout

```
src/main.rs       # all CLI commands, parsing, HTTP client logic, helpers
src/help.rs       # `opaq help` cheatsheet renderer
src/style.rs      # ANSI color + comfy-table helpers
tests/cli_help.rs # integration tests, spawn the bin, assert on --help / help output
```

Single `[[bin]]` named `opaq`. No library crate.

## Common commands

```sh
cargo build                  # debug build
cargo build --release        # release build → target/release/opaq
cargo test                   # 21 tests, ~1s
cargo clippy --all-targets   # must be warning-free
cargo fmt                    # rustfmt
cargo publish --dry-run      # validate before crates.io publish
```

## Conventions

- **No `unwrap()` or `unsafe` in non-test code.** Use `?`, `ok_or`, `map_err`, or pattern-match. Tests may use `unwrap()` / `assert!` freely.
- All command handlers return `CliResult<()>` (alias for `Result<(), String>`). Errors print as `error: <msg>` to stderr, exit code 1.
- HTTP errors come back through `render_resp_error`, which extracts the server's JSON `error` field when present.
- Path parsing: `parse_secret_path` for 3- or 4-segment secret paths (`/ws/proj/key` or `/ws/proj/env/key`), `parse_path3` for scope paths (no key).
- Config path: `~/.config/opaq/config.json` on **all** platforms including macOS (override of dirs default `~/Library/Application Support`). 0600 perms on Unix.
- New flags: prefer kebab-case (`--no-ttl`, `--string-path`).

## Server contract

Base URL: `${server}/api/v1/`

| Op | Method + Path |
| --- | --- |
| Whoami | `GET /me` |
| Get secret | `GET /secrets/{ws}/{proj}[/{env}]/{key}` |
| Set secret | `PUT /secrets/{ws}/{proj}[/{env}]/{key}` body `{value, type}` |
| Delete secret | `DELETE /secrets/{ws}/{proj}[/{env}]/{key}` |
| List | `GET /list/{ws}/{proj}[/{env}]?values=true&merge=false` |
| Principals CRUD | `GET/PUT /principals`, `POST /principals/rotate`, `DELETE /principals/{id}` |

Auth header: `Authorization: Bearer <api_key>`.

When server endpoints change, this CLI must be updated in lockstep — there's no shared schema crate.

## Publishing checklist

1. Bump `version` in `Cargo.toml`.
2. `cargo test && cargo clippy --all-targets`.
3. `cargo publish --dry-run` clean.
4. Tag the release in git: `vX.Y.Z`.
5. `cargo publish`.

## What not to do

- Don't add server logic, DB, or crypto deps here. CLI stays thin — strictly an HTTP client + arg parser.
- Don't introduce async / tokio. CLI uses `reqwest::blocking` deliberately for simplicity.
- Don't take a workspace dep on `opaq-server` — repos are independent on purpose.
