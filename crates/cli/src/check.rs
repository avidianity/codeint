use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use codeint_config::{
    ConfigurationFileError, FindingsShown, SourceSetOverrides, read_configuration,
};
use codeint_git::{GitError, WorkingTree};
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LowerError, ReferenceRanges, ResolveError,
    SkippedFile, SourceFile, resolve_runtime_imports, settle_source_set,
};
use codeint_report::{ReferenceRangeScope, Report};

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
}

struct Measurement {
    model: CodeModel,
    ranges: ReferenceRangeScope,
}

pub(crate) fn check(folder: &Path, findings: FindingsShown) -> Result<Report, CheckError> {
    check_with(folder, &[&TypeScriptAndJavaScript], findings)
}

fn check_with(
    folder: &Path,
    plugins: &[&dyn LanguagePlugin],
    findings: FindingsShown,
) -> Result<Report, CheckError> {
    let tree = WorkingTree::discover(folder)?;
    let configuration = read_configuration(tree.root())?;
    let Measurement { model, ranges } =
        measure(&tree, plugins, &configuration.source_set_overrides)?;
    Ok(Report::summarize(model, &ranges, findings))
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

fn measure(
    tree: &WorkingTree,
    plugins: &[&dyn LanguagePlugin],
    overrides: &SourceSetOverrides,
) -> Result<Measurement, CheckError> {
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
        submodules: tree.submodules().to_vec(),
        ..CodeModel::default()
    };
    let mut measured = BTreeSet::new();
    let mut plugin_of_file = Vec::new();
    for path in tree.files() {
        let Some(&index) = path
            .extension()
            .and_then(|extension| plugins_by_extension.get(extension))
        else {
            continue;
        };
        let bytes = tree.read(path)?;
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                model.skipped.push(SkippedFile {
                    path: path.clone(),
                    reason: format!("not valid UTF-8: {}", error.utf8_error()),
                });
                continue;
            }
        };
        match plugins[index].lower(path, &text) {
            Ok(mut file) => {
                measured.insert(index);
                file.source_set =
                    overrides.source_set(path, settle_source_set(path, &text, file.source_set));
                model.files.push(SourceFile {
                    path: path.clone(),
                    model: file,
                });
                plugin_of_file.push(index);
            }
            Err(LowerError::DeclarationOnly) => model.declaration_only.push(path.clone()),
            Err(reason) => model.skipped.push(SkippedFile {
                path: path.clone(),
                reason: reason.to_string(),
            }),
        }
    }
    for (index, plugin) in plugins.iter().enumerate() {
        let importers: Vec<&SourceFile> = model
            .files
            .iter()
            .zip(&plugin_of_file)
            .filter(|(_, file_plugin)| **file_plugin == index)
            .map(|(file, _)| file)
            .collect();
        let runtime_imports = resolve_runtime_imports(*plugin, tree.files(), &importers)
            .map_err(CheckError::Resolve)?;
        model.runtime_imports.extend(runtime_imports);
    }
    let ranges = reference_range_scope(plugins, &measured)?;
    Ok(Measurement { model, ranges })
}
