#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::format;
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

struct ProgramStartLanguagePlugin;

export!(ProgramStartLanguagePlugin);

impl Guest for ProgramStartLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["prog".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".prog") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        let source_set = if path.ends_with("_test.prog") || path.ends_with(".test.prog") {
            SourceSet::Test
        } else {
            SourceSet::Production
        };
        let source_lines = probe_runtime::count_non_empty_lines(&text);
        let program_start = text.lines().any(|line| line.trim() == "program main");
        let runtime_imports = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("use "))
            .map(|spec| spec.trim().to_string())
            .filter(|spec| !spec.is_empty())
            .collect();
        Ok(probe_runtime::blank_file_model!(
            source_set,
            source_lines,
            runtime_imports: runtime_imports,
            program_start: program_start,
        ))
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| {
                let target = match request.specifier.strip_prefix("./") {
                    Some(relative) => match request.importer.rsplit_once('/') {
                        Some((dir, _)) => format!("{dir}/{relative}"),
                        None => relative.to_string(),
                    },
                    None => request.specifier,
                };
                let candidates = if target.ends_with(".prog") {
                    vec![target.clone()]
                } else {
                    vec![target.clone(), format!("{target}.prog")]
                };
                candidates
                    .into_iter()
                    .find(|candidate| repository_paths.iter().any(|path| path == candidate))
                    .map(|found| Resolution::Resolved(vec![found]))
                    .unwrap_or_else(|| Resolution::Unresolved(format!("missing {target}")))
            })
            .collect()
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
