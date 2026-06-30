# opaq CLI — Credential Profiles

**Date:** 2026-06-30
**Status:** Approved, ready for implementation plan

## Goal

Let one machine hold multiple named opaq credential sets (server + API key) and
switch between them, AWS-CLI style, via a `--profile` flag and `OPAQ_PROFILE`
env var. Existing single-credential users and Docker entrypoints keep working
unchanged.

## Storage

Keep the existing file path `~/.config/opaq/config.json` (no rename — avoids
doc and entrypoint churn). Change its schema from a flat object to a profile
map:

```json
{
  "profiles": {
    "default": { "server": "https://opaq.example.com", "api_key": "opaq_..." },
    "work":    { "server": "https://work.opaq.com",     "api_key": "opaq_..." }
  }
}
```

File permissions stay `0600` on write (unchanged from today).

### Auto-migration

On load, if the file parses as the old flat shape `{ "server": ..., "api_key": ... }`
(detected by a top-level `server` key, no `profiles` key), wrap it as
`profiles.default` and rewrite the file in the new shape. One-time, silent,
backward compatible. After migration the user's existing credential is the
`default` profile, so every command keeps working with zero flags.

## Selection / precedence

`resolve_config` gains a `profile: Option<String>` argument (from the `--profile`
flag) and reads `OPAQ_PROFILE`. Resolution order, highest first:

1. `--profile <name>` flag
2. `OPAQ_PROFILE` env var
3. `OPAQ_SERVER` + `OPAQ_KEY` raw env creds (existing all-or-nothing path — if
   either is set both must be set, and the config file is ignored)
4. the `default` profile from the file

Notes:
- The `--profile` flag wins over raw env creds because it is passed explicitly.
- Raw env creds (path 3) remain unchanged, so Docker entrypoints that set
  `OPAQ_SERVER`/`OPAQ_KEY` and pass no flag/`OPAQ_PROFILE` behave exactly as today.
- An explicitly named profile (paths 1–2) that does not exist in the file is a
  hard error: `profile '<name>' not found`.
- If resolution falls through to `default` and no `default` profile exists, emit
  the existing "not logged in" error with a hint to run `opaq login` or pass
  `--profile`.

## Commands

- `opaq login [--profile NAME] --server URL --key KEY`
  Upserts that profile (omitting `--profile` targets `default`). Creating the
  file or adding a profile preserves other profiles.
- `opaq profile list`
  Lists profile names with their server and a masked key tail (last 4 chars,
  matching the existing `status` display). Marks which profile would resolve as
  active given current flags/env.
- `opaq profile remove NAME`
  Deletes one profile. No confirmation prompt; no special protection for
  `default`. Removing the last profile leaves an empty profile map, which simply
  reads as "not logged in".

A global `--profile <name>` flag is available on every command that reaches the
API (`status`, `get`, `set`, and the rest). It is threaded into `load_config` →
`resolve_config`.

## Error handling

- Named profile not found → `profile '<name>' not found` (non-zero exit).
- Fall-through to missing `default` → existing "not logged in" error + hint.
- `OPAQ_SERVER` set without `OPAQ_KEY` (or vice versa) → existing all-or-nothing
  error, unchanged.
- Malformed config file → existing "invalid config" parse error.

## Testing

Unit tests around the pure logic (no network):

- **Migration:** old flat-shape JSON parses and converts to `profiles.default`.
- **Precedence:** `resolve_config` returns the right creds for each of the four
  paths, including flag-over-env-creds and `OPAQ_PROFILE`-over-default.
- **Unknown profile:** explicitly named missing profile errors.
- **Upsert:** `login --profile work` adds `work` without dropping `default`.

## Out of scope (YAGNI)

- Stored "current/active" profile pointer + `profile use` (switching is
  per-command via flag/env).
- `aws configure`-style interactive wizard.
- Confirmation prompts / `default` deletion protection.
- File rename to `profiles.json`.
