#![forbid(unsafe_code)]

use alloc::string::String;
use alloc::vec::Vec;

pub use crate::runtime::__bindings::language::exports::codeint::plugin::language::{
    DeclaredDep, DepKind, DependencyManifestOwner, FileDiagnostic, FileHazards, FileModel,
    FileScopeArtifact, FlaggedLine, FrameworkProfileResult, FrameworkProfileSelection, Function,
    FunctionCoverage, HazardLine, ImportRequest, LowerError, Manifest, ManifestAnalysis,
    PackageDependency, PackageFrameworkProfiles, ReferenceRange, Resolution, ScopeAnalysis,
    ScopeSummary, SourceSet, SummaryField, Token, TokenKind, WorkspaceError,
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
            is_tooling_file: false,
            diagnostics: Vec::new(),
            scope_artifact: None,
        }
    }
}

impl ManifestAnalysis {
    pub fn empty() -> ManifestAnalysis {
        ManifestAnalysis {
            workspace_packages: Vec::new(),
            invalid_manifests: Vec::new(),
            dependency_manifests: Vec::new(),
            declared_dependencies: Vec::new(),
            manifest_used_dependencies: Vec::new(),
            root_dependency_inheritance: Vec::new(),
        }
    }
}

impl ScopeAnalysis {
    pub fn empty() -> ScopeAnalysis {
        ScopeAnalysis {
            extra_hazards: Vec::new(),
            summaries: Vec::new(),
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

    fn known_dialects() -> Vec<String> {
        Vec::new()
    }

    fn lower(path: String, text: String, dialect: Option<String>) -> Result<FileModel, LowerError>;

    fn flagged_line_rules() -> Vec<String> {
        Vec::new()
    }

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

    fn analyze_manifests(_manifests: Vec<Manifest>) -> ManifestAnalysis {
        ManifestAnalysis::empty()
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
        _package_profiles: Vec<PackageFrameworkProfiles>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn has_scope_analysis() -> bool {
        false
    }

    fn analyze_scope(
        _package_root: String,
        _dialect: Option<String>,
        _artifacts: Vec<FileScopeArtifact>,
    ) -> ScopeAnalysis {
        ScopeAnalysis::empty()
    }
}
