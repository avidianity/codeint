#![forbid(unsafe_code)]

mod duplication;
mod erosion;
mod import_cycles;
mod verbosity;

pub use duplication::{CloneGroup, CloneLines, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion};
pub use import_cycles::{ImportCycle, ImportCycles, UnresolvedImport};
pub use verbosity::{FlaggedLineFinding, Verbosity};
