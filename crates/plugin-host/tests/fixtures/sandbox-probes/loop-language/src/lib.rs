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
    DeclaredDep, DepKind, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
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

    fn lower(
        _path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        if text.contains("LOOP_FOREVER") {
            loop {
                core::hint::spin_loop();
            }
        }
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            text.lines().count() as u32,
        ))
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        if repository_paths
            .iter()
            .any(|path| path == "loop-manifest-paths")
        {
            loop {
                core::hint::spin_loop();
            }
        }
        repository_paths
            .into_iter()
            .filter(|path| path == "trapper" || path.ends_with(".loopmanifest"))
            .collect()
    }

    fn analyze_manifests(
        manifests: Vec<Manifest>,
    ) -> exports::codeint::plugin::language::ManifestAnalysis {
        trigger_loop_or_trap(&manifests);
        let mut analysis = probe_runtime::empty_manifest_analysis!();
        analysis.declared_dependencies = vec![DeclaredDep {
            package_root: String::new(),
            name: "healthy-dep".to_string(),
            kind: DepKind::Runtime,
        }];
        analysis
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
        _package_profiles: Vec<exports::codeint::plugin::language::PackageFrameworkProfiles>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        trigger_loop_or_trap(&manifests);
        repository_paths.into_iter().take(1).collect()
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

    probe_runtime::v05_extra_language_stubs!();
}
