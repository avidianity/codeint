use std::path::Path;

use codeint_config::{Configuration, FindingsShown, read_configuration};
use codeint_git::{CommitId, GitError, Repository, Snapshot};
use codeint_model::CodeModel;
use codeint_report::{BaseCommit, DiffReport, Head};

use crate::check::{BUILT_IN_LANGUAGE_PLUGINS, CheckError, measure_with};

pub(crate) struct Refs<'a> {
    pub(crate) base: &'a str,
    pub(crate) head: Option<&'a str>,
}

struct Sides {
    merge_base: CommitId,
    head: Head,
    head_snapshot: Snapshot,
}

pub(crate) fn diff(
    folder: &Path,
    refs: Refs<'_>,
    findings: FindingsShown,
) -> Result<DiffReport, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let Sides {
        merge_base,
        head,
        head_snapshot,
    } = sides(&repository, &refs)?;
    let base_model = measure_all_languages(&repository.commit(merge_base)?, &configuration)?;
    let head_model = measure_all_languages(&head_snapshot, &configuration)?;
    Ok(DiffReport::compare(
        BaseCommit {
            git_ref: refs.base.to_owned(),
            merge_base: merge_base.to_string(),
        },
        head,
        base_model,
        head_model,
        findings,
    ))
}

fn sides(repository: &Repository, refs: &Refs<'_>) -> Result<Sides, GitError> {
    todo!("{:?} {:?}", repository.root(), refs.head)
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
