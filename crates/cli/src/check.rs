use std::collections::HashMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_config::{
    Configuration, ConfigurationFileError, FindingsShown, LanguageCoverage, SourceSetOverrides,
    read_configuration,
};
use codeint_git::{GitError, Repository, Snapshot};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LowerError, Manifest, RepoPath, ResolveError,
    SkippedFile, SourceFile, Workspace, WorkspaceError, choose_manifests, resolve_runtime_imports,
    settle_source_set,
};
use codeint_plugin_host::{PluginHostError, WasmLanguagePlugin, load_language_plugins};
use codeint_report::{MeasuredLanguage, Report};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

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
    #[error(transparent)]
    PluginHost(#[from] PluginHostError),
}

pub(crate) struct NamedLanguagePlugin<'plugin> {
    pub(crate) name: &'plugin str,
    pub(crate) plugin: &'plugin dyn LanguagePlugin,
}

pub(crate) const BUILT_IN_LANGUAGE_PLUGINS: [NamedLanguagePlugin<'static>; 1] =
    [NamedLanguagePlugin {
        name: "typescript-javascript",
        plugin: &TypeScriptAndJavaScript,
    }];

pub(crate) struct LoadedPlugins {
    external: Vec<(String, WasmLanguagePlugin)>,
}

impl LoadedPlugins {
    pub(crate) fn load(
        repository_root: &Path,
        configuration: &Configuration,
    ) -> Result<LoadedPlugins, CheckError> {
        let reserved: Vec<&str> = BUILT_IN_LANGUAGE_PLUGINS
            .iter()
            .map(|plugin| plugin.name)
            .collect();
        let external = load_language_plugins(repository_root, &configuration.plugins, &reserved)?;
        Ok(LoadedPlugins { external })
    }

    pub(crate) fn as_named(&self) -> Vec<NamedLanguagePlugin<'_>> {
        BUILT_IN_LANGUAGE_PLUGINS
            .iter()
            .map(|named| NamedLanguagePlugin {
                name: named.name,
                plugin: named.plugin,
            })
            .chain(
                self.external
                    .iter()
                    .map(|(name, plugin)| NamedLanguagePlugin {
                        name: name.as_str(),
                        plugin: plugin as &dyn LanguagePlugin,
                    }),
            )
            .collect()
    }
}

pub(crate) fn check(folder: &Path, findings: FindingsShown) -> Result<Report, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let loaded = LoadedPlugins::load(repository.root(), &configuration)?;
    let plugins = loaded.as_named();
    check_loaded(&repository, &configuration, &plugins, findings)
}

#[cfg(test)]
fn check_with(
    folder: &Path,
    plugins: &[NamedLanguagePlugin<'_>],
    findings: FindingsShown,
) -> Result<Report, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    check_loaded(&repository, &configuration, plugins, findings)
}

fn check_loaded(
    repository: &Repository,
    configuration: &Configuration,
    plugins: &[NamedLanguagePlugin<'_>],
    findings: FindingsShown,
) -> Result<Report, CheckError> {
    let tree = repository.working_tree()?;
    let languages = measure_with(&tree, plugins, configuration)?;
    Ok(Report::summarize(
        languages,
        tree.submodules().to_vec(),
        findings,
        &configuration.budgets,
    ))
}

pub(crate) fn measure_with(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<Vec<MeasuredLanguage>, CheckError> {
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name).collect();
    let coverage = configuration.languages.coverage_of(&names)?;
    measure(
        tree,
        plugins,
        &coverage,
        &configuration.source_set_overrides,
    )
}

fn measure(
    tree: &Snapshot,
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
    let covered_files: Vec<(&RepoPath, usize)> = tree
        .files()
        .iter()
        .filter_map(|path| {
            let &index = path
                .extension()
                .and_then(|extension| plugins_by_extension.get(extension))?;
            coverage[index].covers(path).then_some((path, index))
        })
        .collect();
    let outcomes = if let Some(reader) = tree.working_tree_reader() {
        covered_files
            .par_iter()
            .map(|&(path, index)| {
                let bytes = reader.read(path)?;
                Ok((
                    index,
                    lower_bytes(bytes, path, plugins[index].plugin, overrides),
                ))
            })
            .collect::<Result<Vec<_>, CheckError>>()?
    } else {
        let loaded = covered_files
            .iter()
            .map(|&(path, index)| Ok((path, index, tree.read(path)?)))
            .collect::<Result<Vec<_>, CheckError>>()?;
        loaded
            .into_par_iter()
            .map(|(path, index, bytes)| {
                (
                    index,
                    lower_bytes(bytes, path, plugins[index].plugin, overrides),
                )
            })
            .collect()
    };
    for (index, outcome) in outcomes {
        let model = &mut models[index];
        match outcome {
            LoweredFile::Measured(file) => model.files.push(file),
            LoweredFile::DeclarationOnly(path) => model.declaration_only.push(path),
            LoweredFile::Skipped(skipped) => model.skipped.push(skipped),
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
    tree: &Snapshot,
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

enum LoweredFile {
    Measured(SourceFile),
    DeclarationOnly(RepoPath),
    Skipped(SkippedFile),
}

fn lower_bytes(
    bytes: Vec<u8>,
    path: &RepoPath,
    plugin: &dyn LanguagePlugin,
    overrides: &SourceSetOverrides,
) -> LoweredFile {
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => return LoweredFile::Skipped(not_utf8(path.clone(), &error)),
    };
    match plugin.lower(path, &text) {
        Ok(mut file) => {
            file.source_set =
                overrides.source_set(path, settle_source_set(path, &text, file.source_set));
            LoweredFile::Measured(SourceFile {
                path: path.clone(),
                model: file,
            })
        }
        Err(LowerError::DeclarationOnly) => LoweredFile::DeclarationOnly(path.clone()),
        Err(reason) => LoweredFile::Skipped(SkippedFile {
            path: path.clone(),
            reason: reason.to_string(),
        }),
    }
}

fn not_utf8(path: RepoPath, error: &FromUtf8Error) -> SkippedFile {
    SkippedFile {
        path,
        reason: format!("not valid UTF-8: {}", error.utf8_error()),
    }
}
