use std::collections::BTreeSet;
use std::path::Path;

use codeint_model::{LowerError, RepoPath, SourceSet, settle_source_set};
use codeint_plugin_host::{NamedLanguagePlugin, built_in_language_plugin};

use crate::error::EvaluationError;
use crate::scratch::tracked_files;

pub(crate) struct LoweredFile {
    pub(crate) path: String,
    pub(crate) outcome: LoweredOutcome,
}

pub(crate) enum LoweredOutcome {
    Measured {
        source_set: SourceSet,
        functions: Vec<[u32; 3]>,
        imports: BTreeSet<String>,
    },
    Unparsable,
    NotMeasured,
}

pub(crate) fn lower_language_files(
    checkout: &Path,
    language: &str,
) -> Result<Vec<LoweredFile>, EvaluationError> {
    let plugin = built_in_language_plugin(language).ok_or_else(|| EvaluationError::Command {
        command: format!("lower {language} files"),
        detail: format!("{language} is not a built-in Language plugin"),
    })?;
    let extensions = plugin.plugin.extensions();
    Ok(tracked_files(checkout)?
        .into_iter()
        .filter(|path| has_extension(path, &extensions))
        .map(|path| LoweredFile {
            outcome: if is_measured_file(checkout, &path) {
                lower_file(plugin, checkout, &path)
            } else {
                LoweredOutcome::NotMeasured
            },
            path,
        })
        .collect())
}

fn is_measured_file(checkout: &Path, path: &str) -> bool {
    std::fs::symlink_metadata(checkout.join(path)).is_ok_and(|metadata| metadata.is_file())
}

fn has_extension(path: &str, extensions: &[String]) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extensions.iter().any(|known| known == extension))
}

fn lower_file(plugin: NamedLanguagePlugin<'_>, checkout: &Path, path: &str) -> LoweredOutcome {
    let Ok(text) = std::fs::read_to_string(checkout.join(path)) else {
        return LoweredOutcome::NotMeasured;
    };
    let Ok(repo_path) = RepoPath::new(path) else {
        return LoweredOutcome::NotMeasured;
    };
    match plugin.plugin.lower(&repo_path, &text, None) {
        Ok(model) => LoweredOutcome::Measured {
            source_set: settle_source_set(&repo_path, &text, model.source_set),
            functions: model
                .functions
                .iter()
                .map(|function| [function.line, function.end_line, function.decision_points])
                .collect(),
            imports: model.runtime_imports.into_iter().collect(),
        },
        Err(LowerError::Unparsable { .. }) => LoweredOutcome::Unparsable,
        Err(_) => LoweredOutcome::NotMeasured,
    }
}
