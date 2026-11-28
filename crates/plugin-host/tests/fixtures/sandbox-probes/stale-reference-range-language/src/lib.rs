#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
};

struct StaleReferenceRangeLanguagePlugin;

export!(StaleReferenceRangeLanguagePlugin);

impl Guest for StaleReferenceRangeLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["stale".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if path.ends_with(".stale") {
            let lines = probe_runtime::count_non_empty_lines(&text);
            Ok(probe_runtime::blank_file_model!(SourceSet::Production, lines))
        } else {
            Err(LowerError::UnsupportedFile(path))
        }
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
        [
            ("erosion", 2, 0.1, 0.4, "stale v2 erosion panel"),
            ("verbosity", 0, 0.05, 0.25, "invalid v0 verbosity panel"),
            ("duplication", 2, 0.02, 0.22, "fresh v2 duplication panel"),
        ]
        .into_iter()
        .map(|(metric, metric_version, low, high, source)| ReferenceRange {
            metric: metric.to_string(),
            metric_version,
            low,
            high,
            source: source.to_string(),
        })
        .collect()
    }

    probe_runtime::empty_framework_profile_stubs!();

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
