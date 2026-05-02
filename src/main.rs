mod help;
mod style;

use clap::{Parser, Subcommand};
use comfy_table::{Cell, Color};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::ExitCode;
use style::{ansi, bold, dim, make_table};

const TOP_LONG_ABOUT: &str = "\
opaq — config/secret store CLI

Path syntax:
  /workspace/project/key            project-scoped secret
  /workspace/project/env/key        env-scoped secret
  /workspace/project[/env]          list/env scope path
";

const TOP_AFTER_HELP: &str = "\
Run `opaq help` for a quick cheatsheet of common commands,
or `opaq <command> --help` for details on a specific command.
";

const TOP_HELP_TEMPLATE: &str = "\
{before-help}{name} {version}
{about}

{usage-heading} {usage}

Setup:
  genkey         Generate a master-key passphrase for OPAQ_MASTER_KEY

Auth:
  login          Save server URL and API key
  status         Check auth status and key validity

Secrets:
  set            Set a secret value
  get            Get a secret value
  list           List secrets under a project or env
  rm             Remove a secret
  env            Export secrets as KEY=VALUE lines (dotenv compatible)

Admin:
  principal      Manage API key principals (admin only)

Cheatsheet:
  help           Print a cheatsheet of common commands

Options:
{options}

{after-help}";

#[derive(Parser)]
#[command(
    name = "opaq",
    version,
    propagate_version = true,
    about = "Config/secret store CLI",
    long_about = TOP_LONG_ABOUT,
    after_help = TOP_AFTER_HELP,
    help_template = TOP_HELP_TEMPLATE,
    disable_help_subcommand = true,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a random master-key passphrase suitable for OPAQ_MASTER_KEY
    #[command(after_help = "\
Examples:
  opaq genkey
  opaq genkey --length 48
  export OPAQ_MASTER_KEY=$(opaq genkey)
")]
    Genkey {
        /// Passphrase length in characters (minimum 32)
        #[arg(long, default_value_t = 64)]
        length: usize,
    },
    /// Save server URL and API key
    #[command(after_help = "\
Examples:
  opaq login --server https://opaq.example.com --key opaq_abc123
")]
    Login {
        /// Server base URL (e.g. https://opaq.example.com)
        #[arg(long)]
        server: String,
        /// API key issued by an admin
        #[arg(long)]
        key: String,
    },
    /// Check auth status: validate stored API key against the server
    #[command(after_help = "\
Examples:
  opaq status
")]
    Status,
    /// Set a secret value
    #[command(after_help = "\
Examples:
  opaq set /acme/api/prod/STRIPE_KEY --string sk_live_xxx
  opaq set /acme/api/CONFIG --json '{\"timeout\": 30}'
  opaq set /acme/api/prod/CERT --string-path ./cert.pem
")]
    Set {
        /// Path: /workspace/project[/env]/key
        path: String,
        /// Inline string value
        #[arg(long, group = "value")]
        string: Option<String>,
        /// Path to a file whose contents become a string value
        #[arg(long = "string-path", group = "value")]
        string_path: Option<String>,
        /// Inline JSON literal (validated)
        #[arg(long, group = "value")]
        json: Option<String>,
        /// Path to a .json file (contents validated)
        #[arg(long = "json-path", group = "value")]
        json_path: Option<String>,
    },
    /// Get a secret value
    #[command(after_help = "\
Examples:
  opaq get /acme/api/prod/STRIPE_KEY
  opaq get /acme/api/prod/STRIPE_KEY --raw | pbcopy
")]
    Get {
        path: String,
        /// Print raw value only (no metadata, no JSON pretty-printing) for piping
        #[arg(long)]
        raw: bool,
    },
    /// List secrets under a project or env
    #[command(after_help = "\
By default an env-scoped list also includes the project-scoped (3-segment)
secrets that act as defaults. Use --no-merge to see only env-scoped rows.

Examples:
  opaq list /acme/api                  # project + all envs
  opaq list /acme/api/prod --values    # project defaults + prod (merged)
  opaq list /acme/api/prod --no-merge  # only env-scoped rows
")]
    List {
        path: String,
        /// Also fetch and display each secret value
        #[arg(long)]
        values: bool,
        /// Skip merging project-scoped defaults into env-scoped listings
        #[arg(long = "no-merge")]
        no_merge: bool,
    },
    /// Remove a secret
    #[command(after_help = "\
Examples:
  opaq rm /acme/api/prod/STRIPE_KEY
")]
    Rm { path: String },
    /// Export secrets for a project+env as KEY=VALUE lines (dotenv / --env-file compatible)
    #[command(after_help = "\
Examples:
  opaq env /acme/api/prod                   # KEY=val (for --env-file)
  eval \"$(opaq env /acme/api/prod --shell)\"
  cargo run --env-file <(opaq env /acme/api/dev)
