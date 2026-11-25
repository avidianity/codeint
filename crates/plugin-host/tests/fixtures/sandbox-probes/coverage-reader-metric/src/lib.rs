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

use self::codeint::plugin::language::Function;
use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct CoverageReaderMetricPlugin;

export!(CoverageReaderMetricPlugin);

fn finding_for_function(
    file_path: &str,
    function: &Function,
    total_listed: &mut u64,
    total_unexecuted: &mut u64,
) -> Finding {
    let label = function.name.as_deref().unwrap_or("anonymous");
    let (id, message) = match &function.coverage {
        None => (format!("absent:{label}"), "absent".to_string()),
        Some(coverage) => {
            let unexecuted = coverage
                .listed_lines
                .saturating_sub(coverage.executed_lines);
            *total_listed += u64::from(coverage.listed_lines);
            *total_unexecuted += u64::from(unexecuted);
            let prefix = if coverage.executed_lines == 0 {
                "zero"
            } else {
                "covered"
            };
            (
                format!("{prefix}:{label}"),
                format!(
                    "listed={} executed={}",
                    coverage.listed_lines, coverage.executed_lines
                ),
            )
        }
    };
    Finding {
        id,
        path: file_path.to_string(),
        line: function.line,
        end_line: Some(function.end_line),
        message,
    }
}

impl Guest for CoverageReaderMetricPlugin {
    probe_runtime::metric_metadata!(
        "coverage_reader",
        "Coverage reader",
        "Test Metric plugin that reads per-function and per-language coverage facts",
        1
    );

    fn measure(model: CodeModel) -> Measurement {
        let mut total_listed: u64 = 0;
        let mut total_unexecuted: u64 = 0;
        let mut findings = Vec::new();
        for file in &model.files {
            for function in &file.model.functions {
                findings.push(finding_for_function(
                    &file.path,
                    function,
                    &mut total_listed,
                    &mut total_unexecuted,
                ));
            }
        }
        let (_, language_findings) = probe_runtime::language_fact_findings!(
            &model.languages,
            "language",
            "has_coverage",
            "covered",
            "absent",
            |facts: &self::exports::codeint::plugin::metric::LanguageFacts| facts.has_coverage
        );
        findings.extend(language_findings);
        let numerator = total_unexecuted as f64;
        let denominator = total_listed as f64;
        Measurement {
            ratio: probe_runtime::ratio(numerator, denominator),
            numerator,
            denominator,
            findings,
        }
    }
}
