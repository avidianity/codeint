#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Function, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, Token, TokenKind,
    WorkspaceError,
};

struct NoImportsLanguagePlugin;

export!(NoImportsLanguagePlugin);

fn parse_noimp_line(
    line_number: u32,
    trimmed: &str,
    functions: &mut Vec<Function>,
    tokens: &mut Vec<Token>,
) {
    if let Some(rest) = trimmed.strip_prefix("fn ") {
        functions.push(Function {
            line: line_number,
            end_line: line_number,
            decision_points: 0,
            source_lines: 1,
            name: probe_runtime::parse_fn_name(rest),
            cognitive_complexity: 0,
            coverage: None,
        });
    }
    for word in trimmed.split_whitespace() {
        let kind = if word == "fn" {
            TokenKind::Other
        } else {
            TokenKind::Identifier
        };
        tokens.push(Token {
            kind,
            text_hash: probe_runtime::fnv1a_64(word),
            line: line_number,
        });
    }
}

impl Guest for NoImportsLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["noimp".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".noimp") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        if text.contains("SYNTAX_ERROR") {
            return Err(LowerError::Unparsable("syntax error".to_string()));
        }
        let source_set = if path.ends_with("_test.noimp") || path.ends_with(".test.noimp") {
            SourceSet::Test
        } else {
            SourceSet::Production
        };
        let mut source_lines: u32 = 0;
        let mut functions = Vec::new();
        let mut tokens = Vec::new();
        for (index, raw_line) in text.lines().enumerate() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            source_lines += 1;
            parse_noimp_line((index + 1) as u32, trimmed, &mut functions, &mut tokens);
        }
        Ok(probe_runtime::blank_file_model!(
            source_set,
            source_lines,
            functions: functions,
            tokens: tokens,
        ))
    }

    probe_runtime::panic_no_imports_stubs!();
    probe_runtime::fixture_erosion_ranges!("no-imports language fixture", false);
}
