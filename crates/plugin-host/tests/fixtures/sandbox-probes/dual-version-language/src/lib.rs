#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "dual-language-plugin",
    inline: "
        package codeint:dual@0.5.0;

        world dual-language-plugin {
            export codeint:plugin/language@0.4.0;
            export codeint:plugin/language@0.5.0;
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

        package codeint:plugin@0.5.0 {
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

                record file-diagnostic {
                    line: u32,
                    kind: string,
                    message: string,
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
                    is-tooling-file: bool,
                    diagnostics: list<file-diagnostic>,
                    scope-artifact: option<list<u8>>,
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

                record dependency-manifest-owner {
                    package-root: string,
                    path: string,
                }

                record package-dependency {
                    package-root: string,
                    name: string,
                }

                record manifest-analysis {
                    workspace-packages: list<string>,
                    invalid-manifests: list<workspace-error>,
                    dependency-manifests: list<dependency-manifest-owner>,
                    declared-dependencies: list<declared-dep>,
                    manifest-used-dependencies: list<package-dependency>,
                    root-dependency-inheritance: list<string>,
                }

                record package-framework-profiles {
                    package-root: string,
                    active-profiles: list<string>,
                }

                record file-scope-artifact {
                    path: string,
                    source-set: source-set,
                    artifact: list<u8>,
                }

                record file-hazards {
                    path: string,
                    hazard-lines: list<hazard-line>,
                }

                record summary-field {
                    key: string,
                    value: string,
                }

                record scope-summary {
                    kind: string,
                    fields: list<summary-field>,
                }

                record scope-analysis {
                    extra-hazards: list<file-hazards>,
                    summaries: list<scope-summary>,
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
                known-dialects: func() -> list<string>;
                lower: func(path: string, text: string, dialect: option<string>) -> result<file-model, lower-error>;
                flagged-line-rules: func() -> list<string>;
                manifest-paths: func(repository-paths: list<string>) -> list<string>;
                analyze-manifests: func(manifests: list<manifest>) -> manifest-analysis;
                resolve-imports: func(repository-paths: list<string>, manifests: list<manifest>, requests: list<import-request>, package-profiles: list<package-framework-profiles>) -> list<resolution>;
                entry-points: func(repository-paths: list<string>, manifests: list<manifest>) -> list<string>;
                reference-ranges: func() -> list<reference-range>;
                known-framework-profiles: func() -> list<string>;
                resolve-framework-profiles: func(package-root: string, files: list<string>, manifests: list<manifest>, selection: framework-profile-selection) -> result<framework-profile-result, string>;
                has-hazard-rules: func() -> bool;
                hazard-rules: func() -> list<string>;
                has-imports: func() -> bool;
                has-scope-analysis: func() -> bool;
                analyze-scope: func(package-root: string, dialect: option<string>, artifacts: list<file-scope-artifact>) -> scope-analysis;
            }
        }
    ",
});

struct DualVersionLanguagePlugin;

export!(DualVersionLanguagePlugin);

