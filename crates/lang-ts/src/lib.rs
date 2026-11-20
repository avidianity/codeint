#![forbid(unsafe_code)]

mod functions;
mod resolution;
mod rules;
mod runtime_imports;
mod source_set;
mod tokens;
mod workspaces;

use codeint_model::language::{
    FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    WorkspaceError,
};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_parser::config::TokensParserConfig;
use oxc_span::SourceType;

use crate::functions::{CodeLines, functions};
use crate::resolution::{ProjectResolver, manifest_paths};
use crate::runtime_imports::runtime_import_specifiers;
use crate::source_set::source_set;
use crate::tokens::tokens;

pub use rules::flagged_line_rules;

pub(crate) const EXTENSIONS: [&str; 8] = ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"];
const REFERENCE_RANGE_SOURCE: &str =
    "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0";

pub struct TypeScriptAndJavaScript;

impl Guest for TypeScriptAndJavaScript {
    fn extensions() -> Vec<String> {
        EXTENSIONS.iter().map(ToString::to_string).collect()
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let source_type = source_type(&path)?;
        if source_type.is_typescript_definition() {
            return Err(LowerError::DeclarationOnly);
        }
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &text, source_type)
            .with_config(TokensParserConfig)
            .parse();
        if let Some(diagnostic) = parsed.diagnostics.first() {
            let message = match diagnostic.labels.first() {
                Some(label) => format!(
                    "{diagnostic} at {}",
                    position(&text, label.offset() as usize)
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
        let code = blank_out(&text, comment_spans);
        let header_comments = header_comments(&text, &parsed.program.comments, &parsed.tokens);
        let code_lines = CodeLines::new(&code);
        Ok(FileModel {
            source_set: source_set(&path, header_comments),
            source_lines: u32::try_from(code_lines.count()).map_err(|_| LowerError::TooLarge)?,
            functions: functions(&parsed.program, &code_lines),
            tokens: tokens(&text, &parsed.tokens, &parsed.program)?,
            flagged_lines: rules::flag(&parsed.program, &code, &code_lines)?,
            runtime_imports: runtime_import_specifiers(&parsed.program),
        })
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
        let resolver = ProjectResolver::new(&repository_paths, manifests);
        requests
            .iter()
            .map(|request| resolver.resolve(request))
            .collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        [
            ("erosion", 0.352, 0.600),
            ("verbosity", 0.018, 0.347),
            ("duplication", 0.017, 0.346),
            ("import_cycles", 0.000, 0.308),
        ]
        .into_iter()
        .map(|(metric, low, high)| ReferenceRange {
            metric: metric.to_owned(),
            low,
            high,
            source: REFERENCE_RANGE_SOURCE.to_owned(),
        })
        .collect()
    }
}

fn source_type(path: &str) -> Result<SourceType, LowerError> {
    let source_type = SourceType::from_path(path)
        .map_err(|error| LowerError::UnsupportedFile(error.to_string()))?;
    if source_type.is_javascript() {
        Ok(source_type.with_jsx(true))
    } else {
        Ok(source_type)
    }
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
