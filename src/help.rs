use crate::style::{ansi, bold, dim, make_table};
use crate::CliResult;
use comfy_table::Cell;

const VALID_TOPICS: &[&str] = &["setup", "auth", "secrets", "admin", "paths", "examples"];

pub fn print_cheatsheet(topic: Option<&str>) -> CliResult<()> {
    let normalized = topic.map(|t| t.to_ascii_lowercase());
    match normalized.as_deref() {
        None => {
            print_banner();
            section_paths();
            section_setup();
            section_auth();
            section_secrets();
            section_admin();
            section_examples();
        }
        Some("setup") => {
            section_setup();
            println!();
            println!("{}", dim("Run `opaq genkey --help` for options."));
        }
        Some("auth") => {
            section_auth();
            println!();
            println!("{}", dim("Run `opaq login --help` for full options."));
        }
        Some("secrets") => {
            section_secrets();
            println!();
            println!(
                "{}",
                dim("Run `opaq <set|get|list|rm|env> --help` for full options.")
            );
        }
        Some("admin") => {
            section_admin();
            println!();
            println!("{}", dim("Run `opaq principal --help` for full options."));
        }
        Some("paths") => {
            section_paths();
            println!();
            println!(
                "{}",
                dim("3 segments = project-scoped (shared across envs).")
            );
            println!(
                "{}",
                dim("4 segments = env-scoped (overrides project-scoped at fetch time).")
            );
            println!(
                "{}",
                dim("`list` and `env` use scope paths (no trailing /key).")
            );
        }
        Some("examples") => {
            section_examples();
        }
        Some(other) => {
            return Err(format!(
                "unknown help topic '{}'. Valid topics: {}",
                other,
                VALID_TOPICS.join(", ")
            ));
        }
    }
    Ok(())
}

fn print_banner() {
    println!(
        "{} {} config/secret store CLI v{}",
        bold("opaq"),
        dim("—"),
        env!("CARGO_PKG_VERSION"),
    );
    println!();
}

fn header(title: &str) {
    println!("{}", ansi("1;36", title));
}

fn rows_table(rows: &[(&str, &str)]) {
    let mut table = make_table();
    table.set_header(vec![Cell::new("COMMAND"), Cell::new("DESCRIPTION")]);
    for (cmd, desc) in rows {
        table.add_row(vec![Cell::new(cmd), Cell::new(desc)]);
    }
    println!("{}", table);
}

fn section_paths() {
    header("PATHS");
    rows_table(&[
        ("/workspace/project/key", "project-scoped secret"),
        ("/workspace/project/env/key", "env-scoped secret"),
        ("/workspace/project[/env]", "scope path (list / env)"),
    ]);
    println!();
}

fn section_setup() {
    header("SETUP");
    rows_table(&[(
        "opaq genkey [--length N]",
        "generate OPAQ_MASTER_KEY passphrase",
    )]);
    println!();
}

fn section_auth() {
    header("AUTH");
    rows_table(&[
        (
            "opaq login --server URL --key KEY",
            "save creds (~/.config/opaq/config.json)",
        ),
        ("opaq status", "verify stored key against the server"),
    ]);
    println!();
}

fn section_secrets() {
    header("SECRETS");
    rows_table(&[
        ("opaq set <path> --string VAL", "set string secret"),
        (
            "opaq set <path> --string-path FILE",
            "load string from file",
        ),
        (
            "opaq set <path> --json '{\"k\":\"v\"}'",
            "set JSON (validated)",
        ),
        ("opaq set <path> --json-path FILE", "load JSON from file"),
        ("opaq get <path>", "fetch secret (pretty)"),
        ("opaq get <path> --raw", "raw value (pipe-safe)"),
        ("opaq list /ws/proj", "list project + all envs"),
        (
            "opaq list /ws/proj/env --values",
            "list env (merges project defaults)",
        ),
        (
            "opaq list /ws/proj/env --no-merge",
            "list env, env-scoped rows only",
        ),
        ("opaq rm <path>", "delete secret"),
        ("opaq env /ws/proj/env", "KEY=VAL lines (dotenv)"),
        ("opaq env /ws/proj/env --shell", "export KEY='val' lines"),
        (
            "opaq env /ws/proj/env --preserve-case",
            "keep original key case",
        ),
    ]);
    println!();
}

fn section_admin() {
    header("ADMIN");
    rows_table(&[
        (
            "opaq principal set N [--role R] [--ttl T|--no-ttl] [--rename M]",
            "create or update by name (mints key on create)",
        ),
        (
            "opaq principal rotate --id N | --name M",
            "mint a new API key for principal",
        ),
        ("opaq principal list", "list principals"),
        (
            "opaq principal revoke --id N | --name M",
            "disable principal",
        ),
    ]);
    println!();
}

fn section_examples() {
    header("EXAMPLES");
    let lines = [
        "opaq login --server https://opaq.example.com --key opaq_abc123",
        "opaq set /acme/api/prod/STRIPE_KEY --string sk_live_xxx",
        "opaq get /acme/api/prod/STRIPE_KEY --raw | pbcopy",
        "eval \"$(opaq env /acme/api/prod --shell)\"",
        "cargo run --env-file <(opaq env /acme/api/dev)",
    ];
    for l in lines {
        println!("  {}", l);
    }
    println!();
}
