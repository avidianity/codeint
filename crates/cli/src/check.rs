use std::collections::HashMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_config::{
    ConfigurationFileError, FindingsShown, LanguageCoverage, SourceSetOverrides, read_configuration,
};
use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LowerError, Manifest, RepoPath, ResolveError,
    SkippedFile, SourceFile, Workspace, WorkspaceError, choose_manifests, resolve_runtime_imports,
    settle_source_set,
};
use codeint_report::{MeasuredLanguage, Report};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
    #[error("cannot resolve runtime imports")]
    Resolve(#[source] ResolveError),
    #[error("a Language plugin supplies an invalid Reference range")]
    ReferenceRange(#[source] InvalidReferenceRange),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

pub(crate) struct NamedLanguagePlugin<'plugin> {
    pub(crate) name: &'static str,
    pub(crate) plugin: &'plugin dyn LanguagePlugin,
}

pub(crate) const BUILT_IN_LANGUAGE_PLUGINS: [NamedLanguagePlugin<'static>; 1] =
    [NamedLanguagePlugin {
        name: "typescript-javascript",
        plugin: &TypeScriptAndJavaScript,
    }];

pub(crate) fn check(folder: &Path, findings: FindingsShown) -> Result<Report, CheckError> {
    check_with(folder, &BUILT_IN_LANGUAGE_PLUGINS, findings)
}

fn check_with(
    folder: &Path,
    plugins: &[NamedLanguagePlugin<'_>],
    findings: FindingsShown,
) -> Result<Report, CheckError> {
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
    Ok(Report::summarize(
        languages,
        tree.submodules().to_vec(),
        findings,
        &configuration.budgets,
    ))
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
    let manifests_by_plugin = read_manifests(tree, plugins)?;
    let workspace = find_workspace(plugins, &manifests_by_plugin)?;
    let mut models: Vec<CodeModel> = plugins
        .iter()
        .map(|_| CodeModel {
            workspace: workspace.clone(),
            ..CodeModel::default()
        })
        .collect();
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
        let model = &mut models[index];
        let bytes = tree.read(path)?;
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                model.skipped.push(not_utf8(path.clone(), &error));
                continue;
            }
        };
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
        .zip(manifests_by_plugin)
        .map(|((named, mut model), manifests)| {
            let importers: Vec<&SourceFile> = model.files.iter().collect();
            model.runtime_imports =
                resolve_runtime_imports(named.plugin, tree.files(), manifests, &importers)
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

fn read_manifests(
    tree: &WorkingTree,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<Vec<Manifest>>, CheckError> {
    plugins
        .iter()
        .map(|named| {
            choose_manifests(named.plugin, tree.files())
                .map_err(CheckError::Resolve)?
                .into_iter()
                .map(|path| {
                    Ok(Manifest {
                        text: String::from_utf8_lossy(&tree.read(&path)?).into_owned(),
                        path: path.to_string(),
                    })
                })
                .collect()
        })
        .collect()
}

fn find_workspace(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: &[Vec<Manifest>],
) -> Result<Workspace, CheckError> {
    let mut member_roots = Vec::new();
    for (named, manifests) in plugins.iter().zip(manifests_by_plugin) {
        member_roots.extend(named.plugin.workspace_packages(manifests.clone())?);
    }
    Ok(Workspace::new(member_roots))
}

fn not_utf8(path: RepoPath, error: &FromUtf8Error) -> SkippedFile {
    SkippedFile {
        path,
        reason: format!("not valid UTF-8: {}", error.utf8_error()),
    }
}
