use std::collections::BTreeSet;
use std::str::FromStr;

use codeint_model::language::{LowerError, Token, TokenKind};
use codeint_model::token_text_hash;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{File, Item};

use crate::cfg_test::{has_cfg_test, item_attrs};

const RUST_KEYWORDS: [&str; 39] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield",
];

pub(crate) fn tokens(
    text: &str,
    file: &File,
    excluded_lines: &BTreeSet<u32>,
    exclude_cfg_test: bool,
) -> Result<Vec<Token>, LowerError> {
    let import_lines = import_declaration_lines(file, exclude_cfg_test);
    let stream =
        TokenStream::from_str(text).map_err(|error| LowerError::Unparsable(error.to_string()))?;
    let mut output = Vec::new();
    collect_stream(stream, excluded_lines, &import_lines, &mut output);
    Ok(output)
}

fn collect_stream(
    stream: TokenStream,
    excluded_lines: &BTreeSet<u32>,
    import_lines: &BTreeSet<u32>,
    output: &mut Vec<Token>,
) {
    for tree in stream {
        match tree {
            TokenTree::Group(group) => {
                let (open, close) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                if !open.is_empty() {
                    let open_line = group.span_open().start().line as u32;
                    push_token(
                        open_line,
                        open,
                        TokenKind::Other,
                        excluded_lines,
                        import_lines,
                        output,
                    );
                }
                collect_stream(group.stream(), excluded_lines, import_lines, output);
                if !close.is_empty() {
                    let close_line = group.span_close().end().line as u32;
                    push_token(
                        close_line,
                        close,
                        TokenKind::Other,
                        excluded_lines,
                        import_lines,
                        output,
                    );
                }
            }
            TokenTree::Ident(ident) => {
                let line = ident.span().start().line as u32;
                let text = ident.to_string();
                let kind = if RUST_KEYWORDS.contains(&text.as_str()) {
                    TokenKind::Other
                } else {
                    TokenKind::Identifier
                };
                push_token(line, &text, kind, excluded_lines, import_lines, output);
            }
            TokenTree::Literal(literal) => {
                let line = literal.span().start().line as u32;
                let text = literal.to_string();
                push_token(
                    line,
                    &text,
                    TokenKind::Literal,
                    excluded_lines,
                    import_lines,
                    output,
                );
            }
            TokenTree::Punct(punct) => {
                let line = punct.span().start().line as u32;
                let mut buffer = [0u8; 4];
                let text = punct.as_char().encode_utf8(&mut buffer);
                push_token(
                    line,
                    text,
                    TokenKind::Other,
                    excluded_lines,
                    import_lines,
                    output,
                );
            }
        }
    }
}

fn push_token(
    line: u32,
    text: &str,
    default_kind: TokenKind,
    excluded_lines: &BTreeSet<u32>,
    import_lines: &BTreeSet<u32>,
    output: &mut Vec<Token>,
) {
    if excluded_lines.contains(&line) {
        return;
    }
    let kind = if import_lines.contains(&line) {
        TokenKind::ImportOrExport
    } else {
        default_kind
    };
    output.push(Token {
        kind,
        text_hash: token_text_hash(text),
        line,
    });
}

fn import_declaration_lines(file: &File, exclude_cfg_test: bool) -> BTreeSet<u32> {
    let mut collector = ImportLineCollector {
        exclude_cfg_test,
        lines: BTreeSet::new(),
    };
    collector.visit_file(file);
    collector.lines
}

struct ImportLineCollector {
    exclude_cfg_test: bool,
    lines: BTreeSet<u32>,
}

impl<'ast> Visit<'ast> for ImportLineCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.exclude_cfg_test && has_cfg_test(item_attrs(item)) {
            return;
        }
        if matches!(item, Item::Use(_) | Item::ExternCrate(_)) {
            let start = item.span().start().line as u32;
            let end = item.span().end().line as u32;
            for line in start..=end {
                self.lines.insert(line);
            }
            return;
        }
        visit::visit_item(self, item);
    }
}