macro_rules! shared_dual_version_methods {
    ($($ns:ident)::+) => {
        fn extensions() -> Vec<String> {
            vec!["dual".to_string()]
        }

        fn manifest_paths(_paths: Vec<String>) -> Vec<String> {
            vec![]
        }

        fn entry_points(
            repository_paths: Vec<String>,
            _manifests: Vec<$($ns)::+::Manifest>,
        ) -> Vec<String> {
            repository_paths.into_iter().take(1).collect()
        }

        fn reference_ranges() -> Vec<$($ns)::+::ReferenceRange> {
            vec![]
        }

        fn known_framework_profiles() -> Vec<String> {
            vec![]
        }

        fn resolve_framework_profiles(
            _package_root: String,
            _files: Vec<String>,
            _manifests: Vec<$($ns)::+::Manifest>,
            _selection: $($ns)::+::FrameworkProfileSelection,
        ) -> Result<$($ns)::+::FrameworkProfileResult, String> {
            Ok($($ns)::+::FrameworkProfileResult {
                active_profiles: vec![],
                entry_points: vec![],
                generated_paths: vec![],
                reference_ranges: vec![],
                reference_range_note: None,
            })
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

macro_rules! dual_file_model {
    ($($ns:ident)::+, $text:expr $(, $extra_field:ident : $extra_val:expr)* $(,)?) => {
        Ok($($ns)::+::FileModel {
            source_set: $($ns)::+::SourceSet::Production,
            source_lines: $text.lines().count() as u32,
            functions: vec![],
            tokens: vec![],
            flagged_lines: vec![],
            hazard_lines: vec![],
            runtime_imports: vec![],
            program_start: false,
            $($extra_field: $extra_val,)*
        })
    };
}

impl exports::codeint::plugin0_4_0::language::Guest for DualVersionLanguagePlugin {
    shared_dual_version_methods!(exports::codeint::plugin0_4_0::language);

    fn lower(
        _path: String,
        text: String,
    ) -> Result<
        exports::codeint::plugin0_4_0::language::FileModel,
        exports::codeint::plugin0_4_0::language::LowerError,
    > {
        dual_file_model!(exports::codeint::plugin0_4_0::language, text)
    }

    fn workspace_packages(
        _manifests: Vec<exports::codeint::plugin0_4_0::language::Manifest>,
    ) -> Result<Vec<String>, exports::codeint::plugin0_4_0::language::WorkspaceError> {
        Ok(vec![])
    }

    fn resolve_imports(
        _paths: Vec<String>,
        _manifests: Vec<exports::codeint::plugin0_4_0::language::Manifest>,
        _requests: Vec<exports::codeint::plugin0_4_0::language::ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<exports::codeint::plugin0_4_0::language::Resolution> {
        vec![]
    }

    fn declared_dependencies(
        _manifests: Vec<exports::codeint::plugin0_4_0::language::Manifest>,
    ) -> Vec<exports::codeint::plugin0_4_0::language::DeclaredDep> {
        vec![]
    }
}

impl exports::codeint::plugin0_5_0::language::Guest for DualVersionLanguagePlugin {
    shared_dual_version_methods!(exports::codeint::plugin0_5_0::language);

    fn known_dialects() -> Vec<String> {
        vec![]
    }

    fn lower(
        _path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<
        exports::codeint::plugin0_5_0::language::FileModel,
        exports::codeint::plugin0_5_0::language::LowerError,
    > {
        dual_file_model!(
            exports::codeint::plugin0_5_0::language,
            text,
            is_tooling_file: false,
            diagnostics: vec![],
            scope_artifact: None,
        )
    }

    fn flagged_line_rules() -> Vec<String> {
        vec![]
    }

    fn analyze_manifests(
        _manifests: Vec<exports::codeint::plugin0_5_0::language::Manifest>,
    ) -> exports::codeint::plugin0_5_0::language::ManifestAnalysis {
        exports::codeint::plugin0_5_0::language::ManifestAnalysis {
            workspace_packages: vec![],
            invalid_manifests: vec![],
            dependency_manifests: vec![],
            declared_dependencies: vec![exports::codeint::plugin0_5_0::language::DeclaredDep {
                package_root: String::new(),
                name: "dual-dep".to_string(),
                kind: exports::codeint::plugin0_5_0::language::DepKind::Runtime,
            }],
            manifest_used_dependencies: vec![],
            root_dependency_inheritance: vec![],
        }
    }

    fn resolve_imports(
        _paths: Vec<String>,
        _manifests: Vec<exports::codeint::plugin0_5_0::language::Manifest>,
        _requests: Vec<exports::codeint::plugin0_5_0::language::ImportRequest>,
        _package_profiles: Vec<exports::codeint::plugin0_5_0::language::PackageFrameworkProfiles>,
    ) -> Vec<exports::codeint::plugin0_5_0::language::Resolution> {
        vec![]
    }

    fn has_scope_analysis() -> bool {
        false
    }

    fn analyze_scope(
        _package_root: String,
        _dialect: Option<String>,
        _artifacts: Vec<exports::codeint::plugin0_5_0::language::FileScopeArtifact>,
    ) -> exports::codeint::plugin0_5_0::language::ScopeAnalysis {
        exports::codeint::plugin0_5_0::language::ScopeAnalysis {
            extra_hazards: vec![],
            summaries: vec![],
        }
    }
}
