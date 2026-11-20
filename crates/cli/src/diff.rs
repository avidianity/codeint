use std::path::Path;

use codeint_config::{FindingsShown, read_configuration};
use codeint_git::Repository;
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::LanguagePlugin;
use codeint_report::{BaseCommit, DiffReport};

use crate::check::{CheckError, measure};

pub(crate) fn diff(
    folder: &Path,
    base_ref: &str,
    findings: FindingsShown,
) -> Result<DiffReport, CheckError> {
    let plugins: [&dyn LanguagePlugin; 1] = [&TypeScriptAndJavaScript];
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let merge_base = repository.merge_base_with_head(base_ref)?;
    let base = measure(
        &repository.commit(merge_base)?,
        &plugins,
        &configuration.source_set_overrides,
    )?;
    let head = measure(
        &repository.working_tree()?,
        &plugins,
        &configuration.source_set_overrides,
    )?;
    Ok(DiffReport::compare(
        BaseCommit {
            git_ref: base_ref.to_owned(),
            merge_base: merge_base.to_string(),
        },
        base.model,
        head.model,
        findings,
    ))
}