")]
    Env {
        /// Path: /workspace/project/env (env-scoped); also includes project-scoped secrets
        path: String,
        /// Emit `export KEY='value'` lines (for `eval "$(opaq env ...)"`)
        #[arg(long)]
        shell: bool,
        /// Preserve original key case (default uppercases keys for env-var convention)
        #[arg(long = "preserve-case")]
        preserve_case: bool,
    },
    /// Manage API key principals (admin only)
    Principal {
        #[command(subcommand)]
        cmd: PrincipalCmd,
    },
    /// Print a cheatsheet of common commands. Optional topic filters output.
    #[command(after_help = "\
Topics:
  auth       login command
  secrets    set, get, list, rm, env
  admin      principal
  paths      path-syntax reference
  examples   common invocations
")]
    Help {
        /// Topic: auth | secrets | admin | paths | examples
        topic: Option<String>,
    },
}

#[derive(Subcommand)]
enum PrincipalCmd {
    /// Create or update a principal by name (does not rotate keys on update)
    #[command(after_help = "\
If NAME does not match any active principal, a new principal is created and an
API key is minted (printed once). If NAME matches an active principal, the
role / ttl / name are updated in place. The API key is NEVER rotated by this
command — use `principal rotate --name <NAME>` (or `--id <ID>`) instead.

Examples:
  opaq principal set ci-bot --role writer
  opaq principal set ci-bot --ttl 30d
  opaq principal set ci-bot --no-ttl
  opaq principal set ci-bot --rename ci-runner
")]
    Set {
        /// Principal name (lookup key for existing, label for new)
        name: String,
        /// Role: reader, writer, or admin
        #[arg(long)]
        role: Option<String>,
        /// Set TTL: 30d, 12h, 60m, 3600s, or bare seconds
        #[arg(long, conflicts_with = "no_ttl")]
        ttl: Option<String>,
        /// Clear TTL (no expiry) on an existing principal
        #[arg(long = "no-ttl")]
        no_ttl: bool,
        /// Rename an existing principal (errors if NAME doesn't exist)
        #[arg(long)]
        rename: Option<String>,
    },
    /// Rotate the API key for an existing principal (id, name, role, ttl preserved)
    #[command(after_help = "\
The principal's id, name, role, and ttl are preserved. The old API key is
invalidated; the new plaintext is printed once.

Specify the principal by --id or --name. If both are given, --id wins.

Examples:
  opaq principal rotate --name ci-bot
  opaq principal rotate --id 4
")]
    #[command(group(clap::ArgGroup::new("rotate_target").required(true).multiple(true).args(["id", "name"])))]
    Rotate {
        /// Numeric principal ID (from `principal list`)
        #[arg(long)]
        id: Option<i64>,
        /// Principal name
        #[arg(long)]
        name: Option<String>,
    },
    /// Revoke a principal's API key
    #[command(after_help = "\
Specify the principal by --id or --name. If both are given, --id wins.

Examples:
  opaq principal revoke --id 4
  opaq principal revoke --name ci-bot
")]
    #[command(group(clap::ArgGroup::new("revoke_target").required(true).multiple(true).args(["id", "name"])))]
    Revoke {
        /// Numeric principal ID (from `principal list`)
        #[arg(long)]
        id: Option<i64>,
        /// Principal name
        #[arg(long)]
        name: Option<String>,
    },
    /// List principals
    List,
}

#[derive(Serialize, Deserialize)]
struct CliConfig {
    server: String,
    api_key: String,
}

type CliResult<T> = Result<T, String>;

fn config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(".config");
        }
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    config_dir().join("opaq").join("config.json")
}

fn load_config() -> CliResult<CliConfig> {
    let path = config_path();
    let data = std::fs::read_to_string(&path)
        .map_err(|_| "not logged in. Run: opaq login --server URL --key KEY".to_string())?;
    serde_json::from_str(&data).map_err(|e| format!("invalid config: {}", e))
}

