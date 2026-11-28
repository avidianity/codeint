#![forbid(unsafe_code)]

mod dependencies;
mod entry_points;
mod frameworks;
mod functions;
mod nesting;
mod package_scripts;
mod resolution;
mod rules;
mod runtime_imports;
mod sfc;
mod source_set;
mod tokens;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{FileModel, LowerError};
use codeint_model::scan::CoreReferenceRanges;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_parser::config::TokensParserConfig;
use oxc_span::SourceType;

use crate::entry_points::entry_points as discover_entry_points;
use crate::functions::{CodeLines, functions};
use crate::resolution::{ProjectResolver, manifest_paths};
use crate::runtime_imports::runtime_import_specifiers;
use crate::source_set::source_set;
use crate::tokens::tokens;

pub use rules::flagged_line_rules;

pub(crate) const EXTENSIONS: [&str; 10] = [
    "ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs", "vue", "svelte",
];
const REFERENCE_RANGE_SOURCE: &str = "codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0";

const TS_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.187, 0.671),
    (0.121, 0.532),
    (0.000, 0.002),
    (0.018, 0.252),
    Some((0.000, 0.182)),
);

pub struct TypeScriptAndJavaScript;

fn lower_ts(
    _plugin: &TypeScriptAndJavaScript,
    path: &codeint_model::RepoPath,
    text: &str,
    _dialect: Option<&str>,
) -> Result<FileModel, codeint_model::LowerError> {
    let path = path.as_str();
    if sfc::is_sfc(path) {
        return lower_sfc(path, text).map_err(codeint_model::LowerError::from);
    }
    let source_type = source_type(path).map_err(codeint_model::LowerError::from)?;
    if source_type.is_typescript_definition() {
        return Err(codeint_model::LowerError::DeclarationOnly);
    }
    lower_script(path, text, source_type).map_err(codeint_model::LowerError::from)
}

fn ts_entry_points(
    paths: &[codeint_model::RepoPath],
    manifests: &[codeint_model::Manifest],
) -> Vec<String> {
    discover_entry_points(paths, manifests, &EXTENSIONS)
}

fn resolve_ts_imports(
    paths: &[codeint_model::RepoPath],
    manifests: &[codeint_model::Manifest],
    requests: &[codeint_model::language::ImportRequest],
    package_profiles: &[codeint_model::PackageFrameworkProfiles],
) -> Vec<codeint_model::language::Resolution> {
    let resolver = ProjectResolver::new(paths, manifests.to_vec());
    requests
        .iter()
        .map(|request| {
            let active =
                codeint_model::active_profiles_for_path(package_profiles, &request.importer);
            resolver.resolve(request, active)
        })
        .collect()
}

codeint_model::impl_builtin_language_plugin!(
    TypeScriptAndJavaScript,
    EXTENSIONS,
    lower_ts,
    TS_RANGES.build(REFERENCE_RANGE_SOURCE),
    flagged_lines(flagged_line_rules()),
    imports(
        manifest_paths,
        workspaces::analyze_manifests,
        resolve_ts_imports,
        ts_entry_points,
    ),
    frameworks(frameworks),
    hazards(none),
);

fn source_type(path: &str) -> Result<SourceType, LowerError> {
    let source_type = SourceType::from_path(path)
        .map_err(|error| LowerError::UnsupportedFile(error.to_string()))?;
    if source_type.is_javascript() {
        Ok(source_type.with_jsx(true))
    } else {
        Ok(source_type)
    }
}

fn lower_script(path: &str, text: &str, source_type: SourceType) -> Result<FileModel, LowerError> {
    nesting::check_nesting_depth(text)?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, text, source_type)
        .with_config(TokensParserConfig)
        .parse();
    if let Some(diagnostic) = parsed.diagnostics.first() {
        let message = match diagnostic.labels.first() {
            Some(label) => format!(
                "{diagnostic} at {}",
                position(text, label.offset() as usize)
            ),
            None => diagnostic.to_string(),
        };
        return Err(LowerError::Unparsable(message));
    }
    let comment_spans = parsed
        .program
        .comments
        .iter()
        .map(|comment| comment.span.start as usize..comment.span.end as usize);
    let code = blank_out(text, comment_spans);
    let header_comments = header_comments(text, &parsed.program.comments, &parsed.tokens);
    let code_lines = CodeLines::new(&code);
    Ok(FileModel {
        source_lines: u32::try_from(code_lines.count()).map_err(|_| LowerError::TooLarge)?,
        functions: functions(&parsed.program, &code_lines),
        tokens: tokens(text, &parsed.tokens, &parsed.program)?,
        flagged_lines: rules::flag(&parsed.program, &code, &code_lines)?,
        runtime_imports: runtime_import_specifiers(&parsed.program),
        is_tooling_file: source_set::is_tooling_file(path),
        ..codeint_model::empty_file_model(source_set(path, header_comments))
    })
}

