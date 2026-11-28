use std::collections::HashSet;

use crate::{CodeModel, Package, RepoPath, SourceFile};

#[cfg(test)]
mod tests;

const UNIVERSAL_TOOLING_DOT_DIRS: &[&str] = &[".github"];

const UNIVERSAL_TOOLING_DIRS: &[&str] = &["scripts", "docs", "examples", "tools", "benches"];

#[derive(Clone, Debug, Default)]
pub struct UniversalToolingPaths {
    package_roots: HashSet<String>,
}

impl UniversalToolingPaths {
    pub fn from_model(model: &CodeModel) -> UniversalToolingPaths {
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
        UniversalToolingPaths::from_roots(
            workspace_roots.chain(manifest_roots).chain(declared_roots),
        )
    }

    pub fn from_roots<'a>(roots: impl IntoIterator<Item = &'a str>) -> UniversalToolingPaths {
        let package_roots = roots
            .into_iter()
            .map(|root| root.trim_end_matches('/'))
            .filter(|root| !root.is_empty())
            .map(str::to_owned)
            .collect();
        UniversalToolingPaths { package_roots }
    }

    pub fn is_universal_tooling_path(&self, path: &RepoPath) -> bool {
        let repo_rel = path.as_str();
        let (repo_dir, file_name) = split_dir_and_file(repo_rel);
        if file_name.is_empty() {
            return false;
        }
        if has_universal_dot_dir(repo_dir) || starts_with_universal_tooling_dir(repo_rel) {
            return true;
        }

        let mut folder = repo_rel;
        while let Some((prefix, _)) = folder.rsplit_once('/') {
            if self.package_roots.contains(prefix) {
                let pkg_rel = &repo_rel[prefix.len() + 1..];
                if starts_with_universal_tooling_dir(pkg_rel) {
                    return true;
                }
            }
            folder = prefix;
        }
        false
    }

    pub fn is_tooling_file(&self, file: &SourceFile) -> bool {
        file.model.is_tooling_file || self.is_universal_tooling_path(&file.path)
    }
}

fn has_universal_dot_dir(dir_path: Option<&str>) -> bool {
    dir_path.is_some_and(|dir| {
        dir.split('/').any(|segment| {
            UNIVERSAL_TOOLING_DOT_DIRS.contains(&segment.to_ascii_lowercase().as_str())
        })
    })
}

fn split_dir_and_file(path: &str) -> (Option<&str>, &str) {
    match path.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, path),
    }
}

fn starts_with_universal_tooling_dir(relative_path: &str) -> bool {
    let Some((first_dir, _)) = relative_path.split_once('/') else {
        return false;
    };
    UNIVERSAL_TOOLING_DIRS.contains(&first_dir.to_ascii_lowercase().as_str())
}
