#![forbid(unsafe_code)]

mod functions;
mod rules;
mod source_set;

use codeint_model::language::{FileModel, Guest, LowerError, Token};
use oxc_allocator::Allocator;
use oxc_parser::config::TokensParserConfig;
use oxc_parser::{Kind, Parser};
use oxc_span::SourceType;

use crate::functions::{CodeLines, functions};
use crate::source_set::source_set;

pub use rules::flagged_line_rules;

const EXTENSIONS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

pub struct TypeScriptAndJavaScript;

impl Guest for TypeScriptAndJavaScript {
    fn extensions() -> Vec<String> {
        EXTENSIONS.iter().map(ToString::to_string).collect()
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let source_type = SourceType::from_path(&path)
            .map_err(|error| LowerError::UnsupportedFile(error.to_string()))?;
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
        let code_lines = CodeLines::new(&code);
        Ok(FileModel {
            source_set: source_set(&path),
            source_lines: u32::try_from(code_lines.count()).map_err(|_| LowerError::TooLarge)?,
            functions: functions(&parsed.program, &code_lines),
            tokens: tokens(&text, &parsed.tokens)?,
            flagged_lines: rules::flag(&parsed.program, &code, &code_lines)?,
        })
    }
}

fn tokens(text: &str, lexed: &[oxc_parser::Token]) -> Result<Vec<Token>, LowerError> {
    let mut tokens = Vec::with_capacity(lexed.len());
    let mut line: u32 = 1;
    let mut cursor = 0;
    for token in lexed.iter().filter(|token| is_code_token(token.kind())) {
        let start = token.start() as usize;
        let newlines = text.as_bytes()[cursor..start]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
        line = u32::try_from(newlines)
            .ok()
            .and_then(|newlines| line.checked_add(newlines))
            .ok_or(LowerError::TooLarge)?;
        cursor = start;
        tokens.push(Token {
            text: text[start..token.end() as usize].to_owned(),
            line,
        });
    }
    Ok(tokens)
}

fn is_code_token(kind: Kind) -> bool {
    !matches!(kind, Kind::Eof | Kind::Skip | Kind::HashbangComment)
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
