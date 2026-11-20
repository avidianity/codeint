use std::collections::HashMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_metrics::duplication::{MinimumCloneTokens, find_clone_groups};
use codeint_model::{CodeModel, LanguagePlugin, LowerError, RepoPath, SourceFile};
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
    #[error("{path}")]
    Lower {
        path: RepoPath,
        #[source]
        source: LowerError,
    },
    #[error(transparent)]
    Report(#[from] ReportError),
}

pub(crate) fn check(folder: &Path) -> Result<String, CheckError> {
    let tree = WorkingTree::discover(folder)?;
    let model = measure(&tree, &[&TypeScriptAndJavaScript])?;
    let duplication = find_clone_groups(&model, MinimumCloneTokens::PROVISIONAL_DEFAULT);
    Ok(Report::summarize(&model, &duplication).to_toon()?)
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
    let mut model = CodeModel::default();
    for path in tree.files() {
        let Some(plugin) = path
            .extension()
            .and_then(|extension| plugins_by_extension.get(extension))
        else {
            continue;
        };
        let bytes = tree.read(path)?;
        let text = String::from_utf8(bytes).map_err(|source| CheckError::NotUtf8 {
            path: path.clone(),
            source,
        })?;
        let file = plugin
            .lower(path, &text)
            .map_err(|source| CheckError::Lower {
                path: path.clone(),
                source,
            })?;
        model.files.push(SourceFile {
            path: path.clone(),
            model: file,
        });
    }
    Ok(model)
}
