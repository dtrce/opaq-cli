# CLI Credential Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the opaq CLI store multiple named credential sets (server + API key) in one config file and switch between them via `--profile`/`OPAQ_PROFILE`, AWS-style, with transparent migration of existing single-credential configs.

**Architecture:** Replace the flat `config.json` schema with a `{ "profiles": { name: {server,api_key} } }` map. A new `ProfileStore` type owns load (with one-time migration from the flat shape), save, and lookup. `resolve_config` grows flag/env-profile inputs and a four-level precedence. A single `#[arg(global = true)]` `--profile` flag on the root `Cli` struct reaches every subcommand with no per-command churn.

**Tech Stack:** Rust, clap (derive), serde / serde_json, std `BTreeMap`.

## Global Constraints

- File path stays `~/.config/opaq/config.json` — no rename. (`config_path()` unchanged.)
- Config file written with mode `0600` on unix (existing behavior in `save_config`).
- Precedence, highest first: `--profile` flag → `OPAQ_PROFILE` env → `OPAQ_SERVER`+`OPAQ_KEY` raw env (all-or-nothing) → `default` profile.
- Explicitly named profile (flag or `OPAQ_PROFILE`) that is absent → hard error `profile '<name>' not found`.
- Tests live in `#[cfg(test)]` modules inside `src/main.rs`, matching the existing `resolve_config_tests` / `parse_ttl_tests` style. Run with `cargo test`.

---

### Task 1: ProfileStore data model + migration load/save

**Files:**
- Modify: `src/main.rs` — add `use std::collections::BTreeMap;` (near line 7); add `ProfileStore` type and store functions next to `CliConfig`/`save_config` (lines 277–356); add tests in a new `#[cfg(test)] mod profile_store_tests`.

**Interfaces:**
- Consumes: existing `CliConfig { server, api_key }` (line 277), `config_path()`, the `0600` perm logic in `save_config`.
- Produces:
  - `struct ProfileStore { profiles: BTreeMap<String, CliConfig> }` (derives `Serialize, Deserialize, Default`)
  - `fn parse_store(data: &str) -> CliResult<ProfileStore>` — parses new shape, else migrates flat shape into `default`.
  - `fn load_store() -> CliResult<ProfileStore>` — reads file, calls `parse_store`, rewrites file if a migration happened.
  - `fn save_store(store: &ProfileStore) -> CliResult<()>` — writes pretty JSON at `0600` (factored out of `save_config`).

- [ ] **Step 1: Write the failing tests**

Add at the end of `src/main.rs`:

```rust
#[cfg(test)]
mod profile_store_tests {
    use super::{parse_store, CliConfig, ProfileStore};

    #[test]
    fn parses_new_shape() {
        let json = r#"{"profiles":{"work":{"server":"https://w.com","api_key":"wk"}}}"#;
        let store = parse_store(json).unwrap();
        assert_eq!(store.profiles["work"].server, "https://w.com");
        assert_eq!(store.profiles["work"].api_key, "wk");
    }

    #[test]
    fn migrates_flat_shape_into_default() {
        // old config.json was a bare {server, api_key}
        let json = r#"{"server":"https://old.com","api_key":"oldkey"}"#;
        let store = parse_store(json).unwrap();
        assert_eq!(store.profiles.len(), 1);
        assert_eq!(store.profiles["default"].server, "https://old.com");
        assert_eq!(store.profiles["default"].api_key, "oldkey");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_store("not json").is_err());
    }

    #[test]
    fn round_trips_through_serde() {
        let mut store = ProfileStore::default();
        store.profiles.insert(
            "default".into(),
            CliConfig { server: "https://s.com".into(), api_key: "k".into() },
        );
        let json = serde_json::to_string(&store).unwrap();
        let back = parse_store(&json).unwrap();
        assert_eq!(back.profiles["default"].server, "https://s.com");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test profile_store_tests`
Expected: FAIL — `parse_store` and `ProfileStore` not defined (compile error).

- [ ] **Step 3: Add `BTreeMap` import**

At `src/main.rs:7`, after `use std::path::PathBuf;`, add:

```rust
use std::collections::BTreeMap;
```

- [ ] **Step 4: Add the type and functions**

Right after the `CliConfig` struct (currently ends at line 281), add:

```rust
#[derive(Serialize, Deserialize, Default)]
struct ProfileStore {
    profiles: BTreeMap<String, CliConfig>,
}

// Parses the new profile-map shape. If the data is instead the legacy flat
// {server, api_key} config, wrap it as the `default` profile (one-time migration).
fn parse_store(data: &str) -> CliResult<ProfileStore> {
    if let Ok(store) = serde_json::from_str::<ProfileStore>(data) {
        return Ok(store);
    }
    // Fall back to the legacy flat shape before giving up.
    let flat: CliConfig = serde_json::from_str(data)
        .map_err(|e| format!("invalid config: {}", e))?;
    let mut profiles = BTreeMap::new();
    profiles.insert("default".to_string(), flat);
    Ok(ProfileStore { profiles })
}
```

