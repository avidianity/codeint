use std::collections::HashMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_config::{
    ConfigurationFileError, FindingsShown, LanguageCoverage, SourceSetOverrides, read_configuration,
};
use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LowerError, RepoPath, ResolveError,
    SkippedFile, SourceFile, resolve_runtime_imports, settle_source_set,
};
use codeint_report::{MeasuredLanguage, Report, ReportError};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
    #[error("{path} is not valid UTF-8")]
    NotUtf8 {
        path: RepoPath,
        #[source]
        source: FromUtf8Error,
    },
    #[error("cannot resolve runtime imports")]
    Resolve(#[source] ResolveError),
    #[error("a Language plugin supplies an invalid Reference range")]
    ReferenceRange(#[source] InvalidReferenceRange),
    #[error(transparent)]
    Report(#[from] ReportError),
}

pub(crate) struct NamedLanguagePlugin<'plugin> {
    pub(crate) name: &'static str,
    pub(crate) plugin: &'plugin dyn LanguagePlugin,
}

const BUILT_IN_LANGUAGE_PLUGINS: [NamedLanguagePlugin<'static>; 1] = [NamedLanguagePlugin {
    name: "typescript-javascript",
    plugin: &TypeScriptAndJavaScript,
}];

pub(crate) fn check(folder: &Path, findings: FindingsShown) -> Result<String, CheckError> {
    check_with(folder, &BUILT_IN_LANGUAGE_PLUGINS, findings)
}

fn check_with(
    folder: &Path,
    plugins: &[NamedLanguagePlugin<'_>],
    findings: FindingsShown,
) -> Result<String, CheckError> {
    let tree = WorkingTree::discover(folder)?;
    let configuration = read_configuration(tree.root())?;
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name).collect();
    let coverage = configuration.languages.coverage_of(&names)?;
    let languages = measure(
        &tree,
        plugins,
        &coverage,
        &configuration.source_set_overrides,
    )?;
    Ok(Report::summarize(languages, findings).to_toon()?)
}

fn measure(
    tree: &WorkingTree,
    plugins: &[NamedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
    overrides: &SourceSetOverrides,
) -> Result<Vec<MeasuredLanguage>, CheckError> {
    let plugins_by_extension: HashMap<String, usize> = plugins
        .iter()
        .enumerate()
        .flat_map(|(index, named)| {
            named
                .plugin
                .extensions()
                .into_iter()
                .map(move |extension| (extension, index))
        })
        .collect();
    let mut models: Vec<CodeModel> = plugins.iter().map(|_| CodeModel::default()).collect();
    for path in tree.files() {
        let Some(&index) = path
            .extension()
            .and_then(|extension| plugins_by_extension.get(extension))
        else {
            continue;
        };
        if !coverage[index].covers(path) {
            continue;
        }
        let bytes = tree.read(path)?;
        let text = String::from_utf8(bytes).map_err(|source| CheckError::NotUtf8 {
            path: path.clone(),
            source,
        })?;
        let model = &mut models[index];
        match plugins[index].plugin.lower(path, &text) {
            Ok(mut file) => {
                file.source_set =
                    overrides.source_set(path, settle_source_set(path, &text, file.source_set));
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
    plugins
        .iter()
        .zip(models)
        .map(|(named, mut model)| {
            let importers: Vec<&SourceFile> = model.files.iter().collect();
            model.runtime_imports = resolve_runtime_imports(named.plugin, tree.files(), &importers)
                .map_err(CheckError::Resolve)?;
            Ok(MeasuredLanguage {
                plugin: named.name.to_owned(),
                model,
                reference_ranges: named
                    .plugin
                    .reference_ranges()
                    .map_err(CheckError::ReferenceRange)?,
            })
        })
        .collect()
}
