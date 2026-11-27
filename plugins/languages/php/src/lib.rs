#![forbid(unsafe_code)]

mod builtins;
mod dependencies;
mod entry_points;
mod frameworks;
mod functions;
mod imports;
mod lexer;
mod resolution;
mod rules;
pub use rules::flagged_line_rules;
mod source_set;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{ImportRequest, LowerError, Resolution};
use codeint_model::{
    DeclaredDep, FileModel, InvalidReferenceRange, LanguagePlugin, LanguagePluginError, Manifest,
    ReferenceRanges, RepoPath, Token, TokenKind, WorkspaceError, token_text_hash,
    validate_declared_dependencies, validate_entry_points, validate_workspace_packages,
};

use crate::dependencies::extract_declared;
use crate::entry_points::extract as extract_entry_points;
use crate::lexer::{RawKind, RawToken};
use crate::resolution::{manifest_paths as find_manifest_paths, resolve_batch_with_profiles};

pub struct Php;

fn build_tokens(raw_tokens: &[RawToken], import_indices: &BTreeSet<usize>) -> Vec<Token> {
    raw_tokens
        .iter()
        .enumerate()
        .map(|(idx, raw)| Token {
            kind: classify_token(raw, import_indices.contains(&idx)),
            text_hash: token_text_hash(&raw.text),
            line: raw.line,
        })
        .collect()
}

fn classify_token(raw: &RawToken, is_import: bool) -> TokenKind {
    if is_import {
        return TokenKind::ImportOrExport;
    }
    match raw.kind {
        RawKind::Ident | RawKind::Variable => TokenKind::Identifier,
        RawKind::StringLit | RawKind::NumberLit => TokenKind::Literal,
        RawKind::Keyword | RawKind::Punct => TokenKind::Other,
    }
}

impl LanguagePlugin for Php {
    codeint_model::delegate_framework_profile_plugin_methods!(frameworks);

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(
            codeint_model::scan::CoreReferenceRanges {
                erosion: (0.064, 0.591),
                cognitive_complexity: (0.044, 0.542),
                verbosity: (0.000, 0.004),
                duplication: (0.025, 0.209),
                import_cycles: Some((0.000, 0.133)),
            }
            .build("codeint stratified panel of 100 PHP repositories, ADR 0035, codeint 0.8.0"),
        )
    }

    fn extensions(&self) -> Vec<String> {
        vec!["php".to_owned()]
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        find_manifest_paths(repository_paths)
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        validate_workspace_packages(workspaces::workspace_packages(&manifests)?)
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, codeint_model::LowerError> {
        let path = path.as_str();
        if !path.ends_with(".php") {
            return Err(
                LowerError::UnsupportedFile(format!("{path} is not a PHP source file")).into(),
            );
        }
        let lexed = lexer::lex(text).map_err(codeint_model::LowerError::from)?;
        let extracted = imports::extract(&lexed.tokens);
        let source_lines = u32::try_from(lexed.code_lines.len())
            .map_err(|_| codeint_model::LowerError::TooLarge)?;
        Ok(FileModel {
            source_set: source_set::classify(path, &lexed.header_comments),
            source_lines,
            functions: functions::functions(&lexed.tokens, &lexed.code_lines),
            tokens: build_tokens(&lexed.tokens, &extracted.import_token_indices),
            flagged_lines: rules::flag(&lexed.tokens, &lexed.code_lines),
            hazard_lines: Vec::new(),
            runtime_imports: extracted.specifiers,
            program_start: false,
        })
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Ok(resolve_batch_with_profiles(
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
            extract_entry_points(repository_paths, &manifests),
        )
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
