#![forbid(unsafe_code)]

mod boundaries;
mod cognitive_complexity;
mod coverage;
mod dependencies;
mod duplication;
mod erosion;
mod finding_id;
mod hazards;
mod impact;
mod import_cycles;
mod line_share;
mod orphan_files;
mod test_gap;
mod trace;
mod verbosity;

pub use crate::orphan_files::{OrphanFileFinding, OrphanFileKind, OrphanFiles};
pub use boundaries::{
    Boundaries, BoundaryEdgeCheck, BoundaryFinding, BoundaryFindingKind, BoundaryPolicy,
};
pub use cognitive_complexity::{CognitiveComplexity, CognitiveComplexityTotals, ComplexFunction};
pub use coverage::{
    CoverageApplyError, CoverageApplyOutcome, CoverageFileRecord, CoverageFormat, CoverageLimits,
    CoverageParseError, MAX_COVERAGE_REPORT_BYTES, MAX_XML_NESTING_DEPTH, NamedCoverageReport,
    ParsedCoverageReport, PathMatcher, apply_coverage_reports, match_coverage_path,
    parse_cobertura, parse_cobertura_with_limits, parse_coverage_report,
    parse_coverage_report_with_limits, parse_lcov, parse_lcov_with_limits,
};
pub use dependencies::{
    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY, DependencyHygiene, DevInProductionDependencyFinding,
    UNLISTED_DEPENDENCIES_METRIC_KEY, UNUSED_DEPENDENCIES_METRIC_KEY, UnlistedDependencyFinding,
    UnusedDependencyFinding, UnusedDependencyKind,
};
pub use duplication::{
    CloneFamily, CloneGroup, CloneLines, Duplication, Fragment, MinimumCloneTokens,
    MinimumDistinctTokens, NearCloneConfig,
};
pub use erosion::{ErodedFunction, Erosion, FunctionMasses};
pub use finding_id::{FindingContext, FindingId};
pub use hazards::{HAZARDS_METRIC_KEY, HazardLineFinding, HazardLines, Hazards};
pub use impact::{ImpactAnalysis, ImpactDependent, ImpactError, ImpactSummary, ImpactTarget};
pub use import_cycles::{ImportCycle, ImportCycles, ModuleShare, UnresolvedImport};
pub use line_share::LineShare;
pub use test_gap::{TEST_GAP_METRIC_KEY, TestGap, TestGapFinding, TestGapMasses};
pub use trace::{DEFAULT_MAX_TRACE_PATHS, MultiFileImport, TraceError, TraceResult, trace_imports};
pub use verbosity::{FlaggedLineFinding, Verbosity};

pub(crate) fn build_resolved_import_adjacency(
    model: &codeint_model::CodeModel,
    node_count: usize,
    index_of: impl Fn(&codeint_model::RepoPath) -> Option<usize>,
) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); node_count];
    for runtime_import in &model.runtime_imports {
        if let codeint_model::ImportResolution::Resolved(targets) = &runtime_import.resolution
            && let Some(from) = index_of(&runtime_import.importer)
        {
            for target in targets {
                if let Some(to) = index_of(target) {
                    adjacency[from].push(to);
                }
            }
        }
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    adjacency
}

#[cfg(test)]
pub(crate) mod test_support {
    use codeint_model::{FileModel, RepoPath, SourceFile, SourceSet};

    pub(crate) fn repo_path(text: &str) -> RepoPath {
        RepoPath::new(text).expect("valid repo path")
    }

    pub(crate) fn empty_source_file(
        name: &str,
        source_set: SourceSet,
        source_lines: u32,
    ) -> SourceFile {
        SourceFile {
            path: repo_path(name),
            model: FileModel {
                source_set,
                source_lines,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
                is_tooling_file: false,
                diagnostics: Vec::new(),
                scope_artifact: None,
            },
        }
    }
}
