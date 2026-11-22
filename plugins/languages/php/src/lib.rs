#![forbid(unsafe_code)]

mod functions;
mod imports;
mod lexer;
mod resolution;
mod rules;
mod source_set;
mod workspaces;

use codeint_model::language::{
    FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, Token,
    TokenKind, WorkspaceError,
};
use codeint_model::token_text_hash;

use crate::lexer::RawKind;
use crate::resolution::{PhpResolver, manifest_paths};

pub use rules::flagged_line_rules;

pub(crate) const EXTENSIONS: [&str; 1] = ["php"];

pub struct Php;

impl Guest for Php {
    fn extensions() -> Vec<String> {
        EXTENSIONS.iter().map(ToString::to_string).collect()
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".php") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} is not a PHP source file"
            )));
        }
        let lexed = lexer::lex(&text)?;
        let source_set = source_set::classify(&path, &lexed.header_comments);
        let extracted_imports = imports::extract(&lexed.tokens);
        let tokens = lexed
            .tokens
            .iter()
            .enumerate()
            .map(|(idx, token)| {
                let kind = if extracted_imports.import_token_indices.contains(&idx) {
                    TokenKind::ImportOrExport
                } else {
                    match token.kind {
                        RawKind::Ident | RawKind::Variable => TokenKind::Identifier,
                        RawKind::StringLit | RawKind::NumberLit => TokenKind::Literal,
                        RawKind::Keyword | RawKind::Punct => TokenKind::Other,
                    }
                };
                Token {
                    kind,
                    text_hash: token_text_hash(&token.text),
                    line: token.line,
                }
            })
            .collect();
        Ok(FileModel {
            source_set,
            source_lines: u32::try_from(lexed.code_lines.len())
                .map_err(|_| LowerError::TooLarge)?,
            functions: functions::functions(&lexed.tokens, &lexed.code_lines),
            tokens,
            flagged_lines: rules::flag(&lexed.tokens, &lexed.code_lines),
            runtime_imports: extracted_imports.specifiers,
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
        let resolver = PhpResolver::new(&repository_paths, &manifests);
        requests
            .iter()
            .map(|request| resolver.resolve(request))
            .collect()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }
}
