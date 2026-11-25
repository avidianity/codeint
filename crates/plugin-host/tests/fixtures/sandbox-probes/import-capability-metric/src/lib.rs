#![no_std]

extern crate alloc;
extern crate probe_runtime;

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct ImportCapabilityMetricPlugin;

export!(ImportCapabilityMetricPlugin);

impl Guest for ImportCapabilityMetricPlugin {
    probe_runtime::metric_metadata!(
        "import_capability",
        "Import capability",
        "Test Metric plugin that reads per-language import capability",
        1
    );

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
