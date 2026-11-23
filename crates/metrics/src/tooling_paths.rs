use codeint_model::RepoPath;

#[cfg(test)]
mod tests;

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
    "benches",
    "docs",
];

const ANY_DEPTH_TOOLING_FILENAMES: &[&str] = &["conftest.py"];

const ROOT_LEVEL_TOOLING_FILENAMES: &[&str] = &["conf.py", "setup.py", "noxfile.py", "fabfile.py"];

pub(crate) fn is_non_runtime_tooling_path(path: &RepoPath, package_root: Option<&str>) -> bool {
    let repo_rel = path.as_str();
    let (repo_dir, file_name) = split_dir_and_file(repo_rel);
    if file_name.is_empty() {
        return false;
    }

    if let Some(dir_path) = repo_dir
        && dir_path
            .split('/')
            .any(|segment| TOOLING_DOT_DIRS.contains(&segment.to_ascii_lowercase().as_str()))
    {
        return true;
    }

    let pkg_rel = strip_package_root(repo_rel, package_root);
    if starts_with_tooling_dir(repo_rel) || starts_with_tooling_dir(pkg_rel) {
        return true;
    }

    let at_root = !repo_rel.contains('/') || !pkg_rel.contains('/');
    is_tooling_config_filename(file_name, at_root)
}

fn split_dir_and_file(path: &str) -> (Option<&str>, &str) {
    match path.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, path),
    }
}

fn strip_package_root<'a>(repo_rel: &'a str, package_root: Option<&str>) -> &'a str {
    let Some(root) = package_root
        .map(|root| root.trim_end_matches('/'))
        .filter(|root| !root.is_empty())
    else {
        return repo_rel;
    };
    repo_rel
        .strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(repo_rel)
}

fn starts_with_tooling_dir(relative_path: &str) -> bool {
    let Some((first_dir, _)) = relative_path.split_once('/') else {
        return false;
    };
    TOOLING_DIRS.contains(&first_dir.to_ascii_lowercase().as_str())
}

fn is_tooling_config_filename(file_name: &str, at_root: bool) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if ANY_DEPTH_TOOLING_FILENAMES.contains(&lower.as_str()) {
        return true;
    }
    if !at_root {
        return false;
    }
    ROOT_LEVEL_TOOLING_FILENAMES.contains(&lower.as_str())
        || is_leading_dot_config_filename(&lower)
        || is_dotted_config_filename(&lower)
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
