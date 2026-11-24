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

struct HazardFactsMetricPlugin;

export!(HazardFactsMetricPlugin);

impl Guest for HazardFactsMetricPlugin {
    fn key() -> String {
        "hazard_facts".to_string()
    }

    fn name() -> String {
        "Hazard facts".to_string()
    }

    fn description() -> String {
        "Test Metric plugin that reads per-language hazard rule capability".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let mut enabled: usize = 0;
        let mut findings = Vec::new();
        for facts in &model.languages {
            let status = if facts.has_hazard_rules {
                enabled += 1;
                "enabled"
            } else {
                "absent"
            };
            findings.push(Finding {
                id: format!("hazard-rules:{}:{status}", facts.language),
                path: format!("languages/{}", facts.language),
                line: 1,
                end_line: None,
                message: format!(
                    "language={} has_hazard_rules={}",
                    facts.language, facts.has_hazard_rules
                ),
            });
        }
        for file in &model.files {
            for hazard in &file.model.hazard_lines {
                findings.push(Finding {
                    id: format!("hazard:{}:{}:{}", file.path, hazard.line, hazard.rule),
                    path: file.path.clone(),
                    line: hazard.line,
                    end_line: None,
                    message: format!("hazard rule={}", hazard.rule),
                });
            }
        }
        let numerator = enabled as f64;
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
