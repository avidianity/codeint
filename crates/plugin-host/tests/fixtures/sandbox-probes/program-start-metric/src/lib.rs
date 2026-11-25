#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct ProgramStartMetricPlugin;

export!(ProgramStartMetricPlugin);

impl Guest for ProgramStartMetricPlugin {
    probe_runtime::metric_metadata!(
        "program_starts",
        "Program starts",
        "Test Metric plugin that reads program-start marks",
        1
    );

    fn measure(model: CodeModel) -> Measurement {
        let mut findings = Vec::new();
        for file in &model.files {
            if file.model.program_start {
                findings.push(Finding {
                    id: format!("program-start:{}", file.path),
                    path: file.path.clone(),
                    line: 1,
                    end_line: None,
                    message: "program start".to_string(),
                });
            }
        }
        let numerator = findings.len() as f64;
        let denominator = model.files.len() as f64;
        let ratio = if model.files.is_empty() {
            0.0
        } else {
            numerator / denominator
        };
        Measurement {
            ratio,
            numerator,
            denominator,
            findings,
        }
    }
}
