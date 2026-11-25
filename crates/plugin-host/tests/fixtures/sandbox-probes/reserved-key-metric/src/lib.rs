#![no_std]

extern crate alloc;
extern crate probe_runtime;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct ReservedKeyProbe;

export!(ReservedKeyProbe);

impl Guest for ReservedKeyProbe {
    probe_runtime::metric_metadata!(
        "cognitive_complexity",
        "Reserved key",
        "Rejects built-in Metric key reuse",
        1
    );

    fn measure(_model: CodeModel) -> Measurement {
        unreachable!("load must reject the reserved Metric key before measure runs")
    }
}
