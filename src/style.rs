use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

pub fn use_color() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

pub fn ansi(code: &str, s: &str) -> String {
    if use_color() {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

pub fn dim(s: &str) -> String {
    if use_color() {
        format!("\x1b[2m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    if use_color() {
        format!("\x1b[1m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

pub fn make_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}