fn save_config(config: &CliConfig) -> CliResult<()> {
    let path = config_path();
    let parent = path
        .parent()
        .ok_or_else(|| "could not determine config directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("failed to create config dir: {}", e))?;
    let body = serde_json::to_string_pretty(config)
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

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client build should not fail")
}

fn api_url(config: &CliConfig, path: &str) -> String {
    format!("{}/api/v1/{}", config.server.trim_end_matches('/'), path)
}

/// Parse a secret path: /workspace/project/key (project-scoped) or /workspace/project/env/key (env-scoped).
/// Returns (ws, proj, env_opt, key).
fn parse_secret_path(path: &str) -> CliResult<(&str, &str, Option<&str>, &str)> {
    let trimmed = path.trim_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.iter().any(|s| s.is_empty()) {
        return Err("path segments must not be empty (check for stray '/')".into());
    }
    match parts.len() {
        3 => Ok((parts[0], parts[1], None, parts[2])),
        4 => Ok((parts[0], parts[1], Some(parts[2]), parts[3])),
        _ => Err("path must be /workspace/project/key or /workspace/project/env/key".into()),
    }
}

fn secret_url_path(ws: &str, proj: &str, env: Option<&str>, key: &str) -> String {
    match env {
        Some(e) => format!("secrets/{}/{}/{}/{}", ws, proj, e, key),
        None => format!("secrets/{}/{}/{}", ws, proj, key),
    }
}

fn parse_path3(path: &str) -> CliResult<(&str, &str, Option<&str>)> {
    let trimmed = path.trim_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.iter().any(|s| s.is_empty()) {
        return Err("path segments must not be empty (check for stray '/')".into());
    }
    match parts.len() {
        2 => Ok((parts[0], parts[1], None)),
        3 => Ok((parts[0], parts[1], Some(parts[2]))),
        _ => Err("path must have 2 or 3 segments: /workspace/project[/env]".into()),
    }
}

fn print_err(msg: impl AsRef<str>) {
    eprintln!("error: {}", msg.as_ref());
}

fn print_get(path: &str, vtype: &str, value: &str) {
    println!("{} {}", dim("path:"), bold(path));
    println!("{} {}", dim("type:"), vtype);
    println!();
    if vtype == "json" {
        match serde_json::from_str::<serde_json::Value>(value) {
            Ok(v) => match serde_json::to_string_pretty(&v) {
                Ok(pp) => println!("{}", pp),
                Err(_) => println!("{}", value),
            },
            Err(_) => println!("{}", value),
        }
    } else {
        println!("{}", value);
    }
}

fn print_identity(principal: &serde_json::Value) {
    let name = principal
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let id = principal.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let role = principal
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    println!(
        "{} {} {}",
        dim("identity:"),
        bold(name),
        dim(&format!("(id={}, role={})", id, role))
    );
}

fn derive_envs(items: &[serde_json::Value]) -> Vec<String> {
    let mut envs: Vec<String> = items
        .iter()
        .filter_map(|it| {
            let p = it.get("path").and_then(|v| v.as_str())?;
            let parts: Vec<&str> = p.trim_start_matches('/').split('/').collect();
            if parts.len() == 4 {
                Some(parts[2].to_string())
            } else {
                None
            }
        })
        .collect();
    envs.sort();
    envs.dedup();
    envs
}

fn split_secret_path(path: &str) -> (Option<&str>, Option<&str>, Option<&str>, &str) {
    let trimmed = path.trim_start_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    match parts.len() {
        4 => (Some(parts[0]), Some(parts[1]), Some(parts[2]), parts[3]),
        3 => (Some(parts[0]), Some(parts[1]), None, parts[2]),
        _ => (None, None, None, path),
    }
}

const VALUE_DISPLAY_MAX: usize = 60;

fn render_value_inline(vtype: &str, value: &str) -> String {
    let raw = if vtype == "json" {
        match serde_json::from_str::<serde_json::Value>(value) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|_| value.to_string()),
            Err(_) => value.to_string(),
        }
    } else {
        value.replace('\n', " \u{21B5} ")
    };
    if raw.chars().count() > VALUE_DISPLAY_MAX {
        let truncated: String = raw.chars().take(VALUE_DISPLAY_MAX - 1).collect();
        format!("{}…", truncated)
    } else {
        raw
    }
}

fn env_label(env: &Option<String>) -> &str {
    match env {
        Some(e) => e.as_str(),
        None => "(project)",
    }
}

/// Theme-agnostic palette (Solarized accents). These mid-tone hues
/// are designed to have comparable contrast on both light and dark
/// terminal backgrounds, unlike basic ANSI 16 which can wash out.
fn env_color(env: &Option<String>) -> Color {
    // project-scoped: neutral mid-grey readable on both themes
    const PROJECT_COLOR: Color = Color::Rgb {
        r: 131,
        g: 148,
        b: 150,
    }; // sol base0
    const PALETTE: [Color; 7] = [
        Color::Rgb {
            r: 38,
            g: 139,
            b: 210,
        }, // blue
        Color::Rgb {
            r: 42,
            g: 161,
            b: 152,
        }, // cyan
        Color::Rgb {
            r: 133,
            g: 153,
            b: 0,
        }, // green
        Color::Rgb {
            r: 181,
            g: 137,
            b: 0,
        }, // yellow
        Color::Rgb {
            r: 203,
            g: 75,
            b: 22,
        }, // orange
        Color::Rgb {
            r: 211,
            g: 54,
            b: 130,
        }, // magenta
        Color::Rgb {
            r: 108,
            g: 113,
            b: 196,
        }, // violet
    ];
    match env {
        None => PROJECT_COLOR,
        Some(e) => {
            let mut h: u64 = 0;
            for b in e.bytes() {
                h = h.wrapping_mul(131).wrapping_add(b as u64);
            }
            PALETTE[(h as usize) % PALETTE.len()]
        }
    }
}

