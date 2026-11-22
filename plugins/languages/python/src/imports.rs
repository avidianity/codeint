use std::collections::BTreeSet;

use crate::lexer::{LogicalLine, PyToken};

pub(crate) fn scan_imports(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
) -> (Vec<String>, BTreeSet<usize>) {
    let mut runtime_imports = BTreeSet::new();
    let mut marked_indices = BTreeSet::new();
    let mut type_checking_indent: Option<usize> = None;

    for line in logical_lines {
        if type_checking_indent.is_some_and(|tc| line.indent <= tc) {
            type_checking_indent = None;
        }
        let slice = &tokens[line.tokens.clone()];
        if is_type_checking_guard(slice) {
            type_checking_indent = Some(line.indent);
            continue;
        }
        process_import_line(
            line,
            slice,
            type_checking_indent.is_none(),
            &mut marked_indices,
            &mut runtime_imports,
        );
    }

    (runtime_imports.into_iter().collect(), marked_indices)
}

fn process_import_line(
    line: &LogicalLine,
    slice: &[PyToken],
    is_runtime: bool,
    marked_indices: &mut BTreeSet<usize>,
    runtime_imports: &mut BTreeSet<String>,
) {
    let Some(first) = slice.first() else {
        return;
    };
    if first.is_kw("import") {
        marked_indices.extend(line.tokens.clone());
        if is_runtime {
            parse_import_stmt(&slice[1..], runtime_imports);
        }
    } else if first.is_kw("from") {
        marked_indices.extend(line.tokens.clone());
        if is_runtime {
            parse_from_import_stmt(&slice[1..], runtime_imports);
        }
    }
}

fn is_type_checking_guard(slice: &[PyToken]) -> bool {
    let Some(first) = slice.first() else {
        return false;
    };
    if !(first.is_kw("if") || first.is_kw("elif")) || slice.last().is_none_or(|t| t.text != ":") {
        return false;
    }
    match &slice[1..slice.len() - 1] {
        [t] => t.is_ident() && t.text == "TYPE_CHECKING",
        [a, dot, b] => a.is_ident() && dot.text == "." && b.is_ident() && b.text == "TYPE_CHECKING",
        _ => false,
    }
}

fn parse_import_stmt(slice: &[PyToken], out: &mut BTreeSet<String>) {
    let mut idx = 0usize;
    while idx < slice.len() {
        let mut module = String::new();
        while let Some(tok) = slice.get(idx) {
            if tok.is_ident() || tok.text == "." {
                module.push_str(&tok.text);
                idx += 1;
            } else {
                break;
            }
        }
        if !module.is_empty() {
            out.insert(module);
        }
        while let Some(tok) = slice.get(idx) {
            idx += 1;
            if tok.text == "," {
                break;
            }
        }
    }
}

fn parse_from_import_stmt(slice: &[PyToken], out: &mut BTreeSet<String>) {
    let Some(import_pos) = slice.iter().position(|t| t.is_kw("import")) else {
        return;
    };
    let (dots, after_dots) = consume_leading_dots(&slice[..import_pos]);
    let module = collect_dotted_module(&slice[after_dots..import_pos]);
    if dots.is_empty() && module == "__future__" {
        return;
    }
    if !module.is_empty() {
        out.insert(format!("{dots}{module}"));
        return;
    }
    if !dots.is_empty() {
        parse_relative_imported_names(&slice[import_pos + 1..], &dots, out);
    }
}

fn consume_leading_dots(prefix: &[PyToken]) -> (String, usize) {
    let mut dots = String::new();
    let mut idx = 0usize;
    while let Some(tok) = prefix.get(idx) {
        match tok.text.as_str() {
            "." | "..." => {
                dots.push_str(&tok.text);
                idx += 1;
            }
            _ => break,
        }
    }
    (dots, idx)
}

fn collect_dotted_module(tokens: &[PyToken]) -> String {
    let mut module = String::new();
    for tok in tokens {
        if tok.is_ident() || tok.text == "." {
            module.push_str(&tok.text);
        }
    }
    module
}

fn parse_relative_imported_names(slice: &[PyToken], dots: &str, out: &mut BTreeSet<String>) {
    let mut idx = 0usize;
    while idx < slice.len() {
        let tok = &slice[idx];
        if tok.is_ident() {
            out.insert(format!("{dots}{}", tok.text));
            idx += if slice.get(idx + 1).is_some_and(|t| t.is_kw("as")) {
                3
            } else {
                1
            };
        } else {
            idx += 1;
        }
    }
}
