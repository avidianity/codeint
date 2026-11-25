use std::collections::HashSet;

use codeint_model::{CodeModel, Package, RepoPath, Workspace};
use codeint_report::MeasuredLanguage;

use super::errors::CheckError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelectedPackage {
    Root,
    Member(RepoPath),
}

impl SelectedPackage {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            SelectedPackage::Root => ".",
            SelectedPackage::Member(root) => root.as_str(),
        }
    }

    pub(crate) fn matches(&self, package: Package<'_>) -> bool {
        match (self, package) {
            (SelectedPackage::Root, Package::Root) => true,
            (SelectedPackage::Member(selected), Package::Member(actual)) => selected == actual,
            _ => false,
        }
    }
}

pub(crate) fn select_workspace_package(
    workspace: &Workspace,
    requested: &str,
) -> Result<SelectedPackage, CheckError> {
    let normalized = normalize_package_path(requested);
    if workspace.has_members() {
        if normalized == "." {
            return Ok(SelectedPackage::Root);
        }
        if let Ok(candidate) = RepoPath::new(&normalized)
            && workspace.member_roots().any(|root| root == &candidate)
        {
            return Ok(SelectedPackage::Member(candidate));
        }
    }
    Err(CheckError::UnknownWorkspacePackage {
        requested: requested.to_owned(),
        available: workspace.member_roots().map(ToString::to_string).collect(),
        skipped_manifest: workspace.skipped_manifest_in(&normalized).map(|skipped| {
            format!(
                "its {} manifest {} is invalid and was skipped: {}",
                skipped.plugin, skipped.path, skipped.reason
            )
        }),
    })
}

fn normalize_package_path(raw: &str) -> String {
    let replaced = raw.replace('\\', "/");
    let mut trimmed = replaced.as_str();
    while let Some(stripped) = trimmed.strip_prefix("./") {
        trimmed = stripped;
    }
    trimmed = trimmed.trim_end_matches('/');
    if trimmed.is_empty() && !raw.is_empty() && replaced.trim_end_matches('/') == "." {
        return ".".to_owned();
    }
    trimmed.to_owned()
}

pub(crate) fn workspace_of_languages(languages: &[MeasuredLanguage]) -> Workspace {
    let mut workspace = Workspace::default();
    for language in languages {
        workspace.join(language.model.workspace.clone());
    }
    workspace
}

pub(super) fn filter_model_to_package(model: &mut CodeModel, selected: &SelectedPackage) {
    let workspace = std::mem::take(&mut model.workspace);
    model
        .files
        .retain(|file| selected.matches(workspace.package_of(&file.path)));
    if model.files.is_empty() {
        model.languages.clear();
    }
    model
        .skipped
        .retain(|file| selected.matches(workspace.package_of(&file.path)));
    model
        .declaration_only
        .retain(|path| selected.matches(workspace.package_of(path)));
    model
        .runtime_imports
        .retain(|import| selected.matches(workspace.package_of(&import.importer)));
    model
        .dependency_manifests
        .retain(|manifest| selected.matches(workspace.package_of(&manifest.path)));
    let retained_roots: HashSet<String> = model
        .dependency_manifests
        .iter()
        .map(|manifest| manifest.package_root.clone())
        .collect();
    model.declared_dependencies.retain(|dep| {
        retained_roots.contains(&dep.package_root)
            || matches_selected_package_root(selected, &dep.package_root)
    });
    model.manifest_used_dependencies.retain(|dep| {
        retained_roots.contains(&dep.package_root)
            || matches_selected_package_root(selected, &dep.package_root)
    });
}

fn matches_selected_package_root(selected: &SelectedPackage, package_root: &str) -> bool {
    match selected {
        SelectedPackage::Root => package_root.is_empty(),
        SelectedPackage::Member(root) => package_root == root.as_str(),
    }
}

pub(super) fn package_root_matches_selected(
    workspace: &Workspace,
    selected: &SelectedPackage,
    package_root: &str,
) -> bool {
    let actual = if package_root.is_empty() {
        Package::Root
    } else if let Ok(synthetic) = RepoPath::new(format!("{package_root}/manifest")) {
        workspace.package_of(&synthetic)
    } else {
        return matches_selected_package_root(selected, package_root);
    };
    selected.matches(actual)
}

pub(crate) fn filter_languages_to_package(
    languages: &mut [MeasuredLanguage],
    selected: &SelectedPackage,
) {
    for language in languages {
        filter_model_to_package(&mut language.model, selected);
    }
}

pub(super) fn resolve_package_selection(
    languages: &[MeasuredLanguage],
    package: Option<&str>,
) -> Result<Option<(SelectedPackage, Workspace)>, CheckError> {
    let Some(requested) = package else {
        return Ok(None);
    };
    let workspace = workspace_of_languages(languages);
    Ok(Some((
        select_workspace_package(&workspace, requested)?,
        workspace,
    )))
}
