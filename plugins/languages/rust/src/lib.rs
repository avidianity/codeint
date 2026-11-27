#![forbid(unsafe_code)]

mod cfg_test;
mod dependencies;
mod entry_points;
mod functions;
mod imports;
mod resolution;
mod rules;
mod source_set;
mod tokens;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{
    DeclaredDep, FileModel, ImportRequest, LowerError, Manifest, Resolution, SourceSet,
};
use codeint_model::{
    InvalidReferenceRange, LanguagePlugin, LanguagePluginError, ReferenceRanges, RepoPath,
    WorkspaceError, validate_declared_dependencies, validate_entry_points,
    validate_workspace_packages,
};

use crate::cfg_test::excluded_cfg_test_lines;
use crate::entry_points::entry_points as discover_entry_points;
use crate::functions::{CodeLines, functions};
use crate::imports::runtime_import_specifiers;
use crate::resolution::{RustResolver, manifest_paths};
use crate::tokens::tokens;

pub use rules::flagged_line_rules;

pub(crate) const EXTENSIONS: [&str; 1] = ["rs"];

pub struct Rust;

fn lower_rust(path: &str, text: &str) -> Result<FileModel, LowerError> {
    if !path.ends_with(".rs") {
        return Err(LowerError::UnsupportedFile(format!(
            "{path} is not a Rust source file"
        )));
    }
    let parsed = syn::parse_file(text).map_err(|error| {
        let start = error.span().start();
        LowerError::Unparsable(format!(
            "{error} at line {}, column {}",
            start.line,
            start.column + 1
        ))
    })?;
    let source_set = source_set::classify(path, text);
    let exclude_cfg_test = source_set == SourceSet::Production;
    let excluded_lines = if exclude_cfg_test {
        excluded_cfg_test_lines(&parsed)
    } else {
        BTreeSet::new()
    };
    let code_lines = CodeLines::new(text, &excluded_lines);
    Ok(FileModel {
        source_set,
        source_lines: u32::try_from(code_lines.count()).map_err(|_| LowerError::TooLarge)?,
        functions: functions(&parsed, &code_lines, exclude_cfg_test),
        tokens: tokens(text, &parsed, &excluded_lines, exclude_cfg_test)?,
        flagged_lines: rules::flag(&parsed, &code_lines, exclude_cfg_test),
        hazard_lines: Vec::new(),
        runtime_imports: runtime_import_specifiers(&parsed, exclude_cfg_test),
        program_start: false,
    })
}

impl LanguagePlugin for Rust {
    fn extensions(&self) -> Vec<String> {
        EXTENSIONS.iter().map(ToString::to_string).collect()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, codeint_model::LowerError> {
        lower_rust(path.as_str(), text).map_err(codeint_model::LowerError::from)
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(
            codeint_model::scan::CoreReferenceRanges {
                erosion: (0.245, 0.704),
                cognitive_complexity: (0.176, 0.585),
                verbosity: (0.000, 0.005),
                duplication: (0.047, 0.377),
                import_cycles: Some((0.000, 0.611)),
            }
            .build("codeint stratified panel of 100 Rust repositories, ADR 0035, codeint 0.8.0"),
        )
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        let resolver = RustResolver::new(repository_paths, &manifests, &requests);
        Ok(requests
            .iter()
            .map(|request| resolver.resolve(request))
            .collect())
    }

    fn entry_points(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        validate_entry_points(
            repository_paths,
            discover_entry_points(repository_paths, &manifests),
        )
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        manifest_paths(repository_paths)
    }

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        validate_declared_dependencies(dependencies::extract_declared(&manifests))
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        validate_workspace_packages(workspaces::workspace_packages(&manifests)?)
    }

    codeint_model::empty_framework_profile_plugin_methods!();

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_imports(&self) -> bool {
        true
    }
}
