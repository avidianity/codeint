use std::path::Path;

use codeint_config::{Configuration, FindingsShown, read_configuration};
use codeint_git::{CommitId, GitError, Repository, ResultCache, Snapshot};
use codeint_model::CodeModel;
use codeint_report::{BaseCommit, CommitMeasurement, DiffReport, Head, HeadCommit};
use serde::Serialize;

use crate::check::{BUILT_IN_LANGUAGE_PLUGINS, CheckError, measure_with};
use crate::describe;

const CODEINT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct Refs<'a> {
    pub(crate) base: &'a str,
    pub(crate) head: Option<&'a str>,
}

struct Sides {
    merge_base: CommitId,
    head: Head,
    head_commit: Option<CommitId>,
}

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

struct CommitMeasurer<'diff> {
    repository: &'diff Repository,
    configuration: &'diff Configuration,
    settings: MeasurementSettings<'diff>,
    result_cache: Option<&'diff ResultCache>,
}

pub(crate) fn diff(
    folder: &Path,
    refs: Refs<'_>,
    findings: FindingsShown,
    result_cache: Option<&ResultCache>,
) -> Result<DiffReport, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let Sides {
        merge_base,
        head,
        head_commit,
    } = sides(&repository, &refs)?;
    let measurer = CommitMeasurer::new(&repository, &configuration, result_cache);
    let base_measurement = measurer.measure(merge_base)?;
    let head_measurement = match head_commit {
        Some(commit) => measurer.measure(commit)?,
        None => measure_all_languages(&repository.working_tree()?, &configuration)?,
    };
    Ok(DiffReport::compare(
        BaseCommit {
            git_ref: refs.base.to_owned(),
            merge_base: merge_base.to_string(),
        },
        head,
        base_measurement,
        head_measurement,
        findings,
        &configuration.budgets,
        &configuration.regression_limits,
    ))
}

fn sides(repository: &Repository, refs: &Refs<'_>) -> Result<Sides, GitError> {
    match refs.head {
        None => Ok(Sides {
            merge_base: repository.merge_base_with_head(refs.base)?,
            head: Head::WorkingTree,
            head_commit: None,
        }),
        Some(head_ref) => {
            let merge_base = repository.merge_base(refs.base, head_ref)?;
            let head_commit = repository.resolve(head_ref)?;
            Ok(Sides {
                merge_base,
                head: Head::Commit(HeadCommit {
                    git_ref: head_ref.to_owned(),
                    commit: head_commit.to_string(),
                }),
                head_commit: Some(head_commit),
            })
        }
    }
}

impl<'diff> CommitMeasurer<'diff> {
    fn new(
        repository: &'diff Repository,
        configuration: &'diff Configuration,
        result_cache: Option<&'diff ResultCache>,
    ) -> CommitMeasurer<'diff> {
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

    fn measure(&self, commit: CommitId) -> Result<CommitMeasurement, CheckError> {
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

fn measure_all_languages(
    tree: &Snapshot,
    configuration: &Configuration,
) -> Result<CommitMeasurement, CheckError> {
    let languages = measure_with(tree, &BUILT_IN_LANGUAGE_PLUGINS, configuration)?;
    Ok(CommitMeasurement::of(CodeModel::merge(
        languages.into_iter().map(|language| language.model),
    )))
}
