#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    inline: "
        package codeint:plugin@0.1.0;

        interface language {
            record function {
                line: u32,
                end-line: u32,
                decision-points: u32,
                source-lines: u32,
            }

            enum token-kind {
                identifier,
                literal,
                other,
                import-or-export,
            }

            enum source-set {
                production,
                test,
                generated,
                vendored,
            }

            record token {
                kind: token-kind,
                text-hash: u64,
                line: u32,
            }

            record flagged-line {
                line: u32,
                rule: string,
            }

            record file-model {
                source-set: source-set,
                source-lines: u32,
                functions: list<function>,
                tokens: list<token>,
                flagged-lines: list<flagged-line>,
                runtime-imports: list<string>,
            }

            variant resolution {
                resolved(string),
                external,
                unresolved(string),
            }
        }

        interface metric {
            use language.{file-model, resolution};

            record source-file {
                path: string,
                model: file-model,
            }

            record runtime-import {
                importer: string,
                specifier: string,
                resolution: resolution,
            }

            record code-model {
                files: list<source-file>,
                runtime-imports: list<runtime-import>,
                workspace-packages: list<string>,
            }

            record finding {
                id: string,
                path: string,
                line: u32,
                end-line: option<u32>,
                message: string,
            }

            record measurement {
                ratio: f64,
                numerator: f64,
                denominator: f64,
                findings: list<finding>,
            }

            key: func() -> string;
            name: func() -> string;
            description: func() -> string;
            measure: func(model: code-model) -> measurement;
        }

        world metric-plugin {
            export metric;
        }
    ",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct LegacyMetricPlugin;

export!(LegacyMetricPlugin);

impl Guest for LegacyMetricPlugin {
    fn key() -> String {
        "legacy_metric".to_string()
    }

    fn name() -> String {
        "Legacy metric".to_string()
    }

    fn description() -> String {
        "Test Metric plugin targeting codeint:plugin@0.1.0".to_string()
    }

    fn measure(_model: CodeModel) -> Measurement {
        Measurement {
            ratio: 0.0,
            numerator: 0.0,
            denominator: 0.0,
            findings: Vec::new(),
        }
    }
}
