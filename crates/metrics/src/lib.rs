#![forbid(unsafe_code)]

mod duplication;
mod erosion;

pub use duplication::{CloneGroup, Duplication, Fragment, MinimumCloneTokens};
pub use erosion::{ErodedFunction, Erosion};
