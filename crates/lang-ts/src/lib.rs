#![forbid(unsafe_code)]

use codeint_model::language::{FileModel, Guest, LowerError, Token};
use oxc_allocator::Allocator;
use oxc_parser::config::TokensParserConfig;
use oxc_parser::{Kind, Parser};
use oxc_span::SourceType;

const EXTENSIONS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

pub struct TypeScriptAndJavaScript;

impl Guest for TypeScriptAndJavaScript {
    fn extensions() -> Vec<String> {
        EXTENSIONS.iter().map(ToString::to_string).collect()
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let source_type = SourceType::from_path(&path)
            .map_err(|error| LowerError::UnsupportedFile(error.to_string()))?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &text, source_type)
            .with_config(TokensParserConfig)
            .parse();
        if let Some(diagnostic) = parsed.diagnostics.first() {
            return Err(LowerError::Unparsable(diagnostic.to_string()));
        }
        let comment_spans = parsed
            .program
            .comments
            .iter()
            .map(|comment| comment.span.start as usize..comment.span.end as usize);
        let code = blank_out(&text, comment_spans);
        let source_lines = code
            .lines()
            .filter(|line| line.chars().any(is_code_character))
            .count();
        Ok(FileModel {
            source_lines: u32::try_from(source_lines).map_err(|_| LowerError::TooLarge)?,
            tokens: tokens(&text, &parsed.tokens)?,
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

fn blank_out(text: &str, spans: impl Iterator<Item = std::ops::Range<usize>>) -> String {
    let mut code = String::with_capacity(text.len());
    let mut cursor = 0;
    for span in spans {
        code.push_str(&text[cursor..span.start]);
        code.extend(
            text[span.clone()]
                .chars()
                .map(|character| if character == '\n' { '\n' } else { ' ' }),
        );
        cursor = span.end;
    }
    code.push_str(&text[cursor..]);
    code
}

fn is_code_character(character: char) -> bool {
    !character.is_whitespace() && character != '\u{feff}'
}
