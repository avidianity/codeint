#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "dual-language-plugin",
    inline: "
        package codeint:dual@0.4.0;

        world dual-language-plugin {
            export codeint:plugin/language@0.3.0;
            export codeint:plugin/language@0.4.0;
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
        }

        package codeint:plugin@0.4.0 {
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
                    metric-version: u32,
                    low: f64,
                    high: f64,
                    source: string,
                }

                variant framework-profile-selection {
                    auto,
                    disabled,
                    exclude(list<string>),
                    only(list<string>),
                }

                record framework-profile-result {
                    active-profiles: list<string>,
                    entry-points: list<string>,
                    generated-paths: list<string>,
                    reference-ranges: list<reference-range>,
                    reference-range-note: option<string>,
                }

                extensions: func() -> list<string>;
                lower: func(path: string, text: string) -> result<file-model, lower-error>;
                manifest-paths: func(repository-paths: list<string>) -> list<string>;
                workspace-packages: func(manifests: list<manifest>) -> result<list<string>, workspace-error>;
                resolve-imports: func(repository-paths: list<string>, manifests: list<manifest>, requests: list<import-request>, active-profiles: list<string>) -> list<resolution>;
                declared-dependencies: func(manifests: list<manifest>) -> list<declared-dep>;
                entry-points: func(repository-paths: list<string>, manifests: list<manifest>) -> list<string>;
                reference-ranges: func() -> list<reference-range>;
                known-framework-profiles: func() -> list<string>;
                resolve-framework-profiles: func(package-root: string, files: list<string>, manifests: list<manifest>, selection: framework-profile-selection) -> result<framework-profile-result, string>;
                has-hazard-rules: func() -> bool;
                hazard-rules: func() -> list<string>;
                has-imports: func() -> bool;
            }
        }
    ",
});

struct DualVersionLanguagePlugin;

export!(DualVersionLanguagePlugin);

macro_rules! impl_common_guest {
    ($pkg:ident) => {
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
                hazard_lines: vec![],
                runtime_imports: vec![],
                program_start: false,
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

        fn has_hazard_rules() -> bool {
            false
        }

        fn hazard_rules() -> Vec<String> {
            vec![]
        }

        fn has_imports() -> bool {
            true
        }
    };
}

impl exports::codeint::plugin0_3_0::language::Guest for DualVersionLanguagePlugin {
    impl_common_guest!(plugin0_3_0);

    fn resolve_imports(
        _paths: Vec<String>,
        _manifests: Vec<exports::codeint::plugin0_3_0::language::Manifest>,
        _requests: Vec<exports::codeint::plugin0_3_0::language::ImportRequest>,
    ) -> Vec<exports::codeint::plugin0_3_0::language::Resolution> {
        vec![]
    }
}

impl exports::codeint::plugin0_4_0::language::Guest for DualVersionLanguagePlugin {
    impl_common_guest!(plugin0_4_0);

    fn resolve_imports(
        _paths: Vec<String>,
        _manifests: Vec<exports::codeint::plugin0_4_0::language::Manifest>,
        _requests: Vec<exports::codeint::plugin0_4_0::language::ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<exports::codeint::plugin0_4_0::language::Resolution> {
        vec![]
    }

    fn known_framework_profiles() -> Vec<String> {
        vec![]
    }

    fn resolve_framework_profiles(
        _package_root: String,
        _files: Vec<String>,
        _manifests: Vec<exports::codeint::plugin0_4_0::language::Manifest>,
        _selection: exports::codeint::plugin0_4_0::language::FrameworkProfileSelection,
    ) -> Result<exports::codeint::plugin0_4_0::language::FrameworkProfileResult, String> {
        Ok(
            exports::codeint::plugin0_4_0::language::FrameworkProfileResult {
                active_profiles: vec![],
                entry_points: vec![],
                generated_paths: vec![],
                reference_ranges: vec![],
                reference_range_note: None,
            },
        )
    }
}
