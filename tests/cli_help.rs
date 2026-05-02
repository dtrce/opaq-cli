use std::process::Command;

fn opaq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_opaq"))
}

fn help_text(args: &[&str]) -> String {
    let out = opaq().args(args).output().expect("run opaq");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn version_flag_prints_version() {
    let out = opaq()
        .arg("--version")
        .output()
        .expect("run opaq --version");
    assert!(out.status.success(), "exit status: {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("opaq"), "stdout: {}", stdout);
    assert!(stdout.contains("0.1.0"), "stdout: {}", stdout);
}

#[test]
fn top_help_shows_command_groups() {
    let out = opaq().arg("--help").output().expect("run opaq --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Auth"), "stdout: {}", stdout);
    assert!(stdout.contains("Secrets"), "stdout: {}", stdout);
    assert!(stdout.contains("Admin"), "stdout: {}", stdout);
}

#[test]
fn top_help_mentions_cheatsheet() {
    let out = opaq().arg("--help").output().expect("run opaq --help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("opaq help"),
        "expected pointer to `opaq help` in footer; stdout: {}",
        stdout
    );
}

#[test]
fn login_help_describes_flags() {
    let h = help_text(&["login", "--help"]);
    assert!(h.contains("Server base URL"), "{}", h);
    assert!(h.contains("API key"), "{}", h);
}

#[test]
fn status_help_describes_command() {
    let h = help_text(&["status", "--help"]);
    assert!(h.contains("auth status"), "{}", h);
    assert!(h.contains("opaq status"), "missing example; got: {}", h);
}

#[test]
fn top_help_lists_status_under_auth() {
    let h = help_text(&["--help"]);
    assert!(h.contains("status"), "missing status command; got: {}", h);
}

#[test]
fn help_auth_lists_status() {
    let out = opaq().args(["help", "auth"]).output().expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("opaq status"), "{}", stdout);
}

#[test]
fn principal_set_help_describes_flags() {
    let h = help_text(&["principal", "set", "--help"]);
    assert!(h.contains("name") || h.contains("NAME"), "{}", h);
    assert!(h.contains("role"), "{}", h);
    assert!(h.contains("ttl") || h.contains("TTL"), "{}", h);
    assert!(h.contains("rename"), "{}", h);
}

#[test]
fn principal_rotate_help_describes_command() {
    let h = help_text(&["principal", "rotate", "--help"]);
    assert!(h.contains("rotate") || h.contains("Rotate"), "{}", h);
    assert!(h.contains("name") || h.contains("NAME"), "{}", h);
}

#[test]
fn principal_revoke_help_describes_id() {
    let h = help_text(&["principal", "revoke", "--help"]);
    assert!(h.contains("principal ID"), "{}", h);
}

#[test]
fn set_help_shows_example() {
    let h = help_text(&["set", "--help"]);
    assert!(h.contains("opaq set "), "missing example; got: {}", h);
}

#[test]
fn get_help_shows_example() {
    let h = help_text(&["get", "--help"]);
    assert!(h.contains("opaq get "), "missing example; got: {}", h);
}

#[test]
fn env_help_shows_example() {
    let h = help_text(&["env", "--help"]);
    assert!(h.contains("opaq env "), "missing example; got: {}", h);
}

#[test]
fn help_command_runs_successfully() {
    let out = opaq().arg("help").output().expect("run opaq help");
    assert!(out.status.success(), "exit: {}", out.status);
}

#[test]
fn help_unknown_topic_fails_and_lists_topics() {
    let out = opaq().args(["help", "bogus"]).output().expect("run");
    assert!(!out.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("auth"), "stderr: {}", stderr);
    assert!(stderr.contains("secrets"), "stderr: {}", stderr);
    assert!(stderr.contains("admin"), "stderr: {}", stderr);
}

#[test]
fn help_full_contains_all_sections() {
    let out = opaq().arg("help").output().expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for section in ["PATHS", "AUTH", "SECRETS", "ADMIN", "EXAMPLES"] {
        assert!(
            stdout.contains(section),
            "missing {}; got: {}",
            section,
            stdout
        );
    }
}

#[test]
fn help_admin_only_admin_section() {
    let out = opaq().args(["help", "admin"]).output().expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ADMIN"), "{}", stdout);
    assert!(stdout.contains("principal"), "{}", stdout);
    assert!(
        !stdout.contains("SECRETS"),
        "leaked SECRETS section: {}",
        stdout
    );
}

#[test]
fn help_paths_only_paths_section() {
    let out = opaq().args(["help", "paths"]).output().expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("PATHS"), "{}", stdout);
    assert!(stdout.contains("project-scoped"), "{}", stdout);
    assert!(
        !stdout.contains("ADMIN"),
        "leaked ADMIN section: {}",
        stdout
    );
}

#[test]
fn help_secrets_excludes_admin() {
    let out = opaq().args(["help", "secrets"]).output().expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("SECRETS"), "{}", stdout);
    assert!(!stdout.contains("ADMIN"), "{}", stdout);
}

#[test]
fn help_topic_is_case_insensitive() {
    let out = opaq().args(["help", "ADMIN"]).output().expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ADMIN"), "{}", stdout);
}

#[test]
fn help_no_color_produces_no_escapes() {
    let out = opaq()
        .arg("help")
        .env("NO_COLOR", "1")
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('\x1b'), "ANSI escape leaked: {:?}", stdout);
}
