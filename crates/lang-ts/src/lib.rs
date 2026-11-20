#![forbid(unsafe_code)]

mod resolution;
mod runtime_imports;

use codeint_model::language::{FileModel, Guest, ImportRequest, LowerError, Resolution};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::resolution::RelativeResolver;
use crate::runtime_imports::runtime_import_specifiers;

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
        let code = blank_out(&text, comment_spans);
        let source_lines = code
            .lines()
            .filter(|line| line.chars().any(is_code_character))
            .count();
        Ok(FileModel {
            source_lines: u32::try_from(source_lines).map_err(|_| LowerError::TooLarge)?,
            runtime_imports: runtime_import_specifiers(&parsed.program),
        })
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        let resolver = RelativeResolver::new(&repository_paths);
        requests
            .iter()
            .map(|request| resolver.resolve(request))
            .collect()
    }
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
