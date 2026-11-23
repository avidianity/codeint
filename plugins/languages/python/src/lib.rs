#![forbid(unsafe_code)]

mod entry_points;
mod functions;
mod imports;
mod lexer;
mod resolution;
mod rules;
mod source_set;
mod workspaces;

use codeint_model::language::{
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    Token, TokenKind, WorkspaceError,
};
use codeint_model::scan::build_reference_ranges;
use codeint_model::token_text_hash;

use crate::entry_points::discover as discover_entry_points;

pub use rules::flagged_line_rules;

pub struct Python;

impl Guest for Python {
    fn extensions() -> Vec<String> {
        vec!["py".to_owned()]
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        build_reference_ranges(
            "codeint panel of 20 Python repositories, ADR 0013, codeint 0.4.0",
            [
                ("erosion", 0.144, 0.624),
                ("verbosity", 0.014, 0.204),
                ("duplication", 0.012, 0.201),
                ("import_cycles", 0.000, 0.321),
            ],
        )
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        resolution::find_pyprojects(&repository_paths)
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        workspaces::discover(&manifests)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        resolution::resolve_all(&repository_paths, &manifests, &requests)
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        discover_entry_points(&repository_paths, &manifests)
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".py") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} is not a Python source file"
            )));
        }
        let lexed = lexer::scan(&text)?;
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
        let source_lines =
            u32::try_from(lexed.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
        Ok(FileModel {
            functions: functions::extract(&lexed.tokens, &lexed.logical_lines, &lexed.code_lines),
            flagged_lines: rules::evaluate(&lexed.tokens, &lexed.logical_lines, &lexed.code_lines),
            source_set: source_set::classify(&path, &lexed.header_comments),
            runtime_imports,
            tokens,
            source_lines,
        })
    }
}
