use codeint_config::Configuration;
use codeint_git::{CommitId, Repository, ResultCache, Snapshot};
use codeint_model::CodeModel;
use codeint_report::CommitMeasurement;
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

pub(crate) struct CommitMeasurer<'run> {
    repository: &'run Repository,
    configuration: &'run Configuration,
    settings: MeasurementSettings<'run>,
    result_cache: Option<&'run ResultCache>,
}

impl<'run> CommitMeasurer<'run> {
    pub(crate) fn new(
        repository: &'run Repository,
        configuration: &'run Configuration,
        result_cache: Option<&'run ResultCache>,
    ) -> CommitMeasurer<'run> {
        CommitMeasurer {
            repository,
            configuration,
            settings: MeasurementSettings {
                codeint_version: CODEINT_VERSION,
                language_plugins: BUILT_IN_LANGUAGE_PLUGINS
                    .iter()
                    .map(|named| PluginIdentity {
                        name: named.name,
                        version: CODEINT_VERSION,
                    })
                    .collect(),
                configuration,
            },
            result_cache,
        }
    }

    pub(crate) fn measure(&self, commit: CommitId) -> Result<CommitMeasurement, CheckError> {
        if let Some(cached) = self
            .result_cache
            .and_then(|cache| cache.read(commit, &self.settings))
        {
            return Ok(cached);
        }
        let measurement =
            measure_all_languages(&self.repository.commit(commit)?, self.configuration)?;
        if let Some(cache) = self.result_cache
            && let Err(error) = cache.write(commit, &self.settings, &measurement)
        {
            eprintln!(
                "codeint: {}; the next run measures commit {commit} again",
                describe(&error)
            );
        }
        Ok(measurement)
    }
}

pub(crate) fn measure_all_languages(
    tree: &Snapshot,
    configuration: &Configuration,
) -> Result<CommitMeasurement, CheckError> {
    let languages = measure_with(tree, &BUILT_IN_LANGUAGE_PLUGINS, configuration)?;
    Ok(CommitMeasurement::of(CodeModel::merge(
        languages.into_iter().map(|language| language.model),
    )))
}
