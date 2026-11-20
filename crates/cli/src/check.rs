use std::collections::HashMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, LanguagePlugin, LowerError, RepoPath, ResolveError, SkippedFile, SourceFile,
    resolve_runtime_imports,
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
    #[error("cannot resolve runtime imports")]
    Resolve(#[source] ResolveError),
    #[error(transparent)]
    Report(#[from] ReportError),
}

pub(crate) fn check(folder: &Path) -> Result<String, CheckError> {
    let tree = WorkingTree::discover(folder)?;
    let model = measure(&tree, &[&TypeScriptAndJavaScript])?;
    Ok(Report::summarize(&model).to_toon()?)
}

fn measure(tree: &WorkingTree, plugins: &[&dyn LanguagePlugin]) -> Result<CodeModel, CheckError> {
    let plugin_by_extension: HashMap<String, usize> = plugins
        .iter()
        .enumerate()
        .flat_map(|(index, plugin)| {
            plugin
                .extensions()
                .into_iter()
                .map(move |extension| (extension, index))
        })
        .collect();
    let mut model = CodeModel::default();
    let mut plugin_of_file = Vec::new();
    for path in tree.files() {
        let Some(&plugin_index) = path
            .extension()
            .and_then(|extension| plugin_by_extension.get(extension))
        else {
            continue;
        };
        let bytes = tree.read(path)?;
        let text = String::from_utf8(bytes).map_err(|source| CheckError::NotUtf8 {
            path: path.clone(),
            source,
        })?;
        match plugins[plugin_index].lower(path, &text) {
            Ok(file) => {
                model.files.push(SourceFile {
                    path: path.clone(),
                    model: file,
                });
                plugin_of_file.push(plugin_index);
            }
            Err(LowerError::DeclarationOnly) => model.declaration_only.push(path.clone()),
            Err(reason) => model.skipped.push(SkippedFile {
                path: path.clone(),
                reason: reason.to_string(),
            }),
        }
    }
    for (plugin_index, plugin) in plugins.iter().enumerate() {
        let importers: Vec<&SourceFile> = model
            .files
            .iter()
            .zip(&plugin_of_file)
            .filter(|(_, file_plugin)| **file_plugin == plugin_index)
            .map(|(file, _)| file)
            .collect();
        let runtime_imports = resolve_runtime_imports(*plugin, tree.files(), &importers)
            .map_err(CheckError::Resolve)?;
        model.runtime_imports.extend(runtime_imports);
    }
    Ok(model)
}
