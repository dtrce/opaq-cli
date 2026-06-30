# opaq

CLI for [opaq](https://github.com/dtrce/opaq-server), a self-hosted config and secret store.

> ⚠️ **Young project.** opaq is pre-1.0 and under active development. Breaking changes (commands, flags, config format) may land between releases until it stabilizes.

## Install

```sh
cargo install opaq
```

## Usage

```sh
opaq login --server https://opaq.example.com --key opaq_abc123
opaq set /acme/api/prod/STRIPE_KEY --string sk_live_xxx
opaq get /acme/api/prod/STRIPE_KEY --raw | pbcopy
opaq env /acme/api/prod
```

### Profiles

Manage multiple credential sets (server + key) in one config, AWS-style. `opaq login`
without `--profile` writes the `default` profile; an existing single-credential config
is migrated to `default` automatically.

```sh
opaq login --profile work --server https://work.opaq.com --key opaq_xyz
opaq get /acme/api/prod/STRIPE_KEY --profile work   # --profile works on any command
opaq profile list                                    # * marks the active profile
opaq profile remove work
```

Active profile is resolved in this order: `--profile` flag → `OPAQ_PROFILE` env →
`OPAQ_SERVER`+`OPAQ_KEY` raw env → the `default` profile.

### Env credentials (CI / Docker)

Set `OPAQ_SERVER` and `OPAQ_KEY` to make calls without running `opaq login` — handy
for docker entrypoints. Env values override the saved `config.json`.

```sh
export OPAQ_SERVER=https://opaq.example.com
export OPAQ_KEY=opaq_abc123
opaq get /acme/api/prod/STRIPE_KEY --raw
```

For wiring opaq into a Docker image + entrypoint, see
[`skills/opaq-docker-entrypoint`](skills/opaq-docker-entrypoint/SKILL.md) — a guide you
can hand straight to a coding agent.

Run `opaq help` for the full cheatsheet, or `opaq <command> --help` for details.

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
