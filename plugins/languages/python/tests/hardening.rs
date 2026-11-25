use codeint_lang_python::Python;
use codeint_model::{LanguagePlugin, RepoPath, TokenKind};

fn lower(src: &str) -> codeint_model::FileModel {
    Python
        .lower(&RepoPath::new("src/sample.py").unwrap(), src)
        .expect("valid Python 3 source should lower")
}

fn assert_function_decisions(src: &str, expected: &[(Option<&str>, u32, u32)]) {
    let model = lower(src);
    let actual: Vec<(Option<&str>, u32, u32)> = model
        .functions
        .iter()
        .map(|f| (f.name.as_deref(), f.line, f.decision_points))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn pep701_fstrings_and_pep750_tstrings_parse_and_count_expression_decisions() {
    let src = r#"def render(items, flag, left, width):
    same_quote = f"{items["key"]}"
    nested_f = f"outer {f"inner {'yes' if flag else 'no'}"}"
    multiline = f"{
        # comment with " quote
        items.get('a') or items.get('b')
    }"
    debug_expr = f"{width + 1
        # debug comment
        = !r:>10}"
    fmt_nested = f"{width:{'<' if left else '>'}10}"
    fmt_special = f"{width:#x} {width:'<10}"
    named_escape = f"\N{SNOWMAN} {width}"
    regex_braces = rf"\${{\w+}}\{{{width if flag else 0}}}"
    backslash_brace = fr'\{{\}}' + tr'\{{\}}' + f'\{width}'
    tmpl = t"head {items.get('c') if flag else 'd'} tail"
    raw_tmpl = rt"raw {width} {f'x' if left else 'y'}"
    tr_tmpl = tr'''multi
        {width}
    '''
    return same_quote, nested_f, multiline, debug_expr, fmt_nested, fmt_special, named_escape, regex_braces, backslash_brace, tmpl, raw_tmpl, tr_tmpl
"#;

    let model = lower(src);
    assert_eq!(model.functions.len(), 1);
    let func = &model.functions[0];
    assert_eq!(func.name.as_deref(), Some("render"));
    assert_eq!(func.line, 1);
    assert_eq!(func.end_line, 21);
    assert_eq!(func.decision_points, 6);
}

#[test]
fn crlf_backslash_continuations_and_hex_literals_lex_cleanly() {
    let lf = "def calc():\n    a = 'hello \\\nworld'\n    b = r'raw \\\nline'\n    return 0x1e+1 + 0xfe-1 + len(a) + len(b)\n";
    let crlf = lf.replace('\n', "\r\n");

    let lf_model = lower(lf);
    let crlf_model = lower(&crlf);
    assert_eq!(lf_model.functions, crlf_model.functions);
    assert_eq!(lf_model.source_lines, crlf_model.source_lines);

    let expr_tokens = lower("x = 0x1e+1\n").tokens;
    assert_eq!(expr_tokens.len(), 5);
}

#[test]
fn multiline_string_at_end_of_function_updates_end_line_and_source_lines() {
    let src = r#"def documented():
    """
    Line one.
    Line two.
    """

def returns_block():
    return '''
        alpha
        beta
    '''
"#;

    let model = lower(src);
    assert_eq!(model.functions.len(), 2);
    assert_eq!(
        (
            model.functions[0].name.as_deref(),
            model.functions[0].line,
            model.functions[0].end_line,
            model.functions[0].source_lines,
        ),
        (Some("documented"), 1, 5, 5)
    );
    assert_eq!(
        (
            model.functions[1].name.as_deref(),
            model.functions[1].line,
            model.functions[1].end_line,
            model.functions[1].source_lines,
        ),
        (Some("returns_block"), 7, 11, 5)
    );
}

#[test]
fn lambda_boundaries_stop_before_comprehension_clauses_and_dict_colons() {
    let src = r#"async def build_callbacks(items, stream, a, b):
    sync_list = [lambda x=x: x + 1 for x in items]
    filtered = [lambda x=x: x if x > 0 else 0 for x in items if x is not None]
    async_list = [lambda x=x: x async for x in stream]
    mapping = {lambda x: x: a or b}
    curried = lambda x: lambda y: y if y > 0 else 0
    return sync_list, filtered, async_list, mapping, curried
"#;

    assert_function_decisions(
        src,
        &[
            (Some("build_callbacks"), 1, 5),
            (None, 2, 0),
            (None, 3, 1),
            (None, 4, 0),
            (None, 5, 0),
            (None, 6, 0),
            (None, 6, 1),
        ],
    );
}

