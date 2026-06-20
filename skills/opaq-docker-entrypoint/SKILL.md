---
name: opaq-docker-entrypoint
description: Use when wiring the opaq CLI into a Docker image and entrypoint script to load config/secrets at container start without an interactive `opaq login`. Covers install, env-based credentials (OPAQ_SERVER/OPAQ_KEY), entrypoint patterns, and security pitfalls.
---

# Integrate opaq into a Docker entrypoint

`opaq` is a single-binary CLI that reads config/secrets from an opaq-server over HTTPS.
In containers it authenticates from two env vars — **no `opaq login`, no config file needed**:

| Var | Meaning | Example |
| --- | --- | --- |
| `OPAQ_SERVER` | server base URL | `https://opaq.example.com` |
| `OPAQ_KEY` | bearer API key (a principal key) | `opaq_abc123` |

Both must be present. They override any `~/.config/opaq/config.json`. Requires opaq **>= 0.2.0**.

## 1. Install the binary in the image

Prefer a release binary over `cargo install` (smaller, faster builds). Pin a version.

```dockerfile
# multi-stage: grab the static linux binary
FROM debian:bookworm-slim AS opaq
ARG OPAQ_VERSION=0.2.0
ARG TARGETARCH
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
 && case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-gnu ;; \
      arm64) target=aarch64-unknown-linux-gnu ;; \
      *) echo "unsupported arch: $TARGETARCH" >&2; exit 1 ;; \
    esac \
 && curl -fsSL "https://github.com/dtrce/opaq-cli/releases/download/v${OPAQ_VERSION}/opaq-${target}.tar.gz" \
      | tar -xz -C /usr/local/bin opaq \
 && opaq --version

FROM your-app-base
COPY --from=opaq /usr/local/bin/opaq /usr/local/bin/opaq
```

> Adjust the asset name/URL to match the project's release workflow. If a prebuilt
> binary isn't published for your platform, fall back to `cargo install opaq --version 0.2.0`
> in a builder stage.

## 2. Entrypoint patterns

Pick one based on how the app consumes config. All assume `OPAQ_SERVER` and `OPAQ_KEY`
are injected at **runtime** (see security note), not baked into the image.

### A. Export a whole env into the process (most common)

```sh
#!/bin/sh
set -eu

: "${OPAQ_SERVER:?OPAQ_SERVER not set}"
: "${OPAQ_KEY:?OPAQ_KEY not set}"
: "${OPAQ_PATH:?OPAQ_PATH not set}"   # e.g. /acme/api/prod

# fail fast if creds/server are bad before launching the app
opaq status >/dev/null

# load KEY=VAL lines into the environment, then exec the app
set -a
eval "$(opaq env "$OPAQ_PATH" --shell)"
set +a

exec "$@"
```

`opaq env <path> --shell` emits `export KEY='val'` lines (single-quote-escaped, safe for `eval`).
Use plain `opaq env <path>` for dotenv-style `KEY=VAL` lines if you write a file instead.

### B. Write a dotenv file for the app to read

```sh
#!/bin/sh
set -eu
: "${OPAQ_SERVER:?}" "${OPAQ_KEY:?}" "${OPAQ_PATH:?}"
opaq env "$OPAQ_PATH" > /run/secrets/app.env   # tmpfs, not the image layer
exec "$@"
```

### C. Fetch one secret

```sh
#!/bin/sh
set -eu
export DATABASE_URL="$(opaq get /acme/api/prod/DATABASE_URL --raw)"
exec "$@"
```

`--raw` prints only the value (no formatting), pipe-safe.

## 3. Wire it up

```dockerfile
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["your-app"]
```

Run with creds injected at runtime:

```sh
docker run --rm \
  -e OPAQ_SERVER=https://opaq.example.com \
  -e OPAQ_KEY="$OPAQ_KEY" \
  -e OPAQ_PATH=/acme/api/prod \
  your-image
```

## 4. Security — do not skip

- **Never bake `OPAQ_KEY` into the image** (no `ENV OPAQ_KEY=...`, no `ARG` that lands in a layer).
  Inject at runtime via orchestrator secrets (k8s Secret, ECS secret, `docker run -e`, compose `secrets:`).
- Write fetched secrets to **tmpfs** (`/run/...`), never an image layer or a bind-mounted host path.
- Use a **scoped, short-TTL principal key** for the container, not an admin key. Rotate via
  `opaq principal rotate`.
- `opaq status` early so a bad key fails the container at boot, not mid-request.
- `set -eu` so a failed `opaq` call aborts the entrypoint instead of starting the app blind.

## Command reference (container-relevant)

| Command | Use |
| --- | --- |
| `opaq status` | validate `OPAQ_SERVER` + `OPAQ_KEY` against the server |
| `opaq env <ws/proj/env>` | dotenv `KEY=VAL` lines |
| `opaq env <path> --shell` | `export KEY='val'` lines for `eval` |
| `opaq env <path> --preserve-case` | keep original key case |
| `opaq get <path> --raw` | single secret value, pipe-safe |

Full help: `opaq help` or `opaq <cmd> --help`.
