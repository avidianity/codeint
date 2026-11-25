#![forbid(unsafe_code)]

use alloc::string::String;
use alloc::vec::Vec;

pub use crate::runtime::__bindings::metric::codeint::plugin::language::{
    FileModel, FlaggedLine, Function, FunctionCoverage, HazardLine, Resolution, SourceSet, Token,
    TokenKind,
};
pub use crate::runtime::__bindings::metric::exports::codeint::plugin::metric::{
    CodeModel, Finding, LanguageFacts, Measurement, RuntimeImport, SourceFile,
};

impl Measurement {
    pub fn empty() -> Measurement {
        Measurement {
            ratio: 0.0,
            numerator: 0.0,
            denominator: 0.0,
            findings: Vec::new(),
        }
    }

    pub fn from_counts(numerator: f64, denominator: f64, findings: Vec<Finding>) -> Measurement {
        let ratio = if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        };
        Measurement {
            ratio,
            numerator,
            denominator,
            findings,
        }
    }
}

pub trait MetricPlugin {
    fn key() -> String;

    fn name() -> String;

    fn description() -> String;

    fn version() -> u32 {
        1
    }

    fn measure(model: CodeModel) -> Measurement;
}
