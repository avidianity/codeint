#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    inline: "
        package codeint:plugin@0.3.0;

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

            record import-request {
                importer: string,
                specifier: string,
            }

            variant resolution {
                resolved(list<string>),
                external(string),
                builtin,
                unresolved(string),
            }

            enum dep-kind {
                runtime,
                dev,
                optional,
            }

            record declared-dep {
                package-root: string,
                name: string,
                kind: dep-kind,
            }

            variant lower-error {
                unparsable(string),
                unsupported-file(string),
                too-large,
                declaration-only,
            }

            record manifest {
                path: string,
                text: string,
            }

            record workspace-error {
                path: string,
                message: string,
            }

            record reference-range {
                metric: string,
                low: f64,
                high: f64,
                source: string,
            }

            extensions: func() -> list<string>;
            lower: func(path: string, text: string) -> result<file-model, lower-error>;
            manifest-paths: func(repository-paths: list<string>) -> list<string>;
            workspace-packages: func(manifests: list<manifest>) -> result<list<string>, workspace-error>;
            resolve-imports: func(repository-paths: list<string>, manifests: list<manifest>, requests: list<import-request>) -> list<resolution>;
            declared-dependencies: func(manifests: list<manifest>) -> list<declared-dep>;
            entry-points: func(repository-paths: list<string>, manifests: list<manifest>) -> list<string>;
            reference-ranges: func() -> list<reference-range>;
            has-hazard-rules: func() -> bool;
            hazard-rules: func() -> list<string>;
            has-imports: func() -> bool;
        }

        world language-plugin {
            export language;
        }
    ",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    WorkspaceError,
};

struct LegacyV030LanguagePlugin;

export!(LegacyV030LanguagePlugin);

impl Guest for LegacyV030LanguagePlugin {
    fn extensions() -> Vec<String> {
        vec![String::from("legacy03")]
    }

    fn lower(_path: String, _text: String) -> Result<FileModel, LowerError> {
        Err(LowerError::DeclarationOnly)
    }

    probe_runtime::legacy_language_metadata_stubs!();

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
    }
}
