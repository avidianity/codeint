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
    probe_runtime::metric_metadata!("out_of_range_ratio_metric", "Out of range ratio metric", "Returns a ratio above 1 so Conformance rejects it", 1);

    fn measure(_model: CodeModel) -> Measurement {
        Measurement {
            ratio: 1.5,
            numerator: 3.0,
            denominator: 2.0,
            findings: vec![],
        }
    }
}
