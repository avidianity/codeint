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
    DeclaredDep, DepKind, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange,
    Resolution, SourceSet, WorkspaceError,
};

struct LoopLanguagePlugin;

export!(LoopLanguagePlugin);

fn trigger_loop_or_trap(manifests: &[Manifest]) {
    for manifest in manifests {
        if manifest.text.contains("LOOP_FOREVER") {
            loop {
                core::hint::spin_loop();
            }
        }
        if manifest.text.contains("PANIC_TRAP") {
            panic!("probe trap");
        }
    }
}

impl Guest for LoopLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["loop".to_string()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        if text.contains("LOOP_FOREVER") {
            loop {
                core::hint::spin_loop();
            }
        }
        Ok(FileModel {
            source_set: SourceSet::Production,
            source_lines: text.lines().count() as u32,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
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

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        trigger_loop_or_trap(&manifests);
        vec![DeclaredDep {
            package_root: String::new(),
            name: "healthy-dep".to_string(),
            kind: DepKind::Runtime,
        }]
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        trigger_loop_or_trap(&manifests);
        repository_paths.into_iter().take(1).collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }
}
