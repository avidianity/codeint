use std::collections::BTreeSet;

use codeint_model::language::Function;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{BinOp, Block, Expr, File, ImplItemFn, Item, ItemFn, TraitItemFn};

use crate::cfg_test::{has_cfg_test, item_attrs};

pub(crate) struct CodeLines {
    lines: BTreeSet<u32>,
}

impl CodeLines {
    pub(crate) fn new(text: &str, excluded_lines: &BTreeSet<u32>) -> CodeLines {
        let stripped = strip_rust_comments(text);
        let mut lines = BTreeSet::new();
        for (index, line) in stripped.lines().enumerate() {
            let line_number = (index + 1) as u32;
            if !excluded_lines.contains(&line_number) && line.chars().any(|ch| !ch.is_whitespace())
            {
                lines.insert(line_number);
            }
        }
        CodeLines { lines }
    }

    pub(crate) fn count(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn count_in_range(&self, start: u32, end: u32) -> u32 {
        self.lines.range(start..=end).count() as u32
    }

    pub(crate) fn lines_in_range(&self, start: u32, end: u32) -> impl Iterator<Item = u32> + '_ {
        self.lines.range(start..=end).copied()
    }
}

fn strip_rust_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut block_depth = 0usize;
    while i < bytes.len() {
        if block_depth > 0 {
            if bytes[i..].starts_with(b"/*") {
                block_depth += 1;
                out.push_str("  ");
                i += 2;
            } else if bytes[i..].starts_with(b"*/") {
                block_depth -= 1;
                out.push_str("  ");
                i += 2;
            } else if bytes[i] == b'\n' {
                out.push('\n');
                i += 1;
            } else {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            block_depth = 1;
            out.push_str("  ");
            i += 2;
            continue;
        }
        if let Some(next_i) = copy_raw_string(text, i, &mut out) {
            i = next_i;
            continue;
        }
        if bytes[i] == b'"' {
            i = copy_quoted(text, i, b'"', &mut out);
            continue;
        }
        if bytes[i] == b'\''
            && let Some(next_i) = try_copy_char_literal(text, i, &mut out)
        {
            i = next_i;
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn copy_raw_string(text: &str, start: usize, out: &mut String) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = start;
    if bytes[i] == b'b' || bytes[i] == b'c' {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'r' {
        return None;
    }
    i += 1;
    let hash_start = i;
    while i < bytes.len() && bytes[i] == b'#' {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'"' {
        return None;
    }
    let hashes = i - hash_start;
    i += 1;
    out.push_str(&text[start..i]);
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let mut matched = 0usize;
            while matched < hashes
                && i + 1 + matched < bytes.len()
                && bytes[i + 1 + matched] == b'#'
            {
                matched += 1;
            }
            if matched == hashes {
                let end = i + 1 + hashes;
                out.push_str(&text[i..end]);
                return Some(end);
            }
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    Some(i)
}

fn copy_quoted(text: &str, start: usize, quote: u8, out: &mut String) -> usize {
    let bytes = text.as_bytes();
    out.push(quote as char);
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next_ch = text[i + 1..].chars().next().unwrap_or(' ');
            out.push('\\');
            out.push(next_ch);
            i += 1 + next_ch.len_utf8();
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
        if ch as u32 == u32::from(quote) {
            break;
        }
    }
    i
}

fn try_copy_char_literal(text: &str, start: usize, out: &mut String) -> Option<usize> {
    let bytes = text.as_bytes();
    let i = start + 1;
    if i >= bytes.len() {
        return None;
    }
    if bytes[i] == b'\\' {
        let mut cursor = i + 1;
        while cursor < bytes.len() && cursor - start <= 12 {
            if bytes[cursor] == b'\'' {
                out.push_str(&text[start..=cursor]);
                return Some(cursor + 1);
            }
            if bytes[cursor] == b'\n' {
                return None;
            }
            cursor += 1;
        }
        return None;
    }
    let ch = text[i..].chars().next()?;
    let after = i + ch.len_utf8();
    if after < bytes.len() && bytes[after] == b'\'' {
        out.push_str(&text[start..=after]);
        return Some(after + 1);
    }
    None
}

pub(crate) fn functions(
    file: &File,
    code_lines: &CodeLines,
    exclude_cfg_test: bool,
) -> Vec<Function> {
    let mut collector = FunctionCollector {
        code_lines,
        exclude_cfg_test,
        functions: Vec::new(),
    };
    collector.visit_file(file);
    collector
        .functions
        .sort_by_key(|function| (function.line, function.end_line));
    collector.functions
}

struct FunctionCollector<'a> {
    code_lines: &'a CodeLines,
    exclude_cfg_test: bool,
    functions: Vec<Function>,
}

impl FunctionCollector<'_> {
    fn record_function(
        &mut self,
        start_line: u32,
        end_line: u32,
        name: Option<String>,
        block: &Block,
    ) {
        let mut counter = FunctionMetrics {
            decisions: 0,
            cognitive: 0,
            nesting: 0,
            name: name.clone(),
            counted_recursion: false,
            exclude_cfg_test: self.exclude_cfg_test,
            nested_collector: self,
        };
        counter.visit_block(block);
        let decision_points = counter.decisions;
        let cognitive_complexity = counter.cognitive;
        let source_lines = self.code_lines.count_in_range(start_line, end_line);
        self.functions.push(Function {
            line: start_line,
            end_line,
            decision_points,
            source_lines,
            name,
            cognitive_complexity,
        });
    }
}

impl<'ast> Visit<'ast> for FunctionCollector<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.exclude_cfg_test && has_cfg_test(item_attrs(item)) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        let start = node.sig.span().start().line as u32;
        let end = node.block.span().end().line as u32;
        self.record_function(start, end, Some(node.sig.ident.to_string()), &node.block);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        let start = node.sig.span().start().line as u32;
        let end = node.block.span().end().line as u32;
        self.record_function(start, end, Some(node.sig.ident.to_string()), &node.block);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast TraitItemFn) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if let Some(block) = &node.default {
            let start = node.sig.span().start().line as u32;
            let end = block.span().end().line as u32;
            self.record_function(start, end, Some(node.sig.ident.to_string()), block);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogicalKind {
    And,
    Or,
}

struct FunctionMetrics<'a, 'b> {
    decisions: u32,
    cognitive: u32,
    nesting: u32,
    name: Option<String>,
    counted_recursion: bool,
    exclude_cfg_test: bool,
    nested_collector: &'a mut FunctionCollector<'b>,
}

impl FunctionMetrics<'_, '_> {
    fn add_cognitive_nested(&mut self) {
        self.cognitive = self
            .cognitive
            .saturating_add(self.nesting.saturating_add(1));
    }

    fn with_nesting(&mut self, visit: impl FnOnce(&mut Self)) {
        self.nesting = self.nesting.saturating_add(1);
        visit(self);
        self.nesting = self.nesting.saturating_sub(1);
    }

    fn mark_recursion(&mut self) {
        if !self.counted_recursion {
            self.counted_recursion = true;
            self.cognitive = self.cognitive.saturating_add(1);
        }
    }

    fn visit_if_chain(&mut self, expr_if: &syn::ExprIf, is_else_if: bool) {
        self.decisions += 1;
        if is_else_if {
            self.cognitive = self.cognitive.saturating_add(1);
        } else {
            self.add_cognitive_nested();
        }
        self.visit_expr(&expr_if.cond);
        self.with_nesting(|metrics| metrics.visit_block(&expr_if.then_branch));
        if let Some((_, else_expr)) = &expr_if.else_branch {
            if let Expr::If(else_if) = else_expr.as_ref() {
                self.visit_if_chain(else_if, true);
            } else {
                self.cognitive = self.cognitive.saturating_add(1);
                self.with_nesting(|metrics| metrics.visit_expr(else_expr));
            }
        }
    }

    fn visit_logical_chain(&mut self, binary: &syn::ExprBinary) {
        let mut operators = Vec::new();
        let mut leaves = Vec::new();
        collect_logical_chain(binary, &mut operators, &mut leaves);
        self.decisions += operators.len() as u32;
        self.cognitive = self
            .cognitive
            .saturating_add(count_logical_runs(&operators));
        for leaf in leaves {
            self.visit_expr(leaf);
        }
    }
    fn visit_control_flow_expr(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::If(expr_if) => self.visit_if_chain(expr_if, false),
            Expr::While(expr_while) => {
                self.decisions += 1;
                self.add_cognitive_nested();
                self.visit_expr(&expr_while.cond);
                self.with_nesting(|metrics| metrics.visit_block(&expr_while.body));
            }
            Expr::ForLoop(expr_for) => {
                self.decisions += 1;
                self.add_cognitive_nested();
                self.visit_pat(&expr_for.pat);
                self.visit_expr(&expr_for.expr);
                self.with_nesting(|metrics| metrics.visit_block(&expr_for.body));
            }
            Expr::Loop(expr_loop) => {
                self.decisions += 1;
                self.add_cognitive_nested();
                self.with_nesting(|metrics| metrics.visit_block(&expr_loop.body));
            }
            Expr::Match(expr_match) => {
                self.decisions += expr_match.arms.len().saturating_sub(1) as u32;
                self.add_cognitive_nested();
                self.visit_expr(&expr_match.expr);
                self.with_nesting(|metrics| {
                    for arm in &expr_match.arms {
                        if arm.guard.is_some() {
                            metrics.decisions += 1;
                        }
                        metrics.visit_arm(arm);
                    }
                });
            }
            _ => return false,
        }
        true
    }

    fn visit_other_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Binary(binary) if matches!(binary.op, BinOp::And(_) | BinOp::Or(_)) => {
                self.visit_logical_chain(binary);
            }
            Expr::Closure(closure) => {
                self.with_nesting(|metrics| metrics.visit_expr(&closure.body));
            }
            Expr::Try(_) => {
                self.decisions += 1;
                visit::visit_expr(self, expr);
            }
            _ => self.visit_jump_or_call_expr(expr),
        }
    }

    fn visit_jump_or_call_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Break(expr_break) if expr_break.label.is_some() => {
                self.cognitive = self.cognitive.saturating_add(1);
            }
            Expr::Continue(expr_continue) if expr_continue.label.is_some() => {
                self.cognitive = self.cognitive.saturating_add(1);
            }
            Expr::Call(call) if is_direct_call(call, self.name.as_deref()) => {
                self.mark_recursion();
            }
            Expr::MethodCall(call) if is_direct_self_method_call(call, self.name.as_deref()) => {
                self.mark_recursion();
            }
            _ => {}
        }
        visit::visit_expr(self, expr);
    }
}

