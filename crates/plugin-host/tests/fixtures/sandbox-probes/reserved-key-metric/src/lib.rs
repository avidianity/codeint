#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::String;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct ReservedKeyProbe;

export!(ReservedKeyProbe);

impl Guest for ReservedKeyProbe {
    fn description() -> String {
        String::from("Rejects built-in Metric key reuse")
    }

    fn name() -> String {
        String::from("Reserved key")
    }

    fn key() -> String {
        String::from("cognitive_complexity")
    }

    fn measure(_model: CodeModel) -> Measurement {
        unreachable!("load must reject the reserved Metric key before measure runs")
    }
}
