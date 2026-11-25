#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct LoopMetricPlugin;

export!(LoopMetricPlugin);

impl Guest for LoopMetricPlugin {
    probe_runtime::metric_metadata!(
        "loop_metric",
        "Loop metric",
        "Test Metric plugin that loops forever or exhausts memory",
        1
    );

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
