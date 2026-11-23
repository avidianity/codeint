use std::collections::HashSet;

use codeint_model::{CodeModel, Package, RepoPath};

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

#[derive(Clone, Debug, Default)]
pub(crate) struct ToolingPaths {
    package_roots: HashSet<String>,
}

impl ToolingPaths {
    pub(crate) fn from_model(model: &CodeModel) -> ToolingPaths {
        let workspace_roots = model
            .workspace
            .packages()
            .filter_map(|package| match package {
                Package::Root => None,
                Package::Member(root) => Some(root.as_str()),
            });
        let manifest_roots = model
            .dependency_manifests
            .iter()
            .map(|entry| entry.package_root.as_str());
        let declared_roots = model
            .declared_dependencies
            .iter()
            .map(|dep| dep.package_root.as_str());
        ToolingPaths::from_roots(workspace_roots.chain(manifest_roots).chain(declared_roots))
    }

    pub(crate) fn from_roots<'a>(roots: impl IntoIterator<Item = &'a str>) -> ToolingPaths {
        let package_roots = roots
            .into_iter()
            .map(|root| root.trim_end_matches('/'))
            .filter(|root| !root.is_empty())
            .map(str::to_owned)
            .collect();
        ToolingPaths { package_roots }
    }

    pub(crate) fn is_non_runtime_tooling_path(&self, path: &RepoPath) -> bool {
        let repo_rel = path.as_str();
        let (repo_dir, file_name) = split_dir_and_file(repo_rel);
        if file_name.is_empty() {
            return false;
        }
        if has_tooling_dot_dir(repo_dir) || starts_with_tooling_dir(repo_rel) {
            return true;
        }

        let mut deepest_pkg_rel: Option<&str> = None;
        let mut folder = repo_rel;
        while let Some((prefix, _)) = folder.rsplit_once('/') {
            if self.package_roots.contains(prefix) {
                let pkg_rel = &repo_rel[prefix.len() + 1..];
                deepest_pkg_rel.get_or_insert(pkg_rel);
                if starts_with_tooling_dir(pkg_rel) {
                    return true;
                }
            }
            folder = prefix;
        }

        let at_root =
            !repo_rel.contains('/') || deepest_pkg_rel.is_some_and(|rel| !rel.contains('/'));
        is_tooling_config_filename(file_name, at_root)
    }
}

fn has_tooling_dot_dir(dir_path: Option<&str>) -> bool {
    dir_path.is_some_and(|dir| {
        dir.split('/')
            .any(|segment| TOOLING_DOT_DIRS.contains(&segment.to_ascii_lowercase().as_str()))
    })
}

fn split_dir_and_file(path: &str) -> (Option<&str>, &str) {
    match path.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, path),
    }
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
