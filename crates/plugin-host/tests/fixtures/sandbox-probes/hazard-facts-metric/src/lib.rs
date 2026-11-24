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
        let (enabled, mut findings) = probe_runtime::language_fact_findings!(
            &model.languages,
            "hazard-rules",
            "has_hazard_rules",
            "enabled",
            "absent",
            |facts: &self::exports::codeint::plugin::metric::LanguageFacts| facts.has_hazard_rules
        );
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
        Measurement {
            ratio: probe_runtime::ratio(numerator, denominator),
            numerator,
            denominator,
            findings,
        }
    }
}
