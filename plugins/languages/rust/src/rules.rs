use std::collections::BTreeSet;

use codeint_model::language::FlaggedLine;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Arm, Block, Expr, ExprClosure, ExprIf, ExprMatch, File, ImplItemFn, Item, ItemFn, Lit, Pat,
    Stmt, TraitItemFn,
};

use crate::cfg_test::{has_cfg_test, item_attrs};
use crate::functions::CodeLines;

const RULE_IDS: [&str; 6] = [
    "else-after-return",
    "if-else-returns-boolean",
    "trailing-return",
    "match-only-propagates",
    "match-yields-boolean",
    "redundant-closure",
];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULE_IDS.into_iter()
}

pub(crate) fn flag(
    file: &File,
    code_lines: &CodeLines,
    exclude_cfg_test: bool,
) -> Vec<FlaggedLine> {
    let mut visitor = RuleVisitor {
        exclude_cfg_test,
        flagged: BTreeSet::new(),
        code_lines,
    };
    visitor.visit_file(file);
    visitor
        .flagged
        .into_iter()
        .map(|(line, rule)| FlaggedLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

struct RuleVisitor<'a> {
    exclude_cfg_test: bool,
    flagged: BTreeSet<(u32, &'static str)>,
    code_lines: &'a CodeLines,
}

impl RuleVisitor<'_> {
    fn flag_span(&mut self, span: proc_macro2::Span, rule: &'static str) {
        let start = span.start().line as u32;
        let end = span.end().line as u32;
        for line in self.code_lines.lines_in_range(start, end) {
            self.flagged.insert((line, rule));
        }
    }

    fn check_trailing_return(&mut self, block: &Block) {
        let Some(last) = block.stmts.last() else {
            return;
        };
        if let Stmt::Expr(Expr::Return(ret), _) = last {
            self.flag_span(ret.span(), "trailing-return");
        }
    }

    fn check_if(&mut self, expr_if: &ExprIf) {
        if let Some((_, else_branch)) = &expr_if.else_branch {
            if is_if_else_boolean(expr_if) {
                self.flag_span(expr_if.span(), "if-else-returns-boolean");
            } else if block_unconditionally_exits(&expr_if.then_branch) {
                self.flag_span(else_branch.span(), "else-after-return");
            }
        }
    }

    fn check_match(&mut self, expr_match: &ExprMatch) {
        if is_match_only_propagates(expr_match) {
            self.flag_span(expr_match.span(), "match-only-propagates");
        } else if is_match_yields_boolean(expr_match) {
            self.flag_span(expr_match.span(), "match-yields-boolean");
        }
    }

    fn check_closure(&mut self, closure: &ExprClosure) {
        if is_redundant_closure(closure) {
            self.flag_span(closure.span(), "redundant-closure");
        }
    }
}

impl<'ast> Visit<'ast> for RuleVisitor<'_> {
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
        self.check_trailing_return(&node.block);
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        self.check_trailing_return(&node.block);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast TraitItemFn) {
        if self.exclude_cfg_test && has_cfg_test(&node.attrs) {
            return;
        }
        if let Some(block) = &node.default {
            self.check_trailing_return(block);
        }
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_expr_if(&mut self, node: &'ast ExprIf) {
        self.check_if(node);
        visit::visit_expr_if(self, node);
    }

    fn visit_expr_match(&mut self, node: &'ast ExprMatch) {
        self.check_match(node);
        visit::visit_expr_match(self, node);
    }

    fn visit_expr_closure(&mut self, node: &'ast ExprClosure) {
        self.check_closure(node);
        visit::visit_expr_closure(self, node);
    }
}

fn block_unconditionally_exits(block: &Block) -> bool {
    let Some(last) = block.stmts.last() else {
        return false;
    };
    matches!(
        last,
        Stmt::Expr(Expr::Return(_) | Expr::Break(_) | Expr::Continue(_), _)
    )
}

fn single_bool_expr(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Bool(b) => Some(b.value),
            _ => None,
        },
        Expr::Return(ret) => ret.expr.as_deref().and_then(single_bool_expr),
        Expr::Block(block) => block_single_bool(&block.block),
        _ => None,
    }
}

fn block_single_bool(block: &Block) -> Option<bool> {
    if block.stmts.len() != 1 {
        return None;
    }
    match &block.stmts[0] {
        Stmt::Expr(expr, _) => single_bool_expr(expr),
        _ => None,
    }
}

fn is_if_else_boolean(expr_if: &ExprIf) -> bool {
    if matches!(*expr_if.cond, Expr::Let(_)) {
        return false;
    }
    let Some((_, else_expr)) = &expr_if.else_branch else {
        return false;
    };
    if matches!(**else_expr, Expr::If(_)) {
        return false;
    }
    match (
        block_single_bool(&expr_if.then_branch),
        single_bool_expr(else_expr),
    ) {
        (Some(then_val), Some(else_val)) => then_val != else_val,
        _ => false,
    }
}

