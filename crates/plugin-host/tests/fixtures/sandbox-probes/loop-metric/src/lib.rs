#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct LoopMetricPlugin;

export!(LoopMetricPlugin);

impl Guest for LoopMetricPlugin {
    fn key() -> String {
        "loop_metric".to_string()
    }

    fn name() -> String {
        "Loop metric".to_string()
    }

    fn description() -> String {
        "Test Metric plugin that loops forever or exhausts memory".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        if model
            .files
            .iter()
            .any(|file| file.path.contains("hog_metric"))
        {
            let mut chunks: Vec<Vec<u8>> = Vec::new();
            loop {
                let mut chunk = Vec::new();
                chunk.resize(1024 * 1024, 1u8);
                chunks.push(chunk);
                core::hint::black_box(&chunks);
            }
        }
        loop {
            core::hint::spin_loop();
        }
    }
}