#[allow(clippy::type_complexity)]
fn collect_rows(
    items: &[serde_json::Value],
) -> CliResult<(
    Option<String>,
    Option<String>,
    Vec<(Option<String>, String, String, Option<String>)>,
)> {
    let mut header_ws: Option<String> = None;
    let mut header_proj: Option<String> = None;
    let mut rows: Vec<(Option<String>, String, String, Option<String>)> =
        Vec::with_capacity(items.len());

    for it in items {
        let path = json_str(it, "path")?;
        let vtype = json_str(it, "type")?;
        let value = it.get("value").and_then(|v| v.as_str()).map(str::to_string);
        let (ws, proj, env, key) = split_secret_path(path);
        if header_ws.is_none() {
            header_ws = ws.map(str::to_string);
            header_proj = proj.map(str::to_string);
        }
        rows.push((
            env.map(str::to_string),
            key.to_string(),
            vtype.to_string(),
            value,
        ));
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok((header_ws, header_proj, rows))
}

fn rgb_ansi(c: Color) -> String {
    if let Color::Rgb { r, g, b } = c {
        format!("38;2;{};{};{}", r, g, b)
    } else {
        "0".to_string()
    }
}

fn print_header(ws: &Option<String>, proj: &Option<String>, envs: &[String]) {
    if let (Some(w), Some(p)) = (ws, proj) {
        let mut line = format!(
            "{} {} {}{} {} {} {}",
            ansi("2", "workspace"),
            ansi("2", "="),
            ansi("1;36", w),
            ansi("2", ","),
            ansi("2", "project"),
            ansi("2", "="),
            ansi("1;35", p),
        );
        if !envs.is_empty() {
            let colored_envs: Vec<String> = envs
                .iter()
                .map(|e| {
                    let code = rgb_ansi(env_color(&Some(e.clone())));
                    ansi(&code, e)
                })
                .collect();
            if colored_envs.len() == 1 {
                line.push_str(&format!(
                    "{} {} {} {}",
                    ansi("2", ","),
                    ansi("2", "env"),
                    ansi("2", "="),
                    colored_envs[0],
                ));
            } else {
                let body = colored_envs.join(&ansi("2", ", "));
                line.push_str(&format!(
                    "{} {} {}{}{}",
                    ansi("2", ","),
                    ansi("2", "envs"),
                    ansi("2", " { "),
                    body,
                    ansi("2", " }"),
                ));
            }
        }
        println!("{}", line);
    }
}

fn print_list_with_values(items: &[serde_json::Value], envs: &[String]) -> CliResult<()> {
    if items.is_empty() {
        eprintln!("{}", dim("(no secrets)"));
        return Ok(());
    }
    let (ws, proj, rows) = collect_rows(items)?;
    print_header(&ws, &proj, envs);

    let mut table = make_table();
    table.set_header(vec![
        Cell::new("ENV"),
        Cell::new("KEY"),
        Cell::new("TYPE"),
        Cell::new("VALUE"),
    ]);

    for (env, key, vtype, value) in &rows {
        let val_str = match value {
            Some(v) => render_value_inline(vtype, v),
            None => "<no value>".to_string(),
        };
        table.add_row(vec![
            Cell::new(env_label(env)).fg(env_color(env)),
            Cell::new(key),
            Cell::new(vtype),
            Cell::new(val_str),
        ]);
    }
    println!("{}", table);
    Ok(())
}

fn print_list(items: &[serde_json::Value], envs: &[String]) -> CliResult<()> {
    if items.is_empty() {
        let (ws, proj, _) = collect_rows(items).unwrap_or((None, None, vec![]));
        print_header(&ws, &proj, envs);
        eprintln!("{}", dim("(no secrets)"));
        return Ok(());
    }
    let (ws, proj, rows) = collect_rows(items)?;
    print_header(&ws, &proj, envs);

    let mut table = make_table();
    table.set_header(vec![Cell::new("ENV"), Cell::new("KEY"), Cell::new("TYPE")]);
    for (env, key, vtype, _) in &rows {
        table.add_row(vec![
            Cell::new(env_label(env)).fg(env_color(env)),
            Cell::new(key),
            Cell::new(vtype),
        ]);
    }
    println!("{}", table);
    Ok(())
}

fn render_resp_error(resp: reqwest::blocking::Response) -> String {
    let status = resp.status();
    match resp.text() {
        Ok(body) => match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(json) => json
                .get("error")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| body.clone()),
            Err(_) => {
                let trimmed = body.trim();
                if trimmed.is_empty() || trimmed.starts_with('<') {
                    format!("server returned {}", status.as_u16())
                } else {
                    trimmed.to_string()
                }
            }
        },
        Err(e) => format!("failed to read response body: {}", e),
    }
}

