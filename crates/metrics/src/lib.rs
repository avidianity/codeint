#![forbid(unsafe_code)]

mod duplication;
mod erosion;
mod finding_id;
mod import_cycles;
mod verbosity;

pub use duplication::{CloneGroup, CloneLines, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion};
pub use finding_id::FindingId;
pub use import_cycles::{ImportCycle, ImportCycles, UnresolvedImport};
pub use verbosity::{FlaggedLineFinding, Verbosity};
