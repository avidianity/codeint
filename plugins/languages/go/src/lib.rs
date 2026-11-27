#![forbid(unsafe_code)]

mod dependencies;
mod entry_points;
mod functions;
mod go_mod;
mod imports;
mod lexer;
mod resolution;
mod rules;
mod source_set;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{
    DeclaredDep, FileModel, ImportRequest, LowerError, Manifest, Resolution, Token, TokenKind,
};
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::{
    InvalidReferenceRange, LanguagePlugin, LanguagePluginError, ReferenceRanges, RepoPath,
    WorkspaceError, token_text_hash, validate_declared_dependencies, validate_entry_points,
    validate_workspace_packages,
};

use crate::lexer::{GoKind, GoToken};

pub use rules::flagged_line_rules;

pub struct Go;

impl LanguagePlugin for Go {
    fn extensions(&self) -> Vec<String> {
        vec!["go".to_owned()]
    }

    fn has_imports(&self) -> bool {
        true
    }

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(
            CoreReferenceRanges {
                erosion: (0.304, 0.746),
                cognitive_complexity: (0.306, 0.671),
                verbosity: (0.000, 0.004),
                duplication: (0.034, 0.243),
                import_cycles: Some((0.000, 0.000)),
            }
            .build("codeint stratified panel of 100 Go repositories, ADR 0035, codeint 0.8.0"),
        )
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, codeint_model::LowerError> {
        lower_file(path.as_str(), text).map_err(codeint_model::LowerError::from)
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        resolution::find_go_mods(repository_paths)
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
        _active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Ok(resolution::resolve_all(
            repository_paths,
            &manifests,
            &requests,
        ))
    }

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        validate_declared_dependencies(dependencies::extract_declared(&manifests))
    }

    fn entry_points(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        validate_entry_points(
            repository_paths,
            entry_points::discover(repository_paths, &manifests),
        )
    }

    codeint_model::empty_framework_profile_plugin_methods!();
}

fn lower_file(path: &str, text: &str) -> Result<FileModel, LowerError> {
    if !path.ends_with(".go") {
        return Err(LowerError::UnsupportedFile(format!(
            "{path} is not a Go source file"
        )));
    }
    let scanned = lexer::scan(text)?;
    let source_lines = u32::try_from(scanned.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
    let scanned_imports = imports::scan(&scanned.tokens);
    let extracted = functions::extract(&scanned.tokens, &scanned.code_lines);
    let flagged_lines = rules::evaluate(&scanned.tokens, &extracted.spans, &scanned.code_lines);
    let tokens = build_duplication_tokens(&scanned.tokens, &scanned_imports.marked_indices);
    let source_set = source_set::classify(path, &scanned.header_line_comments);
    Ok(FileModel {
        runtime_imports: scanned_imports.specifiers,
        flagged_lines,
        hazard_lines: Vec::new(),
        tokens,
        functions: extracted.functions,
        source_lines,
        source_set,
        program_start: scanned_imports.is_package_main,
    })
}

fn build_duplication_tokens(raw: &[GoToken], import_indices: &BTreeSet<usize>) -> Vec<Token> {
    raw.iter()
        .enumerate()
        .filter(|(_, tok)| tok.kind != GoKind::AutoSemi)
        .map(|(idx, tok)| {
            let kind = if import_indices.contains(&idx) {
                TokenKind::ImportOrExport
            } else if tok.kind == GoKind::Ident {
                TokenKind::Identifier
            } else if matches!(
                tok.kind,
                GoKind::StringLit | GoKind::RuneLit | GoKind::NumberLit
            ) {
                TokenKind::Literal
            } else {
                TokenKind::Other
            };
            Token {
                kind,
                text_hash: token_text_hash(&tok.text),
                line: tok.line,
            }
        })
        .collect()
}
