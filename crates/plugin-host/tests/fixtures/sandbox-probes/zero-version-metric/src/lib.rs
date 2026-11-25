#![no_std]

extern crate alloc;
extern crate probe_runtime;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct ZeroVersionMetricProbe;

export!(ZeroVersionMetricProbe);

impl Guest for ZeroVersionMetricProbe {
    probe_runtime::metric_metadata!(
        "zero_version_metric",
        "Zero version metric",
        "Rejects non-positive Metric version 0 at load time",
        0
    );

    fn measure(_model: CodeModel) -> Measurement {
        unreachable!("load must reject Metric version 0 before measure runs")
    }
}
