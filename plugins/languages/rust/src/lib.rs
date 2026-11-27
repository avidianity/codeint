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

use codeint_model::language::{FileModel, LowerError, SourceSet};
use codeint_model::scan::CoreReferenceRanges;

use crate::cfg_test::excluded_cfg_test_lines;
use crate::entry_points::entry_points as discover_entry_points;
use crate::functions::{CodeLines, functions};
use crate::imports::runtime_import_specifiers;
use crate::resolution::{RustResolver, manifest_paths};
use crate::tokens::tokens;

pub use rules::flagged_line_rules;

pub(crate) const EXTENSIONS: [&str; 1] = ["rs"];

pub struct Rust;

const RUST_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.245, 0.704),
    (0.176, 0.585),
    (0.000, 0.005),
    (0.047, 0.377),
    Some((0.000, 0.611)),
);

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

fn lower_rust_plugin(
    _plugin: &Rust,
    path: &codeint_model::RepoPath,
    text: &str,
) -> Result<FileModel, codeint_model::LowerError> {
    lower_rust(path.as_str(), text).map_err(codeint_model::LowerError::from)
}

codeint_model::impl_builtin_language_plugin!(
    Rust,
    EXTENSIONS,
    lower_rust_plugin,
    RUST_RANGES.build("codeint stratified panel of 100 Rust repositories, ADR 0035, codeint 0.8.0"),
    imports(
        manifest_paths,
        workspaces::workspace_packages,
        |paths, manifests, requests, _| {
            let resolver = RustResolver::new(paths, manifests, requests);
            requests
                .iter()
                .map(|request| resolver.resolve(request))
                .collect()
        },
        dependencies::extract_declared,
        discover_entry_points,
    ),
    frameworks(none),
    hazards(none),
);
