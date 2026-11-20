#![forbid(unsafe_code)]

mod functions;

use codeint_model::language::{FileModel, Guest, LowerError};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::functions::{CodeLines, functions};

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
        let parsed = Parser::new(&allocator, &text, source_type).parse();
        if let Some(diagnostic) = parsed.diagnostics.first() {
            return Err(LowerError::Unparsable(diagnostic.to_string()));
        }
        let comment_spans = parsed
            .program
            .comments
            .iter()
            .map(|comment| comment.span.start as usize..comment.span.end as usize);
        let code_lines = CodeLines::new(&blank_out(&text, comment_spans));
        Ok(FileModel {
            source_lines: u32::try_from(code_lines.count()).map_err(|_| LowerError::TooLarge)?,
            functions: functions(&parsed.program, &code_lines),
        })
    }
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