fn lower_sfc(path: &str, text: &str) -> Result<FileModel, LowerError> {
    let extracted = sfc::extract(path, text)?;
    let mut base_source_set = source_set(path, extracted.header_comments.into_iter());
    let is_tooling_file = source_set::is_tooling_file(path);
    if extracted.scripts.is_empty() {
        return Ok(FileModel {
            is_tooling_file,
            ..codeint_model::empty_file_model(base_source_set)
        });
    }
    let mut total_lines = 0u32;
    let mut all_functions = Vec::new();
    let mut all_tokens = Vec::new();
    let mut all_flagged = Vec::new();
    let mut seen_imports = BTreeSet::new();
    let mut all_imports = Vec::new();
    for block in extracted.scripts {
        let padded = pad_lines(block.start_line, block.body);
        let lowered = lower_script(path, &padded, block.source_type)?;
        if lowered.source_set == codeint_model::language::SourceSet::Generated {
            base_source_set = codeint_model::language::SourceSet::Generated;
        }
        total_lines = total_lines
            .checked_add(lowered.source_lines)
            .ok_or(LowerError::TooLarge)?;
        all_functions.extend(lowered.functions);
        all_tokens.extend(lowered.tokens);
        all_flagged.extend(lowered.flagged_lines);
        for spec in lowered.runtime_imports {
            if seen_imports.insert(spec.clone()) {
                all_imports.push(spec);
            }
        }
    }
    all_functions.sort_by_key(|f| (f.line, f.end_line));
    all_flagged.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.rule.cmp(&b.rule)));
    Ok(FileModel {
        source_lines: total_lines,
        functions: all_functions,
        tokens: all_tokens,
        flagged_lines: all_flagged,
        runtime_imports: all_imports,
        is_tooling_file,
        ..codeint_model::empty_file_model(base_source_set)
    })
}

fn pad_lines(start_line: u32, body: &str) -> String {
    let prefix_newlines = start_line.saturating_sub(1) as usize;
    let mut out = String::with_capacity(prefix_newlines + body.len());
    out.extend(std::iter::repeat_n('\n', prefix_newlines));
    out.push_str(body);
    out
}

fn header_comments<'a>(
    text: &'a str,
    comments: &'a [oxc_ast::Comment],
    lexed: &[oxc_parser::Token],
) -> impl Iterator<Item = &'a str> {
    let first_code = lexed
        .iter()
        .find(|token| tokens::is_code_token(token.kind()))
        .map_or(u32::MAX, oxc_parser::Token::start);
    comments
        .iter()
        .take_while(move |comment| comment.span.end <= first_code)
        .map(|comment| &text[comment.span.start as usize..comment.span.end as usize])
}

fn position(text: &str, offset: usize) -> String {
    let before = text.get(..offset).unwrap_or(text);
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    let line = before.matches('\n').count() + 1;
    let column = before[line_start..].chars().count() + 1;
    format!("line {line}, column {column}")
}

fn blank_out(text: &str, spans: impl Iterator<Item = std::ops::Range<usize>>) -> String {
    let mut code = String::with_capacity(text.len());
    let mut cursor = 0;
    for span in spans {
        code.push_str(&text[cursor..span.start]);
        for character in text[span.clone()].chars() {
            if character == '\n' {
                code.push('\n');
            } else {
                code.extend(std::iter::repeat_n(' ', character.len_utf8()));
            }
        }
        cursor = span.end;
    }
    code.push_str(&text[cursor..]);
    code
}
