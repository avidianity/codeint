use std::path::Path;

use codeint_config::{Configuration, FindingsShown, read_configuration};
use codeint_git::{CommitId, Repository, ResultCache, Snapshot};
use codeint_model::CodeModel;
use codeint_report::{BaseCommit, CommitMeasurement, DiffReport};
use serde::Serialize;

use crate::check::{BUILT_IN_LANGUAGE_PLUGINS, CheckError, measure_with};
use crate::describe;

const CODEINT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize)]
struct MeasurementSettings<'configuration> {
    codeint_version: &'static str,
    language_plugins: Vec<PluginIdentity>,
    configuration: &'configuration Configuration,
}

#[derive(Serialize)]
struct PluginIdentity {
    name: &'static str,
    version: &'static str,
}

pub(crate) fn diff(
    folder: &Path,
    base_ref: &str,
    findings: FindingsShown,
    result_cache: Option<&ResultCache>,
) -> Result<DiffReport, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let merge_base = repository.merge_base_with_head(base_ref)?;
    let base = measure_commit(&repository, merge_base, &configuration, result_cache)?;
    let head = measure_all_languages(&repository.working_tree()?, &configuration)?;
    Ok(DiffReport::compare(
        BaseCommit {
            git_ref: base_ref.to_owned(),
            merge_base: merge_base.to_string(),
        },
        base,
        head,
        findings,
    ))
}

fn measure_commit(
    repository: &Repository,
    commit: CommitId,
    configuration: &Configuration,
    result_cache: Option<&ResultCache>,
) -> Result<CommitMeasurement, CheckError> {
    let settings = MeasurementSettings {
        codeint_version: CODEINT_VERSION,
        language_plugins: BUILT_IN_LANGUAGE_PLUGINS
            .iter()
            .map(|named| PluginIdentity {
                name: named.name,
                version: CODEINT_VERSION,
            })
            .collect(),
        configuration,
    };
    if let Some(cached) = result_cache.and_then(|cache| cache.read(commit, &settings)) {
        return Ok(cached);
    }
    let measurement = measure_all_languages(&repository.commit(commit)?, configuration)?;
    if let Some(cache) = result_cache
        && let Err(error) = cache.write(commit, &settings, &measurement)
    {
        eprintln!(
            "codeint: {}; the next run measures commit {commit} again",
            describe(&error)
        );
    }
    Ok(measurement)
}

fn measure_all_languages(
    tree: &Snapshot,
    configuration: &Configuration,
) -> Result<CommitMeasurement, CheckError> {
    let languages = measure_with(tree, &BUILT_IN_LANGUAGE_PLUGINS, configuration)?;
    Ok(CommitMeasurement::of(CodeModel::merge(
        languages.into_iter().map(|language| language.model),
    )))
}
