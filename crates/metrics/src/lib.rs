#![forbid(unsafe_code)]

mod duplication;
mod erosion;
mod import_cycles;
mod line_share;
mod verbosity;

pub use duplication::{CloneGroup, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion, FunctionMasses};
pub use import_cycles::{ImportCycle, ImportCycles, ModuleShare};
pub use line_share::LineShare;
pub use verbosity::{FlaggedLineFinding, Verbosity};
