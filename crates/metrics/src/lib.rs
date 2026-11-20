#![forbid(unsafe_code)]

mod duplication;
mod erosion;
mod finding_id;
mod import_cycles;
mod line_share;
mod verbosity;

pub use duplication::{CloneGroup, CloneLines, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion, FunctionMasses};
pub use finding_id::FindingId;
pub use import_cycles::{ImportCycle, ImportCycles, ModuleShare, UnresolvedImport};
pub use line_share::LineShare;
pub use verbosity::{FlaggedLineFinding, Verbosity};
