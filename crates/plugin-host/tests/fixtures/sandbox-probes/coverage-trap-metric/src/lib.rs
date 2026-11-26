#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct CoverageTrapMetricPlugin;

export!(CoverageTrapMetricPlugin);

impl Guest for CoverageTrapMetricPlugin {
    probe_runtime::metric_metadata!(
        "coverage_trap",
        "Coverage trap",
        "Test Metric plugin that traps on a function with Coverage facts",
        1
    );

    fn measure(model: CodeModel) -> Measurement {
        for file in &model.files {
            for function in &file.model.functions {
                if function.coverage.is_some() {
                    panic!("measure trapped on function coverage");
                }
            }
        }
        Measurement {
            ratio: 0.0,
            numerator: 0.0,
            denominator: 0.0,
            findings: Vec::new(),
        }
    }
}
