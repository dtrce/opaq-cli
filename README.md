# opaq

CLI for [opaq](https://github.com/dtrce/opaq-server), a self-hosted config and secret store.

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

### Env credentials (CI / Docker)

Set `OPAQ_SERVER` and `OPAQ_KEY` to make calls without running `opaq login` — handy
for docker entrypoints. Env values override the saved `config.json`.

```sh
export OPAQ_SERVER=https://opaq.example.com
export OPAQ_KEY=opaq_abc123
opaq get /acme/api/prod/STRIPE_KEY --raw
```

Run `opaq help` for the full cheatsheet, or `opaq <command> --help` for details.

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
