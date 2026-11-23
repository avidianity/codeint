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
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    SourceSet, WorkspaceError,
};

struct MemoryHogLanguagePlugin;

export!(MemoryHogLanguagePlugin);

impl Guest for MemoryHogLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["hog".to_string()]
    }

    fn lower(_path: String, text: String) -> Result<FileModel, LowerError> {
        if text.contains("HOG_MEMORY") {
            let mut chunks: Vec<Vec<u8>> = Vec::new();
            loop {
                let mut chunk = Vec::new();
                chunk.resize(1024 * 1024, 1u8);
                chunks.push(chunk);
                core::hint::black_box(&chunks);
            }
        }
        if text.contains("ALLOC_2MB") {
            let mut buf = Vec::new();
            buf.resize(2 * 1024 * 1024, 1u8);
            core::hint::black_box(&buf);
        }
        Ok(FileModel {
            source_set: SourceSet::Production,
            source_lines: text.lines().count() as u32,
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
