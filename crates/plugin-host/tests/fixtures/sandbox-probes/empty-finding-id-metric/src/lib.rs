#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::ToString;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct Probe;

export!(Probe);

impl Guest for Probe {
    probe_runtime::metric_metadata!(
        "empty_finding_id_metric",
        "Empty finding id metric",
        "Returns one Finding per function with an empty id and line 0 so Conformance rejects it",
        1
    );

    fn measure(model: CodeModel) -> Measurement {
        let mut findings = Vec::new();
        for file in &model.files {
            for _function in &file.model.functions {
                findings.push(Finding {
                    id: "".to_string(),
                    path: file.path.clone(),
                    line: 0,
                    end_line: None,
                    message: "function".to_string(),
                });
            }
        }
        Measurement {
            ratio: 0.5,
            numerator: 1.0,
            denominator: 2.0,
            findings,
        }
    }
}
