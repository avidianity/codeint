#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "dual-language-plugin",
    inline: "
        package codeint:dual@0.3.0;

        world dual-language-plugin {
            export codeint:plugin/language@0.2.0;
            export codeint:plugin/language@0.3.0;
        }

        package codeint:plugin@0.2.0 {
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
        }

        package codeint:plugin@0.3.0 {
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

                record file-model {
                    source-set: source-set,
                    source-lines: u32,
                    functions: list<function>,
                    tokens: list<token>,
                    flagged-lines: list<flagged-line>,
                    runtime-imports: list<string>,
                    program-start: bool,
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
        }
    ",
});

struct DualVersionLanguagePlugin;

export!(DualVersionLanguagePlugin);

macro_rules! impl_common_guest {
    ($pkg:ident $(, program_start: $program_start:expr)?) => {
        fn extensions() -> Vec<String> {
            vec!["dual".to_string()]
        }

        fn lower(
            _path: String,
            text: String,
        ) -> Result<
            exports::codeint::$pkg::language::FileModel,
            exports::codeint::$pkg::language::LowerError,
        > {
            Ok(exports::codeint::$pkg::language::FileModel {
                source_set: exports::codeint::$pkg::language::SourceSet::Production,
                source_lines: text.lines().count() as u32,
                functions: vec![],
                tokens: vec![],
                flagged_lines: vec![],
                runtime_imports: vec![],
                $(program_start: $program_start,)?
            })
        }

        fn manifest_paths(_paths: Vec<String>) -> Vec<String> {
            vec![]
        }

        fn workspace_packages(
            _manifests: Vec<exports::codeint::$pkg::language::Manifest>,
        ) -> Result<Vec<String>, exports::codeint::$pkg::language::WorkspaceError> {
            Ok(vec![])
        }

        fn resolve_imports(
            _paths: Vec<String>,
            _manifests: Vec<exports::codeint::$pkg::language::Manifest>,
            _requests: Vec<exports::codeint::$pkg::language::ImportRequest>,
        ) -> Vec<exports::codeint::$pkg::language::Resolution> {
            vec![]
        }

        fn declared_dependencies(
            _manifests: Vec<exports::codeint::$pkg::language::Manifest>,
        ) -> Vec<exports::codeint::$pkg::language::DeclaredDep> {
            vec![exports::codeint::$pkg::language::DeclaredDep {
                package_root: String::new(),
                name: "dual-dep".to_string(),
                kind: exports::codeint::$pkg::language::DepKind::Runtime,
            }]
        }

        fn entry_points(
            repository_paths: Vec<String>,
            _manifests: Vec<exports::codeint::$pkg::language::Manifest>,
        ) -> Vec<String> {
            repository_paths.into_iter().take(1).collect()
        }

        fn reference_ranges() -> Vec<exports::codeint::$pkg::language::ReferenceRange> {
            vec![]
        }
    };
}

impl exports::codeint::plugin0_2_0::language::Guest for DualVersionLanguagePlugin {
    impl_common_guest!(plugin0_2_0);
}

impl exports::codeint::plugin0_3_0::language::Guest for DualVersionLanguagePlugin {
    impl_common_guest!(plugin0_3_0, program_start: false);
}
