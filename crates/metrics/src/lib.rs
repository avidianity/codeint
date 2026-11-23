#![forbid(unsafe_code)]

mod boundaries;
mod cognitive_complexity;
mod dependencies;
mod duplication;
mod erosion;
mod finding_id;
mod impact;
mod import_cycles;
mod line_share;
mod orphan_files;
mod trace;
mod verbosity;

pub use crate::orphan_files::{OrphanFileFinding, OrphanFileKind, OrphanFiles};
pub use boundaries::{
    Boundaries, BoundaryEdgeCheck, BoundaryFinding, BoundaryFindingKind, BoundaryPolicy,
};
pub use cognitive_complexity::{CognitiveComplexity, CognitiveComplexityTotals, ComplexFunction};
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
pub use finding_id::FindingId;
pub use impact::{ImpactAnalysis, ImpactDependent, ImpactError, ImpactSummary, ImpactTarget};
pub use import_cycles::{ImportCycle, ImportCycles, ModuleShare, UnresolvedImport};
pub use line_share::LineShare;
pub use trace::{DEFAULT_MAX_TRACE_PATHS, TraceError, TraceResult, trace_imports};
pub use verbosity::{FlaggedLineFinding, Verbosity};