Then add load/save next to `load_config_file`/`save_config` (after line 356):

```rust
// Reads the config file into a ProfileStore. If the file was the legacy flat
// shape, it is migrated and rewritten in place so the next read is the new shape.
fn load_store() -> CliResult<ProfileStore> {
    let path = config_path();
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let was_flat = serde_json::from_str::<ProfileStore>(&data).is_err();
    let store = parse_store(&data)?;
    if was_flat {
        save_store(&store)?;
    }
    Ok(store)
}

fn save_store(store: &ProfileStore) -> CliResult<()> {
    let path = config_path();
    let parent = path
        .parent()
        .ok_or_else(|| "could not determine config directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("failed to create config dir: {}", e))?;
    let body = serde_json::to_string_pretty(store)
        .map_err(|e| format!("failed to serialize config: {}", e))?;
    std::fs::write(&path, body).map_err(|e| format!("failed to write config: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)
            .map_err(|e| format!("failed to read config permissions: {}", e))?
            .permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)
            .map_err(|e| format!("failed to set config permissions: {}", e))?;
    }
    Ok(())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test profile_store_tests`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: ProfileStore model with legacy-config migration"
```

---

### Task 2: Four-level precedence in resolve_config

**Files:**
- Modify: `src/main.rs` — `resolve_config` (lines 314–328) and `load_config` (lines 306–310); update the existing `resolve_config_tests` (lines 1330–1376) to the new signature and add precedence cases.

**Interfaces:**
- Consumes: `ProfileStore` (Task 1), `CliConfig`, `env_var` (line 299).
- Produces:
  - `fn resolve_config(flag_profile: Option<String>, env_profile: Option<String>, env_server: Option<String>, env_key: Option<String>, store: Option<ProfileStore>) -> CliResult<CliConfig>`
  - `fn load_config(flag_profile: Option<String>) -> CliResult<CliConfig>`

- [ ] **Step 1: Rewrite the failing tests**

Replace the entire `mod resolve_config_tests` block (lines 1330–1376) with:

```rust
#[cfg(test)]
mod resolve_config_tests {
    use super::{resolve_config, CliConfig, ProfileStore};
    use std::collections::BTreeMap;

