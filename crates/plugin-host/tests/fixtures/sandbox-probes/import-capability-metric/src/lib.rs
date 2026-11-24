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
        let (with_imports, findings) = probe_runtime::language_fact_findings!(
            &model.languages,
            "imports",
            "has_imports",
            "imports",
            "no-imports",
            |facts: &self::exports::codeint::plugin::metric::LanguageFacts| facts.has_imports
        );
        let numerator = with_imports as f64;
        let denominator = model.languages.len() as f64;
        Measurement {
            ratio: probe_runtime::ratio(numerator, denominator),
            numerator,
            denominator,
            findings,
        }
    }
}
