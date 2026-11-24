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

use codeint::plugin::language::Resolution;
use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct ImportsReaderMetricPlugin;

export!(ImportsReaderMetricPlugin);

impl Guest for ImportsReaderMetricPlugin {
    fn key() -> String {
        "resolved_links".to_string()
    }

    fn name() -> String {
        "Resolved links".to_string()
    }

    fn description() -> String {
        "Test Metric plugin that reads multi-file resolved runtime imports".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let mut findings = Vec::new();
        let mut resolved_imports: u64 = 0;
        let mut multi_target_imports: u64 = 0;

        for import in &model.runtime_imports {
            if let Resolution::Resolved(targets) = &import.resolution {
                resolved_imports += 1;
                if targets.len() > 1 {
                    multi_target_imports += 1;
                }
                findings.push(Finding {
                    id: format!(
                        "{}:{}:{}",
                        import.importer,
                        import.specifier,
                        targets.join(",")
                    ),
                    path: import.importer.clone(),
                    line: 1,
                    end_line: None,
                    message: format!("{} -> [{}]", import.specifier, targets.join(", ")),
                });
            }
        }

        let ratio = if resolved_imports == 0 {
            0.0
        } else {
            multi_target_imports as f64 / resolved_imports as f64
        };

        Measurement {
            ratio,
            numerator: multi_target_imports as f64,
            denominator: resolved_imports as f64,
            findings,
        }
    }
}
