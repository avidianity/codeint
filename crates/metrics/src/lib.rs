#![forbid(unsafe_code)]

mod erosion;
mod verbosity;

pub use erosion::{ErodedFunction, Erosion};
pub use verbosity::{FlaggedLineFinding, Verbosity};