fn is_match_only_propagates(expr_match: &ExprMatch) -> bool {
    if expr_match.arms.len() != 2 || expr_match.arms.iter().any(|arm| arm.guard.is_some()) {
        return false;
    }
    let (first, second) = (&expr_match.arms[0], &expr_match.arms[1]);
    (is_ok_unwrap_arm(first) && is_err_return_arm(second))
        || (is_err_return_arm(first) && is_ok_unwrap_arm(second))
        || (is_some_unwrap_arm(first) && is_none_return_arm(second))
        || (is_none_return_arm(first) && is_some_unwrap_arm(second))
}

fn tuple_variant_single_ident(pat: &Pat, variant: &str) -> Option<String> {
    let Pat::TupleStruct(ts) = pat else {
        return None;
    };
    let last_seg = ts.path.segments.last()?;
    if last_seg.ident != variant || ts.elems.len() != 1 {
        return None;
    }
    let Pat::Ident(ident) = &ts.elems[0] else {
        return None;
    };
    if ident.mutability.is_some() || ident.by_ref.is_some() || ident.subpat.is_some() {
        return None;
    }
    Some(ident.ident.to_string())
}

fn expr_single_ident(expr: &Expr) -> Option<String> {
    let Expr::Path(path) = expr else {
        return None;
    };
    path.path.get_ident().map(ToString::to_string)
}

fn is_ok_unwrap_arm(arm: &Arm) -> bool {
    let Some(bound) = tuple_variant_single_ident(&arm.pat, "Ok") else {
        return false;
    };
    expr_single_ident(&arm.body).is_some_and(|name| name == bound)
}

fn is_err_return_arm(arm: &Arm) -> bool {
    let Some(bound) = tuple_variant_single_ident(&arm.pat, "Err") else {
        return false;
    };
    let Expr::Return(ret) = &*arm.body else {
        return false;
    };
    let Some(Expr::Call(call)) = ret.expr.as_deref() else {
        return false;
    };
    let Expr::Path(func_path) = &*call.func else {
        return false;
    };
    let Some(last) = func_path.path.segments.last() else {
        return false;
    };
    last.ident == "Err"
        && call.args.len() == 1
        && expr_single_ident(&call.args[0]).is_some_and(|name| name == bound)
}

fn is_some_unwrap_arm(arm: &Arm) -> bool {
    let Some(bound) = tuple_variant_single_ident(&arm.pat, "Some") else {
        return false;
    };
    expr_single_ident(&arm.body).is_some_and(|name| name == bound)
}

fn is_none_return_arm(arm: &Arm) -> bool {
    let is_none_pat = match &arm.pat {
        Pat::Ident(ident) => ident.ident == "None",
        Pat::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "None"),
        _ => false,
    };
    if !is_none_pat {
        return false;
    }
    let Expr::Return(ret) = &*arm.body else {
        return false;
    };
    let Some(Expr::Path(path)) = ret.expr.as_deref() else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "None")
}

fn is_match_yields_boolean(expr_match: &ExprMatch) -> bool {
    if expr_match.arms.len() != 2 || expr_match.arms.iter().any(|arm| arm.guard.is_some()) {
        return false;
    }
    let first_pat_bool =
        matches!(&expr_match.arms[0].pat, Pat::Lit(lit) if matches!(lit.lit, Lit::Bool(_)));
    let second_pat_bool_or_wild = matches!(
        &expr_match.arms[1].pat,
        Pat::Wild(_)
            | Pat::Lit(syn::ExprLit {
                lit: Lit::Bool(_),
                ..
            })
    );
    if !(first_pat_bool && second_pat_bool_or_wild) {
        return false;
    }
    match (
        single_bool_expr(&expr_match.arms[0].body),
        single_bool_expr(&expr_match.arms[1].body),
    ) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    }
}

fn is_redundant_closure(closure: &ExprClosure) -> bool {
    if closure.asyncness.is_some()
        || closure.capture.is_some()
        || closure.lifetimes.is_some()
        || closure.inputs.is_empty()
    {
        return false;
    }
    let mut params = Vec::with_capacity(closure.inputs.len());
    for input in &closure.inputs {
        let Pat::Ident(ident) = input else {
            return false;
        };
        if ident.mutability.is_some() || ident.by_ref.is_some() || ident.subpat.is_some() {
            return false;
        }
        params.push(ident.ident.to_string());
    }
    let Expr::Call(call) = &*closure.body else {
        return false;
    };
    if !matches!(&*call.func, Expr::Path(_)) || call.args.len() != params.len() {
        return false;
    }
    call.args
        .iter()
        .zip(&params)
        .all(|(arg, param)| expr_single_ident(arg).is_some_and(|name| name == *param))
}
