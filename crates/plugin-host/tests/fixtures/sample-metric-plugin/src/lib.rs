#![forbid(unsafe_code)]
#![cfg_attr(
    all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"),
    no_std
)]

use codeint_plugin_sdk::Fnv1a64;
use codeint_plugin_sdk::metric::{CodeModel, Finding, Measurement, MetricPlugin};
use codeint_plugin_sdk::prelude::*;

const MAX_FUNCTION_SOURCE_LINES: u32 = 10;

struct SampleLongFunctionsPlugin;

codeint_plugin_sdk::export_metric_plugin!(SampleLongFunctionsPlugin);

impl MetricPlugin for SampleLongFunctionsPlugin {
    fn key() -> String {
        "long_functions".to_string()
    }

    fn name() -> String {
        "Long functions".to_string()
    }

    fn description() -> String {
        "Share of functions with more than 10 source lines".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let mut total_functions: u64 = 0;
        let mut long_functions: u64 = 0;
        let mut occurrences: BTreeMap<u64, u64> = BTreeMap::new();
        let mut scored_findings: Vec<(u32, Finding)> = Vec::new();

        for file in &model.files {
            for function in &file.model.functions {
                total_functions += 1;
                if function.source_lines <= MAX_FUNCTION_SOURCE_LINES {
                    continue;
                }
                long_functions += 1;
                let mut hasher = Fnv1a64::new();
                hasher.update(b"long_functions");
                hasher.update(file.path.as_bytes());
                for token in &file.model.tokens {
                    if token.line >= function.line && token.line <= function.end_line {
                        hasher.update_u64(token.text_hash);
                    }
                }
                let base_hash = hasher.finish();
                let count = occurrences.entry(base_hash).or_insert(0);
                let mut id_hasher = hasher;
                id_hasher.update_u64(*count);
                let id_hash = id_hasher.finish();
                *count += 1;
                scored_findings.push((
                    function.source_lines,
                    Finding {
                        id: format!("{id_hash:016x}"),
                        path: file.path.clone(),
                        line: function.line,
                        end_line: Some(function.end_line),
                        message: format!(
                            "function has {} source lines, over {MAX_FUNCTION_SOURCE_LINES}",
                            function.source_lines
                        ),
                    },
                ));
            }
        }

        scored_findings.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.path.cmp(&right.1.path))
                .then_with(|| left.1.line.cmp(&right.1.line))
        });

        let findings = scored_findings
            .into_iter()
            .map(|(_, finding)| finding)
            .collect();
        Measurement::from_counts(long_functions as f64, total_functions as f64, findings)
    }
}