fn json_str<'a>(v: &'a serde_json::Value, key: &str) -> CliResult<&'a str> {
    v.get(key)
        .and_then(|x| x.as_str())
        .ok_or_else(|| format!("response missing string field '{}'", key))
}

fn parse_json(resp: reqwest::blocking::Response) -> CliResult<serde_json::Value> {
    resp.json::<serde_json::Value>()
        .map_err(|e| format!("failed to parse server response: {}", e))
}

fn fetch_principals(config: &CliConfig) -> CliResult<Vec<serde_json::Value>> {
    let url = api_url(config, "principals");
    let resp = client()
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .send()
        .map_err(|e| format!("request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(render_resp_error(resp));
    }
    let body = parse_json(resp)?;
    body.as_array()
        .cloned()
        .ok_or_else(|| "expected JSON array".to_string())
}

fn lookup_principal_name_by_id(config: &CliConfig, id: i64) -> CliResult<String> {
    for p in fetch_principals(config)? {
        if p.get("id").and_then(|v| v.as_i64()) == Some(id) {
            return p
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .ok_or_else(|| format!("principal {} has no name field", id));
        }
    }
    Err(format!("principal id {} not found", id))
}

fn lookup_principal_id_by_name(config: &CliConfig, name: &str) -> CliResult<i64> {
    for p in fetch_principals(config)? {
        if p.get("name").and_then(|v| v.as_str()) == Some(name) {
            return p
                .get("id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| format!("principal '{}' has no id field", name));
        }
    }
    Err(format!("principal name '{}' not found", name))
}

/// Shell single-quote a string. Internal `'` becomes `'\''`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn parse_ttl(raw: &str) -> CliResult<u64> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("ttl must not be empty".into());
    }
    let last = s.as_bytes()[s.len() - 1];
    let (num_part, mult): (&str, u64) = match last {
        b's' => (&s[..s.len() - 1], 1),
        b'm' => (&s[..s.len() - 1], 60),
        b'h' => (&s[..s.len() - 1], 3600),
        b'd' => (&s[..s.len() - 1], 86_400),
        b'0'..=b'9' => (s, 1),
        _ => {
            return Err(format!(
                "invalid ttl '{}': use a suffix like 30d, 12h, 60m, or 3600s",
                raw
            ))
        }
    };
    let n: u64 = num_part
        .parse()
        .map_err(|_| format!("invalid ttl number in '{}'", raw))?;
    if n == 0 {
        return Err("ttl must be greater than 0".into());
    }
    n.checked_mul(mult)
        .ok_or_else(|| format!("ttl '{}' overflows u64 seconds", raw))
}

fn format_unix_secs(s: &str) -> String {
    if s.parse::<u64>().is_ok() {
        format!("@{}", s)
    } else {
        s.to_string()
    }
}

