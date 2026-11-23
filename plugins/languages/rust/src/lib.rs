#![forbid(unsafe_code)]

mod cfg_test;
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
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    SourceSet, WorkspaceError,
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

impl Guest for Rust {
    fn extensions() -> Vec<String> {
        EXTENSIONS.iter().map(ToString::to_string).collect()
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".rs") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} is not a Rust source file"
            )));
        }
        let parsed = syn::parse_file(&text).map_err(|error| {
            let start = error.span().start();
            LowerError::Unparsable(format!(
                "{error} at line {}, column {}",
                start.line,
                start.column + 1
            ))
        })?;
        let source_set = source_set::classify(&path, &text);
        let exclude_cfg_test = source_set == SourceSet::Production;
        let excluded_lines = if exclude_cfg_test {
            excluded_cfg_test_lines(&parsed)
        } else {
            BTreeSet::new()
        };
        let code_lines = CodeLines::new(&text, &excluded_lines);
        Ok(FileModel {
            source_set,
            source_lines: u32::try_from(code_lines.count()).map_err(|_| LowerError::TooLarge)?,
            functions: functions(&parsed, &code_lines, exclude_cfg_test),
            tokens: tokens(&text, &parsed, &excluded_lines, exclude_cfg_test)?,
            flagged_lines: rules::flag(&parsed, &code_lines, exclude_cfg_test),
            runtime_imports: runtime_import_specifiers(&parsed, exclude_cfg_test),
        })
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        manifest_paths(&repository_paths)
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        workspaces::workspace_packages(&manifests)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        let resolver = RustResolver::new(&repository_paths, &manifests);
        requests
            .iter()
            .map(|request| resolver.resolve(request))
            .collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        codeint_model::scan::build_reference_ranges(
            "codeint panel of 20 Rust repositories, ADR 0010, codeint 0.2.1",
            [
                ("erosion", 0.380, 0.642),
                ("verbosity", 0.060, 0.329),
                ("duplication", 0.050, 0.325),
                ("import_cycles", 0.021, 0.506),
            ],
        )
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        discover_entry_points(&repository_paths, &manifests)
    }
}
