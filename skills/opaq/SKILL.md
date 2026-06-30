---
name: opaq
description: Use when storing, fetching, or listing secrets/config with opaq, managing opaq API keys/principals/roles, calling the opaq-server HTTP API, deciding the 3-vs-4-segment secret path, authenticating the opaq CLI (OPAQ_SERVER/OPAQ_KEY, opaq login), or deploying/operating opaq-server. opaq is a self-hosted secret & config store (Rust CLI + server).
---

# opaq

**opaq** is a self-hosted secret & config store: a Rust server (`opaq-server`, SQLite + AES-256-GCM at rest) plus a single-binary CLI (`opaq`). You store strings or JSON at rigid hierarchical paths, hand out API keys with global roles, and pull values as raw text, JSON, or `KEY=VAL` env lines.

**Core facts that aren't guessable — verify against these, don't invent:**
- API base is `${server}/api/v1`. Bearer auth: `Authorization: Bearer opaq_<64hex>`.
- Paths are **exactly 3 or 4 segments**, each matching `[a-zA-Z0-9_-]{1,64}`. No inheritance on `get`.
- Roles are **global** (`reader`/`writer`/`admin`) — there is NO per-path access control.
- API keys are shown **once** at create/rotate; the server stores only an HMAC fingerprint.
- Server is plain HTTP (default `127.0.0.1:6727`); TLS must sit in front.

For wiring opaq into a Docker image/entrypoint, use the **opaq-docker-entrypoint** skill instead — don't duplicate that here.

## Path model (get this right first)

| Path | Segments | Scope | Meaning |
| --- | --- | --- | --- |
| `/workspace/project/KEY` | 3 | **project-scoped** | shared default across all envs |
| `/workspace/project/env/KEY` | 4 | **env-scoped** | environment-specific value |
| `/workspace/project` | 2 | scope only | target for `list` / no `get` |
| `/workspace/project/env` | 3-as-scope | scope only | target for `list` / `env` |

`get` does an **exact lookup with no fallback** — an env-scoped `get` does NOT inherit the project-scoped key. The merge (env-wins over project defaults) happens only in `list`/`env` operations, and can be disabled with `--no-merge` (CLI) / `merge=false` (API).

## CLI

Binary `opaq`. Install: `cargo install opaq` (or a pinned release tarball for CI/Docker). Run `opaq help [TOPIC]` (`setup|auth|secrets|admin|paths|examples`) or `opaq <cmd> --help`.

**Auth & config (profiles, since 0.3.0).** `~/.config/opaq/config.json` (0600, `~/.config` even on macOS) holds a **profile map**: `{ "profiles": { "default": { "server", "api_key" }, "work": {...} } }`. A legacy flat `{ "server", "api_key" }` config is auto-migrated into the `default` profile on first read. Resolution precedence, highest first: `--profile <name>` flag → `OPAQ_PROFILE` env → `OPAQ_SERVER`+`OPAQ_KEY` raw env (still **all-or-nothing** — set both or neither) → the `default` profile. The `--profile` flag is global (works on any command). An explicitly named profile that doesn't exist errors `profile '<name>' not found`.

```sh
opaq login --server https://opaq.example.com --key opaq_abc123   # writes the `default` profile
opaq login --profile work --server https://work.opaq.com --key opaq_xyz   # upserts `work`
opaq status                                                       # GET /me — identity, role, key tail
opaq get /acme/api/prod/KEY --profile work                        # use a profile for one command
opaq profile list                                                 # names + servers + key tails; * marks active
opaq profile remove work                                          # delete a profile (no confirm, no default protection)
```

Docker/CI entrypoints that set `OPAQ_SERVER`+`OPAQ_KEY` are unaffected — raw env creds still work with no profile or flag.

**Secrets.**
```sh
opaq set /acme/api/prod/STRIPE_KEY --string sk_live_xxx
opaq set /acme/api/CONFIG --json '{"timeout":30}'                 # also --string-path / --json-path <file>
opaq get /acme/api/prod/STRIPE_KEY [--raw]                        # --raw = value only, for piping
opaq list /acme/api [--values] [--no-merge]                       # project + envs; --values fetches each
opaq rm  /acme/api/prod/STRIPE_KEY
opaq env /acme/api/prod [--shell] [--preserve-case]               # KEY=VAL lines; --shell = export KEY='v'
```
`opaq env` merges project defaults in and uppercases keys by default. Consume it with `eval "$(opaq env /acme/api/prod --shell)"` or `--env-file <(opaq env /acme/api/dev)`.

**Admin (principals).** Admin role required.
```sh
opaq principal set <NAME> [--role reader|writer|admin] [--ttl 30d|12h|60m|3600s|3600 | --no-ttl] [--rename NAME]
opaq principal rotate (--id N | --name NAME)     # new key, same id/role/ttl; old key dies
opaq principal list                              # ID NAME ROLE EXPIRES REVOKED
opaq principal revoke (--id N | --name NAME)
```
`principal set` on a **new** name mints a key (printed once). On an existing name it updates in place and **never rotates the key** — use `rotate` for that. When both `--id` and `--name` are given, `--id` wins. `opaq genkey [--length N]` (min 32, default 64) produces a passphrase for the server's `OPAQ_MASTER_KEY`.

