#[cfg(test)]
mod tests;

const RUNTIME_SOURCE_DIRS: &[&str] = &["src", "lib"];

const TOOLING_DOT_DIRS: &[&str] = &[
    ".changeset",
    ".github",
    ".opencode",
    ".claude",
    ".agents",
    ".vscode",
    ".husky",
    ".storybook",
];

const TOOLING_DIRS: &[&str] = &[
    "scripts",
    "examples",
    "example",
    "tool",
    "tools",
    "benchmarks",
    "bench",
    "docs",
];

const UNCONDITIONAL_TOOLING_FILENAMES: &[&str] = &["noxfile.py", "fabfile.py", "conftest.py"];

const ROOT_LEVEL_TOOLING_FILENAMES: &[&str] = &["conf.py", "setup.py"];

pub(crate) fn is_non_runtime_tooling_path(path: &str) -> bool {
    let (dir_path, file_name) = match path.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, path),
    };
    if file_name.is_empty() {
        return false;
    }

    let mut under_runtime_source_dir = false;
    if let Some(dir_path) = dir_path {
        for segment in dir_path.split('/').filter(|segment| !segment.is_empty()) {
            let lower = segment.to_ascii_lowercase();
            if TOOLING_DOT_DIRS.contains(&lower.as_str()) {
                return true;
            }
            if !under_runtime_source_dir && TOOLING_DIRS.contains(&lower.as_str()) {
                return true;
            }
            if RUNTIME_SOURCE_DIRS.contains(&lower.as_str()) {
                under_runtime_source_dir = true;
            }
        }
    }

    is_tooling_config_filename(file_name, under_runtime_source_dir)
}

fn is_tooling_config_filename(file_name: &str, under_runtime_source_dir: bool) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if UNCONDITIONAL_TOOLING_FILENAMES.contains(&lower.as_str()) {
        return true;
    }
    if is_leading_dot_config_filename(&lower) {
        return true;
    }
    if under_runtime_source_dir {
        return false;
    }
    ROOT_LEVEL_TOOLING_FILENAMES.contains(&lower.as_str()) || is_dotted_config_filename(&lower)
}

fn is_leading_dot_config_filename(lower: &str) -> bool {
    let Some(rest) = lower.strip_prefix('.') else {
        return false;
    };
    if rest.starts_with("php-cs-fixer") {
        return true;
    }
    if is_dotted_config_filename(rest) {
        return true;
    }
    rest.split('.')
        .next()
        .is_some_and(|stem| stem.ends_with("rc") && stem != "rc" && stem != "src")
}

fn is_dotted_config_filename(lower: &str) -> bool {
    lower
        .find(".config.")
        .is_some_and(|index| index > 0 && index + ".config.".len() < lower.len())
}
