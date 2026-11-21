pub(crate) mod corpus;

use std::path::Path;

use crate::checks::Violations;
use crate::config::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PerfVerdict {
    Within,
    OverBudget,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MeasuredRun {
    pub(crate) files: usize,
    pub(crate) source_lines: u64,
    pub(crate) elapsed_seconds: f64,
}

pub(crate) fn check_budget(elapsed_seconds: f64, budget_seconds: f64) -> PerfVerdict {
    let _ = (elapsed_seconds, budget_seconds);
    todo!()
}

pub(crate) fn over_budget_message(run: MeasuredRun, budget_seconds: f64) -> String {
    let _ = (
        run.files,
        run.source_lines,
        run.elapsed_seconds,
        budget_seconds,
    );
    todo!()
}

pub(crate) fn run_perf_check(
    root: &Path,
    settings: &Settings,
    binary_override: Option<&Path>,
) -> Result<Violations, String> {
    let spec = corpus::CorpusSpec::from_budget(settings.performance);
    let _ = (
        root,
        binary_override,
        crate::config::temp_dir(),
        corpus::write_corpus,
        spec,
        check_budget,
        over_budget_message,
        PerfVerdict::Within,
        PerfVerdict::OverBudget,
        MeasuredRun {
            files: 0,
            source_lines: 0,
            elapsed_seconds: 0.0,
        },
    );
    todo!()
}