    fn store() -> Option<ProfileStore> {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "default".into(),
            CliConfig { server: "https://default.com".into(), api_key: "dk".into() },
        );
        profiles.insert(
            "work".into(),
            CliConfig { server: "https://work.com".into(), api_key: "wk".into() },
        );
        Some(ProfileStore { profiles })
    }

    #[test]
    fn flag_profile_wins_over_env_creds() {
        let conf = resolve_config(
            Some("work".into()),
            None,
            Some("https://env.com".into()),
            Some("ek".into()),
            store(),
        )
        .unwrap();
        assert_eq!(conf.server, "https://work.com");
    }

    #[test]
    fn env_profile_selects_named() {
        let conf = resolve_config(None, Some("work".into()), None, None, store()).unwrap();
        assert_eq!(conf.api_key, "wk");
    }

    #[test]
    fn named_profile_not_found_errors() {
        assert!(resolve_config(Some("nope".into()), None, None, None, store()).is_err());
    }

    #[test]
    fn raw_env_creds_when_no_profile() {
        let conf = resolve_config(
            None,
            None,
            Some("https://env.com".into()),
            Some("ek".into()),
            store(),
        )
        .unwrap();
        assert_eq!(conf.server, "https://env.com");
    }

    #[test]
    fn partial_env_errors() {
        assert!(resolve_config(None, None, Some("https://env.com".into()), None, store()).is_err());
        assert!(resolve_config(None, None, None, Some("ek".into()), store()).is_err());
    }

    #[test]
    fn falls_through_to_default() {
        let conf = resolve_config(None, None, None, None, store()).unwrap();
        assert_eq!(conf.server, "https://default.com");
    }

    #[test]
    fn errors_when_nothing_set_and_no_store() {
        assert!(resolve_config(None, None, None, None, None).is_err());
    }

    #[test]
    fn errors_when_default_missing() {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "work".into(),
            CliConfig { server: "https://work.com".into(), api_key: "wk".into() },
        );
        let store = Some(ProfileStore { profiles });
        assert!(resolve_config(None, None, None, None, store).is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test resolve_config_tests`
Expected: FAIL — `resolve_config` arity mismatch (compile error).

- [ ] **Step 3: Rewrite `resolve_config` and `load_config`**

Replace `load_config` (lines 306–310) and `resolve_config` (lines 312–328) with:

```rust
fn load_config(flag_profile: Option<String>) -> CliResult<CliConfig> {
    resolve_config(
        flag_profile,
        env_var("OPAQ_PROFILE"),
        env_var("OPAQ_SERVER"),
        env_var("OPAQ_KEY"),
        load_store().ok(),
    )
}

// Precedence, highest first:
//   1. --profile flag        2. OPAQ_PROFILE env
//   3. OPAQ_SERVER+OPAQ_KEY raw env (all-or-nothing)   4. the `default` profile
fn resolve_config(
    flag_profile: Option<String>,
    env_profile: Option<String>,
    env_server: Option<String>,
    env_key: Option<String>,
    store: Option<ProfileStore>,
) -> CliResult<CliConfig> {
    // An explicitly named profile (flag or env) takes precedence over raw env creds.
    if let Some(name) = flag_profile.or(env_profile) {
        return pick_profile(store, &name);
    }
    match (env_server, env_key) {
        (Some(server), Some(api_key)) => Ok(CliConfig { server, api_key }),
        (Some(_), None) => Err("OPAQ_SERVER is set but OPAQ_KEY is missing.".to_string()),
        (None, Some(_)) => Err("OPAQ_KEY is set but OPAQ_SERVER is missing.".to_string()),
        (None, None) => pick_profile(store, "default"),
    }
}

fn pick_profile(store: Option<ProfileStore>, name: &str) -> CliResult<CliConfig> {
    let store = store.ok_or_else(|| {
        "not logged in. Run `opaq login --server URL --key KEY`, or set OPAQ_SERVER and OPAQ_KEY."
            .to_string()
    })?;
    store
        .profiles
        .get(name)
        .map(|c| CliConfig { server: c.server.clone(), api_key: c.api_key.clone() })
        .ok_or_else(|| format!("profile '{}' not found", name))
}
```

Note: when `name == "default"` and the store has no `default`, the `not found` error reads `profile 'default' not found` — acceptable and accurate. (The empty-store case still yields the "not logged in" hint via the `ok_or_else` above.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test resolve_config_tests`
Expected: PASS (8 tests). The crate will not fully build yet because callers of `load_config()` now need an argument — that is Task 3. To check just this module compiles, expect the failures to be only at `load_config()` call sites, not in `resolve_config`/`pick_profile`.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: four-level profile precedence in resolve_config"
```

---

### Task 3: Global --profile flag, threaded load_config, login upsert

**Files:**
- Modify: `src/main.rs` — `Cli` struct (lines 70–73); `run()` signature/body to capture the flag (line 814+); every `load_config()` call site; `Command::Login` handler (lines 832–840).

**Interfaces:**
- Consumes: `load_config(Option<String>)` (Task 2), `load_store`/`save_store`/`ProfileStore` (Task 1).
- Produces: `Command::Login` upserts into the store under the chosen profile name (default `"default"`), preserving other profiles. All API commands resolve creds for the active profile.

- [ ] **Step 1: Add the global flag to `Cli`**

Replace the `Cli` struct (lines 70–73) with:

```rust
struct Cli {
    /// Credential profile to use (overrides OPAQ_PROFILE; defaults to `default`)
    #[arg(long, global = true)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Command,
}
```

- [ ] **Step 2: Capture the flag in `run()` and thread it**

In `run()` (line 814), after `let cli = Cli::parse();` add:

```rust
    let profile = cli.profile.clone();
```

Then change every `load_config()?` call in the `match` to `load_config(profile.clone())?`.

Search to confirm you caught all of them:

Run: `rg -n 'load_config\(\)' src/main.rs`
Expected after edit: no matches.

- [ ] **Step 3: Make `Login` upsert into the store**

Replace the `Command::Login` arm (lines 832–840) with:

```rust
        Command::Login { server, key } => {
            let name = profile.clone().unwrap_or_else(|| "default".to_string());
            // Preserve any existing profiles; start fresh if no file yet.
            let mut store = load_store().unwrap_or_default();
            store.profiles.insert(name.clone(), CliConfig { server, api_key: key });
            save_store(&store)?;
            println!("Logged in (profile: {}).", name);
            Ok(())
        }
```

- [ ] **Step 4: Build and run the existing suite**

Run: `cargo build && cargo test`
Expected: builds clean; all tests pass (parse_ttl, profile_store, resolve_config).

- [ ] **Step 5: Manual smoke test**

Run:
```bash
cargo run -- login --server https://a.example.com --key key_a
cargo run -- login --profile work --server https://b.example.com --key key_b
cat ~/.config/opaq/config.json
```
Expected: file contains both `default` and `work` profiles; `default.server` is `https://a.example.com`.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: global --profile flag and profile-aware login"
```

---

### Task 4: `profile list` and `profile remove` commands

**Files:**
- Modify: `src/main.rs` — add `Profile { cmd: ProfileCmd }` to the `Command` enum (near line 199), add a `ProfileCmd` subcommand enum (near the `PrincipalCmd` enum, line 205), add handler arms in `run()`.

**Interfaces:**
- Consumes: `load_store`/`save_store`/`ProfileStore` (Task 1), `env_var`, `dim`/`bold` (imported at line 9), the active-name logic mirroring Task 2 precedence.
- Produces: `opaq profile list`, `opaq profile remove NAME`.

- [ ] **Step 1: Add the `Profile` command variant**

In the `Command` enum, before the closing `}` of `Help` (after line 202), add:

```rust
    /// Manage credential profiles
    Profile {
        #[command(subcommand)]
        cmd: ProfileCmd,
    },
