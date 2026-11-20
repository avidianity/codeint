#![forbid(unsafe_code)]

mod duplication;
mod erosion;
mod line_share;
mod verbosity;

pub use duplication::{CloneGroup, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion, FunctionMasses};
pub use line_share::LineShare;
pub use verbosity::{FlaggedLineFinding, Verbosity};
