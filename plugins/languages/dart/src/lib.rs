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
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{
    DeclaredDep, FileModel, ImportRequest, LowerError, Manifest, Resolution, Token, TokenKind,
};
use codeint_model::{
    InvalidReferenceRange, LanguagePlugin, LanguagePluginError, ReferenceRanges, RepoPath,
    WorkspaceError, token_text_hash, validate_declared_dependencies, validate_entry_points,
    validate_workspace_packages,
};

use crate::dependencies::extract_declared;
use crate::entry_points::discover as discover_entry_points;
use crate::lexer::{DartKind, DartToken};

pub use rules::flagged_line_rules;

pub struct Dart;

fn lower_tokens(raw: &[DartToken], import_spans: &BTreeSet<usize>) -> Vec<Token> {
    let mut out = Vec::with_capacity(raw.len());
    for (idx, tok) in raw.iter().enumerate() {
        let kind = if import_spans.contains(&idx) {
            TokenKind::ImportOrExport
        } else {
            map_token_kind(tok.kind)
        };
        out.push(Token {
            kind,
            text_hash: token_text_hash(&tok.text),
            line: tok.line,
        });
    }
    out
}

fn map_token_kind(kind: DartKind) -> TokenKind {
    match kind {
        DartKind::Ident => TokenKind::Identifier,
        DartKind::StringLit | DartKind::NumberLit => TokenKind::Literal,
        DartKind::Keyword | DartKind::Punct => TokenKind::Other,
    }
}

fn lower_dart(path: &str, text: &str) -> Result<FileModel, LowerError> {
    if !path.ends_with(".dart") {
        return Err(LowerError::UnsupportedFile(format!(
            "{path} is not a Dart source file"
        )));
    }
    let lexed = lexer::scan(text)?;
    let (runtime_imports, marked_indices) = imports::scan_directives(&lexed.tokens);
    let functions = functions::extract(&lexed.tokens, &lexed.code_lines);
    let flagged_lines = rules::evaluate(&lexed.tokens, &lexed.code_lines);
    let tokens = lower_tokens(&lexed.tokens, &marked_indices);
    let source_set = source_set::classify(path, &lexed.header_comments);
    let source_lines = u32::try_from(lexed.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
    Ok(FileModel {
        runtime_imports,
        flagged_lines,
        hazard_lines: Vec::new(),
        tokens,
        functions,
        source_lines,
        source_set,
        program_start: false,
    })
}

impl LanguagePlugin for Dart {
    codeint_model::delegate_framework_profile_plugin_methods!(frameworks);

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        validate_declared_dependencies(extract_declared(&manifests))
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        validate_workspace_packages(workspaces::discover(&manifests)?)
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        resolution::find_pubspecs(repository_paths)
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        active_profiles: &[String],
    ) -> Result<Vec<Resolution>, LanguagePluginError> {
        Ok(resolution::resolve_dart_imports(
            repository_paths,
            &manifests,
            &requests,
            active_profiles,
        ))
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, codeint_model::LowerError> {
        lower_dart(path.as_str(), text).map_err(codeint_model::LowerError::from)
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

    fn extensions(&self) -> Vec<String> {
        vec!["dart".to_owned()]
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(
            codeint_model::scan::CoreReferenceRanges {
                erosion: (0.039, 0.497),
                cognitive_complexity: (0.000, 0.377),
                verbosity: (0.000, 0.011),
                duplication: (0.027, 0.256),
                import_cycles: Some((0.061, 0.600)),
            }
            .build("codeint stratified panel of 100 Dart repositories, ADR 0035, codeint 0.8.0"),
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
