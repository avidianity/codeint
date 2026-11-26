use std::fmt;

use codeint_model::path_file_name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShellDialect {
    PosixSh,
    Bash,
    Zsh,
}

impl fmt::Display for ShellDialect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ShellDialect {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PosixSh => "sh",
            Self::Bash => "bash",
            Self::Zsh => "zsh",
        }
    }
}

pub fn detect_dialect(path: &str, source: &str) -> ShellDialect {
    let trimmed_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    if let Some(first_line) = trimmed_bom.lines().next()
        && let Some(dialect) = parse_shebang_dialect(first_line)
    {
        return dialect;
    }
    dialect_from_extension(path)
}

pub(crate) fn parse_shebang_flags(source: &str) -> Vec<String> {
    let trimmed_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    let Some(first_line) = trimmed_bom.lines().next() else {
        return Vec::new();
    };
    let Some(rest) = first_line.strip_prefix("#!") else {
        return Vec::new();
    };
    let mut parts = rest.trim().split_ascii_whitespace();
    let Some(first) = parts.next() else {
        return Vec::new();
    };
    if !is_launcher(path_file_name(first)) {
        return parts.map(str::to_owned).collect();
    }
    extract_env_shell_flags(parts)
}

fn extract_env_shell_flags<'a>(parts: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut found_shell = false;
    let mut flags = Vec::new();
    for part in parts {
        if found_shell {
            flags.push(part.to_owned());
            continue;
        }
        if part.starts_with('-') || part.contains('=') {
            continue;
        }
        if dialect_from_interpreter(path_file_name(part)).is_some() {
            found_shell = true;
        }
    }
    flags
}

fn parse_shebang_dialect(first_line: &str) -> Option<ShellDialect> {
    let rest = first_line.strip_prefix("#!")?.trim();
    let mut parts = rest.split_ascii_whitespace();
    let first = parts.next()?;
    let cmd = path_file_name(first);
    if !is_launcher(cmd) {
        return dialect_from_interpreter(cmd);
    }
    for part in parts {
        if part.starts_with('-') || part.contains('=') {
            continue;
        }
        return dialect_from_interpreter(path_file_name(part));
    }
    None
}

fn is_launcher(name: &str) -> bool {
    matches!(name, "env" | "busybox")
}

fn dialect_from_interpreter(name: &str) -> Option<ShellDialect> {
    if name == "bash" || is_versioned_name(name, "bash") {
        Some(ShellDialect::Bash)
    } else if name == "zsh" || is_versioned_name(name, "zsh") {
        Some(ShellDialect::Zsh)
    } else if matches!(name, "sh" | "dash" | "ash" | "posix-sh") {
        Some(ShellDialect::PosixSh)
    } else {
        None
    }
}

fn is_versioned_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|rest| {
        !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    })
}

fn dialect_from_extension(path: &str) -> ShellDialect {
    match path_file_name(path).rsplit_once('.') {
        Some((_, "bash")) => ShellDialect::Bash,
        Some((_, "zsh")) => ShellDialect::Zsh,
        _ => ShellDialect::PosixSh,
    }
}
