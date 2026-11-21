use std::path::Path;

use crate::config::PerformanceBudget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CorpusSpec {
    pub(crate) seed: u64,
    pub(crate) packages: usize,
    pub(crate) files_per_package: usize,
    pub(crate) functions_per_file: usize,
}

impl CorpusSpec {
    pub(crate) fn from_budget(budget: PerformanceBudget) -> CorpusSpec {
        CorpusSpec {
            seed: budget.seed,
            packages: budget.packages,
            files_per_package: budget.files_per_package,
            functions_per_file: budget.functions_per_file,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CorpusSummary {
    pub(crate) files: usize,
    pub(crate) lines: u64,
}

pub(crate) fn write_corpus(root: &Path, spec: CorpusSpec) -> Result<CorpusSummary, String> {
    let _ = (
        root,
        spec.seed,
        spec.packages,
        spec.files_per_package,
        spec.functions_per_file,
        CorpusSummary { files: 0, lines: 0 },
    );
    todo!()
}
