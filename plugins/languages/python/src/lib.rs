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
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, Token, TokenKind,
    WorkspaceError,
};
use codeint_model::scan::build_reference_ranges;
use codeint_model::token_text_hash;

use crate::dependencies::extract_declared;
use crate::entry_points::discover as discover_entry_points;
use crate::resolution::{find_python_manifests, resolve_with_profiles};

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
                ("erosion", 1, 0.144, 0.624),
                ("verbosity", 2, 0.014, 0.204),
                ("duplication", 1, 0.012, 0.201),
                ("import_cycles", 1, 0.000, 0.321),
            ],
        )
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        find_python_manifests(&repository_paths)
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        workspaces::discover(&manifests)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        resolve_with_profiles(&repository_paths, &manifests, &requests, &active_profiles)
    }

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        extract_declared(&manifests)
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        discover_entry_points(&repository_paths, &manifests)
    }

    fn known_framework_profiles() -> Vec<String> {
        frameworks::known_profiles()
    }

    fn resolve_framework_profiles(
        package_root: String,
        files: Vec<String>,
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        frameworks::resolve_framework_profiles(package_root, files, manifests, selection)
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
            hazard_lines: Vec::new(),
            source_set: source_set::classify(&path, &lexed.header_comments),
            runtime_imports,
            tokens,
            source_lines,
            program_start: false,
        })
    }

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
    }
}
