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

fn aliased_strings(count: usize, chunk_bytes: usize) -> Vec<String> {
    let mut shared = String::with_capacity(chunk_bytes);
    for _ in 0..chunk_bytes {
        shared.push('a');
    }
    let ptr = shared.as_mut_ptr();
    let len = shared.len();
    let cap = shared.capacity();
    core::mem::forget(shared);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(unsafe { String::from_raw_parts(ptr, len, cap) });
    }
    out
}

struct MemoryHogLanguagePlugin;

export!(MemoryHogLanguagePlugin);

fn has_directive(manifests: &[Manifest], tag: &str) -> bool {
    manifests.iter().any(|m| m.text.contains(tag))
}

fn alloc_2mb_if_requested(manifests: &[Manifest]) {
    if has_directive(manifests, "ALLOC_2MB") {
        let mut buf = Vec::new();
        buf.resize(2 * 1024 * 1024, 1u8);
        core::hint::black_box(&buf);
    }
}

fn dep(package_root: &str, name: &str) -> DeclaredDep {
    DeclaredDep {
        package_root: package_root.to_string(),
        name: name.to_string(),
        kind: DepKind::Runtime,
    }
}

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
            program_start: false,
        })
    }

    fn manifest_paths(_paths: Vec<String>) -> Vec<String> {
        vec![]
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(vec![])
    }

    fn resolve_imports(
        _paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        vec![]
    }

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        alloc_2mb_if_requested(&manifests);
        if has_directive(&manifests, "ALIASED_BOMB") {
            return aliased_strings(256, 32 * 1024)
                .into_iter()
                .map(|name| DeclaredDep {
                    package_root: String::new(),
                    name,
                    kind: DepKind::Runtime,
                })
                .collect();
        }
        if has_directive(&manifests, "INVALID_EMPTY_NAME") {
            return vec![dep("", "")];
        }
        if has_directive(&manifests, "INVALID_WHITESPACE_NAME") {
            return vec![dep("", "   ")];
        }
        if has_directive(&manifests, "INVALID_TRAVERSAL_ROOT") {
            return vec![dep("../escape", "serde")];
        }
        if has_directive(&manifests, "INVALID_ABSOLUTE_ROOT") {
            return vec![dep("/etc/shadow", "serde")];
        }
        if has_directive(&manifests, "INVALID_BACKSLASH_ROOT") {
            return vec![dep("packages\\app", "serde")];
        }
        vec![dep("", "healthy-dep")]
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        alloc_2mb_if_requested(&manifests);
        if has_directive(&manifests, "ALIASED_BOMB") {
            return aliased_strings(256, 32 * 1024);
        }
        for (tag, bad) in [
            ("INVALID_TRAVERSAL", "../../../etc/passwd"),
            ("INVALID_ABSOLUTE", "/etc/shadow"),
            ("INVALID_UNKNOWN", "src/missing.hog"),
        ] {
            if has_directive(&manifests, tag) {
                return vec![bad.to_string()];
            }
        }
        repository_paths.into_iter().take(1).collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![]
    }
}
