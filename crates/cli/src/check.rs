use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_config::FindingsShown;
use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LowerError, Manifest, ReferenceRanges,
    RepoPath, SkippedFile, SourceFile, Workspace, WorkspaceError,
};
use codeint_report::{ReferenceRangeScope, Report, ReportError};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("{path} is not valid UTF-8")]
    NotUtf8 {
        path: RepoPath,
        #[source]
        source: FromUtf8Error,
    },
    #[error("a Language plugin supplies an invalid Reference range")]
    ReferenceRange(#[source] InvalidReferenceRange),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Report(#[from] ReportError),
}

struct Measurement {
    model: CodeModel,
    ranges: ReferenceRangeScope,
}

pub(crate) fn check(folder: &Path, findings: FindingsShown) -> Result<String, CheckError> {
    check_with(folder, &[&TypeScriptAndJavaScript], findings)
}

fn check_with(
    folder: &Path,
    plugins: &[&dyn LanguagePlugin],
    findings: FindingsShown,
) -> Result<String, CheckError> {
    let tree = WorkingTree::discover(folder)?;
    let Measurement { model, ranges } = measure(&tree, plugins)?;
    Ok(Report::summarize(&model, &ranges, findings).to_toon()?)
}

fn reference_range_scope(
    plugins: &[&dyn LanguagePlugin],
    measured: &BTreeSet<usize>,
) -> Result<ReferenceRangeScope, CheckError> {
    let mut ranges_by_plugin = plugins
        .iter()
        .map(|plugin| plugin.reference_ranges())
        .collect::<Result<Vec<ReferenceRanges>, _>>()
        .map_err(CheckError::ReferenceRange)?;
    match measured.iter().collect::<Vec<_>>().as_slice() {
        [] => Ok(ReferenceRangeScope::OneLanguage(ReferenceRanges::default())),
        [only] => Ok(ReferenceRangeScope::OneLanguage(
            ranges_by_plugin.swap_remove(**only),
        )),
        _ => Ok(ReferenceRangeScope::SeveralLanguages),
    }
}

fn measure(tree: &WorkingTree, plugins: &[&dyn LanguagePlugin]) -> Result<Measurement, CheckError> {
    let plugins_by_extension: HashMap<String, usize> = plugins
        .iter()
        .enumerate()
        .flat_map(|(index, plugin)| {
            plugin
                .extensions()
                .into_iter()
                .map(move |extension| (extension, index))
        })
        .collect();
    let mut model = CodeModel {
        workspace: find_workspace(tree, plugins)?,
        ..CodeModel::default()
    };
    let mut measured = BTreeSet::new();
    for path in tree.files() {
        let Some(&index) = path
            .extension()
            .and_then(|extension| plugins_by_extension.get(extension))
        else {
            continue;
        };
        let text = read_text(tree, path)?;
        match plugins[index].lower(path, &text) {
            Ok(file) => {
                measured.insert(index);
                model.files.push(SourceFile {
                    path: path.clone(),
                    model: file,
                });
            }
            Err(LowerError::DeclarationOnly) => model.declaration_only.push(path.clone()),
            Err(reason) => model.skipped.push(SkippedFile {
                path: path.clone(),
                reason: reason.to_string(),
            }),
        }
    }
    let ranges = reference_range_scope(plugins, &measured)?;
    Ok(Measurement { model, ranges })
}

fn find_workspace(
    tree: &WorkingTree,
    plugins: &[&dyn LanguagePlugin],
) -> Result<Workspace, CheckError> {
    let mut member_roots = Vec::new();
    for plugin in plugins {
        let manifest_names = plugin.workspace_manifests();
        let manifests = tree
            .files()
            .iter()
            .filter(|path| manifest_names.iter().any(|name| name == path.file_name()))
            .map(|path| {
                Ok(Manifest {
                    path: path.to_string(),
                    text: read_text(tree, path)?,
                })
            })
            .collect::<Result<Vec<Manifest>, CheckError>>()?;
        member_roots.extend(plugin.workspace_packages(manifests)?);
    }
    Ok(Workspace::new(member_roots))
}

fn read_text(tree: &WorkingTree, path: &RepoPath) -> Result<String, CheckError> {
    let bytes = tree.read(path)?;
    String::from_utf8(bytes).map_err(|source| CheckError::NotUtf8 {
        path: path.clone(),
        source,
    })
}
