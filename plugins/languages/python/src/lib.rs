#![forbid(unsafe_code)]

mod dependencies;
mod entry_points;
mod frameworks;
mod functions;
mod imports;
mod lexer;
mod resolution;
mod rules;
mod source_set;
mod stdlib;
mod workspaces;

use codeint_model::language::{
    DeclaredDep, FileModel, ImportRequest, LowerError, Manifest, Resolution, Token, TokenKind,
};
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::{
    InvalidReferenceRange, LanguagePlugin, LanguagePluginError, ReferenceRanges, RepoPath,
    WorkspaceError, token_text_hash, validate_declared_dependencies, validate_entry_points,
    validate_workspace_packages,
};

use crate::dependencies::extract_declared;
use crate::entry_points::discover as discover_entry_points;
use crate::resolution::{find_python_manifests, resolve_with_profiles};

pub use rules::flagged_line_rules;

pub struct Python;

impl LanguagePlugin for Python {
    fn extensions(&self) -> Vec<String> {
        vec!["py".to_owned()]
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(
            CoreReferenceRanges {
                erosion: (0.239, 0.717),
                cognitive_complexity: (0.249, 0.637),
                verbosity: (0.000, 0.011),
                duplication: (0.011, 0.220),
                import_cycles: Some((0.000, 0.446)),
            }
            .build("codeint stratified panel of 100 Python repositories, ADR 0035, codeint 0.8.0"),
        )
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        find_python_manifests(repository_paths)
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        validate_workspace_packages(workspaces::discover(&manifests)?)
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Ok(resolve_with_profiles(
            repository_paths,
            &manifests,
            &requests,
            active_profiles,
        ))
    }

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        validate_declared_dependencies(extract_declared(&manifests))
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

    codeint_model::delegate_framework_profile_plugin_methods!(frameworks);

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, codeint_model::LowerError> {
        let path = path.as_str();
        if !path.ends_with(".py") {
            return Err(
                LowerError::UnsupportedFile(format!("{path} is not a Python source file")).into(),
            );
        }
        let lexed = lexer::scan(text).map_err(codeint_model::LowerError::from)?;
        let (runtime_imports, import_indices) =
            imports::scan_imports(&lexed.tokens, &lexed.logical_lines);
        let tokens = lexed
            .tokens
            .iter()
            .enumerate()
            .map(|(idx, tok)| Token {
                kind: if import_indices.contains(&idx) {
                    TokenKind::ImportOrExport
                } else {
                    tok.token_kind
                },
                text_hash: token_text_hash(&tok.text),
                line: tok.line,
            })
            .collect();
        let source_lines = u32::try_from(lexed.code_lines.len())
            .map_err(|_| codeint_model::LowerError::TooLarge)?;
        Ok(FileModel {
            functions: functions::extract(&lexed.tokens, &lexed.logical_lines, &lexed.code_lines),
            flagged_lines: rules::evaluate(&lexed.tokens, &lexed.logical_lines, &lexed.code_lines),
            hazard_lines: Vec::new(),
            source_set: source_set::classify(path, &lexed.header_comments),
            runtime_imports,
            tokens,
            source_lines,
            program_start: false,
        })
    }

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
