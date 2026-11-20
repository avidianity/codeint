use std::collections::HashMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, LanguagePlugin, LowerError, RepoPath, SkippedFile, SourceFile, Workspace,
    WorkspaceError,
};
use codeint_report::{Report, ReportError};

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
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Report(#[from] ReportError),
}

pub(crate) fn check(folder: &Path) -> Result<String, CheckError> {
    let tree = WorkingTree::discover(folder)?;
    let model = measure(&tree, &[&TypeScriptAndJavaScript])?;
    Ok(Report::summarize(&model).to_toon()?)
}

fn measure(tree: &WorkingTree, plugins: &[&dyn LanguagePlugin]) -> Result<CodeModel, CheckError> {
    let plugins_by_extension: HashMap<String, &dyn LanguagePlugin> = plugins
        .iter()
        .flat_map(|plugin| {
            plugin
                .extensions()
                .into_iter()
                .map(move |extension| (extension, *plugin))
        })
        .collect();
    let mut model = CodeModel {
        workspace: find_workspace(tree, plugins)?,
        ..CodeModel::default()
    };
    for path in tree.files() {
        let Some(plugin) = path
            .extension()
            .and_then(|extension| plugins_by_extension.get(extension))
        else {
            continue;
        };
        let text = read_text(tree, path)?;
        match plugin.lower(path, &text) {
            Ok(file) => model.files.push(SourceFile {
                path: path.clone(),
                model: file,
            }),
            Err(LowerError::DeclarationOnly) => model.declaration_only.push(path.clone()),
            Err(reason) => model.skipped.push(SkippedFile {
                path: path.clone(),
                reason: reason.to_string(),
            }),
        }
    }
    Ok(model)
}

fn find_workspace(
    _tree: &WorkingTree,
    _plugins: &[&dyn LanguagePlugin],
) -> Result<Workspace, CheckError> {
    todo!()
}

fn read_text(tree: &WorkingTree, path: &RepoPath) -> Result<String, CheckError> {
    let bytes = tree.read(path)?;
    String::from_utf8(bytes).map_err(|source| CheckError::NotUtf8 {
        path: path.clone(),
        source,
    })
}
