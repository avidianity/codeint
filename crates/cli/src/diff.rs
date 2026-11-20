use std::path::Path;

use codeint_config::{Configuration, FindingsShown, read_configuration};
use codeint_git::{Repository, Snapshot};
use codeint_model::CodeModel;
use codeint_report::{BaseCommit, DiffReport};

use crate::check::{BUILT_IN_LANGUAGE_PLUGINS, CheckError, measure_with};

pub(crate) fn diff(
    folder: &Path,
    base_ref: &str,
    findings: FindingsShown,
) -> Result<DiffReport, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let merge_base = repository.merge_base_with_head(base_ref)?;
    let base = measure_all_languages(&repository.commit(merge_base)?, &configuration)?;
    let head = measure_all_languages(&repository.working_tree()?, &configuration)?;
    Ok(DiffReport::compare(
        BaseCommit {
            git_ref: base_ref.to_owned(),
            merge_base: merge_base.to_string(),
        },
        base,
        head,
        findings,
        &configuration.budgets,
        &configuration.regression_limits,
    ))
}

fn measure_all_languages(
    tree: &Snapshot,
    configuration: &Configuration,
) -> Result<CodeModel, CheckError> {
    let languages = measure_with(tree, &BUILT_IN_LANGUAGE_PLUGINS, configuration)?;
    Ok(CodeModel::merge(
        languages.into_iter().map(|language| language.model),
    ))
}
