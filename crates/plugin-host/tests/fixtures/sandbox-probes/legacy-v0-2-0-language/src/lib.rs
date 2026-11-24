#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    inline: "
        package codeint:plugin@0.2.0;

        interface language {
            record function {
                line: u32,
                end-line: u32,
                decision-points: u32,
                source-lines: u32,
                name: option<string>,
                cognitive-complexity: u32,
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

            record import-request {
                importer: string,
                specifier: string,
            }

            variant resolution {
                resolved(string),
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
        }

        world language-plugin {
            export language;
        }
    ",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    SourceSet, WorkspaceError,
};

struct LegacyV020LanguagePlugin;

export!(LegacyV020LanguagePlugin);

impl Guest for LegacyV020LanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["legacy02".to_string()]
    }

    fn lower(_path: String, _text: String) -> Result<FileModel, LowerError> {
        Ok(FileModel {
            source_set: SourceSet::Production,
            source_lines: 0,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        })
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }
}
