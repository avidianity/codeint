#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::ToString;
use alloc::vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct Probe;

export!(Probe);

impl Guest for Probe {
    probe_runtime::metric_metadata!("invalid_finding_path_metric", "Invalid finding path metric", "Returns a Finding with a parent-directory path so Conformance rejects it", 1);

    fn measure(_model: CodeModel) -> Measurement {
        Measurement {
            ratio: 0.5,
            numerator: 1.0,
            denominator: 2.0,
            findings: vec![Finding {
                id: "bad-path".to_string(),
                path: "../outside.rs".to_string(),
                line: 1,
                end_line: None,
                message: "escapes the repository".to_string(),
            }],
        }
    }
}
