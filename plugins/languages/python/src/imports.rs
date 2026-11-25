use std::collections::BTreeSet;

use crate::functions::find_top_level_colon;
use crate::lexer::{LogicalLine, PyToken};

pub(crate) fn scan_imports(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
) -> (Vec<String>, BTreeSet<usize>) {
    let mut runtime_imports = BTreeSet::new();
    let mut marked_indices = BTreeSet::new();
    let mut type_checking_indent: Option<usize> = None;
    let mut inline_type_checking_line: Option<u32> = None;

    for line in logical_lines {
        if type_checking_indent.is_some_and(|tc| line.indent <= tc) {
            type_checking_indent = None;
        }
        if inline_type_checking_line.is_some_and(|tc_line| line.start_line != tc_line) {
            inline_type_checking_line = None;
        }
        let slice = &tokens[line.tokens.clone()];
        if is_type_checking_guard(slice) {
            type_checking_indent = Some(line.indent);
            continue;
        }
        let is_runtime = type_checking_indent.is_none() && inline_type_checking_line.is_none();
        if let Some(colon_pos) = find_top_level_colon(slice)
            && colon_pos + 1 < slice.len()
        {
            let guard = is_type_checking_guard(&slice[..=colon_pos]);
            if guard {
                inline_type_checking_line = Some(line.end_line);
            }
            process_import_stmt(
                &slice[colon_pos + 1..],
                line.tokens.start + colon_pos + 1,
                is_runtime && !guard,
                &mut marked_indices,
                &mut runtime_imports,
            );
            continue;
        }
        process_import_stmt(
            slice,
            line.tokens.start,
            is_runtime,
            &mut marked_indices,
            &mut runtime_imports,
        );
    }

    (runtime_imports.into_iter().collect(), marked_indices)
}

fn process_import_stmt(
    slice: &[PyToken],
    token_offset: usize,
    is_runtime: bool,
    marked_indices: &mut BTreeSet<usize>,
    runtime_imports: &mut BTreeSet<String>,
) {
    let Some((kw_pos, is_from)) = import_stmt_head(slice) else {
        return;
    };
    marked_indices.extend(token_offset..token_offset + slice.len());
    if !is_runtime {
        return;
    }
    if is_from {
        parse_from_import_stmt(&slice[kw_pos + 1..], runtime_imports);
    } else {
        parse_import_stmt(&slice[kw_pos + 1..], runtime_imports);
    }
}

fn import_stmt_head(slice: &[PyToken]) -> Option<(usize, bool)> {
    let first = slice.first()?;
    if first.is_kw("import") {
        return Some((0, false));
    }
    if first.is_kw("from") {
        return Some((0, true));
    }
    if first.is_ident() && first.text == "lazy" {
        let second = slice.get(1)?;
        if second.is_kw("import") {
            return Some((1, false));
        }
        if second.is_kw("from") {
            return Some((1, true));
        }
    }
    None
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
        } else if tok.text == "*" {
            out.insert(dots.to_owned());
            idx += 1;
        } else {
            idx += 1;
        }
    }
}
