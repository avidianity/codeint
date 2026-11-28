#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
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

        world language-plugin {
            export language;
        }
    ",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, WorkspaceError,
};

struct LegacyV040LanguagePlugin;

export!(LegacyV040LanguagePlugin);

impl Guest for LegacyV040LanguagePlugin {
    fn extensions() -> Vec<String> {
        vec![String::from("legacy04")]
    }

    fn lower(_path: String, _text: String) -> Result<FileModel, LowerError> {
        Err(LowerError::DeclarationOnly)
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
        _active_profiles: Vec<String>,
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

    fn known_framework_profiles() -> Vec<String> {
        Vec::new()
    }

    fn resolve_framework_profiles(
        _package_root: String,
        _files: Vec<String>,
        _manifests: Vec<Manifest>,
        _selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        Ok(FrameworkProfileResult {
            active_profiles: Vec::new(),
            entry_points: Vec::new(),
            generated_paths: Vec::new(),
            reference_ranges: Vec::new(),
            reference_range_note: None,
        })
    }

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