```

- [ ] **Step 2: Add the `ProfileCmd` enum**

After the `PrincipalCmd` enum (after line 275), add:

```rust
#[derive(Subcommand)]
enum ProfileCmd {
    /// List saved profiles
    List,
    /// Remove a saved profile
    Remove {
        /// Profile name to delete
        name: String,
    },
}
```

- [ ] **Step 3: Add an active-profile-name helper**

Near `load_config` (after Task 2's `pick_profile`), add:

```rust
// The profile name that would resolve as active given flag/env, ignoring raw
// env creds (which are not a named profile). Used only for display in `profile list`.
fn active_profile_name(flag_profile: Option<String>) -> String {
    flag_profile
        .or_else(|| env_var("OPAQ_PROFILE"))
        .unwrap_or_else(|| "default".to_string())
}
```

- [ ] **Step 4: Add the handler arms in `run()`**

In the `match cli.command` block, add before the closing of the match (alongside the other arms):

```rust
        Command::Profile { cmd } => match cmd {
            ProfileCmd::List => {
                let store = load_store().unwrap_or_default();
                if store.profiles.is_empty() {
                    println!("No profiles. Run `opaq login` to create one.");
                    return Ok(());
                }
                let active = active_profile_name(profile.clone());
                for (name, conf) in &store.profiles {
                    let marker = if *name == active { "*" } else { " " };
                    let key_tail = if conf.api_key.len() >= 4 {
                        &conf.api_key[conf.api_key.len() - 4..]
                    } else {
                        conf.api_key.as_str()
                    };
                    println!(
                        "{} {}  {}  {}",
                        marker,
                        bold(name),
                        conf.server,
                        dim(&format!("...{}", key_tail)),
                    );
                }
                Ok(())
            }
            ProfileCmd::Remove { name } => {
                let mut store = load_store().unwrap_or_default();
                if store.profiles.remove(&name).is_none() {
                    return Err(format!("profile '{}' not found", name));
                }
                save_store(&store)?;
                println!("Removed profile: {}", name);
                Ok(())
            }
        },
```

- [ ] **Step 5: Build and exercise**

Run:
```bash
cargo build && cargo test
cargo run -- profile list
cargo run -- profile remove work
cargo run -- profile list
```
Expected: builds and tests pass; `list` shows remaining profiles with `*` on the active one; `remove work` drops it; `remove work` again errors `profile 'work' not found`.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: profile list and remove commands"
```

---

## Self-Review

**Spec coverage:**
- Storage / new schema → Task 1 (`ProfileStore`). ✓
- Auto-migration → Task 1 (`parse_store` flat fallback, `load_store` rewrite). ✓
- Four-level precedence → Task 2. ✓
- Unknown-profile hard error → Task 2 (`pick_profile`). ✓
- Missing-`default` "not logged in" → Task 2 (`pick_profile` ok_or_else + `profile 'default' not found`). ✓
- `login --profile` upsert preserving others → Task 3. ✓
- Global `--profile` on every API command → Task 3 (`#[arg(global = true)]`). ✓
- `profile list` with active marker + key tail → Task 4. ✓
- `profile remove` no-confirm, no default protection → Task 4. ✓
- `0600` perms preserved → Task 1 (`save_store`). ✓
- Docker raw-env path unchanged → Task 2 (env-creds branch identical logic). ✓
- Tests for migration / precedence / unknown / upsert → Tasks 1–3. ✓

**Placeholder scan:** none — every code step shows full code.

**Type consistency:** `ProfileStore { profiles: BTreeMap<String, CliConfig> }`, `parse_store`, `load_store`, `save_store`, `resolve_config(5 args)`, `load_config(Option<String>)`, `pick_profile`, `active_profile_name`, `ProfileCmd::{List,Remove}` used consistently across tasks.

**Note for executor:** `save_config` (the old single-profile writer at line 336) becomes dead after Task 3 removes its only caller (the old `Login` arm). Delete it in Task 3 Step 3 if the compiler warns `function is never used`; `load_config_file` likewise becomes unused once Task 2 lands — remove it to keep the build warning-clean.
