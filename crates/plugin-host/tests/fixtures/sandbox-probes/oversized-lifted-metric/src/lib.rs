#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::String;
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
        "oversized_lifted_metric",
        "Oversized lifted metric",
        "Returns enough findings to exceed host lifted-size budget while staying within guest linear memory",
        1
    );

    fn measure(_model: CodeModel) -> Measurement {
        let count = 24_000usize;
        let mut findings = Vec::with_capacity(count);
        for _ in 0..count {
            findings.push(Finding {
                id: String::new(),
                path: String::new(),
                line: 1,
                end_line: None,
                message: String::new(),
            });
        }
        Measurement {
            ratio: 0.5,
            numerator: 1.0,
            denominator: 2.0,
            findings,
        }
    }
}