impl<'ast> Visit<'ast> for FunctionMetrics<'_, '_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.exclude_cfg_test && has_cfg_test(item_attrs(item)) {
            return;
        }
        self.nested_collector.visit_item(item);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if !self.visit_control_flow_expr(expr) {
            self.visit_other_expr(expr);
        }
    }
}

fn collect_logical_chain<'ast>(
    binary: &'ast syn::ExprBinary,
    operators: &mut Vec<LogicalKind>,
    leaves: &mut Vec<&'ast Expr>,
) {
    let Some(kind) = logical_kind(&binary.op) else {
        return;
    };
    collect_logical_operand(&binary.left, operators, leaves);
    operators.push(kind);
    collect_logical_operand(&binary.right, operators, leaves);
}

fn collect_logical_operand<'ast>(
    expr: &'ast Expr,
    operators: &mut Vec<LogicalKind>,
    leaves: &mut Vec<&'ast Expr>,
) {
    match expr {
        Expr::Paren(paren) => collect_logical_operand(&paren.expr, operators, leaves),
        Expr::Group(group) => collect_logical_operand(&group.expr, operators, leaves),
        Expr::Binary(binary) if logical_kind(&binary.op).is_some() => {
            collect_logical_chain(binary, operators, leaves);
        }
        other => leaves.push(other),
    }
}

fn logical_kind(op: &BinOp) -> Option<LogicalKind> {
    match op {
        BinOp::And(_) => Some(LogicalKind::And),
        BinOp::Or(_) => Some(LogicalKind::Or),
        _ => None,
    }
}

fn count_logical_runs(operators: &[LogicalKind]) -> u32 {
    if operators.is_empty() {
        return 0;
    }
    let switches = operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count();
    (switches + 1) as u32
}

fn is_direct_call(call: &syn::ExprCall, current_name: Option<&str>) -> bool {
    let Some(expected) = current_name else {
        return false;
    };
    let Expr::Path(expr_path) = call.func.as_ref() else {
        return false;
    };
    let segments = &expr_path.path.segments;
    match segments.len() {
        1 => segments[0].ident == expected,
        2 => segments[0].ident == "Self" && segments[1].ident == expected,
        _ => false,
    }
}

fn is_direct_self_method_call(call: &syn::ExprMethodCall, current_name: Option<&str>) -> bool {
    let Some(expected) = current_name else {
        return false;
    };
    if call.method != expected {
        return false;
    }
    matches!(
        call.receiver.as_ref(),
        Expr::Path(path) if path.path.is_ident("self")
    )
}
