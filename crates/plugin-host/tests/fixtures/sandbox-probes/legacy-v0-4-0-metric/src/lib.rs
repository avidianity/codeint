#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::String;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "metric-plugin",
    inline: "
        package codeint:plugin@0.4.0;

        interface language {
            record function-coverage {
                listed-lines: u32,
                executed-lines: u32,
            }

            record function {
                line: u32,
                end-line: u32,
                decision-points: u32,
                source-lines: u32,
                name: option<string>,
                cognitive-complexity: u32,
                coverage: option<function-coverage>,
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

            record hazard-line {
                line: u32,
                rule: string,
            }

            record file-model {
                source-set: source-set,
                source-lines: u32,
                functions: list<function>,
                tokens: list<token>,
                flagged-lines: list<flagged-line>,
                hazard-lines: list<hazard-line>,
                runtime-imports: list<string>,
                program-start: bool,
            }

            variant resolution {
                resolved(list<string>),
                external(string),
                builtin,
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

            record language-facts {
                language: string,
                has-coverage: bool,
                has-hazard-rules: bool,
                has-imports: bool,
            }

            record code-model {
                files: list<source-file>,
                runtime-imports: list<runtime-import>,
                workspace-packages: list<string>,
                languages: list<language-facts>,
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
            version: func() -> u32;
            measure: func(model: code-model) -> measurement;
        }

        world metric-plugin {
            export metric;
        }
    ",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

struct LegacyV040MetricPlugin;

export!(LegacyV040MetricPlugin);

impl Guest for LegacyV040MetricPlugin {
    fn key() -> String {
        String::from("legacy_v0_4_0_metric")
    }

    fn name() -> String {
        String::from("Legacy 0.4.0 metric")
    }

    fn description() -> String {
        String::from("Test Metric plugin targeting codeint:plugin@0.4.0")
    }

    fn version() -> u32 {
        1
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
