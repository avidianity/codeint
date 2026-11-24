#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct ImportCapabilityMetricPlugin;

export!(ImportCapabilityMetricPlugin);

impl Guest for ImportCapabilityMetricPlugin {
    fn key() -> String {
        "import_capability".to_string()
    }

    fn name() -> String {
        "Import capability".to_string()
    }

    fn description() -> String {
        "Test Metric plugin that reads per-language import capability".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let mut with_imports: usize = 0;
        let mut findings = Vec::new();
        for facts in &model.languages {
            let status = if facts.has_imports {
                with_imports += 1;
                "imports"
            } else {
                "no-imports"
            };
            findings.push(Finding {
                id: format!("imports:{}:{status}", facts.language),
                path: format!("languages/{}", facts.language),
                line: 1,
                end_line: None,
                message: format!(
                    "language={} has_imports={}",
                    facts.language, facts.has_imports
                ),
            });
        }
        let numerator = with_imports as f64;
        let denominator = model.languages.len() as f64;
        let ratio = if model.languages.is_empty() {
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
