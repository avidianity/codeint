#![forbid(unsafe_code)]

use alloc::string::String;
use alloc::vec::Vec;

pub use crate::runtime::__bindings::language::exports::codeint::plugin::language::{
    DeclaredDep, DepKind, FileModel, FlaggedLine, FrameworkProfileResult,
    FrameworkProfileSelection, Function, FunctionCoverage, HazardLine, ImportRequest, LowerError,
    Manifest, ReferenceRange, Resolution, SourceSet, Token, TokenKind, WorkspaceError,
};

impl FileModel {
    pub fn empty(source_set: SourceSet) -> FileModel {
        FileModel {
            source_set,
            source_lines: 0,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        }
    }
}

impl FrameworkProfileResult {
    pub fn empty() -> FrameworkProfileResult {
        FrameworkProfileResult {
            active_profiles: Vec::new(),
            entry_points: Vec::new(),
            generated_paths: Vec::new(),
            reference_ranges: Vec::new(),
            reference_range_note: None,
        }
    }
}

pub trait LanguagePlugin {
    fn extensions() -> Vec<String>;

    fn lower(path: String, text: String) -> Result<FileModel, LowerError>;

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
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
        Ok(FrameworkProfileResult::empty())
    }

    fn has_imports() -> bool {
        false
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }
}