#[test]
fn soft_keyword_case_and_single_line_wildcard_avoid_false_decision_points() {
    let src = r#"def handle(value, flag):
    case: int = 1
    case = lambda v: v
    match value:
        case 1: return 10
        case 2 if flag: return 20
        case _: return 0
"#;

    assert_function_decisions(src, &[(Some("handle"), 1, 3), (None, 3, 0)]);
}

#[test]
fn single_line_def_semicolon_suite_and_modern_python_311_313_syntax() {
    let src = r#"def inline_fn(x): y = x + 1; return y if y > 0 else 0

def generic_fn[T: (int, str)](
    items: list[T],
) -> list[T]:
    return [item for item in items if (n := len(str(item))) > 0]

async def handle_group[T](coro: T) -> None:
    try:
        await coro
    except* ValueError:
        pass
    except* KeyError:
        pass
"#;

    assert_function_decisions(
        src,
        &[
            (Some("inline_fn"), 1, 1),
            (Some("generic_fn"), 3, 2),
            (Some("handle_group"), 8, 2),
        ],
    );
}

#[test]
fn single_line_clause_imports_type_checking_and_pep810_lazy_imports() {
    let src = r#"lazy import json
lazy from pkg.sub import (
    alpha,
    beta,
)
from .. import (
    first as f_alias,
    second,
)
if sys_win: import msvcrt
if TYPE_CHECKING: import typing_only; from pkg.types import Secret
def f():
    return json
"#;

    let model = lower(src);
    assert_eq!(
        model.runtime_imports,
        ["..first", "..second", "json", "msvcrt", "pkg.sub"]
    );
    assert!(
        model
            .tokens
            .iter()
            .filter(|t| t.line <= 9)
            .all(|t| t.kind == TokenKind::ImportOrExport)
    );
    let line_10_kinds: Vec<TokenKind> = model
        .tokens
        .iter()
        .filter(|t| t.line == 10)
        .map(|t| t.kind)
        .collect();
    assert_eq!(
        line_10_kinds,
        [
            TokenKind::Other,
            TokenKind::Identifier,
            TokenKind::Other,
            TokenKind::ImportOrExport,
            TokenKind::ImportOrExport,
        ]
    );
}

#[test]
fn flagged_line_rules_eliminate_false_positives_and_keep_full_planted_defect_recall() {
    let clean = r#"def safe_elif_chain(a, b):
    if a:
        x = 1
    elif b:
        return 2
    else:
        x = 3
    return x

def safe_except_exempt():
    try:
        work()
    except ValueError:
        raise
    except Exception:
        recover()

def safe_except_star_exempt():
    try:
        work()
    except* ValueError:
        raise
    except* Exception:
        recover()

def safe_optional_return(items) -> int | None:
    for item in items:
        if item > 0:
            return item
    return None

def safe_untyped_optional_return(items):
    for item in items:
        if item > 0:
            return item
    return None

def safe_compound_ternary(flag, x, fallback):
    a = True if flag else False or fallback
    b = x == True if flag else False
    return a, b
"#;

    let clean_model = lower(clean);
    assert!(
        clean_model.flagged_lines.is_empty(),
        "expected zero false positives, got {:?}",
        clean_model.flagged_lines
    );

    let planted = r#"def planted_all(x, flag):
    if x > 1:
        return 1
    elif x == 1:
        return 0
    else:
        return -1

def planted_bool_return(x):
    if x > 0:
        return True
    else:
        return False

def planted_cond_bool(flag):
    return True if flag else False

def planted_reraise():
    try:
        work()
    except ValueError as err:
        raise err

def planted_trailing_none() -> None:
    work()
    return None
"#;

    let planted_model = lower(planted);
    let mut rules_hit: Vec<&str> = planted_model
        .flagged_lines
        .iter()
        .map(|fl| fl.rule.as_str())
        .collect();
    rules_hit.sort_unstable();
    rules_hit.dedup();
    assert_eq!(
        rules_hit,
        [
            "conditional-yields-boolean",
            "else-after-return",
            "except-only-reraises",
            "if-else-returns-boolean",
            "trailing-bare-return",
        ]
    );
}
