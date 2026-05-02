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

Run `opaq help` for the full cheatsheet, or `opaq <command> --help` for details.

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
