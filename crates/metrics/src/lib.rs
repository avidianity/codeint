#![forbid(unsafe_code)]

mod duplication;
mod erosion;
mod verbosity;

pub use duplication::{CloneGroup, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion};
pub use verbosity::{FlaggedLineFinding, Verbosity};