fn run() -> CliResult<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Genkey { length } => {
            if length < 32 {
                return Err("length must be at least 32".into());
            }
            use rand::distributions::Alphanumeric;
            use rand::Rng;
            let key: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(length)
                .map(char::from)
                .collect();
            println!("{}", key);
            Ok(())
        }
        Command::Login { server, key } => {
            let config = CliConfig {
                server,
                api_key: key,
            };
            save_config(&config)?;
            println!("Logged in.");
            Ok(())
        }
        Command::Status => {
            let config = load_config()?;
            let url = api_url(&config, "me");
            let resp = client()
                .get(&url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .send()
                .map_err(|e| format!("server unreachable: {} (server: {})", e, config.server))?;

            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(format!(
                    "API key invalid or revoked (server: {}). Mint a new key (admin) or run `opaq login` again.",
                    config.server
                ));
            }
            if !resp.status().is_success() {
                return Err(render_resp_error(resp));
            }

            let body = parse_json(resp)?;
            let principal = body.get("principal").cloned().unwrap_or_default();
            let key_tail = if config.api_key.len() >= 4 {
                &config.api_key[config.api_key.len() - 4..]
            } else {
                config.api_key.as_str()
            };

            println!("{} {}", dim("status:"), bold("ok"));
            println!("{} {}", dim("server:"), config.server);
            print_identity(&principal);
            println!("{} ...{}", dim("key tail:"), key_tail);
            Ok(())
        }
        Command::Set {
            path,
            string,
            string_path,
            json,
            json_path,
        } => {
            let config = load_config()?;
            let (ws, proj, env, key) = parse_secret_path(&path)?;

            let (val, vtype) = match (string, string_path, json, json_path) {
                (Some(s), None, None, None) => (s, "string"),
                (None, Some(p), None, None) => {
                    // Files commonly end with a trailing newline from editors;
                    // strip it so `get --raw` round-trips the visible content.
                    let raw = std::fs::read_to_string(&p)
                        .map_err(|e| format!("--string-path: failed to read {}: {}", p, e))?;
                    (raw.trim_end_matches(['\r', '\n']).to_string(), "string")
                }
                (None, None, Some(j), None) => {
                    serde_json::from_str::<serde_json::Value>(&j)
                        .map_err(|e| format!("--json: invalid JSON: {}", e))?;
                    (j, "json")
                }
                (None, None, None, Some(p)) => {
                    let raw = std::fs::read_to_string(&p)
                        .map_err(|e| format!("--json-path: failed to read {}: {}", p, e))?;
                    let body = raw.trim_end_matches(['\r', '\n']).to_string();
                    serde_json::from_str::<serde_json::Value>(&body).map_err(|e| {
                        format!("--json-path: file {} contains invalid JSON: {}", p, e)
                    })?;
                    (body, "json")
                }
                (None, None, None, None) => {
                    return Err(
                        "exactly one of --string, --string-path, --json, --json-path is required"
                            .into(),
                    )
                }
                _ => {
                    return Err(
                        "--string/--string-path/--json/--json-path are mutually exclusive".into(),
                    )
                }
            };

            let url = api_url(&config, &secret_url_path(ws, proj, env, key));
            let resp = client()
                .put(&url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .json(&serde_json::json!({ "value": val, "type": vtype }))
                .send()
                .map_err(|e| format!("request failed: {}", e))?;

            if resp.status().is_success() {
                println!("OK");
                Ok(())
            } else {
                Err(render_resp_error(resp))
            }
        }
        Command::Get { path, raw } => {
            let config = load_config()?;
            let (ws, proj, env, key) = parse_secret_path(&path)?;
            let url = api_url(&config, &secret_url_path(ws, proj, env, key));
            let resp = client()
                .get(&url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .send()
                .map_err(|e| format!("request failed: {}", e))?;

            if resp.status().is_success() {
                let body = parse_json(resp)?;
                let value = json_str(&body, "value")?;
                if raw {
                    print!("{}", value);
                    return Ok(());
                }
                let p = json_str(&body, "path")?;
                let vtype = json_str(&body, "type")?;
                print_get(p, vtype, value);
                Ok(())
            } else {
                Err(render_resp_error(resp))
            }
        }
        Command::List {
            path,
            values,
            no_merge,
        } => {
            let config = load_config()?;
            let (ws, proj, env_opt) = parse_path3(&path)?;
            let mut url = match &env_opt {
                Some(env) => api_url(&config, &format!("list/{}/{}/{}", ws, proj, env)),
                None => api_url(&config, &format!("list/{}/{}", ws, proj)),
            };
            let mut params: Vec<&str> = Vec::new();
            if values {
                params.push("values=true");
            }
            if no_merge && env_opt.is_some() {
                params.push("merge=false");
            }
            if !params.is_empty() {
                url.push('?');
                url.push_str(&params.join("&"));
            }
            let resp = client()
                .get(&url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .send()
                .map_err(|e| format!("request failed: {}", e))?;

            if !resp.status().is_success() {
                return Err(render_resp_error(resp));
            }
            let body = parse_json(resp)?;
            let arr = body
                .as_array()
                .ok_or_else(|| "expected JSON array from server".to_string())?
                .clone();
            let envs = derive_envs(&arr);

            if values {
                print_list_with_values(&arr, &envs)
            } else {
                print_list(&arr, &envs)
            }
        }
        Command::Env {
            path,
            shell,
            preserve_case,
        } => {
            let config = load_config()?;
            let trimmed = path.trim_matches('/');
            let parts: Vec<&str> = trimmed.split('/').collect();
            if parts.len() != 3 || parts.iter().any(|s| s.is_empty()) {
                return Err("env requires /workspace/project/env".into());
            }
            let (ws, proj, env) = (parts[0], parts[1], parts[2]);

            // single round trip: project-scoped + every env-scoped, with values
            let url = api_url(&config, &format!("list/{}/{}?values=true", ws, proj));
            let resp = client()
                .get(&url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .send()
                .map_err(|e| format!("request failed: {}", e))?;
            if !resp.status().is_success() {
                return Err(render_resp_error(resp));
            }
            let arr = parse_json(resp)?
                .as_array()
                .ok_or_else(|| "expected JSON array".to_string())?
                .clone();

            // Filter to (project-scoped) + (matching env-scoped). When both
            // scopes define the same output key, env-scoped wins.
            #[derive(Clone, Copy, PartialEq, Eq)]
            enum Scope {
                Project,
                Env,
            }

            let mut merged: std::collections::BTreeMap<String, (Scope, String)> =
                std::collections::BTreeMap::new();
            for item in &arr {
                let p = json_str(item, "path")?;
                let segs: Vec<&str> = p.trim_start_matches('/').split('/').collect();
                let (raw_key, scope) = match segs.len() {
                    3 => (segs[2].to_string(), Scope::Project),
                    4 if segs[2] == env => (segs[3].to_string(), Scope::Env),
                    _ => continue,
                };
                let val = json_str(item, "value")?.to_string();

                let out_key = if preserve_case {
                    raw_key
                } else {
                    raw_key.to_uppercase()
                };

                use std::collections::btree_map::Entry;
                match merged.entry(out_key) {
                    Entry::Vacant(e) => {
                        e.insert((scope, val));
                    }
                    Entry::Occupied(mut e) => match (e.get().0, scope) {
                        (Scope::Project, Scope::Env) => {
                            e.insert((scope, val));
                        }
                        (Scope::Env, Scope::Project) => { /* keep env-scoped */ }
                        _ => {
                            e.insert((scope, val));
                        }
                    },
                }
            }

            for (k, (_, v)) in &merged {
                if shell {
                    println!("export {}={}", k, shell_quote(v));
                } else if v.contains('\n') {
                    eprintln!("warn: value for {} contains newlines, skipping (incompatible with --env-file)", k);
                } else {
                    println!("{}={}", k, v);
                }
            }
            Ok(())
        }
        Command::Rm { path } => {
            let config = load_config()?;
            let (ws, proj, env, key) = parse_secret_path(&path)?;
            let url = api_url(&config, &secret_url_path(ws, proj, env, key));
            let resp = client()
                .delete(&url)
                .header("Authorization", format!("Bearer {}", config.api_key))
                .send()
                .map_err(|e| format!("request failed: {}", e))?;

            if resp.status().is_success() {
                println!("OK");
                Ok(())
            } else {
                Err(render_resp_error(resp))
            }
        }
        Command::Principal { cmd } => match cmd {
            PrincipalCmd::Set {
                name,
                role,
                ttl,
                no_ttl,
                rename,
            } => {
                let config = load_config()?;
                let ttl_seconds = ttl.as_deref().map(parse_ttl).transpose()?;
                let mut payload = serde_json::json!({ "name": name });
                if let Some(r) = &role {
                    payload["role"] = serde_json::Value::from(r.clone());
                }
                if let Some(secs) = ttl_seconds {
                    payload["ttl_seconds"] = serde_json::Value::from(secs);
                }
                if no_ttl {
                    payload["clear_ttl"] = serde_json::Value::from(true);
                }
                if let Some(rn) = &rename {
                    payload["rename"] = serde_json::Value::from(rn.clone());
                }

                let url = api_url(&config, "principals");
                let resp = client()
                    .put(&url)
                    .header("Authorization", format!("Bearer {}", config.api_key))
                    .json(&payload)
                    .send()
                    .map_err(|e| format!("request failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(render_resp_error(resp));
                }
                let body = parse_json(resp)?;
                let action = json_str(&body, "action")?.to_string();
                let id = body.get("id").map(|v| v.to_string()).unwrap_or_default();
                let nm = json_str(&body, "name")?.to_string();
                let rl = json_str(&body, "role")?.to_string();
                let expires_at = body
                    .get("expires_at")
                    .and_then(|v| v.as_str())
                    .map(format_unix_secs)
                    .unwrap_or_else(|| "(never)".to_string());

                let mut table = make_table();
                table.set_header(vec![Cell::new("FIELD"), Cell::new("VALUE")]);
                table.add_row(vec![Cell::new("Action"), Cell::new(&action)]);
                table.add_row(vec![Cell::new("ID"), Cell::new(&id)]);
                table.add_row(vec![Cell::new("Name"), Cell::new(&nm)]);
                table.add_row(vec![Cell::new("Role"), Cell::new(&rl)]);
                table.add_row(vec![Cell::new("Expires"), Cell::new(&expires_at)]);
                if let Some(key) = body.get("key").and_then(|v| v.as_str()) {
                    table.add_row(vec![Cell::new("Key"), Cell::new(key)]);
                }
                println!("{}", table);
                if action == "created" {
                    println!();
                    println!("{}", bold("SAVE THIS KEY. It will not be shown again."));
                }
                Ok(())
            }
            PrincipalCmd::List => {
                let config = load_config()?;
                let url = api_url(&config, "principals");
                let resp = client()
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", config.api_key))
                    .send()
                    .map_err(|e| format!("request failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(render_resp_error(resp));
                }
                let body = parse_json(resp)?;
                let arr = body
                    .as_array()
                    .ok_or_else(|| "expected JSON array".to_string())?;
                let mut table = make_table();
                table.set_header(vec![
                    Cell::new("ID"),
                    Cell::new("NAME"),
                    Cell::new("ROLE"),
                    Cell::new("EXPIRES"),
                    Cell::new("REVOKED"),
                ]);
                for p in arr {
                    let expires = p
                        .get("expires_at")
                        .and_then(|v| v.as_str())
                        .map(format_unix_secs)
                        .unwrap_or_default();
                    table.add_row(vec![
                        Cell::new(p.get("id").map(|v| v.to_string()).unwrap_or_default()),
                        Cell::new(p.get("name").and_then(|v| v.as_str()).unwrap_or("?")),
                        Cell::new(p.get("role").and_then(|v| v.as_str()).unwrap_or("?")),
                        Cell::new(expires),
                        Cell::new(p.get("revoked_at").and_then(|v| v.as_str()).unwrap_or("")),
                    ]);
                }
                println!("{}", table);
                Ok(())
            }
            PrincipalCmd::Rotate { id, name } => {
                let config = load_config()?;
                let resolved_name = match (id, name) {
                    (Some(id), _) => lookup_principal_name_by_id(&config, id)?,
                    (None, Some(name)) => name,
                    (None, None) => unreachable!("clap ArgGroup requires id or name"),
                };
                let url = api_url(&config, "principals/rotate");
                let resp = client()
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", config.api_key))
                    .json(&serde_json::json!({ "name": resolved_name }))
                    .send()
                    .map_err(|e| format!("request failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(render_resp_error(resp));
                }
                let body = parse_json(resp)?;
                let id = body.get("id").map(|v| v.to_string()).unwrap_or_default();
                let nm = json_str(&body, "name")?.to_string();
                let rl = json_str(&body, "role")?.to_string();
                let key = json_str(&body, "key")?.to_string();
                let expires_at = body
                    .get("expires_at")
                    .and_then(|v| v.as_str())
                    .map(format_unix_secs)
                    .unwrap_or_else(|| "(never)".to_string());

                let mut table = make_table();
                table.set_header(vec![Cell::new("FIELD"), Cell::new("VALUE")]);
                table.add_row(vec![Cell::new("ID"), Cell::new(&id)]);
                table.add_row(vec![Cell::new("Name"), Cell::new(&nm)]);
                table.add_row(vec![Cell::new("Role"), Cell::new(&rl)]);
                table.add_row(vec![Cell::new("Expires"), Cell::new(&expires_at)]);
                table.add_row(vec![Cell::new("Key"), Cell::new(&key)]);
                println!("{}", table);
                println!();
                println!("{}", bold("SAVE THIS KEY. The old key is now invalid."));
                Ok(())
            }
            PrincipalCmd::Revoke { id, name } => {
                let config = load_config()?;
                let resolved_id = match (id, name) {
                    (Some(id), _) => id,
                    (None, Some(name)) => lookup_principal_id_by_name(&config, &name)?,
                    (None, None) => unreachable!("clap ArgGroup requires id or name"),
                };
                let url = api_url(&config, &format!("principals/{}", resolved_id));
                let resp = client()
                    .delete(&url)
                    .header("Authorization", format!("Bearer {}", config.api_key))
                    .send()
                    .map_err(|e| format!("request failed: {}", e))?;

                if resp.status().is_success() {
                    println!("OK");
                    Ok(())
                } else {
                    Err(render_resp_error(resp))
                }
            }
        },
        Command::Help { topic } => help::print_cheatsheet(topic.as_deref()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            print_err(e);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod parse_ttl_tests {
    use super::parse_ttl;

    #[test]
    fn parses_seconds_suffix() {
        assert_eq!(parse_ttl("30s").unwrap(), 30);
    }

    #[test]
    fn parses_minutes_suffix() {
        assert_eq!(parse_ttl("5m").unwrap(), 300);
    }

    #[test]
    fn parses_hours_suffix() {
        assert_eq!(parse_ttl("2h").unwrap(), 7200);
    }

    #[test]
    fn parses_days_suffix() {
        assert_eq!(parse_ttl("30d").unwrap(), 2_592_000);
    }

    #[test]
    fn parses_bare_number_as_seconds() {
        assert_eq!(parse_ttl("60").unwrap(), 60);
    }

    #[test]
    fn rejects_zero() {
        assert!(parse_ttl("0d").is_err());
    }

    #[test]
    fn rejects_unknown_suffix() {
        assert!(parse_ttl("3w").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_ttl("abc").is_err());
        assert!(parse_ttl("").is_err());
    }
}
