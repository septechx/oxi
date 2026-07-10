use std::collections::HashMap;
use std::path::{Component, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    message: String,
}

type Diagnostics = HashMap<String, Diagnostic>;

fn main() {
    let mut args = std::env::args();
    args.next();
    let Some(rel_path) = args.next() else {
        eprintln!("Usage: oxic_diag_lint <path>");
        std::process::exit(1);
    };

    let cwd = std::env::current_dir().unwrap();
    let path = cwd.join(rel_path);

    let mut found = vec![];
    let mut error = false;
    search(&mut found, path, &mut error);

    let mut seen = HashMap::new();
    for diagnostics in found {
        for (key, diagnostic) in diagnostics.1 {
            seen.entry(key.clone())
                .and_modify(|previous: &mut PathBuf| {
                    eprintln!("[{}]: Duplicate key: {}", diagnostics.0.display(), key);
                    eprintln!("    Previously defined at: {}", previous.display());
                    error = true;
                })
                .or_insert(diagnostics.0.clone());
            if !validate_key(&key) {
                eprintln!("[{}]: Invalid key: {}", diagnostics.0.display(), key);
                error = true;
            }
            if !validate_level(&diagnostic.level) {
                eprintln!(
                    "[{}]: Invalid level: {}",
                    diagnostics.0.display(),
                    diagnostic.level
                );
                error = true;
            }
            if !validate_message(&diagnostic.message) {
                eprintln!(
                    "[{}]: Invalid message: {}",
                    diagnostics.0.display(),
                    diagnostic.message
                );
                error = true;
            }
        }
    }

    if error {
        std::process::exit(1);
    }
}

fn search(found: &mut Vec<(PathBuf, Diagnostics)>, dir: PathBuf, error: &mut bool) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            search(found, path, error);
        } else if path.file_name().unwrap() == "diagnostics.toml" {
            let contents = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "[{}]: Failed to read diagnostics file: {}",
                        path.display(),
                        e
                    );
                    *error = true;
                    continue;
                }
            };
            let diagnostics: Diagnostics = match toml::from_str(&contents) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[{}]: Failed to parse diagnostics: {}", path.display(), e);
                    *error = true;
                    continue;
                }
            };
            found.push((normalize_path(path), diagnostics));
        }
    }
}

fn validate_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn validate_level(level: &str) -> bool {
    matches!(level, "Warning" | "Error" | "Fatal")
}

fn validate_message(message: &str) -> bool {
    if message.is_empty() {
        return false;
    }

    let mut rest = message;
    let mut args = Vec::new();
    while let Some(start) = rest.find('{') {
        if let Some(end) = rest[start..].find('}') {
            let candidate = &rest[start + 1..start + end];
            args.push(candidate);
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }

    for arg in &args {
        if arg.is_empty() || !validate_key(arg) {
            return false;
        }
    }

    true
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path.components().fold(PathBuf::new(), |mut p, c| {
        match c {
            Component::ParentDir => {
                p.pop();
            }
            Component::CurDir => {}
            _ => p.push(c.as_os_str()),
        }
        p
    })
}