## Server HTTP API

Base `/api/v1`. All endpoints except `GET /healthz` require the bearer header. Roles: reader = GET/list; writer = + PUT/DELETE secrets; admin = + manage principals. Every 4xx/5xx body is `{ "error": "message" }`. `401` = missing/bad/revoked/expired key, `403` = role too low, `404` = not found, `413` = body > 1 MiB.

| Method + path | Role | Body / notes |
| --- | --- | --- |
| `GET /healthz` | none | `{ "ok": true }` |
| `GET /me` | any | `{ "principal": { id, name, role, expires_at } }` |
| `PUT /secrets/{ws}/{proj}[/{env}]/{key}` | writer | `{ "type": "string"\|"json", "value": "..." }` → `{ path, type }`. `value` is **always a string**; for `type:"json"` it must be a JSON-encoded string (e.g. `"{\"a\":1}"`, not `{"a":1}`) — server validates it parses. |
| `GET /secrets/{ws}/{proj}[/{env}]/{key}` | reader | → `{ path, type, value }`; 404 if absent (no fallback) |
| `DELETE /secrets/{ws}/{proj}[/{env}]/{key}` | writer | → `{ ok: true }` |
| `GET /list/{ws}/{proj}[/{env}]?values=true&merge=false` | reader | → `[ { path, type, value? } ]` |
| `PUT /principals` | admin | upsert by `name`; see below |
| `POST /principals/rotate` | admin or self | `{ "name" }` → `{ id, name, role, key, expires_at }` |
| `GET /principals` | admin | → `[ { id, name, role, created_at, revoked_at, expires_at } ]` |
| `DELETE /principals/{id}` | admin | → `{ ok: true }` |

`PUT /principals` body: `{ name (required), role?, ttl_seconds?, clear_ttl?, rename? }`. `ttl_seconds` and `clear_ttl` are mutually exclusive (400). Response includes `"action": "created"|"updated"`; the plaintext `"key"` is present **only** on `created`. Guards: cannot demote/revoke the last admin (403); name collision (400); rename of non-existent name (404).

```sh
curl -H "Authorization: Bearer $OPAQ_KEY" \
  -X PUT "$OPAQ_SERVER/api/v1/secrets/acme/api/prod/STRIPE_KEY" \
  -H 'Content-Type: application/json' -d '{"type":"string","value":"sk_live_xxx"}'

# JSON secret: "value" is the JSON encoded AS A STRING, not a nested object
curl -H "Authorization: Bearer $OPAQ_KEY" \
  -X PUT "$OPAQ_SERVER/api/v1/secrets/acme/api/prod/CONFIG" \
  -H 'Content-Type: application/json' -d '{"type":"json","value":"{\"a\":1}"}'
```

## Deploying opaq-server

Single binary, SQLite file, needs a master-key passphrase. Default bind is loopback — terminate TLS in front (Fly proxy / reverse proxy).

| Env var | Default | Purpose |
| --- | --- | --- |
| `OPAQ_MASTER_KEY` | **required** | passphrase ≥32 ASCII alphanumerics; Argon2id → AES-256 key. Server panics without it. |
| `OPAQ_HOST` | `127.0.0.1` | bind address |
| `OPAQ_PORT` | `6727` | bind port |
| `OPAQ_DB` | `opaq.db` | SQLite path (persist this volume) |

The master key never lives in the DB; a stolen DB is useless without it. There is **no master-key rotation** — choose it once. Root admin key is auto-created and printed on first boot; capture it. `just` recipes: `docker-build`, `docker-run` (ephemeral tmpfs `/data`), `docker-deploy` (persistent volume), `docker-logs`, `release VERSION`.

```sh
export OPAQ_MASTER_KEY=$(opaq genkey)
just docker-deploy        # then grab the root key from `just docker-logs`
```

## Common mistakes

| Mistake | Reality |
| --- | --- |
| Expecting env-scoped `get` to fall back to the project default | No inheritance. `get` is exact; only `list`/`env` merge. |
| Setting `OPAQ_SERVER` without `OPAQ_KEY` (or vice versa) | All-or-nothing — errors. Set both or rely on `config.json`. |
| Using `principal set` to rotate a key | `set` on an existing name never rotates. Use `principal rotate`. |
| Expecting to read a key again after creating a principal | Shown once. Server keeps only an HMAC fingerprint. Re-create or `rotate`. |
| Trying to scope a key to one path/workspace | Roles are global. No per-path ACL exists. |
| Pathing with 1, 2, or 5 segments | Exactly 3 (project) or 4 (env). Each segment `[a-zA-Z0-9_-]{1,64}`. |
| Sending a nested object as a JSON secret's `value` | `value` is always a string. For `type:"json"`, JSON-encode it: `"{\"a\":1}"`. |
| Exposing the server directly | Plain HTTP. Put TLS in front. |
| Planning a master-key rotation | Not implemented. The passphrase is permanent for that DB. |
