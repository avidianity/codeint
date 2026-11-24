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

struct CoverageReaderMetricPlugin;

export!(CoverageReaderMetricPlugin);

impl Guest for CoverageReaderMetricPlugin {
    fn key() -> String {
        "coverage_reader".to_string()
    }

    fn name() -> String {
        "Coverage reader".to_string()
    }

    fn description() -> String {
        "Test Metric plugin that reads per-function coverage facts".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let mut total_listed: u64 = 0;
        let mut total_unexecuted: u64 = 0;
        let mut findings = Vec::new();

        for file in &model.files {
            for function in &file.model.functions {
                let label = function.name.as_deref().unwrap_or("anonymous");
                let (id, message) = match &function.coverage {
                    None => (format!("absent:{label}"), "absent".to_string()),
                    Some(coverage) => {
                        let unexecuted = coverage
                            .listed_lines
                            .saturating_sub(coverage.executed_lines);
                        total_listed += u64::from(coverage.listed_lines);
                        total_unexecuted += u64::from(unexecuted);
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
                findings.push(Finding {
                    id,
                    path: file.path.clone(),
                    line: function.line,
                    end_line: Some(function.end_line),
                    message,
                });
            }
        }

        let ratio = if total_listed == 0 {
            0.0
        } else {
            total_unexecuted as f64 / total_listed as f64
        };

        Measurement {
            ratio,
            numerator: total_unexecuted as f64,
            denominator: total_listed as f64,
            findings,
        }
    }
}
